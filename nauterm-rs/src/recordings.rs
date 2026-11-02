//! RS-owned session history and opt-in encrypted output. All database, crypto
//! and filesystem work is serialized off the render thread; no input is logged.
pub(crate) mod capture;
mod orphans;
mod presentation;
mod retention;
mod sanitizer;
mod view;
use crate::{
    native::{Core, SessionEvent},
    vault::Vault,
};
use anyhow::{Context, Result, ensure};
use async_channel::Sender;
use nauterm_core::recording_lease::RecordingLease;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
pub use view::{Logs, ReplayRequested};
use zeroize::Zeroizing;

const QUEUED_BYTES: usize = 8 * 1024 * 1024;
const MAX_EVENTS: usize = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub enabled: bool,
    pub capture: bool,
    pub retention_days: u64,
    pub max_session_bytes: u64,
    pub max_total_bytes: u64,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: true,
            capture: false,
            retention_days: 30,
            max_session_bytes: 100 * 1024 * 1024,
            max_total_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}
impl Preferences {
    pub const DAYS: [u64; 5] = [7, 30, 90, 180, 365];
    pub const SESSION_MIB: [u64; 5] = [25, 50, 100, 250, 500];
    pub const TOTAL_GIB: [u64; 5] = [1, 2, 5, 10, 20];
    pub fn validate(self) -> Result<()> {
        ensure!(
            Self::DAYS.contains(&self.retention_days),
            "Invalid recording retention period."
        );
        ensure!(
            Self::SESSION_MIB
                .iter()
                .any(|value| value * 1024 * 1024 == self.max_session_bytes),
            "Invalid per-session recording limit."
        );
        ensure!(
            Self::TOTAL_GIB
                .iter()
                .any(|value| value * 1024 * 1024 * 1024 == self.max_total_bytes),
            "Invalid total recording storage limit."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    #[serde(default)]
    pub local_id: Option<i64>,
    #[serde(default)]
    pub host_id: Option<i64>,
    #[serde(default)]
    pub host_uuid: Option<String>,
    pub title: String,
    pub theme_id: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub shell_path: Option<String>,
    pub work_dir: Option<String>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub capture_file: String,
    #[serde(default)]
    pub capture_bytes: u64,
    pub capture_sha256: Option<String>,
    pub columns: Option<u32>,
    pub rows: Option<u32>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}
impl Entry {
    pub fn matches(&self, query: &str) -> bool {
        let text = format!(
            "{} {} {} {}",
            self.title,
            self.host.as_deref().unwrap_or("local"),
            self.username.as_deref().unwrap_or_default(),
            self.cwd.as_deref().unwrap_or_default()
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|word| text.contains(&word.to_lowercase()))
    }
    pub fn target(&self) -> String {
        self.host
            .as_ref()
            .map(|host| {
                let user = self
                    .username
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(|value| format!("{value}@"))
                    .unwrap_or_default();
                let host = if host.contains(':') && !host.starts_with('[') {
                    format!("[{host}]")
                } else {
                    host.clone()
                };
                let port = self.port.map(|port| format!(":{port}")).unwrap_or_default();
                format!("{user}{host}{port}")
            })
            .unwrap_or_else(|| "Local terminal".into())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub message: String,
    pub connection_kind: Option<String>,
}
impl Event {
    fn new(kind: &str, message: &str) -> Self {
        Self {
            timestamp: now(),
            kind: kind.into(),
            message: message.chars().take(4096).collect(),
            connection_kind: None,
        }
    }
}
#[derive(Clone)]
pub struct Catalog {
    pub entries: Vec<Entry>,
    pub active: BTreeSet<String>,
    pub warnings: Vec<String>,
    pub storage_bytes: Option<u64>,
    pub next_offset: usize,
    pub has_more: bool,
}
impl Catalog {
    fn append_page(&mut self, page: Self) {
        let mut ids: BTreeSet<_> = self.entries.iter().map(|entry| entry.id.clone()).collect();
        self.entries.extend(
            page.entries
                .into_iter()
                .filter(|entry| ids.insert(entry.id.clone())),
        );
        self.active.extend(page.active);
        self.warnings = page.warnings;
        self.next_offset = page.next_offset;
        self.has_more = page.has_more;
    }
}
fn read_catalog_page(
    offset: usize,
    limit: usize,
    mut fetch: impl FnMut(usize, usize) -> Result<Vec<Entry>>,
) -> Result<(Vec<Entry>, usize, bool)> {
    let mut entries = Vec::new();
    let limit = limit.max(1);
    while entries.len() < limit {
        let batch = (limit - entries.len()).min(500);
        let page = fetch(offset + entries.len(), batch)?;
        let done = page.len() < batch;
        entries.extend(page);
        if done {
            break;
        }
    }
    let next_offset = offset + entries.len();
    let has_more = entries.len() == limit;
    Ok((entries, next_offset, has_more))
}
/// Prepared recording context, without a persisted log or buffered output.
/// SSH holds this until its actual connection-established event.
#[derive(Clone)]
pub(crate) struct Start {
    pub service: Service,
    pub preferences: Preferences,
    pub title: String,
    pub theme: crate::terminal_appearance::ThemeId,
    pub started_at: i64,
    pub generation: usize,
    pub host_id: Option<i64>,
    pub host_uuid: Option<String>,
}
impl Start {
    pub fn new(
        service: Service,
        preferences: Preferences,
        title: &str,
        theme: crate::terminal_appearance::ThemeId,
    ) -> Self {
        let generation = service.generation();
        Self {
            service,
            preferences,
            title: title.into(),
            theme,
            started_at: now(),
            generation,
            host_id: None,
            host_uuid: None,
        }
    }
}
impl Event {
    pub(crate) fn connection(event: &SessionEvent) -> Option<Self> {
        matches!(
            event.kind.as_str(),
            "connected" | "error" | "session_closed" | "exit_status"
        )
        .then(|| Self {
            connection_kind: Some(event.kind.clone()),
            ..Self::new("connection", &event.message)
        })
    }
}
pub fn local_hostname() -> String {
    #[cfg(unix)]
    {
        let mut bytes = [0u8; 256];
        // SAFETY: the writable buffer has the passed size; a missing terminator
        // is bounded by this allocation, never read as an unbounded C string.
        if unsafe { libc::gethostname(bytes.as_mut_ptr().cast(), bytes.len()) } == 0 {
            let end = bytes
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(bytes.len());
            if end > 0 {
                return String::from_utf8_lossy(&bytes[..end]).into_owned();
            }
        }
    }
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Local machine".into())
}
fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
fn identifier() -> String {
    let bits = (rand::random::<u128>() & !(0xf000_u128 << 64 | 0xc000_u128 << 48))
        | (0x4000_u128 << 64 | 0x8000_u128 << 48);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        bits >> 96,
        (bits >> 80) & 0xffff,
        (bits >> 64) & 0xffff,
        (bits >> 48) & 0xffff,
        bits & 0xffffffffffff
    )
}

type Job = Box<dyn FnOnce(&mut Runtime) + Send>;
#[derive(Clone)]
pub struct Service {
    worker: Arc<Worker>,
    history: crate::shell_history::store::Service,
}
struct Worker {
    queue: Sender<Job>,
    updates: async_channel::Receiver<()>,
    thread: Mutex<Option<JoinHandle<()>>>,
    bytes: Arc<AtomicUsize>,
    recording_epoch: Arc<AtomicUsize>,
    history_enabled: AtomicBool,
}
impl Worker {
    fn shutdown(&self) {
        self.queue.close();
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
fn workers() -> &'static Mutex<Vec<Weak<Worker>>> {
    static WORKERS: OnceLock<Mutex<Vec<Weak<Worker>>>> = OnceLock::new();
    WORKERS.get_or_init(Mutex::default)
}
impl Service {
    pub async fn replay(
        &self,
        entry: Entry,
        palette: crate::terminal_appearance::Palette,
        cancel: Arc<AtomicBool>,
    ) -> Result<crate::native::Session> {
        // Only catalogue validation runs on the recorder queue. Large decrypt/VT
        // parsing jobs must not delay live recording checkpoints and output.
        let (core, path, current) = self
            .dispatch(move |runtime| {
                ensure!(
                    !runtime.active.contains_key(&entry.id),
                    "Close the session before replaying its recording."
                );
                let current = runtime
                    .entry(&entry.id)?
                    .context("This log no longer exists.")?;
                ensure!(
                    current.capture_sha256.is_some()
                        && current.capture_file == entry.capture_file
                        && current.capture_sha256 == entry.capture_sha256,
                    "The recording changed or has no finalized output. Refresh and try again."
                );
                Ok((
                    runtime.core.clone(),
                    runtime.capture_path(&current)?,
                    current,
                ))
            })
            .await?;
        smol::unblock(move || {
            use sha2::{Digest, Sha256};
            use std::io::Read;
            let cancelled = || -> Result<()> {
                ensure!(
                    !cancel.load(Ordering::Acquire),
                    "Recording replay was cancelled."
                );
                Ok(())
            };
            cancelled()?;
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                metadata.is_file() && metadata.len() == current.capture_bytes,
                "The encrypted capture is incomplete or has changed."
            );
            let mut file = fs::File::open(&path)?;
            let mut hash = Sha256::new();
            let mut buffer = [0u8; 64 * 1024];
            loop {
                cancelled()?;
                let size = file.read(&mut buffer)?;
                if size == 0 {
                    break;
                }
                hash.update(&buffer[..size]);
            }
            let digest = hash
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            ensure!(
                current
                    .capture_sha256
                    .as_deref()
                    .is_some_and(|expected| expected.eq_ignore_ascii_case(&digest)),
                "The encrypted capture has been modified."
            );
            let session = core.replay(
                current.columns.unwrap_or(100),
                current.rows.unwrap_or(30),
                palette,
            )?;
            let mut filter = sanitizer::Sanitizer::default();
            core.capture.read(&path, &current.id, |bytes| {
                cancelled()?;
                let output = filter.add(bytes);
                for part in output.chunks(64 * 1024) {
                    cancelled()?;
                    session.write_replay(part)?;
                }
                Ok(())
            })?;
            cancelled()?;
            let tail = filter.finish();
            session.write_replay(&tail)?;
            session.bottom();
            Ok(session)
        })
        .await
    }
    pub fn new(
        core: Arc<Core>,
        vault: Vault,
        directory: PathBuf,
        history: crate::shell_history::store::Service,
    ) -> Result<Self> {
        let (queue, receiver) = async_channel::unbounded::<Job>();
        let (notify, updates) = async_channel::bounded(1);
        let runtime_history = history.clone();
        let thread = std::thread::Builder::new()
            .name("nauterm-rs-recordings".into())
            .spawn(move || {
                let mut runtime = Runtime {
                    core,
                    vault,
                    directory,
                    history: runtime_history,
                    active: BTreeMap::new(),
                    ignored: BTreeSet::new(),
                    preferences: None,
                    warnings: vec![],
                    notify,
                };
                if let Err(error) = runtime.recover() {
                    runtime.warn(format!("Recording recovery: {error}"));
                }
                if let Err(error) = runtime.cleanup_orphans(true) {
                    runtime.warn(format!("Recording cleanup: {error}"));
                }
                while let Ok(job) = receiver.recv_blocking() {
                    job(&mut runtime);
                }
                for id in runtime.active.keys().cloned().collect::<Vec<_>>() {
                    runtime.finish(&id);
                }
            })?;
        let worker = Arc::new(Worker {
            queue,
            updates,
            thread: Mutex::new(Some(thread)),
            bytes: Arc::new(AtomicUsize::new(0)),
            recording_epoch: Arc::new(AtomicUsize::new(0)),
            history_enabled: AtomicBool::new(true),
        });
        let mut workers = workers().lock().unwrap();
        workers.retain(|worker| worker.strong_count() > 0);
        workers.push(Arc::downgrade(&worker));
        Ok(Self { worker, history })
    }
    pub fn notifications(&self) -> async_channel::Receiver<()> {
        self.worker.updates.clone()
    }
    pub fn configure(&self, preferences: Preferences) -> Result<()> {
        preferences.validate()?;
        if self
            .worker
            .history_enabled
            .swap(preferences.enabled, Ordering::AcqRel)
            && !preferences.enabled
        {
            self.worker.recording_epoch.fetch_add(1, Ordering::AcqRel);
        }
        self.worker
            .queue
            .try_send(Box::new(move |runtime| {
                if runtime.preferences == Some(preferences) {
                    return;
                }
                runtime.preferences = Some(preferences);
                if !preferences.enabled {
                    for id in runtime.active.keys().cloned().collect::<Vec<_>>() {
                        runtime.ignored.insert(id.clone());
                        runtime.finalize_log(&id, "Session history disabled in settings.");
                    }
                } else if !preferences.capture {
                    for id in runtime
                        .active
                        .iter()
                        .filter(|(_, active)| active.writer.is_some())
                        .map(|(id, _)| id.clone())
                        .collect::<Vec<_>>()
                    {
                        runtime.end_capture(&id, "Output recording disabled in settings.", false);
                    }
                }
                if let Err(error) = runtime.retain() {
                    runtime.warn(format!("Recording retention: {error}"));
                }
            }))
            .map_err(|_| anyhow::anyhow!("Recording service has stopped"))
    }
    pub fn prepare_shutdown() {
        let workers: Vec<_> = workers()
            .lock()
            .unwrap()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for worker in &workers {
            worker.queue.close();
        }
        for worker in workers {
            worker.shutdown();
        }
    }
    async fn dispatch<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Runtime) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (sender, receiver) = async_channel::bounded(1);
        self.worker
            .queue
            .send(Box::new(move |runtime| {
                let _ = sender.try_send(job(runtime));
            }))
            .await
            .map_err(|_| anyhow::anyhow!("Recording service has stopped"))?;
        receiver
            .recv()
            .await
            .context("Recording operation interrupted")?
    }
    pub(crate) fn generation(&self) -> usize {
        self.worker.recording_epoch.load(Ordering::Acquire)
    }

