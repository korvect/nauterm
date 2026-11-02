use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use encoding_rs::Encoding;

use crate::output_queue::{append_output, clear_output, drain_output_chunk};
use crate::pty::WakeupCallback;
use crate::session::SessionEvent;
use crate::terminal::{TerminalGeometry, TerminalOptions};

mod codec;
mod protocol;
mod socket;

use codec::{decode_network_output, encoding_from_label};
use protocol::TelnetProtocol;
use socket::{connect_tcp, is_transient_io, telnet_runtime_error};

const TELNET_READ_BUFFER_SIZE: usize = 16 * 1024;
const TELNET_POLL_INTERVAL: Duration = Duration::from_millis(8);

pub struct TelnetTransport {
    sender: Sender<TelnetCommand>,
    exited: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

pub struct TelnetPump {
    pub output: Vec<u8>,
    pub events: Vec<SessionEvent>,
    pub exited: bool,
    pub has_more: bool,
}

enum TelnetCommand {
    Input(Vec<u8>),
    Resize(TerminalGeometry),
    Shutdown,
}

struct TelnetTarget {
    host: String,
    port: u16,
    geometry: TerminalGeometry,
    terminal_options: TerminalOptions,
    encoding: String,
}

impl TelnetTransport {
    pub fn connect(
        geometry: TerminalGeometry,
        terminal_options: TerminalOptions,
        host: &str,
        port: u16,
        encoding: &str,
    ) -> io::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let exited = Arc::new(AtomicBool::new(false));
        let output = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let wakeup = Arc::new(Mutex::new(None));
        let shutdown = Arc::new(AtomicBool::new(false));
        let target = TelnetTarget {
            host: host.to_owned(),
            port,
            geometry,
            terminal_options,
            encoding: encoding.to_owned(),
        };
        let worker = spawn_telnet_worker(
            target,
            receiver,
            exited.clone(),
            output.clone(),
            events.clone(),
            wakeup.clone(),
            shutdown.clone(),
        )?;

        Ok(Self {
            sender,
            exited,
            output,
            events,
            wakeup,
            shutdown,
            worker: Some(worker),
        })
    }

    pub fn resize(&mut self, geometry: TerminalGeometry) {
        let _ = self.sender.send(TelnetCommand::Resize(geometry));
    }

    pub fn queue_input(&mut self, bytes: &[u8]) -> bool {
        self.sender
            .send(TelnetCommand::Input(bytes.to_vec()))
            .is_ok()
    }

    pub fn set_wakeup_callback(&mut self, callback: Option<WakeupCallback>) {
        if let Ok(mut wakeup) = self.wakeup.lock() {
            *wakeup = callback;
        }
        notify_wakeup(&self.wakeup);
    }

    pub fn drain_output(&mut self) -> TelnetPump {
        let exited = self.exited.load(Ordering::Acquire);
        let (output, has_more) = drain_output_chunk(&self.output);
        if has_more {
            notify_wakeup(&self.wakeup);
        }
        let events = self
            .events
            .lock()
            .map(|mut events| events.drain(..).collect())
            .unwrap_or_default();

        TelnetPump {
            output,
            events,
            exited,
            has_more,
        }
    }

    pub fn clear_pending_output(&mut self) {
        clear_output(&self.output);
    }
}

impl Drop for TelnetTransport {
    fn drop(&mut self) {
        if let Ok(mut wakeup) = self.wakeup.lock() {
            *wakeup = None;
        }
        self.shutdown.store(true, Ordering::Release);
        let _ = self.sender.send(TelnetCommand::Shutdown);
        crate::pty::join_worker(&mut self.worker, "Telnet");
    }
}

fn spawn_telnet_worker(
    target: TelnetTarget,
    receiver: Receiver<TelnetCommand>,
    exited: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
    shutdown: Arc<AtomicBool>,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("nauterm-telnet".to_owned())
        .spawn(move || {
            run_telnet_worker(target, receiver, exited, output, events, wakeup, shutdown)
        })
}

