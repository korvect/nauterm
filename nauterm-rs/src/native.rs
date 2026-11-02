//! Owned access to the shared Nauterm Rust core. No C ABI or dynamic loader.
use crate::terminal_appearance::Palette as TerminalPalette;
use anyhow::{Context, Result, ensure};
use async_channel::Receiver;
use nauterm_core::{
    session::SessionManager,
    terminal::{TerminalDefaultColors, TerminalOptions},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, RwLock, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
mod commands;
pub(crate) use commands::{CommandOwner, CommandTarget};

pub struct Core {
    pub(crate) capture: crate::recordings::capture::Api,
    sftp: crate::sftp_client::Api,
    forwarding: crate::port_forward::Api,
}

pub struct Database {
    pub(crate) inner: nauterm_core::NautermDatabase,
}
impl Database {
    pub fn call<T: serde::de::DeserializeOwned>(
        &mut self,
        request: &impl serde::Serialize,
    ) -> Result<T> {
        let request = zeroize::Zeroizing::new(serde_json::to_string(request)?);
        let response = self
            .inner
            .handle_json_request(&request)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(serde_json::from_value(response)?)
    }
}

pub(crate) fn sessions<T>(work: impl FnOnce(&mut SessionManager) -> T) -> Result<T> {
    let mut manager = nauterm_core::runtime::sessions()
        .lock()
        .map_err(|_| anyhow::anyhow!("Terminal session manager unavailable"))?;
    Ok(work(&mut manager))
}
fn terminal_options(palette: TerminalPalette) -> TerminalOptions {
    terminal_defaults()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .options(palette)
}
#[derive(Clone)]
struct TerminalDefaults {
    runtime: nauterm_core::terminal::TerminalRuntimeOptions,
    preferences: crate::terminal_preferences::Preferences,
}
impl TerminalDefaults {
    fn options(&self, palette: TerminalPalette) -> TerminalOptions {
        let mut options = self.preferences.options();
        options.default_colors = TerminalDefaultColors::from_palette(&palette.native_colors())
            .expect("validated theme palette");
        assert!(self.runtime.apply_to(&mut options));
        options
    }
}
fn terminal_defaults() -> &'static RwLock<TerminalDefaults> {
    static DEFAULTS: OnceLock<RwLock<TerminalDefaults>> = OnceLock::new();
    DEFAULTS.get_or_init(|| {
        RwLock::new(TerminalDefaults {
            runtime: crate::terminal_appearance::Appearance::default().runtime_options(),
            preferences: Default::default(),
        })
    })
}
pub fn configure_terminal_defaults(
    appearance: &crate::terminal_appearance::Appearance,
    preferences: &crate::terminal_preferences::Preferences,
) {
    *terminal_defaults()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = TerminalDefaults {
        runtime: appearance.runtime_options(),
        preferences: preferences.clone(),
    };
}
fn mapped<T: serde::de::DeserializeOwned>(value: impl serde::Serialize) -> Result<T> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}

impl Core {
    pub fn discover() -> Result<Arc<Self>> {
        nauterm_core::application::configure(nauterm_core::application::Client::NautermRs)
            .map_err(anyhow::Error::msg)?;
        static CORE: OnceLock<Arc<Core>> = OnceLock::new();
        Ok(CORE
            .get_or_init(|| {
                nauterm_core::runtime::initialize();
                Arc::new(Self {
                    capture: Default::default(),
                    sftp: Default::default(),
                    forwarding: Default::default(),
                })
            })
            .clone())
    }
    pub fn require_database(&self) -> Result<()> {
        ensure!(
            !nauterm_core::TEST_CREDENTIAL_STORE,
            "The test credential backend cannot open persistent RS data."
        );
        Ok(())
    }
    pub(crate) fn sftp_api(&self) -> Result<&crate::sftp_client::Api> {
        Ok(&self.sftp)
    }
    pub(crate) fn port_forward_api(&self) -> Result<&crate::port_forward::Api> {
        Ok(&self.forwarding)
    }

    // RS unit tests must never obtain a persistent database handle, even if a
    // future test accidentally reaches a production storage path.
    #[cfg(test)]
    pub fn open_database(self: &Arc<Self>, _path: &Path) -> Result<Database> {
        anyhow::bail!(
            "Persistent database access is disabled in RS automatic tests; use a model or mock."
        )
    }

