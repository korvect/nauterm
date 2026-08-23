use std::collections::HashMap;
use std::sync::mpsc::{self, SyncSender};
use std::thread::{self, JoinHandle};

use crate::mosh::{MoshServerCommand, MoshSessionTransport};
use crate::pty::WakeupCallback;
use crate::serial::{SerialConfig, SerialTransport};
use crate::ssh::{HostKeyTrustMode, ShellHistoryReceiver, SshProxyConfig, SshTransport};
use crate::telnet::TelnetTransport;
use crate::terminal::{
    TerminalCommandBlock, TerminalEmulator, TerminalEmulatorBackend, TerminalEngine,
    TerminalGeometry, TerminalOptions, TerminalSearchDirection, TerminalSearchResult,
    TerminalSnapshot,
};

pub type SessionId = u64;
const MAX_POLL_OUTPUT_CHUNKS: usize = 32;

#[allow(dead_code)]
pub const INPUT_STATUS_ACCEPTED: u32 = 0;
#[allow(dead_code)]
pub const INPUT_STATUS_BACKPRESSURE: u32 = 1;
#[allow(dead_code)]
pub const INPUT_STATUS_CLOSED: u32 = 2;
#[allow(dead_code)]
pub const INPUT_STATUS_INVALID: u32 = 3;

#[derive(Clone, Debug, serde::Serialize)]
pub struct SessionEvent {
    pub kind: String,
    pub message: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub serial_port: Option<String>,
    pub baud_rate: Option<u32>,
    pub username: Option<String>,
    pub fingerprint: Option<String>,
    pub method: Option<String>,
    pub exit_status: Option<u32>,
    pub latency_ms: Option<u32>,
    pub state_num: Option<u64>,
}

#[allow(dead_code)]
impl SessionEvent {
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
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
        }
    }

    pub fn with_host_port(mut self, host: &str, port: u16) -> Self {
        self.host = Some(host.to_owned());
        self.port = Some(port);
        self
    }

    pub fn with_serial(mut self, serial_port: &str, baud_rate: u32) -> Self {
        self.serial_port = Some(serial_port.to_owned());
        self.baud_rate = Some(baud_rate);
        self
    }

    pub fn with_username(mut self, username: &str) -> Self {
        self.username = Some(username.to_owned());
        self
    }

    pub fn with_fingerprint(mut self, fingerprint: &str) -> Self {
        self.fingerprint = Some(fingerprint.to_owned());
        self
    }

    pub fn with_method(mut self, method: &str) -> Self {
        self.method = Some(method.to_owned());
        self
    }

    pub fn with_exit_status(mut self, exit_status: u32) -> Self {
        self.exit_status = Some(exit_status);
        self
    }

    pub fn with_latency_ms(mut self, latency_ms: u32) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    pub fn with_state_num(mut self, state_num: u64) -> Self {
        self.state_num = Some(state_num);
        self
    }
}

pub struct SessionManager {
    next_id: SessionId,
    sessions: HashMap<SessionId, TerminalSessionActor>,
}

const SESSION_ACTOR_QUEUE_CAPACITY: usize = 256;

type TerminalSessionWork = Box<dyn FnOnce(&mut TerminalSession) + Send + 'static>;

enum TerminalSessionMessage {
    Run(TerminalSessionWork),
    Shutdown,
}

/// Owns a terminal session on one dedicated OS thread.
///
/// The handle is safe to move between FFI caller threads, while the terminal
/// emulator, its borrowed render state, and all ordering-sensitive session
/// state never leave the actor thread.
struct TerminalSessionActor {
    sender: SyncSender<TerminalSessionMessage>,
    worker: Option<JoinHandle<()>>,
}

impl TerminalSessionActor {
    fn spawn(factory: impl FnOnce() -> Option<TerminalSession> + Send + 'static) -> Option<Self> {
        let (sender, receiver) =
            mpsc::sync_channel::<TerminalSessionMessage>(SESSION_ACTOR_QUEUE_CAPACITY);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("nauterm-terminal-session".to_owned())
            .spawn(move || {
                let Some(mut session) = factory() else {
                    let _ = ready_sender.send(false);
                    return;
                };
                if ready_sender.send(true).is_err() {
                    return;
                }
                while let Ok(message) = receiver.recv() {
                    match message {
                        TerminalSessionMessage::Run(work) => work(&mut session),
                        TerminalSessionMessage::Shutdown => break,
                    }
                }
                session.engine.set_wakeup_callback(None);
                session.transport.set_wakeup_callback(None);
                session.wakeup = None;
                session.clear_pending_output_for_close();
            })
            .ok()?;

        if ready_receiver.recv().ok() != Some(true) {
            let _ = worker.join();
            return None;
        }
        Some(Self {
            sender,
            worker: Some(worker),
        })
    }

