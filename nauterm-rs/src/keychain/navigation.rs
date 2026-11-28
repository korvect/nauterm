use super::*;
use crate::management::navigation::{Key, Section};

impl Keychain {
    pub(crate) fn collection_shortcut(
        &mut self,
        command: crate::shortcuts::Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.focus.is_focused(window) {
            return false;
        }
        if self.busy(cx) || self.loading {
            return true;
        }
        let sections = self.navigation_sections(window, cx);
        let Some(item) = self.navigation.active(&self.selection, &sections) else {
            return false;
        };
        match command {
            crate::shortcuts::Command::Edit if self.selection.len() == 1 => {
                self.edit(item, window, cx)
            }
            crate::shortcuts::Command::Duplicate => self.duplicate(cx),
            crate::shortcuts::Command::Edit => {}
            _ => return false,
        }
        true
    }
    fn navigation_sections(&self, window: &Window, cx: &App) -> Vec<Section<Item>> {
        let visible = self.visible(cx);
        let keys: Vec<_> = visible
            .iter()
            .filter(|(item, _, _)| item.key)
            .map(|(item, _, _)| *item)
            .collect();
        let identities: Vec<_> = visible
            .iter()
            .filter(|(item, _, _)| !item.key)
            .map(|(item, _, _)| *item)
            .collect();
        let width = f32::from(window.viewport_size().width);
        let available = self
            .parent_width
            .unwrap_or_else(|| width - layout::sidebar_width(width))
            - 52.
            - layout::DRAWER_WIDTH * self.drawer_motion.amount();
        let columns = layout::CardGrid::new(available, keys.len().max(identities.len()), self.list)
            .columns as usize;
        vec![
            Section::new(keys, columns),
            Section::new(identities, columns),
        ]
    }

    pub(super) fn navigate_cards(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.focus.is_focused(window)
            || self.busy(cx)
            || self.loading
            || crate::components::overlay::is_modal_open(window, cx)
        {
            return;
        }
        let shortcut = crate::shortcuts::current(cx)
            .matching(crate::shortcuts::Scope::Collection, &event.keystroke);
        let command = Key::read(&event.keystroke);
        if shortcut.is_none() && command.is_none() {
            return;
        }
        let sections = self.navigation_sections(window, cx);
        let Some(current) = self.navigation.active(&self.selection, &sections) else {
            return;
        };
        if let Some(shortcut) = shortcut {
            if self.collection_shortcut(shortcut, window, cx) {
                cx.stop_propagation();
            }
            return;
        }
        let Some(command) = command else {
            return;
        };
        match command {
            Key::Activate => self.edit(current, window, cx),
            Key::Move(direction) => {
                let Some(item) = self.navigation.navigate(
                    &mut self.selection,
                    &sections,
                    direction,
                    event.keystroke.modifiers,
                ) else {
                    return;
                };
                if self.selection.len() == 1
                    && self.selection.contains(&item)
                    && (self.editor.is_some() || self.export.is_some())
                    && self.active_item() != Some(item)
                {
                    self.edit_with_focus(item, true, window, cx);
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn select_card(&mut self, item: Item, modifiers: Modifiers, cx: &mut Context<Self>) {
        let visible: Vec<_> = self
            .visible(cx)
            .into_iter()
            .map(|(value, _, _)| value)
            .filter(|value| value.key == item.key)
            .collect();
        self.navigation
            .select(&mut self.selection, &visible, item, modifiers);
        cx.notify();
    }
}
