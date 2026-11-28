//! Runtime ownership tree: workspace tab → splits → panes → session tabs.
//! No terminal handles or authentication data are serialized by this model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

pub trait SessionId {
    fn session_id(&self) -> u64;
}
pub struct Pane<T> {
    pub id: u64,
    pub sessions: Vec<T>,
    pub composer_visible: bool,
    selected: u64,
}
impl<T: SessionId> Pane<T> {
    pub fn restored(
        id: u64,
        sessions: Vec<T>,
        selected: u64,
        composer_visible: bool,
    ) -> Option<Self> {
        if sessions.is_empty() {
            return None;
        }
        let selected = sessions
            .iter()
            .find(|item| item.session_id() == selected)
            .unwrap_or(&sessions[0])
            .session_id();
        Some(Self {
            id,
            sessions,
            selected,
            composer_visible,
        })
    }
    fn new(id: u64, session: T) -> Self {
        Self {
            id,
            selected: session.session_id(),
            sessions: vec![session],
            composer_visible: true,
        }
    }
    pub fn active(&self) -> &T {
        self.sessions
            .iter()
            .find(|item| item.session_id() == self.selected)
            .unwrap_or(&self.sessions[0])
    }
}
pub struct Split<T> {
    pub id: u64,
    pub axis: SplitAxis,
    pub children: Vec<Node<T>>,
    pub fractions: Vec<f32>,
}
pub enum Node<T> {
    Pane(Pane<T>),
    Split(Split<T>),
}
impl<T: SessionId> Node<T> {
    fn panes<'a>(&'a self, result: &mut Vec<&'a Pane<T>>) {
        match self {
            Self::Pane(pane) => result.push(pane),
            Self::Split(split) => split.children.iter().for_each(|child| child.panes(result)),
        }
    }
    fn pane_mut(&mut self, id: u64) -> Option<&mut Pane<T>> {
        match self {
            Self::Pane(pane) => (pane.id == id).then_some(pane),
            Self::Split(split) => split
                .children
                .iter_mut()
                .find_map(|child| child.pane_mut(id)),
        }
    }
    pub fn split(&self, id: u64) -> Option<&Split<T>> {
        match self {
            Self::Pane(_) => None,
            Self::Split(split) if split.id == id => Some(split),
            Self::Split(split) => split.children.iter().find_map(|child| child.split(id)),
        }
    }
    fn split_mut(&mut self, id: u64) -> Option<&mut Split<T>> {
        match self {
            Self::Pane(_) => None,
            Self::Split(split) => {
                if split.id == id {
                    Some(split)
                } else {
                    split
                        .children
                        .iter_mut()
                        .find_map(|child| child.split_mut(id))
                }
            }
        }
    }
    fn insert_split(
        self,
        target: u64,
        pane: &mut Option<Pane<T>>,
        axis: SplitAxis,
        id: u64,
    ) -> Self {
        match self {
            Self::Pane(old) if old.id == target => {
                let mut new = pane.take().expect("target validated");
                new.composer_visible = old.composer_visible;
                Self::Split(Split {
                    id,
                    axis,
                    children: vec![Self::Pane(old), Self::Pane(new)],
                    fractions: vec![0.5, 0.5],
                })
            }
            Self::Split(mut split) => {
                split.children = split
                    .children
                    .into_iter()
                    .map(|child| {
                        if pane.is_some() {
                            child.insert_split(target, pane, axis, id)
                        } else {
                            child
                        }
                    })
                    .collect();
                Self::Split(split)
            }
            other => other,
        }
    }
    fn remove(self, id: u64, removed: &mut Option<T>) -> Option<Self> {
        match self {
            Self::Pane(mut pane) => {
                if let Some(index) = pane
                    .sessions
                    .iter()
                    .position(|item| item.session_id() == id)
                {
                    *removed = Some(pane.sessions.remove(index));
                    if pane.sessions.is_empty() {
                        return None;
                    }
                    if pane.selected == id {
                        pane.selected =
                            pane.sessions[index.min(pane.sessions.len() - 1)].session_id();
                    }
                }
                Some(Self::Pane(pane))
            }
            Self::Split(mut split) => {
                let count = split.children.len();
                split.children = split
                    .children
                    .into_iter()
                    .filter_map(|child| child.remove(id, removed))
                    .collect();
                if split.children.len() == 1 {
                    return split.children.pop();
                }
                if split.children.is_empty() {
                    return None;
                }
                if count != split.children.len() {
                    split.fractions = vec![1. / split.children.len() as f32; split.children.len()];
                }
                Some(Self::Split(split))
            }
        }
    }
    pub fn min_height(&self, leaf_height: f32) -> f32 {
        match self {
            Self::Pane(_) => leaf_height,
            Self::Split(split) => {
                let heights = split
                    .children
                    .iter()
                    .map(|child| child.min_height(leaf_height));
                if split.axis == SplitAxis::Vertical {
                    heights.sum::<f32>() + 5. * split.children.len().saturating_sub(1) as f32
                } else {
                    heights.fold(leaf_height, f32::max)
                }
            }
        }
    }
}