    pub(crate) fn begin_at(
        &self,
        mut entry: Entry,
        preferences: Preferences,
        generation: usize,
    ) -> Option<Recorder> {
        if !preferences.enabled {
            return None;
        }
        entry.id = identifier();
        if entry.started_at <= 0 {
            entry.started_at = now();
        }
        let id = entry.id.clone();
        let error = Arc::new(Mutex::new(None));
        let issue = error.clone();
        let epoch = self.worker.recording_epoch.clone();
        self.worker
            .queue
            .try_send(Box::new(move |runtime| {
                // A pending SSH start can outlive a settings change. Its old
                // snapshot must never overwrite the current global preference.
                let current = *runtime.preferences.get_or_insert(preferences);
                if !current.enabled || generation != epoch.load(Ordering::Acquire) {
                    runtime.ignored.insert(entry.id.clone());
                    return;
                }
                if let Err(error) = runtime.begin(entry, preferences.capture && current.capture) {
                    *issue.lock().unwrap() = Some(error.to_string());
                    runtime.warn(format!("Could not start session history: {error}"));
                }
            }))
            .ok()?;
        Some(Recorder {
            id,
            service: self.clone(),
            capture: preferences.capture,
            failed: AtomicBool::new(false),
            error,
            metadata: Mutex::new(None),
        })
    }
    pub async fn catalog(&self, offset: usize, limit: usize) -> Result<Catalog> {
        self.dispatch(move |runtime| {
            let (mut entries, next_offset, has_more) =
                read_catalog_page(offset, limit, |offset, limit| {
                    runtime.db(json!({"op":"list_terminal_logs", "limit":limit, "offset":offset}))
                })?;
            for entry in &mut entries {
                if let Some(active) = runtime.active.get(&entry.id) {
                    *entry = active.entry.clone();
                }
            }
            let storage_bytes = if offset == 0 {
                match orphans::disk_usage(&runtime.directory) {
                    Ok(bytes) => Some(bytes),
                    Err(error) => {
                        runtime.warn(format!("Recording storage usage: {error}"));
                        None
                    }
                }
            } else {
                None
            };
            let mut active: BTreeSet<_> = runtime.active.keys().cloned().collect();
            for entry in &entries {
                if entry.ended_at.is_none()
                    && !active.contains(&entry.id)
                    && RecordingLease::try_acquire_existing(&runtime.directory, &entry.id)?
                        .is_none()
                {
                    active.insert(entry.id.clone());
                }
            }
            Ok(Catalog {
                entries,
                active,
                warnings: runtime.warnings.clone(),
                storage_bytes,
                next_offset,
                has_more,
            })
        })
        .await
    }
    /// Clear-all prepares every finished row independently of UI pagination.
    pub async fn finished_entries(&self) -> Result<Vec<Entry>> {
        self.dispatch(|runtime| {
            runtime
                .entries()?
                .into_iter()
                .filter(|entry| !runtime.active.contains_key(&entry.id))
                .filter_map(|entry| {
                    if entry.ended_at.is_none() {
                        match RecordingLease::try_acquire_existing(&runtime.directory, &entry.id) {
                            Ok(None) => return None,
                            Err(error) => return Some(Err(error.into())),
                            Ok(Some(_)) => {}
                        }
                    }
                    Some(Ok(entry))
                })
                .collect()
        })
        .await
    }
    pub async fn events(&self, id: String) -> Result<Vec<Event>> {
        self.dispatch(move |runtime| {
            if let Some(active) = runtime.active.get(&id) {
                return Ok(active.events.clone());
            }
            runtime.db(json!({"op":"list_terminal_log_events", "log_id":id}))
        })
        .await
    }
    pub async fn delete(&self, entries: Vec<Entry>) -> Result<usize> {
        self.dispatch(move |runtime| runtime.delete(entries)).await
    }

