//! Event-driven directory watching, debounced stable saves, one worker per edit.
use super::*;
use notify::{EventKind, RecursiveMode, Watcher};
use std::sync::atomic::AtomicU64;
use std::{sync::mpsc, thread, time::Duration};

pub use crate::file_conflict::Resolution;
pub enum Event {
    Status(String),
    AuthorizationRequired {
        generation: u64,
        remote_path: String,
        local_path: PathBuf,
        error: String,
    },
    Conflict {
        generation: u64,
        remote_path: String,
        missing: bool,
    },
    Stopped {
        recovery: Option<PathBuf>,
    },
}
enum Command {
    Changed,
    Resolve(u64, Resolution),
    Reauthorize(u64, Arc<crate::sftp_client::Sudo>),
    Close,
}
struct Control {
    connection: Weak<Connection>,
    sender: mpsc::SyncSender<Command>,
    queued: AtomicBool,
    closing: AtomicBool,
    done: AtomicBool,
    authorization: AtomicU64,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}
impl Control {
    fn changed(&self) {
        if !self.queued.swap(true, Ordering::SeqCst)
            && self.sender.try_send(Command::Changed).is_err()
        {
            self.queued.store(false, Ordering::SeqCst);
        }
    }
    fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
        if let Some(connection) = self.connection.upgrade() {
            connection.transfers.begin_close();
        }
        let _ = self.sender.try_send(Command::Close);
    }
}
static CLOSING: AtomicBool = AtomicBool::new(false);
fn registry() -> &'static Mutex<Vec<Arc<Control>>> {
    static REGISTRY: OnceLock<Mutex<Vec<Arc<Control>>>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}
pub struct Handle {
    control: Arc<Control>,
    pub events: async_channel::Receiver<Event>,
}
impl Handle {
    pub fn start(mut document: Document) -> Result<Self> {
        ensure!(
            !CLOSING.load(Ordering::SeqCst),
            "Remote editing is shutting down."
        );
        document.mark_opened();
        document.connection.transfers.activate_edit();
        let (sender, receiver) = mpsc::sync_channel(8);
        let (events, incoming) = async_channel::unbounded();
        let control = Arc::new(Control {
            connection: Arc::downgrade(&document.connection),
            sender,
            queued: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            done: AtomicBool::new(false),
            authorization: AtomicU64::new(0),
            thread: Mutex::default(),
        });
        let listener = control.clone();
        let watched = document.local_path.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if event.as_ref().map_or(true, |event| {
                    !matches!(event.kind, EventKind::Access(_))
                        && event.paths.iter().any(|path| path == &watched)
                }) {
                    listener.changed();
                }
            })?;
        watcher.watch(document.directory(), RecursiveMode::NonRecursive)?;
        let worker = control.clone();
        let thread = thread::Builder::new()
            .name("nauterm-rs-remote-edit".into())
            .spawn(move || {
                let _watcher = watcher;
                run(&mut document, &worker, receiver, events);
                worker.done.store(true, Ordering::SeqCst);
            })?;
        *control.thread.lock().unwrap() = Some(thread);
        let mut workers = registry().lock().unwrap();
        workers.retain(|worker| !worker.done.load(Ordering::SeqCst));
        if CLOSING.load(Ordering::SeqCst) {
            control.close();
        }
        workers.push(control.clone());
        control.changed();
        Ok(Self {
            control,
            events: incoming,
        })
    }
    pub fn resolve(&self, generation: u64, resolution: Resolution) -> Result<()> {
        self.control
            .sender
            .try_send(Command::Resolve(generation, resolution))
            .context("This edit session has closed or is busy.")
    }
    pub fn close(&self) {
        self.control.close();
    }
    pub fn authorization_target(&self, generation: u64, path: &str) -> Result<AuthorizationTarget> {
        ensure!(
            generation != 0
                && !self.control.closing.load(Ordering::SeqCst)
                && self.control.authorization.load(Ordering::SeqCst) == generation,
            "This authorization request is no longer current."
        );
        self.control
            .connection
            .upgrade()
            .context("The edit session closed.")?
            .authorization_target(path)
    }
    pub fn reauthorize(&self, generation: u64, sudo: Arc<crate::sftp_client::Sudo>) -> Result<()> {
        ensure!(
            sudo.current()
                && generation != 0
                && !self.control.closing.load(Ordering::SeqCst)
                && self.control.authorization.load(Ordering::SeqCst) == generation,
            "This authorization request is no longer current."
        );
        self.control
            .sender
            .try_send(Command::Reauthorize(generation, sudo))
            .context("This edit session has closed or is busy.")
    }
    pub fn is_running(&self) -> bool {
        !self.control.done.load(Ordering::SeqCst)
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.close();
    }
}

