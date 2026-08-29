use std::env;
use std::ffi::{c_char, c_void, CStr, CString};
use std::io;
use std::path::PathBuf;
use std::slice;

use libloading::Library;
use serde::Deserialize;
use zeroize::Zeroize;

use crate::pty::WakeupCallback;
use crate::session::SessionEvent;
use crate::ssh::{start_mosh_server, HostKeyTrustMode, SshProxyConfig};
use crate::terminal::{TerminalEnvironmentVariable, TerminalGeometry, TerminalOptions};

const DEFAULT_MOSH_SERVER_COMMAND: &str = "mosh-server new -s -l LANG=en_US.UTF-8";
const DEFAULT_MOSH_SERVER_LOCALE: &str = "C.UTF-8";
const NAUTERM_MOSH_ABI_VERSION: u32 = 1;

type CreateTransportFn = unsafe extern "C" fn(
    columns: u16,
    rows: u16,
    host: *const c_char,
    ssh_port: u16,
    username: *const c_char,
    udp_port: u16,
    bootstrap_key: *const c_char,
) -> NautermMoshCreateResult;
type FreeTransportFn = unsafe extern "C" fn(handle: *mut c_void);
type QueueInputFn = unsafe extern "C" fn(handle: *mut c_void, bytes: *const u8, len: usize) -> u32;
type ResizeFn = unsafe extern "C" fn(handle: *mut c_void, columns: u16, rows: u16) -> bool;
type NotifyNetworkChangedFn = unsafe extern "C" fn(handle: *mut c_void) -> bool;
type SetWakeupFn = unsafe extern "C" fn(
    handle: *mut c_void,
    callback: Option<extern "C" fn(*mut c_void)>,
    user_data: *mut c_void,
);
type AbiVersionFn = unsafe extern "C" fn() -> u32;
type DrainFn = unsafe extern "C" fn(handle: *mut c_void) -> NautermMoshDrainResult;
type CommitScreenFn = unsafe extern "C" fn(handle: *mut c_void, state_num: u64) -> bool;
type ClearOutputFn = unsafe extern "C" fn(handle: *mut c_void);
type FreeBytesFn = unsafe extern "C" fn(ptr: *mut u8, len: usize, capacity: usize);
type FreeStringFn = unsafe extern "C" fn(ptr: *mut c_char);

#[repr(u32)]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputQueueStatus {
    Accepted = 0,
    Backpressure = 1,
    Closed = 2,
    Invalid = 3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MoshBootstrap {
    pub port: u16,
    pub key: String,
}

