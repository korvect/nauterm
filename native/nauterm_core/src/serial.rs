#[cfg(unix)]
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Read, Write};
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Serialize;

use crate::output_queue::{append_output, clear_output, drain_output_chunk};
use crate::pty::WakeupCallback;

const SERIAL_READ_BUFFER_SIZE: usize = 16 * 1024;
const SERIAL_POLL_INTERVAL: Duration = Duration::from_millis(8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialParity {
    None,
    Even,
    Odd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialFlowControl {
    None,
    Software,
    Hardware,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SerialConfig {
    pub baud_rate: u32,
    pub data_bits: u8,
    pub parity: SerialParity,
    pub stop_bits: u8,
    pub flow_control: SerialFlowControl,
}

#[derive(Clone, Debug, Serialize)]
pub struct SerialPortInfo {
    pub path: String,
    pub display_name: String,
    pub description: Option<String>,
}

impl SerialConfig {
    pub fn new(
        baud_rate: u32,
        data_bits: u8,
        parity: SerialParity,
        stop_bits: u8,
        flow_control: SerialFlowControl,
    ) -> io::Result<Self> {
        if !(5..=8).contains(&data_bits) {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!("unsupported data bits {data_bits}; choose 5, 6, 7, or 8"),
            ));
        }
        if stop_bits != 1 && stop_bits != 2 {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!("unsupported stop bits {stop_bits}; choose 1 or 2"),
            ));
        }

        Ok(Self {
            baud_rate,
            data_bits,
            parity,
            stop_bits,
            flow_control,
        })
    }

    pub fn summary(&self) -> String {
        let parity = match self.parity {
            SerialParity::None => "N",
            SerialParity::Even => "E",
            SerialParity::Odd => "O",
        };
        let flow = match self.flow_control {
            SerialFlowControl::None => "none",
            SerialFlowControl::Software => "software",
            SerialFlowControl::Hardware => "hardware",
        };
        format!(
            "{} baud, {}{}{}, flow {}",
            self.baud_rate, self.data_bits, parity, self.stop_bits, flow
        )
    }
}

pub struct SerialTransport {
    path: String,
    config: SerialConfig,
    sender: Sender<SerialCommand>,
    exited: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    error: Arc<Mutex<Option<String>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
    worker: Option<JoinHandle<()>>,
}

pub struct SerialPump {
    pub output: Vec<u8>,
    pub exited: bool,
    pub error: Option<String>,
    pub has_more: bool,
}

enum SerialCommand {
    Input(Vec<u8>),
    Shutdown,
}

impl SerialTransport {
    #[cfg(unix)]
    pub fn open(path: &str, config: SerialConfig) -> io::Result<Self> {
        let file = open_serial_file(path, config)?;
        let (sender, receiver) = mpsc::channel();
        let exited = Arc::new(AtomicBool::new(false));
        let output = Arc::new(Mutex::new(Vec::new()));
        let error = Arc::new(Mutex::new(None));
        let wakeup = Arc::new(Mutex::new(None));
        let worker = spawn_serial_worker(
            file,
            receiver,
            exited.clone(),
            output.clone(),
            error.clone(),
            wakeup.clone(),
        )?;

        Ok(Self {
            path: path.to_owned(),
            config,
            sender,
            exited,
            output,
            error,
            wakeup,
            worker: Some(worker),
        })
    }