    fn call<R: Send + 'static>(
        &self,
        work: impl FnOnce(&mut TerminalSession) -> R + Send + 'static,
    ) -> Option<R> {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.sender
            .send(TerminalSessionMessage::Run(Box::new(move |session| {
                let _ = reply_sender.send(work(session));
            })))
            .ok()?;
        reply_receiver.recv().ok()
    }

    fn shutdown(&mut self) {
        let _ = self.sender.send(TerminalSessionMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for TerminalSessionActor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct TerminalSession {
    engine: Box<dyn TerminalEmulator>,
    transport: SessionTransport,
    options: TerminalOptions,
    events: Vec<SessionEvent>,
    mosh_screen: MoshAlternateScreenTracker,
    wakeup: Option<WakeupCallback>,
}

struct MoshAlternateScreenTracker {
    active: bool,
    bracketed_paste_exit: bool,
    suppress_leading_breaks: bool,
    primary_row_offset: usize,
}

impl Default for MoshAlternateScreenTracker {
    fn default() -> Self {
        Self {
            active: false,
            bracketed_paste_exit: false,
            suppress_leading_breaks: true,
            primary_row_offset: 0,
        }
    }
}

impl MoshAlternateScreenTracker {
    fn write_update(&mut self, engine: &mut dyn TerminalEmulator, output: &[u8]) {
        // mosh-server consumes application smcup/rmcup sequences. Recreate the
        // local screen boundary only for modes interactive full-screen apps use.
        let normalized;
        let mut detected_primary_row_offset = None;
        let mut clear_primary_scrollback = false;
        let output = if self.suppress_leading_breaks && !self.active {
            normalized = remove_mosh_leading_line_breaks(output);
            if !normalized.is_empty() {
                self.suppress_leading_breaks = false;
                clear_primary_scrollback = true;
            }
            // An ordinary initial shell frame can reserve row 1 as a
            // synthetic screen origin and paint the prompt at row 2. Keep the
            // correction for later framebuffer diffs instead of applying it
            // to this batch only.
            if is_mosh_initial_shell_frame(output) || is_mosh_initial_shell_frame(&normalized) {
                detected_primary_row_offset = Some(1);
            }
            normalized.as_slice()
        } else {
            output
        };
        let leaves_full_screen = self.active
            && (contains_bytes(output, b"\x1b[?1049l")
                || mosh_output_leaves_full_screen(output)
                || (self.bracketed_paste_exit && contains_bytes(output, b"\x1b[?2004l")));
        if leaves_full_screen {
            let remote_cursor_row = last_absolute_cursor_row(output);
            if !contains_bytes(output, b"\x1b[?1049l") {
                engine.write_bytes(b"\x1b[?1049l");
            }
            self.active = false;
            self.bracketed_paste_exit = false;
            if let Some(remote_row) = remote_cursor_row {
                let local_row = engine.snapshot().cursor_row + 2;
                self.primary_row_offset = remote_row.saturating_sub(local_row);
            }
        } else if contains_bytes(output, b"\x1b[?1049h") {
            self.active = true;
            self.bracketed_paste_exit = false;
            self.primary_row_offset = 0;
        } else if !self.active {
            if let Some(bracketed_paste_exit) = mosh_full_screen_entry(output) {
                engine.write_bytes(b"\x1b[?1049h");
                self.active = true;
                self.bracketed_paste_exit = bracketed_paste_exit;
                self.primary_row_offset = 0;
            }
        }

        if !output.is_empty() {
            let row_offset = detected_primary_row_offset.unwrap_or(self.primary_row_offset);
            if !self.active && row_offset > 0 {
                engine.write_bytes(&translate_absolute_cursor_rows(output, row_offset));
            } else {
                engine.write_bytes(output);
            }
        }
        if let Some(row_offset) = detected_primary_row_offset {
            if !self.active {
                self.primary_row_offset = row_offset;
            }
        }
        if clear_primary_scrollback && !self.active {
            // A Mosh screen update is a framebuffer repaint, not output that
            // should extend the local terminal's history. Alacritty can retain
            // the pre-repaint prompt when the first frame is followed by the
            // initial geometry resize. ED 3 clears only that stale scrollback
            // while preserving the corrected visible screen.
            engine.write_bytes_without_capture(b"\x1b[3J");
        }
    }

    fn note_resize(&mut self) {
        if !self.active {
            self.suppress_leading_breaks = true;
        }
    }

    fn restore(&mut self, engine: &mut dyn TerminalEmulator) {
        if self.active {
            engine.write_bytes(b"\x1b[?1049l");
            self.active = false;
            self.bracketed_paste_exit = false;
        }
    }
}

fn last_absolute_cursor_row(output: &[u8]) -> Option<usize> {
    let mut last = None;
    let mut index = 0;
    while index + 2 < output.len() {
        if output[index] != 0x1b || output[index + 1] != b'[' {
            index += 1;
            continue;
        }
        let parameter_start = index + 2;
        let mut end = parameter_start;
        while end < output.len() && (output[end].is_ascii_digit() || output[end] == b';') {
            end += 1;
        }
        if end < output.len() && matches!(output[end], b'H' | b'f') {
            let row_end = output[parameter_start..end]
                .iter()
                .position(|byte| *byte == b';')
                .map_or(end, |offset| parameter_start + offset);
            if row_end > parameter_start {
                if let Ok(row) = std::str::from_utf8(&output[parameter_start..row_end])
                    .unwrap_or_default()
                    .parse::<usize>()
                {
                    last = Some(row.max(1));
                }
            }
        }
        index = end.saturating_add(1);
    }
    last
}

fn translate_absolute_cursor_rows(output: &[u8], row_offset: usize) -> Vec<u8> {
    let mut translated = Vec::with_capacity(output.len());
    let mut index = 0;
    while index < output.len() {
        if index + 2 >= output.len() || output[index] != 0x1b || output[index + 1] != b'[' {
            translated.push(output[index]);
            index += 1;
            continue;
        }
        let parameter_start = index + 2;
        let mut end = parameter_start;
        while end < output.len() && (output[end].is_ascii_digit() || output[end] == b';') {
            end += 1;
        }
        if end >= output.len() || !matches!(output[end], b'H' | b'f') {
            translated.extend_from_slice(&output[index..end.min(output.len())]);
            index = end;
            continue;
        }
        let row_end = output[parameter_start..end]
            .iter()
            .position(|byte| *byte == b';')
            .map_or(end, |offset| parameter_start + offset);
        let row = std::str::from_utf8(&output[parameter_start..row_end])
            .ok()
            .and_then(|value| value.parse::<usize>().ok());
        if let Some(row) = row {
            translated.extend_from_slice(b"\x1b[");
            translated
                .extend_from_slice(row.saturating_sub(row_offset).max(1).to_string().as_bytes());
            translated.extend_from_slice(&output[row_end..=end]);
        } else {
            translated.extend_from_slice(&output[index..=end]);
        }
        index = end + 1;
    }
    translated
}

fn mosh_full_screen_entry(output: &[u8]) -> Option<bool> {
    let strong_mode = [
        b"\x1b[?1000h".as_slice(),
        b"\x1b[?1002h".as_slice(),
        b"\x1b[?1003h".as_slice(),
        b"\x1b[?1004h".as_slice(),
        b"\x1b[?1006h".as_slice(),
    ]
    .iter()
    .any(|sequence| contains_bytes(output, sequence));
    if strong_mode {
        return Some(false);
    }

    let homes_cursor = contains_bytes(output, b"\x1b[1;1H")
        || contains_bytes(output, b"\x1b[H")
        || contains_bytes(output, b"\x1b[1;1f");
    let vim_placeholder_rows = output
        .windows(3)
        .filter(|window| *window == b"\r\n~")
        .count();
    (homes_cursor && vim_placeholder_rows >= 2).then_some(true)
}

fn mosh_output_leaves_full_screen(output: &[u8]) -> bool {
    [
        b"\x1b[?1000l".as_slice(),
        b"\x1b[?1002l".as_slice(),
        b"\x1b[?1003l".as_slice(),
        b"\x1b[?1004l".as_slice(),
        b"\x1b[?1006l".as_slice(),
    ]
    .iter()
    .any(|sequence| contains_bytes(output, sequence))
}

fn remove_mosh_leading_line_breaks(output: &[u8]) -> Vec<u8> {
    let prefix_end = mosh_control_prefix_end(output);
    let mut content_start = prefix_end;
    loop {
        if output[content_start..].starts_with(b"\r\n") {
            content_start += 2;
        } else if output[content_start..].starts_with(b"\n")
            || output[content_start..].starts_with(b"\r")
        {
            content_start += 1;
        } else {
            break;
        }
    }
    if content_start == prefix_end {
        output.to_vec()
    } else {
        let mut normalized = Vec::with_capacity(output.len() - (content_start - prefix_end));
        normalized.extend_from_slice(&output[..prefix_end]);
        normalized.extend_from_slice(&output[content_start..]);
        normalized
    }
}

fn mosh_control_prefix_end(output: &[u8]) -> usize {
    let mut index = 0;
    while index < output.len() {
        match output[index] {
            0x1b => {
                let Some(end) = ansi_escape_sequence_end(output, index) else {
                    break;
                };
                index = end;
            }
            byte if byte < 0x20 && !matches!(byte, b'\r' | b'\n') => {
                index += 1;
            }
            0x7f => {
                index += 1;
            }
            _ => break,
        }
    }
    index
}

fn ansi_escape_sequence_end(output: &[u8], start: usize) -> Option<usize> {
    let kind = *output.get(start + 1)?;
    match kind {
        b'[' => output[start + 2..]
            .iter()
            .position(|byte| (0x40..=0x7e).contains(byte))
            .map(|offset| start + 3 + offset),
        b']' => ansi_string_end(output, start + 2, true),
        b'P' | b'X' | b'^' | b'_' => ansi_string_end(output, start + 2, false),
        _ => {
            let mut index = start + 1;
            while output
                .get(index)
                .is_some_and(|byte| (0x20..=0x2f).contains(byte))
            {
                index += 1;
            }
            output
                .get(index)
                .filter(|byte| (0x30..=0x7e).contains(*byte))
                .map(|_| index + 1)
        }
    }
}

fn ansi_string_end(output: &[u8], start: usize, bell_terminated: bool) -> Option<usize> {
    let mut index = start;
    while index < output.len() {
        if bell_terminated && output[index] == 0x07 {
            return Some(index + 1);
        }
        if output[index..].starts_with(b"\x1b\\") {
            return Some(index + 2);
        }
        index += 1;
    }
    None
}

fn is_mosh_initial_shell_frame(output: &[u8]) -> bool {
    contains_bytes(output, b"\x1b[2J")
        && mosh_full_screen_entry(output).is_none()
        && !contains_bytes(output, b"\x1b[?1049h")
        && output
            .windows(4)
            .filter(|window| *window == b"\x1b[K\n")
            .count()
            >= 2
        && last_absolute_cursor_row(output) == Some(2)
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

enum SessionTransport {
    Local,
    Ssh(SshTransport),
    Mosh(MoshSessionTransport),
    Telnet(TelnetTransport),
    Serial(SerialTransport),
    Disconnected,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self {
            next_id: 1,
            sessions: HashMap::new(),
        }
    }
}

impl SessionManager {
    pub fn create_local(
        &mut self,
        columns: usize,
        rows: usize,
        options: TerminalOptions,
    ) -> Option<SessionId> {
        let id = self.insert_new(
            columns,
            rows,
            SessionTransport::Local,
            options,
            Vec::new(),
            None,
            false,
            true,
        );
        (id != 0).then_some(id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_ssh(
        &mut self,
        columns: usize,
        rows: usize,
        options: TerminalOptions,
        host: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
        private_key: Option<&str>,
        passphrase: Option<&str>,
        known_hosts_path: Option<&str>,
        host_key_trust_mode: HostKeyTrustMode,
        proxy: Option<SshProxyConfig>,
        ssh_keepalive_interval_seconds: u32,
        encoding: &str,
    ) -> SessionId {
        let geometry = TerminalGeometry::new(columns, rows);
        match SshTransport::connect(
            geometry,
            options.clone(),
            host,
            port,
            username,
            password,
            private_key,
            passphrase,
            known_hosts_path,
            host_key_trust_mode,
            proxy,
            ssh_keepalive_interval_seconds,
            encoding,
        ) {
            Ok(ssh) => self.insert_new(
                geometry.columns,
                geometry.rows,
                SessionTransport::Ssh(ssh),
                options,
                Vec::new(),
                None,
                false,
                false,
            ),
            Err(error) => self.insert_new(
                geometry.columns,
                geometry.rows,
                SessionTransport::Disconnected,
                options,
                Vec::new(),
                Some(
                    format!("SSH session {username}@{host}:{port}\r\nFailed to start SSH transport: {error}\r\n")
                        .into_bytes(),
                ),
                true,
                false,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_mosh(
        &mut self,
        columns: usize,
        rows: usize,
        options: TerminalOptions,
        host: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
        private_key: Option<&str>,
        passphrase: Option<&str>,
        known_hosts_path: Option<&str>,
        host_key_trust_mode: HostKeyTrustMode,
        proxy: Option<SshProxyConfig>,
        server_command: MoshServerCommand,
    ) -> SessionId {
        let geometry = TerminalGeometry::new(columns, rows);
        match MoshSessionTransport::connect(
            &options,
            geometry,
            host,
            port,
            username,
            password,
            private_key,
            passphrase,
            known_hosts_path,
            host_key_trust_mode,
            proxy,
            server_command,
        ) {
            Ok(mosh) => self.insert_new(
                geometry.columns,
                geometry.rows,
                SessionTransport::Mosh(mosh),
                options,
                Vec::new(),
                None,
                false,
                false,
            ),
            Err(error) => {
                let mut initial_events = error.events;
                if initial_events.is_empty() {
                    initial_events.push(
                        SessionEvent::new(
                            "error",
                            format!("Failed to start Mosh transport: {}", error.message),
                        )
                        .with_host_port(host, port)
                        .with_username(username),
                    );
                }
                self.insert_new(
                    geometry.columns,
                    geometry.rows,
                    SessionTransport::Disconnected,
                    options,
                    initial_events,
                    None,
                    true,
                    false,
                )
            }
        }
    }

    pub fn create_serial(
        &mut self,
        columns: usize,
        rows: usize,
        options: TerminalOptions,
        serial_port: &str,
        config: SerialConfig,
    ) -> SessionId {
        let start_event = SessionEvent::new(
            "connect_start",
            format!("Opening serial port {serial_port} at {}.", config.summary()),
        )
        .with_serial(serial_port, config.baud_rate);
        match SerialTransport::open(serial_port, config) {
            Ok(serial) => self.insert_new(
                columns,
                rows,
                SessionTransport::Serial(serial),
                options,
                vec![
                    start_event,
                    SessionEvent::new(
                        "connected",
                        format!("Serial port {serial_port} is open at {}.", config.summary()),
                    )
                    .with_serial(serial_port, config.baud_rate),
                ],
                None,
                false,
                false,
            ),
            Err(error) => self.insert_new(
                columns,
                rows,
                SessionTransport::Disconnected,
                options,
                vec![
                    start_event,
                    SessionEvent::new("error", error.to_string())
                        .with_serial(serial_port, config.baud_rate),
                ],
                None,
                true,
                false,
            ),
        }
    }

    pub fn create_telnet(
        &mut self,
        columns: usize,
        rows: usize,
        options: TerminalOptions,
        host: &str,
        port: u16,
        encoding: &str,
    ) -> SessionId {
        let geometry = TerminalGeometry::new(columns, rows);
        match TelnetTransport::connect(geometry, options.clone(), host, port, encoding) {
            Ok(telnet) => self.insert_new(
                geometry.columns,
                geometry.rows,
                SessionTransport::Telnet(telnet),
                options,
                Vec::new(),
                None,
                false,
                false,
            ),
            Err(error) => self.insert_new(
                geometry.columns,
                geometry.rows,
                SessionTransport::Disconnected,
                options,
                Vec::new(),
                Some(
                    format!(
                        "Telnet session {host}:{port}\r\nFailed to start Telnet transport: {error}\r\n"
                    )
                    .into_bytes(),
                ),
                true,
                false,
            ),
        }
    }

    pub fn create_serial_error(
        &mut self,
        columns: usize,
        rows: usize,
        options: TerminalOptions,
        serial_port: &str,
        baud_rate: u32,
        message: impl Into<String>,
    ) -> SessionId {
        self.insert_new(
            columns,
            rows,
            SessionTransport::Disconnected,
            options,
            vec![SessionEvent::new("error", message).with_serial(serial_port, baud_rate)],
            None,
            true,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reconnect_ssh(
        &mut self,
        id: SessionId,
        host: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
        private_key: Option<&str>,
        passphrase: Option<&str>,
        known_hosts_path: Option<&str>,
        host_key_trust_mode: HostKeyTrustMode,
        proxy: Option<SshProxyConfig>,
        ssh_keepalive_interval_seconds: u32,
        encoding: &str,
    ) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        let host = host.to_owned();
        let username = username.to_owned();
        let password = password.map(str::to_owned);
        let private_key = private_key.map(str::to_owned);
        let passphrase = passphrase.map(str::to_owned);
        let known_hosts_path = known_hosts_path.map(str::to_owned);
        let encoding = encoding.to_owned();
        actor
            .call(move |session| {
                let snapshot = session.engine.snapshot();
                let geometry = TerminalGeometry::new(snapshot.columns, snapshot.rows);
                session.transport.set_wakeup_callback(None);
                session.events.clear();
                match SshTransport::connect(
                    geometry,
                    session.options.clone(),
                    &host,
                    port,
                    &username,
                    password.as_deref(),
                    private_key.as_deref(),
                    passphrase.as_deref(),
                    known_hosts_path.as_deref(),
                    host_key_trust_mode,
                    proxy,
                    ssh_keepalive_interval_seconds,
                    &encoding,
                ) {
                    Ok(mut ssh) => {
                        ssh.set_wakeup_callback(session.wakeup);
                        session.transport = SessionTransport::Ssh(ssh);
                        session.engine.mark_running();
                    }
                    Err(error) => {
                        session.transport = SessionTransport::Disconnected;
                        session.engine.mark_exited();
                        session.events.push(
                            SessionEvent::new(
                                "error",
                                format!("Failed to restart SSH transport: {error}"),
                            )
                            .with_host_port(&host, port)
                            .with_username(&username),
                        );
                    }
                }
                true
            })
            .unwrap_or(false)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reconnect_mosh(
        &mut self,
        id: SessionId,
        host: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
        private_key: Option<&str>,
        passphrase: Option<&str>,
        known_hosts_path: Option<&str>,
        host_key_trust_mode: HostKeyTrustMode,
        proxy: Option<SshProxyConfig>,
        server_command: MoshServerCommand,
    ) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        let host = host.to_owned();
        let username = username.to_owned();
        let password = password.map(str::to_owned);
        let private_key = private_key.map(str::to_owned);
        let passphrase = passphrase.map(str::to_owned);
        let known_hosts_path = known_hosts_path.map(str::to_owned);
        actor
            .call(move |session| {
                let snapshot = session.engine.snapshot();
                let geometry = TerminalGeometry::new(snapshot.columns, snapshot.rows);
                session.transport.set_wakeup_callback(None);
                session.events.clear();
                match MoshSessionTransport::connect(
                    &session.options,
                    geometry,
                    &host,
                    port,
                    &username,
                    password.as_deref(),
                    private_key.as_deref(),
                    passphrase.as_deref(),
                    known_hosts_path.as_deref(),
                    host_key_trust_mode,
                    proxy,
                    server_command,
                ) {
                    Ok(mut mosh) => {
                        mosh.set_wakeup_callback(session.wakeup);
                        session.transport = SessionTransport::Mosh(mosh);
                        session.mosh_screen = MoshAlternateScreenTracker::default();
                        session.engine.mark_running();
                    }
                    Err(error) => {
                        session.transport = SessionTransport::Disconnected;
                        session.engine.mark_exited();
                        if error.events.is_empty() {
                            session.events.push(
                                SessionEvent::new(
                                    "error",
                                    format!("Failed to restart Mosh transport: {}", error.message),
                                )
                                .with_host_port(&host, port)
                                .with_username(&username),
                            );
                        } else {
                            session.events.extend(error.events);
                        }
                    }
                }
                true
            })
            .unwrap_or(false)
    }

    pub fn close(&mut self, id: SessionId) -> bool {
        let Some(mut actor) = self.sessions.remove(&id) else {
            return false;
        };
        actor.shutdown();
        true
    }

    pub fn resize(
        &mut self,
        id: SessionId,
        columns: usize,
        rows: usize,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        actor
            .call(move |session| {
                let geometry = TerminalGeometry::new(columns, rows);
                session.engine.resize(
                    geometry.columns,
                    geometry.rows,
                    cell_width_px,
                    cell_height_px,
                );
                session.transport.resize(geometry);
                if matches!(&session.transport, SessionTransport::Mosh(_)) {
                    session.mosh_screen.note_resize();
                }
                true
            })
            .unwrap_or(false)
    }

    pub fn notify_network_changed(&mut self, id: SessionId) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        actor
            .call(|session| session.transport.notify_network_changed())
            .unwrap_or(false)
    }

    pub fn exit_alternate_screen(&mut self, id: SessionId) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        actor
            .call(|session| {
                if !session.engine.is_alt_screen() {
                    return false;
                }
                session.engine.write_bytes(b"\x1b[?1049l");
                true
            })
            .unwrap_or(false)
    }

    pub fn scroll_lines(&mut self, id: SessionId, lines: i32) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        actor
            .call(move |session| {
                session.engine.scroll_lines(lines);
                true
            })
            .unwrap_or(false)
    }

    pub fn scroll_page_up(&mut self, id: SessionId) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        actor
            .call(|session| {
                session.engine.scroll_page_up();
                true
            })
            .unwrap_or(false)
    }

    pub fn scroll_page_down(&mut self, id: SessionId) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        actor
            .call(|session| {
                session.engine.scroll_page_down();
                true
            })
            .unwrap_or(false)
    }

    pub fn scroll_to_bottom(&mut self, id: SessionId) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        actor
            .call(|session| {
                session.engine.scroll_to_bottom();
                true
            })
            .unwrap_or(false)
    }

    pub fn search(
        &mut self,
        id: SessionId,
        query: &str,
        direction: TerminalSearchDirection,
        origin_row: usize,
        origin_column: usize,
    ) -> TerminalSearchResult {
        let Some(actor) = self.sessions.get(&id) else {
            return TerminalSearchResult::not_found(0, 0);
        };
        let query = query.to_owned();
        actor
            .call(move |session| {
                session
                    .engine
                    .search(&query, direction, origin_row, origin_column)
            })
            .unwrap_or_else(|| TerminalSearchResult::not_found(0, 0))
    }

    pub fn selection_text(&self, id: SessionId, start: i64, end: i64) -> Option<String> {
        self.sessions
            .get(&id)?
            .call(move |session| session.engine.selection_text(start, end))
    }

    pub fn command_block_at(&self, id: SessionId, offset: i64) -> Option<TerminalCommandBlock> {
        self.sessions
            .get(&id)?
            .call(move |session| session.engine.command_block_at(offset))
            .flatten()
    }

    pub fn prompt_click_move(
        &self,
        id: SessionId,
        offset: i64,
    ) -> Option<crate::terminal::TerminalPromptClickMove> {
        self.sessions
            .get(&id)?
            .call(move |session| session.engine.prompt_click_move(offset))
            .flatten()
    }

    pub fn set_wakeup_callback(&mut self, id: SessionId, callback: Option<WakeupCallback>) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        actor
            .call(move |session| {
                session.engine.set_wakeup_callback(callback);
                session.transport.set_wakeup_callback(callback);
                session.wakeup = callback;
                true
            })
            .unwrap_or(false)
    }

    pub fn prepare_runtime_shutdown(&mut self) {
        for actor in self.sessions.values_mut() {
            actor.shutdown();
        }
        self.sessions.clear();
    }

    pub fn poll(&mut self, id: SessionId) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        actor.call(TerminalSession::poll).unwrap_or(false)
    }

    pub fn is_exited(&self, id: SessionId) -> bool {
        self.sessions
            .get(&id)
            .and_then(|actor| actor.call(|session| session.engine.is_exited()))
            .unwrap_or(true)
    }

    pub fn write_bytes(&mut self, id: SessionId, bytes: &[u8]) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        let bytes = bytes.to_vec();
        actor
            .call(move |session| {
                session.engine.write_bytes(&bytes);
                session.flush_terminal_writes();
                true
            })
            .unwrap_or(false)
    }

    pub fn request_ssh_shell_history(
        &mut self,
        id: SessionId,
    ) -> Result<ShellHistoryReceiver, String> {
        let actor = self
            .sessions
            .get(&id)
            .ok_or_else(|| "terminal session was not found".to_owned())?;
        actor
            .call(|session| match &mut session.transport {
                SessionTransport::Ssh(ssh) => ssh.request_shell_history(),
                _ => Err("terminal session is not an SSH connection".to_owned()),
            })
            .unwrap_or_else(|| Err("terminal session actor stopped".to_owned()))
    }

    pub fn send_input_bytes(&mut self, id: SessionId, bytes: &[u8]) -> bool {
        self.send_input_bytes_status(id, bytes) == INPUT_STATUS_ACCEPTED
    }

    pub fn send_input_bytes_status(&mut self, id: SessionId, bytes: &[u8]) -> u32 {
        let Some(actor) = self.sessions.get(&id) else {
            return INPUT_STATUS_CLOSED;
        };
        if bytes.is_empty() {
            return INPUT_STATUS_INVALID;
        }
        let bytes = bytes.to_vec();
        actor
            .call(move |session| match &mut session.transport {
                SessionTransport::Local => {
                    if session.engine.send_input_bytes(&bytes) {
                        INPUT_STATUS_ACCEPTED
                    } else {
                        INPUT_STATUS_CLOSED
                    }
                }
                SessionTransport::Ssh(ssh) => {
                    if ssh.queue_input(&bytes) {
                        INPUT_STATUS_ACCEPTED
                    } else {
                        INPUT_STATUS_CLOSED
                    }
                }
                SessionTransport::Mosh(mosh) => mosh.queue_input_status(&bytes),
                SessionTransport::Telnet(telnet) => {
                    if telnet.queue_input(&bytes) {
                        INPUT_STATUS_ACCEPTED
                    } else {
                        INPUT_STATUS_CLOSED
                    }
                }
                SessionTransport::Serial(serial) => {
                    if serial.queue_input(&bytes) {
                        INPUT_STATUS_ACCEPTED
                    } else {
                        INPUT_STATUS_CLOSED
                    }
                }
                SessionTransport::Disconnected => INPUT_STATUS_CLOSED,
            })
            .unwrap_or(INPUT_STATUS_CLOSED)
    }

    pub fn snapshot(&self, id: SessionId) -> Option<TerminalSnapshot> {
        self.sessions
            .get(&id)?
            .call(|session| session.engine.snapshot())
    }

    pub fn clipboard(&self, id: SessionId) -> Option<String> {
        self.sessions
            .get(&id)?
            .call(|session| session.engine.clipboard())
    }

    pub fn bell_count(&self, id: SessionId) -> Option<u64> {
        self.sessions
            .get(&id)?
            .call(|session| session.engine.bell_count())
    }

    pub fn drain_events(&mut self, id: SessionId) -> Vec<SessionEvent> {
        self.sessions
            .get(&id)
            .and_then(|actor| actor.call(|session| session.events.drain(..).collect()))
            .unwrap_or_default()
    }

    pub fn drain_output_capture(&mut self, id: SessionId) -> Vec<u8> {
        self.sessions
            .get(&id)
            .and_then(|actor| actor.call(|session| session.engine.drain_output_capture()))
            .unwrap_or_default()
    }

    pub fn suppress_output_until(&mut self, id: SessionId, marker: &[u8]) -> bool {
        let marker = marker.to_vec();
        self.sessions
            .get(&id)
            .and_then(|actor| {
                actor.call(move |session| session.engine.suppress_output_until(&marker))
            })
            .unwrap_or(false)
    }

    pub fn cancel_output_suppression(&mut self, id: SessionId) -> bool {
        let Some(actor) = self.sessions.get(&id) else {
            return false;
        };
        actor
            .call(|session| {
                session.engine.cancel_output_suppression();
                true
            })
            .unwrap_or(false)
    }

    #[cfg(test)]
    fn insert(
        &mut self,
        engine: TerminalEngine,
        transport: SessionTransport,
        options: TerminalOptions,
    ) -> SessionId {
        self.insert_with_events(engine, transport, options, Vec::new())
    }

    #[cfg(test)]
    fn insert_with_events(
        &mut self,
        engine: TerminalEngine,
        transport: SessionTransport,
        options: TerminalOptions,
        events: Vec<SessionEvent>,
    ) -> SessionId {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let actor = TerminalSessionActor::spawn(move || {
            Some(TerminalSession {
                engine: Box::new(engine),
                transport,
                options,
                events,
                mosh_screen: MoshAlternateScreenTracker::default(),
                wakeup: None,
            })
        });
        if let Some(actor) = actor {
            self.sessions.insert(id, actor);
            id
        } else {
            0
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_new(
        &mut self,
        columns: usize,
        rows: usize,
        transport: SessionTransport,
        options: TerminalOptions,
        events: Vec<SessionEvent>,
        initial_output: Option<Vec<u8>>,
        mark_exited: bool,
        start_local_pty: bool,
    ) -> SessionId {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let actor_options = options.clone();
        let actor = TerminalSessionActor::spawn(move || {
            let mut engine = create_terminal_emulator(columns, rows, actor_options.clone()).ok()?;
            if start_local_pty && !engine.start_local_pty() {
                return None;
            }
            if let Some(output) = initial_output {
                engine.write_bytes(&output);
            }
            if mark_exited {
                engine.mark_exited();
            }
            Some(TerminalSession {
                engine,
                transport,
                options: actor_options,
                events,
                mosh_screen: MoshAlternateScreenTracker::default(),
                wakeup: None,
            })
        });
        if let Some(actor) = actor {
            self.sessions.insert(id, actor);
            id
        } else {
            0
        }
    }
}

pub(crate) fn create_terminal_emulator(
    columns: usize,
    rows: usize,
    options: TerminalOptions,
) -> Result<Box<dyn TerminalEmulator>, String> {
    match options.emulator_backend {
        TerminalEmulatorBackend::Alacritty => Ok(Box::new(TerminalEngine::new_with_options(
            columns, rows, options,
        ))),
        TerminalEmulatorBackend::Ghostty => {
            #[cfg(feature = "terminal-ghostty")]
            {
                crate::ghostty_terminal::GhosttyTerminalEngine::new(columns, rows, options)
                    .map(|engine| Box::new(engine) as Box<dyn TerminalEmulator>)
            }
            #[cfg(not(feature = "terminal-ghostty"))]
            {
                Err("Ghostty terminal support is not available in this build".to_owned())
            }
        }
    }
}

impl TerminalSession {
    fn poll(&mut self) -> bool {
        let (mosh_batches, output, exited) = match &mut self.transport {
            SessionTransport::Local => return self.engine.pump_local_pty(),
            SessionTransport::Ssh(ssh) => {
                let (output, exited) = drain_ssh_output(ssh, &mut self.events);
                (Vec::new(), output, exited)
            }
            SessionTransport::Mosh(mosh) => drain_mosh_output(mosh, &mut self.events),
            SessionTransport::Serial(serial) => {
                let serial_port = serial.path().to_owned();
                let config = serial.config();
                let (output, exited) =
                    drain_serial_output(serial, &mut self.events, &serial_port, config.baud_rate);
                (Vec::new(), output, exited)
            }
            SessionTransport::Telnet(telnet) => {
                let (output, exited) = drain_telnet_output(telnet, &mut self.events);
                (Vec::new(), output, exited)
            }
            SessionTransport::Disconnected => return false,
        };

        let changed = !mosh_batches.is_empty() || !output.is_empty() || exited;
        if let Some(enabled) =
            self.events
                .iter()
                .rev()
                .find_map(|event| match event.kind.as_str() {
                    "mosh_echo_enabled" => Some(true),
                    "mosh_echo_disabled" => Some(false),
                    _ => None,
                })
        {
            self.engine.set_input_echo_enabled(enabled);
        }
        if !output.is_empty() {
            self.engine.write_bytes(&output);
        }
        if !mosh_batches.is_empty() {
            if let SessionTransport::Mosh(mosh) = &mut self.transport {
                for batch in mosh_batches {
                    self.mosh_screen
                        .write_update(self.engine.as_mut(), &batch.output);
                    if mosh.commit_screen(batch.state_num) {
                        self.events.push(
                            SessionEvent::new(
                                "mosh_screen_committed",
                                "Mosh screen state committed to the terminal engine.",
                            )
                            .with_state_num(batch.state_num),
                        );
                    }
                }
            }
        }
        self.flush_terminal_writes();
        if exited {
            if matches!(&self.transport, SessionTransport::Mosh(_)) {
                self.mosh_screen.restore(self.engine.as_mut());
            }
            self.transport = SessionTransport::Disconnected;
            self.engine.mark_exited();
        }

        changed
    }

    fn flush_terminal_writes(&mut self) {
        let writes = self.engine.drain_transport_writes();
        if writes.is_empty() {
            return;
        }

        match &mut self.transport {
            SessionTransport::Ssh(ssh) => {
                for write in writes {
                    ssh.queue_input(write.as_bytes());
                }
            }
            SessionTransport::Mosh(mosh) => {
                for write in writes {
                    mosh.queue_input(write.as_bytes());
                }
            }
            SessionTransport::Serial(serial) => {
                for write in writes {
                    serial.queue_input(write.as_bytes());
                }
            }
            SessionTransport::Telnet(telnet) => {
                for write in writes {
                    telnet.queue_input(write.as_bytes());
                }
            }
            SessionTransport::Local | SessionTransport::Disconnected => {}
        }
    }

    fn clear_pending_output_for_close(&mut self) {
        self.engine.clear_pending_output_for_close();
        match &mut self.transport {
            SessionTransport::Ssh(ssh) => ssh.clear_pending_output(),
            SessionTransport::Mosh(mosh) => mosh.clear_pending_output(),
            SessionTransport::Telnet(telnet) => telnet.clear_pending_output(),
            SessionTransport::Serial(serial) => serial.clear_pending_output(),
            SessionTransport::Local | SessionTransport::Disconnected => {}
        }
    }
}

fn drain_ssh_output(ssh: &mut SshTransport, events: &mut Vec<SessionEvent>) -> (Vec<u8>, bool) {
    let mut output = Vec::new();
    let mut exited = false;
    for _ in 0..MAX_POLL_OUTPUT_CHUNKS {
        let pump = ssh.drain_output();
        output.extend_from_slice(&pump.output);
        events.extend(pump.events);
        exited = pump.exited;
        if !pump.has_more {
            break;
        }
    }
    (output, exited)
}

fn drain_mosh_output(
    mosh: &mut MoshSessionTransport,
    events: &mut Vec<SessionEvent>,
) -> (Vec<crate::mosh::MoshScreenBatch>, Vec<u8>, bool) {
    let mut batches = Vec::new();
    let mut exited = false;
    for _ in 0..MAX_POLL_OUTPUT_CHUNKS {
        let pump = mosh.drain_output();
        if let Some(batch) = pump.screen_batch {
            batches.push(batch);
        }
        events.extend(pump.events);
        exited = mosh_drain_exited(pump.exited, pump.has_more);
        if !pump.has_more {
            break;
        }
    }
    (batches, Vec::new(), exited)
}

fn mosh_drain_exited(exited: bool, has_more: bool) -> bool {
    exited && !has_more
}

fn drain_telnet_output(
    telnet: &mut TelnetTransport,
    events: &mut Vec<SessionEvent>,
) -> (Vec<u8>, bool) {
    let mut output = Vec::new();
    let mut exited = false;
    for _ in 0..MAX_POLL_OUTPUT_CHUNKS {
        let pump = telnet.drain_output();
        output.extend_from_slice(&pump.output);
        events.extend(pump.events);
        exited = pump.exited;
        if !pump.has_more {
            break;
        }
    }
    (output, exited)
}

fn drain_serial_output(
    serial: &mut SerialTransport,
    events: &mut Vec<SessionEvent>,
    serial_port: &str,
    baud_rate: u32,
) -> (Vec<u8>, bool) {
    let mut output = Vec::new();
    let mut exited = false;
    for _ in 0..MAX_POLL_OUTPUT_CHUNKS {
        let pump = serial.drain_output();
        output.extend_from_slice(&pump.output);
        if let Some(error) = pump.error.as_ref() {
            events.push(
                SessionEvent::new("error", error.to_owned()).with_serial(serial_port, baud_rate),
            );
        } else if pump.exited {
            events.push(
                SessionEvent::new(
                    "session_closed",
                    format!("Serial port {serial_port} disconnected."),
                )
                .with_serial(serial_port, baud_rate),
            );
        }
        exited = pump.exited;
        if !pump.has_more {
            break;
        }
    }
    (output, exited)
}

impl SessionTransport {
    fn resize(&mut self, geometry: TerminalGeometry) {
        match self {
            SessionTransport::Ssh(ssh) => ssh.resize(geometry),
            SessionTransport::Mosh(mosh) => mosh.resize(geometry),
            SessionTransport::Telnet(telnet) => telnet.resize(geometry),
            SessionTransport::Local
            | SessionTransport::Serial(_)
            | SessionTransport::Disconnected => {}
        }
    }

    fn set_wakeup_callback(&mut self, callback: Option<WakeupCallback>) {
        match self {
            SessionTransport::Ssh(ssh) => ssh.set_wakeup_callback(callback),
            SessionTransport::Mosh(mosh) => mosh.set_wakeup_callback(callback),
            SessionTransport::Telnet(telnet) => telnet.set_wakeup_callback(callback),
            SessionTransport::Serial(serial) => serial.set_wakeup_callback(callback),
            SessionTransport::Local | SessionTransport::Disconnected => {}
        }
    }

    fn notify_network_changed(&mut self) -> bool {
        match self {
            SessionTransport::Mosh(mosh) => mosh.notify_network_changed(),
            SessionTransport::Ssh(_)
            | SessionTransport::Telnet(_)
            | SessionTransport::Serial(_)
            | SessionTransport::Local
            | SessionTransport::Disconnected => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_unique_session_ids() {
        let mut manager = SessionManager::default();

        let ssh = manager.create_ssh(
            80,
            24,
            TerminalOptions::default(),
            "example.test",
            22,
            "admin",
            None,
            None,
            None,
            None,
            HostKeyTrustMode::Strict,
            None,
            20,
            "UTF-8",
        );
        let serial = manager.create_serial(
            80,
            24,
            TerminalOptions::default(),
            "/dev/tty.test",
            SerialConfig::new(
                115200,
                8,
                crate::serial::SerialParity::None,
                1,
                crate::serial::SerialFlowControl::None,
            )
            .unwrap(),
        );

        assert_ne!(ssh, serial);
        assert!(manager.snapshot(ssh).is_some());
        assert!(manager.close(ssh));
        assert!(manager.snapshot(ssh).is_none());
    }

    #[test]
    fn exposes_terminal_bell_and_clipboard_state() {
        let mut manager = SessionManager::default();
        let id = manager.insert(
            TerminalEngine::new(8, 2),
            SessionTransport::Local,
            TerminalOptions::default(),
        );

        assert!(manager.write_bytes(id, b"\x07\x1b]52;c;SGVsbG8=\x07"));
        assert_eq!(manager.bell_count(id), Some(1));
        assert_eq!(manager.clipboard(id).as_deref(), Some("Hello"));
    }

    #[test]
    fn ssh_reconnect_preserves_the_existing_terminal_engine() {
        let mut manager = SessionManager::default();
        let options = TerminalOptions::default();
        let mut engine = TerminalEngine::new_with_options(24, 4, options.clone());
        engine.write_bytes(b"history-before-reconnect");
        engine.mark_exited();
        let id = manager.insert(engine, SessionTransport::Disconnected, options);

        assert!(manager.reconnect_ssh(
            id,
            "127.0.0.1",
            9,
            "test",
            None,
            None,
            None,
            None,
            HostKeyTrustMode::Strict,
            None,
            20,
            "UTF-8",
        ));

        let snapshot = manager.snapshot(id).unwrap();
        assert!(terminal_snapshot_text(&snapshot).contains("history-before-reconnect"));
        assert!(!manager.is_exited(id));
        assert!(manager.close(id));
    }

    #[test]
    fn reconnect_exits_only_a_real_alternate_screen() {
        let mut manager = SessionManager::default();
        let options = TerminalOptions::default();
        let mut engine = TerminalEngine::new_with_options(24, 4, options.clone());
        engine.write_bytes(b"primary-prompt$");
        let id = manager.insert(engine, SessionTransport::Disconnected, options);

        assert!(!manager.exit_alternate_screen(id));
        let primary = manager.snapshot(id).unwrap();
        assert!(terminal_snapshot_text(&primary).contains("primary-prompt$"));

        assert!(manager.write_bytes(id, b"\x1b[?1049halternate"));
        assert!(manager.exit_alternate_screen(id));
        let restored = manager.snapshot(id).unwrap();
        assert!(terminal_snapshot_text(&restored).contains("primary-prompt$"));
        assert!(!terminal_snapshot_text(&restored).contains("alternate"));
    }

    #[test]
    fn mosh_drain_does_not_exit_while_screen_batches_remain() {
        assert!(!mosh_drain_exited(true, true));
        assert!(mosh_drain_exited(true, false));
        assert!(!mosh_drain_exited(false, false));
    }

    #[test]
    fn mosh_full_screen_update_restores_primary_screen_after_vim_exit() {
        let mut engine = TerminalEngine::new(40, 24);
        engine.write_bytes(b"PRIMARY_PROMPT");
        let mut tracker = MoshAlternateScreenTracker::default();

        let mut vim_screen = b"\x1b[?25l\x1b[1;1HVIM_BUFFER\r\n".to_vec();
        for _ in 0..21 {
            vim_screen.extend_from_slice(b"~\r\n");
        }
        vim_screen.extend_from_slice(b"~\x1b[1;1H\x1b[?25h\x1b[?2004h\x1b[?1004h");
        tracker.write_update(&mut engine, &vim_screen);

        assert!(tracker.active);
        assert!(engine.is_alt_screen());
        assert!(terminal_snapshot_text(&engine.snapshot()).contains("VIM_BUFFER"));

        tracker.write_update(
            &mut engine,
            b"\x1b[24;1H\x1b[0m:q!\r\x1b[?2004l\x1b[?1004l\r\nPRIMARY_PROMPT",
        );

        assert!(!tracker.active);
        assert!(!engine.is_alt_screen());
        let restored = terminal_snapshot_text(&engine.snapshot());
        assert!(restored.contains("PRIMARY_PROMPT"));
        assert!(!restored.contains('~'));
    }

    #[test]
    fn mosh_vim_exit_keeps_followup_output_compact_and_reopens_a_clean_alt_screen() {
        let mut engine = TerminalEngine::new(40, 6);
        engine.write_bytes(b"sh$ vim first");
        let mut tracker = MoshAlternateScreenTracker::default();

        tracker.write_update(
            &mut engine,
            b"\x1b[1;1Hfirst\r\n~\r\n~\r\n~\r\n~\x1b[?2004h",
        );
        tracker.write_update(&mut engine, b"\x1b[6;1H\x1b[?2004lsh$ ");
        tracker.write_update(&mut engine, b"\x1b[5;1Hdata\x1b[6;1Hsh$ ");

        assert!(!engine.is_alt_screen());
        assert!(engine.snapshot().cursor_row < 5);

        tracker.write_update(
            &mut engine,
            b"\x1b[1;1Hsecond\r\n~\r\n~\r\n~\r\n~\x1b[?2004h",
        );

        let reopened = terminal_snapshot_text(&engine.snapshot());
        assert!(engine.is_alt_screen());
        assert!(reopened.contains("second"));
        assert_eq!(reopened.matches('~').count(), 4);
        assert!(!reopened.contains("first"));
    }

    #[test]
    fn mosh_bracketed_paste_alone_does_not_trigger_alternate_screen() {
        let mut engine = TerminalEngine::new(40, 6);
        let mut tracker = MoshAlternateScreenTracker::default();

        tracker.write_update(&mut engine, b"\x1b[?25l\r\nsh$ \x1b[?2004h");

        assert!(!tracker.active);
        assert!(!engine.is_alt_screen());
    }

    #[test]
    fn mosh_vim_placeholders_trigger_alt_screen_without_focus_reporting() {
        let mut engine = TerminalEngine::new(40, 6);
        let mut tracker = MoshAlternateScreenTracker::default();

        tracker.write_update(&mut engine, b"\x1b[1;1Hfile.txt\r\n~\r\n~\r\n~\x1b[?2004h");
        assert!(tracker.active);
        assert!(engine.is_alt_screen());

        tracker.write_update(&mut engine, b"\x1b[?2004l\r\nsh$ ");
        assert!(!tracker.active);
        assert!(!engine.is_alt_screen());
    }

    #[test]
    fn mosh_initial_and_resize_updates_drop_synthetic_leading_lines() {
        let mut engine = TerminalEngine::new(40, 6);
        let mut tracker = MoshAlternateScreenTracker::default();

        tracker.write_update(&mut engine, b"\x1b[?25l\r\n\r\nFIRST_PROMPT");
        assert_eq!(engine.snapshot().cursor_row, 0);

        tracker.note_resize();
        tracker.write_update(&mut engine, b"\x1b[?25l\r\nRESIZED_PROMPT");
        assert_eq!(engine.snapshot().cursor_row, 0);
    }

    fn assert_empty_mosh_batch_keeps_initial_prompt_compact(engine: &mut dyn TerminalEmulator) {
        let mut tracker = MoshAlternateScreenTracker::default();

        tracker.write_update(engine, b"\r\n");
        assert!(tracker.suppress_leading_breaks);

        tracker.write_update(
            engine,
            b"\x1b[r\x1b[0m\x1b[?25l\x1b]0;admin@localhost:~\x07\x1b[32m\nsh-3.2$ \x1b[?25h",
        );

        let snapshot = engine.snapshot();
        assert_eq!(snapshot.history_lines, 0);
        assert_eq!(snapshot.cursor_row, 0);
        assert_eq!(snapshot.cursor_column, 8);
        assert_eq!(
            terminal_snapshot_text(&snapshot)
                .matches("sh-3.2$ ")
                .count(),
            1
        );
    }

    #[test]
    fn alacritty_mosh_empty_initial_batch_does_not_shift_the_prompt() {
        let mut engine = TerminalEngine::new_with_options(
            40,
            6,
            TerminalOptions {
                scrollback_lines: 0,
                ..TerminalOptions::default()
            },
        );
        assert_empty_mosh_batch_keeps_initial_prompt_compact(&mut engine);
    }

    #[test]
    fn alacritty_mosh_resize_redraw_does_not_retain_the_previous_prompt() {
        let mut engine = TerminalEngine::new_with_options(
            80,
            24,
            TerminalOptions {
                scrollback_lines: 0,
                ..TerminalOptions::default()
            },
        );
        let mut tracker = MoshAlternateScreenTracker::default();

        tracker.write_update(&mut engine, b"");
        tracker.write_update(&mut engine, b"\x1b[?25l\nsh-3.2$ \x1b[?25h");
        engine.resize(100, 30);
        tracker.note_resize();
        tracker.write_update(
            &mut engine,
            b"\x1b[r\x1b[0m\x1b[H\x1b[2J\x1b[?25l\x1b[K\nsh-3.2$ \x1b[K\r\n\x1b[K\n\x1b[K\n\x1b[K\x1b[2;9H\x1b[?25h",
        );

        let snapshot = engine.snapshot();
        assert_eq!(snapshot.history_lines, 0);
        assert_eq!(snapshot.cursor_row, 0);
        assert_eq!(snapshot.cursor_column, 8);
        assert_eq!(
            terminal_snapshot_text(&snapshot)
                .matches("sh-3.2$ ")
                .count(),
            1
        );
    }

    #[test]
    fn alacritty_mosh_primary_repaint_discards_stale_prompt_scrollback() {
        let mut engine = TerminalEngine::new_with_options(
            12,
            2,
            TerminalOptions {
                scrollback_lines: 100,
                ..TerminalOptions::default()
            },
        );
        engine.write_bytes(b"OLD_PROMPT\r\nfiller\r\n");
        assert!(engine.snapshot().history_lines > 0);

        let mut tracker = MoshAlternateScreenTracker::default();
        tracker.write_update(
            &mut engine,
            b"\x1b[r\x1b[0m\x1b[H\x1b[2J\x1b[?25l\x1b[K\nNEW_PROMPT\x1b[K\x1b[2;11H\x1b[?25h",
        );

        let snapshot = engine.snapshot();
        assert_eq!(snapshot.history_lines, 0);
        assert!(!terminal_snapshot_text(&snapshot).contains("OLD_PROMPT"));
        assert_eq!(
            terminal_snapshot_text(&snapshot)
                .matches("NEW_PROMPT")
                .count(),
            1
        );
    }

    #[cfg(feature = "terminal-ghostty")]
    #[test]
    fn ghostty_mosh_empty_initial_batch_does_not_shift_the_prompt() {
        let mut engine = crate::ghostty_terminal::GhosttyTerminalEngine::new(
            40,
            6,
            TerminalOptions {
                emulator_backend: TerminalEmulatorBackend::Ghostty,
                scrollback_lines: 0,
                ..TerminalOptions::default()
            },
        )
        .unwrap();
        assert_empty_mosh_batch_keeps_initial_prompt_compact(&mut engine);
    }

    fn assert_mosh_initial_row_offset_persists(engine: &mut dyn TerminalEmulator) {
        let mut tracker = MoshAlternateScreenTracker::default();

        tracker.write_update(
            engine,
            b"\x1b[r\x1b[0m\x1b[H\x1b[2J\x1b[?25l\x1b[K\n\x1b[K\n\x1b[K\x1b[2;1HFIRST_PROMPT",
        );
        tracker.write_update(engine, b"\x1b[2;1HSECOND_PROMPT");

        let snapshot = engine.snapshot();
        let text = terminal_snapshot_text(&snapshot);
        assert_eq!(snapshot.history_lines, 0);
        assert_eq!(snapshot.cursor_row, 0);
        assert!(!text.contains("FIRST_PROMPT"));
        assert_eq!(text.matches("SECOND_PROMPT").count(), 1);
    }

    #[test]
    fn alacritty_mosh_initial_shell_frame_offset_persists() {
        let mut engine = TerminalEngine::new_with_options(
            40,
            6,
            TerminalOptions {
                scrollback_lines: 0,
                ..TerminalOptions::default()
            },
        );
        assert_mosh_initial_row_offset_persists(&mut engine);
    }

    #[cfg(feature = "terminal-ghostty")]
    #[test]
    fn ghostty_mosh_initial_shell_frame_offset_persists() {
        let mut engine = crate::ghostty_terminal::GhosttyTerminalEngine::new(
            40,
            6,
            TerminalOptions {
                emulator_backend: TerminalEmulatorBackend::Ghostty,
                scrollback_lines: 0,
                ..TerminalOptions::default()
            },
        )
        .unwrap();
        assert_mosh_initial_row_offset_persists(&mut engine);
    }

    #[test]
    fn mosh_large_shell_output_does_not_trigger_alternate_screen() {
        let mut engine = TerminalEngine::new(40, 6);
        let mut tracker = MoshAlternateScreenTracker::default();

        tracker.write_update(
            &mut engine,
            b"\x1b[1;1Hline1\r\nline2\r\nline3\r\nline4\r\nline5\r\nline6",
        );

        assert!(!tracker.active);
        assert!(!engine.is_alt_screen());
    }

    #[cfg(feature = "terminal-ghostty")]
    #[test]
    fn session_actor_constructs_and_owns_the_selected_ghostty_backend() {
        let mut manager = SessionManager::default();
        let id = manager.create_serial_error(
            8,
            2,
            TerminalOptions {
                emulator_backend: TerminalEmulatorBackend::Ghostty,
                ..TerminalOptions::default()
            },
            "unavailable",
            9600,
            "test transport error",
        );
        assert_ne!(id, 0);
        let snapshot = manager.snapshot(id).unwrap();
        assert_eq!(snapshot.emulator_backend, TerminalEmulatorBackend::Ghostty);
        assert!(manager.close(id));
    }

    fn terminal_snapshot_text(snapshot: &TerminalSnapshot) -> String {
        let mut output = String::new();
        for cell in &snapshot.cells {
            let start = cell.text_offset as usize;
            let end = start + cell.text_len as usize;
            output.push_str(std::str::from_utf8(&snapshot.text[start..end]).unwrap());
        }
        output
    }
}
