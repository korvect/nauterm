//! Live session ownership is separate from persisted, public workspace metadata.
use crate::model::Id;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopTab {
    Vaults,
    Sftp,
    Workspace,
    Session(usize),
}
pub struct TopTabs {
    pub sftp: bool,
    pub workspace: bool,
}
impl TopTabs {
    pub fn fixed_count(&self) -> usize {
        1 + usize::from(self.sftp) + usize::from(self.workspace)
    }
    pub fn target(&self, index: usize) -> TopTab {
        if index == 0 {
            TopTab::Vaults
        } else if self.sftp && index == 1 {
            TopTab::Sftp
        } else if self.workspace && index == self.fixed_count() - 1 {
            TopTab::Workspace
        } else {
            TopTab::Session(index - self.fixed_count())
        }
    }
    pub fn index(&self, target: TopTab) -> Option<usize> {
        match target {
            TopTab::Vaults => Some(0),
            TopTab::Sftp => self.sftp.then_some(1),
            TopTab::Workspace => self.workspace.then(|| self.fixed_count() - 1),
            TopTab::Session(index) => Some(self.fixed_count() + index),
        }
    }
}

pub trait ScopedSession {
    fn id(&self) -> u64;
    fn workspace(&self) -> Id;
}
pub struct Sessions<T> {
    tabs: Vec<T>,
    selected: BTreeMap<Id, u64>,
}
impl<T> Default for Sessions<T> {
    fn default() -> Self {
        Self {
            tabs: vec![],
            selected: BTreeMap::new(),
        }
    }
}
impl<T: ScopedSession> Sessions<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.tabs.iter()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.tabs.iter_mut()
    }
    pub fn in_workspace(&self, workspace: Id) -> impl Iterator<Item = &T> {
        self.tabs
            .iter()
            .filter(move |tab| tab.workspace() == workspace)
    }
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }
    pub fn push(&mut self, tab: T) {
        assert!(
            !self.tabs.iter().any(|other| other.id() == tab.id()),
            "Session IDs are unique for the app lifetime"
        );
        self.selected.insert(tab.workspace(), tab.id());
        self.tabs.push(tab);
    }
    pub fn selected(&self, workspace: Id) -> Option<u64> {
        self.selected
            .get(&workspace)
            .copied()
            .filter(|id| self.in_workspace(workspace).any(|tab| tab.id() == *id))
            .or_else(|| self.in_workspace(workspace).next().map(ScopedSession::id))
    }
    pub fn select(&mut self, id: u64) -> bool {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id() == id) else {
            return false;
        };
        self.selected.insert(tab.workspace(), id);
        true
    }
    pub fn remove(&mut self, id: u64) -> Option<T> {
        let index = self.tabs.iter().position(|tab| tab.id() == id)?;
        let owner = self.tabs[index].workspace();
        let owner_index = self.tabs[..index]
            .iter()
            .filter(|tab| tab.workspace() == owner)
            .count();
        let tab = self.tabs.remove(index);
        if self.selected.get(&owner) == Some(&id) {
            let siblings: Vec<_> = self.in_workspace(owner).map(ScopedSession::id).collect();
            if let Some(next) = siblings.get(owner_index.min(siblings.len().saturating_sub(1))) {
                self.selected.insert(owner, *next);
            } else {
                self.selected.remove(&owner);
            }
        }
        Some(tab)
    }
    pub fn close_workspace(&mut self, workspace: Id) {
        self.selected.remove(&workspace);
        self.tabs.retain(|tab| tab.workspace() != workspace);
    }
}

pub const SIDEBAR_WIDTH: f32 = 258.;

