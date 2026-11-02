use super::*;
use crate::components::{SelectAction, SelectCreate, SelectEdited};
use crate::keychain::selector::{KeyKind, KeySelector};
use crate::keychain::{Keychain, Kind, RelatedEvent};
use crate::vault::Inventory;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Ssh,
    Telnet,
}
impl Target {
    pub fn id(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Telnet => "telnet",
        }
    }
}
impl Editor {
    pub(super) fn auth(&self, target: Target) -> Option<&Authentication> {
        match target {
            Target::Ssh => self.selects.as_ref().map(|selects| &selects.auth),
            Target::Telnet => self.host.as_ref().map(|draft| &draft.telnet.auth),
        }
    }
    pub(super) fn auth_mut(&mut self, target: Target) -> Option<&mut Authentication> {
        match target {
            Target::Ssh => self.selects.as_mut().map(|selects| &mut selects.auth),
            Target::Telnet => self.host.as_mut().map(|draft| &mut draft.telnet.auth),
        }
    }
    fn auth_credential(&self, target: Target) -> Option<CredentialRef> {
        match target {
            Target::Ssh => self.credential,
            Target::Telnet => self
                .host
                .as_ref()?
                .telnet
                .profile
                .identity_id
                .map(CredentialRef::Identity),
        }
    }
    fn set_auth_credential(&mut self, target: Target, value: Option<CredentialRef>) {
        match target {
            Target::Ssh => self.credential = value,
            Target::Telnet => {
                self.host.as_mut().unwrap().telnet.profile.identity_id = match value {
                    Some(CredentialRef::Identity(id)) => Some(id),
                    _ => None,
                }
            }
        }
    }
    fn auth_username(&self, target: Target) -> Entity<InputState> {
        match target {
            Target::Ssh => self.fields[3].1.clone(),
            Target::Telnet => self.host.as_ref().unwrap().telnet.username.clone(),
        }
    }
    fn auth_password_id(&self, target: Target) -> Option<String> {
        let draft = self.host.as_ref()?;
        match target {
            Target::Ssh => draft.setup.password_id.clone(),
            Target::Telnet => draft.telnet.profile.password_id.clone(),
        }
    }
    fn clear_auth_password_id(&mut self, target: Target) {
        let draft = self.host.as_mut().unwrap();
        match target {
            Target::Ssh => draft.setup.password_id = None,
            Target::Telnet => draft.telnet.profile.password_id = None,
        }
    }
}
#[derive(Clone, Copy)]
enum AuthField {
    Username,
    Password,
    Key,
}
pub(super) struct Authentication {
    username: SelectModel<CredentialRef>,
    password: Entity<InputState>,
    password_select: SelectModel<CredentialRef>,
    keys: KeySelector,
    reference: Option<String>,
    generation: u64,
    changed: bool,
    loading: bool,
    pub(super) error: Option<String>,
    _events: Vec<Subscription>,
}
fn identity(reference: Option<CredentialRef>) -> Option<CredentialRef> {
    reference.filter(|reference| matches!(reference, CredentialRef::Identity(_)))
}
fn key(reference: Option<CredentialRef>) -> Option<CredentialRef> {
    reference.filter(|reference| matches!(reference, CredentialRef::Key(_)))
}
fn identity_options(inventory: &Inventory) -> Vec<SelectOption<CredentialRef>> {
    inventory
        .identities
        .iter()
        .map(|entry| SelectOption::new(CredentialRef::Identity(entry.id), entry.name.clone()))
        .collect()
}
fn key_id(reference: Option<CredentialRef>) -> Option<i64> {
    match reference {
        Some(CredentialRef::Key(id)) => Some(id),
        _ => None,
    }
}
impl Authentication {
    pub fn new(
        credential: Option<CredentialRef>,
        username: &Entity<InputState>,
        inventory: &Inventory,
        window: &mut Window,
        cx: &mut Context<Nauterm>,
    ) -> Self {
        Self::for_target(Target::Ssh, credential, username, inventory, window, cx)
    }
    pub fn for_target(
        target: Target,
        credential: Option<CredentialRef>,
        username: &Entity<InputState>,
        inventory: &Inventory,
        window: &mut Window,
        cx: &mut Context<Nauterm>,
    ) -> Self {
        let password = cx.new(|cx| InputState::new(window, cx).masked(true));
        let username = SelectModel::editable(
            identity_options(inventory),
            identity(credential),
            username,
            window,
            cx,
        );
        let password_select =
            SelectModel::editable(identity_options(inventory), None, &password, window, cx);
        let keys = KeySelector::new(inventory, key_id(credential), window, cx);
        let mut events = Vec::new();
        for (field, select) in [
            (AuthField::Username, &username),
            (AuthField::Password, &password_select),
        ] {
            events.push(cx.subscribe_in(
                &select.state,
                window,
                move |this, state, event: &SelectEvent<CredentialRef>, window, cx| {
                    if !this.editor.as_ref().is_some_and(|editor| {
                        editor
                            .auth(target)
                            .is_some_and(|auth| auth.owns_select(field, state.entity_id()))
                            && editor.host.as_ref().is_some_and(|draft| !draft.busy)
                    }) {
                        return;
                    }
                    let SelectEvent::Confirm(value) = event;
                    this.select_host_credential(target, field, value.flatten(), window, cx);
                },
            ));
            if matches!(field, AuthField::Username) {
                events.push(cx.subscribe_in(
                    &select.state,
                    window,
                    move |this, state, event: &SelectCreate, window, cx| {
                        if event.action == "identity"
                            && this
                                .editor
                                .as_ref()
                                .and_then(|editor| editor.auth(target))
                                .is_some_and(|auth| auth.owns_select(field, state.entity_id()))
                        {
                            this.open_related_credential(
                                target,
                                Kind::Identity,
                                &event.text,
                                window,
                                cx,
                            );
                        }
                    },
                ));
            }
        }
        if target == Target::Ssh {
            events.push(cx.subscribe_in(
                &keys.select.state,
                window,
                |this, state, event: &SelectEvent<i64>, window, cx| {
                    if !this.editor.as_ref().is_some_and(|editor| {
                        editor
                            .auth(Target::Ssh)
                            .is_some_and(|auth| auth.keys.select.state == *state)
                            && editor.host.as_ref().is_some_and(|draft| !draft.busy)
                    }) {
                        return;
                    }
                    let SelectEvent::Confirm(value) = event;
                    this.select_host_credential(
                        Target::Ssh,
                        AuthField::Key,
                        value.flatten().map(CredentialRef::Key),
                        window,
                        cx,
                    );
                },
            ));
            events.push(cx.subscribe_in(
                &keys.select.state,
                window,
                |this, state, event: &SelectCreate, window, cx| {
                    if !this
                        .editor
                        .as_ref()
                        .and_then(|editor| editor.auth(Target::Ssh))
                        .is_some_and(|auth| auth.keys.select.state == *state)
                    {
                        return;
                    }
                    let kind = match event.action {
                        "key" => Kind::Key,
                        "certificate" => Kind::Certificate,
                        _ => return,
                    };
                    this.open_related_credential(Target::Ssh, kind, &event.text, window, cx);
                },
            ));
        }
        events.push(
            cx.subscribe(&username.state, move |this, state, _: &SelectEdited, cx| {
                if let Some(editor) = this.editor.as_mut().filter(|editor| {
                    editor
                        .auth(target)
                        .is_some_and(|auth| auth.username.state.entity_id() == state.entity_id())
                }) {
                    if identity(editor.auth_credential(target)).is_some() {
                        editor.set_auth_credential(target, None);
                    }
                    cx.notify();
                }
            }),
        );
        events.push(
            cx.subscribe(&password, move |this, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(editor) = this.editor.as_mut().filter(|editor| {
                        editor
                            .auth(target)
                            .is_some_and(|auth| auth.password.entity_id() == state.entity_id())
                    })
                {
                    let auth = editor.auth_mut(target).unwrap();
                    auth.changed = true;
                    auth.generation = auth.generation.wrapping_add(1);
                    auth.loading = false;
                    auth.error = None;
                    editor.clear_auth_password_id(target);
                    cx.notify();
                }
            }),
        );
        Self {
            username,
            password,
            password_select,
            keys,
            reference: None,
            generation: 0,
            changed: false,
            loading: false,
            error: None,
            _events: events,
        }
    }
    fn owns_select(&self, field: AuthField, id: EntityId) -> bool {
        match field {
            AuthField::Username => self.username.state.entity_id() == id,
            AuthField::Password => self.password_select.state.entity_id() == id,
            AuthField::Key => self.keys.select.state.entity_id() == id,
        }
    }
    pub fn reconcile(
        &mut self,
        inventory: &Inventory,
        credential: Option<CredentialRef>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.username.reconcile(
            identity_options(inventory),
            identity(credential),
            window,
            cx,
        );
        self.password_select
            .reconcile(identity_options(inventory), None, window, cx);
        self.keys
            .reconcile(inventory, key_id(credential), window, cx);
    }
    pub fn blocked(&self) -> bool {
        self.loading || self.error.is_some()
    }
    pub fn manual_password_present(&self, cx: &App) -> bool {
        !self.password.read(cx).value().is_empty()
    }
    pub fn inherited_password(&mut self, before: &Option<String>, after: &Option<String>) {
        if before != after {
            self.changed = false;
        }
    }
    pub fn validate(&self, cx: &App) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.blocked(),
            "Resolve the saved password loading error before saving."
        );
        crate::host_password::validate(&self.password.read(cx).value())
    }
    pub fn password_change(&self, cx: &App) -> Option<zeroize::Zeroizing<String>> {
        self.changed
            .then(|| zeroize::Zeroizing::new(self.password.read(cx).value().to_string()))
    }
    pub(super) fn password_snapshot(&self, cx: &App) -> Option<zeroize::Zeroizing<String>> {
        (self.changed || self.manual_password_present(cx))
            .then(|| zeroize::Zeroizing::new(self.password.read(cx).value().to_string()))
    }
    pub(super) fn set_password_draft(
        &mut self,
        value: zeroize::Zeroizing<String>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.clear_password(window, cx);
        self.password
            .update(cx, |input, cx| input.set_value(value.as_str(), window, cx));
    }
    fn clear_password(&mut self, window: &mut Window, cx: &mut App) {
        self.changed = true;
        self.generation = self.generation.wrapping_add(1);
        self.loading = false;
        self.error = None;
        self.reference = None;
        self.password
            .update(cx, |input, cx| input.set_value("", window, cx));
    }
}