    #[cfg(not(unix))]
    pub fn open(path: &str, _config: SerialConfig) -> io::Result<Self> {
        Err(io::Error::new(
            ErrorKind::Unsupported,
            format!("serial port {path} is not supported on this platform yet"),
        ))
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn config(&self) -> SerialConfig {
        self.config
    }

    pub fn queue_input(&mut self, bytes: &[u8]) -> bool {
        self.sender
            .send(SerialCommand::Input(bytes.to_vec()))
            .is_ok()
    }

    pub fn set_wakeup_callback(&mut self, callback: Option<WakeupCallback>) {
        if let Ok(mut wakeup) = self.wakeup.lock() {
            *wakeup = callback;
        }
        notify_wakeup(&self.wakeup);
    }

    pub fn drain_output(&mut self) -> SerialPump {
        let (output, has_more) = drain_output_chunk(&self.output);
        if has_more {
            notify_wakeup(&self.wakeup);
        }
        let exited = self.exited.load(Ordering::Acquire);
        let error = self.error.lock().ok().and_then(|mut error| error.take());

        SerialPump {
            output,
            exited,
            error,
            has_more,
        }
    }

    pub fn clear_pending_output(&mut self) {
        clear_output(&self.output);
    }
}

impl Drop for SerialTransport {
    fn drop(&mut self) {
        if let Ok(mut wakeup) = self.wakeup.lock() {
            *wakeup = None;
        }
        let _ = self.sender.send(SerialCommand::Shutdown);
        crate::pty::join_worker(&mut self.worker, "serial");
    }
}

#[cfg(unix)]
fn spawn_serial_worker(
    mut file: File,
    receiver: Receiver<SerialCommand>,
    exited: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    error: Arc<Mutex<Option<String>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("nauterm-serial".to_owned())
        .spawn(move || run_serial_worker(&mut file, receiver, exited, output, error, wakeup))
}

#[cfg(unix)]
fn run_serial_worker(
    file: &mut File,
    receiver: Receiver<SerialCommand>,
    exited: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    error: Arc<Mutex<Option<String>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
) {
    let mut pending_input = Vec::new();
    let mut buffer = [0; SERIAL_READ_BUFFER_SIZE];

    loop {
        if !drain_serial_commands(&receiver, &mut pending_input) {
            break;
        }
        if let Err(write_error) = flush_pending_input(file, &mut pending_input) {
            set_serial_error(&error, serial_runtime_error("write to", &write_error));
            break;
        }

        match file.read(&mut buffer) {
            Ok(0) => {}
            Ok(read) => {
                if append_output(&output, &buffer[..read]) {
                    notify_wakeup(&wakeup);
                }
            }
            Err(error) if is_transient_io(&error) => {}
            Err(read_error) => {
                set_serial_error(&error, serial_runtime_error("read from", &read_error));
                break;
            }
        }

        thread::sleep(SERIAL_POLL_INTERVAL);
    }

    exited.store(true, Ordering::Release);
    notify_wakeup(&wakeup);
}

#[cfg(unix)]
fn drain_serial_commands(receiver: &Receiver<SerialCommand>, pending_input: &mut Vec<u8>) -> bool {
    loop {
        match receiver.try_recv() {
            Ok(SerialCommand::Input(bytes)) => pending_input.extend_from_slice(&bytes),
            Ok(SerialCommand::Shutdown) | Err(TryRecvError::Disconnected) => return false,
            Err(TryRecvError::Empty) => return true,
        }
    }
}

#[cfg(unix)]
fn flush_pending_input(file: &mut File, pending_input: &mut Vec<u8>) -> io::Result<()> {
    while !pending_input.is_empty() {
        match file.write(pending_input) {
            Ok(0) => break,
            Ok(written) => {
                pending_input.drain(..written);
            }
            Err(error) if is_transient_io(&error) => break,
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

#[cfg(unix)]
fn open_serial_file(path: &str, config: SerialConfig) -> io::Result<File> {
    let serial_path = path.to_owned();
    let path = CString::new(path)
        .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "serial path contains NUL"))?;
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(serial_open_error(&serial_path, io::Error::last_os_error()));
    }

    let file = unsafe { File::from_raw_fd(fd) };
    claim_exclusive_serial_fd(file.as_raw_fd(), &serial_path)?;
    configure_serial_fd(file.as_raw_fd(), config)?;
    Ok(file)
}

#[cfg(unix)]
fn claim_exclusive_serial_fd(fd: i32, path: &str) -> io::Result<()> {
    let result = unsafe { libc::ioctl(fd, libc::TIOCEXCL as libc::c_ulong) };
    if result == 0 {
        return Ok(());
    }

    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::EBUSY) {
        return Err(io::Error::new(
            error.kind(),
            format!("serial port {path} is already in use"),
        ));
    }

    if matches!(
        error.raw_os_error(),
        Some(libc::EINVAL) | Some(libc::ENOTTY) | Some(libc::ENOSYS)
    ) {
        return Ok(());
    }

