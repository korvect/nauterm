//! Typed port-forward requests and an owned background controller for the
//! reference native implementation. No network or join runs on the UI thread.
use crate::native::{Core, HostTrust, SshArguments, SshOptions};
use anyhow::{Context, Result, bail, ensure};
use async_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Local,
    Remote,
    Dynamic,
}
impl Kind {
    pub const ALL: [Self; 3] = [Self::Local, Self::Remote, Self::Dynamic];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
            Self::Dynamic => "dynamic",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Remote => "Remote",
            Self::Dynamic => "Dynamic",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub id: u64,
    pub kind: Kind,
    pub bind_address: String,
    pub bind_port: u16,
    pub destination: Option<(String, u16)>,
}
impl Spec {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.id > 0, "Forwarding rule is not saved.");
        ensure!(self.bind_port > 0, "Port must be between 1 and 65535.");
        validate_address(&self.bind_address)?;
        match (self.kind, &self.destination) {
            (Kind::Dynamic, None) => {}
            (Kind::Local | Kind::Remote, Some((host, port))) => {
                validate_address(host)?;
                ensure!(*port > 0, "Destination port must be between 1 and 65535.");
            }
            _ => bail!("The forwarding type and destination do not match."),
        }
        Ok(())
    }
}
fn validate_address(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && !value.chars().any(|c| c.is_whitespace() || c.is_control()),
        "An address without whitespace or control characters is required."
    );
    Ok(())
}
pub fn endpoint(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}
pub fn parse_destination(value: &str) -> Result<(String, u16)> {
    let (host, port) = if let Some(value) = value.strip_prefix('[') {
        let (host, port) = value.split_once("]:").context("Use [IPv6]:port.")?;
        ensure!(
            host.parse::<std::net::Ipv6Addr>().is_ok(),
            "Invalid IPv6 destination."
        );
        (host, port)
    } else {
        let (host, port) = value.rsplit_once(':').context("Use destination:port.")?;
        ensure!(!host.contains(':'), "Use [IPv6]:port.");
        (host, port)
    };
    validate_address(host)?;
    let port = port
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .context("Destination port must be between 1 and 65535.")?;
    Ok((host.to_owned(), port))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Starting,
    Running,
    Stopping,
    #[default]
    Stopped,
    Error,
}
impl State {
    pub fn active(self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Stopping)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "Starting…",
            Self::Running => "Enabled",
            Self::Stopping => "Stopping…",
            Self::Stopped => "Stopped",
            Self::Error => "Error",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Status {
    pub id: u64,
    pub state: State,
    pub error: Option<String>,
    pub bound_port: Option<u16>,
    pub active_connections: u32,
}
impl Status {
    pub fn new(id: u64, state: State) -> Self {
        Self {
            id,
            state,
            error: None,
            bound_port: None,
            active_connections: 0,
        }
    }
    pub fn failed(id: u64, error: impl ToString) -> Self {
        Self {
            error: Some(error.to_string()),
            ..Self::new(id, State::Error)
        }
    }
    /// A failed stop leaves a potentially live rule owned and retryable.
    pub fn stop_failed(&mut self, error: impl ToString) {
        if !matches!(self.state, State::Starting | State::Running) {
            self.state = State::Running;
        }
        self.error = Some(error.to_string());
    }
}

#[derive(Default)]
pub(crate) struct Api;
impl Api {
    fn with_manager<T>(
        &self,
        work: impl FnOnce(&mut nauterm_core::port_forward::PortForwardManager) -> T,
    ) -> Result<T> {
        let mut manager = nauterm_core::port_forward::port_forward_manager()
            .lock()
            .map_err(|_| anyhow::anyhow!("Forwarding manager unavailable"))?;
        Ok(work(&mut manager))
    }
    fn status(&self, id: u64) -> Result<Status> {
        let status = self.with_manager(|manager| manager.status(id))?;
        Ok(serde_json::from_value(serde_json::to_value(status)?)?)
    }
    fn stop(&self, id: u64) -> Result<bool> {
        self.with_manager(|manager| manager.stop(id))
    }
}

trait Backend: Send + Sync + 'static {
    fn start(&self, spec: &Spec, options: &SshOptions) -> Result<Status>;
    fn stop(&self, id: u64) -> Result<()>;
    fn status(&self, id: u64) -> Result<Status>;
}
struct NativeBackend {
    core: Arc<Core>,
    ids: Mutex<HashMap<u64, u64>>,
}
impl NativeBackend {
    fn native_id(&self, id: u64) -> Result<u64> {
        self.ids
            .lock()
            .unwrap()
            .get(&id)
            .copied()
            .context("Forwarding runtime is no longer available.")
    }
}
impl Drop for NativeBackend {
    fn drop(&mut self) {
        if let Ok(api) = self.core.port_forward_api() {
            for native in self
                .ids
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .values()
            {
                let _ = api.stop(*native);
            }
        }
    }
}
impl Backend for NativeBackend {
    fn start(&self, spec: &Spec, options: &SshOptions) -> Result<Status> {
        spec.validate()?;
        ensure!(
            options.trust == HostTrust::Strict,
            "Port forwarding requires strict host-key verification. Connect from Hosts to review an unknown key first."
        );
        let args = SshArguments::new(options)?;
        let api = self.core.port_forward_api()?;
        static NEXT_NATIVE: AtomicU64 = AtomicU64::new(1 << 63);
        let native = NEXT_NATIVE
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| anyhow::anyhow!("Forwarding runtime IDs are exhausted."))?;
        {
            let mut owned = self.ids.lock().unwrap();
            ensure!(
                !owned.contains_key(&spec.id),
                "Forwarding rule is already running."
            );
            owned.insert(spec.id, native);
        }
        let config = nauterm_core::port_forward::PortForwardConfig {
            id: native,
            forward_type: spec.kind.as_str().to_owned(),
            ssh_host: options.host.clone(),
            ssh_port: options.port,
            username: options.username.clone(),
            password: Some(options.password.to_string()),
            private_key: Some(options.private_key.to_string()),
            certificate: Some(options.certificate.to_string()),
            passphrase: Some(args.auth().to_owned()),
            known_hosts_path: Some(
                options
                    .known_hosts
                    .to_str()
                    .context("Known hosts path is not UTF-8")?
                    .to_owned(),
            ),
            proxy: args.native_proxy(),
            bind_address: spec.bind_address.clone(),
            bind_port: spec.bind_port,
            destination_host: spec.destination.as_ref().map(|(host, _)| host.clone()),
            destination_port: spec.destination.as_ref().map_or(0, |(_, port)| *port),
        };
        let native_status = api.with_manager(|manager| manager.start(config))?;
        let mut status: Status = serde_json::from_value(serde_json::to_value(native_status)?)?;
        status.id = spec.id;
        Ok(status)
    }
    fn stop(&self, id: u64) -> Result<()> {
        let native = self.ids.lock().unwrap().get(&id).copied();
        if let Some(native) = native {
            let api = self.core.port_forward_api()?;
            if !api.stop(native)? {
                ensure!(
                    api.status(native)?.state == State::Stopped,
                    "The forwarding rule could not be stopped."
                );
            }
            self.ids.lock().unwrap().remove(&id);
        }
        Ok(())
    }
    fn status(&self, id: u64) -> Result<Status> {
        let mut status = self.core.port_forward_api()?.status(self.native_id(id)?)?;
        status.id = id;
        Ok(status)
    }
}