impl MoshBootstrap {
    pub(crate) fn parse(output: &str) -> Result<Self, MoshError> {
        for line in output.lines() {
            let mut fields = line.trim().split_ascii_whitespace();
            if fields.next() != Some("MOSH") || fields.next() != Some("CONNECT") {
                continue;
            }
            let port = fields
                .next()
                .ok_or(MoshError::InvalidBootstrap)?
                .parse::<u16>()
                .map_err(|_| MoshError::InvalidBootstrap)?;
            if port == 0 {
                return Err(MoshError::InvalidBootstrap);
            }
            let key = fields.next().ok_or(MoshError::InvalidBootstrap)?;
            if key.is_empty() || fields.next().is_some() {
                return Err(MoshError::InvalidBootstrap);
            }
            return Ok(Self {
                port,
                key: key.to_owned(),
            });
        }
        Err(MoshError::BootstrapMissing)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MoshServerCommand {
    pub command: String,
}

impl Default for MoshServerCommand {
    fn default() -> Self {
        Self {
            command: DEFAULT_MOSH_SERVER_COMMAND.to_owned(),
        }
    }
}

impl MoshServerCommand {
    pub(crate) fn render(&self) -> Result<String, MoshError> {
        let command = self.command.trim();
        if command.is_empty() || command.contains(['\r', '\n', '\0']) {
            return Err(MoshError::InvalidServerCommand);
        }
        Ok(command.to_owned())
    }

    pub(crate) fn render_for_terminal(
        &self,
        terminal_options: &TerminalOptions,
    ) -> Result<String, MoshError> {
        let command = self.render()?;
        if command != DEFAULT_MOSH_SERVER_COMMAND {
            return Ok(command);
        }
        Ok(default_mosh_server_command(terminal_options))
    }
}

pub(crate) struct MoshSessionTransport {
    library: MoshFfiLibrary,
    handle: *mut c_void,
    pending_events: Vec<SessionEvent>,
}

unsafe impl Send for MoshSessionTransport {}

pub(crate) struct MoshScreenBatch {
    pub state_num: u64,
    pub output: Vec<u8>,
}

#[allow(dead_code)]
pub(crate) struct MoshSessionPump {
    pub screen_batch: Option<MoshScreenBatch>,
    pub events: Vec<SessionEvent>,
    pub exited: bool,
    pub has_more: bool,
    pub queued_input_bytes: usize,
    pub queued_input_commands: usize,
    pub queued_screen_batches: usize,
    pub queued_screen_bytes: usize,
    pub deferred_screen_bytes: usize,
    pub dropped_unauthenticated_packets: u64,
}

struct MoshFfiLibrary {
    _library: Library,
    create_transport: CreateTransportFn,
    free_transport: FreeTransportFn,
    queue_input: QueueInputFn,
    resize: ResizeFn,
    notify_network_changed: NotifyNetworkChangedFn,
    set_wakeup_callback: SetWakeupFn,
    drain: DrainFn,
    commit_screen: CommitScreenFn,
    clear_output: ClearOutputFn,
    free_bytes: FreeBytesFn,
    free_string: FreeStringFn,
}

unsafe impl Send for MoshFfiLibrary {}

#[derive(Deserialize)]
struct ExternalSessionEvent {
    kind: String,
    message: String,
    host: Option<String>,
    port: Option<u16>,
    serial_port: Option<String>,
    baud_rate: Option<u32>,
    username: Option<String>,
    fingerprint: Option<String>,
    method: Option<String>,
    exit_status: Option<u32>,
    latency_ms: Option<u32>,
    state_num: Option<u64>,
}

#[repr(C)]
struct NautermMoshCreateResult {
    struct_size: usize,
    abi_version: u32,
    handle: *mut c_void,
    error_ptr: *mut c_char,
}

#[repr(C)]
struct NautermMoshDrainResult {
    struct_size: usize,
    abi_version: u32,
    output_ptr: *mut u8,
    output_len: usize,
    output_capacity: usize,
    state_num: u64,
    has_screen_batch: bool,
    events_json_ptr: *mut c_char,
    exited: bool,
    has_more: bool,
    queued_input_bytes: usize,
    queued_input_commands: usize,
    queued_screen_batches: usize,
    queued_screen_bytes: usize,
    deferred_screen_bytes: usize,
    dropped_unauthenticated_packets: u64,
}

#[derive(Debug)]
pub(crate) enum MoshError {
    InvalidServerCommand,
    InvalidBootstrap,
    BootstrapMissing,
}

impl std::fmt::Display for MoshError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidServerCommand => {
                formatter.write_str("The mosh-server command is invalid.")
            }
            Self::InvalidBootstrap => {
                formatter.write_str("The mosh-server bootstrap output is invalid.")
            }
            Self::BootstrapMissing => {
                formatter.write_str("The mosh-server bootstrap output is missing.")
            }
        }
    }
}

impl std::error::Error for MoshError {}

pub(crate) struct MoshConnectError {
    pub message: String,
    pub events: Vec<SessionEvent>,
}

impl From<io::Error> for MoshConnectError {
    fn from(error: io::Error) -> Self {
        Self {
            message: error.to_string(),
            events: Vec::new(),
        }
    }
}