    #[cfg(not(test))]
    pub fn open_database(self: &Arc<Self>, path: &Path) -> Result<Database> {
        self.require_database()?;
        ensure!(
            path.is_absolute(),
            "Database path must be explicit and absolute"
        );
        let inner = nauterm_core::NautermDatabase::open(path)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
            .context("Cannot open the encrypted Nauterm RS database")?;
        Ok(Database { inner })
    }
    pub fn local(self: &Arc<Self>, palette: TerminalPalette) -> Result<Session> {
        self.local_configured(None, None, palette)
    }
    pub fn local_shell(self: &Arc<Self>, shell: &str, palette: TerminalPalette) -> Result<Session> {
        ensure!(
            crate::shells::available(shell),
            "The selected shell is not an available executable."
        );
        self.local_configured(Some(shell), None, palette)
    }
    pub fn restore_local(
        self: &Arc<Self>,
        shell: Option<&str>,
        directory: Option<&str>,
        palette: TerminalPalette,
    ) -> Result<Session> {
        ensure!(
            shell.is_none_or(crate::shells::available),
            "The local shell is not available."
        );
        ensure!(
            directory.is_none_or(|directory| crate::recovery::safe_directory(directory)
                && Path::new(directory).is_dir()),
            "The local directory is not available."
        );
        self.local_configured(shell, directory, palette)
    }
    fn local_configured(
        self: &Arc<Self>,
        shell: Option<&str>,
        directory: Option<&str>,
        palette: TerminalPalette,
    ) -> Result<Session> {
        let defaults = terminal_defaults()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut options = defaults.options(palette);
        options.shell_path = shell
            .or(defaults.preferences.shell_path.as_deref())
            .map(str::to_owned);
        ensure!(
            options
                .shell_path
                .as_deref()
                .is_none_or(crate::shells::available),
            "The configured shell is not an available executable."
        );
        options.working_directory = directory.map(PathBuf::from);
        let history_shell = options.shell_path.clone().or_else(|| {
            if cfg!(windows) {
                let path = std::env::var_os("PATH")?;
                for name in ["pwsh.exe", "powershell.exe", "cmd.exe"] {
                    for directory in std::env::split_paths(&path) {
                        let candidate = directory.join(name);
                        if candidate.is_file() {
                            return candidate.to_str().map(str::to_owned);
                        }
                    }
                }
                None
            } else {
                std::env::var("SHELL").ok()
            }
        });
        let integration = crate::ai::terminal_runner::Integration::local(
            options.shell_path.as_deref(),
            options.command.is_none(),
        );
        if let Some(integration) = &integration {
            options
                .environment
                .retain(|entry| entry.variable != "NAUTERM_SHELL_INTEGRATION_TOKEN");
            options
                .environment
                .push(nauterm_core::terminal::TerminalEnvironmentVariable {
                    variable: "NAUTERM_SHELL_INTEGRATION_TOKEN".into(),
                    value: integration.token.clone(),
                });
        }
        let id = sessions(|manager| manager.create_local(100, 30, options))?
            .context("Cannot open a local PTY")?;
        let session = self.attach(
            id,
            palette.native_colors(),
            HistorySource::Local(history_shell),
        )?;
        *session.command_integration.lock().unwrap() = integration;
        Ok(session)
    }
    pub(crate) fn fido2(self: &Arc<Self>) -> Result<crate::fido2::Client> {
        Ok(crate::fido2::Client::new(self.clone()))
    }
    pub fn serial_ports(&self) -> Result<Vec<crate::serial::Port>> {
        mapped(nauterm_core::serial::list_serial_ports())
    }
    pub fn serial(
        self: &Arc<Self>,
        config: &crate::serial::Config,
        palette: TerminalPalette,
    ) -> Result<Session> {
        use nauterm_core::serial::{SerialConfig, SerialFlowControl, SerialParity};
        config.validate()?;
        let native = SerialConfig::new(
            config.baud,
            config.data_bits as u8,
            match config.parity as u32 {
                1 => SerialParity::Even,
                2 => SerialParity::Odd,
                _ => SerialParity::None,
            },
            config.stop_bits as u8,
            match config.flow as u32 {
                1 => SerialFlowControl::Software,
                2 => SerialFlowControl::Hardware,
                _ => SerialFlowControl::None,
            },
        )?;
        let id = sessions(|manager| {
            manager.create_serial(100, 30, terminal_options(palette), &config.path, native)
        })?;
        ensure!(id != 0, "Could not create serial session");
        self.attach(
            id,
            palette.native_colors(),
            HistorySource::Serial(config.clone()),
        )
    }
    pub fn telnet(
        self: &Arc<Self>,
        options: &crate::telnet::Options,
        palette: TerminalPalette,
    ) -> Result<Session> {
        options.validate()?;
        let id = sessions(|manager| {
            manager.create_telnet(
                100,
                30,
                terminal_options(palette),
                &options.host,
                options.port,
                options.encoding.label(),
            )
        })?;
        ensure!(id != 0, "Could not create Telnet session");
        let mut session = self.attach(
            id,
            palette.native_colors(),
            HistorySource::Telnet {
                host: options.host.clone(),
                port: options.port,
                username: options.username.clone(),
            },
        )?;
        session.telnet = Some(crate::telnet::Session {
            options: options.clone(),
            login: Default::default(),
        });
        Ok(session)
    }

    pub(crate) fn ssh_recorded(
        self: &Arc<Self>,
        options: &SshOptions,
        palette: TerminalPalette,
        recording: Option<crate::recordings::Start>,
    ) -> Result<Session> {
        self.remote_recorded(options, None, palette, recording)
    }
    pub(crate) fn mosh_recorded(
        self: &Arc<Self>,
        options: &SshOptions,
        config: &crate::mosh::Config,
        palette: TerminalPalette,
        recording: Option<crate::recordings::Start>,
    ) -> Result<Session> {
        config.validate()?;
        self.remote_recorded(options, Some(config), palette, recording)
    }
    fn remote_recorded(
        self: &Arc<Self>,
        options: &SshOptions,
        mosh: Option<&crate::mosh::Config>,
        palette: TerminalPalette,
        recording: Option<crate::recordings::Start>,
    ) -> Result<Session> {
        let args = SshArguments::new(options)?;
        let defaults = terminal_defaults()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut terminal = defaults.options(palette);
        terminal.environment = options
            .environment
            .iter()
            .map(
                |value| nauterm_core::terminal::TerminalEnvironmentVariable {
                    variable: value.variable.clone(),
                    value: value.value.clone(),
                },
            )
            .collect();
        let id = sessions(|manager| {
            if let Some(config) = mosh {
                return manager.create_mosh(
                    100,
                    30,
                    terminal,
                    &options.host,
                    options.port,
                    &options.username,
                    Some(&options.password),
                    Some(&options.private_key),
                    Some(&options.certificate),
                    Some(args.auth()),
                    Some(&args.known_hosts),
                    options.trust.native(),
                    args.native_proxy(),
                    config.native(),
                );
            }
            manager.create_ssh(
                100,
                30,
                terminal,
                &options.host,
                options.port,
                &options.username,
                Some(&options.password),
                Some(&options.private_key),
                Some(&options.certificate),
                Some(args.auth()),
                Some(&args.known_hosts),
                options.trust.native(),
                args.native_proxy(),
                defaults.preferences.ssh_keepalive_seconds,
                options.encoding.label(),
            )
        })?;
        ensure!(id != 0, "Could not create SSH session");
        let mut session = self.attach(
            id,
            palette.native_colors(),
            HistorySource::Ssh {
                host: options.host.clone(),
                port: options.port,
                username: options.username.clone(),
            },
        )?;
        session.mosh = mosh.cloned();
        session.defer_recording(recording);
        *session.startup.lock().unwrap() =
            Some(crate::host_session::startup_input(&options.startup)?);
        Ok(session)
    }
    fn attach(
        self: &Arc<Self>,
        id: u64,
        palette: [u32; 19],
        history_source: HistorySource,
    ) -> Result<Session> {
        let (sender, receiver) = async_channel::bounded(1);
        let notify = sender.clone();
        let subscription = sessions(|manager| {
            manager.subscribe(id, move || {
                let _ = notify.try_send(());
            })
        })?
        .context("Cannot subscribe to terminal events")?;
        let session = Session {
            palette: Some(palette),
            core: self.clone(),
            id,
            _subscription: subscription,
            sender,
            receiver,
            size: (100, 30),
            history_source,
            telnet: None,
            mosh: None,
            history_generation: Arc::new(AtomicUsize::new(0)),
            system_info_inflight: Arc::new(AtomicBool::new(false)),
            history_capture: crate::shell_history::Capture::default(),
            recording: None,
            pending_recording: None,
            deferred_events: vec![],
            connection_history_recorded: false,
            startup: Mutex::new(None),
            runtime_options: None,
            command_output: None,
            command_integration: Arc::new(Mutex::new(None)),
        };
        let _ = session.sender.try_send(());
        Ok(session)
    }
    pub(crate) fn replay(
        self: &Arc<Self>,
        columns: u32,
        rows: u32,
        palette: TerminalPalette,
    ) -> Result<Session> {
        let size = (columns.clamp(2, 1000), rows.clamp(1, 500));
        let id = sessions(|manager| {
            manager.create_replay(size.0 as usize, size.1 as usize, terminal_options(palette))
        })?;
        ensure!(id != 0, "Cannot create recording replay terminal");
        let mut session = self.attach(id, palette.native_colors(), HistorySource::Replay)?;
        session.size = size;
        Ok(session)
    }
    pub fn prepare_shutdown() {
        nauterm_core::runtime::prepare_shutdown();
    }
}

