use super::*;

#[derive(Default)]
pub(super) struct PageDrawer {
    pub editor: Option<Editor>,
    history: Option<Entity<history::History>>,
    history_events: Option<Subscription>,
    selection: BTreeSet<Id>,
    selection_kind: Option<ItemKind>,
    navigation: crate::management::navigation::Navigation<(ItemKind, Id)>,
    motion: crate::components::panel::Presence<RetiredDrawer>,
}

fn exchange_drawer<T: Default>(
    saved: &mut HashMap<Page, T>,
    previous: Page,
    next: Page,
    current: T,
) -> T {
    saved.insert(previous, current);
    saved.remove(&next).unwrap_or_default()
}

impl Nauterm {
    fn take_page_drawer(&mut self) -> PageDrawer {
        let mut saved = PageDrawer {
            editor: self.editor.take(),
            history: self.history.take(),
            history_events: self.history_events.take(),
            selection: std::mem::take(&mut self.selection),
            selection_kind: self.selection_kind.take(),
            navigation: std::mem::take(&mut self.card_navigation),
            motion: std::mem::take(&mut self.drawer_motion),
        };
        if saved.editor.is_none() && saved.history.is_none() {
            saved.motion.sample(false, std::time::Instant::now(), true);
        }
        saved
    }
    pub(super) fn switch_drawer_page(&mut self, page: Page) {
        if self.page == page {
            return;
        }
        let previous = self.take_page_drawer();
        let saved = exchange_drawer(&mut self.saved_drawers, self.page, page, previous);
        self.editor = saved.editor;
        self.history = saved.history;
        self.history_events = saved.history_events;
        self.selection = saved.selection;
        self.selection_kind = saved.selection_kind;
        self.card_navigation = saved.navigation;
        self.drawer_motion = saved.motion;
        self.page = page;
    }
    pub(super) fn find_editor_mut(
        &mut self,
        matches: &impl Fn(&Editor) -> bool,
    ) -> Option<&mut Editor> {
        self.editor
            .iter_mut()
            .chain(
                self.saved_drawers
                    .values_mut()
                    .filter_map(|saved| saved.editor.as_mut()),
            )
            .find_map(|editor| editor.find_mut(matches))
    }
    /// Finish work in its owning drawer without changing the visible page/focus.
    pub(super) fn with_editor_context<R>(
        &mut self,
        matches: impl Fn(&Editor) -> bool,
        window: &mut Window,
        cx: &mut Context<Self>,
        work: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) -> R,
    ) -> Option<R> {
        if self.editor.as_ref().is_some_and(&matches) {
            return Some(work(self, window, cx));
        }
        let page = self.saved_drawers.iter().find_map(|(page, saved)| {
            saved.editor.as_ref().is_some_and(&matches).then_some(*page)
        })?;
        Some(self.in_drawer_page(page, window, cx, work))
    }
    pub(super) fn in_drawer_page<R>(
        &mut self,
        page: Page,
        window: &mut Window,
        cx: &mut Context<Self>,
        work: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) -> R,
    ) -> R {
        let previous = self.page;
        if page == previous {
            return work(self, window, cx);
        }
        let focus = window.focused(cx);
        self.switch_drawer_page(page);
        let result = work(self, window, cx);
        self.switch_drawer_page(previous);
        if let Some(focus) = focus {
            focus.focus(window, cx);
        }
        result
    }

    pub(super) fn retire_editor(&mut self) {
        if let Some(editor) = self.editor.take() {
            self.drawer_motion
                .retire(RetiredDrawer::Editor(Box::new(editor)));
        }
    }
    pub(super) fn retire_history(&mut self) {
        if let Some(history) = self.history.take() {
            self.drawer_motion.retire(RetiredDrawer::History(history));
        }
    }
    pub(super) fn animated_drawer(
        &mut self,
        amount: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.editor.is_some() || self.history.is_some();
        let content = if self.editor.is_some() {
            self.drawer(window, cx)
        } else if let Some(history) = &self.history {
            history.clone().into_any_element()
        } else {
            match self.drawer_motion.take_retired() {
                Some(RetiredDrawer::Editor(editor)) => {
                    // Paint with the former model, then remove it from the live
                    // slot before events/async callbacks can be dispatched.
                    self.editor = Some(*editor);
                    let content = self.drawer(window, cx);
                    self.retire_editor();
                    content
                }
                Some(RetiredDrawer::History(history)) => {
                    let content = history.clone().into_any_element();
                    self.drawer_motion.retire(RetiredDrawer::History(history));
                    content
                }
                None => div().into_any_element(),
            }
        };
        crate::components::panel::slide(
            "workspace-drawer-slide",
            layout::DRAWER_WIDTH,
            amount,
            open,
            content,
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::exchange_drawer;
    use crate::model::Page;
    use std::{collections::HashMap, rc::Rc};

    #[derive(Default)]
    struct Draft {
        text: String,
        level: u8,
        owner: Rc<()>,
    }

    #[test]
    fn pages_keep_independent_drafts_until_their_own_close() {
        let host = Rc::new(());
        let snippet = Rc::new(());
        let mut saved = HashMap::new();
        let mut current = Draft {
            text: "unsaved host".into(),
            level: 3,
            owner: host.clone(),
        };
        current = exchange_drawer(&mut saved, Page::Hosts, Page::Snippets, current);
        assert!(current.text.is_empty());
        assert_eq!(Rc::strong_count(&host), 2);
        current.text = "unsaved script".into();
        current.level = 2;
        current.owner = snippet.clone();
        current = exchange_drawer(&mut saved, Page::Snippets, Page::Hosts, current);
        assert_eq!((&*current.text, current.level), ("unsaved host", 3));
        assert!(Rc::ptr_eq(&current.owner, &host));
        assert_eq!(Rc::strong_count(&snippet), 2);
        drop(current); // Closing this drawer must leave the other page's draft alive.
        assert_eq!(Rc::strong_count(&host), 1);
        current = exchange_drawer(&mut saved, Page::Hosts, Page::Snippets, Draft::default());
        assert_eq!((&*current.text, current.level), ("unsaved script", 2));
        assert!(Rc::ptr_eq(&current.owner, &snippet));
        current = exchange_drawer(&mut saved, Page::Snippets, Page::Hosts, current);
        assert!(current.text.is_empty());
        assert_eq!(Rc::strong_count(&snippet), 2);
    }
}
