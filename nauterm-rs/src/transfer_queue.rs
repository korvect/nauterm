//! Shared, UI-independent transfer scheduling. A paused editor retains its
//! immutable payload until resume/cancel; shutdown never depends on UI pumping.
use crate::{
    file_commit::DownloadTarget,
    file_operation::Operation,
    native::{Core, SshOptions},
    sftp_client::{Cancellation, Progress, ProgressFeed, Request, TaskResult},
    sftp_preferences::Limits,
};
use anyhow::{Context, Result, ensure};
use std::{
    any::Any,
    sync::{
        Arc, Condvar, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};
mod permission;
pub use permission::{PermissionTarget, PermissionTicket};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Queued,
    Running,
    Pausing,
    Cancelling,
    Paused,
    Completed,
    Failed,
    Cancelled,
}
impl Status {
    pub fn running(self) -> bool {
        matches!(self, Self::Running | Self::Pausing | Self::Cancelling)
    }
    pub fn finished(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Pausing => "Pausing…",
            Self::Cancelling => "Cancelling…",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
    fn complete(self, ok: bool) -> Self {
        if ok {
            Self::Completed
        } else {
            match self {
                Self::Pausing => Self::Paused,
                Self::Cancelling => Self::Cancelled,
                _ => Self::Failed,
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Download,
    Upload,
    Edit,
    Move,
    Copy,
    Delete,
    NewFolder,
}
impl Kind {
    fn operation(operation: &Operation) -> Self {
        match operation {
            Operation::Download { .. } => Self::Download,
            Operation::Upload { .. } => Self::Upload,
            Operation::Move { .. } => Self::Move,
            Operation::Copy { .. } => Self::Copy,
            Operation::Delete { .. } => Self::Delete,
            Operation::Mkdir { .. } => Self::NewFolder,
        }
    }
    pub fn wire(self) -> &'static str {
        match self {
            Self::Download => "download",
            Self::Upload => "upload",
            Self::Edit => "edit",
            Self::Move => "move",
            Self::Copy => "copy",
            Self::Delete => "delete",
            Self::NewFolder => "mkdir",
        }
    }
    pub fn from_wire(value: &str) -> Option<Self> {
        Some(match value {
            "download" => Self::Download,
            "upload" => Self::Upload,
            "edit" => Self::Edit,
            "move" => Self::Move,
            "copy" => Self::Copy,
            "delete" => Self::Delete,
            _ => return None,
        })
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Download => "Download",
            Self::Upload => "Upload",
            Self::Edit => "Edit",
            Self::Move => "Move",
            Self::Copy => "Copy",
            Self::Delete => "Delete",
            Self::NewFolder => "New Folder",
        }
    }
}
pub type Hook = Arc<dyn Fn() -> Result<()> + Send + Sync>;
pub type Payload = Arc<dyn Any + Send + Sync>;
#[derive(Debug)]
pub struct GuardRejected(pub String);
impl std::fmt::Display for GuardRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for GuardRejected {}
#[derive(Debug)]
struct Uncertain(String);
impl std::fmt::Display for Uncertain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Uncertain {}

#[derive(Clone)]
pub struct Spec {
    pub owner: u64,
    pub options: Arc<SshOptions>,
    pub sudo: Option<Arc<crate::sftp_client::Sudo>>,
    pub operation: Operation,
    pub kind: Option<Kind>,
    pub replace_local: bool,
    pub core: Option<Arc<Core>>,
    pub label: Option<String>,
    pub display_source: Option<String>,
    pub display_target: Option<String>,
    pub before: Option<Hook>,
    pub after: Option<Hook>,
    pub payload: Option<Payload>,
    pub refresh_owner: bool,
    pub refresh_local: bool,
    pub inherit_transfer_threads: bool,
}
impl Spec {
    pub fn kind(&self) -> Kind {
        self.kind
            .unwrap_or_else(|| Kind::operation(&self.operation))
    }
    pub fn new(owner: u64, options: Arc<SshOptions>, operation: Operation) -> Self {
        Self {
            owner,
            options,
            sudo: None,
            operation,
            kind: None,
            replace_local: false,
            core: None,
            label: None,
            display_source: None,
            display_target: None,
            before: None,
            after: None,
            payload: None,
            refresh_owner: true,
            refresh_local: true,
            inherit_transfer_threads: false,
        }
    }
}
pub(crate) struct Plan {
    spec: Spec,
    download: Mutex<Option<DownloadTarget>>,
}
impl Plan {
    fn execute(&self, attempt: &Attempt, feed: &ProgressFeed) -> Result<TaskResult> {
        if self.spec.sudo.as_ref().is_some_and(|sudo| !sudo.current()) {
            return Err(crate::sftp_client::SudoExpired("Sudo authorization expired. This attempt did not start a remote operation. Authenticate again to continue.".into()).into());
        }
        let _payload = self.spec.payload.as_ref();
        if let Some(before) = &self.spec.before {
            before()?;
        }
        let core = match &self.spec.core {
            Some(core) => core.clone(),
            None => Core::discover()?,
        };
        let request = attempt.request(core)?;
        let mut wire = self.spec.operation.clone();
        if let Operation::Download { local_path, .. } = &mut wire {
            let mut target = self.download.lock().unwrap();
            if target.is_none() {
                *target = Some(DownloadTarget::new(
                    std::path::Path::new(local_path),
                    self.spec.replace_local,
                )?);
            }
            *local_path = target
                .as_ref()
                .unwrap()
                .staged()
                .to_str()
                .context("The download staging path cannot be encoded.")?
                .into();
        }
        let result = if self.spec.sudo.is_some() {
            request.execute_authorized(
                &self.spec.options,
                &wire,
                feed,
                self.spec.sudo.as_deref(),
            )?
        } else {
            request.execute(&self.spec.options, &wire, feed)?
        };
        if !result.ok && self.spec.sudo.as_ref().is_some_and(|sudo| !sudo.current()) {
            return Err(Uncertain(format!(
                "{}. Check the destination for partial changes, then choose With sudo again to authenticate. This task will not be replayed automatically.",
                result.error.as_deref().unwrap_or("The privileged SFTP session ended")
            )).into());
        }
        if result.ok {
            if let Some(download) = self.download.lock().unwrap().as_ref() {
                download.publish()?;
            }
            if let Some(after) = &self.spec.after {
                after().map_err(|error| {
                    anyhow::Error::new(Uncertain(format!(
                        "The transfer completed but verification failed: {error}. Check the destination before retrying."
                    )))
                })?;
            }
        }
        Ok(result)
    }
    fn conflicts(&self, other: &Self) -> bool {
        self.spec.operation.conflicts(
            &other.spec.operation,
            self.spec.options.host == other.spec.options.host
                && self.spec.options.port == other.spec.options.port,
        )
    }
}
#[derive(Default)]
struct Attempt {
    cancelled: AtomicBool,
    native: Mutex<Option<Cancellation>>,
}
impl Attempt {
    fn request(&self, core: Arc<Core>) -> Result<Request> {
        let mut native = self.native.lock().unwrap();
        ensure!(
            !self.cancelled.load(Ordering::SeqCst),
            "SFTP transfer cancelled."
        );
        let (request, cancel) = Request::new(core)?;
        *native = Some(cancel);
        Ok(request)
    }
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(native) = self.native.lock().unwrap().as_ref() {
            native.cancel();
        }
    }
}
#[derive(Default)]
struct Completion {
    result: Mutex<Option<Result<TaskResult>>>,
    ready: Condvar,
}
impl Completion {
    fn finish(&self, result: Result<TaskResult>) {
        *self.result.lock().unwrap() = Some(result);
        self.ready.notify_all();
    }
}
struct Scope {
    id: u64,
    closing: AtomicBool,
    cancelled: AtomicBool,
    active_edit: AtomicBool,
}
struct Job {
    kind: Kind,
    created_at: i64,
    finished_at: Option<i64>,
    uuid: String,
    history_record: Option<nauterm_core::SftpTaskHistoryEntry>,
    recorded_outcome: Option<(Status, u64)>,
    id: u64,
    owner: u64,
    label: String,
    source: String,
    target: String,
    display_progress: Option<String>,
    local_target: Option<String>,
    refresh_owner: bool,
    pausable: bool,
    permission_retry: bool,
    status: Status,
    error: Option<String>,
    plan: Option<Arc<Plan>>,
    attempt: u64,
    control: Option<Arc<Attempt>>,
    feed: ProgressFeed,
    reply: Option<Arc<Completion>>,
    scope: Option<Arc<Scope>>,
}
impl Job {
    fn reservation(&self) -> bool {
        self.plan.is_some()
            && (self.status.running() || matches!(self.status, Status::Paused | Status::Failed))
    }
    fn conflicts(&self, other: &Self) -> bool {
        self.plan
            .as_ref()
            .zip(other.plan.as_ref())
            .is_some_and(|(a, b)| a.conflicts(b))
    }
}
struct State {
    history: Option<Arc<crate::transfer_history::History>>,
    history_owners: std::collections::HashMap<u64, crate::transfer_history::Identity>,
    jobs: Vec<Job>,
    next: u64,
    draining: bool,
    closed: bool,
    view: bool,
    limits: Limits,
}
fn record_history(history: Option<&Arc<crate::transfer_history::History>>, job: &mut Job) {
    if !job.status.finished() {
        return;
    }
    let finished = *job
        .finished_at
        .get_or_insert_with(|| chrono::Utc::now().timestamp_millis().max(job.created_at));
    let outcome = (job.status, job.attempt);
    if job.recorded_outcome == Some(outcome) {
        return;
    }
    let (Some(history), Some(mut record)) = (history, job.history_record.clone()) else {
        return;
    };
    record.status = match job.status {
        Status::Completed => "completed",
        Status::Failed => "failed",
        Status::Cancelled => "cancelled",
        _ => return,
    }
    .into();
    let progress = job.feed.latest();
    record.bytes = progress.bytes.min(i64::MAX as u64) as i64;
    record.total_bytes = progress.total.min(i64::MAX as u64) as i64;
    record.error.clone_from(&job.error);
    record.finished_at = finished;
    if history.save(record) {
        job.recorded_outcome = Some(outcome);
    }
}
fn next_job(state: &State) -> Option<usize> {
    if state.closed
        || state.jobs.iter().filter(|job| job.status.running()).count()
            >= state.limits.concurrent_tasks
    {
        return None;
    }
    state.jobs.iter().position(|job| {
        job.status == Status::Queued
            && job.plan.is_some()
            && !state
                .jobs
                .iter()
                .any(|other| other.id != job.id && other.reservation() && job.conflicts(other))
    })
}
struct Inner {
    state: Mutex<State>,
    changes: async_channel::Sender<()>,
    incoming: async_channel::Receiver<()>,
    workers: Mutex<Vec<thread::JoinHandle<()>>>,
}
#[derive(Clone)]
pub struct Service(Arc<Inner>);
#[derive(Clone)]
pub struct WeakService(Weak<Inner>);
impl WeakService {
    pub fn upgrade(&self) -> Option<Service> {
        self.0.upgrade().map(Service)
    }
}
pub struct Snapshot {
    pub kind: Kind,
    pub created_at: i64,
    pub finished_at: Option<i64>,
    pub uuid: String,
    pub history_id: Option<i64>,
    pub id: u64,
    pub owner: u64,
    pub label: String,
    pub source: String,
    pub target: String,
    pub local_target: Option<String>,
    pub status: Status,
    pub pausable: bool,
    pub resumable: bool,
    pub permission_retry: bool,
    pub attempt: u64,
    pub progress: Progress,
    pub error: Option<String>,
    pub refresh_owner: bool,
}
impl Snapshot {
    fn stable_id(&self) -> u64 {
        self.history_id.map_or(self.id, |id| id as u64)
    }
    pub fn oldest_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.created_at
            .cmp(&other.created_at)
            .then_with(|| self.stable_id().cmp(&other.stable_id()))
    }
    pub fn compare(&self, other: &Self) -> std::cmp::Ordering {
        let group = |status: Status| {
            if status.running() {
                0
            } else if status.finished() {
                2
            } else {
                1
            }
        };
        group(self.status).cmp(&group(other.status)).then_with(|| {
            if self.status.finished() {
                other
                    .finished_at
                    .unwrap_or(other.created_at)
                    .cmp(&self.finished_at.unwrap_or(self.created_at))
                    .then_with(|| other.oldest_cmp(self))
            } else {
                self.oldest_cmp(other)
            }
        })
    }
}
static CLOSING: AtomicBool = AtomicBool::new(false);
static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<Vec<Weak<Inner>>> {
    static REGISTRY: OnceLock<Mutex<Vec<Weak<Inner>>>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}
