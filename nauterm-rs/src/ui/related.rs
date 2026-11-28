//! Resource drawers retain their actual editor state while creating references.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Target {
    ParentGroup,
    ProtocolGroup,
    StartupSnippet,
}
impl Target {
    fn kind(self) -> ItemKind {
        match self {
            Self::ParentGroup | Self::ProtocolGroup => ItemKind::Group,
            Self::StartupSnippet => ItemKind::Snippet,
        }
    }
}
pub(super) struct Parent {
    editor: Box<Editor>,
    target: Target,
    focus: FocusHandle,
}
impl Parent {
    pub(super) fn retain_child(self, mut child: Editor, cx: &App) -> Editor {
        let mut editor = *self.editor;
        child.park_related_drawers(cx);
        if child.has_draft(cx) {
            editor.child_drafts.insert(self.target, child);
        }
        editor
    }
}

pub(super) struct ProxyEditor {
    pub view: Entity<crate::proxies::Proxies>,
    _events: Subscription,
}

impl Editor {
    pub(super) fn park_related_drawers(&mut self, cx: &App) {
        if let Some(related) = self.related_credential.take()
            && related.view.read(cx).has_draft(cx)
        {
            self.credential_drafts.insert(related.key, related);
        }
        if let Some(related) = self.related_proxy.take()
            && related.view.read(cx).has_draft(cx)
        {
            self.proxy_draft = Some(related);
        }
    }

    pub(super) fn can_choose_group_parent(&self, data: &AppState) -> bool {
        self.kind != ItemKind::Group
            || (data.group_can_be_nested(self.group_exclusion())
                && !self.parent.as_ref().is_some_and(|parent| {
                    parent.target == Target::ParentGroup && parent.editor.kind == ItemKind::Group
                }))
    }
    pub(super) fn parent_group_allowed(&self, data: &AppState, parent: Id) -> bool {
        data.groups.iter().any(|group| group.id == parent)
            && (self.kind != ItemKind::Group
                || (self.can_choose_group_parent(data)
                    && data.group_parent_allowed(self.group_exclusion(), parent)))
    }
    pub(super) fn tag_pickers(&self) -> Vec<Entity<crate::tags::picker::Picker>> {
        let mut pickers: Vec<_> = self
            .tags
            .as_ref()
            .map(|draft| draft.view.clone())
            .into_iter()
            .collect();
        if let Some(parent) = &self.parent {
            pickers.extend(parent.editor.tag_pickers());
        }
        pickers
    }
    pub(super) fn clear_proxy_references(&mut self, deleted: &BTreeSet<i64>) {
        if self.proxy_id.is_some_and(|id| deleted.contains(&id)) {
            self.proxy_id = None;
        }
        if let Some(parent) = &mut self.parent {
            parent.editor.clear_proxy_references(deleted);
        }
    }
    pub(super) fn find_mut(&mut self, matches: &impl Fn(&Editor) -> bool) -> Option<&mut Editor> {
        if matches(self) {
            Some(self)
        } else {
            self.parent.as_mut()?.editor.find_mut(matches)
        }
    }
    pub(super) fn edited(&self) -> Option<(ItemKind, Id)> {
        self.id
            .map(|id| (self.kind, id))
            .or_else(|| self.parent.as_ref()?.editor.edited())
    }
    pub(super) fn group_exclusion(&self) -> Option<Id> {
        if self.kind == ItemKind::Group {
            self.id
                .or_else(|| self.parent.as_ref()?.editor.group_exclusion())
        } else {
            None
        }
    }
}

