use super::*;
use crate::host_session::Variable;
use anyhow::{Result, ensure};

struct Row {
    id: u64,
    name: Entity<InputState>,
    value: Entity<InputState>,
}
pub(super) struct EnvironmentEditor {
    scroll: ScrollHandle,
    generation: u64,
    rows: Vec<Row>,
    next: u64,
    loading: bool,
    pub(super) error: Option<String>,
    validation_error: Option<String>,
}
impl EnvironmentEditor {
    fn new(entries: &[Variable], window: &mut Window, cx: &mut Context<Nauterm>) -> Self {
        let rows = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| Row {
                id: index as u64,
                name: cx
                    .new(|cx| InputState::new(window, cx).default_value(entry.variable.clone())),
                value: cx.new(|cx| InputState::new(window, cx).default_value(entry.value.clone())),
            })
            .collect();
        Self {
            scroll: ScrollHandle::new(),
            generation: rand::random(),
            rows,
            next: entries.len() as u64,
            loading: false,
            error: None,
            validation_error: None,
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
enum Resource {
    Host(Host),
    Group(Group),
}
impl Resource {
    fn id(&self) -> Id {
        match self {
            Self::Host(host) => host.id,
            Self::Group(group) => group.id,
        }
    }
    fn kind(&self) -> ItemKind {
        match self {
            Self::Host(_) => ItemKind::Host,
            Self::Group(_) => ItemKind::Group,
        }
    }
    fn current(&self, data: &AppState) -> Option<Self> {
        match self {
            Self::Host(host) => data
                .hosts
                .iter()
                .find(|saved| saved.id == host.id)
                .cloned()
                .map(Self::Host),
            Self::Group(group) => data
                .groups
                .iter()
                .find(|saved| saved.id == group.id)
                .cloned()
                .map(Self::Group),
        }
    }
    fn setup_mut(&mut self) -> Option<&mut crate::host_session::Setup> {
        match self {
            Self::Host(host) if host.supports_ssh() => Some(&mut host.ssh),
            Self::Group(group) => group.ssh.as_mut().map(|profile| &mut profile.setup),
            _ => None,
        }
    }
    fn telnet_mut(&mut self) -> Option<&mut crate::telnet::Profile> {
        match self {
            Self::Host(host) => host.connections.telnet.as_mut(),
            Self::Group(group) => group.telnet.as_mut(),
        }
    }
    fn environment_id(&self) -> Option<&String> {
        match self {
            Self::Host(host) => host.ssh.environment_id.as_ref(),
            Self::Group(group) => group
                .ssh
                .as_ref()
                .and_then(|profile| profile.setup.environment_id.as_ref()),
        }
    }
    fn password_id(&self) -> Option<&String> {
        match self {
            Self::Host(host) => host.ssh.password_id.as_ref(),
            Self::Group(group) => group
                .ssh
                .as_ref()
                .and_then(|profile| profile.setup.password_id.as_ref()),
        }
    }
    fn insert(mut self, data: &mut AppState) -> Id {
        let id = if self.id() == 0 {
            data.allocate_id()
        } else {
            self.id()
        };
        match &mut self {
            Self::Host(host) => {
                host.id = id;
                data.hosts.retain(|saved| saved.id != id);
                data.hosts.push(host.clone());
            }
            Self::Group(group) => {
                group.id = id;
                data.groups.retain(|saved| saved.id != id);
                data.groups.push(group.clone());
            }
        }
        id
    }
}
struct PendingResource {
    value: Resource,
    before: Option<Resource>,
    editor: EntityId,
    tag_ids: Option<BTreeSet<String>>,
}
impl Nauterm {
    pub(super) fn open_host_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let Some(draft) = &mut editor.host else {
            return;
        };
        if draft.busy || draft.inheriting {
            return;
        }
        let token = editor.fields[0].1.entity_id();
        if let Some(entries) = &draft.environment {
            draft.environment_page = Some(EnvironmentEditor::new(entries, window, cx));
            self.focus.focus(window, cx);
            cx.notify();
            return;
        }
        let Some(id) = draft.setup.environment_id.clone() else {
            return;
        };
        let generation = rand::random();
        draft.environment_page = Some(EnvironmentEditor {
            scroll: ScrollHandle::new(),
            generation,
            rows: vec![],
            next: 0,
            loading: true,
            error: None,
            validation_error: None,
        });
        let vault = self.ensure_keychain(cx).read(cx).vault();
        self.focus.focus(window, cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = match vault {
                Some(vault) => vault.host_environment(id).await,
                None => Err(anyhow::anyhow!("The encrypted database is unavailable.")),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(editor) =
                    this.find_editor_mut(&|editor| editor.fields[0].1.entity_id() == token)
                else {
                    return;
                };
                let Some(draft) = &mut editor.host else {
                    return;
                };
                if !draft
                    .environment_page
                    .as_ref()
                    .is_some_and(|page| page.generation == generation)
                {
                    return;
                }
                match result {
                    Ok(entries) => {
                        if draft.environment_page.is_some() {
                            draft.environment_page =
                                Some(EnvironmentEditor::new(&entries, window, cx));
                        }
                        draft.environment = Some(entries);
                    }
                    Err(error) => {
                        if let Some(page) = &mut draft.environment_page {
                            page.loading = false;
                            page.error = Some(error.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn leave_host_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.editor {
            if let Some(draft) = &mut editor.host {
                draft.environment_page = None;
            }
            editor.fields[0]
                .1
                .update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }
    fn add_environment_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self
            .editor
            .as_mut()
            .and_then(|editor| editor.host.as_mut())
            .and_then(|draft| draft.environment_page.as_mut())
        else {
            return;
        };
        if page.loading {
            return;
        }
        page.error = None;
        let name = cx.new(|cx| InputState::new(window, cx));
        let value = cx.new(|cx| InputState::new(window, cx));
        page.rows.push(Row {
            id: page.next,
            name: name.clone(),
            value,
        });
        page.next += 1;
        name.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }
    fn apply_environment_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = self.editor.as_mut().and_then(|editor| editor.host.as_mut()) else {
            return;
        };
        let Some(page) = &mut draft.environment_page else {
            return;
        };
        if page.loading {
            return;
        }
        let values: Vec<_> = page
            .rows
            .iter()
            .filter_map(|row| {
                let variable = row.name.read(cx).value().trim().to_owned();
                (!variable.is_empty()).then(|| Variable {
                    variable,
                    value: row.value.read(cx).value().to_string(),
                })
            })
            .collect();
        if let Err(error) = crate::host_session::validate_environment(&values) {
            page.validation_error = Some(error.to_string());
            cx.notify();
            return;
        }
        draft.environment_changed |= draft.environment.as_ref() != Some(&values);
        draft.environment = Some(values);
        draft.cancel_inheritance();
        self.leave_host_environment(window, cx);
    }
    pub(super) fn host_environment_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let editor = self.editor.as_ref().unwrap();
        let page = editor
            .host
            .as_ref()
            .and_then(|draft| draft.environment_page.as_ref())
            .unwrap();
        let p = Palette::new(self.data.dark);
        let unavailable = page.loading
            || (page.error.is_some() && editor.host.as_ref().unwrap().environment.is_none());
        let mut frame = crate::components::DrawerFrame::new(
            "host-environment-editor",
            "Environment Variables",
            self.data.dark,
        )
        .scroll_handle(&page.scroll)
        .action(
            Button::new("back-host-environment")
                .toolbar()
                .ghost()
                .icon(IconName::ArrowRightToLine)
                .tooltip("Back to Host")
                .on_click(
                    cx.listener(|this, _, window, cx| this.leave_host_environment(window, cx)),
                ),
        )
        .section(self.editor_section(
            "Environment variables",
            vec![
                    div()
                        .child(crate::localization::view(format!(
                            "Set environment variables for {}.",
                            editor.fields[0].1.read(cx).value()
                        )))
                        .into_any_element(),
                    div()
                        .text_color(p.muted)
                        .child(crate::localization::text(cx, "rs.ui.copy.sshAcceptEnv"))
                        .into_any_element(),
                    Button::new("add-host-variable")
                        .large()
                        .w_full()
                        .label("Add variable")
                        .icon(crate::assets::icon(IconName::Plus))
                        .disabled(unavailable)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.add_environment_row(window, cx)),
                        )
                        .into_any_element(),
                ],
        ));
        for row in &page.rows {
            let id = row.id;
            let group: SharedString = format!("environment-row-{id}").into();
            frame = frame.section(
                div()
                    .id(("environment-row", id))
                    .group(group.clone())
                    .rounded(px(10.))
                    .bg(p.surface)
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeights::MEDIUM)
                                    .child(crate::localization::text(cx, "common.label.variable")),
                            )
                            .child(
                                div()
                                    .invisible()
                                    .group_hover(group, |view| view.visible())
                                    .child(
                                        Button::new(("remove-host-variable", id))
                                            .xsmall()
                                            .ghost()
                                            .icon(crate::assets::icon(IconName::X))
                                            .tooltip("Remove variable")
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Some(page) = this
                                                    .editor
                                                    .as_mut()
                                                    .and_then(|editor| editor.host.as_mut())
                                                    .and_then(|draft| {
                                                        draft.environment_page.as_mut()
                                                    })
                                                {
                                                    page.rows.retain(|row| row.id != id);
                                                    page.error = None;
                                                }
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(Field::input(&row.name, "Variable"))
                    .child(Field::input(&row.value, "Value")),
            );
        }
        frame
            .when_some(
                crate::components::validation_message(page.validation_error.as_deref(), cx),
                |frame, error| frame.section(error),
            )
            .section(
                div()
                    .when(page.loading, |view| {
                        view.child(crate::localization::text(
                            cx,
                            "rs.ui.copy.loadingEnvironment",
                        ))
                    })
                    .when(page.error.is_some(), |view| {
                        view.when(unavailable && !page.loading, |view| {
                            view.child(
                                Button::new("retry-host-environment")
                                    .small()
                                    .ghost()
                                    .label("Retry")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_host_environment(window, cx)
                                    })),
                            )
                        })
                    }),
            )
            .footer(
                Button::new("apply-host-environment")
                    .large()
                    .w_full()
                    .accent()
                    .label("Save")
                    .disabled(unavailable)
                    .on_click(
                        cx.listener(|this, _, window, cx| {
                            this.apply_environment_editor(window, cx)
                        }),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn save_host_candidate(
        &mut self,
        host: Host,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_connection_candidate(Resource::Host(host), window, cx);
    }
    pub(super) fn save_group_candidate(
        &mut self,
        group: Group,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_connection_candidate(Resource::Group(group), window, cx);
    }
    fn save_connection_candidate(
        &mut self,
        mut resource: Resource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor = self.editor.as_ref().unwrap();
        let draft = editor.host.as_ref().unwrap();
        let environment = (resource.setup_mut().is_some() && draft.environment_changed)
            .then(|| draft.environment.clone())
            .flatten();
        let manual_auth = !matches!(editor.credential, Some(CredentialRef::Identity(_)));
        let password = (resource.setup_mut().is_some() && manual_auth)
            .then(|| editor.selects.as_ref().unwrap().auth.password_change(cx))
            .flatten();
        if let Some(setup) = resource.setup_mut() {
            if !manual_auth || password.is_some() {
                setup.password_id = None;
            }
            if draft.environment_changed {
                setup.environment_id = None;
            }
        }
        let telnet_password = resource
            .telnet_mut()
            .filter(|profile| profile.identity_id.is_none())
            .and_then(|_| draft.telnet.auth.password_change(cx));
        if let Some(profile) = resource.telnet_mut()
            && (profile.identity_id.is_some() || telnet_password.is_some())
        {
            profile.password_id = None;
        }
        let pending = PendingResource {
            before: resource.current(&self.data),
            editor: editor.fields[0].1.entity_id(),
            value: resource.clone(),
            tag_ids: match &resource {
                Resource::Host(host) => Some(crate::tags::ids(&self.data.tags, &host.tags)),
                _ => None,
            },
        };
        let environment = environment.filter(|entries| !entries.is_empty());
        let password = password.filter(|password| !password.is_empty());
        let telnet_password = telnet_password.filter(|value| !value.is_empty());
        if environment.is_some() || password.is_some() || telnet_password.is_some() {
            let Some(vault) = self.ensure_keychain(cx).read(cx).vault() else {
                self.editor.as_mut().unwrap().error =
                    Some("The encrypted database is unavailable.".into());
                cx.notify();
                return;
            };
            self.editor.as_mut().unwrap().host.as_mut().unwrap().busy = true;
            cx.spawn_in(window, async move |this, cx| {
                let token = pending.editor;
                let mut created_environments = vec![];
                let mut created_passwords = vec![];
                let result = async {
                    if let Some(environment) = environment {
                        let id = vault.save_host_environment(environment).await?;
                        resource.setup_mut().unwrap().environment_id = Some(id.clone());
                        created_environments.push(id);
                    }
                    if let Some(password) = password {
                        let id = vault.save_host_password(password).await?;
                        resource.setup_mut().unwrap().password_id = Some(id.clone());
                        created_passwords.push(id);
                    }
                    if let Some(password) = telnet_password {
                        let id = vault.save_host_password(password).await?;
                        resource.telnet_mut().unwrap().password_id = Some(id.clone());
                        created_passwords.push(id);
                    }
                    anyhow::Ok(())
                }
                .await;
                let published = this
                    .update_in(cx, |this, window, cx| {
                        match result {
                            Ok(()) => {}
                            Err(error) => {
                                this.host_save_failed(pending.editor, error.to_string(), cx);
                                return false;
                            }
                        }
                        let pending = PendingResource {
                            value: resource,
                            ..pending
                        };
                        match this.publish_connection_resource(pending, window, cx) {
                            Ok(()) => true,
                            Err(error) => {
                                this.host_save_failed(token, error.to_string(), cx);
                                false
                            }
                        }
                    })
                    .unwrap_or(false);
                // Publication copied draft values into their original native
                // rows. Temporary encrypted references can be dropped on both
                // success and failure; canonical profile references survive.
                let environment = vault.remove_host_environments(created_environments).await;
                let password = vault.remove_host_passwords(created_passwords).await;
                let errors: Vec<_> = [environment, password]
                    .into_iter()
                    .filter_map(Result::err)
                    .map(|error| error.to_string())
                    .collect();
                if !errors.is_empty() {
                    let _ = this.update(cx, |this, cx| {
                        this.notice = Some(format!(
                            "{}; encrypted draft cleanup failed: {}",
                            if published {
                                "Settings saved"
                            } else {
                                "Settings save failed"
                            },
                            errors.join("; ")
                        ));
                        cx.notify();
                    });
                }
            })
            .detach();
        } else {
            let token = pending.editor;
            if let Err(error) = self.publish_connection_resource(
                PendingResource {
                    value: resource,
                    ..pending
                },
                window,
                cx,
            ) {
                self.host_save_failed(token, error.to_string(), cx);
            }
        }
        cx.notify();
    }
    fn host_save_failed(&mut self, token: EntityId, error: String, cx: &mut Context<Self>) {
        if let Some(editor) =
            self.find_editor_mut(&|editor| editor.fields[0].1.entity_id() == token)
        {
            editor.error = Some(format!("Could not save: {error}"));
            if let Some(draft) = &mut editor.host {
                draft.busy = false;
            }
        } else {
            self.notice = Some(format!("Could not save connection settings: {error}"));
        }
        cx.notify();
    }
    fn publish_connection_resource(
        &mut self,
        mut pending: PendingResource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        ensure!(self.resources_ready(), "Workspace storage is unavailable.");
        let mut candidate = self.data.clone();
        if pending.value.id() != 0 {
            ensure!(
                pending.before.is_some() && pending.value.current(&candidate) == pending.before,
                "This configuration changed while saving. Reopen it and try again."
            );
        }
        let kind = pending.value.kind();
        if let (Resource::Host(host), Some(ids)) = (&mut pending.value, &pending.tag_ids) {
            host.tags = crate::tags::names(&self.data.tags, ids);
        }
        let id = pending.value.insert(&mut candidate);
        crate::tags::materialize(&mut candidate);
        // An earlier async save must not resurrect a proxy deleted while its
        // encrypted password/environment was being written.
        candidate.clear_proxy_references(&self.deleted_proxies);
        self.store.save(&mut candidate)?;
        self.data = candidate;
        for session in self
            .terminals
            .iter()
            .flat_map(|tab| tab.tree.sessions())
            .filter(|session| kind == ItemKind::Host && session.host_id() == Some(id))
        {
            let appearance = session.appearance(&self.data);
            let result = match &session.content {
                TabContent::Terminal(terminal) => {
                    terminal.update(cx, |terminal, cx| terminal.set_appearance(&appearance, cx))
                }
                TabContent::Connection(connection) => connection.update(cx, |connection, _| {
                    connection.set_terminal_palette(appearance.palette())
                }),
                TabContent::Preparing(_) => Ok(()),
            };
            if let Err(error) = result {
                self.notice = Some(error.to_string());
            }
        }
        self.with_editor_context(
            |editor| editor.fields[0].1.entity_id() == pending.editor,
            window,
            cx,
            |this, window, cx| this.complete_resource_editor(kind, id, window, cx),
        );
        self.cleanup_host_environments(
            pending
                .before
                .as_ref()
                .and_then(Resource::environment_id)
                .cloned()
                .into_iter()
                .collect(),
            cx,
        );
        self.cleanup_host_passwords(
            pending
                .before
                .as_ref()
                .and_then(Resource::password_id)
                .cloned()
                .into_iter()
                .collect(),
            cx,
        );
        cx.notify();
        Ok(())
    }
    pub(super) fn cleanup_host_environments(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        let ids: Vec<_> = ids
            .into_iter()
            .filter(|id| !self.data.environment_in_use(id))
            .collect();
        if ids.is_empty() {
            return;
        }
        let Some(vault) = self.ensure_keychain(cx).read(cx).vault() else {
            self.notice =
                Some("Settings saved; encrypted environment cleanup is unavailable.".into());
            return;
        };
        cx.spawn(async move |this, cx| {
            if let Err(error) = vault.remove_host_environments(ids).await {
                let _ = this.update(cx, |this, cx| {
                    this.notice = Some(format!(
                        "Settings updated; encrypted environment cleanup failed: {error}"
                    ));
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub(super) fn cleanup_host_passwords(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        let ids: Vec<_> = ids
            .into_iter()
            .filter(|id| !self.data.password_in_use(id))
            .collect();
        if ids.is_empty() {
            return;
        }
        let Some(vault) = self.ensure_keychain(cx).read(cx).vault() else {
            self.notice = Some("Settings saved; encrypted password cleanup is unavailable.".into());
            return;
        };
        cx.spawn(async move |this, cx| {
            if let Err(error) = vault.remove_host_passwords(ids).await {
                let _ = this.update(cx, |this, cx| {
                    this.notice = Some(format!(
                        "Settings updated; encrypted password cleanup failed: {error}"
                    ));
                    cx.notify();
                });
            }
        })
        .detach();
    }
}
