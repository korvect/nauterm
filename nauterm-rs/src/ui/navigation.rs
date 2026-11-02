use super::*;
use crate::management::navigation::{Key, Section};

type CardKey = (ItemKind, Id);

impl Nauterm {
    pub(super) fn resource_shortcut(
        &mut self,
        command: crate::shortcuts::Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.focus.is_focused(window) {
            return false;
        }
        let sections = self.card_sections(window, cx);
        let selected = self.selected_card_keys();
        let Some((kind, id)) = self.card_navigation.active(&selected, &sections) else {
            return false;
        };
        if kind == ItemKind::Forward
            && self
                .selection
                .iter()
                .any(|id| self.forward_mutations.contains(id))
        {
            return true;
        }
        match command {
            crate::shortcuts::Command::Edit if selected.len() == 1 => {
                self.open_editor(kind, Some(id), window, cx)
            }
            crate::shortcuts::Command::Duplicate if kind == ItemKind::Snippet => {
                self.duplicate_snippets(self.selection.clone(), window, cx)
            }
            crate::shortcuts::Command::Duplicate if kind != ItemKind::SnippetPackage => {
                self.duplicate_resources(kind, cx)
            }
            crate::shortcuts::Command::Edit | crate::shortcuts::Command::Duplicate => {}
            _ => return false,
        }
        true
    }
    fn card_sections(&self, window: &Window, cx: &App) -> Vec<Section<CardKey>> {
        let preferences = self.collection_view();
        let query = self
            .searches
            .get(&self.page)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let sorted = |kind, mut items: Vec<(&str, Id)>| {
            items.sort_by(|a, b| preferences.sort.compare(*a, *b));
            items
                .into_iter()
                .map(|(_, id)| (kind, id))
                .collect::<Vec<_>>()
        };
        let sections = match self.page {
            Page::Hosts => {
                let tags = self.active_tag_filter();
                vec![
                    sorted(
                        ItemKind::Group,
                        self.data
                            .groups
                            .iter()
                            .filter(|group| {
                                group.parent == self.group
                                    && group.name.to_lowercase().contains(&query.to_lowercase())
                            })
                            .map(|group| (group.name.as_str(), group.id))
                            .collect(),
                    ),
                    sorted(
                        ItemKind::Host,
                        self.data
                            .hosts
                            .iter()
                            .filter(|host| {
                                (self.group.is_none() || host.group_id == self.group)
                                    && self.data.host_matches(host, &query)
                                    && tags.matches(host)
                            })
                            .map(|host| (host.name.as_str(), host.id))
                            .collect(),
                    ),
                ]
            }
            Page::Snippets if self.snippet_state.ready => {
                let package = self.snippet_state.package.filter(|id| {
                    self.snippet_state
                        .catalog
                        .packages
                        .iter()
                        .any(|package| package.id == *id)
                });
                vec![
                    sorted(
                        ItemKind::SnippetPackage,
                        self.snippet_state
                            .catalog
                            .packages
                            .iter()
                            .filter(|value| {
                                package.is_none()
                                    && value
                                        .name
                                        .to_lowercase()
                                        .contains(query.trim().to_lowercase().as_str())
                            })
                            .map(|value| (value.name.as_str(), value.id))
                            .collect(),
                    ),
                    sorted(
                        ItemKind::Snippet,
                        self.data
                            .snippets
                            .iter()
                            .filter(|snippet| {
                                package.is_none_or(|id| snippet.package_id == Some(id))
                                    && snippet.matches_query(&query)
                            })
                            .map(|snippet| (snippet.name.as_str(), snippet.id))
                            .collect(),
                    ),
                ]
            }
            Page::PortForwards => vec![sorted(
                ItemKind::Forward,
                self.data
                    .forwards
                    .iter()
                    .filter(|rule| self.forward_matches(rule, &query))
                    .map(|rule| (rule.name.as_str(), rule.id))
                    .collect(),
            )],
            _ => return vec![],
        };
        let width = f32::from(window.viewport_size().width);
        let available =
            width - self.sidebar_width - layout::DRAWER_WIDTH * self.drawer_motion.amount() - 52.;
        let columns = layout::CardGrid::new(
            available,
            sections.iter().map(Vec::len).max().unwrap_or(0),
            preferences.list || self.page == Page::PortForwards,
        )
        .columns as usize;
        sections
            .into_iter()
            .map(|items| Section::new(items, columns))
            .collect()
    }

    fn selected_card_keys(&self) -> BTreeSet<CardKey> {
        self.selection_kind
            .into_iter()
            .flat_map(|kind| self.selection.iter().map(move |id| (kind, *id)))
            .collect()
    }

    pub(super) fn select_collection_card(
        &mut self,
        kind: ItemKind,
        id: Id,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sections = self.card_sections(window, cx);
        let Some(section) = sections
            .iter()
            .find(|section| section.items.contains(&(kind, id)))
        else {
            return;
        };
        let mut selected = self.selected_card_keys();
        self.card_navigation
            .select(&mut selected, &section.items, (kind, id), modifiers);
        self.selection = selected.iter().map(|(_, id)| *id).collect();
        self.selection_kind = (!selected.is_empty()).then_some(kind);
        if self.editor.is_some() && !modifiers.shift && !modifiers.platform && !modifiers.control {
            self.open_editor(kind, Some(id), window, cx);
        }
        cx.notify();
    }

    pub(super) fn navigate_collection(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.focus.is_focused(window) || crate::components::overlay::is_modal_open(window, cx) {
            return;
        }
        let shortcut = self
            .data
            .shortcuts
            .matching(crate::shortcuts::Scope::Collection, &event.keystroke);
        let command = Key::read(&event.keystroke);
        if shortcut.is_none() && command.is_none() {
            return;
        }
        let sections = self.card_sections(window, cx);
        let mut selected = self.selected_card_keys();
        let Some(current) = self.card_navigation.active(&selected, &sections) else {
            return;
        };
        if let Some(shortcut) = shortcut {
            if self.resource_shortcut(shortcut, window, cx) {
                cx.stop_propagation();
            }
            return;
        }
        let Some(command) = command else {
            return;
        };
        match command {
            Key::Activate => match current {
                (ItemKind::Host, id) => {
                    if let Some(host) = self.data.hosts.iter().find(|host| host.id == id).cloned() {
                        self.open_ssh(host, window, cx);
                    }
                }
                (ItemKind::Group, id) => self.enter_group(Some(id), window, cx),
                (ItemKind::SnippetPackage, id) => self.open_snippet_package(Some(id), window, cx),
                (ItemKind::Snippet, id) => self.run_snippet_locally(id, window, cx),
                (kind, id) => self.open_editor(kind, Some(id), window, cx),
            },
            Key::Move(direction) => {
                let Some((kind, id)) = self.card_navigation.navigate(
                    &mut selected,
                    &sections,
                    direction,
                    event.keystroke.modifiers,
                ) else {
                    return;
                };
                self.selection = selected.iter().map(|(_, id)| *id).collect();
                self.selection_kind = (!selected.is_empty()).then_some(kind);
                if self.editor.is_some()
                    && selected.len() == 1
                    && selected.contains(&(kind, id))
                    && self.editor.as_ref().and_then(Editor::edited) != Some((kind, id))
                {
                    self.open_editor(kind, Some(id), window, cx);
                    self.focus.focus(window, cx);
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
}
