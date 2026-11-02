//! Typed access to the reference SFTP subsystem. No terminal shell is opened.
//! Requests are cancellable, own the loaded core, and finish before app teardown.
use crate::file_operation::Operation;
use crate::native::{Core, SessionEvent, SshArguments, SshOptions};
use anyhow::{Context, Result, ensure};
#[cfg(test)]
use async_channel::Receiver;
use async_channel::Sender;
use serde::Deserialize;
use std::sync::{
    Arc, Condvar, Mutex, OnceLock, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Default)]
pub(crate) struct Api;

/// Transient, endpoint-owned authorization. No password is retained here or in
/// task snapshots; existing queued work owns its lease independently of the UI.
pub struct Sudo {
    core: Arc<Core>,
    id: String,
    gate: Mutex<()>,
    authenticated: Mutex<Option<std::time::Instant>>,
    single_use: bool,
}
/// Transient captured endpoint data, never serialized into task/history state.
pub struct AuthorizationTarget {
    pub options: Arc<SshOptions>,
    pub directory: String,
    pub sudo: Arc<Sudo>,
}
/// russh-sftp's remote status display, not an OS/local permission error.
pub fn remote_permission_denied(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    !error.contains("(os error ")
        && (error.starts_with("permission denied:") || error.contains(": permission denied:"))
}
/// Raised only before a file operation starts, or by a read-only listing.
/// Transport failure after a write begins is uncertain, not authorization retry.
#[derive(Debug)]
pub struct SudoExpired(pub String);
impl std::fmt::Display for SudoExpired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SudoExpired {}
fn sudo_not_started(error: &str) -> bool {
    matches!(
        error,
        "sudo SFTP session expired after 15 minutes" | "sudo SFTP session is not authenticated"
    )
}
fn sudo_session_ended(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "sudo sftp session",
        "channel closed",
        "connection closed",
        "connection reset",
        "broken pipe",
        "unexpected eof",
        "transport error",
    ]
    .iter()
    .any(|pattern| error.contains(pattern))
}
impl Sudo {
    pub(crate) fn core(&self) -> Arc<Core> {
        self.core.clone()
    }
    pub fn new(core: Arc<Core>) -> Result<Arc<Self>> {
        Self::new_mode(core, false)
    }
    pub fn new_single_use(core: Arc<Core>) -> Result<Arc<Self>> {
        Self::new_mode(core, true)
    }
    pub fn single_use(&self) -> bool {
        self.single_use
    }
    fn new_mode(core: Arc<Core>, single_use: bool) -> Result<Arc<Self>> {
        let token: [u8; 16] = rand::random();
        let id = token
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(Arc::new(Self {
            core,
            id: format!("nauterm-rs-{id}"),
            gate: Mutex::new(()),
            authenticated: Mutex::new(None),
            single_use,
        }))
    }
    pub fn current(&self) -> bool {
        self.authenticated
            .lock()
            .unwrap()
            .is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(15 * 60))
    }
    fn invalidate(&self) {
        *self.authenticated.lock().unwrap() = None;
    }
}
fn sudo_cleanup() -> &'static Mutex<Vec<std::thread::JoinHandle<()>>> {
    static WORKERS: OnceLock<Mutex<Vec<std::thread::JoinHandle<()>>>> = OnceLock::new();
    WORKERS.get_or_init(Mutex::default)
}
impl Drop for Sudo {
    fn drop(&mut self) {
        let core = self.core.clone();
        let id = self.id.clone();
        let mut workers = sudo_cleanup().lock().unwrap();
        workers.retain(|worker| !worker.is_finished());
        workers.push(std::thread::spawn(move || {
            let _core = core;
            nauterm_core::ssh::close_sudo_sftp_session(&id);
        }));
    }
}