struct PendingRecording {
    start: crate::recordings::Start,
    events: Vec<crate::recordings::Event>,
}
pub struct Session {
    command_output: Option<commands::Tap>,
    command_integration: Arc<Mutex<Option<crate::ai::terminal_runner::Integration>>>,
    recording: Option<crate::recordings::Recorder>,
    pending_recording: Option<PendingRecording>,
    deferred_events: Vec<SessionEvent>,
    connection_history_recorded: bool,
    startup: Mutex<Option<zeroize::Zeroizing<Vec<u8>>>>,
    palette: Option<[u32; 19]>,
    runtime_options: Option<nauterm_core::terminal::TerminalRuntimeOptions>,
    core: Arc<Core>,
    id: u64,
    _subscription: nauterm_core::subscription::Subscription,
    sender: async_channel::Sender<()>,
    receiver: Receiver<()>,
    size: (u32, u32),
    history_source: HistorySource,
    telnet: Option<crate::telnet::Session>,
    mosh: Option<crate::mosh::Config>,
    history_generation: Arc<AtomicUsize>,
    system_info_inflight: Arc<AtomicBool>,
    history_capture: crate::shell_history::Capture,
}

#[derive(Clone)]
enum HistorySource {
    Replay,
    Serial(crate::serial::Config),
    Telnet {
        host: String,
        port: u16,
        username: String,
    },
    Local(Option<String>),
    Ssh {
        host: String,
        port: u16,
        username: String,
    },
}
pub struct HistoryRequest {
    _core: Arc<Core>,
    id: u64,
    source: HistorySource,
    lease: SessionLease,
}
pub(crate) struct SystemInfoRequest {
    _core: Arc<Core>,
    options: SshOptions,
    lease: SessionLease,
    inflight: Arc<AtomicBool>,
}
enum HostOsSource {
    Connected(nauterm_core::ssh::HostOsReceiver),
    Auxiliary(Box<SshOptions>),
}
pub(crate) struct HostOsRequest {
    source: HostOsSource,
    pub lease: HostOsLease,
}
#[derive(Clone)]
pub(crate) enum HostOsLease {
    Terminal(SessionLease),
    Sftp(Weak<()>),
}
impl HostOsLease {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Terminal(lease) => lease.validate(),
            Self::Sftp(owner) => {
                ensure!(owner.upgrade().is_some(), "SFTP endpoint owner changed");
                Ok(())
            }
        }
    }
}
impl HostOsRequest {
    pub fn for_sftp(options: &SshOptions, owner: Weak<()>) -> Self {
        Self {
            source: HostOsSource::Auxiliary(Box::new(options.for_new_connection())),
            lease: HostOsLease::Sftp(owner),
        }
    }
    pub fn read(self) -> Result<crate::avatar::Platform> {
        self.lease.validate()?;
        let detected = match self.source {
            HostOsSource::Connected(receiver) => receiver
                .recv_timeout(std::time::Duration::from_secs(30))
                .context("Host OS detection did not complete")?
                .map_err(anyhow::Error::msg)?,
            HostOsSource::Auxiliary(options) => {
                let args = SshArguments::new(&options)?;
                nauterm_core::ssh::detect_host_os_blocking_with_trust(
                    &options.host,
                    options.port,
                    &options.username,
                    Some(&options.password),
                    Some(&options.private_key),
                    Some(&options.certificate),
                    Some(args.auth()),
                    Some(&args.known_hosts),
                    args.native_proxy(),
                    HostTrust::Strict.native(),
                )
            }
        };
        self.lease.validate()?;
        ensure!(detected.error.is_none(), "Host OS detection failed");
        crate::host_platform::detected(detected.os, detected.distro)
    }
}
pub(crate) struct BlockRequest {
    core: Arc<Core>,
    id: u64,
    lease: SessionLease,
    absolute: i64,
    frame: Arc<Frame>,
}
impl BlockRequest {
    pub fn read(self) -> Result<Option<crate::terminal::blocks::Block>> {
        self.lease.validate()?;
        let block = read_command_block(&self.core, self.id, self.absolute, &self.frame)?;
        self.lease.validate()?;
        Ok(block)
    }
}
impl SystemInfoRequest {
    /// Blocking collector on a separate strictly verified SSH exec connection.
    /// Never injects a command into the user's PTY or accepts/writes host keys.
    pub fn read(self) -> Result<Option<crate::system_info::data::Snapshot>> {
        self.lease.validate()?;
        // Dropping a UI task cannot interrupt synchronous FFI. Keep this gate
        // on the session, so rapid hide/show or pane changes never overlap probes.
        if self
            .inflight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(None);
        }
        struct Release(Arc<AtomicBool>);
        impl Drop for Release {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _release = Release(self.inflight.clone());
        let args = SshArguments::new(&self.options)?;
        let native = nauterm_core::ssh::collect_host_system_info_blocking_with_trust(
            &self.options.host,
            self.options.port,
            &self.options.username,
            Some(&self.options.password),
            Some(&self.options.private_key),
            Some(&self.options.certificate),
            Some(args.auth()),
            Some(&args.known_hosts),
            args.native_proxy(),
            HostTrust::Strict.native(),
        );
        let mut info: crate::system_info::data::Snapshot = mapped(native)?;
        info.processes.truncate(200);
        info.network_interfaces.truncate(256);
        info.filesystems.truncate(256);
        self.lease.validate()?;
        Ok(Some(info))
    }
}
#[derive(Clone)]
pub(crate) struct SessionLease {
    counter: Arc<AtomicUsize>,
    generation: usize,
}
impl SessionLease {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.counter.load(Ordering::Acquire) == self.generation,
            "Terminal session owner changed"
        );
        Ok(())
    }
}
impl HistoryRequest {
    /// Blocking, session-bound read. No second SSH connection or shell command
    /// is sent to the terminal prompt. Closed/reconnected owners invalidate it.
    pub fn read(self) -> Result<Vec<crate::shell_history::Entry>> {
        self.lease.validate()?;
        let entries = match &self.source {
            HistorySource::Local(Some(shell)) => crate::shell_history::Locations::environment()
                .map(|locations| crate::shell_history::read_local(shell, &locations))
                .transpose()?
                .unwrap_or_default(),
            HistorySource::Local(None)
            | HistorySource::Replay
            | HistorySource::Serial(_)
            | HistorySource::Telnet { .. } => vec![],
            HistorySource::Ssh { .. } => {
                let receiver = sessions(|manager| manager.request_ssh_shell_history(self.id))?
                    .map_err(anyhow::Error::msg)?;
                let (shell, content) = receiver
                    .recv_timeout(std::time::Duration::from_secs(8))
                    .context("SSH shell history request timed out")?
                    .map_err(anyhow::Error::msg)?;
                let content = zeroize::Zeroizing::new(content);
                ensure!(
                    content.len() <= 16 * 1024 * 1024,
                    "Shell history response exceeded its limit"
                );
                let format = shell
                    .as_deref()
                    .and_then(crate::shell_history::Format::for_shell)
                    .context("Unsupported remote shell history format")?;
                crate::shell_history::parse(
                    &content,
                    format,
                    chrono::Utc::now().timestamp_millis(),
                    crate::shell_history::LIMIT,
                )
            }
        };
        self.lease.validate()?;
        Ok(entries)
    }
}