    pub async fn export_readable(&self, entry: Entry, destination: PathBuf) -> Result<()> {
        self.dispatch(move |runtime| {
            let text = runtime.readable(&entry)?;
            let target = crate::file_commit::DownloadTarget::new(&destination, false)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target.staged())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(0o600))?;
            }
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            target.publish()
        })
        .await
    }
    /// Export decrypted VT bytes only after the user selects a destination.
    /// Publication occurs only after the reader authenticates the entire file.
    pub async fn export(&self, entry: Entry, destination: PathBuf, output: bool) -> Result<()> {
        self.dispatch(move |runtime| {
            ensure!(
                !runtime.active.contains_key(&entry.id),
                "Close the session before exporting its log."
            );
            let current = runtime
                .entry(&entry.id)?
                .context("This log no longer exists. Refresh and try again.")?;
            ensure!(
                current.capture_file == entry.capture_file,
                "The recording changed. Refresh and try again."
            );
            let target = crate::file_commit::DownloadTarget::new(&destination, false)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target.staged())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(0o600))?;
            }
            if output {
                ensure!(
                    current.capture_sha256.is_some()
                        && current.capture_sha256 == entry.capture_sha256,
                    "The recording changed or has no finalized output. Refresh and try again."
                );
                let path = runtime.capture_path(&current)?;
                let info = runtime.core.capture.read(&path, &current.id, |bytes| {
                    file.write_all(bytes)?;
                    Ok(())
                })?;
                info.verify(
                    current.capture_sha256.as_deref().unwrap(),
                    current.capture_bytes,
                )?;
            } else {
                let events: Vec<Event> =
                    runtime.db(json!({"op":"list_terminal_log_events", "log_id":current.id}))?;
                serde_json::to_writer_pretty(&mut file, &json!({"log":current,"events":events}))?;
            }
            file.sync_all()?;
            drop(file);
            target.publish()
        })
        .await
    }
}