impl MoshSessionTransport {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn connect(
        terminal_options: &TerminalOptions,
        geometry: TerminalGeometry,
        host: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
        private_key: Option<&str>,
        certificate: Option<&str>,
        passphrase: Option<&str>,
        known_hosts_path: Option<&str>,
        host_key_trust_mode: HostKeyTrustMode,
        proxy: Option<SshProxyConfig>,
        server_command: MoshServerCommand,
    ) -> Result<Self, MoshConnectError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| MoshConnectError {
                message: format!("failed to start Mosh SSH bootstrap runtime: {error}"),
                events: Vec::new(),
            })?;
        let bootstrap = match runtime.block_on(start_mosh_server(
            host,
            port,
            username,
            password,
            private_key,
            certificate,
            passphrase,
            known_hosts_path,
            proxy.as_ref(),
            host_key_trust_mode,
            &server_command,
            terminal_options,
        )) {
            Ok(bootstrap) => bootstrap,
            Err((message, events)) => {
                return Err(MoshConnectError { message, events });
            }
        };
        let library = MoshFfiLibrary::load()?;
        let native_host = CString::new(bootstrap.udp_host.clone())
            .map_err(|error| io::Error::other(error.to_string()))?;
        let native_username =
            CString::new(username).map_err(|error| io::Error::other(error.to_string()))?;
        let columns = geometry_dimension(geometry.columns)?;
        let rows = geometry_dimension(geometry.rows)?;
        let mut bootstrap_key = bootstrap.bootstrap.key.into_bytes();
        if bootstrap_key.contains(&0) {
            bootstrap_key.zeroize();
            return Err(io::Error::other("invalid Mosh bootstrap key").into());
        }
        bootstrap_key.push(0);
        let result = unsafe {
            (library.create_transport)(
                columns,
                rows,
                native_host.as_ptr(),
                port,
                native_username.as_ptr(),
                bootstrap.bootstrap.port,
                bootstrap_key.as_ptr() as *const c_char,
            )
        };
        bootstrap_key.zeroize();
        validate_create_result(&result)?;
        if result.handle.is_null() {
            let error = library.take_string(result.error_ptr);
            return Err(io::Error::other(if error.is_empty() {
                "failed to create Mosh transport".to_owned()
            } else {
                error
            })
            .into());
        }
        let mut events = bootstrap.events;
        events.push(
            SessionEvent::new(
                "connected",
                format!(
                    "SSH connection established and Mosh session connected to {username}@{host}."
                ),
            )
            .with_host_port(host, port)
            .with_username(username),
        );
        Ok(Self {
            library,
            handle: result.handle,
            pending_events: events,
        })
    }

    pub(crate) fn resize(&mut self, geometry: TerminalGeometry) {
        if let (Ok(columns), Ok(rows)) = (
            geometry_dimension(geometry.columns),
            geometry_dimension(geometry.rows),
        ) {
            unsafe {
                (self.library.resize)(self.handle, columns, rows);
            }
        }
    }

    pub(crate) fn notify_network_changed(&mut self) -> bool {
        unsafe { (self.library.notify_network_changed)(self.handle) }
    }

    pub(crate) fn queue_input_status(&mut self, bytes: &[u8]) -> u32 {
        unsafe { (self.library.queue_input)(self.handle, bytes.as_ptr(), bytes.len()) }
    }

    pub(crate) fn queue_input(&mut self, bytes: &[u8]) -> bool {
        self.queue_input_status(bytes) == InputQueueStatus::Accepted as u32
    }

    pub(crate) fn set_wakeup_callback(&mut self, callback: Option<WakeupCallback>) {
        let (callback, user_data) = match callback {
            Some(callback) => {
                let (callback_fn, user_data) = callback.raw_parts();
                (Some(callback_fn), user_data)
            }
            None => (None, std::ptr::null_mut()),
        };
        unsafe {
            (self.library.set_wakeup_callback)(self.handle, callback, user_data);
        }
    }

    pub(crate) fn drain_output(&mut self) -> MoshSessionPump {
        let result = unsafe { (self.library.drain)(self.handle) };
        if result.struct_size != std::mem::size_of::<NautermMoshDrainResult>()
            || result.abi_version != NAUTERM_MOSH_ABI_VERSION
        {
            return MoshSessionPump {
                screen_batch: None,
                events: vec![SessionEvent {
                    kind: "error".to_owned(),
                    message: format!(
                        "nauterm_mosh_ffi ABI mismatch while draining output (version {}, struct size {}).",
                        result.abi_version, result.struct_size
                    ),
                    host: None,
                    port: None,
                    serial_port: None,
                    baud_rate: None,
                    username: None,
                    fingerprint: None,
                    method: None,
                    exit_status: None,
                    latency_ms: None,
                    state_num: None,
                }],
                exited: true,
                has_more: false,
                queued_input_bytes: 0,
                queued_input_commands: 0,
                queued_screen_batches: 0,
                queued_screen_bytes: 0,
                deferred_screen_bytes: 0,
                dropped_unauthenticated_packets: 0,
            };
        }
        let mut events = std::mem::take(&mut self.pending_events);
        if !result.events_json_ptr.is_null() {
            let json = self.library.take_string(result.events_json_ptr);
            if !json.is_empty() {
                if let Ok(external) = serde_json::from_str::<Vec<ExternalSessionEvent>>(&json) {
                    events.extend(external.into_iter().map(session_event_from_external));
                }
            }
        }
        let mut invalid_screen_batch = false;
        let screen_batch = if result.has_screen_batch {
            if result.output_ptr.is_null() {
                if result.output_len != 0 || result.output_capacity != 0 {
                    invalid_screen_batch = true;
                    events.push(SessionEvent {
                        kind: "error".to_owned(),
                        message: "nauterm_mosh_ffi returned an invalid empty screen batch pointer."
                            .to_owned(),
                        host: None,
                        port: None,
                        serial_port: None,
                        baud_rate: None,
                        username: None,
                        fingerprint: None,
                        method: None,
                        exit_status: None,
                        latency_ms: None,
                        state_num: Some(result.state_num),
                    });
                    None
                } else {
                    Some(MoshScreenBatch {
                        state_num: result.state_num,
                        output: Vec::new(),
                    })
                }
            } else if result.output_len > result.output_capacity {
                invalid_screen_batch = true;
                events.push(SessionEvent {
                    kind: "error".to_owned(),
                    message: "nauterm_mosh_ffi returned an invalid screen batch length.".to_owned(),
                    host: None,
                    port: None,
                    serial_port: None,
                    baud_rate: None,
                    username: None,
                    fingerprint: None,
                    method: None,
                    exit_status: None,
                    latency_ms: None,
                    state_num: Some(result.state_num),
                });
                None
            } else {
                let output =
                    unsafe { slice::from_raw_parts(result.output_ptr, result.output_len) }.to_vec();
                unsafe {
                    (self.library.free_bytes)(
                        result.output_ptr,
                        result.output_len,
                        result.output_capacity,
                    );
                }
                Some(MoshScreenBatch {
                    state_num: result.state_num,
                    output,
                })
            }
        } else if !result.output_ptr.is_null()
            || result.output_len != 0
            || result.output_capacity != 0
        {
            invalid_screen_batch = true;
            events.push(SessionEvent {
                kind: "error".to_owned(),
                message: "nauterm_mosh_ffi returned output without a screen batch.".to_owned(),
                host: None,
                port: None,
                serial_port: None,
                baud_rate: None,
                username: None,
                fingerprint: None,
                method: None,
                exit_status: None,
                latency_ms: None,
                state_num: None,
            });
            None
        } else {
            None
        };
        MoshSessionPump {
            screen_batch,
            events,
            exited: result.exited || invalid_screen_batch,
            has_more: result.has_more,
            queued_input_bytes: result.queued_input_bytes,
            queued_input_commands: result.queued_input_commands,
            queued_screen_batches: result.queued_screen_batches,
            queued_screen_bytes: result.queued_screen_bytes,
            deferred_screen_bytes: result.deferred_screen_bytes,
            dropped_unauthenticated_packets: result.dropped_unauthenticated_packets,
        }
    }

    pub(crate) fn commit_screen(&mut self, state_num: u64) -> bool {
        unsafe { (self.library.commit_screen)(self.handle, state_num) }
    }

    pub(crate) fn clear_pending_output(&mut self) {
        unsafe {
            (self.library.clear_output)(self.handle);
        }
    }
}

