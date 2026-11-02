//! Live progress replaces only live snapshots. History and its row ordering are
//! retained until metadata that actually affects the list changes.
use super::{Snapshot, presentation};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Copy)]
enum Source {
    Live(usize),
    History(usize),
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Outcome {
    created: i64,
    finished: Option<i64>,
    status: crate::transfer_queue::Status,
    bytes: u64,
    total: u64,
}
impl Outcome {
    fn of(job: &Snapshot) -> Self {
        Self {
            created: job.created_at,
            finished: job.finished_at,
            status: job.status,
            bytes: job.progress.bytes,
            total: job.progress.total,
        }
    }
}
struct Removal {
    versions: HashMap<String, Vec<Outcome>>,
    committed: bool,
}
#[derive(Default)]
pub(super) struct Rows {
    live: Vec<Snapshot>,
    history: Arc<[Snapshot]>,
    order: Vec<Source>,
    queue_positions: HashMap<u64, usize>,
    has_finished: bool,
    removals: HashMap<u64, Removal>,
    dirty: bool,
}
fn same_order(a: &Snapshot, b: &Snapshot) -> bool {
    a.id == b.id
        && a.uuid == b.uuid
        && a.status == b.status
        && a.created_at == b.created_at
        && a.finished_at == b.finished_at
        && a.history_id == b.history_id
}
impl Rows {
    /// None means there is no new history publication (also the initial empty
    /// state when storage is unavailable). Never clone historical row strings.
    pub fn update(&mut self, live: Vec<Snapshot>, history: Option<Arc<[Snapshot]>>) {
        let history = history.unwrap_or_else(|| self.history.clone());
        let history_changed = !Arc::ptr_eq(&history, &self.history);
        let changed = self.dirty
            || history_changed
            || live.len() != self.live.len()
            || live.iter().zip(&self.live).any(|(a, b)| !same_order(a, b));
        self.live = live;
        self.history = history;
        if !changed {
            return;
        }
        self.retire_committed();
        self.dirty = false;
        self.queue_positions = presentation::queue_positions(&self.live);
        let live_ids: HashSet<_> = self.live.iter().map(|job| job.uuid.as_str()).collect();
        let mut order: Vec<_> = self
            .live
            .iter()
            .enumerate()
            .filter(|(_, job)| !self.hidden(job))
            .map(|(index, _)| Source::Live(index))
            .chain(
                self.history
                    .iter()
                    .enumerate()
                    .filter(|(_, job)| !live_ids.contains(job.uuid.as_str()) && !self.hidden(job))
                    .map(|(index, _)| Source::History(index)),
            )
            .collect();
        order.sort_by(|a, b| self.source(*a).compare(self.source(*b)));
        self.has_finished = order
            .iter()
            .any(|source| self.source(*source).status.finished());
        self.order = order;
    }
    /// Masks are memory-only. A pending removal survives intermediate/stale
    /// publications; a later retry with a different outcome stays visible.
    pub fn hide_finished(&mut self, token: u64, uuid: Option<&str>) {
        let mut versions: HashMap<String, Vec<Outcome>> = HashMap::new();
        for job in self
            .live
            .iter()
            .chain(self.history.iter())
            .filter(|job| job.status.finished() && uuid.is_none_or(|uuid| uuid == job.uuid))
        {
            let values = versions.entry(job.uuid.clone()).or_default();
            let outcome = Outcome::of(job);
            if !values.contains(&outcome) {
                values.push(outcome);
            }
        }
        self.removals.insert(
            token,
            Removal {
                versions,
                committed: false,
            },
        );
        self.dirty = true;
    }
    pub fn settle_removal(&mut self, token: u64, committed: bool) {
        if committed {
            if let Some(removal) = self.removals.get_mut(&token) {
                removal.committed = true;
            }
        } else {
            self.removals.remove(&token);
        }
        self.dirty = true;
    }
    fn hidden(&self, job: &Snapshot) -> bool {
        self.removals.values().any(|removal| {
            removal
                .versions
                .get(&job.uuid)
                .is_some_and(|versions| versions.contains(&Outcome::of(job)))
        })
    }
    fn retire_committed(&mut self) {
        if !self.removals.values().any(|removal| removal.committed) {
            return;
        }
        let current: HashMap<_, _> = self
            .history
            .iter()
            .map(|job| (job.uuid.as_str(), Outcome::of(job)))
            .collect();
        self.removals.retain(|_, removal| {
            if removal.committed {
                removal.versions.retain(|uuid, versions| {
                    current
                        .get(uuid.as_str())
                        .is_some_and(|version| versions.contains(version))
                });
            }
            !removal.versions.is_empty()
        });
    }
    fn source(&self, source: Source) -> &Snapshot {
        match source {
            Source::Live(index) => &self.live[index],
            Source::History(index) => &self.history[index],
        }
    }
    pub fn get(&self, index: usize) -> Option<&Snapshot> {
        self.order.get(index).map(|source| self.source(*source))
    }
    pub fn live(&self) -> &[Snapshot] {
        &self.live
    }
    pub fn len(&self) -> usize {
        self.order.len()
    }
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
    pub fn has_finished(&self) -> bool {
        self.has_finished
    }
    pub fn find(&self, id: u64) -> Option<&Snapshot> {
        self.order
            .iter()
            .map(|source| self.source(*source))
            .find(|job| job.id == id)
    }
    pub fn queue_position(&self, id: u64) -> Option<usize> {
        self.queue_positions.get(&id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer_queue::{Kind, Status};
    fn job(id: u64, status: Status) -> Snapshot {
        Snapshot {
            id,
            uuid: format!("task-{id}"),
            history_id: None,
            owner: 0,
            kind: Kind::Upload,
            created_at: id as i64,
            finished_at: status.finished().then_some(id as i64 + 10),
            label: "Upload".into(),
            source: format!("/local/{id}"),
            target: format!("/remote/{id}"),
            local_target: None,
            status,
            pausable: false,
            resumable: false,
            permission_retry: false,
            attempt: 1,
            progress: Default::default(),
            error: None,
            refresh_owner: false,
        }
    }
    #[test]
    fn pending_delete_survives_intermediate_snapshots_and_rolls_back_on_failure() {
        let mut rows = Rows::default();
        rows.update(vec![], Some(vec![job(1, Status::Completed)].into()));
        rows.hide_finished(1, Some("task-1"));
        rows.update(vec![], None);
        assert!(rows.is_empty());
        rows.update(vec![], Some(Arc::default()));
        // A save queued ahead of Delete can publish an old result again.
        rows.update(vec![], Some(vec![job(1, Status::Completed)].into()));
        assert!(rows.is_empty());
        rows.settle_removal(1, false);
        rows.update(vec![], None);
        assert!(rows.find(1).is_some());
    }
    #[test]
    fn overlapping_clear_and_delete_do_not_unhide_each_other_or_hide_new_outcomes() {
        let mut rows = Rows::default();
        rows.update(
            vec![],
            Some(vec![job(1, Status::Completed), job(2, Status::Completed)].into()),
        );
        rows.hide_finished(1, Some("task-1"));
        rows.hide_finished(2, None);
        rows.settle_removal(1, false);
        rows.update(vec![], None);
        assert!(rows.is_empty());
        // Clear committed, but a failed reload leaves the old publication.
        rows.settle_removal(2, true);
        rows.update(vec![job(3, Status::Running)], None);
        assert_eq!(rows.len(), 1);
        let mut retry = job(1, Status::Completed);
        retry.finished_at = Some(100);
        rows.update(
            vec![job(3, Status::Running)],
            Some(vec![retry, job(2, Status::Completed)].into()),
        );
        assert_eq!(rows.len(), 2);
        assert!(rows.find(1).is_some());
        assert!(rows.find(2).is_none());
        rows.update(vec![job(3, Status::Running)], Some(Arc::default()));
        assert!(rows.removals.is_empty());
        assert_eq!(rows.len(), 1);
    }
    #[test]
    fn progress_does_not_rebuild_history_or_order_but_updates_the_visible_row() {
        let history: Arc<[Snapshot]> = (1..=10_000)
            .map(|id| job(id, Status::Completed))
            .collect::<Vec<_>>()
            .into();
        let mut rows = Rows::default();
        rows.update(vec![job(10_001, Status::Running)], Some(history.clone()));
        let order = rows.order.as_ptr();
        let path = rows.find(5000).unwrap().source.as_ptr();
        let mut live = job(10_001, Status::Running);
        live.progress.bytes = 123;
        rows.update(vec![live], Some(history.clone()));
        assert_eq!(rows.order.as_ptr(), order);
        assert!(Arc::ptr_eq(&rows.history, &history));
        assert_eq!(rows.find(5000).unwrap().source.as_ptr(), path);
        assert_eq!(rows.get(0).unwrap().progress.bytes, 123);
        assert_eq!(rows.len(), 10_001);
    }
    #[test]
    fn status_and_publication_changes_rebuild_and_deduplicate_by_task_identity() {
        let history: Arc<[Snapshot]> =
            vec![job(1, Status::Failed), job(2, Status::Completed)].into();
        let mut rows = Rows::default();
        rows.update(vec![job(1, Status::Running)], Some(history));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.get(0).unwrap().status, Status::Running);
        assert_eq!(rows.queue_position(1), None);
        rows.update(vec![job(1, Status::Queued)], None);
        assert_eq!(rows.queue_position(1), Some(1));
        assert_eq!(rows.get(0).unwrap().status, Status::Queued);
        // Native acknowledgement of Delete/Clear replaces the publication;
        // progress-only updates cannot bring its removed records back.
        rows.update(vec![], Some(Arc::default()));
        assert!(rows.is_empty());
        assert!(!rows.has_finished());
        rows.update(vec![job(3, Status::Running)], None);
        assert_eq!(rows.len(), 1);
        assert!(!rows.has_finished());
    }
}