pub struct PaneTree<T> {
    root: Option<Node<T>>,
    selected: u64,
}
impl<T: SessionId> PaneTree<T> {
    pub fn from_root(root: Node<T>, selected: u64) -> Self {
        let mut panes = vec![];
        root.panes(&mut panes);
        let selected = panes
            .iter()
            .find(|pane| pane.id == selected)
            .unwrap_or(&panes[0])
            .id;
        Self {
            root: Some(root),
            selected,
        }
    }
    #[cfg(test)]
    pub fn new(pane_id: u64, session: T) -> Self {
        Self::new_with_composer(pane_id, session, true)
    }
    pub fn new_with_composer(pane_id: u64, session: T, composer_visible: bool) -> Self {
        let mut pane = Pane::new(pane_id, session);
        pane.composer_visible = composer_visible;
        Self {
            root: Some(Node::Pane(pane)),
            selected: pane_id,
        }
    }
    pub fn root(&self) -> Option<&Node<T>> {
        self.root.as_ref()
    }
    pub fn panes(&self) -> Vec<&Pane<T>> {
        let mut result = vec![];
        if let Some(root) = &self.root {
            root.panes(&mut result);
        }
        result
    }
    pub fn sessions(&self) -> impl Iterator<Item = &T> {
        self.panes().into_iter().flat_map(|pane| &pane.sessions)
    }
    pub fn session(&self, id: u64) -> Option<&T> {
        self.sessions().find(|item| item.session_id() == id)
    }
    pub fn session_mut(&mut self, id: u64) -> Option<&mut T> {
        let pane = self
            .panes()
            .into_iter()
            .find(|pane| pane.sessions.iter().any(|item| item.session_id() == id))?
            .id;
        self.root
            .as_mut()?
            .pane_mut(pane)?
            .sessions
            .iter_mut()
            .find(|item| item.session_id() == id)
    }
    pub fn selected_pane(&self) -> u64 {
        self.selected
    }
    pub fn toggle_composer(&mut self, pane: u64) {
        if let Some(pane) = self.root.as_mut().and_then(|root| root.pane_mut(pane)) {
            pane.composer_visible = !pane.composer_visible;
        }
    }
    pub fn active(&self) -> Option<&T> {
        self.panes()
            .into_iter()
            .find(|pane| pane.id == self.selected)
            .map(Pane::active)
    }
    pub fn select_pane(&mut self, id: u64) -> bool {
        if self.panes().iter().any(|pane| pane.id == id) {
            self.selected = id;
            true
        } else {
            false
        }
    }
    pub fn select_session(&mut self, id: u64) -> bool {
        let Some(pane) = self
            .panes()
            .into_iter()
            .find(|pane| pane.sessions.iter().any(|item| item.session_id() == id))
            .map(|pane| pane.id)
        else {
            return false;
        };
        self.root.as_mut().unwrap().pane_mut(pane).unwrap().selected = id;
        self.selected = pane;
        true
    }
    pub fn insert_session(&mut self, target: u64, session: T) -> Result<(), T> {
        if self.session(session.session_id()).is_some() {
            return Err(session);
        }
        let Some(pane) = self.root.as_mut().and_then(|root| root.pane_mut(target)) else {
            return Err(session);
        };
        pane.selected = session.session_id();
        pane.sessions.push(session);
        self.selected = target;
        Ok(())
    }
    pub fn insert_split(
        &mut self,
        target: u64,
        new_pane: u64,
        session: T,
        axis: SplitAxis,
        id: u64,
    ) -> Result<(), T> {
        if !self.panes().iter().any(|pane| pane.id == target)
            || self.panes().iter().any(|pane| pane.id == new_pane)
            || self.session(session.session_id()).is_some()
            || self
                .root
                .as_ref()
                .is_some_and(|root| root.split(id).is_some())
        {
            return Err(session);
        }
        let root = self.root.take().expect("target validated");
        self.root =
            Some(root.insert_split(target, &mut Some(Pane::new(new_pane, session)), axis, id));
        self.selected = new_pane;
        Ok(())
    }
    pub fn remove_session(&mut self, id: u64) -> Option<T> {
        let root = self.root.take()?;
        let mut removed = None;
        self.root = root.remove(id, &mut removed);
        if !self.panes().iter().any(|pane| pane.id == self.selected)
            && let Some(first) = self.panes().first()
        {
            self.selected = first.id;
        }
        removed
    }
    pub fn resize(&mut self, split: u64, boundary: usize, delta: f32, extent: f32) -> bool {
        if !extent.is_finite() || extent <= 0. || !delta.is_finite() {
            return false;
        }
        let Some(split) = self.root.as_mut().and_then(|root| root.split_mut(split)) else {
            return false;
        };
        if boundary + 1 >= split.fractions.len() {
            return false;
        }
        let min = 0.18_f32.min(1. / (split.children.len() * 2) as f32);
        let left = split.fractions[boundary];
        let right = split.fractions[boundary + 1];
        let change = (delta / extent).clamp(min - left, right - min);
        if change == 0. {
            return false;
        }
        split.fractions[boundary] += change;
        split.fractions[boundary + 1] -= change;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, PartialEq, Eq)]
    struct Item(u64);
    impl SessionId for Item {
        fn session_id(&self) -> u64 {
            self.0
        }
    }
    #[test]
    fn nested_splits_inner_tabs_selection_and_collapse_preserve_ownership() {
        let mut tree = PaneTree::new(1, Item(1));
        tree.insert_split(1, 2, Item(2), SplitAxis::Horizontal, 10)
            .unwrap();
        tree.insert_split(2, 3, Item(3), SplitAxis::Vertical, 11)
            .unwrap();
        tree.insert_session(3, Item(4)).unwrap();
        assert_eq!(tree.active(), Some(&Item(4)));
        tree.select_session(3);
        assert_eq!(tree.active(), Some(&Item(3)));
        assert_eq!(tree.root().unwrap().min_height(280.), 565.);
        assert_eq!(tree.remove_session(3), Some(Item(3)));
        assert_eq!(tree.active(), Some(&Item(4)));
        assert_eq!(tree.remove_session(4), Some(Item(4)));
        assert!(tree.root().unwrap().split(11).is_none());
        assert_eq!(tree.active(), Some(&Item(1)));
        assert!(tree.select_pane(2));
        assert_eq!(tree.active(), Some(&Item(2)));
        assert_eq!(tree.remove_session(1), Some(Item(1)));
        assert!(matches!(tree.root(), Some(Node::Pane(_))));
        assert_eq!(tree.active(), Some(&Item(2)));
        assert_eq!(tree.remove_session(2), Some(Item(2)));
        assert!(tree.root().is_none());
    }
    #[test]
    fn resizing_is_scoped_clamped_and_does_not_recreate_sessions() {
        let mut tree = PaneTree::new(1, Item(1));
        tree.insert_split(1, 2, Item(2), SplitAxis::Horizontal, 10)
            .unwrap();
        tree.insert_split(2, 3, Item(3), SplitAxis::Vertical, 11)
            .unwrap();
        assert!(tree.resize(10, 0, 2000., 1000.));
        let root = tree.root().unwrap();
        assert_eq!(root.split(10).unwrap().fractions, [0.82, 0.18]);
        assert_eq!(root.split(11).unwrap().fractions, [0.5, 0.5]);
        assert!(!tree.resize(11, 0, f32::NAN, 1000.));
        assert!(!tree.resize(11, 0, 10., 0.));
        assert!(!tree.resize(11, 1, 10., 1000.));
        assert_eq!(
            tree.sessions()
                .map(SessionId::session_id)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }
    #[test]
    fn stale_or_duplicate_insertions_return_the_unpublished_session() {
        let mut tree = PaneTree::new(1, Item(1));
        assert_eq!(tree.insert_session(99, Item(2)), Err(Item(2)));
        assert_eq!(tree.insert_session(1, Item(1)), Err(Item(1)));
        assert_eq!(
            tree.insert_split(1, 1, Item(2), SplitAxis::Horizontal, 10),
            Err(Item(2))
        );
        assert_eq!(tree.remove_session(99), None);
        assert_eq!(tree.active(), Some(&Item(1)));
    }
    #[test]
    fn composer_visibility_belongs_to_the_pane_and_is_inherited_by_its_split() {
        let mut tree = PaneTree::new_with_composer(1, Item(1), false);
        tree.insert_session(1, Item(2)).unwrap();
        assert!(!tree.panes()[0].composer_visible);
        tree.insert_split(1, 3, Item(3), SplitAxis::Horizontal, 10)
            .unwrap();
        assert!(tree.panes().iter().all(|pane| !pane.composer_visible));
        tree.toggle_composer(3);
        assert!(!tree.panes()[0].composer_visible);
        assert!(tree.panes()[1].composer_visible);
    }
    #[test]
    fn removing_one_inner_tab_drops_only_that_resource_not_its_siblings() {
        use std::sync::{Arc, Mutex};
        struct Owned(u64, Arc<Mutex<Vec<u64>>>);
        impl SessionId for Owned {
            fn session_id(&self) -> u64 {
                self.0
            }
        }
        impl Drop for Owned {
            fn drop(&mut self) {
                self.1.lock().unwrap().push(self.0);
            }
        }
        let drops = Arc::new(Mutex::new(vec![]));
        let mut tree = PaneTree::new(1, Owned(1, drops.clone()));
        assert!(tree.insert_session(1, Owned(2, drops.clone())).is_ok());
        assert!(
            tree.insert_split(1, 3, Owned(3, drops.clone()), SplitAxis::Horizontal, 10)
                .is_ok()
        );
        drop(tree.remove_session(2));
        assert_eq!(*drops.lock().unwrap(), [2]);
        drop(tree.remove_session(1));
        assert_eq!(*drops.lock().unwrap(), [2, 1]);
        assert_eq!(tree.active().map(SessionId::session_id), Some(3));
        drop(tree);
        assert_eq!(*drops.lock().unwrap(), [2, 1, 3]);
    }
}
