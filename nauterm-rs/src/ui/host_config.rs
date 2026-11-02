use super::*;
use gpui_kit::base::input::InputState;

pub(super) struct Draft {
    pub automatic_name: Option<super::host_name::Draft>,
    pub kind: HostKind,
    pub telnet: super::telnet_config::Draft,
    pub mosh: super::mosh_config::Draft,
    pub shell: Option<String>,
    pub directory: Entity<InputState>,
    pub setup: crate::host_session::Setup,
    pub environment: Option<Vec<crate::host_session::Variable>>,
    pub environment_changed: bool,
    pub environment_page: Option<super::host_environment::EnvironmentEditor>,
    pub busy: bool,
    pub ssh_enabled: bool,
    pub ssh_configured: bool,
    pub inheritance: crate::host_session::Inheritance,
    pub environment_merges: Vec<String>,
    pub inheritance_generation: u64,
    pub inheriting: bool,
    pub inheritance_error: Option<String>,
    more: bool,
    startup: SelectModel<Id>,
    encoding: SelectModel<crate::host_session::Encoding>,
    types: SelectModel<HostKind>,
    shells: SelectModel<String>,
    _events: Vec<Subscription>,
}
fn startup_options(snippets: &[Snippet], selected: Option<Id>) -> Vec<SelectOption<Id>> {
    let mut options = vec![];
    if let Some(id) = selected.filter(|id| !snippets.iter().any(|snippet| snippet.id == *id)) {
        options.push(SelectOption::new(
            id,
            format!("Unavailable snippet (#{id})"),
        ));
    }
    options.extend(
        snippets
            .iter()
            .map(|snippet| SelectOption::new(snippet.id, snippet.name.clone())),
    );
    options
}
fn shell_options(
    shells: Vec<crate::shells::Shell>,
    selected: Option<&str>,
) -> Vec<SelectOption<String>> {
    let mut options = vec![];
    if let Some(selected) = selected.filter(|path| !shells.iter().any(|shell| shell.path == *path))
    {
        options.push(SelectOption::new(selected.into(), selected.to_owned()));
    }
    options.extend(
        shells
            .into_iter()
            .map(|shell| SelectOption::new(shell.path.clone(), shell.path)),
    );
    options
}
impl Draft {
    pub fn new(
        host: Option<&Host>,
        snippets: &[Snippet],
        inventory: &crate::vault::Inventory,
        window: &mut Window,
        cx: &mut Context<Nauterm>,
    ) -> Self {
        let kind = host.map(|host| host.kind).unwrap_or_default();
        let shell = host.and_then(|host| host.shell_path.clone());
        let setup = host.map(|host| host.ssh.clone()).unwrap_or_default();
        let startup = SelectModel::new(
            startup_options(snippets, setup.startup_snippet),
            setup.startup_snippet,
            window,
            cx,
        );
        let encoding = SelectModel::new(
            crate::host_session::Encoding::ALL
                .into_iter()
                .map(|encoding| SelectOption::localized(encoding, encoding.label()))
                .collect(),
            Some(setup.encoding),
            window,
            cx,
        );
        let startup_event = cx.subscribe(
            &startup.state,
            |this, state, event: &SelectEvent<Id>, cx| {
                let SelectEvent::Confirm(value) = event;
                if let Some(draft) = this.editor.as_mut().and_then(|editor| editor.host.as_mut())
                    && draft.startup.state.entity_id() == state.entity_id()
                {
                    draft.setup.startup_snippet = value.flatten();
                    cx.notify();
                }
            },
        );
        let startup_create = cx.subscribe_in(
            &startup.state,
            window,
            |this, state, event: &crate::components::SelectCreate, window, cx| {
                if event.action == "snippet"
                    && this
                        .editor
                        .as_ref()
                        .and_then(|editor| editor.host.as_ref())
                        .is_some_and(|draft| draft.startup.state == *state)
                {
                    let focus = state.read(cx).focus_handle(cx);
                    this.open_related_resource(
                        related::Target::StartupSnippet,
                        &event.text,
                        focus,
                        window,
                        cx,
                    );
                }
            },
        );
        let encoding_event = cx.subscribe(
            &encoding.state,
            |this, state, event: &SelectEvent<crate::host_session::Encoding>, cx| {
                if let Some(draft) = this.editor.as_mut().and_then(|editor| editor.host.as_mut())
                    && draft.encoding.state.entity_id() == state.entity_id()
                    && let SelectEvent::Confirm(Some(Some(value))) = event
                {
                    draft.setup.encoding = *value;
                    draft.inheritance.encoding_configured = true;
                    cx.notify();
                }
            },
        );
        let types = SelectModel::new(
            [HostKind::Remote, HostKind::Local]
                .into_iter()
                .map(|kind| SelectOption::localized(kind, kind.label()))
                .collect(),
            Some(kind),
            window,
            cx,
        );
        let shells = SelectModel::new(
            shell_options(vec![], shell.as_deref()),
            shell.clone(),
            window,
            cx,
        );
        let type_event = cx.subscribe(
            &types.state,
            |this, state, event: &SelectEvent<HostKind>, cx| {
                if let Some(draft) = this.editor.as_mut().and_then(|editor| editor.host.as_mut())
                    && draft.types.state.entity_id() == state.entity_id()
                    && let SelectEvent::Confirm(Some(Some(kind))) = event
                {
                    draft.kind = *kind;
                    if *kind == HostKind::Local {
                        draft.cancel_inheritance();
                    }
                    cx.notify();
                }
            },
        );
        let shell_event = cx.subscribe(
            &shells.state,
            |this, state, event: &SelectEvent<String>, cx| {
                let SelectEvent::Confirm(value) = event;
                if let Some(draft) = this.editor.as_mut().and_then(|editor| editor.host.as_mut())
                    && draft.shells.state.entity_id() == state.entity_id()
                {
                    draft.shell = value.clone().flatten();
                    cx.notify();
                }
            },
        );
        let target = shells.state.clone();
        cx.spawn_in(window, async move |this, cx| {
            let available = cx
                .background_spawn(async { crate::shells::discover() })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Some(draft) = this
                    .find_editor_mut(&|editor| {
                        editor
                            .host
                            .as_ref()
                            .is_some_and(|draft| draft.shells.state == target)
                    })
                    .and_then(|editor| editor.host.as_mut())
                {
                    draft.shells.reconcile(
                        shell_options(available, draft.shell.as_deref()),
                        draft.shell.clone(),
                        window,
                        cx,
                    );
                    cx.notify();
                }
            });
        })
        .detach();
        Self {
            automatic_name: None,
            ssh_enabled: host.is_none_or(|host| host.connections.ssh),
            ssh_configured: host.is_some(),
            telnet: super::telnet_config::Draft::new(host, inventory, window, cx),
            mosh: super::mosh_config::Draft::new(host, window, cx),
            inheritance: crate::host_session::Inheritance {
                port_configured: host.is_some(),
                encoding_configured: host.is_some(),
                password_configured: false,
            },
            environment_merges: vec![],
            inheritance_generation: 0,
            inheriting: false,
            inheritance_error: None,
            environment: setup.environment_id.is_none().then(Vec::new),
            setup,
            startup,
            encoding,
            more: false,
            busy: false,
            environment_changed: false,
            environment_page: None,
            kind,
            shell,
            types,
            shells,
            directory: cx.new(|cx| {
                InputState::new(window, cx).default_value(
                    host.and_then(|host| host.work_dir.clone())
                        .unwrap_or_default(),
                )
            }),
            _events: vec![
                type_event,
                shell_event,
                startup_event,
                startup_create,
                encoding_event,
            ],
        }
    }
    pub fn values(&self, cx: &App) -> (HostKind, Option<String>, Option<String>) {
        if self.kind == HostKind::Remote {
            return (self.kind, None, None);
        }
        let directory = self.directory.read(cx).value().trim().to_owned();
        (
            self.kind,
            self.shell.clone(),
            (!directory.is_empty()).then_some(directory),
        )
    }
    pub fn validate(&self, cx: &App) -> anyhow::Result<()> {
        if self.kind == HostKind::Local {
            let (kind, shell_path, work_dir) = self.values(cx);
            Host {
                kind,
                shell_path,
                work_dir,
                ..Default::default()
            }
            .validate_endpoint()?;
            return Ok(());
        }
        self.telnet.validate(cx)?;
        if !self.ssh_enabled {
            return Ok(());
        }
        anyhow::ensure!(
            !self.inheriting && self.inheritance_error.is_none(),
            "Resolve the Group environment loading error before saving."
        );
        self.mosh.value(cx)?;
        self.setup.validate()?;
        if let Some(environment) = &self.environment {
            crate::host_session::validate_environment(environment)?;
        }
        Ok(())
    }
    pub fn reconcile(&mut self, snippets: &[Snippet], window: &mut Window, cx: &mut App) {
        self.mosh.reconcile(window, cx);
        self.encoding.reconcile(
            crate::host_session::Encoding::ALL
                .into_iter()
                .map(|encoding| SelectOption::localized(encoding, encoding.label()))
                .collect(),
            Some(self.setup.encoding),
            window,
            cx,
        );
        self.startup.reconcile(
            startup_options(snippets, self.setup.startup_snippet),
            self.setup.startup_snippet,
            window,
            cx,
        );
    }
    pub fn cancel_inheritance(&mut self) {
        self.inheritance_generation = self.inheritance_generation.wrapping_add(1);
        self.inheriting = false;
        self.inheritance_error = None;
        self.environment_merges.clear();
    }
    pub fn watch_port(&mut self, input: &Entity<InputState>, cx: &mut Context<Nauterm>) {
        self._events
            .push(cx.subscribe(input, |this, input, event: &InputEvent, _| {
                if matches!(event, InputEvent::Change)
                    && let Some(editor) = this.editor.as_mut().filter(|editor| {
                        editor
                            .fields
                            .get(2)
                            .is_some_and(|(_, field)| field.entity_id() == input.entity_id())
                    })
                    && let Some(draft) = &mut editor.host
                {
                    draft.inheritance.port_configured = true;
                }
            }));
    }
}

