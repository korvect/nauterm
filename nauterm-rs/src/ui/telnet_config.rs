//! Independent Telnet settings, using the same credential and theme controls as SSH.
use super::host_credentials::{Authentication, Target};
use super::*;
use crate::{
    host_session::{Encoding, Inheritance},
    telnet::Profile,
    vault::Inventory,
};

pub(super) struct Draft {
    pub enabled: bool,
    pub configured: bool,
    pub profile: Profile,
    pub port: Entity<InputState>,
    pub username: Entity<InputState>,
    pub auth: Authentication,
    pub theme: host_themes::Draft,
    pub inheritance: Inheritance,
    encoding: SelectModel<Encoding>,
    _events: Vec<Subscription>,
}
impl Draft {
    pub fn new(
        host: Option<&Host>,
        inventory: &Inventory,
        window: &mut Window,
        cx: &mut Context<Nauterm>,
    ) -> Self {
        let existing = host.and_then(|host| host.connections.telnet.as_ref());
        let profile = existing.cloned().unwrap_or_default();
        let port = cx.new(|cx| InputState::new(window, cx).default_value(profile.port.to_string()));
        let username =
            cx.new(|cx| InputState::new(window, cx).default_value(profile.username.clone()));
        let auth = Authentication::for_target(
            Target::Telnet,
            profile.identity_id.map(CredentialRef::Identity),
            &username,
            inventory,
            window,
            cx,
        );
        let theme = host_themes::Draft::new(profile.theme, window, cx);
        let encoding = SelectModel::new(encoding_options(), Some(profile.encoding), window, cx);
        let events = vec![
            cx.subscribe(&port, |this, state, event: &InputEvent, _| {
                if matches!(event, InputEvent::Change)
                    && let Some(draft) =
                        this.editor.as_mut().and_then(|editor| editor.host.as_mut())
                    && draft.telnet.port.entity_id() == state.entity_id()
                {
                    draft.telnet.inheritance.port_configured = true;
                }
            }),
            cx.subscribe(
                &encoding.state,
                |this, state, event: &SelectEvent<Encoding>, cx| {
                    if let Some(draft) =
                        this.editor.as_mut().and_then(|editor| editor.host.as_mut())
                        && draft.telnet.encoding.state.entity_id() == state.entity_id()
                        && let SelectEvent::Confirm(Some(Some(value))) = event
                    {
                        draft.telnet.profile.encoding = *value;
                        draft.telnet.inheritance.encoding_configured = true;
                        cx.notify();
                    }
                },
            ),
        ];
        Self {
            enabled: existing.is_some(),
            configured: host.is_some(),
            profile,
            port,
            username,
            auth,
            theme,
            encoding,
            inheritance: Inheritance {
                port_configured: existing.is_some(),
                encoding_configured: existing.is_some(),
                password_configured: false,
            },
            _events: events,
        }
    }
    pub fn value(&self, cx: &App) -> Option<Profile> {
        self.enabled.then(|| Profile {
            port: self.port.read(cx).value().trim().parse().unwrap_or(0),
            username: if self.profile.identity_id.is_some() {
                String::new()
            } else {
                self.username.read(cx).value().trim().to_owned()
            },
            theme: self.theme.selected,
            ..self.profile.clone()
        })
    }
    pub fn validate(&self, cx: &App) -> anyhow::Result<()> {
        if let Some(profile) = self.value(cx) {
            profile.validate()?;
            self.auth.validate(cx)?;
        }
        Ok(())
    }
    pub fn reconcile(&mut self, inventory: &Inventory, window: &mut Window, cx: &mut App) {
        self.auth.reconcile(
            inventory,
            self.profile.identity_id.map(CredentialRef::Identity),
            window,
            cx,
        );
        self.encoding
            .reconcile(encoding_options(), Some(self.profile.encoding), window, cx);
    }
    pub fn inherit(&mut self, parent: &Profile, window: &mut Window, cx: &mut App) {
        if !self.configured {
            self.enabled = true;
            self.configured = true;
        }
        if !self.enabled {
            return;
        }
        let mut profile = self.value(cx).unwrap();
        let previous = profile.password_id.clone();
        self.inheritance.password_configured = self.auth.manual_password_present(cx);
        crate::telnet::inherit_defaults(&mut profile, parent, &mut self.inheritance);
        self.auth
            .inherited_password(&previous, &profile.password_id);
        if self.port.read(cx).value().as_ref() != profile.port.to_string() {
            self.port.update(cx, |input, cx| {
                input.set_value(profile.port.to_string(), window, cx)
            });
        }
        if self.username.read(cx).value().as_ref() != profile.username {
            self.username.update(cx, |input, cx| {
                input.set_value(profile.username.clone(), window, cx)
            });
        }
        self.theme.selected = profile.theme;
        self.profile = profile;
    }
}
fn encoding_options() -> Vec<SelectOption<Encoding>> {
    Encoding::ALL
        .into_iter()
        .map(|value| SelectOption::localized(value, value.label()))
        .collect()
}
impl Nauterm {
    pub(super) fn set_editor_telnet_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Some(draft) = self
            .editor
            .as_mut()
            .and_then(|editor| editor.host.as_mut())
            .filter(|draft| !draft.busy)
        {
            draft.telnet.enabled = enabled;
            draft.telnet.configured = true;
        }
        cx.notify();
    }
    pub(super) fn add_protocol_control(
        &self,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let draft = editor.host.as_ref().unwrap();
        let ssh = !draft.ssh_enabled;
        let telnet = !draft.telnet.enabled;
        if !ssh && !telnet {
            return div().into_any_element();
        }
        let owner = cx.weak_entity();
        Button::new("add-host-protocol")
            .ghost()
            .w_full()
            .disabled(draft.busy)
            .icon(crate::assets::icon(IconName::Plus))
            .label("Add protocol")
            .dropdown_menu(move |mut menu, _, _| {
                if ssh {
                    let owner = owner.clone();
                    menu = menu.item(MenuItem::localized("SSH").on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |this, cx| this.set_editor_ssh_enabled(true, cx));
                    }));
                }
                if telnet {
                    let owner = owner.clone();
                    menu = menu.item(MenuItem::localized("Telnet").on_click(move |_, _, cx| {
                        let _ =
                            owner.update(cx, |this, cx| this.set_editor_telnet_enabled(true, cx));
                    }));
                }
                menu
            })
            .into_any_element()
    }
    pub(super) fn telnet_editor_section(
        &self,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let draft = editor.host.as_ref().unwrap();
        let p = Palette::new(self.data.dark);
        let mut fields = self.protocol_authentication_fields(editor, Target::Telnet, cx);
        fields.push(draft.telnet.encoding.field("Encoding", draft.busy));
        fields.push(
            div()
                .mt(px(2.))
                .pt(px(12.))
                .border_t_1()
                .border_color(p.border)
                .child(self.protocol_theme_section(editor, Target::Telnet, cx))
                .into_any_element(),
        );
        div()
            .rounded(px(10.))
            .bg(p.surface)
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.protocol_editor_header(editor, Target::Telnet, cx))
            .children(fields)
            .into_any_element()
    }
}