impl Drop for MoshSessionTransport {
    fn drop(&mut self) {
        unsafe {
            (self.library.free_transport)(self.handle);
        }
    }
}

impl MoshFfiLibrary {
    fn load() -> io::Result<Self> {
        let mut errors = Vec::new();
        for candidate in mosh_library_candidates() {
            let library = unsafe { Library::new(&candidate) };
            let library = match library {
                Ok(library) => library,
                Err(error) => {
                    errors.push(format!("{candidate}: {error}"));
                    continue;
                }
            };
            let loaded = unsafe { Self::from_library(library) };
            match loaded {
                Ok(library) => return Ok(library),
                Err(error) => errors.push(format!("{candidate}: {error}")),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Unable to load nauterm_mosh_ffi native library.\n{}",
                errors.join("\n")
            ),
        ))
    }

    unsafe fn from_library(library: Library) -> io::Result<Self> {
        let abi_version = *library
            .get::<AbiVersionFn>(b"nauterm_mosh_abi_version\0")
            .map_err(io::Error::other)?;
        let reported_abi_version = abi_version();
        if reported_abi_version != NAUTERM_MOSH_ABI_VERSION {
            return Err(io::Error::other(format!(
                "nauterm_mosh_ffi ABI version mismatch: expected {}, got {}",
                NAUTERM_MOSH_ABI_VERSION, reported_abi_version
            )));
        }
        Ok(Self {
            create_transport: *library
                .get::<CreateTransportFn>(b"nauterm_mosh_transport_create\0")
                .map_err(io::Error::other)?,
            free_transport: *library
                .get::<FreeTransportFn>(b"nauterm_mosh_transport_free\0")
                .map_err(io::Error::other)?,
            queue_input: *library
                .get::<QueueInputFn>(b"nauterm_mosh_transport_queue_input\0")
                .map_err(io::Error::other)?,
            resize: *library
                .get::<ResizeFn>(b"nauterm_mosh_transport_resize\0")
                .map_err(io::Error::other)?,
            notify_network_changed: *library
                .get::<NotifyNetworkChangedFn>(b"nauterm_mosh_transport_notify_network_changed\0")
                .map_err(io::Error::other)?,
            set_wakeup_callback: *library
                .get::<SetWakeupFn>(b"nauterm_mosh_transport_set_wakeup_callback\0")
                .map_err(io::Error::other)?,
            drain: *library
                .get::<DrainFn>(b"nauterm_mosh_transport_drain\0")
                .map_err(io::Error::other)?,
            commit_screen: *library
                .get::<CommitScreenFn>(b"nauterm_mosh_transport_commit_screen\0")
                .map_err(io::Error::other)?,
            clear_output: *library
                .get::<ClearOutputFn>(b"nauterm_mosh_transport_clear_pending_output\0")
                .map_err(io::Error::other)?,
            free_bytes: *library
                .get::<FreeBytesFn>(b"nauterm_mosh_bytes_free\0")
                .map_err(io::Error::other)?,
            free_string: *library
                .get::<FreeStringFn>(b"nauterm_mosh_string_free\0")
                .map_err(io::Error::other)?,
            _library: library,
        })
    }

    fn take_string(&self, ptr: *mut c_char) -> String {
        if ptr.is_null() {
            return String::new();
        }
        let text = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe {
            (self.free_string)(ptr);
        }
        text
    }
}