#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub bytes: u64,
    pub total: u64,
    pub path: String,
}
struct ProgressSink {
    latest: Mutex<Progress>,
    #[cfg(test)]
    changed: Sender<()>,
    wakeup: Option<Sender<()>>,
}
#[derive(Clone)]
pub struct ProgressFeed {
    sink: Arc<ProgressSink>,
    #[cfg(test)]
    pub changed: Receiver<()>,
}
impl ProgressFeed {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::with_notifier(None)
    }
    pub(crate) fn with_notifier(wakeup: Option<Sender<()>>) -> Self {
        #[cfg(test)]
        let (changed, receiver) = async_channel::bounded(1);
        Self {
            sink: Arc::new(ProgressSink {
                latest: Mutex::default(),
                #[cfg(test)]
                changed,
                wakeup,
            }),
            #[cfg(test)]
            changed: receiver,
        }
    }
    pub fn latest(&self) -> Progress {
        self.sink.latest.lock().unwrap().clone()
    }
    pub(crate) fn complete(&self, bytes: u64, path: String) {
        *self.sink.latest.lock().unwrap() = Progress {
            bytes,
            total: bytes,
            path,
        };
    }
}
fn report_progress(sink: &ProgressSink, bytes: u64, total: u64, path: &str) {
    let path = path.chars().take(16 * 1024).collect();
    if let Ok(mut latest) = sink.latest.lock() {
        *latest = Progress { bytes, total, path };
    }
    #[cfg(test)]
    let _ = sink.changed.try_send(());
    if let Some(wakeup) = &sink.wakeup {
        let _ = wakeup.try_send(());
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct TaskResult {
    pub ok: bool,
    pub bytes: u64,
    pub item_kind: String,
    pub events: Vec<SessionEvent>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Entry {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
    pub modified: Option<u64>,
    #[serde(default)]
    pub mode: Option<u32>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Listing {
    pub directory: String,
    pub entries: Vec<Entry>,
    pub events: Vec<SessionEvent>,
    pub error: Option<String>,
}
impl Listing {
    fn validate(&self) -> Result<()> {
        if self.error.is_some() {
            return Ok(());
        }
        ensure!(
            self.directory.starts_with('/') && !self.directory.contains('\0'),
            "Invalid SFTP directory response"
        );
        for entry in &self.entries {
            ensure!(
                !entry.name.is_empty()
                    && ![".", ".."].contains(&entry.name.as_str())
                    && !entry.name.contains(['/', '\0']),
                "Invalid SFTP filename response"
            );
        }
        Ok(())
    }
}

#[derive(Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Pending,
    Running,
    Finished,
}
#[derive(Default)]
struct State {
    phase: Phase,
    cancelled: bool,
}
struct Active {
    id: u64,
    core: Arc<Core>,
    state: Mutex<State>,
    finished: Condvar,
}
impl Active {
    fn cancel(&self) {
        let mut state = self.state.lock().unwrap();
        if state.cancelled || state.phase == Phase::Finished {
            return;
        }
        state.cancelled = true;
        if state.phase == Phase::Running {
            // The native API records cancellation even if its runtime has not
            // registered yet, so cancelling a just-started request is effective.
            nauterm_core::ssh::cancel_sftp_task(self.id);
        }
    }
}
fn active_requests() -> &'static Mutex<Vec<Weak<Active>>> {
    static REQUESTS: OnceLock<Mutex<Vec<Weak<Active>>>> = OnceLock::new();
    REQUESTS.get_or_init(Mutex::default)
}
static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1 << 63);
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

pub struct Request(Arc<Active>);
/// UI-owned guard. Dropping it cancels work even while the worker owns Request.
pub struct Cancellation(Arc<Active>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.cancel();
    }
}
impl Drop for Cancellation {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
impl Request {
    pub fn new(core: Arc<Core>) -> Result<(Self, Cancellation)> {
        ensure!(
            !SHUTTING_DOWN.load(Ordering::SeqCst),
            "SFTP is shutting down"
        );
        core.sftp_api()?;
        let active = Arc::new(Active {
            id: NEXT_REQUEST.fetch_add(1, Ordering::SeqCst),
            core,
            state: Mutex::default(),
            finished: Condvar::new(),
        });
        let mut requests = active_requests().lock().unwrap();
        requests.retain(|request| request.strong_count() > 0);
        requests.push(Arc::downgrade(&active));
        Ok((Self(active.clone()), Cancellation(active)))
    }

