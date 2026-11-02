use super::*;
use crate::components::Checkbox;
use crate::snippet_store::Catalog;
use crate::snippets::ToolOrigin;
use crate::typography::FontWeights;
use gpui_kit::base::input::TextareaState;

pub(super) struct InlineSnippet {
    origin: ToolOrigin,
    label: String,
    description: Entity<InputState>,
    fields: SnippetDraft,
    pub(super) error: Option<String>,
    validation_error: Option<String>,
}
fn package_options(packages: &[crate::snippet_store::Package]) -> Vec<SelectOption<Id>> {
    packages
        .iter()
        .map(|package| SelectOption::new(package.id, package.name.clone()))
        .collect()
}

#[derive(Default)]
pub(super) struct SnippetState {
    pub ready: bool,
    pub busy: bool,
    pub error: Option<String>,
    pub catalog: Catalog,
    pub package: Option<Id>,
}

pub(super) struct SnippetDraft {
    script: Entity<TextareaState>,
    groups: BTreeSet<Id>,
    hosts: BTreeSet<Id>,
    package_id: Option<Id>,
    packages: SelectModel<Id>,
    _packages: Subscription,
    _package_creation: Subscription,
    _script: Subscription,
}
impl SnippetDraft {
    pub(super) fn select_package(&mut self, id: Id) {
        self.package_id = Some(id);
    }
    fn package_field(&self, busy: bool) -> AnyElement {
        self.packages.field_with_actions(
            "Package",
            busy,
            vec![
                crate::components::SelectAction::new(
                    "package",
                    "Create package",
                    IconName::Package,
                )
                .create_from_query(),
            ],
        )
    }
    pub(super) fn new(
        snippet: Option<&Snippet>,
        packages: &[crate::snippet_store::Package],
        current_package: Option<Id>,
        window: &mut Window,
        cx: &mut Context<Nauterm>,
    ) -> Self {
        Self::build(snippet, packages, current_package, None, window, cx)
    }
    fn build(
        snippet: Option<&Snippet>,
        packages: &[crate::snippet_store::Package],
        current_package: Option<Id>,
        tool: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Nauterm>,
    ) -> Self {
        let package_id = snippet.map_or_else(
            || {
                if tool.is_some() {
                    None
                } else {
                    current_package.or_else(|| packages.first().map(|p| p.id))
                }
            },
            |snippet| snippet.package_id,
        );
        let packages = SelectModel::new(package_options(packages), package_id, window, cx);
        let events = cx.subscribe(
            &packages.state,
            move |this, state, event: &SelectEvent<Id>, cx| {
                let draft = if let Some(id) = tool {
                    this.terminals
                        .iter_mut()
                        .find(|tab| tab.id == id)
                        .and_then(|tab| tab.tools.snippet_editor.as_mut())
                        .map(|draft| &mut draft.fields)
                } else {
                    this.editor.as_mut().and_then(|e| e.snippet.as_mut())
                };
                if let Some(draft) = draft
                    && draft.packages.state.entity_id() == state.entity_id()
                {
                    let SelectEvent::Confirm(value) = event;
                    draft.package_id = value.flatten();
                    cx.notify();
                }
            },
        );
        let creation = cx.subscribe_in(
            &packages.state,
            window,
            move |this, state, event: &crate::components::SelectCreate, window, cx| {
                if event.action == "package" {
                    this.create_snippet_package(tool, state.entity_id(), &event.text, window, cx);
                }
            },
        );
        let script = cx.new(|cx| {
            TextareaState::new(window, cx)
                .submit_on_enter(false)
                .auto_grow(6, 10)
                .default_value(snippet.map(|s| s.command.clone()).unwrap_or_default())
        });
        let script_events = cx.subscribe(&script, move |this, state, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) || state.read(cx).value().trim().is_empty() {
                return;
            }
            let error = if let Some(id) = tool {
                this.terminals
                    .iter_mut()
                    .find(|tab| tab.id == id)
                    .and_then(|tab| tab.tools.snippet_editor.as_mut())
                    .filter(|editor| editor.fields.script.entity_id() == state.entity_id())
                    .map(|editor| &mut editor.error)
            } else {
                this.editor
                    .as_mut()
                    .filter(|editor| {
                        editor
                            .snippet
                            .as_ref()
                            .is_some_and(|draft| draft.script.entity_id() == state.entity_id())
                    })
                    .map(|editor| &mut editor.error)
            };
            if let Some(error) = error
                && error
                    .as_ref()
                    .is_some_and(|error| error.starts_with("Script"))
            {
                *error = None;
                cx.notify();
            }
        });
        Self {
            package_id,
            packages,
            _packages: events,
            _package_creation: creation,
            script,
            _script: script_events,
            groups: snippet
                .map(|s| s.target_group_ids.clone())
                .unwrap_or_default(),
            hosts: snippet
                .map(|s| s.target_host_ids.clone())
                .unwrap_or_default(),
        }
    }
}
impl Nauterm {
    fn snippet_package_draft(
        &mut self,
        tool: Option<u64>,
        selector: EntityId,
    ) -> Option<&mut SnippetDraft> {
        let draft = if let Some(id) = tool {
            self.terminals
                .iter_mut()
                .find(|tab| tab.id == id)
                .and_then(|tab| tab.tools.snippet_editor.as_mut())
                .map(|editor| &mut editor.fields)
        } else {
            self.editor
                .as_mut()
                .and_then(|editor| editor.snippet.as_mut())
        };
        draft.filter(|draft| draft.packages.state.entity_id() == selector)
    }
    fn create_snippet_package(
        &mut self,
        tool: Option<u64>,
        selector: EntityId,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if name.trim().is_empty()
            || !self.resources_ready()
            || self.snippet_package_draft(tool, selector).is_none()
        {
            return;
        }
        let name = name.trim().to_owned();
        self.mutate_snippets(
            move |catalog| {
                anyhow::ensure!(
                    !catalog
                        .packages
                        .iter()
                        .any(|package| package.name.trim().to_lowercase() == name.to_lowercase()),
                    "A package with this name already exists."
                );
                catalog.save_package(None, name).map(Some)
            },
            move |this, id, window, cx| {
                let Some(id) = id else {
                    return;
                };
                let options = package_options(&this.snippet_state.catalog.packages);
                if let Some(draft) = this.snippet_package_draft(tool, selector) {
                    draft.select_package(id);
                    draft.packages.reconcile(options, Some(id), window, cx);
                }
            },
            window,
            cx,
        );
    }
    pub(super) fn snippet_from_history(
        &mut self,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.snippet_state.ready || self.snippet_state.busy || command.trim().is_empty() {
            return;
        }
        self.open_editor(ItemKind::Snippet, None, window, cx);
        if let Some(editor) = &self.editor {
            editor.fields[0]
                .1
                .update(cx, |input, cx| input.set_value(command, window, cx));
            if let Some(draft) = &editor.snippet {
                draft
                    .script
                    .update(cx, |input, cx| input.set_value(command, window, cx));
            }
        }
        cx.notify();
    }
    pub(super) fn refresh_snippet_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.snippet_state.ready {
            return;
        }
        let options = package_options(&self.snippet_state.catalog.packages);
        if let Some(draft) = self.editor.as_mut().and_then(|e| e.snippet.as_mut()) {
            draft
                .packages
                .reconcile(options.clone(), draft.package_id, window, cx);
        }
        for tab in self.terminals.iter_mut() {
            if let Some(editor) = &mut tab.tools.snippet_editor {
                editor.fields.packages.reconcile(
                    options.clone(),
                    editor.fields.package_id,
                    window,
                    cx,
                );
            }
        }
    }
    pub(super) fn refresh_snippet_composers(&mut self, cx: &mut Context<Self>) {
        if !self.terminals.is_empty()
            && !self.snippet_state.ready
            && !self.snippet_state.busy
            && self.snippet_state.error.is_none()
        {
            self.load_snippets(cx);
        }
        for session in self.terminals.iter().flat_map(|tab| tab.tree.sessions()) {
            let host = match session.restore_target {
                Some(
                    RestoreTarget::Host { id, .. }
                    | RestoreTarget::TelnetHost { id, .. }
                    | RestoreTarget::MoshHost { id, .. },
                ) => Some(id),
                _ => None,
            };
            if let TabContent::Terminal(terminal) = &session.content {
                let sources = if self.snippet_state.ready {
                    self.data
                        .snippets
                        .iter()
                        .filter(|s| s.matches_host(host, &self.data))
                        .map(|s| s.command.clone())
                        .collect()
                } else {
                    vec![]
                };
                terminal.update(cx, |terminal, cx| {
                    terminal.set_composer_sources(sources, cx)
                });
            }
        }
    }
    fn inline_origin(tab: &TerminalTab) -> ToolOrigin {
        let session = tab.active();
        ToolOrigin {
            tab: tab.id,
            session: session.id,
            host: match session.restore_target {
                Some(
                    RestoreTarget::Host { id, .. }
                    | RestoreTarget::TelnetHost { id, .. }
                    | RestoreTarget::MoshHost { id, .. },
                ) => Some(id),
                _ => None,
            },
        }
    }
    fn create_inline_snippet(
        &mut self,
        tab_id: u64,
        session_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.snippet_state.ready || self.snippet_state.busy {
            return;
        }
        let Some(tab) = self
            .terminals
            .iter()
            .find(|tab| tab.id == tab_id && tab.active().id == session_id)
        else {
            return;
        };
        let origin = Self::inline_origin(tab);
        let label = tab.active().title(cx);
        let fields = SnippetDraft::build(
            None,
            &self.snippet_state.catalog.packages,
            None,
            Some(tab_id),
            window,
            cx,
        );
        let input = fields.script.clone();
        let description = cx.new(|cx| InputState::new(window, cx));
        let Some(tab) = self.terminals.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        tab.tools.snippet_editor = Some(InlineSnippet {
            origin,
            label,
            fields,
            description,
            error: None,
            validation_error: None,
        });
        self.notice = None;
        input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }
    fn cancel_inline_snippet(&mut self, tab_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.snippet_state.busy {
            return;
        }
        if let Some(tab) = self.terminals.iter_mut().find(|tab| tab.id == tab_id) {
            tab.tools.snippet_editor = None;
            tab.focus(window, cx);
        }
        cx.notify();
    }
    fn save_inline_snippet(&mut self, tab_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.terminals.iter().find(|tab| tab.id == tab_id) else {
            return;
        };
        let Some(editor) = &tab.tools.snippet_editor else {
            return;
        };
        let token = editor.fields.script.entity_id();
        let origin = editor.origin;
        if let Err(error) = crate::snippets::command(editor.fields.script.read(cx).value().as_ref())
        {
            self.terminals
                .iter_mut()
                .find(|tab| tab.id == tab_id)
                .and_then(|tab| tab.tools.snippet_editor.as_mut())
                .unwrap()
                .validation_error = Some(error.to_string());
            cx.notify();
            return;
        }
        let result = crate::snippets::tool_draft(
            editor.description.read(cx).value().as_ref(),
            editor.fields.script.read(cx).value().as_ref(),
            editor.fields.package_id,
            origin,
            Self::inline_origin(tab),
            &self.data,
        );
        let snippet = match result {
            Ok(snippet) => snippet,
            Err(error) => {
                if let Some(editor) = self
                    .terminals
                    .iter_mut()
                    .find(|tab| tab.id == tab_id)
                    .and_then(|tab| tab.tools.snippet_editor.as_mut())
                {
                    editor.error = Some(error.to_string());
                }
                cx.notify();
                return;
            }
        };
        if let Some(editor) = self
            .terminals
            .iter_mut()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.tools.snippet_editor.as_mut())
        {
            editor.error = None;
        }
        self.notice = None;
        self.mutate_snippets(
            move |catalog| catalog.save(snippet, None).map(Some),
            move |this, _, window, cx| {
                let selected = this.page == Page::Sessions
                    && !this.overview
                    && this.active_terminal() == Some(tab_id);
                if let Some(tab) = this.terminals.iter_mut().find(|tab| tab.id == tab_id)
                    && tab
                        .tools
                        .snippet_editor
                        .as_ref()
                        .is_some_and(|editor| editor.fields.script.entity_id() == token)
                {
                    tab.tools.snippet_editor = None;
                    if selected && tab.active().id == origin.session {
                        tab.focus(window, cx);
                    }
                }
            },
            window,
            cx,
        );
    }
    fn inline_snippet_editor(
        &self,
        tab: &TerminalTab,
        editor: &InlineSnippet,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = Palette::terminal(tab.active().appearance(&self.data).palette());
        let id = tab.id;
        let busy = self.snippet_state.busy;
        let same = Self::inline_origin(tab) == editor.origin;
        let script_error = editor
            .validation_error
            .clone()
            .filter(|error| error.starts_with("Script"));
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .text_color(p.text)
            .child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .px(px(8.))
                    .border_b_1()
                    .border_color(p.border)
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        Button::new("back-inline-snippets")
                            .toolbar()
                            .ghost()
                            .custom(ButtonCustomVariant::for_surface(p))
                            .icon(IconName::ArrowLeft)
                            .tooltip("Back to snippets")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.cancel_inline_snippet(id, window, cx)
                            })),
                    )
                    .child(div().font_weight(FontWeights::MEDIUM).child(crate::localization::text(cx, "workspace.label.newSnippet"))),
            )
            .child(
                div()
                    .id("inline-snippet-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .scrollbars()
                    .child(
                        div()
                            .w_full()
                            .flex_shrink_0()
                            .px(px(12.))
                            .pt(px(14.))
                            .pb(px(18.))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child(div().w_full().flex_shrink_0().child(
                                editor.fields.packages.terminal_form_with_actions(
                                    "Package",
                                    busy,
                                    p,
                                    vec![crate::components::SelectAction::new(
                                        "package",
                                        "Create package",
                                        IconName::Package,
                                    )],
                                ),
                            ))
                            .child(
                                div().w_full().flex_shrink_0().child(
                                    Field::input(&editor.description, "Description")
                                        .with_size(gpui_kit::component::Size::Medium)
                                        .with_palette(Some(p))
                                        .disabled(busy),
                                ),
                            )
                            .child(
                                div().w_full().flex_shrink_0().child(
                                    Field::multiline(&editor.fields.script, "Script *", 230.)
                                        .with_size(gpui_kit::component::Size::Medium)
                                        .with_palette(Some(p))
                                        .disabled(busy)
                                        .error(script_error.map(Into::into)),
                                ),
                            )
                            .when(editor.origin.host.is_some(), |view| {
                                view.child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(p.muted)
                                        .child(crate::localization::view(format!("Available on {}", editor.label))),
                                )
                            })
                            .when(!same, |view| {
                                view.child(div().text_color(p.muted).child(
                                    crate::localization::text(cx, "Select the original terminal pane before saving this snippet."),
                                ))
                            }),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(p.border)
                    .px(px(12.))
                    .pt(px(10.))
                    .pb(px(12.))
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        Button::new("cancel-inline-snippet")
                            .with_size(gpui_kit::component::Size::Medium)
                            .h(px(32.))
                            .outline()
                            .custom(ButtonCustomVariant::for_surface(p))
                            .border_color(p.border)
                            .label("Cancel")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.cancel_inline_snippet(id, window, cx)
                            })),
                    )
                    .child(
                        Button::new("save-inline-snippet")
                            .with_size(gpui_kit::component::Size::Medium)
                            .h(px(32.))
                            .primary()
                            .custom(
                                ButtonCustomVariant::for_surface(p)
                                    .color(p.accent.into())
                                    .foreground(rgb(0xffffff).into())
                                    .hover(p.accent.blend(rgb(0xffffff).alpha(0.08)).into())
                                    .active(p.accent.blend(rgb(0x000000).alpha(0.08)).into()),
                            )
                            .label(if busy { "Saving…" } else { "Save" })
                            .disabled(busy || !same)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.save_inline_snippet(id, window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn load_snippets(&mut self, cx: &mut Context<Self>) {
        if self.snippet_state.ready || self.snippet_state.busy {
            return;
        }
        let vault = if self.resources_ready() {
            self.ensure_keychain(cx).read(cx).vault()
        } else {
            None
        };
        let Some(vault) = vault else {
            self.snippet_state.error =
                Some("The Nauterm database is unavailable. Retry after Keychain loads.".into());
            cx.notify();
            return;
        };
        self.snippet_state.busy = true;
        self.snippet_state.error = None;
        let legacy = self.data.legacy_snippets.clone();
        cx.spawn(async move |this, cx| {
            let result = vault.snippet_catalog(legacy).await;
            let _ = this.update(cx, |this, cx| {
                this.snippet_state.busy = false;
                let result = result.and_then(|catalog| {
                    this.data = this.store.publish_snippets(&this.data, &catalog)?;
                    Ok(catalog)
                });
                match result {
                    Ok(catalog) => {
                        this.data.snippets = catalog.snippets.clone();
                        this.snippet_state.catalog = catalog;
                        this.snippet_state.ready = true;
                    }
                    Err(error) => this.snippet_state.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn snippet_loading(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_1()
            .min_h_0()
            .map(crate::components::management::collection_padding)
            .flex()
            .flex_col()
            .gap(px(12.))
            .when(self.snippet_state.busy, |view| {
                view.child(crate::localization::text(cx, "rs.ui.copy.loadingSnippets"))
            })
            .when(
                self.page != Page::Snippets && self.snippet_state.error.is_some(),
                |view| {
                    view.child(
                        div().flex().justify_end().child(
                            Button::new("reload-tool-snippets")
                                .toolbar()
                                .ghost()
                                .icon(IconName::RefreshCw)
                                .tooltip("Reload snippets")
                                .disabled(self.snippet_state.busy)
                                .on_click(cx.listener(|this, _, _, cx| this.load_snippets(cx))),
                        ),
                    )
                },
            )
            .into_any_element()
    }
    fn mutate_snippets(
        &mut self,
        work: impl FnOnce(&mut Catalog) -> anyhow::Result<Option<Id>> + Send + 'static,
        done: impl FnOnce(&mut Self, Option<Id>, &mut Window, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.snippet_state.ready || self.snippet_state.busy {
            self.notice = Some("Wait for the snippet library to finish loading/saving.".into());
            cx.notify();
            return;
        }
        let Some(vault) = self.ensure_keychain(cx).read(cx).vault() else {
            self.notice = Some("Snippet database is unavailable.".into());
            cx.notify();
            return;
        };
        let before = self.snippet_state.catalog.clone();
        let page = self.page;
        let token = self
            .editor
            .as_ref()
            .map(|editor| editor.fields[0].1.entity_id());
        self.snippet_state.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let result = vault.update_snippets(before, work).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.snippet_state.busy = false;
                match result {
                    Ok((catalog, id)) => {
                        if this
                            .snippet_state
                            .package
                            .is_some_and(|id| !catalog.packages.iter().any(|p| p.id == id))
                        {
                            this.snippet_state.package = None;
                        }
                        this.data.snippets = catalog.snippets.clone();
                        this.snippet_state.catalog = catalog;
                        this.notice = None;
                        this.in_drawer_page(page, window, cx, |this, window, cx| {
                            done(this, id, window, cx)
                        });
                    }
                    Err(error) => {
                        this.notice = Some(error.to_string());
                        let message = this.notice.clone();
                        if let Some(editor) = this.find_editor_mut(&|editor| {
                            Some(editor.fields[0].1.entity_id()) == token
                        }) && matches!(editor.kind, ItemKind::Snippet | ItemKind::SnippetPackage)
                        {
                            editor.error = message;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn save_snippet_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(draft) = &editor.snippet else {
            return;
        };
        let token = draft.script.entity_id();
        let existing = editor.id;
        let name = editor.fields[0].1.read(cx).value().trim().to_owned();
        let snippet = match crate::snippets::draft(
            &name,
            draft.script.read(cx).value().as_ref(),
            draft.groups.clone(),
            draft.hosts.clone(),
            draft.package_id,
            self.data.snippets.iter().find(|s| Some(s.id) == existing),
        ) {
            Ok(snippet) => snippet,
            Err(error) => {
                self.editor.as_mut().unwrap().validation_error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        self.mutate_snippets(
            move |catalog| catalog.save(snippet, existing).map(Some),
            move |this, id, window, cx| {
                if this
                    .editor
                    .as_ref()
                    .and_then(|e| e.snippet.as_ref())
                    .is_some_and(|draft| draft.script.entity_id() == token)
                    && let Some(id) = id
                {
                    this.complete_resource_editor(ItemKind::Snippet, id, window, cx);
                }
            },
            window,
            cx,
        );
    }
    pub(super) fn duplicate_snippets(
        &mut self,
        ids: BTreeSet<Id>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mutate_snippets(
            move |catalog| {
                catalog.duplicate_many(&ids)?;
                Ok(None)
            },
            |_, _, _, _| {},
            window,
            cx,
        );
    }
    pub(super) fn open_snippet_package(
        &mut self,
        package: Option<Id>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if package.is_some_and(|id| {
            !self
                .snippet_state
                .catalog
                .packages
                .iter()
                .any(|p| p.id == id)
        }) {
            return;
        }
        self.snippet_state.package = package;
        self.selection.clear();
        self.selection_kind = None;
        self.retire_editor();
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn snippet_editor_actions(
        &self,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let kind = editor.kind;
        let Some(id) = editor
            .id
            .filter(|_| matches!(kind, ItemKind::Snippet | ItemKind::SnippetPackage))
        else {
            return vec![];
        };
        let token = editor.fields[0].1.entity_id();
        let owner = cx.weak_entity();
        vec![
            Button::new("snippet-editor-actions")
                .toolbar()
                .ghost()
                .icon(IconName::Ellipsis)
                .tooltip(if kind == ItemKind::Snippet {
                    "Snippet actions"
                } else {
                    "Package actions"
                })
                .disabled(self.snippet_state.busy)
                .dropdown_menu(move |menu, _, _| {
                    let duplicate = owner.clone();
                    let delete = owner.clone();
                    menu.when(kind == ItemKind::Snippet, |menu| {
                        menu.item(
                            MenuItem::localized("Duplicate")
                                .icon(IconName::Copy)
                                .on_click(move |_, window, cx| {
                                    let _ = duplicate.update(cx, |this, cx| {
                                        if this.editor.as_ref().is_some_and(|editor| {
                                            editor.fields[0].1.entity_id() == token
                                        }) {
                                            this.duplicate_snippets(
                                                BTreeSet::from([id]),
                                                window,
                                                cx,
                                            );
                                        }
                                    });
                                }),
                        )
                    })
                    .item(
                        MenuItem::localized("Delete")
                            .icon(IconName::Trash)
                            .destructive(true)
                            .on_click(move |_, _, cx| {
                                let _ = delete.update(cx, |this, cx| {
                                    if !this.snippet_state.busy
                                        && this.editor.as_ref().is_some_and(|editor| {
                                            editor.fields[0].1.entity_id() == token
                                        })
                                    {
                                        this.selection = BTreeSet::from([id]);
                                        this.selection_kind = Some(kind);
                                        this.delete_pending = true;
                                        cx.notify();
                                    }
                                });
                            }),
                    )
                })
                .into_any_element(),
        ]
    }
    pub(super) fn delete_snippets(
        &mut self,
        packages: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids = self.selection.clone();
        self.mutate_snippets(
            move |catalog| {
                catalog.delete(&ids, packages)?;
                Ok(None)
            },
            |this, _, _, _| {
                this.selection.clear();
                this.retire_editor();
                this.delete_pending = false;
            },
            window,
            cx,
        );
    }
    pub(super) fn save_snippet_package(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        let id = editor.id;
        let field = editor.fields[0].1.entity_id();
        let name = editor.fields[0].1.read(cx).value().to_string();
        if name.trim().is_empty() {
            self.editor.as_mut().unwrap().validation_error = Some("Name is required.".into());
            cx.notify();
            return;
        }
        self.mutate_snippets(
            move |catalog| catalog.save_package(id, name).map(Some),
            move |this, id, window, cx| {
                if this.editor.as_ref().is_some_and(|e| {
                    e.kind == ItemKind::SnippetPackage && e.fields[0].1.entity_id() == field
                }) && let Some(id) = id
                {
                    this.complete_resource_editor(ItemKind::SnippetPackage, id, window, cx);
                }
            },
            window,
            cx,
        );
    }
    pub(super) fn run_snippet_locally(
        &mut self,
        id: Id,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(snippet) = self.data.snippets.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        if let Err(error) = snippet.validate() {
            self.notice = Some(error.to_string());
            cx.notify();
            return;
        }
        if let Some(terminal) =
            self.open_local_terminal_in(None, self.data.selected_workspace, false, window, cx)
        {
            self.retire_editor();
            if let Err(error) = terminal.update(cx, |terminal, cx| {
                terminal.apply_snippet(&snippet.command, true, window, cx)
            }) {
                self.notice = Some(error.to_string());
            }
        }
        cx.notify();
    }
    pub(super) fn snippet_editor_sections(
        &self,
        editor: &Editor,
        draft: &SnippetDraft,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let palette = Palette::new(self.data.dark);
        let token = draft.script.entity_id();
        let target_list = |groups: bool, cx: &mut Context<Self>| {
            let mut items: Vec<_> = if groups {
                self.data
                    .groups
                    .iter()
                    .map(|g| (g.id, g.name.clone(), None::<String>))
                    .collect()
            } else {
                self.data
                    .hosts
                    .iter()
                    .map(|h| {
                        (
                            h.id,
                            h.name.clone(),
                            Some(if h.is_remote() {
                                format!(
                                    "{}@{}:{}",
                                    if h.username.trim().is_empty() {
                                        "ssh"
                                    } else {
                                        &h.username
                                    },
                                    h.address,
                                    h.port
                                )
                            } else {
                                "Local".into()
                            }),
                        )
                    })
                    .collect()
            };
            let selected = if groups { &draft.groups } else { &draft.hosts };
            // Deleted targets stay targeted, rather than silently becoming
            // global. Keep an explicit removable row for these saved IDs.
            for id in selected {
                if !items.iter().any(|(item, _, _)| item == id) {
                    items.push((
                        *id,
                        format!(
                            "Unavailable {} (#{id})",
                            if groups { "group" } else { "host" }
                        ),
                        None,
                    ));
                }
            }
            let count = items.len();
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeights::MEDIUM)
                        .child(crate::localization::view(if groups {
                            "Groups"
                        } else {
                            "Hosts"
                        })),
                )
                .child(
                    div()
                        .border_1()
                        .border_color(palette.border)
                        .rounded(px(8.))
                        .overflow_hidden()
                        .when(items.is_empty(), |view| {
                            view.child(div().p(px(12.)).text_color(palette.muted).child(
                                crate::localization::view(if groups {
                                    "No groups"
                                } else {
                                    "No hosts"
                                }),
                            ))
                        })
                        .children(items.into_iter().enumerate().map(
                            |(index, (id, label, subtitle))| {
                                div()
                                    .when(index + 1 < count, |row| {
                                        row.border_b_1().border_color(palette.border)
                                    })
                                    .child(
                                        Checkbox::new(
                                            SharedString::from(format!(
                                                "snippet-target-{groups}-{id}"
                                            )),
                                            label,
                                            selected.contains(&id),
                                        )
                                        .list_row()
                                        .when_some(subtitle, |checkbox, subtitle| {
                                            checkbox.description(subtitle)
                                        })
                                        .disabled(self.snippet_state.busy)
                                        .on_change(
                                            cx.listener(move |this, checked: &bool, _, cx| {
                                                if this.snippet_state.busy {
                                                    return;
                                                }
                                                if let Some(draft) = this
                                                    .editor
                                                    .as_mut()
                                                    .and_then(|editor| editor.snippet.as_mut())
                                                    .filter(|draft| {
                                                        draft.script.entity_id() == token
                                                    })
                                                {
                                                    let ids = if groups {
                                                        &mut draft.groups
                                                    } else {
                                                        &mut draft.hosts
                                                    };
                                                    if *checked {
                                                        ids.insert(id);
                                                    } else {
                                                        ids.remove(&id);
                                                    }
                                                    cx.notify();
                                                }
                                            }),
                                        ),
                                    )
                            },
                        )),
                )
                .into_any_element()
        };
        vec![
            self.editor_section(
                "Snippet",
                vec![
                    draft.package_field(self.snippet_state.busy),
                    self.editor_field(editor, 0, "Description"),
                    Field::multiline_auto(&draft.script, "Script *")
                        .disabled(self.snippet_state.busy)
                        .error(
                            editor
                                .validation_error
                                .clone()
                                .filter(|error| error.starts_with("Script"))
                                .map(Into::into),
                        )
                        .into_any_element(),
                ],
            ),
            self.editor_section(
                "Targets",
                vec![target_list(true, cx), target_list(false, cx)],
            ),
        ]
    }
    pub(super) fn apply_library_snippet(
        &mut self,
        tab_id: u64,
        session_id: u64,
        snippet_id: Id,
        run: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(
                self.page == Page::Sessions
                    && !self.overview
                    && self.active_terminal() == Some(tab_id),
                "The target terminal changed. Try again."
            );
            let tab = self
                .terminals
                .iter()
                .find(|tab| tab.id == tab_id)
                .ok_or_else(|| anyhow::anyhow!("The target terminal was closed."))?;
            anyhow::ensure!(
                !tab.show_sftp && tab.active().id == session_id,
                "The target pane changed. Try again."
            );
            let session = tab.active();
            let host = match session.restore_target {
                Some(
                    RestoreTarget::Host { id, .. }
                    | RestoreTarget::TelnetHost { id, .. }
                    | RestoreTarget::MoshHost { id, .. },
                ) => Some(id),
                _ => None,
            };
            let snippet = self
                .data
                .snippets
                .iter()
                .find(|s| s.id == snippet_id)
                .ok_or_else(|| anyhow::anyhow!("Snippet is no longer available."))?;
            anyhow::ensure!(
                snippet.matches_host(host, &self.data),
                "This snippet is not available for the target host."
            );
            let TabContent::Terminal(terminal) = &session.content else {
                anyhow::bail!("The target terminal is not connected.");
            };
            terminal.update(cx, |terminal, cx| {
                terminal.apply_snippet(&snippet.command, run, window, cx)
            })
        })();
        if let Err(error) = result {
            self.notice = Some(error.to_string());
        }
        cx.notify();
    }
    pub(super) fn snippet_library(&self, tab: &TerminalTab, cx: &mut Context<Self>) -> AnyElement {
        if !self.snippet_state.ready {
            return self.snippet_loading(cx);
        }
        if let Some(editor) = &tab.tools.snippet_editor {
            return self.inline_snippet_editor(tab, editor, cx);
        }
        let p = Palette::terminal(tab.active().appearance(&self.data).palette());
        let session = tab.active();
        let host = Self::inline_origin(tab).host;
        let query = tab
            .tools
            .snippet_search
            .as_ref()
            .map(|search| search.read(cx).value().to_string())
            .unwrap_or_default();
        let mut snippets: Vec<_> = self
            .data
            .snippets
            .iter()
            .filter(|s| s.matches_host(host, &self.data) && s.matches_query(&query))
            .collect();
        let sort = tab.tools.snippet_sort;
        snippets.sort_by(|a, b| sort.compare((&a.name, a.id), (&b.name, b.id)));
        let ready = matches!(&session.content, TabContent::Terminal(terminal)
            if terminal.read(cx).can_run_history());
        let tab_id = tab.id;
        let session_id = session.id;
        let sort_owner = cx.weak_entity();
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(12.))
                    .pt(px(12.))
                    .pb(px(10.))
                    .border_b_1()
                    .border_color(p.border)
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(13.))
                                    .font_weight(FontWeights::SEMIBOLD)
                                    .child(crate::localization::text(cx, "common.label.snippets")),
                            )
                            .child(
                                Button::new("create-tool-snippet")
                                    .xsmall()
                                    .h(px(28.))
                                    .px(px(8.))
                                    .custom(
                                        ButtonCustomVariant::for_surface(p).color(p.control.into()),
                                    )
                                    .label("New snippet")
                                    .icon(crate::assets::icon(IconName::Plus))
                                    .disabled(self.snippet_state.busy)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.create_inline_snippet(tab_id, session_id, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("sort-tool-snippets")
                                    .ghost()
                                    .size(px(28.))
                                    .p_0()
                                    .custom(
                                        ButtonCustomVariant::for_surface(p)
                                            .foreground(p.muted.into()),
                                    )
                                    .icon(sort.icon())
                                    .tooltip(format!("Sort: {}", sort.label()))
                                    .dropdown_menu(move |menu, _, _| {
                                        let owner = sort_owner.clone();
                                        crate::management::sort_menu(
                                            menu.palette(p).width(190.),
                                            sort,
                                            move |order, _, cx| {
                                                let _ = owner.update(cx, |this, cx| {
                                                    if let Some(tab) = this
                                                        .terminals
                                                        .iter_mut()
                                                        .find(|tab| tab.id == tab_id)
                                                    {
                                                        tab.tools.snippet_sort = order;
                                                        tab.tools
                                                            .snippet_scroll
                                                            .set_offset(point(px(0.), px(0.)));
                                                        cx.notify();
                                                    }
                                                });
                                            },
                                        )
                                    }),
                            ),
                    )
                    .when_some(tab.tools.snippet_search.clone(), |view, search| {
                        view.child(
                            Input::new(&search)
                                .palette(p)
                                .h(px(34.))
                                .text_size(px(12.))
                                .aria_label(crate::localization::text(cx, "Search snippets"))
                                .prefix(icon(IconName::Search).size(px(16.))),
                        )
                    }),
            )
            .child(
                div()
                    .id("snippet-library-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&tab.tools.snippet_scroll)
                    .scrollbars_with(&tab.tools.snippet_scroll)
                    .px(px(8.))
                    .pt(px(8.))
                    .pb(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .when(snippets.is_empty(), |view| {
                        view.child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(12.))
                                .line_height(relative(1.5))
                                .font_weight(FontWeights::MEDIUM)
                                .text_color(p.muted)
                                .child(crate::localization::view(if query.trim().is_empty() {
                                    "No snippets available."
                                } else {
                                    "No matching results."
                                })),
                        )
                    })
                    .children(snippets.into_iter().map(|snippet| {
                        let id = snippet.id;
                        let package = snippet
                            .package_id
                            .and_then(|id| {
                                self.snippet_state
                                    .catalog
                                    .packages
                                    .iter()
                                    .find(|package| package.id == id)
                            })
                            .map(|p| p.name.clone());
                        div()
                            .id(("tool-snippet-row", id))
                            .relative()
                            .flex_shrink_0()
                            .rounded(px(8.))
                            .bg(p.control.alpha(0.52))
                            .hover(|v| v.bg(p.control))
                            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                if event.click_count() == 2 && ready {
                                    this.apply_library_snippet(
                                        tab_id, session_id, id, true, window, cx,
                                    );
                                }
                            }))
                            .px(px(10.))
                            .py(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .line_height(relative(1.5))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeights::MEDIUM)
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(snippet.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(p.muted)
                                            .font_family("monospace")
                                            .line_clamp(2)
                                            .child(snippet.command.clone()),
                                    )
                                    .when_some(package, |view, package| {
                                        view.child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(p.muted)
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .child(package),
                                        )
                                    }),
                            )
                            .child(
                                Button::new(("snippet-prompt", id))
                                    .ghost()
                                    .size(px(25.))
                                    .p_0()
                                    .custom(
                                        ButtonCustomVariant::for_surface(p)
                                            .foreground(p.muted.into()),
                                    )
                                    .icon(IconName::Copy)
                                    .tooltip("Copy to terminal prompt")
                                    .disabled(!ready)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.apply_library_snippet(
                                            tab_id, session_id, id, false, window, cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new(("snippet-run", id))
                                    .ghost()
                                    .size(px(26.))
                                    .p_0()
                                    .custom(
                                        ButtonCustomVariant::for_surface(p)
                                            .foreground(p.accent.into()),
                                    )
                                    .icon(IconName::Play)
                                    .tooltip("Run snippet")
                                    .disabled(!ready)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.apply_library_snippet(
                                            tab_id, session_id, id, true, window, cx,
                                        )
                                    })),
                            )
                    })),
            )
            .into_any_element()
    }
    pub(super) fn snippet_collection(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        if !self.snippet_state.ready {
            return self.snippet_loading(cx);
        }
        let p = Palette::new(self.data.dark);
        let query = self.searches[&Page::Snippets]
            .read(cx)
            .value()
            .to_lowercase();
        let package = self.snippet_state.package.filter(|id| {
            self.snippet_state
                .catalog
                .packages
                .iter()
                .any(|p| p.id == *id)
        });
        let mut packages: Vec<_> = self
            .snippet_state
            .catalog
            .packages
            .iter()
            .filter(|p| package.is_none() && p.name.to_lowercase().contains(query.trim()))
            .collect();
        let mut snippets: Vec<_> = self
            .data
            .snippets
            .iter()
            .filter(|s| {
                package.is_none_or(|id| s.package_id == Some(id)) && s.matches_query(&query)
            })
            .collect();
        let preferences = self.collection_view();
        packages.sort_by(|a, b| preferences.sort.compare((&a.name, a.id), (&b.name, b.id)));
        snippets.sort_by(|a, b| preferences.sort.compare((&a.name, a.id), (&b.name, b.id)));
        if packages.is_empty() && snippets.is_empty() && package.is_none() {
            let empty = crate::components::EmptyState::new(
                IconName::Code,
                if query.is_empty() {
                    "No snippets yet"
                } else {
                    "No snippets found"
                },
                self.data.dark,
            )
            .description(if query.is_empty() {
                "Save frequently used commands as snippets so they are ready in every terminal."
            } else {
                "Try another search."
            })
            .on_click(cx.listener(|this, _, window, cx| {
                this.retire_editor();
                this.selection.clear();
                this.selection_kind = None;
                this.focus.focus(window, cx);
                cx.notify();
            }));
            return if query.is_empty() {
                empty
                    .action(
                        Button::new("empty-add-snippet")
                            .primary()
                            .icon(crate::assets::icon(IconName::Plus))
                            .label("Add snippet")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_editor(ItemKind::Snippet, None, window, cx)
                            })),
                    )
                    .into_any_element()
            } else {
                empty.into_any_element()
            };
        }
        let grid = layout::CardGrid::new(
            width - 52.,
            packages.len().max(snippets.len()),
            preferences.list,
        );
        div()
            .id("collection-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .scrollbars()
            .map(crate::components::management::collection_padding)
            .flex()
            .flex_col()
            .gap(px(18.))
            .on_click(cx.listener(|this, _, window, cx| {
                this.selection.clear();
                this.selection_kind = None;
                this.retire_editor();
                this.focus.focus(window, cx);
                cx.notify();
            }))
            .when_some(package, |view, id| {
                view.child(
                    Breadcrumb::new(
                        "snippet-breadcrumb",
                        [
                            (None, crate::localization::text(cx, "Snippets")),
                            (
                                Some(id),
                                self.snippet_state
                                    .catalog
                                    .packages
                                    .iter()
                                    .find(|p| p.id == id)
                                    .map(|p| p.name.as_str())
                                    .unwrap_or("Package")
                                    .to_owned()
                                    .into(),
                            ),
                        ],
                        p,
                    )
                    .on_select(cx.listener(|this, _, window, cx| {
                        this.open_snippet_package(None, window, cx);
                    })),
                )
            })
            .child(
                div()
                    .min_w_0()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(crate::components::management::COLLECTION_SECTION_GAP))
                    .when(!packages.is_empty(), |view| {
                        view.child(
                            crate::components::management::collection_section("Packages", cx)
                                .child(crate::components::card_grid(grid).children(
                                    packages.iter().map(|package| {
                                        self.card(
                                            ItemKind::SnippetPackage,
                                            package.id,
                                            package.name.clone(),
                                            format!(
                                                "{} Snippets",
                                                self.data
                                                    .snippets
                                                    .iter()
                                                    .filter(|s| s.package_id == Some(package.id))
                                                    .count()
                                            ),
                                            cx,
                                        )
                                    }),
                                )),
                        )
                    })
                    .when(!snippets.is_empty(), |view| {
                        view.child(
                            crate::components::management::collection_section("Snippets", cx)
                                .child(crate::components::card_grid(grid).children(
                                    snippets.iter().map(|s| {
                                        self.card(
                                            ItemKind::Snippet,
                                            s.id,
                                            s.name.clone(),
                                            s.command.clone(),
                                            cx,
                                        )
                                    }),
                                )),
                        )
                    })
                    .when(packages.is_empty() && snippets.is_empty(), |view| {
                        view.child(div().text_color(p.muted).child(crate::localization::view(
                            if query.is_empty() {
                                "No snippets yet. Save frequently used commands for your terminals."
                            } else {
                                "No snippets found."
                            },
                        )))
                    }),
            )
            .into_any_element()
    }
}