fn validate_create_result(result: &NautermMoshCreateResult) -> io::Result<()> {
    if result.struct_size != std::mem::size_of::<NautermMoshCreateResult>() {
        return Err(io::Error::other(format!(
            "nauterm_mosh_ffi create result size mismatch: expected {}, got {}",
            std::mem::size_of::<NautermMoshCreateResult>(),
            result.struct_size
        )));
    }
    if result.abi_version != NAUTERM_MOSH_ABI_VERSION {
        return Err(io::Error::other(format!(
            "nauterm_mosh_ffi create result ABI mismatch: expected {}, got {}",
            NAUTERM_MOSH_ABI_VERSION, result.abi_version
        )));
    }
    Ok(())
}

fn geometry_dimension(value: usize) -> io::Result<u16> {
    u16::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Mosh terminal geometry exceeds 65535 cells.",
        )
    })
}

fn default_mosh_server_command(terminal_options: &TerminalOptions) -> String {
    let locale = resolve_mosh_locale(&terminal_options.environment);
    let mut command = format!(
        "mosh-server new -s -c {} -l LANG={locale} -l TERM={}",
        mosh_color_count(terminal_options),
        terminal_options.terminal_type.term()
    );
    if let Some(color_term) = terminal_options.color_term.env_value() {
        command.push_str(" -l COLORTERM=");
        command.push_str(color_term);
    }
    command.push_str(
        " -- /bin/sh -c 'printf \"\\033]4545;Shell;%s\\007\" \"$SHELL\"; \
         case \"${SHELL##*/}\" in \
         zsh) exec \"$SHELL\" --histignorespace -i -l ;; \
         bash) case \":${HISTCONTROL:-}:\" in *:ignorespace:*|*:ignoreboth:*) ;; \
         *) HISTCONTROL=\"ignorespace${HISTCONTROL:+:$HISTCONTROL}\" ;; esac; \
         export HISTCONTROL; exec \"$SHELL\" -i -l ;; \
         *) exec \"${SHELL:-/bin/sh}\" -l ;; esac'",
    );
    command
}

