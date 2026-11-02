//! Shared management behavior; each page retains its own view preferences.
pub mod navigation;
use crate::components::menu::{Menu, MenuItem};
use gpui_kit::{App, Window, assets::IconName};
use std::{cmp::Ordering, rc::Rc};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    NameAscending,
    NameDescending,
    NewestFirst,
    OldestFirst,
}
impl SortOrder {
    pub const ALL: [Self; 4] = [
        Self::NameAscending,
        Self::NameDescending,
        Self::NewestFirst,
        Self::OldestFirst,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::NameAscending => "Name A–Z",
            Self::NameDescending => "Name Z–A",
            Self::NewestFirst => "Newest to oldest",
            Self::OldestFirst => "Oldest to newest",
        }
    }
    pub fn icon(self) -> IconName {
        match self {
            Self::NameAscending => IconName::ArrowDownAZ,
            Self::NameDescending => IconName::ArrowDownZA,
            Self::NewestFirst => IconName::CalendarArrowDown,
            Self::OldestFirst => IconName::CalendarArrowUp,
        }
    }
    /// IDs supply the reference's creation-order fallback when no date exists.
    pub fn compare(self, left: (&str, u64), right: (&str, u64)) -> Ordering {
        let ordinal = left.1.cmp(&right.1);
        let name = || {
            left.0
                .to_lowercase()
                .cmp(&right.0.to_lowercase())
                .then(ordinal)
        };
        match self {
            Self::NameAscending => name(),
            Self::NameDescending => name().reverse(),
            Self::NewestFirst => ordinal.reverse(),
            Self::OldestFirst => ordinal,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct CollectionView {
    pub list: bool,
    pub sort: SortOrder,
    pub search_open: bool,
}

pub fn sort_menu(
    mut menu: Menu,
    selected: SortOrder,
    changed: impl Fn(SortOrder, &mut Window, &mut App) + 'static,
) -> Menu {
    let changed = Rc::new(changed);
    for order in SortOrder::ALL {
        let changed = changed.clone();
        menu = menu.item(
            MenuItem::localized(order.label())
                .icon(order.icon())
                .checked(order == selected)
                .on_click(move |_, window, cx| changed(order, window, cx)),
        );
    }
    menu
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn management_sort_has_stable_ties_and_creation_order() {
        let items = [("beta", 3), ("ALPHA", 2), ("alpha", 1)];
        for (order, expected) in [
            (SortOrder::NameAscending, [1, 2, 3]),
            (SortOrder::NameDescending, [3, 2, 1]),
            (SortOrder::NewestFirst, [3, 2, 1]),
            (SortOrder::OldestFirst, [1, 2, 3]),
        ] {
            let mut sorted = items;
            sorted.sort_by(|left, right| order.compare(*left, *right));
            assert_eq!(sorted.map(|item| item.1), expected);
        }
    }
}
