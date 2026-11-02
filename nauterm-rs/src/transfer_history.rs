//! Public transfer records on a serial worker; completion never needs UI pumping.
use crate::{
    sftp_client::Progress,
    transfer_queue::{Kind, Snapshot, Status},
    vault::Vault,
};
use anyhow::Result;
use nauterm_core::SftpTaskHistoryEntry as Entry;
use std::sync::{Arc, Mutex, OnceLock, Weak};

const RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;
fn cutoff() -> i64 {
    chrono::Utc::now()
        .timestamp_millis()
        .saturating_sub(RETENTION_MS)
}

pub trait Store: Send + Sync + 'static {
    fn load(&self) -> Result<Vec<Entry>>;
    fn save(&self, entry: Entry) -> Result<i64>;
    fn delete(&self, uuid: String) -> Result<()>;
    fn clear(&self) -> Result<()>;
}
impl Store for Vault {
    fn load(&self) -> Result<Vec<Entry>> {
        self.dispatch_blocking(|db| Ok(db.inner.list_sftp_task_history(cutoff())?))
    }
    fn save(&self, entry: Entry) -> Result<i64> {
        self.dispatch_blocking(move |db| Ok(db.inner.save_sftp_task_history(&entry, cutoff())?))
    }
    fn delete(&self, uuid: String) -> Result<()> {
        self.dispatch_blocking(move |db| {
            db.inner.delete_sftp_task_history_by_uuid(&uuid)?;
            Ok(())
        })
    }
    fn clear(&self) -> Result<()> {
        self.dispatch_blocking(|db| {
            db.inner.clear_sftp_task_history()?;
            Ok(())
        })
    }
}
enum Work {
    Save(Box<Entry>),
    Delete(String, async_channel::Sender<Result<()>>),
    Clear(async_channel::Sender<Result<()>>),
    Reload,
}
#[derive(Default)]
struct State {
    rows: Arc<[Entry]>,
    snapshots: Arc<[Snapshot]>,
    error: Option<String>,
}
type Publication = (Arc<[Entry]>, Arc<[Snapshot]>);
fn project(rows: Vec<Entry>, previous: Publication) -> Publication {
    if previous.0.as_ref() == rows.as_slice() {
        return previous;
    }
    let mut snapshots: Vec<_> = rows.iter().filter_map(snapshot).collect();
    snapshots.sort_by(Snapshot::compare);
    (Arc::from(rows), Arc::from(snapshots))
}
pub struct History {
    sender: async_channel::Sender<Work>,
    state: Arc<Mutex<State>>,
    incoming: async_channel::Receiver<()>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}
struct Registered {
    history: Weak<History>,
    state: Weak<Mutex<State>>,
    sender: async_channel::Sender<Work>,
}
fn registry() -> &'static Mutex<Vec<Registered>> {
    static REGISTRY: OnceLock<Mutex<Vec<Registered>>> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}