impl Nauterm {
    pub(super) fn open_related_proxy(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.resource_editor_busy(cx)
            || !self.editor.as_ref().is_some_and(|editor| {
                editor.host.is_some()
                    && editor.related_proxy.is_none()
                    && editor.related_credential.is_none()
            })
        {
            return;
        }
        if let Some(related) = self.editor.as_mut().unwrap().proxy_draft.take() {
            related
                .view
                .update(cx, |view, cx| view.resume_draft(window, cx));
            self.editor.as_mut().unwrap().related_proxy = Some(related);
            cx.notify();
            return;
        }
        let owner = self.ensure_proxies(cx);
        let Some(source) = owner.update(cx, |view, cx| view.related_source(cx)) else {
            self.editor.as_mut().unwrap().error =
                Some("Proxy storage is not ready. Open Proxies and retry.".into());
            cx.notify();
            return;
        };
        let view = cx.new(|cx| crate::proxies::Proxies::related(source, name, window, cx));
        let events = cx.subscribe_in(
            &view,
            window,
            |this, view, event: &crate::proxies::RelatedEvent, window, cx| {
                this.with_editor_context(
                    |editor| {
                        editor
                            .related_proxy
                            .as_ref()
                            .is_some_and(|related| related.view == *view)
                    },
                    window,
                    cx,
                    |this, window, cx| {
                        let Some(editor) = this.editor.as_mut().filter(|editor| {
                            editor
                                .related_proxy
                                .as_ref()
                                .is_some_and(|related| related.view == *view)
                        }) else {
                            return;
                        };
                        if let crate::proxies::RelatedEvent::Saved(id) = event {
                            editor.proxy_id = Some(*id);
                        }
                        let related = editor.related_proxy.take().unwrap();
                        if matches!(event, crate::proxies::RelatedEvent::Cancelled)
                            && view.read(cx).has_draft(cx)
                        {
                            editor.proxy_draft = Some(related);
                        }
                        if let Some(selects) = &editor.selects {
                            selects
                                .proxies
                                .state
                                .read(cx)
                                .focus_handle(cx)
                                .focus(window, cx);
                        }
                        cx.notify();
                    },
                );
            },
        );
        self.editor.as_mut().unwrap().related_proxy = Some(ProxyEditor {
            view,
            _events: events,
        });
        cx.notify();
    }
    pub(super) fn parent_group_field(&self, editor: &Editor) -> AnyElement {
        let actions = if editor.can_choose_group_parent(&self.data) {
            vec![crate::components::SelectAction::new(
                "group",
                "workspace.action.createGroup",
                crate::assets::GROUP_ICON,
            )]
        } else {
            vec![]
        };
        editor.selects.as_ref().unwrap().groups.field_with_actions(
            "workspace.label.parentGroup",
            editor.host.as_ref().is_some_and(|draft| draft.busy),
            actions,
        )
    }
    pub(super) fn resource_editor_busy(&self, cx: &App) -> bool {
        self.editor.as_ref().is_some_and(|editor| {
            editor.host.as_ref().is_some_and(|draft| draft.busy)
                || editor
                    .related_credential
                    .as_ref()
                    .is_some_and(|related| related.view.read(cx).busy(cx))
                || editor
                    .related_proxy
                    .as_ref()
                    .is_some_and(|related| related.view.read(cx).editor_busy(cx))
                || editor
                    .forward
                    .as_ref()
                    .is_some_and(|forward| forward.saving())
                || (matches!(editor.kind, ItemKind::Snippet | ItemKind::SnippetPackage)
                    && self.snippet_state.busy)
        })
    }
    fn sync_resource_selection(&mut self) {
        if let Some((kind, id)) = self.editor.as_ref().and_then(Editor::edited) {
            self.selection = BTreeSet::from([id]);
            self.selection_kind = Some(kind);
        } else {
            self.selection.clear();
            self.selection_kind = None;
        }
    }
    pub(super) fn open_related_resource(
        &mut self,
        target: Target,
        name: &str,
        focus: FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.resource_editor_busy(cx) || !self.resources_ready() {
            return;
        }
        if !self.editor.as_ref().is_some_and(|editor| {
            editor.related_credential.is_none()
                && editor.related_proxy.is_none()
                && match target {
                    Target::ParentGroup => {
                        editor.host.is_some() && editor.can_choose_group_parent(&self.data)
                    }
                    Target::StartupSnippet => editor.host.is_some(),
                    Target::ProtocolGroup => editor.kind == ItemKind::Host && editor.host.is_some(),
                }
        }) {
            return;
        }
        if target == Target::StartupSnippet
            && (!self.snippet_state.ready || self.snippet_state.busy)
        {
            self.load_snippets(cx);
            return;
        }
        let Some(mut parent) = self.editor.take() else {
            return;
        };
        let restored = parent.child_drafts.remove(&target);
        let reopening = restored.is_some();
        if reopening {
            self.editor = restored;
            self.focus_resource_editor(window, cx);
        } else {
            self.build_editor(target.kind(), None, None, None, window, cx);
        }
        let Some(editor) = &mut self.editor else {
            self.editor = Some(parent);
            return;
        };
        editor.parent = Some(Parent {
            editor: Box::new(parent),
            target,
            focus,
        });
        if !reopening {
            editor.fields[0]
                .1
                .update(cx, |field, cx| field.set_value(name, window, cx));
            editor.baseline = editor.fingerprint(cx);
        }
        self.sync_resource_selection();
        cx.notify();
    }
    pub(super) fn back_related_resource(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.parent.is_some())
        {
            return false;
        }
        if self.resource_editor_busy(cx) {
            return true;
        }
        self.restore_resource_parent(None, true, window, cx);
        true
    }
    pub(super) fn restore_resource_parent(
        &mut self,
        saved: Option<(ItemKind, Id)>,
        keep_draft: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut child = self.editor.take().expect("a related editor is open");
        let parent = child.parent.take().expect("a related editor has a parent");
        let focus = parent.focus.clone();
        let target = parent.target;
        self.editor = Some(if keep_draft {
            parent.retain_child(child, cx)
        } else {
            *parent.editor
        });
        if let Some((_, id)) = saved.filter(|(kind, _)| *kind == target.kind()) {
            let editor = self.editor.as_mut().unwrap();
            match target {
                // Creating a protocol template does not select a parent Group.
                Target::ProtocolGroup => {}
                Target::ParentGroup => {
                    if editor.parent_group_allowed(&self.data, id) {
                        editor.group = Some(id);
                        self.inherit_editor_group(window, cx);
                    } else {
                        editor.error = Some("The new group cannot be this group's parent.".into());
                    }
                }
                Target::StartupSnippet => {
                    if let Some(draft) = &mut editor.host {
                        draft.setup.startup_snippet = Some(id);
                    }
                }
            }
        }
        self.sync_resource_selection();
        focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn complete_resource_editor(
        &mut self,
        kind: ItemKind,
        id: Id,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.parent.is_some())
        {
            self.restore_resource_parent(Some((kind, id)), false, window, cx);
        } else {
            self.retire_editor();
            self.selection = BTreeSet::from([id]);
            self.selection_kind = Some(kind);
            self.focus.focus(window, cx);
        }
    }
}