fn mosh_color_count(terminal_options: &TerminalOptions) -> u16 {
    match terminal_options.terminal_type {
        crate::terminal::TerminalType::Xterm256Color => 256,
        crate::terminal::TerminalType::Xterm16Color => 16,
        crate::terminal::TerminalType::Xterm => 8,
    }
}

fn resolve_mosh_locale(environment: &[TerminalEnvironmentVariable]) -> String {
    for name in ["LC_ALL", "LC_CTYPE", "LANG"] {
        let Some(value) = environment
            .iter()
            .find(|entry| entry.variable.trim().eq_ignore_ascii_case(name))
            .map(|entry| entry.value.trim())
        else {
            continue;
        };
        if is_utf8_locale(value) {
            return value.to_owned();
        }
    }
    DEFAULT_MOSH_SERVER_LOCALE.to_owned()
}

fn is_utf8_locale(value: &str) -> bool {
    if value.is_empty() || value.contains(['\r', '\n', '\0']) {
        return false;
    }
    let normalized = value.trim().to_ascii_lowercase();
    (normalized.contains("utf-8") || normalized.contains("utf8"))
        && normalized
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'@' | b'-'))
}

fn session_event_from_external(event: ExternalSessionEvent) -> SessionEvent {
    SessionEvent {
        kind: event.kind,
        message: event.message,
        host: event.host,
        port: event.port,
        serial_port: event.serial_port,
        baud_rate: event.baud_rate,
        username: event.username,
        fingerprint: event.fingerprint,
        method: event.method,
        exit_status: event.exit_status,
        latency_ms: event.latency_ms,
        state_num: event.state_num,
    }
}

fn mosh_library_candidates() -> Vec<String> {
    let name = mosh_library_name();
    let separator = std::path::MAIN_SEPARATOR_STR;
    let root = env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .to_string_lossy()
        .into_owned();
    let external_root = env::var("NAUTERM_MOSH_REPO_DIR").ok().or_else(|| {
        let candidate = PathBuf::from(&root).join("..").join("nauterm-mosh");
        if candidate.exists() {
            Some(candidate.to_string_lossy().into_owned())
        } else {
            None
        }
    });
    let executable_directory = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .into_owned();
    let candidates = vec![
        [root.as_str(), "..", "nauterm-mosh", "target", "debug", name].join(separator),
        [
            root.as_str(),
            "..",
            "nauterm-mosh",
            "target",
            "release",
            name,
        ]
        .join(separator),
        format!("{executable_directory}{separator}{name}"),
        [executable_directory.as_str(), "lib", name].join(separator),
        if cfg!(target_os = "macos") {
            [executable_directory.as_str(), "..", "Frameworks", name].join(separator)
        } else {
            name.to_owned()
        },
        name.to_owned(),
    ];
    let mut candidates = candidates;
    if let Some(external_root) = external_root {
        candidates.push([external_root.as_str(), "target", "debug", name].join(separator));
        candidates.push([external_root.as_str(), "target", "release", name].join(separator));
    }
    let mut unique = Vec::new();
    for candidate in candidates {
        if !unique.contains(&candidate) {
            unique.push(candidate);
        }
    }
    unique
}

