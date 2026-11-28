use super::*;
use crate::terminal_appearance::ThemeId;
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Key {
    kind: ItemKind,
    id: Option<Id>,
    initial_group: Option<Id>,
    initial_package: Option<Id>,
}

impl Key {
    pub(super) fn new(
        kind: ItemKind,
        id: Option<Id>,
        initial_group: Option<Id>,
        initial_package: Option<Id>,
    ) -> Self {
        Self {
            kind,
            id,
            initial_group: if id.is_none() && matches!(kind, ItemKind::Host | ItemKind::Group) {
                initial_group
            } else {
                None
            },
            initial_package: if id.is_none() && kind == ItemKind::Snippet {
                initial_package
            } else {
                None
            },
        }
    }
}

impl Editor {
    pub(super) fn fingerprint(&self, cx: &App) -> u64 {
        let mut hash = DefaultHasher::new();
        for (_, input) in &self.fields {
            input.read(cx).value().hash(&mut hash);
        }
        self.group.hash(&mut hash);
        self.credential.hash(&mut hash);
        self.proxy_id.hash(&mut hash);
        if let Some(tags) = &self.tags {
            tags.view.read(cx).selected().hash(&mut hash);
        }
        if let Some(forward) = &self.forward {
            forward.draft_fields().hash(&mut hash);
        }
        if let Some(snippet) = &self.snippet {
            snippet.hash_fields(cx, &mut hash);
        }
        if let Some(theme) = &self.theme {
            theme.selected.map(ThemeId::id).hash(&mut hash);
            theme.dark_selected.map(ThemeId::id).hash(&mut hash);
        }
        if let Some(selects) = &self.selects {
            selects.auth.hash_password(cx, &mut hash);
        }
        if let Some(host) = &self.host {
            host.kind.hash(&mut hash);
            host.shell.hash(&mut hash);
            host.directory.read(cx).value().hash(&mut hash);
            host.selected_icon(cx).hash(&mut hash);
            host.ssh_enabled.hash(&mut hash);
            host.setup.startup_snippet.hash(&mut hash);
            host.setup.encoding.label().hash(&mut hash);
            host.setup.compatibility_mode.hash(&mut hash);
            host.environment_changed.hash(&mut hash);
            if host.environment_changed {
                for variable in host.environment.iter().flatten() {
                    variable.variable.hash(&mut hash);
                    variable.value.hash(&mut hash);
                }
            }
            if host.kind == HostKind::Serial {
                host.serial.read(cx).hash_fields(cx, &mut hash);
            }
            host.mosh.enabled.hash(&mut hash);
            host.mosh.command.read(cx).value().hash(&mut hash);
            let telnet = &host.telnet;
            telnet.enabled.hash(&mut hash);
            telnet.port.read(cx).value().hash(&mut hash);
            telnet.username.read(cx).value().hash(&mut hash);
            telnet.profile.identity_id.hash(&mut hash);
            telnet.profile.encoding.label().hash(&mut hash);
            telnet.theme.selected.map(ThemeId::id).hash(&mut hash);
            telnet.theme.dark_selected.map(ThemeId::id).hash(&mut hash);
            telnet.auth.hash_password(cx, &mut hash);
        }
        hash.finish()
    }

    pub(super) fn has_draft(&self, cx: &App) -> bool {
        !self.child_drafts.is_empty()
            || !self.credential_drafts.is_empty()
            || self.proxy_draft.is_some()
            || self
                .related_credential
                .as_ref()
                .is_some_and(|related| related.view.read(cx).has_draft(cx))
            || self
                .related_proxy
                .as_ref()
                .is_some_and(|related| related.view.read(cx).has_draft(cx))
            || self.fingerprint(cx) != self.baseline
    }
}

#[derive(Clone, Copy)]
enum EditorAction {
    Connect,
    Duplicate,
    Delete,
    Discard,
}