impl Session {
    pub(crate) fn mosh_config(&self) -> Option<crate::mosh::Config> {
        self.mosh.clone()
    }
    pub(crate) fn reconnect_boundary(&self) -> Result<()> {
        ensure!(
            matches!(self.history_source, HistorySource::Ssh { .. }),
            "Only SSH transports can reconnect."
        );
        ensure!(
            sessions(|manager| {
                manager.exit_alternate_screen(self.id);
                manager.write_bytes(
                    self.id,
                    b"\r\n\x1b[2m[Connection lost. Reconnecting...]\x1b[0m\r\n",
                )
            })?,
            "Could not mark the reconnect boundary."
        );
        Ok(())
    }
    pub(crate) fn is_mosh(&self) -> bool {
        self.mosh.is_some()
    }
    pub fn is_replay(&self) -> bool {
        matches!(self.history_source, HistorySource::Replay)
    }
    pub(crate) fn write_replay(&self, bytes: &[u8]) -> Result<()> {
        ensure!(
            self.is_replay(),
            "Output injection is available only to a replay terminal."
        );
        ensure!(
            sessions(|manager| manager.write_replay(self.id, bytes.to_vec()))?,
            "Cannot decode recording output."
        );
        Ok(())
    }
    pub fn prompt_click_move(
        &self,
        absolute: i64,
        frame: &Frame,
    ) -> Result<Option<crate::terminal::prompt_click::Movement>> {
        let offset = absolute - i64::from(frame.columns) * i64::from(frame.history_lines);
        ensure!(
            offset >= 0 && offset < i64::from(frame.columns) * i64::from(frame.rows),
            "Prompt cursor target is outside the screen"
        );
        mapped(sessions(|manager| {
            manager.prompt_click_move(self.id, offset)
        })?)
    }
    pub fn local_shell(&self) -> Option<String> {
        match &self.history_source {
            HistorySource::Local(shell) => shell.clone(),
            _ => None,
        }
    }
    pub fn is_local(&self) -> bool {
        matches!(self.history_source, HistorySource::Local(_))
    }
    pub fn serial_config(&self) -> Option<&crate::serial::Config> {
        match &self.history_source {
            HistorySource::Serial(config) => Some(config),
            _ => None,
        }
    }
    pub fn telnet_options(&self) -> Option<&crate::telnet::Options> {
        self.telnet.as_ref().map(|session| &session.options)
    }