pub struct Recorder {
    id: String,
    service: Service,
    capture: bool,
    failed: AtomicBool,
    error: Arc<Mutex<Option<String>>>,
    metadata: Mutex<Option<Metadata>>,
}
#[derive(PartialEq, Eq)]
struct Metadata {
    title: String,
    cwd: Option<String>,
    columns: u32,
    rows: u32,
    exited: bool,
}
impl Recorder {
    pub fn commands(&self, commands: &[String]) {
        if commands.is_empty() {
            return;
        }
        let commands = commands.to_vec();
        let id = self.id.clone();
        let timestamp = now();
        let epoch = self.service.history.epoch();
        let issue = self.error.clone();
        let _ = self.service.worker.queue.try_send(Box::new(move |runtime| {
            if runtime.ignored.contains(&id) {
                return;
            }
            // A final CommandStarted can be drained after the session exits.
            // Global command history does not depend on a live log entry.
            if let Err(error) = runtime.history.append(commands, timestamp, epoch) {
                let message = format!("Could not save shell history: {error}");
                *issue.lock().unwrap() = Some(message.clone());
                runtime.warn(message);
            }
        }));
    }
    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }
    pub fn event(&self, event: &SessionEvent) {
        if let Some(event) = Event::connection(event) {
            self.events(vec![event]);
        }
    }
    pub(crate) fn events(&self, events: Vec<Event>) {
        let id = self.id.clone();
        let _ = self.service.worker.queue.try_send(Box::new(move |runtime| {
            if let Some(active) = runtime.active.get_mut(&id) {
                for event in events {
                    active.push(event);
                }
            }
            runtime.checkpoint(&id, false);
        }));
    }
    pub fn update(&self, title: &str, cwd: Option<String>, columns: u32, rows: u32, exited: bool) {
        let next = Metadata {
            title: title.to_owned(),
            cwd: cwd.clone(),
            columns,
            rows,
            exited,
        };
        let mut previous = self.metadata.lock().unwrap();
        if previous.as_ref() == Some(&next) {
            return;
        }
        *previous = Some(next);
        drop(previous);
        let id = self.id.clone();
        let title: String = title.chars().take(512).collect();
        let _ = self.service.worker.queue.try_send(Box::new(move |runtime| {
            if let Some(active) = runtime.active.get_mut(&id) {
                if !title.is_empty() {
                    active.entry.title = title;
                }
                active.entry.cwd = cwd;
                active.entry.columns = Some(columns);
                active.entry.rows = Some(rows);
            }
            if exited {
                runtime.finish(&id);
            } else {
                runtime.checkpoint(&id, false);
            }
        }));
    }
    pub fn output(&self, bytes: &[u8]) {
        if !self.capture || self.failed.load(Ordering::Relaxed) || bytes.is_empty() {
            return;
        }
        let length = bytes.len();
        if self
            .service
            .worker
            .bytes
            .try_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(length)
                    .filter(|total| *total <= QUEUED_BYTES)
            })
            .is_err()
        {
            if !self.failed.swap(true, Ordering::AcqRel) {
                let message = "Output capture stopped because its bounded queue was full.";
                *self.error.lock().unwrap() = Some(message.into());
                let id = self.id.clone();
                let _ = self
                    .service
                    .worker
                    .queue
                    .try_send(Box::new(move |runtime| runtime.stop_capture(&id, message)));
            }
            return;
        }
        // The reservation is released even if shutdown rejects this job.
        struct Budget(Arc<AtomicUsize>, usize);
        impl Drop for Budget {
            fn drop(&mut self) {
                self.0.fetch_sub(self.1, Ordering::AcqRel);
            }
        }
        let budget = Budget(self.service.worker.bytes.clone(), length);
        let bytes = Zeroizing::new(bytes.to_vec());
        let id = self.id.clone();
        let _ = self.service.worker.queue.try_send(Box::new(move |runtime| {
            let _budget = budget;
            if let Err(error) = runtime.append(&id, &bytes) {
                runtime.stop_capture(&id, &error.to_string());
            }
        }));
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        let id = self.id.clone();
        let _ = self.service.worker.queue.try_send(Box::new(move |runtime| {
            runtime.finish(&id);
            runtime.ignored.remove(&id);
        }));
    }
}
struct Active {
    _lease: RecordingLease,
    entry: Entry,
    events: Vec<Event>,
    writer: Option<capture::Writer>,
    saved: Instant,
}
impl Active {
    fn push(&mut self, event: Event) {
        if self.events.len() == MAX_EVENTS {
            self.events.remove(0);
        }
        self.events.push(event);
    }
}
struct Runtime {
    core: Arc<Core>,
    vault: Vault,
    directory: PathBuf,
    history: crate::shell_history::store::Service,
    active: BTreeMap<String, Active>,
    ignored: BTreeSet<String>,
    preferences: Option<Preferences>,
    warnings: Vec<String>,
    notify: Sender<()>,
}
impl Runtime {
    fn readable(&self, entry: &Entry) -> Result<Zeroizing<String>> {
        ensure!(
            !self.active.contains_key(&entry.id),
            "Close the session before viewing its output recording."
        );
        let current = self
            .entry(&entry.id)?
            .context("This log no longer exists.")?;
        ensure!(
            current.capture_sha256.is_some()
                && current.capture_file == entry.capture_file
                && current.capture_sha256 == entry.capture_sha256,
            "The recording changed or has no finalized output. Refresh and try again."
        );
        let (text, info) = self.core.capture.readable(
            &self.capture_path(&current)?,
            &current.id,
            current.columns.unwrap_or(80),
            current.rows.unwrap_or(24),
        )?;
        info.verify(
            current.capture_sha256.as_deref().unwrap(),
            current.capture_bytes,
        )?;
        Ok(text)
    }
    fn db<T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        request: serde_json::Value,
    ) -> Result<T> {
        smol::block_on(self.vault.dispatch(move |db| db.call(&request)))
    }
    fn warn(&mut self, warning: String) {
        if self.warnings.last() != Some(&warning) {
            if self.warnings.len() == 20 {
                self.warnings.remove(0);
            }
            self.warnings.push(warning);
            let _ = self.notify.try_send(());
        }
    }
    fn entries(&self) -> Result<Vec<Entry>> {
        let mut entries = vec![];
        loop {
            let page: Vec<Entry> =
                self.db(json!({"op":"list_terminal_logs", "limit":500,"offset":entries.len()}))?;
            let done = page.len() < 500;
            entries.extend(page);
            if done {
                return Ok(entries);
            }
        }
    }
    fn entry(&self, id: &str) -> Result<Option<Entry>> {
        self.db(json!({"op":"get_terminal_log","log_id":id}))
    }
    fn capture_path(&self, entry: &Entry) -> Result<PathBuf> {
        managed_path(&self.directory, &entry.id, &entry.capture_file)
    }
    fn save(&self, active: &Active) -> Result<()> {
        let _: String =
            self.db(json!({"op":"save_terminal_log", "log":active.entry, "events":active.events}))?;
        let _ = self.notify.try_send(());
        Ok(())
    }
    fn begin(&mut self, mut entry: Entry, capture: bool) -> Result<()> {
        let lease = RecordingLease::try_acquire(&self.directory, &entry.id)?
            .context("Recording is already in use")?;
        let mut active = Active {
            _lease: lease,
            entry: entry.clone(),
            events: vec![Event {
                timestamp: entry.started_at,
                ..Event::new("connection", "Session started")
            }],
            writer: None,
            saved: Instant::now(),
        };
        // Publish the reference before opening output; a crash never leaves an
        // unreferenced capture. Capture failures do not prevent history or SSH.
        if capture {
            entry.capture_file = format!("{}.ntrcap", entry.id);
            active.entry = entry;
        }
        self.save(&active)?;
        if capture {
            let result = (|| {
                ensure!(
                    !fs::symlink_metadata(&self.directory)
                        .is_ok_and(|meta| meta.file_type().is_symlink()),
                    "Refusing a symlinked recording directory"
                );
                self.core.capture.prepare(&self.directory)?;
                self.core
                    .capture
                    .writer(&self.capture_path(&active.entry)?, &active.entry.id)
            })();
            match result {
                Ok(writer) => active.writer = Some(writer),
                Err(error) => {
                    active.entry.capture_file.clear();
                    active.push(Event::new(
                        "connection",
                        &format!("Output capture unavailable: {error}"),
                    ));
                    self.warn(format!("Output capture unavailable: {error}"));
                    self.save(&active)?;
                }
            }
        }
        self.active.insert(active.entry.id.clone(), active);
        Ok(())
    }
    fn append(&mut self, id: &str, bytes: &[u8]) -> Result<()> {
        let Some(active) = self.active.get_mut(id) else {
            return Ok(());
        };
        let Some(writer) = &mut active.writer else {
            return Ok(());
        };
        for chunk in bytes.chunks(16 * 1024 * 1024) {
            writer.append(chunk)?;
        }
        self.checkpoint(id, false);
        Ok(())
    }
    fn stop_capture(&mut self, id: &str, message: &str) {
        self.end_capture(id, message, true);
    }
    fn end_capture(&mut self, id: &str, message: &str, report_error: bool) {
        if let Some(active) = self.active.get_mut(id) {
            active.push(Event::new("connection", message));
            if let Some(writer) = active.writer.take() {
                match writer.finish() {
                    Ok(info) => {
                        active.entry.capture_bytes = info.ciphertext_bytes;
                        active.entry.capture_sha256 = Some(info.file_sha256);
                    }
                    Err(error) => self.warn(format!("Capture finalization: {error}")),
                }
            }
        }
        if report_error {
            self.warn(message.into());
        }
        self.checkpoint(id, true);
    }
    fn checkpoint(&mut self, id: &str, force: bool) {
        let Some(active) = self.active.get_mut(id) else {
            return;
        };
        if !force && active.saved.elapsed() < Duration::from_secs(2) {
            return;
        }
        if let Some(writer) = &mut active.writer {
            match writer.checkpoint() {
                Ok(info) => active.entry.capture_bytes = info.committed_ciphertext_bytes,
                Err(error) => {
                    self.warn(format!("Capture checkpoint: {error}"));
                    return;
                }
            }
        }
        active.saved = Instant::now();
        let result = self.save(self.active.get(id).unwrap());
        if let Err(error) = result {
            self.warn(format!("Saving session history: {error}"));
        }
    }
    fn finish(&mut self, id: &str) {
        if self.finalize_log(id, "Session ended")
            && let Err(error) = self.retain()
        {
            self.warn(format!("Recording retention: {error}"));
        }
    }
    fn finalize_log(&mut self, id: &str, message: &str) -> bool {
        let Some(mut active) = self.active.remove(id) else {
            return false;
        };
        active.entry.ended_at = Some(now());
        active.push(Event::new("exit", message));
        if let Some(writer) = active.writer.take() {
            match writer.finish() {
                Ok(info) => {
                    active.entry.capture_bytes = info.ciphertext_bytes;
                    active.entry.capture_sha256 = Some(info.file_sha256);
                }
                Err(error) => self.warn(format!("Capture finalization: {error}")),
            }
        }
        if let Err(error) = self.save(&active) {
            self.warn(format!("Finalizing session history: {error}"));
        }
        true
    }
    fn recover(&mut self) -> Result<()> {
        for candidate in self.entries()?.into_iter().filter(|entry| {
            entry.ended_at.is_none()
                || (!entry.capture_file.is_empty() && entry.capture_sha256.is_none())
        }) {
            let Some(_lease) =
                RecordingLease::try_acquire_existing(&self.directory, &candidate.id)?
            else {
                continue;
            };
            let Some(mut entry) = self.entry(&candidate.id)? else {
                continue;
            };
            if entry.ended_at.is_some()
                && (entry.capture_file.is_empty() || entry.capture_sha256.is_some())
            {
                continue;
            }
            if !entry.capture_file.is_empty() && entry.capture_sha256.is_none() {
                let result = self
                    .capture_path(&entry)
                    .and_then(|path| self.core.capture.recover(&path, &entry.id));
                match result {
                    Ok(info) => {
                        entry.capture_bytes = info.ciphertext_bytes;
                        entry.capture_sha256 = Some(info.file_sha256);
                    }
                    Err(error) => self.warn(format!("Could not recover {}: {error}", entry.title)),
                }
            }
            let mut events: Vec<Event> =
                self.db(json!({"op":"list_terminal_log_events", "log_id":entry.id}))?;
            if entry.ended_at.is_none() {
                events.push(Event::new(
                    "exit",
                    "The previous session ended unexpectedly.",
                ));
                entry.ended_at = Some(now());
            }
            let _: String =
                self.db(json!({"op":"save_terminal_log", "log":entry,"events":events}))?;
        }
        Ok(())
    }
    fn delete(&mut self, entries: Vec<Entry>) -> Result<usize> {
        // Hold every selected log through validation, DB commit and file cleanup.
        // try_acquire is nonblocking, so overlapping batches cannot deadlock.
        let mut leases = Vec::with_capacity(entries.len());
        for entry in &entries {
            ensure!(
                !self.active.contains_key(&entry.id),
                "Close the selected sessions before deleting their logs."
            );
            leases.push(
                RecordingLease::try_acquire(&self.directory, &entry.id)?
                    .context("A selected recording is in use by another client")?,
            );
            let actual = self
                .entry(&entry.id)?
                .context("A log changed. Refresh and try again.")?;
            ensure!(
                actual.capture_file == entry.capture_file
                    && actual.capture_sha256 == entry.capture_sha256
                    && actual.ended_at == entry.ended_at,
                "A recording changed. Refresh and try again."
            );
            if !actual.capture_file.is_empty() {
                self.capture_path(&actual)?;
            }
        }
        let expected = entries.clone();
        let removed = smol::block_on(self.vault.dispatch(move |database| {
            database.inner.atomic(|database| {
                let mut removed = vec![];
                for expected in expected {
                    let actual = database
                        .get_terminal_log(&expected.id)?
                        .context("A log changed. Refresh and try again.")?;
                    ensure!(
                        actual.capture_file == expected.capture_file
                            && actual.capture_sha256 == expected.capture_sha256,
                        "A recording changed. Refresh and try again."
                    );
                    removed.push((
                        expected.id.clone(),
                        database.delete_terminal_log(&expected.id)?,
                    ));
                }
                anyhow::Ok(removed)
            })
        }))?;
        for (id, file) in &removed {
            if let Some(file) = file.as_ref().filter(|file| !file.is_empty()) {
                self.cleanup_capture(id, file)?;
            }
        }
        Ok(removed.len())
    }
    fn cleanup_capture(&mut self, id: &str, file: &str) -> Result<()> {
        // Only the exact record-owned file and its checkpoint sidecar.
        let path = managed_path(&self.directory, id, file)?;
        for path in [path.clone(), path.with_extension("ntrcap.state")] {
            match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
                    if let Err(error) = fs::remove_file(&path) {
                        self.warn(format!(
                            "Capture cleanup failed ({}): {error}",
                            path.display()
                        ));
                    }
                }
                other => self.warn(format!(
                    "Capture cleanup was refused ({}): {other:?}",
                    path.display()
                )),
            }
        }
        Ok(())
    }
}