impl Nauterm {
    pub(super) fn host_editor_actions(
        &self,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(id) = editor.id.filter(|_| editor.kind == ItemKind::Host) else {
            return vec![];
        };
        let owner = cx.weak_entity();
        vec![
            Button::new("host-editor-actions")
                .toolbar()
                .ghost()
                .disabled(editor.host.as_ref().is_some_and(|draft| draft.busy))
                .icon(IconName::Ellipsis)
                .tooltip("Host actions")
                .dropdown_menu(move |menu, _, _| {
                    let connect = owner.clone();
                    let duplicate = owner.clone();
                    let delete = owner.clone();
                    menu.item(
                        MenuItem::localized("Connect")
                            .icon(IconName::Terminal)
                            .on_click(move |_, window, cx| {
                                let _ = connect.update(cx, |this, cx| {
                                    if let Some(host) =
                                        this.data.hosts.iter().find(|host| host.id == id).cloned()
                                    {
                                        this.open_ssh(host, window, cx);
                                    }
                                });
                            }),
                    )
                    .item(
                        MenuItem::localized("Duplicate")
                            .icon(IconName::Copy)
                            .on_click(move |_, _, cx| {
                                let _ = duplicate.update(cx, |this, cx| {
                                    this.selection = BTreeSet::from([id]);
                                    this.selection_kind = Some(ItemKind::Host);
                                    this.duplicate_resources(ItemKind::Host, cx);
                                });
                            }),
                    )
                    .separator()
                    .item(
                        MenuItem::localized("Delete")
                            .icon(IconName::Trash)
                            .destructive(true)
                            .on_click(move |_, _, cx| {
                                let _ = delete.update(cx, |this, cx| {
                                    this.selection = BTreeSet::from([id]);
                                    this.selection_kind = Some(ItemKind::Host);
                                    this.delete_pending = true;
                                    cx.notify();
                                });
                            }),
                    )
                })
                .into_any_element(),
        ]
    }
    pub(super) fn host_editor_sections(
        &self,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let draft = editor
            .host
            .as_ref()
            .expect("Host editor owns its configuration draft");
        let local = draft.kind == HostKind::Local;
        let endpoint = if local {
            draft.shells.filterable_field("Shell", draft.busy)
        } else {
            self.editor_field(editor, 1, "Host")
        };
        let mut sections = vec![
            self.editor_section(
                if local { "Shell" } else { "Address" },
                vec![
                    div()
                        .flex()
                        .items_start()
                        .gap(px(12.))
                        .child(
                            div()
                                .size(px(40.))
                                .flex_shrink_0()
                                .rounded(px(10.))
                                .bg(rgb(if local { 0x075e92 } else { 0xff5425 }))
                                .text_color(rgb(0xffffff))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon(if local {
                                    IconName::Terminal
                                } else {
                                    IconName::Globe
                                })),
                        )
                        .child(div().flex_1().min_w_0().child(endpoint))
                        .into_any_element(),
                ],
            ),
            self.editor_section(
                "General",
                vec![
                    self.editor_field(editor, 0, "Name *"),
                    draft.types.field("Type", draft.busy),
                    self.parent_group_field(editor),
                    editor
                        .tags
                        .as_ref()
                        .expect("Host owns tag selection")
                        .view
                        .clone()
                        .into_any_element(),
                ],
            ),
        ];
        if local {
            sections.push(self.editor_section("Theme", vec![self.host_theme_section(editor, cx)]));
            sections.push(self.editor_section(
                "Local",
                vec![
                        Field::input(&draft.directory, "Work dir")
                            .disabled(draft.busy)
                            .into_any_element(),
                    ],
            ));
        } else {
            if draft.ssh_enabled {
                sections.push(self.ssh_editor_section(editor, cx));
            }
            if draft.telnet.enabled {
                sections.push(self.telnet_editor_section(editor, cx));
            }
            sections.push(self.add_protocol_control(editor, cx));
        }
        sections
    }
    pub(super) fn ssh_editor_section(&self, editor: &Editor, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.data.dark);
        let draft = editor.host.as_ref().unwrap();
        let selects = editor.selects.as_ref().unwrap();
        let mut fields = self.host_authentication_fields(editor, cx);
        if draft.more {
            fields.push(draft.startup.field_with_actions(
                "Startup snippet",
                draft.busy || !self.snippet_state.ready || self.snippet_state.busy,
                vec![crate::components::SelectAction::new(
                    "snippet",
                    "Create snippet",
                    IconName::Code,
                )],
            ));
            fields.push(selects.proxies.field_with_actions(
                "Proxy",
                draft.busy,
                vec![crate::components::SelectAction::new(
                    "proxy",
                    "Create proxy",
                    IconName::Network,
                )],
            ));
            let summary = draft
                .environment
                .as_ref()
                .map(|entries| {
                    entries
                        .iter()
                        .map(|entry| format!("{}={}", entry.variable, entry.value))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|| {
                    if draft.setup.environment_id.is_some() {
                        crate::localization::text(cx, "Configured").to_string()
                    } else {
                        String::new()
                    }
                });
            fields.push(
                Field::trigger(
                    Button::new("edit-host-environment")
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .line_clamp(1)
                                .child(summary.clone()),
                        )
                        .trailing_icon(icon(IconName::ChevronRight).text_color(p.muted))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_host_environment(window, cx)
                        })),
                    "Environment variables",
                    !summary.is_empty() || draft.setup.environment_id.is_some(),
                )
                .disabled(draft.busy || draft.inheriting)
                .into_any_element(),
            );
            fields.push(draft.encoding.field("Encoding", draft.busy));
            fields.extend(draft.mosh.fields(draft.busy));
        }
        fields.push(
            div()
                .mt(px(2.))
                .pt(px(12.))
                .border_t_1()
                .border_color(p.border)
                .child(self.host_theme_section(editor, cx))
                .into_any_element(),
        );
        if !draft.more {
            fields.push(
                Button::new("host-ssh-show-more")
                    .small()
                    .ghost()
                    .w_full()
                    .label("Show more")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(draft) =
                            this.editor.as_mut().and_then(|editor| editor.host.as_mut())
                        {
                            draft.more = true;
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        div()
            .rounded(px(10.))
            .bg(p.surface)
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.protocol_editor_header(editor, super::host_credentials::Target::Ssh, cx))
            .children(fields)
            .into_any_element()
    }

    pub(super) fn protocol_editor_header(
        &self,
        editor: &Editor,
        target: super::host_credentials::Target,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use super::host_credentials::Target;
        let draft = editor.host.as_ref().unwrap();
        let p = Palette::new(self.data.dark);
        let (title, port_label, port) = match target {
            Target::Ssh => ("SSH on", "SSH port", &editor.fields[2].1),
            Target::Telnet => ("Telnet on", "Telnet port", &draft.telnet.port),
        };
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .pb(px(12.))
            .border_b_1()
            .border_color(p.border)
            .text_size(px(13.))
            .line_height(px(18.))
            .font_weight(FontWeights::SEMIBOLD)
            .child(crate::localization::view(title))
            .child(
                Input::new(port)
                    .digits_only()
                    .small()
                    .w(px(64.))
                    .font_weight(FontWeights::REGULAR)
                    .disabled(draft.busy)
                    .aria_label(port_label),
            )
            .child(crate::localization::text(cx, "common.label.port"))
            .child(div().flex_1())
            .child(self.protocol_actions(editor, target, cx))
            .into_any_element()
    }
}