fn mosh_library_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "libnauterm_mosh_ffi.dylib"
    } else if cfg!(target_os = "windows") {
        "nauterm_mosh_ffi.dll"
    } else {
        "libnauterm_mosh_ffi.so"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::TerminalType;

    fn test_terminal_options(environment: Vec<TerminalEnvironmentVariable>) -> TerminalOptions {
        TerminalOptions {
            environment,
            ..TerminalOptions::default()
        }
    }

    #[test]
    fn default_server_command_tracks_terminal_capabilities() {
        let command = MoshServerCommand::default()
            .render_for_terminal(&TerminalOptions::default())
            .unwrap();
        assert_eq!(
            command,
            "mosh-server new -s -c 256 -l LANG=C.UTF-8 -l TERM=xterm-256color \
             -l COLORTERM=truecolor -- /bin/sh -c 'printf \"\\033]4545;Shell;%s\\007\" \"$SHELL\"; \
             case \"${SHELL##*/}\" in \
             zsh) exec \"$SHELL\" --histignorespace -i -l ;; \
             bash) case \":${HISTCONTROL:-}:\" in *:ignorespace:*|*:ignoreboth:*) ;; \
             *) HISTCONTROL=\"ignorespace${HISTCONTROL:+:$HISTCONTROL}\" ;; esac; \
             export HISTCONTROL; exec \"$SHELL\" -i -l ;; \
             *) exec \"${SHELL:-/bin/sh}\" -l ;; esac'"
        );
    }

    #[test]
    fn default_server_command_hides_shell_integration_from_history() {
        let command = MoshServerCommand::default()
            .render_for_terminal(&TerminalOptions::default())
            .unwrap();

        assert!(command.contains("zsh) exec \"$SHELL\" --histignorespace"));
        assert!(command.contains("HISTCONTROL=\"ignorespace"));
    }

    #[test]
    fn default_server_command_declares_terminal_color_count() {
        for (terminal_type, colors) in [
            (TerminalType::Xterm256Color, 256),
            (TerminalType::Xterm16Color, 16),
            (TerminalType::Xterm, 8),
        ] {
            let options = TerminalOptions {
                terminal_type,
                ..TerminalOptions::default()
            };
            let command = MoshServerCommand::default()
                .render_for_terminal(&options)
                .unwrap();
            assert!(command.contains(&format!("-c {colors}")));
        }
    }

    #[test]
    fn custom_server_command_is_preserved_verbatim() {
        let command = MoshServerCommand {
            command: "mosh-server new -s -l LANG=zh_CN.UTF-8 -p 60009".to_owned(),
        }
        .render_for_terminal(&test_terminal_options(Vec::new()))
        .unwrap();
        assert_eq!(command, "mosh-server new -s -l LANG=zh_CN.UTF-8 -p 60009");
    }

    #[test]
    fn default_server_command_prefers_utf8_locale_overrides_from_terminal_environment() {
        let command = MoshServerCommand::default()
            .render_for_terminal(&test_terminal_options(vec![
                TerminalEnvironmentVariable {
                    variable: "LANG".to_owned(),
                    value: "ja_JP.UTF-8".to_owned(),
                },
                TerminalEnvironmentVariable {
                    variable: "LC_CTYPE".to_owned(),
                    value: "zh_CN.UTF-8".to_owned(),
                },
            ]))
            .unwrap();
        assert!(command.contains("-l LANG=zh_CN.UTF-8"));
    }

    #[test]
    fn default_server_command_ignores_non_utf8_locale_overrides() {
        let command = MoshServerCommand::default()
            .render_for_terminal(&test_terminal_options(vec![TerminalEnvironmentVariable {
                variable: "LC_ALL".to_owned(),
                value: "C".to_owned(),
            }]))
            .unwrap();
        assert!(command.contains("-l LANG=C.UTF-8"));
    }

    #[test]
    fn default_server_command_rejects_shell_metacharacters_in_locale() {
        for value in [
            "C.UTF-8; touch /tmp/pwned",
            "$(touch /tmp/pwned).UTF-8",
            "`touch /tmp/pwned`.UTF-8",
            "C.UTF-8 value",
        ] {
            let command = MoshServerCommand::default()
                .render_for_terminal(&test_terminal_options(vec![TerminalEnvironmentVariable {
                    variable: "LANG".to_owned(),
                    value: value.to_owned(),
                }]))
                .unwrap();
            assert!(
                command.contains("-l LANG=C.UTF-8"),
                "unsafe locale was interpolated: {value:?} -> {command}"
            );
        }
    }

    #[test]
    fn default_server_command_accepts_safe_utf8_locale_variants() {
        for value in ["C.UTF-8", "en_US.UTF-8", "zh_CN.UTF-8@variant"] {
            let command = MoshServerCommand::default()
                .render_for_terminal(&test_terminal_options(vec![TerminalEnvironmentVariable {
                    variable: "LANG".to_owned(),
                    value: value.to_owned(),
                }]))
                .unwrap();
            assert!(command.contains(&format!("-l LANG={value}")));
        }
    }
}