/// Exact reference monitor geometry; add one tile for New Connect.
pub fn monitor_columns(item_count: usize, width: f32, height: f32) -> usize {
    let limit = item_count
        .min(((width + 10.) / 430.).floor().max(1.) as usize)
        .max(1);
    (1..=limit)
        .min_by(|a, b| {
            let score = |columns: usize| {
                let rows = item_count.div_ceil(columns).max(1);
                let tile_width = (width - 10. * (columns - 1) as f32) / columns as f32;
                let tile_height = (height - 10. * (rows - 1) as f32) / rows as f32;
                (tile_width / tile_height.max(1.) - 1.6).abs() + rows as f32 * 0.03
            };
            score(*a).total_cmp(&score(*b))
        })
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::{ScopedSession, Sessions, TopTab, TopTabs, monitor_columns};
    #[test]
    fn top_tab_indices_follow_enabled_fixed_pages_without_skipping_sessions() {
        for sftp in [false, true] {
            for workspace in [false, true] {
                let tabs = TopTabs { sftp, workspace };
                let mut expected = vec![TopTab::Vaults];
                if sftp {
                    expected.push(TopTab::Sftp);
                }
                if workspace {
                    expected.push(TopTab::Workspace);
                }
                assert_eq!(tabs.fixed_count(), expected.len());
                expected.extend((0..9).map(TopTab::Session));
                for (index, target) in expected.into_iter().enumerate() {
                    assert_eq!(tabs.target(index), target);
                    assert_eq!(tabs.index(target), Some(index));
                }
                assert_eq!(tabs.index(TopTab::Sftp).is_some(), sftp);
                assert_eq!(tabs.index(TopTab::Workspace).is_some(), workspace);
            }
        }
    }
    #[derive(Debug, PartialEq)]
    struct Tab(u64, u64);
    impl ScopedSession for Tab {
        fn id(&self) -> u64 {
            self.0
        }
        fn workspace(&self) -> u64 {
            self.1
        }
    }
    #[test]
    fn switching_and_closing_preserve_other_workspaces_and_selection() {
        let mut sessions = Sessions::default();
        for tab in [Tab(1, 10), Tab(2, 20), Tab(3, 10), Tab(4, 10)] {
            sessions.push(tab);
        }
        assert_eq!(sessions.selected(10), Some(4));
        assert_eq!(sessions.selected(20), Some(2));
        assert!(sessions.select(3));
        assert_eq!(sessions.remove(3), Some(Tab(3, 10)));
        assert_eq!(sessions.selected(10), Some(4));
        sessions.close_workspace(10);
        assert_eq!(sessions.selected(10), None);
        assert_eq!(sessions.selected(20), Some(2));
        assert_eq!(sessions.iter().collect::<Vec<_>>(), vec![&Tab(2, 20)]);
        assert!(!sessions.select(3));
        assert_eq!(sessions.remove(3), None);
    }
    #[test]
    fn background_completion_updates_the_original_tab_not_the_selected_workspace() {
        let mut sessions = Sessions::default();
        sessions.push(Tab(1, 10));
        sessions.push(Tab(2, 20));
        let target = sessions.iter_mut().find(|tab| tab.id() == 1).unwrap();
        assert_eq!(target.workspace(), 10);
        assert_eq!(sessions.selected(20), Some(2));
        sessions.close_workspace(10);
        assert!(
            !sessions.iter_mut().any(|tab| tab.id() == 1),
            "Late results must be dropped before creating a connection"
        );
    }
    #[test]
    fn overview_geometry_matches_the_reference_minimum_width() {
        assert_eq!(monitor_columns(1, 1600., 800.), 1);
        assert_eq!(monitor_columns(4, 840., 700.), 1);
        assert_eq!(monitor_columns(2, 900., 700.), 2);
        assert_eq!(monitor_columns(8, 1700., 900.), 3);
    }
    #[test]
    fn workspace_metadata_rename_selection_and_last_close_survive_json_round_trip()
    -> anyhow::Result<()> {
        let mut state = crate::model::AppState::default();
        let default = state.selected_workspace;
        let second = state.add_workspace("  zsh  ")?;
        assert_eq!(state.workspace().name, "zsh");
        state.rename_workspace(second, "  工作区 QA  ")?;
        assert!(state.rename_workspace(second, "  ").is_err());
        assert_eq!(state.workspace().name, "工作区 QA");
        state.close_workspace(default)?;
        let mut restored: crate::model::AppState =
            serde_json::from_str(&serde_json::to_string(&state)?)?;
        assert_eq!(restored.selected_workspace, second);
        assert_eq!(restored.workspace().name, "工作区 QA");
        restored.close_workspace(second)?;
        assert_ne!(restored.selected_workspace, second);
        assert_eq!(restored.workspace().name, "Default");
        assert!(restored.close_workspace(second).is_err());
        restored.validate()?;
        let json = serde_json::to_string(&restored)?;
        assert!(!json.contains("password") && !json.contains("private_key"));
        Ok(())
    }
    #[test]
    fn closing_one_workspace_drops_only_its_owned_session_resources() {
        use std::sync::{Arc, Mutex};
        struct Owned {
            id: u64,
            owner: u64,
            dropped: Arc<Mutex<Vec<u64>>>,
        }
        impl ScopedSession for Owned {
            fn id(&self) -> u64 {
                self.id
            }
            fn workspace(&self) -> u64 {
                self.owner
            }
        }
        impl Drop for Owned {
            fn drop(&mut self) {
                self.dropped.lock().unwrap().push(self.id);
            }
        }
        let dropped = Arc::new(Mutex::new(vec![]));
        let mut sessions = Sessions::default();
        for (id, owner) in [(1, 10), (2, 20), (3, 10)] {
            sessions.push(Owned {
                id,
                owner,
                dropped: dropped.clone(),
            });
        }
        sessions.close_workspace(10);
        assert_eq!(*dropped.lock().unwrap(), [1, 3]);
        assert_eq!(sessions.selected(20), Some(2));
        drop(sessions);
        assert_eq!(*dropped.lock().unwrap(), [1, 3, 2]);
    }
}