pub fn is_shutting_down() -> bool {
    CLOSING.load(Ordering::SeqCst)
}
impl Service {
    pub fn download_targets(&self) -> std::collections::HashSet<std::path::PathBuf> {
        self.0
            .state
            .lock()
            .unwrap()
            .jobs
            .iter()
            .filter_map(|job| {
                let plan = job.plan.as_ref()?;
                match &plan.spec.operation {
                    Operation::Download { local_path, .. } => Some(local_path.into()),
                    _ => None,
                }
            })
            .collect()
    }
    pub fn downgrade(&self) -> WeakService {
        WeakService(Arc::downgrade(&self.0))
    }
    pub fn new(view: bool) -> Self {
        let (changes, incoming) = async_channel::bounded(1);
        let inner = Arc::new(Inner {
            state: Mutex::new(State {
                history: None,
                history_owners: Default::default(),
                jobs: vec![],
                next: 1,
                draining: is_shutting_down(),
                closed: false,
                view,
                limits: Limits::default(),
            }),
            changes,
            incoming,
            workers: Mutex::new(vec![]),
        });
        let mut registry = registry().lock().unwrap();
        registry.retain(|queue| queue.strong_count() > 0);
        registry.push(Arc::downgrade(&inner));
        Self(inner)
    }
    pub fn changes(&self) -> async_channel::Receiver<()> {
        self.0.incoming.clone()
    }
    pub fn set_history(&self, history: Arc<crate::transfer_history::History>) {
        self.0.state.lock().unwrap().history = Some(history);
    }
    pub fn register_history_owner(&self, owner: u64, identity: crate::transfer_history::Identity) {
        self.0
            .state
            .lock()
            .unwrap()
            .history_owners
            .insert(owner, identity);
    }
    pub fn set_limits(&self, limits: Limits) -> Result<()> {
        limits.validate()?;
        let mut state = self.0.state.lock().unwrap();
        state.limits = limits;
        Self::pump(&self.0, &mut state);
        self.wake();
        Ok(())
    }
    pub fn client(&self, owner: u64) -> Client {
        Client {
            service: self.clone(),
            owner,
            scope: Arc::new(Scope {
                id: NEXT_SCOPE.fetch_add(1, Ordering::Relaxed),
                closing: AtomicBool::new(false),
                cancelled: AtomicBool::new(false),
                active_edit: AtomicBool::new(false),
            }),
        }
    }
    pub fn snapshots(&self) -> Vec<Snapshot> {
        self.0
            .state
            .lock()
            .unwrap()
            .jobs
            .iter()
            .map(|job| Snapshot {
                kind: job.kind,
                created_at: job.created_at,
                finished_at: job.finished_at,
                uuid: job.uuid.clone(),
                history_id: None,
                id: job.id,
                owner: job.owner,
                label: job.label.clone(),
                source: job.source.clone(),
                target: job.target.clone(),
                local_target: job.local_target.clone(),
                status: job.status,
                pausable: job.pausable
                    && !job
                        .scope
                        .as_ref()
                        .is_some_and(|scope| scope.closing.load(Ordering::SeqCst)),
                resumable: job.plan.is_some(),
                permission_retry: job.permission_retry && job.plan.is_some(),
                attempt: job.attempt,
                progress: {
                    let mut progress = job.feed.latest();
                    if let Some(path) = &job.display_progress {
                        progress.path.clone_from(path);
                    }
                    progress
                },
                error: job.error.clone(),
                refresh_owner: job.refresh_owner,
            })
            .collect()
    }
    pub fn has_work(&self) -> bool {
        self.0
            .state
            .lock()
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.plan.is_some())
    }
    pub fn enqueue(&self, spec: Spec) -> Result<u64> {
        self.submit(spec, None, None)
    }
    fn submit(
        &self,
        mut spec: Spec,
        scope: Option<Arc<Scope>>,
        reply: Option<Arc<Completion>>,
    ) -> Result<u64> {
        spec.operation.validate()?;
        let mut state = self.0.state.lock().unwrap();
        if spec.inherit_transfer_threads {
            spec.operation
                .set_transfer_threads(state.limits.transfer_threads);
        }
        ensure!(!state.closed, "The transfer queue is closed.");
        ensure!(
            !state.draining
                || scope
                    .as_ref()
                    .is_some_and(|scope| scope.closing.load(Ordering::SeqCst)
                        || scope.active_edit.load(Ordering::SeqCst)),
            "The application is shutting down."
        );
        ensure!(
            !scope
                .as_ref()
                .is_some_and(|scope| scope.cancelled.load(Ordering::SeqCst)),
            "Remote edit transfer cancelled."
        );
        let id = state.next;
        state.next += 1;
        let uuid = uuid::Uuid::new_v4().to_string();
        let history_record = state
            .history_owners
            .get(&spec.owner)
            .and_then(|identity| crate::transfer_history::entry(identity, &spec, uuid.clone()));
        let job = Job {
            kind: spec.kind(),
            created_at: history_record.as_ref().map_or_else(
                || chrono::Utc::now().timestamp_millis(),
                |record| record.created_at,
            ),
            finished_at: None,
            uuid,
            history_record,
            recorded_outcome: None,
            id,
            owner: spec.owner,
            label: spec
                .label
                .clone()
                .unwrap_or_else(|| spec.operation.label().into()),
            source: spec
                .display_source
                .clone()
                .unwrap_or_else(|| spec.operation.source().into()),
            target: spec
                .display_target
                .clone()
                .unwrap_or_else(|| spec.operation.target().into()),
            display_progress: if matches!(spec.operation, Operation::Download { .. }) {
                spec.display_target.clone()
            } else {
                spec.display_source.clone()
            },
            local_target: (spec.refresh_local
                && matches!(spec.operation, Operation::Download { .. }))
            .then(|| spec.operation.target().into()),
            refresh_owner: spec.refresh_owner,
            pausable: spec.operation.pausable(),
            permission_retry: false,
            status: Status::Queued,
            error: None,
            attempt: 0,
            control: None,
            feed: ProgressFeed::with_notifier(Some(self.0.changes.clone())),
            reply,
            scope,
            plan: Some(Arc::new(Plan {
                spec,
                download: Mutex::default(),
            })),
        };
        if job
            .scope
            .as_ref()
            .is_some_and(|scope| scope.closing.load(Ordering::SeqCst))
        {
            ensure!(
                !state.jobs.iter().any(|other| other.reservation()
                    && !other.status.running()
                    && job.conflicts(other)),
                "A paused/failed overlapping transfer blocks this final save; the local edit is retained."
            );
        }
        state.jobs.push(job);
        Self::pump(&self.0, &mut state);
        self.wake();
        Ok(id)
    }
    fn wake(&self) {
        let _ = self.0.changes.try_send(());
    }
    fn pump(inner: &Arc<Inner>, state: &mut State) {
        while let Some(index) = next_job(state) {
            let job = &mut state.jobs[index];
            let id = job.id;
            job.status = Status::Running;
            job.finished_at = None;
            job.error = None;
            job.attempt += 1;
            let attempt = Arc::new(Attempt::default());
            job.control = Some(attempt.clone());
            let plan = job.plan.as_ref().unwrap().clone();
            let feed = job.feed.clone();
            let worker = inner.clone();
            let thread = thread::Builder::new()
                .name("nauterm-rs-transfer".into())
                .spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        plan.execute(&attempt, &feed)
                    }))
                    .unwrap_or_else(|_| {
                        Err(Uncertain(
                            "Transfer worker stopped unexpectedly. Check the destination before retrying."
                                .into(),
                        )
                        .into())
                    });
                    Service::finish(&worker, id, result);
                });
            match thread {
                Ok(thread) => {
                    let mut workers = inner.workers.lock().unwrap();
                    workers.retain(|thread| !thread.is_finished());
                    workers.push(thread);
                }
                Err(error) => {
                    job.status = Status::Failed;
                    job.error = Some(error.to_string());
                    job.plan = None;
                    job.control = None;
                    record_history(state.history.as_ref(), job);
                    if let Some(reply) = job.reply.take() {
                        reply.finish(Err(error.into()));
                    }
                }
            }
        }
    }
    fn finish(inner: &Arc<Inner>, id: u64, mut result: Result<TaskResult>) {
        let mut state = inner.state.lock().unwrap();
        let history = state.history.clone();
        let view = state.view;
        let draining = state.draining;
        let Some(job) = state.jobs.iter_mut().find(|job| job.id == id) else {
            return;
        };
        let ok = result.as_ref().is_ok_and(|result| result.ok);
        // Explicit cancellation must not be turned into a new conflict prompt
        // by a simultaneously rejected preflight. Completed writes still win.
        if !ok && job.status == Status::Cancelling {
            result = Err(anyhow::anyhow!(
                "Transfer cancelled. Remote partial data may remain."
            ));
        }
        if let Ok(result) = &result
            && result.ok
        {
            job.feed.complete(result.bytes, result.item_kind.clone());
            if let Some(record) = &mut job.history_record {
                record.item_kind.clone_from(&result.item_kind);
            }
        }
        let no_retry = result.as_ref().err().is_some_and(|error| {
            error.is::<GuardRejected>()
                || error.is::<Uncertain>()
                || error.is::<crate::sftp_client::SudoExpired>()
        });
        job.status = if no_retry {
            Status::Failed
        } else {
            job.status.complete(ok)
        };
        if !ok
            && job
                .scope
                .as_ref()
                .is_some_and(|scope| scope.closing.load(Ordering::SeqCst))
            && job.status == Status::Paused
        {
            job.status = Status::Cancelled;
        }
        job.control = None;
        job.error = match &result {
            Ok(result) => result.error.clone().or_else(|| {
                (!result.ok)
                    .then(|| result.events.last().map(|event| event.message.clone()))
                    .flatten()
            }),
            Err(error) => Some(error.to_string()),
        };
        if job.status == Status::Paused {
            job.error = None;
        }
        if job.status == Status::Cancelled {
            job.error = Some("Transfer cancelled. Remote partial data may remain.".into());
        }
        if job.status == Status::Failed {
            job.error
                .get_or_insert_with(|| "SFTP task failed.".into())
                .push_str(" Check the destination before retrying; part of the operation may have completed.");
        }
        job.permission_retry =
            view && !draining
                && job.scope.is_none()
                && job.status == Status::Failed
                && !no_retry
                && job
                    .plan
                    .as_ref()
                    .is_some_and(|plan| plan.spec.sudo.is_none())
                && result.as_ref().is_ok_and(|result| {
                    !result.ok
                        && result.error.as_ref().is_some_and(|error| {
                            crate::sftp_client::remote_permission_denied(error)
                        })
                });
        let linked_closing = job.scope.as_ref().is_some_and(|scope| {
            scope.closing.load(Ordering::SeqCst) || scope.cancelled.load(Ordering::SeqCst)
        });
        record_history(history.as_ref(), job);
        let terminal = matches!(job.status, Status::Completed | Status::Cancelled)
            || no_retry
            || (job.status == Status::Failed
                && !job.permission_retry
                && (!job.pausable
                    || !view
                    || draining
                    || linked_closing
                    || job
                        .plan
                        .as_ref()
                        .and_then(|plan| plan.spec.sudo.as_ref())
                        .is_some_and(|sudo| sudo.single_use())));
        if terminal {
            job.plan = None;
            if let Some(reply) = job.reply.take() {
                reply.finish(result);
            }
        }
        Self::pump(inner, &mut state);
        let _ = inner.changes.try_send(());
    }
    pub fn pause_toggle(&self, id: u64) {
        let mut state = self.0.state.lock().unwrap();
        if let Some(job) = state.jobs.iter_mut().find(|job| job.id == id) {
            if job.permission_retry {
                return;
            }
            if job
                .scope
                .as_ref()
                .is_some_and(|scope| scope.closing.load(Ordering::SeqCst))
            {
                return;
            }
            match job.status {
                Status::Queued => job.status = Status::Paused,
                Status::Running if job.pausable => {
                    job.status = Status::Pausing;
                    if let Some(control) = &job.control {
                        control.cancel();
                    }
                }
                Status::Paused | Status::Failed if job.pausable && job.plan.is_some() => {
                    job.status = Status::Queued;
                    job.finished_at = None;
                }
                _ => return,
            }
        }
        Self::pump(&self.0, &mut state);
        self.wake();
    }
    fn cancel_locked(job: &mut Job) {
        if job.status.finished() {
            // Releasing retry resources must preserve the recorded outcome.
            job.plan = None;
            if let Some(reply) = job.reply.take() {
                reply.finish(Err(anyhow::anyhow!(
                    job.error
                        .clone()
                        .unwrap_or_else(|| "Transfer has ended.".into())
                )));
            }
            return;
        }
        if job.status.running() {
            job.status = Status::Cancelling;
            if let Some(control) = &job.control {
                control.cancel();
            }
        } else if job.plan.is_some() {
            job.status = Status::Cancelled;
            job.plan = None;
            job.error = Some("Transfer cancelled. Remote partial data may remain.".into());
            if let Some(reply) = job.reply.take() {
                reply.finish(Err(anyhow::anyhow!(
                    "Remote edit transfer cancelled; the local copy is retained."
                )));
            }
        }
    }
    pub fn cancel(&self, id: u64) {
        let mut state = self.0.state.lock().unwrap();
        let history = state.history.clone();
        if let Some(job) = state.jobs.iter_mut().find(|job| job.id == id) {
            Self::cancel_locked(job);
            record_history(history.as_ref(), job);
        }
        Self::pump(&self.0, &mut state);
        self.wake();
    }
    pub fn dismiss(&self, id: u64) {
        let mut state = self.0.state.lock().unwrap();
        if let Some(job) = state
            .jobs
            .iter_mut()
            .find(|job| job.id == id && job.status.finished())
        {
            Self::cancel_locked(job);
        }
        state
            .jobs
            .retain(|job| job.id != id || job.status.running());
        Self::pump(&self.0, &mut state);
        self.wake();
    }
    pub fn clear_finished(&self) {
        for task in self
            .snapshots()
            .into_iter()
            .filter(|job| job.status.finished())
        {
            self.dismiss(task.id);
        }
    }
    pub fn detach_view(&self) {
        self.drain(false);
    }
    fn drain(&self, close: bool) {
        let mut state = self.0.state.lock().unwrap();
        let history = state.history.clone();
        state.view = false;
        state.draining = true;
        state.closed |= close;
        for job in &mut state.jobs {
            if close || job.scope.is_none() || !job.status.running() {
                Self::cancel_locked(job);
                record_history(history.as_ref(), job);
            }
        }
        Self::pump(&self.0, &mut state);
        self.wake();
    }
    pub fn close(&self) {
        self.drain(true);
        let workers = std::mem::take(&mut *self.0.workers.lock().unwrap());
        for worker in workers {
            let _ = worker.join();
        }
    }
}
#[derive(Clone)]
pub struct Client {
    service: Service,
    owner: u64,
    scope: Arc<Scope>,
}
impl Client {
    pub fn execute(&self, mut spec: Spec) -> Result<TaskResult> {
        spec.owner = self.owner;
        let reply = Arc::new(Completion::default());
        let id = self
            .service
            .submit(spec, Some(self.scope.clone()), Some(reply.clone()))?;
        Ticket {
            service: self.service.clone(),
            id,
            reply,
            finished: false,
        }
        .wait()
    }
    pub fn activate_edit(&self) {
        self.scope.active_edit.store(true, Ordering::SeqCst);
    }
    pub fn begin_close(&self) {
        self.scope.closing.store(true, Ordering::SeqCst);
        self.cancel_scope(false);
    }
    pub fn cancel(&self) {
        self.scope.cancelled.store(true, Ordering::SeqCst);
        self.cancel_scope(true);
    }
    fn cancel_scope(&self, all: bool) {
        let mut state = self.service.0.state.lock().unwrap();
        let history = state.history.clone();
        for job in &mut state.jobs {
            if job
                .scope
                .as_ref()
                .is_some_and(|scope| scope.id == self.scope.id)
                && (all || !job.status.running())
            {
                Service::cancel_locked(job);
                record_history(history.as_ref(), job);
            }
        }
        Service::pump(&self.service.0, &mut state);
        self.service.wake();
    }
}
struct Ticket {
    service: Service,
    id: u64,
    reply: Arc<Completion>,
    finished: bool,
}
impl Ticket {
    fn wait(mut self) -> Result<TaskResult> {
        let result = {
            let mut result = self.reply.result.lock().unwrap();
            loop {
                if let Some(result) = result.take() {
                    break result;
                }
                result = self.reply.ready.wait(result).unwrap();
            }
        };
        self.finished = true;
        result
    }
}
impl Drop for Ticket {
    fn drop(&mut self) {
        if !self.finished {
            self.service.cancel(self.id);
        }
    }
}
pub fn begin_shutdown() {
    CLOSING.store(true, Ordering::SeqCst);
    begin_window_close();
}
pub fn begin_window_close() {
    let queues: Vec<_> = registry()
        .lock()
        .unwrap()
        .iter()
        .filter_map(Weak::upgrade)
        .collect();
    for queue in queues {
        Service(queue).drain(false);
    }
}
pub fn finish_shutdown() {
    let queues = std::mem::take(&mut *registry().lock().unwrap());
    for queue in queues.into_iter().filter_map(|queue| queue.upgrade()) {
        Service(queue).close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> Spec {
        let options = SshOptions {
            host: "example.test".into(),
            port: 22,
            username: "user".into(),
            known_hosts: "/unused-test-trust".into(),
            trust: Default::default(),
            temporary_trust: None,
            proxy: None,
            encoding: Default::default(),
            environment: vec![],
            startup: Default::default(),
            password: zeroize::Zeroizing::new("test-only-secret".into()),
            private_key: Default::default(),
            certificate: Default::default(),
            passphrase: Default::default(),
            fido2_pin: Default::default(),
        };
        Spec::new(
            1,
            Arc::new(options),
            Operation::Upload {
                local_path: "/local/file".into(),
                remote_path: "/remote/file".into(),
                replace_existing: false,
                transfer_threads: 1,
            },
        )
    }
    fn failed_job(reply: Arc<Completion>) -> Job {
        Job {
            kind: Kind::Upload,
            created_at: 10,
            finished_at: Some(20),
            uuid: "failed".into(),
            history_record: None,
            recorded_outcome: None,
            id: 1,
            owner: 1,
            label: "Upload".into(),
            source: "/local/file".into(),
            target: "/remote/file".into(),
            display_progress: None,
            local_target: None,
            refresh_owner: false,
            pausable: true,
            permission_retry: false,
            status: Status::Failed,
            error: Some("Permission denied".into()),
            plan: Some(Arc::new(Plan {
                spec: spec(),
                download: Mutex::new(None),
            })),
            attempt: 1,
            control: None,
            feed: ProgressFeed::new(),
            reply: Some(reply.clone()),
            scope: None,
        }
    }
    #[test]
    fn deleting_a_record_cannot_be_undone_by_cleanup_but_a_new_attempt_can_save() -> Result<()> {
        #[derive(Default)]
        struct Memory(Mutex<std::collections::HashMap<String, nauterm_core::SftpTaskHistoryEntry>>);
        impl crate::transfer_history::Store for Memory {
            fn load(&self) -> Result<Vec<nauterm_core::SftpTaskHistoryEntry>> {
                Ok(self.0.lock().unwrap().values().cloned().collect())
            }
            fn save(&self, mut entry: nauterm_core::SftpTaskHistoryEntry) -> Result<i64> {
                entry.id = Some(1);
                self.0
                    .lock()
                    .unwrap()
                    .insert(entry.uuid.clone().unwrap(), entry);
                Ok(1)
            }
            fn delete(&self, uuid: String) -> Result<()> {
                self.0.lock().unwrap().remove(&uuid);
                Ok(())
            }
            fn clear(&self) -> Result<()> {
                self.0.lock().unwrap().clear();
                Ok(())
            }
        }
        let store = Arc::new(Memory::default());
        let history = crate::transfer_history::History::new(store.clone())?;
        let mut job = failed_job(Arc::new(Completion::default()));
        job.history_record = crate::transfer_history::entry(
            &crate::transfer_history::Identity {
                uuid: None,
                host: "example.test".into(),
                username: "user".into(),
                port: 22,
            },
            &spec(),
            job.uuid.clone(),
        );
        job.created_at = job.history_record.as_ref().unwrap().created_at;
        job.finished_at = Some(job.created_at);
        record_history(Some(&history), &mut job);
        history.delete(job.uuid.clone())?.recv_blocking()??;
        Service::cancel_locked(&mut job);
        record_history(Some(&history), &mut job);
        history.close();
        assert!(store.0.lock().unwrap().is_empty());
        let next = crate::transfer_history::History::new(store.clone())?;
        job.attempt += 1;
        job.finished_at = None;
        record_history(Some(&next), &mut job);
        next.close();
        assert_eq!(store.0.lock().unwrap().len(), 1);
        Ok(())
    }
    #[test]
    fn closing_a_failed_retry_preserves_its_outcome_and_completes_the_waiter() {
        let service = Service::new(true);
        let reply = Arc::new(Completion::default());
        service
            .0
            .state
            .lock()
            .unwrap()
            .jobs
            .push(failed_job(reply.clone()));
        service.drain(true);
        let snapshots = service.snapshots();
        assert_eq!(snapshots[0].status, Status::Failed);
        assert_eq!(snapshots[0].finished_at, Some(20));
        assert!(!snapshots[0].resumable);
        assert_eq!(
            reply
                .result
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .unwrap_err()
                .to_string(),
            "Permission denied"
        );
    }
    #[test]
    fn remote_edit_history_keeps_its_type_and_excludes_authentication_material() {
        let mut spec = spec();
        spec.kind = Some(Kind::Edit);
        let entry = crate::transfer_history::entry(
            &crate::transfer_history::Identity {
                uuid: None,
                host: "example.test".into(),
                username: "user".into(),
                port: 22,
            },
            &spec,
            "edit-task".into(),
        )
        .unwrap();
        assert_eq!(entry.task_type, "edit");
        assert_eq!(entry.target_path, "/remote/file");
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("test-only-secret"));
        assert!(!json.contains("password"));
        assert!(!json.contains("private_key"));
    }
}