    Err(io::Error::new(
        error.kind(),
        format!("failed to reserve serial port {path}: {error}"),
    ))
}

#[cfg(unix)]
fn configure_serial_fd(fd: i32, config: SerialConfig) -> io::Result<()> {
    let mut termios = unsafe {
        let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
        if libc::tcgetattr(fd, termios.as_mut_ptr()) != 0 {
            return Err(io::Error::last_os_error());
        }
        termios.assume_init()
    };

    unsafe { libc::cfmakeraw(&mut termios) };
    termios.c_cflag |= libc::CLOCAL | libc::CREAD;
    termios.c_cflag &= !libc::CSIZE;
    termios.c_cflag |= data_bits_flag(config.data_bits)?;
    apply_parity(&mut termios, config.parity);
    apply_stop_bits(&mut termios, config.stop_bits)?;
    apply_flow_control(&mut termios, config.flow_control)?;
    termios.c_cc[libc::VMIN] = 0;
    termios.c_cc[libc::VTIME] = 0;

    let speed = baud_rate_to_speed(config.baud_rate).ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("unsupported baud rate {}", config.baud_rate),
        )
    })?;
    if unsafe { libc::cfsetispeed(&mut termios, speed) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::cfsetospeed(&mut termios, speed) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } != 0 {
        let error = io::Error::last_os_error();
        return Err(io::Error::new(
            error.kind(),
            format!("serial device rejected {}: {error}", config.summary()),
        ));
    }

    Ok(())
}

#[cfg(unix)]
fn data_bits_flag(data_bits: u8) -> io::Result<libc::tcflag_t> {
    match data_bits {
        5 => Ok(libc::CS5),
        6 => Ok(libc::CS6),
        7 => Ok(libc::CS7),
        8 => Ok(libc::CS8),
        _ => Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("unsupported data bits {data_bits}; choose 5, 6, 7, or 8"),
        )),
    }
}

#[cfg(unix)]
fn apply_parity(termios: &mut libc::termios, parity: SerialParity) {
    match parity {
        SerialParity::None => {
            termios.c_cflag &= !libc::PARENB;
            termios.c_cflag &= !libc::PARODD;
        }
        SerialParity::Even => {
            termios.c_cflag |= libc::PARENB;
            termios.c_cflag &= !libc::PARODD;
        }
        SerialParity::Odd => {
            termios.c_cflag |= libc::PARENB;
            termios.c_cflag |= libc::PARODD;
        }
    }
}