pub(super) struct Related {
    pub view: Entity<Keychain>,
    _events: Subscription,
}

impl Nauterm {
    fn select_host_credential(
        &mut self,
        target: Target,
        field: AuthField,
        value: Option<CredentialRef>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = if let Some(CredentialRef::Identity(id)) = value {
            self.keychain
                .as_ref()
                .and_then(|view| {
                    view.read(cx)
                        .inventory()
                        .identities
                        .into_iter()
                        .find(|entry| entry.id == id)
                })
                .map(|entry| entry.name)
        } else {
            None
        };
        let Some(editor) = &mut self.editor else {
            return;
        };
        if matches!(field, AuthField::Key) {
            editor.set_auth_credential(target, key(value));
            let auth = editor.auth_mut(target).unwrap();
            if value.is_none() {
                auth.keys.clear();
                auth.username
                    .state
                    .read(cx)
                    .focus_handle(cx)
                    .focus(window, cx);
            }
        } else if let Some(reference) = identity(value) {
            editor.set_auth_credential(target, Some(reference));
            editor.auth_username(target).update(cx, |input, cx| {
                input.set_value(
                    name.unwrap_or_else(|| "Unavailable item".into()),
                    window,
                    cx,
                )
            });
            editor.clear_auth_password_id(target);
            let auth = editor.auth_mut(target).unwrap();
            auth.clear_password(window, cx);
            auth.keys.clear();
            if matches!(field, AuthField::Password) {
                auth.username
                    .state
                    .read(cx)
                    .focus_handle(cx)
                    .focus(window, cx);
            }
        } else {
            if identity(editor.auth_credential(target)).is_some() {
                editor.set_auth_credential(target, None);
            }
            match field {
                AuthField::Username => editor
                    .auth_username(target)
                    .update(cx, |input, cx| input.set_value("", window, cx)),
                AuthField::Password => {
                    editor.clear_auth_password_id(target);
                    editor.auth_mut(target).unwrap().clear_password(window, cx);
                }
                AuthField::Key => unreachable!(),
            }
        }
        cx.notify();
    }
    pub(super) fn load_editor_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for target in [Target::Ssh, Target::Telnet] {
            self.load_protocol_password(target, window, cx);
        }
    }
    fn load_protocol_password(
        &mut self,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let token = editor.fields[0].1.entity_id();
        let reference = editor.auth_password_id(target);
        let selected_identity = identity(editor.auth_credential(target)).is_some();
        let Some(auth) = editor.auth_mut(target) else {
            return;
        };
        if auth.changed || selected_identity || auth.reference == reference {
            return;
        }
        auth.reference = reference.clone();
        auth.generation = auth.generation.wrapping_add(1);
        auth.error = None;
        auth.password
            .update(cx, |input, cx| input.set_value("", window, cx));
        let Some(id) = reference else {
            auth.loading = false;
            return;
        };
        auth.loading = true;
        let generation = auth.generation;
        let vault = self.ensure_keychain(cx).read(cx).vault();
        cx.spawn_in(window, async move |this, cx| {
            let result = match vault { Some(vault) => vault.host_password(id).await, None => Err(anyhow::anyhow!("The encrypted password could not be loaded. Resolve the Keychain storage error and retry.")) };
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(editor) = this.find_editor_mut(&|editor| editor.fields[0].1.entity_id() == token) else { return; };
                let Some(auth) = editor.auth_mut(target) else { return; };
                if auth.generation != generation || auth.changed { return; }
                auth.loading = false;
                match result {
                    Ok(password) => auth.password.update(cx, |input, cx| input.set_value(password.as_str().to_owned(), window, cx)),
                    Err(error) => auth.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    pub(super) fn host_authentication_fields(
        &self,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        self.protocol_authentication_fields(editor, Target::Ssh, cx)
    }
    pub(super) fn protocol_authentication_fields(
        &self,
        editor: &Editor,
        target: Target,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let auth = editor.auth(target).unwrap();
        let busy = editor.host.as_ref().is_some_and(|draft| draft.busy);
        let selected_identity = identity(editor.auth_credential(target)).is_some();
        let mut fields = vec![auth.username.field_with_actions(
            if selected_identity {
                "Identity"
            } else {
                "Username"
            },
            busy,
            vec![SelectAction::new(
                "identity",
                "Create identity",
                IconName::Contact,
            )],
        )];
        if selected_identity {
            return fields;
        }
        fields.push(auth.password_select.field("Password", busy || auth.loading));
        if auth.loading {
            fields.push(
                div()
                    .text_size(px(12.))
                    .text_color(Palette::new(self.data.dark).muted)
                    .child(crate::localization::text(cx, "rs.ui.copy.loadingPassword"))
                    .into_any_element(),
            );
        }
        if auth.error.is_some() {
            fields.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(
                                Button::new(SharedString::from(format!(
                                    "retry-{}-password",
                                    target.id()
                                )))
                                .small()
                                .ghost()
                                .label("Retry")
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        if let Some(auth) = this
                                            .editor
                                            .as_mut()
                                            .and_then(|editor| editor.auth_mut(target))
                                        {
                                            auth.reference = None;
                                        }
                                        this.load_protocol_password(target, window, cx);
                                    },
                                )),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "clear-{}-password",
                                    target.id()
                                )))
                                .small()
                                .ghost()
                                .label("Clear saved password")
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.select_host_credential(
                                            target,
                                            AuthField::Password,
                                            None,
                                            window,
                                            cx,
                                        )
                                    },
                                )),
                            ),
                    )
                    .into_any_element(),
            );
        }
        let owner = cx.weak_entity();
        let token = editor.fields[0].1.entity_id();
        if target == Target::Ssh {
            fields.push(auth.keys.view(
                busy,
                move |kind, _, cx| {
                    let _ = owner.update(cx, |this, cx| this.select_host_key_kind(kind, token, cx));
                },
                cx,
            ));
        }
        fields
    }
    fn select_host_key_kind(&mut self, kind: KeyKind, token: EntityId, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.as_mut().filter(|editor| {
            editor.fields[0].1.entity_id() == token
                && editor.host.as_ref().is_some_and(|draft| !draft.busy)
                && identity(editor.credential).is_none()
        }) else {
            return;
        };
        let auth = &mut editor.selects.as_mut().unwrap().auth;
        if auth.keys.choose_kind(kind, cx) {
            editor.credential = None;
        }
        cx.notify();
    }
    pub(super) fn open_related_credential(
        &mut self,
        target: Target,
        kind: Kind,
        initial_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = self.editor.as_ref().filter(|editor| {
            matches!(editor.kind, ItemKind::Host | ItemKind::Group)
                && editor.related_credential.is_none()
                && editor.related_proxy.is_none()
                && editor.host.as_ref().is_some_and(|draft| !draft.busy)
        }) else {
            return;
        };
        let username = if identity(editor.auth_credential(target)).is_some() {
            String::new()
        } else {
            editor.auth_username(target).read(cx).value().to_string()
        };
        let keychain = self.ensure_keychain(cx);
        let Some(source) = keychain.update(cx, |view, cx| view.related_source(cx)) else {
            self.editor.as_mut().unwrap().error =
                Some("Keychain is unavailable. Open Keychain and retry.".into());
            cx.notify();
            return;
        };
        let view =
            cx.new(|cx| Keychain::related(source, kind, initial_name, &username, window, cx));
        let return_to_identity = kind == Kind::Identity;
        let events = cx.subscribe_in(
            &view,
            window,
            move |this, view, event: &RelatedEvent, window, cx| {
                this.with_editor_context(
                    |editor| {
                        editor
                            .related_credential
                            .as_ref()
                            .is_some_and(|related| related.view == *view)
                    },
                    window,
                    cx,
                    |this, window, cx| {
                        if !this.editor.as_ref().is_some_and(|editor| {
                            editor
                                .related_credential
                                .as_ref()
                                .is_some_and(|related| related.view == *view)
                        }) {
                            return;
                        }
                        if let RelatedEvent::Saved(credential) = event {
                            this.select_host_credential(
                                target,
                                if identity(Some(*credential)).is_some() {
                                    AuthField::Username
                                } else {
                                    AuthField::Key
                                },
                                Some(*credential),
                                window,
                                cx,
                            );
                        }
                        let editor = this.editor.as_mut().unwrap();
                        editor.related_credential = None;
                        if let Some(auth) = editor.auth(target) {
                            let focus = if return_to_identity {
                                auth.username.state.read(cx).focus_handle(cx)
                            } else {
                                auth.keys.select.state.read(cx).focus_handle(cx)
                            };
                            focus.focus(window, cx);
                        }
                        cx.notify();
                    },
                );
            },
        );
        self.editor.as_mut().unwrap().related_credential = Some(Related {
            view,
            _events: events,
        });
        cx.notify();
    }
}