    /// Blocking; call only from a background executor. Credentials remain
    /// transient/zeroized and use the exact reference trust/authentication code.
    pub fn list(self, options: &SshOptions, directory: &str) -> Result<Listing> {
        self.list_authorized(options, directory, None, None)
    }
    pub fn list_authorized(
        self,
        options: &SshOptions,
        directory: &str,
        sudo: Option<&Sudo>,
        password: Option<&str>,
    ) -> Result<Listing> {
        let _gate = sudo.map(|sudo| sudo.gate.lock().unwrap());
        if let Some(sudo) = sudo {
            ensure!(
                Arc::ptr_eq(&sudo.core, &self.0.core),
                "Sudo belongs to another native core."
            );
            if password.is_none() && !sudo.current() {
                return Err(SudoExpired(
                    "Sudo authorization expired. Authenticate again to continue.".into(),
                )
                .into());
            }
        }
        ensure!(
            !directory.trim().is_empty() && directory.len() <= 64 * 1024,
            "A directory path is required"
        );
        let arguments = SshArguments::new(options)?;
        {
            let mut state = self.0.state.lock().unwrap();
            ensure!(
                !state.cancelled && !SHUTTING_DOWN.load(Ordering::SeqCst),
                "SFTP listing cancelled"
            );
            state.phase = Phase::Running;
        }
        struct Completion<'a>(&'a Active);
        impl Drop for Completion<'_> {
            fn drop(&mut self) {
                self.0.state.lock().unwrap().phase = Phase::Finished;
                self.0.finished.notify_all();
            }
        }
        let completion = Completion(&self.0);
        let listing = if let Some(sudo) = sudo {
            nauterm_core::ssh::sudo_browsing::list(
                self.0.id,
                &options.host,
                options.port,
                &options.username,
                Some(&options.password),
                Some(&options.private_key),
                Some(&options.certificate),
                Some(arguments.auth()),
                Some(
                    options
                        .known_hosts
                        .to_str()
                        .context("Known hosts path is not UTF-8")?,
                ),
                arguments.native_proxy(),
                directory,
                options.trust.native(),
                &sudo.id,
                password,
            )
        } else {
            nauterm_core::ssh::list_sftp_directory_entries_blocking_with_trust(
                self.0.id,
                &options.host,
                options.port,
                &options.username,
                Some(&options.password),
                Some(&options.private_key),
                Some(&options.certificate),
                Some(arguments.auth()),
                Some(
                    options
                        .known_hosts
                        .to_str()
                        .context("Known hosts path is not UTF-8")?,
                ),
                arguments.native_proxy(),
                directory,
                options.trust.native(),
            )
        };
        let listing: Listing = serde_json::from_value(serde_json::to_value(listing)?)?;
        listing.validate()?;
        if let Some(sudo) = sudo {
            if listing.error.is_none() {
                let mut at = sudo.authenticated.lock().unwrap();
                if at.is_none() {
                    *at = Some(std::time::Instant::now());
                }
            } else if listing
                .error
                .as_ref()
                .is_some_and(|error| sudo_session_ended(error))
            {
                sudo.invalidate();
                if password.is_none() {
                    return Err(SudoExpired(listing.error.clone().unwrap()).into());
                }
            }
        }
        drop(completion);
        Ok(listing)
    }

    pub fn execute(
        self,
        options: &SshOptions,
        operation: &Operation,
        feed: &ProgressFeed,
    ) -> Result<TaskResult> {
        self.execute_authorized(options, operation, feed, None)
    }
    pub fn execute_authorized(
        self,
        options: &SshOptions,
        operation: &Operation,
        feed: &ProgressFeed,
        sudo: Option<&Sudo>,
    ) -> Result<TaskResult> {
        let _gate = sudo.map(|sudo| sudo.gate.lock().unwrap());
        if let Some(sudo) = sudo {
            ensure!(
                Arc::ptr_eq(&sudo.core, &self.0.core),
                "Sudo belongs to another native core."
            );
            if !sudo.current() {
                return Err(SudoExpired(
                    "Sudo authorization expired before this attempt started a remote operation."
                        .into(),
                )
                .into());
            }
        }
        operation.validate()?;
        let arguments = SshArguments::new(options)?;
        let mut wire = serde_json::to_value(operation)?;
        if let Some(sudo) = sudo {
            wire["sudo_session_id"] = sudo.id.as_str().into();
        }
        let wire = serde_json::to_string(&wire)?;
        {
            let mut state = self.0.state.lock().unwrap();
            ensure!(
                !state.cancelled && !SHUTTING_DOWN.load(Ordering::SeqCst),
                "SFTP task cancelled"
            );
            state.phase = Phase::Running;
        }
        struct Completion<'a>(&'a Active);
        impl Drop for Completion<'_> {
            fn drop(&mut self) {
                self.0.state.lock().unwrap().phase = Phase::Finished;
                self.0.finished.notify_all();
            }
        }
        let _completion = Completion(&self.0);
        let sink = feed.sink.clone();
        let result = nauterm_core::ssh::execute_sftp_task_with_progress(
            self.0.id,
            &options.host,
            options.port,
            &options.username,
            Some(&options.password),
            Some(&options.private_key),
            Some(&options.certificate),
            Some(arguments.auth()),
            Some(
                options
                    .known_hosts
                    .to_str()
                    .context("Known hosts path is not UTF-8")?,
            ),
            arguments.native_proxy(),
            &wire,
            options.trust.native(),
            Some(Arc::new(move |bytes, total, path| {
                report_progress(&sink, bytes, total, path)
            })),
        );
        let result: TaskResult = serde_json::from_value(serde_json::to_value(result)?)?;
        ensure!(
            !result.ok || result.error.is_none(),
            "Native SFTP returned an inconsistent task result"
        );
        if let Some(sudo) = sudo
            && result
                .error
                .as_ref()
                .is_some_and(|error| error.to_ascii_lowercase().contains("sudo sftp session"))
        {
            sudo.invalidate();
            if result
                .error
                .as_ref()
                .is_some_and(|error| sudo_not_started(error))
            {
                return Err(SudoExpired(result.error.clone().unwrap()).into());
            }
        }
        Ok(result)
    }
}