#[cfg(unix)]
fn apply_stop_bits(termios: &mut libc::termios, stop_bits: u8) -> io::Result<()> {
    match stop_bits {
        1 => termios.c_cflag &= !libc::CSTOPB,
        2 => termios.c_cflag |= libc::CSTOPB,
        _ => {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!("unsupported stop bits {stop_bits}; choose 1 or 2"),
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn apply_flow_control(
    termios: &mut libc::termios,
    flow_control: SerialFlowControl,
) -> io::Result<()> {
    match flow_control {
        SerialFlowControl::None => {
            termios.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
            clear_hardware_flow_control(termios);
        }
        SerialFlowControl::Software => {
            termios.c_iflag |= libc::IXON | libc::IXOFF;
            termios.c_iflag &= !libc::IXANY;
            clear_hardware_flow_control(termios);
        }
        SerialFlowControl::Hardware => {
            termios.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
            set_hardware_flow_control(termios)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn clear_hardware_flow_control(termios: &mut libc::termios) {
    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd"
    ))]
    {
        termios.c_cflag &= !libc::CRTSCTS;
    }
}

#[cfg(unix)]
fn set_hardware_flow_control(termios: &mut libc::termios) -> io::Result<()> {
    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd"
    ))]
    {
        termios.c_cflag |= libc::CRTSCTS;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(io::Error::new(
        ErrorKind::InvalidInput,
        "hardware flow control is not supported on this platform",
    ))
}

#[cfg(unix)]
fn serial_open_error(path: &str, error: io::Error) -> io::Error {
    let message = match error.kind() {
        ErrorKind::NotFound => format!("serial port {path} was not found"),
        ErrorKind::PermissionDenied => {
            format!("permission denied opening serial port {path}; check device permissions")
        }
        _ if error.raw_os_error() == Some(libc::EBUSY) => {
            format!("serial port {path} is already in use")
        }
        _ => format!("failed to open serial port {path}: {error}"),
    };
    io::Error::new(error.kind(), message)
}

#[cfg(unix)]
fn serial_runtime_error(operation: &str, error: &io::Error) -> String {
    match error.kind() {
        ErrorKind::PermissionDenied => {
            format!("Permission denied while trying to {operation} the serial device.")
        }
        ErrorKind::NotFound
        | ErrorKind::BrokenPipe
        | ErrorKind::UnexpectedEof
        | ErrorKind::ConnectionAborted
        | ErrorKind::ConnectionReset => {
            format!("Serial device disconnected while trying to {operation} it.")
        }
        _ if matches!(error.raw_os_error(), Some(libc::EIO) | Some(libc::ENODEV)) => {
            format!(
                "Serial device was unplugged or became unavailable while trying to {operation} it."
            )
        }
        _ => format!("Serial I/O error while trying to {operation} the device: {error}"),
    }
}

#[cfg(unix)]
fn set_serial_error(error_slot: &Arc<Mutex<Option<String>>>, message: String) {
    if let Ok(mut error_slot) = error_slot.lock() {
        if error_slot.is_none() {
            *error_slot = Some(message);
        }
    }
}

#[cfg(unix)]
fn baud_rate_to_speed(baud_rate: u32) -> Option<libc::speed_t> {
    Some(match baud_rate {
        1200 => libc::B1200,
        2400 => libc::B2400,
        4800 => libc::B4800,
        9600 => libc::B9600,
        19200 => libc::B19200,
        38400 => libc::B38400,
        57600 => libc::B57600,
        115200 => libc::B115200,
        #[cfg(any(
            target_os = "android",
            target_os = "freebsd",
            target_os = "ios",
            target_os = "linux",
            target_os = "macos",
            target_os = "netbsd"
        ))]
        230400 => libc::B230400,
        _ => return None,
    })
}

pub fn list_serial_ports() -> Vec<SerialPortInfo> {
    let mut ports: Vec<SerialPortInfo> = Vec::new();
    #[cfg(unix)]
    {
        collect_unix_serial_ports(&mut ports);
    }
    ports.sort_by(|left, right| left.path.cmp(&right.path));
    ports.dedup_by(|left, right| left.path == right.path);
    ports
}

#[cfg(unix)]
fn collect_unix_serial_ports(ports: &mut Vec<SerialPortInfo>) {
    let Ok(entries) = fs::read_dir("/dev") else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_serial_device_name(file_name) {
            continue;
        }
        let path_text = path.to_string_lossy().to_string();
        ports.push(SerialPortInfo {
            display_name: file_name.to_owned(),
            path: path_text,
            description: None,
        });
    }

    collect_serial_by_id_ports(ports);
}

#[cfg(unix)]
fn collect_serial_by_id_ports(ports: &mut Vec<SerialPortInfo>) {
    let by_id = Path::new("/dev/serial/by-id");
    let Ok(entries) = fs::read_dir(by_id) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| normalize_path(&path));
        ports.push(SerialPortInfo {
            path: canonical.to_string_lossy().to_string(),
            display_name: file_name.to_owned(),
            description: Some(path.to_string_lossy().to_string()),
        });
    }
}

#[cfg(unix)]
fn normalize_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    Path::new("/dev").join(path)
}

#[cfg(unix)]
fn is_serial_device_name(name: &str) -> bool {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        name.starts_with("cu.") || name.starts_with("tty.")
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        name.starts_with("ttyUSB")
            || name.starts_with("ttyACM")
            || name.starts_with("ttyAMA")
            || name.starts_with("ttyS")
            || name.starts_with("ttyTHS")
            || name.starts_with("rfcomm")
    }
}

fn notify_wakeup(wakeup: &Arc<Mutex<Option<WakeupCallback>>>) {
    if let Ok(wakeup) = wakeup.lock() {
        if let Some(callback) = *wakeup {
            callback.call();
        }
    }
}

#[cfg(unix)]
fn is_transient_io(error: &io::Error) -> bool {
    matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock)
}
