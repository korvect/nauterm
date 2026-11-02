//! Ordered card selection shared by the workspace collections. IDs, not grid
//! indexes, retain the cursor/anchor when search, sorting or width changes.
use gpui_kit::{Keystroke, Modifiers};
use std::{cell::Cell, collections::BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}
#[derive(Clone, Copy)]
pub enum Key {
    Move(Direction),
    Activate,
}
impl Key {
    pub fn read(key: &Keystroke) -> Option<Self> {
        if key.modifiers.alt {
            return None;
        }
        Some(match key.key.as_str() {
            "left" => Self::Move(Direction::Left),
            "right" => Self::Move(Direction::Right),
            "up" => Self::Move(Direction::Up),
            "down" => Self::Move(Direction::Down),
            "enter" if key.modifiers == Modifiers::default() => Self::Activate,
            _ => return None,
        })
    }
}

pub struct Section<K> {
    pub items: Vec<K>,
    pub columns: usize,
}
impl<K> Section<K> {
    pub fn new(items: Vec<K>, columns: usize) -> Self {
        Self {
            items,
            columns: columns.max(1),
        }
    }
}

pub struct Navigation<K: Copy> {
    active: Option<K>,
    anchor: Option<K>,
    reveal: Cell<Option<K>>,
}
impl<K: Copy> Default for Navigation<K> {
    fn default() -> Self {
        Self {
            active: None,
            anchor: None,
            reveal: Cell::new(None),
        }
    }
}
impl<K: Copy + Ord> Navigation<K> {
    pub fn reset(&mut self) {
        self.active = None;
        self.anchor = None;
        self.reveal.set(None);
    }

    pub fn active(&self, selected: &BTreeSet<K>, sections: &[Section<K>]) -> Option<K> {
        let visible = |key: &K| sections.iter().any(|section| section.items.contains(key));
        self.active
            .filter(|key| selected.contains(key) && visible(key))
            .or_else(|| {
                sections
                    .iter()
                    .flat_map(|section| &section.items)
                    .find(|key| selected.contains(key))
                    .copied()
            })
    }

    pub fn select(
        &mut self,
        selected: &mut BTreeSet<K>,
        visible: &[K],
        target: K,
        modifiers: Modifiers,
    ) {
        self.reveal.set(None);
        if !visible.contains(&target) {
            return;
        }
        if !visible.iter().any(|key| selected.contains(key)) {
            selected.clear();
            self.reset();
        } else if self
            .active
            .is_none_or(|key| !selected.contains(&key) || !visible.contains(&key))
        {
            // Edits/context actions can select an item without a pointer click.
            // Treat that selection as the next range's origin too.
            self.active = visible.iter().find(|key| selected.contains(key)).copied();
            self.anchor = self.active;
        } else if selected.len() == 1 {
            self.anchor = self.active;
        }
        let toggle = modifiers.platform || modifiers.control;
        if modifiers.shift
            && let Some(anchor) = self.anchor.filter(|key| visible.contains(key))
        {
            let start = visible.iter().position(|key| *key == anchor).unwrap();
            let end = visible.iter().position(|key| *key == target).unwrap();
            if !toggle {
                selected.clear();
            }
            selected.extend(&visible[start.min(end)..=start.max(end)]);
            self.active = Some(target);
            return;
        }
        if toggle {
            if !selected.remove(&target) {
                selected.insert(target);
            }
            self.active = selected.contains(&target).then_some(target).or_else(|| {
                visible
                    .iter()
                    .rev()
                    .find(|key| selected.contains(key))
                    .copied()
            });
        } else {
            *selected = BTreeSet::from([target]);
            self.active = Some(target);
        }
        self.anchor = self.active;
    }

    pub fn navigate(
        &mut self,
        selected: &mut BTreeSet<K>,
        sections: &[Section<K>],
        direction: Direction,
        modifiers: Modifiers,
    ) -> Option<K> {
        let current = self.active(selected, sections)?;
        let (section_index, index) =
            sections.iter().enumerate().find_map(|(section, values)| {
                values
                    .items
                    .iter()
                    .position(|key| *key == current)
                    .map(|index| (section, index))
            })?;
        let section = &sections[section_index];
        let length = section.items.len();
        let last_row = (length - 1) / section.columns * section.columns;
        let candidate = match direction {
            Direction::Left => index.saturating_sub(1),
            Direction::Right => (index + 1).min(length - 1),
            Direction::Up => index.checked_sub(section.columns).unwrap_or(index),
            Direction::Down if index + section.columns < length => index + section.columns,
            Direction::Down if index < last_row => length - 1,
            Direction::Down => index,
        };
        let (visible, target, modifiers) = if candidate != index {
            if modifiers.shift && self.anchor.is_none() {
                self.anchor = Some(current);
                self.active = Some(current);
            }
            (&section.items, section.items[candidate], modifiers)
        } else {
            let adjacent = match direction {
                Direction::Left | Direction::Up => sections[..section_index]
                    .iter()
                    .rev()
                    .find(|section| !section.items.is_empty())
                    .map(|section| (section, *section.items.last().unwrap())),
                Direction::Right | Direction::Down => sections[section_index + 1..]
                    .iter()
                    .find(|section| !section.items.is_empty())
                    .map(|section| (section, section.items[0])),
            };
            let Some((section, target)) = adjacent else {
                return Some(current);
            };
            // Collection boundaries never extend a range into another type.
            self.reset();
            (&section.items, target, Modifiers::default())
        };
        self.select(selected, visible, target, modifiers);
        self.reveal.set(Some(target));
        Some(target)
    }