fn run_telnet_worker(
    target: TelnetTarget,
    receiver: Receiver<TelnetCommand>,
    exited: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
    shutdown: Arc<AtomicBool>,
) {
    push_event(
        &events,
        &wakeup,
        SessionEvent::new(
            "connect_start",
            format!("Connecting to telnet://{}:{}.", target.host, target.port),
        )
        .with_host_port(&target.host, target.port),
    );

    match run_telnet_session(
        target,
        receiver,
        output.clone(),
        events.clone(),
        wakeup.clone(),
        shutdown.clone(),
    ) {
        Ok(()) => {}
        Err(error) => {
            push_event(
                &events,
                &wakeup,
                SessionEvent::new("error", format!("Telnet session ended: {error}")),
            );
        }
    }

    exited.store(true, Ordering::Release);
    notify_wakeup(&wakeup);
}

fn run_telnet_session(
    target: TelnetTarget,
    receiver: Receiver<TelnetCommand>,
    output: Arc<Mutex<Vec<u8>>>,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
    shutdown: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut stream = connect_tcp(&target.host, target.port, &shutdown)?;
    stream
        .set_nonblocking(true)
        .map_err(|error| format!("failed to set telnet socket non-blocking: {error}"))?;
    let _ = stream.set_nodelay(true);

    push_event(
        &events,
        &wakeup,
        SessionEvent::new(
            "connected",
            format!("Connected to telnet://{}:{}.", target.host, target.port),
        )
        .with_host_port(&target.host, target.port),
    );

    let encoding = encoding_from_label(&target.encoding);
    let mut decoder = encoding.new_decoder();
    let mut protocol = TelnetProtocol::new(target.geometry, target.terminal_options);
    let mut pending_input = protocol.initial_commands();
    let mut buffer = [0; TELNET_READ_BUFFER_SIZE];

    loop {
        if !drain_telnet_commands(&receiver, &mut pending_input, &mut protocol, encoding) {
            break;
        }
        if let Err(write_error) = flush_pending_input(&mut stream, &mut pending_input) {
            return Err(telnet_runtime_error("write to", &write_error));
        }

        match stream.read(&mut buffer) {
            Ok(0) => {
                push_event(
                    &events,
                    &wakeup,
                    SessionEvent::new("session_closed", "Telnet session closed.")
                        .with_host_port(&target.host, target.port),
                );
                break;
            }
            Ok(read) => {
                let response = protocol.receive(&buffer[..read]);
                if !response.commands.is_empty() {
                    pending_input.extend_from_slice(&response.commands);
                }
                if !response.output.is_empty() {
                    let decoded = decode_network_output(&mut decoder, &response.output);
                    if !decoded.is_empty() {
                        push_output(&output, &wakeup, &decoded);
                    }
                }
            }
            Err(error) if is_transient_io(&error) => {}
            Err(read_error) => {
                return Err(telnet_runtime_error("read from", &read_error));
            }
        }

        thread::sleep(TELNET_POLL_INTERVAL);
    }

    Ok(())
}

fn drain_telnet_commands(
    receiver: &Receiver<TelnetCommand>,
    pending_input: &mut Vec<u8>,
    protocol: &mut TelnetProtocol,
    encoding: &'static Encoding,
) -> bool {
    loop {
        match receiver.try_recv() {
            Ok(TelnetCommand::Input(bytes)) => {
                pending_input.extend(protocol.encode_input(&bytes, encoding));
            }
            Ok(TelnetCommand::Resize(geometry)) => {
                pending_input.extend(protocol.resize(geometry));
            }
            Ok(TelnetCommand::Shutdown) | Err(TryRecvError::Disconnected) => return false,
            Err(TryRecvError::Empty) => return true,
        }
    }
}

fn flush_pending_input(stream: &mut TcpStream, pending_input: &mut Vec<u8>) -> io::Result<()> {
    while !pending_input.is_empty() {
        match stream.write(pending_input) {
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

fn push_output(
    output: &Arc<Mutex<Vec<u8>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    bytes: &[u8],
) {
    if append_output(output, bytes) {
        notify_wakeup(wakeup);
    }
}

fn push_event(
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    event: SessionEvent,
) {
    if let Ok(mut events) = events.lock() {
        events.push(event);
    }
    notify_wakeup(wakeup);
}

fn notify_wakeup(wakeup: &Arc<Mutex<Option<WakeupCallback>>>) {
    if let Ok(wakeup) = wakeup.lock() {
        if let Some(callback) = *wakeup {
            callback.call();
        }
    }
}