    pub fn command_block_at(
        &self,
        absolute: i64,
        frame: &Frame,
    ) -> Result<Option<crate::terminal::blocks::Block>> {
        read_command_block(&self.core, self.id, absolute, frame)
    }
    pub fn command_block_request(&self, absolute: i64, frame: Arc<Frame>) -> BlockRequest {
        BlockRequest {
            core: self.core.clone(),
            id: self.id,
            lease: self.lease(),
            absolute,
            frame,
        }
    }
    pub fn local_command_context(
        &self,
        absolute: i64,
        frame: &Frame,
    ) -> Result<LocalCommandContext> {
        ensure!(self.is_local(), "Local paths require a local terminal");
        Ok(self
            .command_block_at(absolute, frame)?
            .map(|block| LocalCommandContext {
                directory: block.directory,
                prompt: Some(block.prompt),
            })
            .unwrap_or_default())
    }
    pub(crate) fn system_info_request(&self, options: &SshOptions) -> Result<SystemInfoRequest> {
        ensure!(
            matches!(&self.history_source, HistorySource::Ssh { host, port, username }
                if *host == options.host && *port == options.port && *username == options.username),
            "System information requires its original SSH endpoint"
        );
        Ok(SystemInfoRequest {
            _core: self.core.clone(),
            options: options.for_new_connection(),
            lease: self.lease(),
            inflight: self.system_info_inflight.clone(),
        })
    }
    pub(crate) fn host_os_request(&self, options: &SshOptions) -> Result<HostOsRequest> {
        ensure!(
            matches!(&self.history_source, HistorySource::Ssh { host, port, username }
            if *host == options.host && *port == options.port && *username == options.username),
            "Host OS detection requires its connected SSH endpoint"
        );
        let source = if self.mosh.is_some() {
            HostOsSource::Auxiliary(Box::new(options.for_new_connection()))
        } else {
            HostOsSource::Connected(
                sessions(|manager| manager.request_ssh_host_os(self.id))?
                    .map_err(anyhow::Error::msg)?,
            )
        };
        Ok(HostOsRequest {
            source,
            lease: HostOsLease::Terminal(self.lease()),
        })
    }
    fn lease(&self) -> SessionLease {
        SessionLease {
            counter: self.history_generation.clone(),
            generation: self.history_generation.load(Ordering::Acquire),
        }
    }
    pub(crate) fn path_source(&self, options: Option<SshOptions>) -> crate::path_reader::Source {
        match &self.history_source {
            HistorySource::Replay | HistorySource::Serial(_) | HistorySource::Telnet { .. } => {
                crate::path_reader::Source::default()
            }
            HistorySource::Local(_) => crate::path_reader::Source::local(self.lease()),
            HistorySource::Ssh {
                host,
                port,
                username,
            } => options
                .filter(|options| {
                    options.host.eq_ignore_ascii_case(host)
                        && options.port == *port
                        && options.username == *username
                })
                .map(|options| {
                    crate::path_reader::Source::remote(
                        self.core.clone(),
                        options.for_new_connection(),
                        self.lease(),
                    )
                })
                .unwrap_or_default(),
        }
    }
    pub fn take_shell_commands(&mut self) -> Vec<String> {
        let commands = self.history_capture.take();
        if let Some(recorder) = &self.recording {
            recorder.commands(&commands);
        }
        commands
    }
    pub fn history_request(&self) -> HistoryRequest {
        HistoryRequest {
            _core: self.core.clone(),
            id: self.id,
            source: self.history_source.clone(),
            lease: self.lease(),
        }
    }
    pub fn history_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(
            if matches!(self.history_source, HistorySource::Ssh { .. }) {
                5
            } else {
                2
            },
        )
    }
    pub fn supports_shell_history(&self) -> bool {
        matches!(
            self.history_source,
            HistorySource::Local(_) | HistorySource::Ssh { .. }
        )
    }
    pub fn apply_palette(&mut self, palette: TerminalPalette) -> Result<()> {
        let colors = palette.native_colors();
        if self.palette == Some(colors) {
            return Ok(());
        }
        let colors_native =
            TerminalDefaultColors::from_palette(&colors).context("Invalid terminal palette")?;
        ensure!(
            sessions(|manager| manager.rs_set_palette(self.id, colors_native))?,
            "Could not update terminal palette"
        );
        self.palette = Some(colors);
        Ok(())
    }
    pub fn apply_runtime_options(
        &mut self,
        options: nauterm_core::terminal::TerminalRuntimeOptions,
    ) -> Result<()> {
        if self.runtime_options != Some(options) {
            ensure!(
                sessions(|manager| manager.rs_set_runtime_options(self.id, options))?,
                "Could not update terminal settings"
            );
            self.runtime_options = Some(options);
        }
        Ok(())
    }
    /// Blocking operation: the caller must run it on a background worker and
    /// keep this Session alive until completion. It never opens another SSH
    /// connection and never infers success from transport completion alone.
    pub fn export_public_key(&self, request: &crate::export_request::ExportRequest) -> Result<()> {
        request.validate()?;
        let request = serde_json::from_value(serde_json::to_value(request)?)?;
        let receiver = sessions(|manager| manager.request_ssh_key_export(self.id, request))?
            .map_err(anyhow::Error::msg)?;
        receiver
            .recv_timeout(std::time::Duration::from_secs(20))
            .context("Key export did not complete. Check the remote file before retrying.")?
            .map_err(anyhow::Error::msg)
    }
    pub fn reconnect_remote(&mut self, options: &SshOptions) -> Result<()> {
        ensure!(
            !self.is_replay(),
            "A recording replay cannot start an SSH connection."
        );
        let args = SshArguments::new(options)?;
        let keepalive = terminal_defaults()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .preferences
            .ssh_keepalive_seconds;
        ensure!(
            sessions(|manager| {
                // Invalidate request owners under the same lock that switches
                // transport, so an old approval cannot race onto a new SSH peer.
                self.history_generation.fetch_add(1, Ordering::AcqRel);
                self.close_command_output();
                *self.command_integration.lock().unwrap() = None;
                if let Some(config) = &self.mosh {
                    return manager.reconnect_mosh(
                        self.id,
                        &options.host,
                        options.port,
                        &options.username,
                        Some(&options.password),
                        Some(&options.private_key),
                        Some(&options.certificate),
                        Some(args.auth()),
                        Some(&args.known_hosts),
                        options.trust.native(),
                        args.native_proxy(),
                        config.native(),
                    );
                }
                manager.reconnect_ssh(
                    self.id,
                    &options.host,
                    options.port,
                    &options.username,
                    Some(&options.password),
                    Some(&options.private_key),
                    Some(&options.certificate),
                    Some(args.auth()),
                    Some(&args.known_hosts),
                    options.trust.native(),
                    args.native_proxy(),
                    keepalive,
                    options.encoding.label(),
                )
            })?,
            "SSH reconnect was rejected"
        );
        self.history_source = HistorySource::Ssh {
            host: options.host.clone(),
            port: options.port,
            username: options.username.clone(),
        };
        self.history_capture = crate::shell_history::Capture::default();
        *self.startup.lock().unwrap() = Some(crate::host_session::startup_input(&options.startup)?);
        let _ = self.sender.try_send(());
        Ok(())
    }

    fn native_events(&self) -> Result<Vec<SessionEvent>> {
        mapped(sessions(|manager| manager.drain_events(self.id))?)
    }
    fn record_events(&mut self, events: &[SessionEvent]) {
        if let Some(pending) = &mut self.pending_recording {
            pending.events.extend(
                events
                    .iter()
                    .filter_map(crate::recordings::Event::connection),
            );
            if pending.events.len() > 2000 {
                pending.events.drain(..pending.events.len() - 2000);
            }
            if events.iter().any(|event| event.kind == "connected") {
                let pending = self.pending_recording.take().unwrap();
                self.start_recording(&pending.start);
                if let Some(recorder) = &self.recording {
                    recorder.events(pending.events);
                    self.connection_history_recorded = true;
                }
            }
        } else if let Some(recorder) = &self.recording {
            for event in events {
                recorder.event(event);
            }
        }
    }
    pub fn drain_events(&mut self) -> Result<Vec<SessionEvent>> {
        let new = self.native_events()?;
        self.record_events(&new);
        let mut events = std::mem::take(&mut self.deferred_events);
        events.extend(new);
        if events.iter().any(|event| event.kind == "connected")
            && let Some(command) = self
                .startup
                .lock()
                .unwrap()
                .take()
                .filter(|bytes| !bytes.is_empty())
            && let Err(error) = self.send(&command)
        {
            events.push(SessionEvent {
                kind: "warning".into(),
                message: format!("Could not send the startup snippet: {error}"),
                ..Default::default()
            });
        }
        Ok(events)
    }

    pub(crate) fn record(&mut self, start: &crate::recordings::Start) {
        if self.is_replay() || self.recording.is_some() {
            return;
        }
        self.pending_recording = None;
        self.start_recording(start);
    }
    pub(crate) fn defer_recording(&mut self, recording: Option<crate::recordings::Start>) {
        self.pending_recording = recording.map(|start| PendingRecording {
            start,
            events: vec![],
        });
    }
    fn start_recording(&mut self, start: &crate::recordings::Start) {
        let mut entry = crate::recordings::Entry {
            title: start.title.clone(),
            host_id: start.host_id,
            host_uuid: start.host_uuid.clone(),
            theme_id: Some(start.theme.id().into()),
            started_at: start.started_at,
            ..Default::default()
        };
        match &self.history_source {
            HistorySource::Replay => return,
            // As in the reference catalog, serial sessions have no SSH
            // endpoint/account or local shell. Device parameters are retained
            // in their native connection events rather than fake SSH fields.
            HistorySource::Serial(_) => {}
            HistorySource::Local(shell) => {
                entry.shell_path = shell.clone().or_else(|| std::env::var("SHELL").ok());
                entry.username = std::env::var("USER")
                    .or_else(|_| std::env::var("USERNAME"))
                    .ok();
            }
            HistorySource::Ssh {
                host,
                port,
                username,
            }
            | HistorySource::Telnet {
                host,
                port,
                username,
            } => {
                entry.host = Some(host.clone());
                entry.port = Some(*port);
                entry.username = Some(username.clone());
            }
        }
        self.recording = start
            .service
            .begin_at(entry, start.preferences, start.generation);
    }
    pub fn recording_frame(&self, frame: &Frame) {
        if let Some(recorder) = &self.recording {
            recorder.update(
                "",
                frame.working_directory.clone(),
                frame.columns,
                frame.rows,
                frame.exited,
            );
        }
    }
    pub fn recording_error(&self) -> Option<String> {
        self.recording
            .as_ref()
            .and_then(|recorder| recorder.error())
    }
    pub fn recording_connection_history<'a>(
        &mut self,
        events: impl IntoIterator<Item = &'a SessionEvent>,
    ) {
        if self.connection_history_recorded {
            return;
        }
        if let Some(recorder) = &self.recording {
            for event in events {
                recorder.event(event);
            }
            self.connection_history_recorded = true;
        }
    }

    pub fn notifications(&self) -> Receiver<()> {
        self.receiver.clone()
    }

    pub fn send(&self, bytes: &[u8]) -> Result<()> {
        ensure!(!self.is_replay(), "Recording replay is read-only.");
        if bytes.contains(&3)
            && let Some(tap) = &self.command_output
        {
            tap.interrupted.cancel();
        }
        ensure!(
            sessions(|manager| manager.send_input_bytes(self.id, bytes))?,
            "PTY input was rejected"
        );
        Ok(())
    }

    pub fn paste(&self, text: &str) -> Result<()> {
        self.send(self.encode_paste(text)?.as_bytes())
    }

    pub(crate) fn encode_paste(&self, text: &str) -> Result<String> {
        sessions(|manager| manager.encode_paste(self.id, text.to_owned()))?
            .map_err(anyhow::Error::msg)
    }

    pub fn selected_text(&self, start: i64, end: i64) -> Option<String> {
        sessions(|manager| manager.selection_text(self.id, start, end))
            .ok()
            .flatten()
    }
    pub fn word_selection_at(
        &self,
        absolute: i64,
        boundaries: &str,
        frame: &Frame,
    ) -> Result<Option<crate::terminal_search::Selection>> {
        let offset = absolute - i64::from(frame.columns) * i64::from(frame.history_lines);
        let range =
            sessions(|manager| manager.word_selection_at(self.id, offset, boundaries.to_owned()))?;
        range
            .map(|range| {
                let selection = crate::terminal_search::Selection::from_native(
                    range,
                    frame.columns,
                    frame.rows,
                    frame.history_lines,
                )?;
                ensure!(
                    selection.contains(absolute),
                    "Native word selection missed its reference cell"
                );
                Ok(selection)
            })
            .transpose()
    }

    pub fn search(
        &self,
        query: &str,
        direction: crate::terminal_search::Direction,
        origin: (u32, u32),
    ) -> Result<crate::terminal_search::NativeResult> {
        let direction = match direction as u32 {
            1 => nauterm_core::terminal::TerminalSearchDirection::Backward,
            _ => nauterm_core::terminal::TerminalSearchDirection::Forward,
        };
        let result: crate::terminal_search::NativeResult = mapped(sessions(|manager| {
            manager.search(
                self.id,
                query,
                direction,
                origin.0 as usize,
                origin.1 as usize,
            )
        })?)?;
        result.validate()?;
        Ok(result)
    }

    pub fn scroll(&self, lines: i32) {
        let _ = sessions(|manager| manager.scroll_lines(self.id, lines));
    }
    pub fn bottom(&self) {
        let _ = sessions(|manager| manager.scroll_to_bottom(self.id));
    }
    pub fn page_up(&self) {
        let _ = sessions(|manager| manager.scroll_page_up(self.id));
    }
    pub fn page_down(&self) {
        let _ = sessions(|manager| manager.scroll_page_down(self.id));
    }

    pub fn resize(&mut self, columns: u32, rows: u32, cell_width: u32, cell_height: u32) {
        let size = terminal_grid_size(columns, rows);
        if size != self.size
            && sessions(|manager| {
                manager.resize(
                    self.id,
                    size.0 as usize,
                    size.1 as usize,
                    cell_width,
                    cell_height,
                )
            })
            .unwrap_or(false)
        {
            self.size = size;
        }
    }

    pub fn refresh(&mut self) -> Result<Frame> {
        sessions(|manager| manager.poll(self.id))?;
        if self.pending_recording.is_some() {
            let events = self.native_events()?;
            self.record_events(&events);
            self.deferred_events.extend(events);
            if self.deferred_events.len() > 2000 {
                self.deferred_events
                    .drain(..self.deferred_events.len() - 2000);
            }
        }
        let raw = sessions(|manager| manager.snapshot(self.id))?
            .context("Terminal snapshot is unavailable")?;
        if self.pending_recording.is_none() {
            let bytes =
                zeroize::Zeroizing::new(sessions(|manager| manager.drain_output_capture(self.id))?);
            self.history_capture.feed_bytes(&bytes);
            if let Some(tap) = &self.command_output {
                tap.publish(&bytes);
            }
            let login = self
                .telnet
                .as_mut()
                .map(|session| session.login.feed_bytes(&bytes, &session.options))
                .unwrap_or_default();
            for credential in login {
                let options = &self
                    .telnet
                    .as_ref()
                    .expect("Telnet login owns its options")
                    .options;
                let value = match credential {
                    crate::telnet::Credential::Username => options.username.trim(),
                    crate::telnet::Credential::Password => options.password.as_str(),
                };
                let mut input = zeroize::Zeroizing::new(value.as_bytes().to_vec());
                input.push(b'\r');
                if let Err(error) = self.send(&input) {
                    self.deferred_events.push(SessionEvent {
                        kind: "warning".into(),
                        message: format!("Could not send saved Telnet login input: {error}"),
                        ..Default::default()
                    });
                }
            }
            if let Some(recorder) = &self.recording {
                recorder.output(&bytes);
            }
        }
        let exited = sessions(|manager| manager.is_exited(self.id))?;
        if exited {
            self.close_command_output();
        }
        ensure!(
            (2..=1000).contains(&raw.columns) && (1..=500).contains(&raw.rows),
            "Invalid terminal dimensions"
        );
        ensure!(
            raw.cells.len() == raw.columns * raw.rows,
            "Invalid terminal cell count"
        );
        let text = &raw.text;
        let hyperlinks = &raw.hyperlink_text;
        let mut hyperlink_cache: HashMap<(u32, u32), Arc<str>> = HashMap::new();
        let cells = raw
            .cells
            .iter()
            .map(|cell| {
                let start = cell.text_offset as usize;
                let end = start
                    .checked_add(cell.text_len as usize)
                    .context("Cell text overflow")?;
                let text =
                    std::str::from_utf8(text.get(start..end).context("Cell text out of bounds")?)?;
                let hyperlink = if cell.hyperlink_len == 0 {
                    None
                } else if let Some(link) =
                    hyperlink_cache.get(&(cell.hyperlink_offset, cell.hyperlink_len))
                {
                    Some(link.clone())
                } else {
                    let start = cell.hyperlink_offset as usize;
                    let end = start
                        .checked_add(cell.hyperlink_len as usize)
                        .context("Cell hyperlink overflow")?;
                    let link: Arc<str> = std::str::from_utf8(
                        hyperlinks
                            .get(start..end)
                            .context("Cell hyperlink out of bounds")?,
                    )?
                    .into();
                    hyperlink_cache
                        .insert((cell.hyperlink_offset, cell.hyperlink_len), link.clone());
                    Some(link)
                };
                Ok(Cell {
                    text: text.to_owned(),
                    hyperlink,
                    foreground: cell.foreground,
                    background: cell.background,
                    flags: cell.flags,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Frame {
            columns: raw.columns as u32,
            rows: raw.rows as u32,
            cells,
            title: raw.title,
            bell_count: raw.bell_count,
            working_directory: sessions(|manager| manager.rs_working_directory(self.id))?,
            alternate_screen: raw.alternate_screen,
            cursor: (raw.cursor_column as u32, raw.cursor_row as u32),
            cursor_visible: !self.is_replay() && raw.cursor_visible,
            cursor_shape: raw.cursor_shape,
            cursor_blinking: raw.cursor_blinking,
            cursor_color: raw.cursor_color,
            keyboard_mode: if self.is_replay() {
                0
            } else {
                raw.keyboard_mode
            },
            input_echo_enabled: raw.input_echo_enabled,
            graphics: crate::terminal_graphics::Snapshot::from_native(
                raw.graphic_images,
                raw.graphic_placements,
            ),
            history_lines: raw.history_lines as u32,
            display_offset: raw.display_offset as u32,
            exited,
        })
    }
}

#[derive(Default)]
pub struct LocalCommandContext {
    pub directory: Option<String>,
    pub prompt: Option<String>,
}

fn read_command_block(
    _core: &Core,
    id: u64,
    absolute: i64,
    frame: &Frame,
) -> Result<Option<crate::terminal::blocks::Block>> {
    let offset = absolute - i64::from(frame.columns) * i64::from(frame.history_lines);
    let block: Option<crate::terminal::blocks::RawBlock> =
        mapped(sessions(|manager| manager.command_block_at(id, offset))?)?;
    block
        .map(|block| {
            let start = block.start;
            let mut block = block.into_block(absolute, frame, String::new())?;
            block.prompt = sessions(|manager| {
                manager.selection_text(id, start, start + i64::from(frame.columns))
            })?
            .unwrap_or_default();
            Ok(block)
        })
        .transpose()
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum HostTrust {
    #[default]
    Strict = 0,
    AcceptAndSave = 1,
    AcceptOnce = 2,
}

/// Transient authentication data. Deliberately not Serialize or Debug; secrets
/// never enter workspace.json, logs, or the host model.
impl HostTrust {
    pub(crate) fn native(self) -> nauterm_core::ssh::HostKeyTrustMode {
        match self {
            Self::Strict => nauterm_core::ssh::HostKeyTrustMode::Strict,
            Self::AcceptAndSave => nauterm_core::ssh::HostKeyTrustMode::AcceptAndSave,
            Self::AcceptOnce => nauterm_core::ssh::HostKeyTrustMode::AcceptOnce,
        }
    }
}

#[derive(Clone)]
pub struct SshOptions {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub known_hosts: PathBuf,
    pub trust: HostTrust,
    // Keep a one-workflow SFTP known-hosts directory alive across background
    // requests. Never serialize it or promote it into persistent user trust.
    pub temporary_trust: Option<Arc<tempfile::TempDir>>,
    pub proxy: Option<crate::proxies::Transport>,
    pub encoding: crate::host_session::Encoding,
    pub environment: Vec<crate::host_session::Variable>,
    pub startup: zeroize::Zeroizing<String>,
    pub password: zeroize::Zeroizing<String>,
    pub private_key: zeroize::Zeroizing<String>,
    pub certificate: zeroize::Zeroizing<String>,
    pub passphrase: zeroize::Zeroizing<String>,
    pub fido2_pin: zeroize::Zeroizing<String>,
}

impl SshOptions {
    pub fn for_new_connection(&self) -> Self {
        let mut options = self.clone();
        options.trust = HostTrust::Strict;
        options
    }
    pub fn new(host: String, port: u16, username: String, known_hosts: PathBuf) -> Self {
        Self {
            host,
            port,
            username,
            known_hosts,
            trust: HostTrust::Strict,
            temporary_trust: None,
            proxy: None,
            encoding: Default::default(),
            environment: vec![],
            startup: Default::default(),
            password: Default::default(),
            private_key: Default::default(),
            certificate: Default::default(),
            passphrase: Default::default(),
            fido2_pin: Default::default(),
        }
    }

    pub fn prepare_temporary_sftp_trust(&mut self) -> Result<()> {
        if self.trust == HostTrust::AcceptOnce && self.temporary_trust.is_none() {
            let directory = Arc::new(
                tempfile::Builder::new()
                    .prefix("nauterm-rs-sftp-trust-")
                    .tempdir()?,
            );
            self.known_hosts = directory.path().join("known_hosts");
            self.temporary_trust = Some(directory);
            // The native save is confined to this private temporary directory.
            // Subsequent requests switch to strict verification of this key.
            self.trust = HostTrust::AcceptAndSave;
        }
        Ok(())
    }
}

pub(crate) struct SshArguments {
    known_hosts: String,
    auth: zeroize::Zeroizing<String>,
    proxy: Option<nauterm_core::ssh::SshProxyConfig>,
}
impl SshArguments {
    pub(crate) fn auth(&self) -> &str {
        &self.auth
    }
    pub(crate) fn native_proxy(&self) -> Option<nauterm_core::ssh::SshProxyConfig> {
        self.proxy.clone()
    }
    pub(crate) fn new(options: &SshOptions) -> Result<Self> {
        ensure!(
            !options.host.trim().is_empty() && options.port != 0,
            "Host and port are required"
        );
        ensure!(!options.username.trim().is_empty(), "Username is required");
        let known_hosts = options
            .known_hosts
            .to_str()
            .context("Known hosts path is not UTF-8")?;
        ensure!(!known_hosts.is_empty(), "Known hosts path is required");
        for value in [
            &options.host,
            &options.username,
            known_hosts,
            options.password.as_str(),
            options.private_key.as_str(),
            options.certificate.as_str(),
        ] {
            ensure!(
                !value.contains('\0'),
                "SSH argument contains a NUL character"
            );
        }
        crate::host_session::validate_environment(&options.environment)?;
        let _ = crate::host_session::startup_input(&options.startup)?;
        Ok(Self {
            known_hosts: known_hosts.to_owned(),
            auth: crate::fido2::authentication(&options.passphrase, &options.fido2_pin)?,
            proxy: options
                .proxy
                .as_ref()
                .map(crate::proxies::Transport::native),
        })
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct SessionEvent {
    pub kind: String,
    pub message: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub fingerprint: Option<String>,
    pub method: Option<String>,
    pub exit_status: Option<u32>,
    pub latency_ms: Option<u32>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.history_generation.fetch_add(1, Ordering::AcqRel);
        self.close_command_output();
        let _ = sessions(|manager| {
            manager.set_wakeup_callback(self.id, None);
            manager.close(self.id);
        });
        self.receiver.close();
        self.recording.take();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub hyperlink: Option<Arc<str>>,
    pub foreground: u32,
    pub background: u32,
    pub flags: u16,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    pub columns: u32,
    pub rows: u32,
    pub cells: Vec<Cell>,
    pub graphics: crate::terminal_graphics::Snapshot,
    pub title: String,
    pub bell_count: u64,
    pub working_directory: Option<String>,
    pub alternate_screen: bool,
    pub cursor: (u32, u32),
    pub cursor_visible: bool,
    pub cursor_shape: u32,
    pub cursor_blinking: bool,
    pub cursor_color: u32,
    pub keyboard_mode: u32,
    pub input_echo_enabled: bool,
    pub history_lines: u32,
    pub display_offset: u32,
    pub exited: bool,
}

/// The layout comparison must use the same bounds as the resize request.
/// Otherwise an oversized viewport keeps refreshing an already-clamped grid.
pub(crate) fn terminal_grid_size(columns: u32, rows: u32) -> (u32, u32) {
    (columns.clamp(2, 1000), rows.clamp(1, 500))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_terminal_layout_and_resize_compare_the_same_grid() {
        for (requested, expected) in [
            ((0, 0), (2, 1)),
            ((240, 80), (240, 80)),
            ((1500, 900), (1000, 500)),
            ((u32::MAX, u32::MAX), (1000, 500)),
        ] {
            assert_eq!(terminal_grid_size(requested.0, requested.1), expected);
            assert_eq!(terminal_grid_size(expected.0, expected.1), expected);
        }
    }
    #[test]
    fn duplicated_connections_keep_auth_but_require_fresh_strict_host_verification() {
        let mut original = SshOptions::new(
            "127.0.0.1".into(),
            22,
            "fixture".into(),
            PathBuf::from("isolated-known-hosts"),
        );
        original.trust = HostTrust::AcceptOnce;
        original.password = zeroize::Zeroizing::new("test-only".into());
        original.certificate = zeroize::Zeroizing::new("test-certificate".into());
        let copy = original.for_new_connection();
        assert!(copy.trust == HostTrust::Strict);
        assert!(original.trust == HostTrust::AcceptOnce);
        assert_eq!(*copy.password, *original.password);
        assert_eq!(*copy.certificate, *original.certificate);
        assert_eq!(copy.known_hosts, original.known_hosts);
        assert_eq!(copy.username, original.username);
    }
}