    /// Only keyboard navigation requests scrolling. Pointer selection and
    /// ordinary rerenders must not pull a manually scrolled list back.
    pub fn take_reveal(&self, key: K) -> bool {
        if self.reveal.get() == Some(key) {
            self.reveal.set(None);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ragged_rows_and_section_boundaries_follow_the_current_grid() {
        let mut navigation = Navigation::default();
        let sections = [
            Section::new((10..18).collect::<Vec<_>>(), 3),
            Section::new(vec![20, 21], 3),
        ];
        let mut selected = BTreeSet::new();
        navigation.select(&mut selected, &sections[0].items, 15, Modifiers::default());
        assert_eq!(
            navigation.navigate(
                &mut selected,
                &sections,
                Direction::Down,
                Modifiers::default()
            ),
            Some(17)
        );
        // The short last row clamps to its final card, then the next section.
        assert_eq!(selected, BTreeSet::from([17]));
        assert_eq!(
            navigation.navigate(
                &mut selected,
                &sections,
                Direction::Right,
                Modifiers {
                    shift: true,
                    ..Default::default()
                }
            ),
            Some(20)
        );
        assert_eq!(selected, BTreeSet::from([20]));
        assert_eq!(
            navigation.navigate(
                &mut selected,
                &sections,
                Direction::Up,
                Modifiers::default()
            ),
            Some(17)
        );
        // A resize/list switch changes the stride without losing item identity.
        let list = [Section::new((10..18).collect::<Vec<_>>(), 1)];
        assert_eq!(
            navigation.navigate(&mut selected, &list, Direction::Up, Modifiers::default()),
            Some(16)
        );
    }

    #[test]
    fn ranges_shrink_and_union_in_visible_order_with_stable_identity_anchors() {
        let mut navigation = Navigation::default();
        let visible = [7, 2, 9, 1];
        let mut selected = BTreeSet::new();
        navigation.select(&mut selected, &visible, 2, Modifiers::default());
        let range = Modifiers {
            shift: true,
            ..Default::default()
        };
        navigation.select(&mut selected, &visible, 1, range);
        assert_eq!(selected, BTreeSet::from([2, 9, 1]));
        navigation.select(&mut selected, &visible, 7, range);
        assert_eq!(selected, BTreeSet::from([7, 2]));
        navigation.select(
            &mut selected,
            &visible,
            9,
            Modifiers {
                control: true,
                ..range
            },
        );
        assert_eq!(selected, BTreeSet::from([7, 2, 9]));
        // Reordering must retain ID 2 as the anchor, rather than index 1.
        navigation.select(&mut selected, &[1, 9, 2, 7], 7, range);
        assert_eq!(selected, BTreeSet::from([2, 7]));
        navigation.select(
            &mut selected,
            &[1, 9, 2, 7],
            7,
            Modifiers {
                platform: true,
                ..Default::default()
            },
        );
        assert_eq!(selected, BTreeSet::from([2]));
    }

    #[test]
    fn empty_or_hidden_selection_never_picks_a_default_and_reveal_is_one_shot() {
        let mut navigation = Navigation::default();
        let mut selected = BTreeSet::new();
        let sections = [Section::new(vec![9, 4, 8], 2)];
        assert_eq!(
            navigation.navigate(
                &mut selected,
                &sections,
                Direction::Right,
                Modifiers::default()
            ),
            None
        );
        navigation.select(&mut selected, &sections[0].items, 9, Modifiers::default());
        assert!(!navigation.take_reveal(9));
        navigation.navigate(
            &mut selected,
            &sections,
            Direction::Right,
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(selected, BTreeSet::from([9, 4]));
        assert!(!navigation.take_reveal(9));
        assert!(navigation.take_reveal(4));
        assert!(!navigation.take_reveal(4));
        let filtered = [Section::new(vec![8], 1)];
        assert_eq!(
            navigation.navigate(
                &mut selected,
                &filtered,
                Direction::Down,
                Modifiers::default()
            ),
            None
        );
        // A context/hover edit can collapse a prior range to its active card.
        selected = BTreeSet::from([4]);
        navigation.navigate(
            &mut selected,
            &sections,
            Direction::Down,
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(selected, BTreeSet::from([4, 8]));
        selected.clear();
        navigation.select(
            &mut selected,
            &sections[0].items,
            8,
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(selected, BTreeSet::from([8]));
    }
}