impl Nauterm {
    pub(super) fn resource_editor_menu(
        &self,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        use crate::components::management::{discard_changes_item, editor_more_button};
        let token = editor.fields[0].1.entity_id();
        let kind = editor.kind;
        let id = editor.id;
        let has_draft = editor.has_draft(cx);
        let owner = cx.weak_entity();
        editor_more_button("resource-editor-more")
            .disabled(
                self.resource_editor_busy(cx)
                    || id.is_some_and(|id| {
                        kind == ItemKind::Forward && self.forward_mutations.contains(&id)
                    }),
            )
            .dropdown_menu(move |menu, _, _| {
                let item = |item: MenuItem, action| {
                    let owner = owner.clone();
                    item.on_click(move |_, window, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            this.resource_editor_action(token, action, window, cx);
                        });
                    })
                };
                menu.width(180.)
                    .when(id.is_some() && kind == ItemKind::Host, |menu| {
                        menu.item(item(
                            MenuItem::localized("common.action.connect").icon(IconName::Terminal),
                            EditorAction::Connect,
                        ))
                    })
                    .when(
                        id.is_some()
                            && matches!(kind, ItemKind::Host | ItemKind::Group | ItemKind::Snippet),
                        |menu| {
                            menu.item(item(
                                MenuItem::localized("workspace.action.duplicate")
                                    .icon(IconName::Copy),
                                EditorAction::Duplicate,
                            ))
                        },
                    )
                    .when(id.is_some(), |menu| {
                        menu.item(item(
                            MenuItem::localized("common.action.delete")
                                .icon(IconName::Trash)
                                .destructive(true),
                            EditorAction::Delete,
                        ))
                        .separator()
                    })
                    .item(item(discard_changes_item(has_draft), EditorAction::Discard))
            })
    }

    fn resource_editor_action(
        &mut self,
        token: EntityId,
        action: EditorAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = self
            .editor
            .as_ref()
            .filter(|editor| editor.fields[0].1.entity_id() == token)
        else {
            return;
        };
        if self.resource_editor_busy(cx)
            || (editor.kind == ItemKind::Forward
                && editor
                    .id
                    .is_some_and(|id| self.forward_mutations.contains(&id)))
        {
            return;
        }
        match action {
            EditorAction::Connect => self.save_editor(true, window, cx),
            EditorAction::Duplicate | EditorAction::Delete => {
                let Some((kind, id)) = editor.edited() else {
                    return;
                };
                self.selection = BTreeSet::from([id]);
                self.selection_kind = Some(kind);
                if matches!(action, EditorAction::Delete) {
                    self.delete_pending = true;
                    cx.notify();
                } else if kind == ItemKind::Snippet {
                    self.duplicate_snippets(BTreeSet::from([id]), window, cx);
                } else {
                    self.duplicate_resources(kind, cx);
                }
            }
            EditorAction::Discard => {
                if !editor.has_draft(cx) {
                    return;
                }
                if editor.parent.is_some() {
                    self.restore_resource_parent(None, false, window, cx);
                } else {
                    if let Some(key) = editor.draft_key {
                        self.editor_drafts.remove(&key);
                    }
                    self.retire_editor();
                    self.selection.clear();
                    self.selection_kind = None;
                    self.delete_pending = false;
                    self.focus.focus(window, cx);
                    cx.notify();
                }
            }
        }
    }

    pub(super) fn dismiss_editor(&mut self, cx: &App) -> bool {
        if self.resource_editor_busy(cx) {
            return false;
        }
        while let Some(mut child) = self.editor.take() {
            let Some(parent) = child.parent.take() else {
                self.editor = Some(child);
                break;
            };
            self.editor = Some(parent.retain_child(child, cx));
        }
        if let Some(mut editor) = self.editor.take() {
            editor.park_related_drawers(cx);
            let key = editor.draft_key.filter(|_| editor.has_draft(cx));
            let form =
                crate::editor_drafts::Retired::put_form(key, editor, &mut self.editor_drafts);
            self.drawer_motion.retire(RetiredDrawer::Editor(form));
        }
        true
    }

    pub(super) fn prune_editor_drafts(&mut self) {
        self.editor_drafts.retain(|key, _| {
            let valid_group = key
                .initial_group
                .is_none_or(|id| self.data.groups.iter().any(|item| item.id == id));
            let valid_package = key.initial_package.is_none_or(|id| {
                self.snippet_state
                    .catalog
                    .packages
                    .iter()
                    .any(|item| item.id == id)
            });
            valid_group
                && valid_package
                && key.id.is_none_or(|id| match key.kind {
                    ItemKind::Host => self.data.hosts.iter().any(|item| item.id == id),
                    ItemKind::Group => self.data.groups.iter().any(|item| item.id == id),
                    ItemKind::Forward => self.data.forwards.iter().any(|item| item.id == id),
                    ItemKind::Snippet => self
                        .snippet_state
                        .catalog
                        .snippets
                        .iter()
                        .any(|item| item.id == id),
                    ItemKind::SnippetPackage => self
                        .snippet_state
                        .catalog
                        .packages
                        .iter()
                        .any(|item| item.id == id),
                    ItemKind::Proxy => false,
                })
        });
    }
}