pub fn prepare_shutdown() {
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
    close_window();
}
pub fn close_window() {
    let requests: Vec<_> = active_requests()
        .lock()
        .unwrap()
        .iter()
        .filter_map(Weak::upgrade)
        .collect();
    for request in &requests {
        request.cancel();
    }
    for request in requests {
        let mut state = request.state.lock().unwrap();
        while state.phase == Phase::Running {
            state = request.finished.wait(state).unwrap();
        }
    }
    for worker in std::mem::take(&mut *sudo_cleanup().lock().unwrap()) {
        let _ = worker.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_is_coalesced() {
        let feed = ProgressFeed::new();
        for bytes in 0..100 {
            report_progress(&feed.sink, bytes, 100, "/fixture/中文");
        }
        assert_eq!(feed.latest().bytes, 99);
        assert_eq!(feed.latest().total, 100);
        assert_eq!(feed.latest().path, "/fixture/中文");
        assert!(feed.changed.try_recv().is_ok());
        assert!(feed.changed.try_recv().is_err());
    }

    #[test]
    fn rejects_server_entries_that_escape_the_displayed_directory() {
        for name in ["", ".", "..", "../secrets", "/etc", "nul\0name"] {
            let listing = Listing {
                directory: "/home/fixture".into(),
                entries: vec![Entry {
                    name: name.into(),
                    is_directory: false,
                    size: 0,
                    modified: None,
                    mode: None,
                }],
                events: vec![],
                error: None,
            };
            assert!(listing.validate().is_err());
        }
    }
}