struct StartRequest {
    generation: u64,
    spec: Spec,
    options: SshOptions,
}
pub type StopResult = std::result::Result<(), String>;
enum Command {
    Start(Box<StartRequest>),
    Stop {
        id: u64,
        generation: u64,
        acknowledged: Option<Sender<StopResult>>,
    },
    Shutdown,
}
#[derive(Clone, Debug)]
pub struct Event {
    pub generation: u64,
    pub status: Status,
}
pub struct Runtime {
    pub generation: u64,
    pub status: Status,
}
impl Runtime {
    pub fn apply(&mut self, event: Event) -> bool {
        if self.generation != event.generation || self.status.id != event.status.id {
            return false;
        }
        self.status = event.status;
        true
    }
}
pub fn recovery_ids(runtimes: &HashMap<u64, Runtime>) -> Vec<u64> {
    let mut ids: Vec<_> = runtimes
        .iter()
        .filter(|(_, run)| matches!(run.status.state, State::Starting | State::Running))
        .map(|(id, _)| *id)
        .collect();
    ids.sort_unstable();
    ids
}
struct Worker {
    queue: mpsc::Sender<Command>,
    closing: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
impl Worker {
    fn cancel(&self) {
        self.closing.store(true, Ordering::Release);
        let _ = self.queue.send(Command::Shutdown);
    }
    fn join(&self) {
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
}
fn workers() -> &'static Mutex<Vec<Arc<Worker>>> {
    static WORKERS: OnceLock<Mutex<Vec<Arc<Worker>>>> = OnceLock::new();
    WORKERS.get_or_init(Mutex::default)
}
pub struct Service {
    worker: Arc<Worker>,
    pub events: Receiver<Event>,
}
impl Service {
    pub fn new(core: Arc<Core>) -> Result<Self> {
        core.port_forward_api()?;
        Self::with_backend(Arc::new(NativeBackend {
            core,
            ids: Mutex::new(HashMap::new()),
        }))
    }
    fn with_backend(backend: Arc<dyn Backend>) -> Result<Self> {
        let (queue, commands) = mpsc::channel();
        let (events, receiver) = async_channel::unbounded();
        let closing = Arc::new(AtomicBool::new(false));
        let cancelled = closing.clone();
        let thread = thread::Builder::new()
            .name("nauterm-rs-forwards".into())
            .spawn(move || run(backend, commands, events, cancelled))?;
        let worker = Arc::new(Worker {
            queue,
            closing,
            thread: Mutex::new(Some(thread)),
        });
        let mut registered = workers().lock().unwrap();
        registered.retain(|worker| {
            let finished = worker
                .thread
                .lock()
                .unwrap()
                .as_ref()
                .is_none_or(JoinHandle::is_finished);
            if finished {
                worker.join();
            }
            !finished
        });
        registered.push(worker.clone());
        Ok(Self {
            worker,
            events: receiver,
        })
    }
    pub fn start(&self, generation: u64, spec: Spec, options: SshOptions) -> Result<()> {
        spec.validate()?;
        ensure!(
            !self.worker.closing.load(Ordering::Acquire),
            "Forwarding service is closing."
        );
        self.worker
            .queue
            .send(Command::Start(Box::new(StartRequest {
                generation,
                spec,
                options,
            })))
            .context("Forwarding service is unavailable.")
    }
    pub fn stop(&self, id: u64, generation: u64) -> Result<()> {
        self.worker
            .queue
            .send(Command::Stop {
                id,
                generation,
                acknowledged: None,
            })
            .context("Forwarding service is unavailable.")
    }
    /// Receipt after this worker has processed older starts and stopped its
    /// owned native rule. The UI can await it before publishing an edit/delete.
    pub fn stop_with_ack(&self, id: u64, generation: u64) -> Result<Receiver<StopResult>> {
        ensure!(
            !self.worker.closing.load(Ordering::Acquire),
            "Forwarding service is closing."
        );
        let (acknowledged, receipt) = async_channel::bounded(1);
        self.worker
            .queue
            .send(Command::Stop {
                id,
                generation,
                acknowledged: Some(acknowledged),
            })
            .context("Forwarding service is unavailable.")?;
        Ok(receipt)
    }
    pub fn prepare_shutdown() {
        let workers = std::mem::take(&mut *workers().lock().unwrap());
        for worker in &workers {
            worker.cancel();
        }
        for worker in workers {
            worker.join();
        }
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.worker.cancel();
    }
}

fn run(
    backend: Arc<dyn Backend>,
    commands: mpsc::Receiver<Command>,
    events: Sender<Event>,
    closing: Arc<AtomicBool>,
) {
    let mut active: HashMap<u64, (u64, Status)> = HashMap::new();
    while !closing.load(Ordering::Acquire) {
        // The reference exposes status polling, not forwarding wakeup events.
        // Sleep indefinitely when idle; poll only owned live rules off-thread.
        let command = if active.is_empty() {
            commands
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        } else {
            commands.recv_timeout(Duration::from_millis(500))
        };
        if closing.load(Ordering::Acquire) {
            break;
        }
        match command {
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::Start(request)) => {
                let StartRequest {
                    generation,
                    spec,
                    options,
                } = *request;
                let id = spec.id;
                if active.contains_key(&id) {
                    // Never let the native start replace somebody's live rule.
                    continue;
                }
                let mut status = backend
                    .start(&spec, &options)
                    .unwrap_or_else(|error| Status::failed(id, error));
                if status.state.active() {
                    active.insert(id, (generation, status.clone()));
                } else if let Err(error) = backend.stop(id) {
                    status.state = State::Stopping;
                    status.error = Some(format!("Forwarding cleanup failed: {error}"));
                    active.insert(id, (generation, status.clone()));
                }
                let _ = events.try_send(Event { generation, status });
            }
            Ok(Command::Stop {
                id,
                generation,
                acknowledged,
            }) => {
                let result = if active.contains_key(&id) {
                    backend.stop(id)
                } else {
                    Ok(())
                };
                let status = match &result {
                    Ok(()) => {
                        active.remove(&id);
                        Status::new(id, State::Stopped)
                    }
                    Err(error) => {
                        // A failed stop must not make a live rule look inactive:
                        // keep Stop/recovery ownership available for a retry.
                        let mut status = active
                            .get(&id)
                            .map(|(_, status)| status.clone())
                            .unwrap_or_else(|| Status::new(id, State::Running));
                        if !status.state.active() {
                            status.state = State::Running;
                        }
                        status.error = Some(error.to_string());
                        active.insert(id, (generation, status.clone()));
                        status
                    }
                };
                let _ = events.try_send(Event { generation, status });
                if let Some(acknowledged) = acknowledged {
                    let _ = acknowledged.try_send(result.map_err(|error| error.to_string()));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let ids: Vec<_> = active.keys().copied().collect();
                for id in ids {
                    let mut status = backend
                        .status(id)
                        .unwrap_or_else(|error| Status::failed(id, error));
                    let finished = if !status.state.active() {
                        match backend.stop(id) {
                            Ok(()) => true,
                            Err(error) => {
                                status.state = State::Stopping;
                                status.error = Some(format!("Forwarding cleanup failed: {error}"));
                                false
                            }
                        }
                    } else {
                        false
                    };
                    let (generation, previous) = active.get_mut(&id).unwrap();
                    if *previous != status {
                        *previous = status.clone();
                        let _ = events.try_send(Event {
                            generation: *generation,
                            status: status.clone(),
                        });
                    }
                    if finished {
                        active.remove(&id);
                    }
                }
            }
        }
    }
    for id in active.keys() {
        let _ = backend.stop(*id);
    }
    // Dropping queued requests clears their zeroizing credentials too.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_stop_retains_binding_and_retry_ownership() {
        let mut status = Status::new(7, State::Stopping);
        status.bound_port = Some(42001);
        status.active_connections = 3;
        status.stop_failed("Synthetic stop failure");
        assert!(status.state.active());
        assert_eq!(status.bound_port, Some(42001));
        assert_eq!(status.active_connections, 3);
        assert_eq!(status.error.as_deref(), Some("Synthetic stop failure"));
        let runs = HashMap::from([(
            7,
            Runtime {
                generation: 2,
                status,
            },
        )]);
        assert_eq!(recovery_ids(&runs), vec![7]);
    }

    #[test]
    fn old_failure_callbacks_cannot_replace_a_new_attempt() {
        let mut run = Runtime {
            generation: 3,
            status: Status::new(7, State::Running),
        };
        assert!(!run.apply(Event {
            generation: 2,
            status: Status::failed(7, "Old failure")
        }));
        assert!(!run.apply(Event {
            generation: 3,
            status: Status::failed(8, "Other rule")
        }));
        assert_eq!(run.status.state, State::Running);
        assert!(run.status.error.is_none());
        assert!(run.apply(Event {
            generation: 3,
            status: Status::failed(7, "Current failure")
        }));
        assert_eq!(run.status.error.as_deref(), Some("Current failure"));
    }
}