fn valid_identifier(id: &str) -> bool {
    nauterm_core::recording_lease::valid_recording_id(id)
}
fn capture_name(id: &str, file: &str) -> bool {
    file == format!("{id}.ntrcap")
        || file
            .strip_prefix(&format!("{id}."))
            .and_then(|file| file.strip_suffix(".ntrcap"))
            .is_some_and(|revision| {
                revision.len() == 32 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
}
fn managed_path(directory: &Path, id: &str, file: &str) -> Result<PathBuf> {
    ensure!(valid_identifier(id), "Invalid recording identifier.");
    ensure!(
        capture_name(id, file),
        "Refusing a capture path not owned by this recording."
    );
    ensure!(
        !fs::symlink_metadata(directory).is_ok_and(|meta| meta.file_type().is_symlink()),
        "Refusing a symlinked recording directory."
    );
    let path = directory.join(file);
    if let Ok(meta) = fs::symlink_metadata(&path) {
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "Refusing a non-regular capture file."
        );
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    fn fixture_entry(id: usize) -> super::Entry {
        serde_json::from_value(
            serde_json::json!({"id":id.to_string(), "title":"Synthetic log", "started_at":id}),
        )
        .unwrap()
    }
    #[test]
    fn catalog_pages_stop_at_the_requested_window_and_report_the_next_offset() -> anyhow::Result<()>
    {
        let entries: Vec<_> = (0..71).map(fixture_entry).collect();
        let mut calls = vec![];
        let mut fetch = |offset, limit| {
            calls.push((offset, limit));
            Ok(entries.iter().skip(offset).take(limit).cloned().collect())
        };
        let (first, next, more) = super::read_catalog_page(0, 40, &mut fetch)?;
        assert_eq!((first.len(), next, more), (40, 40, true));
        let (second, next, more) = super::read_catalog_page(next, 40, &mut fetch)?;
        assert_eq!((second.len(), next, more), (31, 71, false));
        assert_eq!(calls, [(0, 40), (40, 40)]);
        let entries: Vec<_> = (0..620).map(fixture_entry).collect();
        let mut calls = vec![];
        let (loaded, next, more) = super::read_catalog_page(0, 600, |offset, limit| {
            calls.push((offset, limit));
            Ok(entries.iter().skip(offset).take(limit).cloned().collect())
        })?;
        assert_eq!((loaded.len(), next, more), (600, 600, true));
        assert_eq!(calls, [(0, 500), (500, 100)]);
        Ok(())
    }
    #[test]
    fn overlapping_pages_deduplicate_rows_without_reusing_the_database_offset() {
        let mut catalog = super::Catalog {
            entries: vec![fixture_entry(0), fixture_entry(1)],
            active: Default::default(),
            warnings: vec![],
            storage_bytes: Some(123),
            next_offset: 2,
            has_more: true,
        };
        catalog.append_page(super::Catalog {
            entries: vec![fixture_entry(1), fixture_entry(2)],
            active: Default::default(),
            warnings: vec![],
            storage_bytes: None,
            next_offset: 4,
            has_more: true,
        });
        assert_eq!(
            catalog
                .entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["0", "1", "2"]
        );
        assert_eq!(catalog.next_offset, 4);
        assert_eq!(catalog.storage_bytes, Some(123));
    }
    #[test]
    fn shared_recording_metadata_keeps_its_host_relation_when_rewritten() -> anyhow::Result<()> {
        let original: nauterm_core::TerminalLogEntry = serde_json::from_value(serde_json::json!({
            "id": "12345678-1234-4000-8000-123456789012", "local_id": 19,
            "title": "Synthetic Host", "host_id": 7,
            "host_uuid": "12345678-1234-4000-8000-123456789007",
            "host": "fixture.invalid", "port": 2222, "username": "fixture",
            "capture_file": "", "started_at": 1, "ended_at": 2
        }))?;
        let mut entry: super::Entry = serde_json::from_value(serde_json::to_value(&original)?)?;
        entry.capture_file = "synthetic-revision.ntrcap".into();
        entry.capture_bytes = 64;
        let rewritten: nauterm_core::TerminalLogEntry =
            serde_json::from_value(serde_json::to_value(&entry)?)?;
        assert_eq!(rewritten.local_id, original.local_id);
        assert_eq!(rewritten.host_id, original.host_id);
        assert_eq!(rewritten.host_uuid, original.host_uuid);
        assert_eq!(rewritten.host, original.host);
        let legacy: super::Entry = serde_json::from_value(
            serde_json::json!({"id":"old", "title":"Legacy", "started_at":1}),
        )?;
        assert!(
            legacy.local_id.is_none() && legacy.host_id.is_none() && legacy.host_uuid.is_none()
        );
        Ok(())
    }

    #[test]
    fn recording_lease_protects_metadata_only_sessions_and_releases_on_drop() -> anyhow::Result<()>
    {
        use nauterm_core::recording_lease::{RecordingLease, recording_id_for_file};
        let directory = tempfile::tempdir()?;
        let id = super::identifier();
        assert!(RecordingLease::try_acquire_existing(directory.path(), &id)?.is_none());
        assert_eq!(std::fs::read_dir(directory.path())?.count(), 0);
        let lease = RecordingLease::try_acquire(directory.path(), &id)?.unwrap();
        assert!(RecordingLease::try_acquire(directory.path(), &id)?.is_none());
        assert_eq!(
            recording_id_for_file(&format!("{id}.ntrcap.state")),
            Some(id.as_str())
        );
        assert!(recording_id_for_file(&format!(".nauterm-recording.{id}.lock")).is_none());
        drop(lease);
        assert!(RecordingLease::try_acquire_existing(directory.path(), &id)?.is_some());
        Ok(())
    }
    #[test]
    fn recordings_default_to_metadata_only_and_paths_are_record_owned() -> anyhow::Result<()> {
        let config: super::Preferences = serde_json::from_str("{}")?;
        assert!(config.enabled && !config.capture);
        config.validate()?;
        assert_eq!(config.retention_days, 30);
        assert_eq!(config.max_session_bytes, 100 * 1024 * 1024);
        assert_eq!(config.max_total_bytes, 2 * 1024 * 1024 * 1024);
        assert!(
            super::Preferences {
                max_total_bytes: 0,
                ..config
            }
            .validate()
            .is_err()
        );
        let directory = tempfile::tempdir()?;
        let id = super::identifier();
        let name = format!("{id}.ntrcap");
        assert!(super::managed_path(directory.path(), &id, &name).is_ok());
        assert!(
            super::managed_path(directory.path(), &id, &format!("{id}.{:032x}.ntrcap", 42)).is_ok()
        );
        assert!(super::managed_path(directory.path(), &id, &format!("{id}.other.ntrcap")).is_err());
        assert!(super::managed_path(directory.path(), &id, "../anything").is_err());
        assert!(
            super::managed_path(
                directory.path(),
                &id,
                &format!("{}.ntrcap", super::identifier())
            )
            .is_err()
        );
        #[cfg(unix)]
        {
            let source = tempfile::NamedTempFile::new()?;
            std::os::unix::fs::symlink(source.path(), directory.path().join(&name))?;
            assert!(super::managed_path(directory.path(), &id, &name).is_err());
        }
        Ok(())
    }
}
