use super::*;
use crate::port_forward::{self, Kind, State, Status};
use crate::typography::FontWeights;
use anyhow::{Context as _, Result, ensure};
mod mutations;

pub(super) use crate::port_forward::Runtime;
pub(super) struct ForwardSelects {
    kind: Kind,
    host: Option<Id>,
    types: SelectModel<Kind>,
    hosts: SelectModel<Id>,
    pub(super) errors: [Option<SharedString>; 5],
    pub(super) host_error: Option<SharedString>,
    saving: bool,
    _events: Vec<Subscription>,
}
fn host_options(hosts: &[Host]) -> Vec<SelectOption<Id>> {
    hosts
        .iter()
        .filter(|host| host.supports_ssh())
        .map(|host| {
            SelectOption::new(
                host.id,
                format!("{} ({}:{})", host.name, host.address, host.port),
            )
        })
        .collect()
}
impl ForwardSelects {
    pub fn saving(&self) -> bool {
        self.saving
    }
    pub fn reconcile(&mut self, hosts: &[Host], window: &mut Window, cx: &mut App) {
        self.hosts
            .reconcile(host_options(hosts), self.host, window, cx);
    }
}
impl Nauterm {
    pub(super) fn create_forward(
        &mut self,
        kind: Kind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_editor(ItemKind::Forward, None, window, cx);
        if let Some(forward) = self
            .editor
            .as_mut()
            .filter(|editor| editor.kind == ItemKind::Forward && editor.id.is_none())
            .and_then(|editor| editor.forward.as_mut())
        {
            forward.kind = kind;
            forward.types.reconcile(
                Kind::ALL
                    .into_iter()
                    .map(|kind| SelectOption::localized(kind, kind.label()))
                    .collect(),
                Some(kind),
                window,
                cx,
            );
        }
        cx.notify();
    }
    pub(super) fn forward_editor_actions(
        &self,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(id) = editor.id.filter(|_| editor.kind == ItemKind::Forward) else {
            return vec![];
        };
        let busy = self.forward_mutations.contains(&id);
        let token = editor.fields[0].1.entity_id();
        let owner = cx.weak_entity();
        vec![
            Button::new("forward-editor-actions")
                .toolbar()
                .ghost()
                .icon(IconName::Ellipsis)
                .tooltip("Forwarding actions")
                .dropdown_menu(move |menu, _, _| {
                    let owner = owner.clone();
                    menu.item(
                        MenuItem::localized("Delete")
                            .icon(IconName::Trash)
                            .destructive(true)
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    if this.forward_mutations.contains(&id)
                                        || !this.data.forwards.iter().any(|rule| rule.id == id)
                                        || !this.editor.as_ref().is_some_and(|editor| {
                                            editor.fields[0].1.entity_id() == token
                                        })
                                    {
                                        return;
                                    }
                                    this.selection = BTreeSet::from([id]);
                                    this.selection_kind = Some(ItemKind::Forward);
                                    this.delete_pending = true;
                                    cx.notify();
                                });
                            }),
                    )
                })
                .into_any_element(),
        ]
    }
    pub(super) fn active_forward_count(&self) -> usize {
        self.forward_states
            .values()
            .filter(|run| run.status.state.active())
            .count()
    }
    pub(super) fn forward_active(&self, id: Id) -> bool {
        self.forward_states
            .get(&id)
            .is_some_and(|run| run.status.state.active())
    }
    pub(super) fn forward_selects(
        &self,
        id: Option<Id>,
        fields: &[(&'static str, Entity<InputState>)],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ForwardSelects {
        let rule = id.and_then(|id| self.data.forwards.iter().find(|rule| rule.id == id));
        let kind = rule.map_or(Kind::Local, |rule| rule.kind);
        let host = rule.and_then(|rule| rule.host_id);
        let types = SelectModel::new(
            Kind::ALL
                .into_iter()
                .map(|kind| SelectOption::localized(kind, kind.label()))
                .collect(),
            Some(kind),
            window,
            cx,
        );
        let hosts = SelectModel::new(host_options(&self.data.hosts), host, window, cx);
        let type_event = cx.subscribe_in(
            &types.state,
            window,
            |this, state, event: &SelectEvent<Kind>, window, cx| {
                if let Some(editor) = this.editor.as_mut().filter(|editor| {
                    editor
                        .forward
                        .as_ref()
                        .is_some_and(|forward| forward.types.state == *state)
                }) && let SelectEvent::Confirm(Some(Some(kind))) = event
                {
                    let forward = editor.forward.as_mut().unwrap();
                    forward.kind = *kind;
                    if *kind == Kind::Dynamic {
                        editor.fields[3]
                            .1
                            .update(cx, |field, cx| field.set_value("", window, cx));
                        editor.fields[4]
                            .1
                            .update(cx, |field, cx| field.set_value("", window, cx));
                        forward.errors[3] = None;
                        forward.errors[4] = None;
                    }
                    cx.notify();
                }
            },
        );
        let host_event = cx.subscribe_in(
            &hosts.state,
            window,
            |this, state, event: &SelectEvent<Id>, _, cx| {
                if let Some(forward) = this
                    .editor
                    .as_mut()
                    .and_then(|editor| editor.forward.as_mut())
                    && forward.hosts.state == *state
                {
                    let SelectEvent::Confirm(value) = event;
                    forward.host = value.flatten();
                    if forward.host.is_some() {
                        forward.host_error = None;
                    }
                    cx.notify();
                }
            },
        );
        let mut events = vec![type_event, host_event];
        for index in [0, 2, 3, 4] {
            events.push(cx.subscribe(
                &fields[index].1,
                move |this, input, event: &InputEvent, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let Some(editor) = this.editor.as_mut().filter(|editor| {
                        editor
                            .fields
                            .get(index)
                            .is_some_and(|(_, field)| field.entity_id() == input.entity_id())
                    }) else {
                        return;
                    };
                    let text = editor.fields[index].1.read(cx).value();
                    let valid = if matches!(index, 2 | 4) {
                        text.trim().parse::<u16>().is_ok_and(|port| port > 0)
                    } else {
                        !text.trim().is_empty()
                    };
                    if valid && let Some(forward) = &mut editor.forward {
                        forward.errors[index] = None;
                        cx.notify();
                    }
                },
            ));
        }
        ForwardSelects {
            kind,
            host,
            types,
            hosts,
            errors: Default::default(),
            host_error: None,
            saving: false,
            _events: events,
        }
    }
    pub(super) fn save_forward_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(forward) = &editor.forward else {
            return;
        };
        if forward.saving {
            return;
        }
        let values: Vec<_> = editor
            .fields
            .iter()
            .map(|(_, field)| field.read(cx).value().trim().to_owned())
            .collect();
        let valid_port = |text: &str| text.parse::<u16>().is_ok_and(|port| port > 0);
        let mut errors: [Option<SharedString>; 5] = Default::default();
        errors[0] = values[0]
            .is_empty()
            .then(|| "Rule name is required.".into());
        errors[2] = (!valid_port(&values[2])).then(|| {
            if forward.kind == Kind::Remote {
                "Remote port number is required.".into()
            } else {
                "Local port number is required.".into()
            }
        });
        let host_error = (!self
            .data
            .hosts
            .iter()
            .any(|host| host.supports_ssh() && Some(host.id) == forward.host))
        .then(|| {
            if forward.kind == Kind::Remote {
                SharedString::from("Select a remote host.")
            } else {
                SharedString::from("Select an intermediate host.")
            }
        });
        if forward.kind != Kind::Dynamic {
            errors[3] = values[3]
                .is_empty()
                .then(|| "Destination address is required.".into());
            errors[4] =
                (!valid_port(&values[4])).then(|| "Destination port number is required.".into());
        }
        let invalid = host_error.is_some() || errors.iter().any(Option::is_some);
        let editor = self.editor.as_mut().unwrap();
        let forward = editor.forward.as_mut().unwrap();
        forward.errors = errors;
        forward.host_error = host_error;
        if invalid {
            editor.error = None;
            cx.notify();
            return;
        }
        let token = editor.fields[0].1.entity_id();
        let result = (|| -> Result<Forward> {
            ensure!(self.resources_ready(), "Workspace storage is unavailable.");
            let editor = self
                .editor
                .as_ref()
                .context("The editor is no longer available.")?;
            ensure!(
                editor
                    .id
                    .is_none_or(|id| !self.forward_mutations.contains(&id)),
                "This forwarding rule is already being updated."
            );
            let choices = editor
                .forward
                .as_ref()
                .context("Forwarding fields are unavailable.")?;
            let host = self
                .data
                .hosts
                .iter()
                .find(|host| host.supports_ssh() && Some(host.id) == choices.host)
                .context("Select an available SSH host.")?;
            let values: Vec<_> = editor
                .fields
                .iter()
                .map(|(_, input)| input.read(cx).value().trim().to_owned())
                .collect();
            ensure!(!values[0].is_empty(), "Rule name is required.");
            let mut data = self.data.clone();
            let id = editor.id.unwrap_or_else(|| data.allocate_id());
            let destination = if choices.kind == Kind::Dynamic {
                String::new()
            } else {
                port_forward::endpoint(
                    &values[3],
                    values[4]
                        .parse::<u16>()
                        .ok()
                        .filter(|port| *port > 0)
                        .context("Destination port must be between 1 and 65535.")?,
                )
            };
            let rule = Forward {
                id,
                name: values[0].clone(),
                host: host.name.clone(),
                local_port: values[2]
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port > 0)
                    .context("Port must be between 1 and 65535.")?,
                destination,
                kind: choices.kind,
                host_id: Some(host.id),
                bind_address: if values[1].is_empty() {
                    "127.0.0.1".into()
                } else {
                    values[1].clone()
                },
            };
            rule.spec()?;
            Ok(rule)
        })();
        match result {
            Ok(rule) => self.save_forward_after_stop(rule, token, window, cx),
            Err(error) => {
                if let Some(editor) = &mut self.editor {
                    editor.validation_error = Some(error.to_string());
                }
            }
        }
        cx.notify();
    }
    fn ensure_forward_service(&mut self, cx: &mut Context<Self>) -> Result<()> {
        if self.forward_service.is_some() {
            return Ok(());
        }
        if self.core.is_none() {
            self.core = Some(Core::discover()?);
        }
        let service = port_forward::Service::new(self.core.as_ref().unwrap().clone())?;
        let events = service.events.clone();
        self.forward_events = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let updated = this.update(cx, |this, cx| {
                    if let Some(run) = this.forward_states.get_mut(&event.status.id)
                        && run.apply(event)
                    {
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        }));
        self.forward_service = Some(service);
        Ok(())
    }
    fn toggle_forward(&mut self, id: Id, cx: &mut Context<Self>) {
        if self.forward_mutations.contains(&id) {
            return;
        }
        if self.forward_active(id) {
            let generation = self.next_forward_generation;
            self.next_forward_generation += 1;
            let run = self.forward_states.get_mut(&id).unwrap();
            if run.status.state == State::Stopping && run.status.error.is_none() {
                return;
            }
            run.generation = generation;
            run.status.state = State::Stopping;
            run.status.error = None;
            let result = self
                .forward_service
                .as_ref()
                .context("Forwarding service is unavailable.")
                .and_then(|service| service.stop(id, generation));
            if let Err(error) = result {
                run.status.stop_failed(error);
            }
            cx.notify();
            return;
        }
        self.start_forward(id, cx);
    }
    pub(super) fn start_forward(&mut self, id: Id, cx: &mut Context<Self>) {
        // Startup recovery is a start request, never a toggle that could stop a
        // rule already started by an explicit user action.
        if self.exiting.is_some() || self.forward_active(id) || self.forward_mutations.contains(&id)
        {
            return;
        }
        let generation = self.next_forward_generation;
        self.next_forward_generation += 1;
        let prepared = (|| -> Result<_> {
            let rule = self
                .data
                .forwards
                .iter()
                .find(|rule| rule.id == id)
                .context("Forwarding rule is no longer available.")?
                .clone();
            let spec = rule.spec()?;
            let host = self
                .data
                .hosts
                .iter()
                .find(|host| host.supports_ssh() && Some(host.id) == rule.host_id)
                .context("Edit this rule and select an available SSH host.")?
                .clone();
            self.ensure_forward_service(cx)?;
            let vault = if host.credential.is_some()
                || host.proxy_id.is_some()
                || host.ssh.password_id.is_some()
            {
                Some(self.ensure_keychain(cx).read(cx).vault().context(
                    "The saved credential vault is not ready. Retry after Keychain has loaded.",
                )?)
            } else {
                None
            };
            Ok((spec, host, vault))
        })();
        let (spec, host, vault) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.forward_states.insert(
                    id,
                    Runtime {
                        generation,
                        status: Status::failed(id, error),
                    },
                );
                cx.notify();
                return;
            }
        };
        let options = SshOptions::new(
            host.address,
            host.port,
            host.username,
            self.store.paths.known_hosts(),
        );
        self.forward_states.insert(
            id,
            Runtime {
                generation,
                status: Status::new(id, State::Starting),
            },
        );
        cx.spawn(async move |this, cx| {
            let result = match vault {
                Some(vault) => {
                    vault
                        .apply_connection(host.credential, host.proxy_id, host.ssh, options)
                        .await
                }
                None => Ok(options),
            };
            let _ = this.update(cx, |this, cx| {
                // A stop, deletion or subsequent request invalidates credential
                // resolution before it can create a network connection.
                if this.exiting.is_some()
                    || !this.forward_states.get(&id).is_some_and(|run| {
                        run.generation == generation && run.status.state == State::Starting
                    })
                {
                    return;
                }
                let result = result.and_then(|options| {
                    this.forward_service
                        .as_ref()
                        .context("Forwarding service is unavailable.")?
                        .start(generation, spec, options)
                });
                if let Err(error) = result {
                    this.forward_states.get_mut(&id).unwrap().status = Status::failed(id, error);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn forward_editor_sections(
        &self,
        editor: &Editor,
        forward: &ForwardSelects,
        cx: &App,
    ) -> Vec<AnyElement> {
        let busy = forward.saving;
        let field = |index: usize, label: &str| {
            let field = Field::input(&editor.fields[index].1, label.to_owned());
            let field = if matches!(index, 2 | 4) {
                field.digits_only()
            } else {
                field
            };
            field
                .disabled(busy)
                .error(forward.errors[index].clone())
                .into_any_element()
        };
        let mut sections = vec![
            self.editor_section(
                "Rule",
                vec![
                    field(0, "Rule name *"),
                    forward.types.required_field("Type", busy),
                ],
            ),
            self.editor_section(
                if forward.kind == Kind::Remote {
                    "Remote Port Address"
                } else {
                    "Local Port Address"
                },
                vec![
                    field(1, "Bind Address"),
                    field(
                        2,
                        if forward.kind == Kind::Remote {
                            "Remote port number *"
                        } else {
                            "Local port number *"
                        },
                    ),
                ],
            ),
            self.editor_section(
                if forward.kind == Kind::Remote {
                    "Remote Host"
                } else {
                    "Intermediate Host"
                },
                vec![forward.hosts.field_with_error(
                    if forward.kind == Kind::Remote {
                        "Remote host *"
                    } else {
                        "Intermediate host *"
                    },
                    busy,
                    forward.host_error.clone(),
                )],
            ),
        ];
        if forward.kind != Kind::Dynamic {
            sections.push(self.editor_section(
                "Destination",
                vec![
                    field(3, "Destination address *"),
                    field(4, "Destination port number *"),
                ],
            ));
        }
        if busy {
            sections.push(
                div()
                    .text_size(px(11.))
                    .text_color(Palette::new(self.data.dark).muted)
                    .child(crate::localization::text(cx, "rs.ui.copy.stoppingRule"))
                    .into_any_element(),
            );
        }
        sections
    }
    pub(super) fn forward_collection(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.searches[&Page::PortForwards]
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let mut rules: Vec<_> = self
            .data
            .forwards
            .iter()
            .filter(|rule| self.forward_matches(rule, &query))
            .collect();
        let preferences = self.collection_view();
        rules.sort_by(|a, b| preferences.sort.compare((&a.name, a.id), (&b.name, b.id)));
        if rules.is_empty() {
            let empty = crate::components::EmptyState::new(
                IconName::ArrowLeftRight,
                if query.is_empty() {
                    "No forwarding rules yet"
                } else {
                    "No matching forwarding rules"
                },
                self.data.dark,
            )
            .description(if query.is_empty() {
                "Add a forwarding rule to route local, remote, or dynamic traffic through SSH."
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
                        Button::new("empty-add-forward")
                            .primary()
                            .icon(crate::assets::icon(IconName::Plus))
                            .label("Add forwarding")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_forward(Kind::Local, window, cx)
                            })),
                    )
                    .into_any_element()
            } else {
                empty.into_any_element()
            };
        }
        div()
            .id("collection-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .scrollbars()
            .p(px(26.))
            .on_click(cx.listener(|this, _, window, cx| {
                this.selection.clear();
                this.selection_kind = None;
                this.retire_editor();
                this.focus.focus(window, cx);
                cx.notify();
            }))
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().mb(px(2.)).font_weight(FontWeights::MEDIUM).child(
                        crate::localization::text(cx, "workspace.label.portForwarding"),
                    ))
                    .children(rules.iter().map(|rule| self.forward_card(rule, cx))),
            )
            .into_any_element()
    }
    pub(super) fn forward_matches(&self, rule: &Forward, query: &str) -> bool {
        let host = self
            .data
            .hosts
            .iter()
            .find(|host| Some(host.id) == rule.host_id);
        rule.matches_query(
            query,
            host.map_or(rule.host.as_str(), |host| host.name.as_str()),
            self.forward_states
                .get(&rule.id)
                .map(|runtime| &runtime.status),
        )
    }
    fn forward_card(&self, rule: &Forward, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.data.dark);
        let id = rule.id;
        let status = self
            .forward_states
            .get(&id)
            .map(|run| run.status.clone())
            .unwrap_or_else(|| Status::new(id, State::Stopped));
        let active = status.state.active();
        let selected = self.selection_kind == Some(ItemKind::Forward)
            && self.selection.contains(&id)
            || self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.edited() == Some((ItemKind::Forward, id)));
        let host = self
            .data
            .hosts
            .iter()
            .find(|host| Some(host.id) == rule.host_id)
            .map_or_else(
                || format!("Unavailable host ({})", rule.host),
                |host| host.name.clone(),
            );
        let bind = port_forward::endpoint(
            &rule.bind_address,
            status.bound_port.unwrap_or(rule.local_port),
        );
        let path = match rule.kind {
            Kind::Dynamic => format!("{bind}  ->  SOCKS via {host}"),
            Kind::Remote => format!("{bind}  ->  {}", rule.destination),
            Kind::Local => format!("{bind}  ->  {host}  ->  {}", rule.destination),
        };
        let color = if status.error.is_some() || status.state == State::Error {
            rgb(0xd24135)
        } else if active {
            rgb(if self.data.dark { 0x30d158 } else { 0x35d394 })
        } else {
            p.muted
        };
        let base = rgb(0x075e92);
        let start_palette = ButtonCustomVariant::new(cx)
            .color(
                if self.data.dark {
                    base.alpha(0.16)
                } else {
                    rgb(0xe0e4e6)
                }
                .into(),
            )
            .foreground(base.into())
            .hover(
                if self.data.dark {
                    base.alpha(0.22)
                } else {
                    crate::components::motion::mix(rgb(0xffffff), base, 0.14)
                }
                .into(),
            )
            .active(
                if self.data.dark {
                    base.alpha(0.28)
                } else {
                    crate::components::motion::mix(rgb(0xffffff), base, 0.22)
                }
                .into(),
            );
        let delete_owner = cx.entity().downgrade();
        let edit_owner = delete_owner.clone();
        crate::components::item_card(("forward-card", id), self.data.dark, selected, true)
            .accessibility_label(rule.name.clone())
            .reveal_on_layout(self.card_navigation.take_reveal((ItemKind::Forward, id)))
            .h_auto()
            .p(px(14.))
            .selection_outline(
                if selected || active {
                    p.accent
                } else {
                    p.border
                },
                px(if selected || active { 2. } else { 1. }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                this.focus.focus(window, cx);
                this.select_collection_card(ItemKind::Forward, id, event.modifiers(), window, cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| {
                    if this.selection_kind != Some(ItemKind::Forward)
                        || !this.selection.contains(&id)
                    {
                        this.selection = BTreeSet::from([id]);
                        this.selection_kind = Some(ItemKind::Forward);
                    }
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                div()
                                    .px(px(9.))
                                    .h(px(24.))
                                    .flex()
                                    .items_center()
                                    .rounded(px(8.))
                                    .bg(rgba(0x075e921a))
                                    .border_1()
                                    .border_color(rgba(0x075e9233))
                                    .text_size(px(11.))
                                    .font_weight(FontWeights::SEMIBOLD)
                                    .child(crate::localization::view(rule.kind.label())),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(FontWeights::SEMIBOLD)
                                            .child(rule.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(11.))
                                            .text_color(p.muted)
                                            .child(format!(
                                                "{}: {host}",
                                                if rule.kind == Kind::Remote {
                                                    "Remote host"
                                                } else {
                                                    "Via"
                                                }
                                            )),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .p(px(10.))
                            .rounded(px(8.))
                            .bg(p.background)
                            .border_1()
                            .border_color(p.border)
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeights::SEMIBOLD)
                                    .text_color(p.muted)
                                    .child(crate::localization::text(cx, "common.label.path")),
                            )
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_size(px(12.))
                                    .line_clamp(2)
                                    .child(path),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .h(px(24.))
                                    .px(px(9.))
                                    .rounded(px(8.))
                                    .flex()
                                    .items_center()
                                    .flex_shrink_0()
                                    .bg(color.alpha(if active { 0.14 } else { 0.06 }))
                                    .border_1()
                                    .border_color(if active { color } else { p.border })
                                    .text_size(px(11.))
                                    .font_weight(FontWeights::SEMIBOLD)
                                    .text_color(color)
                                    .child(crate::localization::view(if status.error.is_some() {
                                        "Error"
                                    } else {
                                        status.state.label()
                                    })),
                            )
                            .when(status.state == State::Running, |row| {
                                row.child(div().text_size(px(11.)).text_color(p.muted).child(
                                    crate::localization::view(format!(
                                        "{} active",
                                        status.active_connections
                                    )),
                                ))
                            })
                            .child(div().flex_1())
                            .child(
                                Button::new(("toggle-forward", id))
                                    .xsmall()
                                    .when(!active, |button| button.custom(start_palette))
                                    .label(if active { "Stop" } else { "Start" })
                                    .disabled(
                                        (status.state == State::Stopping && status.error.is_none())
                                            || self.forward_mutations.contains(&id),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_forward(id, cx)
                                    })),
                            )
                            .child(
                                Button::new(("edit-forward", id))
                                    .xsmall()
                                    .ghost()
                                    .label("Edit")
                                    .disabled(self.forward_mutations.contains(&id))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_editor(ItemKind::Forward, Some(id), window, cx)
                                    })),
                            ),
                    )
                    .when_some(status.error, |view, error| {
                        view.child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(0xd24135))
                                .line_clamp(2)
                                .child(crate::localization::message(cx, &error)),
                        )
                    }),
            )
            .context_menu(move |menu, _, menu_cx| {
                let shortcuts = crate::shortcuts::current(menu_cx);
                let edit_owner = edit_owner.clone();
                let delete_owner = delete_owner.clone();
                let duplicate_owner = edit_owner.clone();
                let menu_entity = menu_cx.entity();
                let (single, edit_disabled, delete_disabled) = edit_owner
                    .update(menu_cx, |this, cx| {
                        this.menu_subscription =
                            Some(cx.subscribe(&menu_entity, |this, _, _: &DismissEvent, cx| {
                                if !this.delete_pending
                                    && let Some(editor) = &this.editor
                                    && let Some(id) = editor.id
                                {
                                    this.selection = BTreeSet::from([id]);
                                    this.selection_kind = Some(editor.kind);
                                    cx.notify();
                                }
                            }));
                        (
                            this.selection.len() == 1,
                            this.forward_mutations.contains(&id) || this.selection.len() != 1,
                            this.selection
                                .iter()
                                .any(|id| this.forward_mutations.contains(id)),
                        )
                    })
                    .unwrap_or((false, true, true));
                menu.card_menu()
                    .when(single, |menu| {
                        menu.item(
                            shortcuts
                                .menu(
                                    MenuItem::localized("Edit").icon(IconName::Pencil),
                                    crate::shortcuts::Command::Edit,
                                )
                                .disabled(edit_disabled)
                                .on_click(move |_, window, cx| {
                                    let _ = edit_owner.update(cx, |this, cx| {
                                        this.open_editor(ItemKind::Forward, Some(id), window, cx)
                                    });
                                }),
                        )
                    })
                    .item(
                        shortcuts
                            .menu(
                                MenuItem::localized("Duplicate"),
                                crate::shortcuts::Command::Duplicate,
                            )
                            .icon(IconName::Copy)
                            .on_click(move |_, _, cx| {
                                let _ = duplicate_owner.update(cx, |this, cx| {
                                    this.duplicate_resources(ItemKind::Forward, cx)
                                });
                            }),
                    )
                    .separator()
                    .item(
                        MenuItem::localized("Delete")
                            .icon(IconName::Trash)
                            .destructive(true)
                            .disabled(delete_disabled)
                            .on_click(move |_, _, cx| {
                                let _ = delete_owner.update(cx, |this, cx| {
                                    this.notice = None;
                                    this.delete_pending = true;
                                    cx.notify();
                                });
                            }),
                    )
            })
            .into_any_element()
    }
}