fn run(
    document: &mut Document,
    control: &Control,
    receiver: mpsc::Receiver<Command>,
    events: async_channel::Sender<Event>,
) {
    let mut sampling = false;
    let mut previous = None;
    let mut conflict: Option<(u64, Arc<Snapshot>, Option<Version>)> = None;
    let mut failed = false;
    let mut next = 0;
    let interval = Duration::from_millis(350);
    loop {
        if control.closing.load(Ordering::SeqCst) {
            break;
        }
        let command = if sampling && conflict.is_none() {
            receiver.recv_timeout(interval)
        } else {
            receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        let save = match command {
            Ok(Command::Changed) => {
                control.queued.store(false, Ordering::SeqCst);
                if control.authorization.load(Ordering::SeqCst) != 0 {
                    // Keep accepting local edits, but don't repeatedly prompt
                    // or retry a remote save while authorization is pending.
                    continue;
                }
                sampling = true;
                previous = None;
                continue;
            }
            Ok(Command::Close) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::Reauthorize(generation, sudo)) => {
                if generation == 0 || control.authorization.load(Ordering::SeqCst) != generation {
                    continue;
                }
                match document.connection.renew_sudo(sudo) {
                    Ok(()) => {
                        control.authorization.store(0, Ordering::SeqCst);
                        // Capture the latest stable local edit, then restart
                        // version checks. An old overwrite/conflict approval
                        // never carries across the authentication pause.
                        sampling = true;
                        previous = None;
                        let _ = events.try_send(Event::Status("Authenticated; checking the latest local and remote versions before saving…".into()));
                    }
                    Err(error) => {
                        let _ = events.try_send(Event::AuthorizationRequired {
                            generation,
                            remote_path: document.remote_path.clone(),
                            local_path: document.local_path.clone(),
                            error: error.to_string(),
                        });
                    }
                }
                continue;
            }
            Ok(Command::Resolve(generation, resolution)) => {
                let Some((current, snapshot, approved)) = conflict.take() else {
                    continue;
                };
                if current != generation {
                    conflict = Some((current, snapshot, approved));
                    continue;
                }
                if resolution == Resolution::Cancel {
                    sampling = false;
                    failed = true;
                    let _ = events.try_send(Event::Status("Upload paused; the remote file was not changed. The local edit is retained.".into()));
                    continue;
                }
                Some((snapshot, Some((resolution, approved))))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let current = match document.fingerprint() {
                    Ok(current) => current,
                    Err(error) => {
                        sampling = false;
                        failed = true;
                        let _ = events.try_send(Event::Status(format!(
                            "Unable to read the edited file: {error}. Local copy: {}",
                            document.local_path.display()
                        )));
                        continue;
                    }
                };
                if current == document.uploaded {
                    sampling = false;
                    continue;
                }
                if previous.as_ref() != Some(&current) {
                    previous = Some(current);
                    continue;
                }
                match document.snapshot() {
                    Ok(snapshot) => Some((snapshot, None)),
                    Err(error) => {
                        sampling = false;
                        failed = true;
                        let _ = events.try_send(Event::Status(format!(
                            "Unable to snapshot the edited file: {error}"
                        )));
                        continue;
                    }
                }
            }
        };
        if let Some((snapshot, resolution)) = save {
            sampling = false;
            let _ = events.try_send(Event::Status(format!("Saving {}…", document.remote_path)));
            let lock = lock_for(document.connection.key(&document.remote_path));
            let _lock = lock.lock().unwrap();
            if control.closing.load(Ordering::SeqCst) {
                break;
            }
            match document.save(&snapshot, resolution) {
                Ok(Save::Saved) => {
                    failed = false;
                    sampling = document.changed();
                    previous = None;
                    let _ =
                        events.try_send(Event::Status(format!("Saved {}", document.remote_path)));
                }
                Ok(Save::Conflict(version)) => {
                    next += 1;
                    let missing = version.is_none();
                    conflict = Some((next, snapshot, version));
                    let _ = events.try_send(Event::Conflict {
                        generation: next,
                        remote_path: document.remote_path.clone(),
                        missing,
                    });
                }
                Ok(Save::Paused) => {
                    failed = true;
                }
                Err(error) => {
                    failed = true;
                    if error.is::<crate::sftp_client::SudoExpired>() {
                        next += 1;
                        control.authorization.store(next, Ordering::SeqCst);
                        let _ = events.try_send(Event::AuthorizationRequired {
                            generation: next,
                            remote_path: document.remote_path.clone(),
                            local_path: document.local_path.clone(),
                            error: error.to_string(),
                        });
                    } else {
                        let _ = events.try_send(Event::Status(format!(
                            "Unable to save: {error}. Local copy: {}",
                            document.local_path.display()
                        )));
                    }
                }
            }
        }
    }
    control.authorization.store(0, Ordering::SeqCst);
    // One final stable save, never an implicit conflict resolution or automatic
    // retry of a failed/uncertain write. Unsent content remains recoverable.
    if conflict.is_none() && !failed && document.changed() {
        let first = document.fingerprint();
        thread::sleep(interval);
        if let (Ok(first), Ok(second)) = (first, document.fingerprint())
            && first == second
            && let Ok(snapshot) = document.snapshot()
        {
            let lock = lock_for(document.connection.key(&document.remote_path));
            let _lock = lock.lock().unwrap();
            let _ = document.save(&snapshot, None);
        }
    }
    let recovery = document.finish();
    if let Some(path) = &recovery {
        eprintln!("Unsaved remote edits retained at {}", path.display());
    }
    let _ = events.try_send(Event::Stopped { recovery });
}
pub fn prepare_shutdown() {
    CLOSING.store(true, Ordering::SeqCst);
    close_window();
}
pub fn close_window() {
    let workers = std::mem::take(&mut *registry().lock().unwrap());
    for worker in &workers {
        worker.close();
    }
    for worker in workers {
        if let Some(thread) = worker.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
}