impl History {
    pub fn new(store: Arc<dyn Store>) -> Result<Arc<Self>> {
        let (sender, queue) = async_channel::unbounded();
        let (changed, incoming) = async_channel::bounded(1);
        let state = Arc::new(Mutex::new(State::default()));
        let output = state.clone();
        let thread = std::thread::Builder::new()
            .name("nauterm-transfer-history".into())
            .spawn(move || {
                let publish = |result: Result<Vec<Entry>>| {
                    // Project large histories on this worker, outside the
                    // state lock and the UI thread. Unchanged reloads retain
                    // the same publication so progress never invalidates it.
                    let result = result.map(|rows| {
                        let previous = {
                            let state = output.lock().unwrap();
                            (state.rows.clone(), state.snapshots.clone())
                        };
                        project(rows, previous)
                    });
                    let mut state = output.lock().unwrap();
                    let (notify, retired) = match result {
                        Ok((rows, snapshots)) => {
                            let notify =
                                !Arc::ptr_eq(&state.snapshots, &snapshots) || state.error.is_some();
                            let retired = (
                                std::mem::replace(&mut state.rows, rows),
                                std::mem::replace(&mut state.snapshots, snapshots),
                            );
                            state.error = None;
                            (notify, Some(retired))
                        }
                        Err(error) => {
                            let error = format!("SFTP task history: {error}");
                            let notify = state.error.as_ref() != Some(&error);
                            state.error = Some(error);
                            (notify, None)
                        }
                    };
                    drop(state);
                    // Release large old inventories without holding the lock
                    // the UI uses to obtain its immutable publication.
                    drop(retired);
                    if notify {
                        let _ = changed.try_send(());
                    }
                };
                publish(store.load());
                while let Ok(work) = queue.recv_blocking() {
                    let changed_store = !matches!(work, Work::Reload);
                    let mut completion = None;
                    let result = match work {
                        Work::Save(entry) => store.save(*entry).map(drop),
                        Work::Delete(uuid, sender) => {
                            completion = Some(sender);
                            store.delete(uuid)
                        }
                        Work::Clear(sender) => {
                            completion = Some(sender);
                            store.clear()
                        }
                        Work::Reload => Ok(()),
                    };
                    let outcome = result
                        .as_ref()
                        .map(|_| ())
                        .map_err(|error| anyhow::anyhow!(error.to_string()));
                    let saved = result.is_ok() && changed_store;
                    publish(result.and_then(|_| store.load()));
                    // The mutation outcome is distinct from a refresh failure:
                    // never undo a committed delete because reloading failed.
                    if let Some(sender) = completion {
                        let _ = sender.try_send(outcome);
                    }
                    if saved {
                        let mut registry = registry().lock().unwrap();
                        registry.retain(|item| item.history.strong_count() > 0);
                        for item in registry.iter() {
                            if !item.state.ptr_eq(&Arc::downgrade(&output)) {
                                let _ = item.sender.try_send(Work::Reload);
                            }
                        }
                    }
                }
            })?;
        let history = Arc::new(Self {
            sender,
            state,
            incoming,
            thread: Mutex::new(Some(thread)),
        });
        let mut registry = registry().lock().unwrap();
        registry.retain(|item| item.history.strong_count() > 0);
        registry.push(Registered {
            history: Arc::downgrade(&history),
            state: Arc::downgrade(&history.state),
            sender: history.sender.clone(),
        });
        Ok(history)
    }
    pub fn reload(&self) {
        let _ = self.sender.try_send(Work::Reload);
    }
    pub fn changes(&self) -> async_channel::Receiver<()> {
        self.incoming.clone()
    }
    pub fn error(&self) -> Option<String> {
        self.state.lock().unwrap().error.clone()
    }
    pub fn save(&self, entry: Entry) -> bool {
        self.sender.try_send(Work::Save(Box::new(entry))).is_ok()
    }
    pub fn delete(&self, uuid: String) -> Result<async_channel::Receiver<Result<()>>> {
        let (sender, receiver) = async_channel::bounded(1);
        self.sender
            .try_send(Work::Delete(uuid, sender))
            .map_err(|_| anyhow::anyhow!("Task history is closing."))?;
        Ok(receiver)
    }
    pub fn clear(&self) -> Result<async_channel::Receiver<Result<()>>> {
        let (sender, receiver) = async_channel::bounded(1);
        self.sender
            .try_send(Work::Clear(sender))
            .map_err(|_| anyhow::anyhow!("Task history is closing."))?;
        Ok(receiver)
    }
    pub fn snapshots(&self) -> Arc<[Snapshot]> {
        self.state.lock().unwrap().snapshots.clone()
    }
    pub(crate) fn close(&self) {
        self.sender.close();
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
}
impl Drop for History {
    fn drop(&mut self) {
        self.close();
    }
}
pub fn prepare_shutdown() {
    let histories: Vec<_> = registry()
        .lock()
        .unwrap()
        .iter()
        .filter_map(|item| item.history.upgrade())
        .collect();
    for history in histories {
        history.close();
    }
}
fn snapshot(row: &Entry) -> Option<Snapshot> {
    let status = match row.status.as_str() {
        "completed" => Status::Completed,
        "failed" => Status::Failed,
        "cancelled" => Status::Cancelled,
        _ => return None,
    };
    let id = row.id.filter(|id| *id > 0)?;
    let kind = Kind::from_wire(&row.task_type)?;
    Some(Snapshot {
        kind,
        created_at: row.created_at,
        finished_at: Some(row.finished_at),
        id: (1 << 63) | id as u64,
        uuid: row.uuid.clone()?,
        history_id: Some(id),
        owner: 0,
        label: kind.label().into(),
        source: row.source_path.clone(),
        target: row.target_path.clone(),
        local_target: (row.task_type == "download").then(|| row.target_path.clone()),
        status,
        pausable: false,
        resumable: false,
        permission_retry: false,
        attempt: 0,
        progress: Progress {
            bytes: row.bytes.max(0) as u64,
            total: row.total_bytes.max(0) as u64,
            path: row.display_name.clone(),
        },
        error: row.error.clone(),
        refresh_owner: false,
    })
}

#[derive(Clone)]
pub struct Identity {
    pub uuid: Option<String>,
    pub host: String,
    pub username: String,
    pub port: u16,
}

pub fn entry(
    identity: &Identity,
    spec: &crate::transfer_queue::Spec,
    uuid: String,
) -> Option<Entry> {
    let kind = spec.kind();
    if kind == Kind::NewFolder {
        return None;
    }
    let source = spec
        .display_source
        .as_deref()
        .unwrap_or_else(|| spec.operation.source());
    let target = spec
        .display_target
        .as_deref()
        .unwrap_or_else(|| spec.operation.target());
    let now = chrono::Utc::now().timestamp_millis();
    Some(Entry {
        id: None,
        uuid: Some(uuid),
        host_uuid: identity.uuid.clone(),
        task_type: kind.wire().into(),
        host: identity.host.clone(),
        username: identity.username.clone(),
        port: i64::from(identity.port),
        status: "completed".into(),
        display_name: std::path::Path::new(if source.is_empty() { target } else { source })
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        source_path: source.into(),
        target_path: target.into(),
        created_at: now,
        finished_at: now,
        bytes: 0,
        total_bytes: 0,
        item_kind: "file".into(),
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Memory(Mutex<Vec<Entry>>);
    impl Store for Memory {
        fn load(&self) -> Result<Vec<Entry>> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn save(&self, mut entry: Entry) -> Result<i64> {
            let mut rows = self.0.lock().unwrap();
            if let Some(row) = rows.iter_mut().find(|row| row.uuid == entry.uuid) {
                entry.id = row.id;
                *row = entry;
                return Ok(row.id.unwrap());
            }
            entry.id = Some(rows.len() as i64 + 1);
            let id = entry.id.unwrap();
            rows.push(entry);
            Ok(id)
        }
        fn delete(&self, uuid: String) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .retain(|row| row.uuid.as_ref() != Some(&uuid));
            Ok(())
        }
        fn clear(&self) -> Result<()> {
            self.0.lock().unwrap().clear();
            Ok(())
        }
    }
    fn row() -> Entry {
        serde_json::from_value(serde_json::json!({
            "uuid": "019b0000-0000-7000-8000-000000000003", "type":"upload",
            "host":"example.test", "username":"user", "port":22, "status":"failed",
            "display_name":"folder", "source_path":"/local/folder", "target_path":"/remote/folder",
            "created_at":1, "finished_at":2, "bytes":3, "total_bytes":10, "item_kind":"directory", "error":"interrupted"
        })).unwrap()
    }
    #[test]
    fn unchanged_reload_reuses_the_projection_and_changes_publish_a_new_snapshot() {
        let mut entry = row();
        entry.id = Some(1);
        let first = project(vec![entry.clone()], (Arc::default(), Arc::default()));
        let old = first.1.clone();
        let same = project(vec![entry.clone()], first);
        assert!(Arc::ptr_eq(&old, &same.1));
        entry.bytes = 9;
        let newer = project(vec![entry], same);
        assert!(!Arc::ptr_eq(&old, &newer.1));
        assert_eq!(old[0].progress.bytes, 3);
        assert_eq!(newer.1[0].progress.bytes, 9);
        let cleared = project(vec![], newer);
        assert!(cleared.1.is_empty());
        assert_eq!(old[0].progress.bytes, 3);
    }
    #[test]
    fn worker_drains_records_and_reloads_read_only_history_without_credentials() -> Result<()> {
        let store = Arc::new(Memory::default());
        let history = History::new(store.clone())?;
        history.save(row());
        let mut finished = row();
        finished.status = "completed".into();
        finished.bytes = 10;
        finished.error = None;
        history.save(finished);
        history.close();
        let snapshots = history.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].status, Status::Completed);
        assert_eq!(snapshots[0].progress.bytes, 10);
        assert_eq!(snapshots[0].created_at, 1);
        assert_eq!(snapshots[0].finished_at, Some(2));
        assert_eq!(snapshots[0].kind, Kind::Upload);
        assert!(
            !snapshots[0].pausable && !snapshots[0].resumable && !snapshots[0].permission_retry
        );
        assert!(history.error().is_none());
        let reopened = History::new(store.clone())?;
        let deleted = reopened.delete(row().uuid.unwrap())?;
        reopened.close();
        assert!(deleted.recv_blocking()?.is_ok());
        assert!(store.load()?.is_empty());
        let clear = History::new(store.clone())?;
        clear.save(row());
        let cleared = clear.clear()?;
        clear.close();
        assert!(cleared.recv_blocking()?.is_ok());
        assert!(store.load()?.is_empty());
        Ok(())
    }
}
