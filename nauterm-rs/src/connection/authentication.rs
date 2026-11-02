//! Connection prompts reuse Keychain metadata and its existing nested editor.
//! Secrets are resolved by the vault worker only after explicit submission.
use super::*;
use crate::typography::FontWeights;
use crate::{
    components::{Spinner, management::ButtonGroup, menu::MenuItem},
    keychain::{Keychain, Kind, RelatedEvent},
    model::CredentialRef,
    vault::{
        Vault,
        authentication::{self as credentials, Binding, Catalog, Credential},
    },
};
use gpui_kit::component::Icon;

#[derive(Default)]
pub(super) struct State {
    pub(super) vault: Option<Vault>,
    pub(super) keychain: Option<WeakEntity<Keychain>>,
    binding: Option<Binding>,
    catalog: Option<Catalog>,
    selected: Option<i64>,
    loaded: bool,
    loaded_keys: bool,
    pub(super) loading: bool,
    pub busy: bool,
    generation: u64,
    task: Option<Task<()>>,
    pub(super) related: Option<(Entity<Keychain>, Subscription)>,
}
impl State {
    pub fn invalidate(&mut self) {
        self.generation += 1;
        self.task = None;
        self.catalog = None;
        self.loaded = false;
        self.loaded_keys = false;
        self.loading = false;
        self.busy = false;
    }
}
impl Connection {
    pub(crate) fn uses_ssh_authentication(&self) -> bool {
        self.target.ssh().is_some()
    }
    pub(crate) fn with_authentication(
        mut self,
        vault: Option<Vault>,
        keychain: Option<WeakEntity<Keychain>>,
        binding: Option<Binding>,
    ) -> Self {
        self.auth.vault = vault;
        self.auth.keychain = keychain;
        self.auth.binding = binding.filter(|binding| {
            self.target
                .ssh()
                .is_some_and(|options| binding.matches(options))
        });
        self
    }
    pub(crate) fn configure_authentication(
        &mut self,
        vault: Option<Vault>,
        keychain: WeakEntity<Keychain>,
        binding: Option<Binding>,
        cx: &mut Context<Self>,
    ) {
        self.auth.invalidate();
        self.auth.vault = vault;
        self.auth.keychain = Some(keychain);
        self.auth.binding = binding.filter(|binding| {
            self.target
                .ssh()
                .is_some_and(|options| binding.matches(options))
        });
        cx.notify();
    }
    pub(super) fn ensure_authentication(&mut self, cx: &mut Context<Self>) {
        if self.state.phase != Phase::Authentication || self.requires_fido2_pin() {
            return;
        }
        let keys = self.mode == AuthMode::PublicKey;
        if self.auth.loading
            || self.auth.busy
            || self.auth.loaded && (!keys || self.auth.loaded_keys)
        {
            return;
        }
        let Some(vault) = self.auth.vault.clone() else {
            return;
        };
        let binding = self.auth.binding.clone();
        self.auth.loading = true;
        self.auth.generation += 1;
        let generation = self.auth.generation;
        self.auth.task = Some(cx.spawn(async move |this, cx| {
            let result = vault.authentication_catalog(binding, keys).await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.auth.generation {
                    return;
                }
                this.auth.loading = false;
                this.auth.loaded = true;
                this.auth.loaded_keys = keys;
                match result {
                    Ok(catalog) => {
                        if this.auth.selected.is_none() {
                            this.auth.selected = catalog.selected_key;
                        }
                        this.error = catalog.save_error.clone();
                        this.auth.catalog = Some(catalog);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }
    pub(super) fn needs_key_passphrase(&self) -> bool {
        self.state
            .logs
            .iter()
            .rev()
            .take_while(|log| log.event.kind != "connect_start")
            .any(|log| {
                log.event.kind == "auth_passphrase_required"
                    && log.event.method.as_deref() != Some("fido2")
            })
    }
    fn can_authenticate(&self, cx: &App) -> bool {
        if self.auth.busy {
            return false;
        }
        if self.requires_fido2_pin() {
            return !self.fido2_pin.read(cx).value().is_empty();
        }
        let username = self.username.read(cx).value();
        if username.trim().is_empty() || username.chars().any(char::is_control) {
            return false;
        }
        match self.mode {
            AuthMode::Password => !self.password.read(cx).value().is_empty(),
            AuthMode::PublicKey => self
                .auth
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.keys.as_ref())
                .is_some_and(|keys| {
                    keys.iter()
                        .any(|key| Some(key.summary.id) == self.auth.selected)
                }),
        }
    }
    pub(super) fn authenticate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.submit_authentication(self.auth.binding.is_some(), window, cx);
    }
    fn submit_authentication(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_authenticate(cx) {
            return;
        }
        if self.requires_fido2_pin() {
            let Some(options) = self.target.ssh_mut() else {
                return;
            };
            options.fido2_pin = Zeroizing::new(self.fido2_pin.read(cx).value().to_string());
            options.password.zeroize();
            self.fido2_pin
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.start(window, cx);
            return;
        }
        let Some(mut options) = self.target.ssh().cloned() else {
            return;
        };
        options.username = self.username.read(cx).value().trim().to_owned();
        let credential = match self.mode {
            AuthMode::Password => {
                Credential::Password(Zeroizing::new(self.password.read(cx).value().to_string()))
            }
            AuthMode::PublicKey => {
                let Some(key) = self
                    .auth
                    .catalog
                    .as_ref()
                    .and_then(|catalog| catalog.keys.as_ref())
                    .and_then(|keys| {
                        keys.iter()
                            .find(|key| Some(key.summary.id) == self.auth.selected)
                    })
                    .cloned()
                else {
                    return;
                };
                Credential::Key {
                    key,
                    passphrase: self
                        .needs_key_passphrase()
                        .then(|| Zeroizing::new(self.passphrase.read(cx).value().to_string()))
                        .filter(|passphrase| !passphrase.is_empty()),
                }
            }
        };
        let save_target = if save {
            let Some(target) = self
                .auth
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.save_target.clone())
            else {
                return;
            };
            Some(target)
        } else {
            None
        };
        // An explicit password-only Continue is independent of Keychain
        // availability and does not need to open a database transaction.
        let credential = match credential {
            Credential::Password(password) if !save => {
                match credentials::apply_password(&mut options, password) {
                    Ok(()) => self.finish_authentication(options, window, cx),
                    Err(error) => {
                        self.error = Some(error.to_string());
                        cx.notify();
                    }
                }
                return;
            }
            credential => credential,
        };
        let Some(vault) = self.auth.vault.clone() else {
            self.error = Some("Keychain is unavailable. Open Keychain and retry.".into());
            cx.notify();
            return;
        };
        self.auth.busy = true;
        self.error = None;
        self.auth.generation += 1;
        let generation = self.auth.generation;
        self.auth.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = vault
                .authenticate_connection(options, credential, save_target)
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if generation != this.auth.generation {
                    return;
                }
                this.auth.busy = false;
                match result {
                    Ok(options) => {
                        if save && let Some(owner) = &this.auth.keychain {
                            let _ = owner.update(cx, |owner, cx| owner.refresh(cx));
                        }
                        this.finish_authentication(options, window, cx);
                    }
                    Err(error) => {
                        this.error = Some(error.to_string());
                        cx.notify();
                    }
                }
            });
        }));
        cx.notify();
    }
    fn finish_authentication(
        &mut self,
        options: SshOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.target.ssh_mut() else {
            return;
        };
        *target = options;
        for input in [&self.password, &self.passphrase] {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.start(window, cx);
    }
    pub(super) fn add_authentication_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.auth.busy || self.auth.related.is_some() {
            return;
        }
        let source = self.auth.keychain.as_ref().and_then(|owner| {
            owner
                .update(cx, |owner, cx| owner.related_source(cx))
                .ok()
                .flatten()
        });
        let Some(source) = source else {
            return;
        };
        let view = cx.new(|cx| Keychain::related(source, Kind::Key, "", "", window, cx));
        let events = cx.subscribe_in(
            &view,
            window,
            |this, view, event: &RelatedEvent, window, cx| {
                if this
                    .auth
                    .related
                    .as_ref()
                    .is_none_or(|(current, _)| current != view)
                {
                    return;
                }
                if let RelatedEvent::Saved(CredentialRef::Key(id)) = event {
                    this.auth.selected = Some(*id);
                    this.auth.invalidate();
                    this.error = None;
                }
                this.auth.related = None;
                this.focus.focus(window, cx);
                cx.notify();
            },
        );
        self.auth.related = Some((view, events));
        cx.notify();
    }
    pub(super) fn authentication_keys(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(self.dark);
        let keys = self
            .auth
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.keys.as_ref());
        if self.auth.loading
            || keys.is_none() && self.auth.vault.is_some() && !self.auth.loaded_keys
        {
            return div()
                .h(px(64.))
                .flex()
                .items_center()
                .justify_center()
                .child(Spinner::new())
                .into_any_element();
        }
        let Some(keys) = keys.filter(|keys| !keys.is_empty()) else {
            return div()
                .h(px(64.))
                .rounded(px(13.))
                .bg(p.surface)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.))
                .text_color(p.muted)
                .child(crate::localization::text(
                    cx,
                    "workspace.description.noPublicKeysAvailable",
                ))
                .into_any_element();
        };
        div()
            .id("authentication-key-list")
            .min_h_0()
            .max_h(px(220.))
            .overflow_y_scroll()
            .scrollbars()
            .flex()
            .flex_col()
            .gap(px(10.))
            .children(keys.iter().map(|key| {
                let id = key.summary.id;
                let selected = self.auth.selected == Some(id);
                let subtitle = key.summary.algorithm.clone();
                Button::new(("authentication-key", id as u64))
                    .h(px(64.))
                    .w_full()
                    .flex_shrink_0()
                    .px(px(14.))
                    .rounded(px(13.))
                    .disabled(self.auth.busy)
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(p.surface.into())
                            .foreground(p.text.into())
                            .hover(p.hover.into())
                            .active(p.control.into()),
                    )
                    .border_1()
                    .border_color(if selected { p.accent } else { p.surface })
                    .child(
                        div()
                            .flex()
                            .w_full()
                            .items_center()
                            .gap(px(14.))
                            .child(
                                div()
                                    .size(px(40.))
                                    .flex_shrink_0()
                                    .rounded(px(10.))
                                    .bg(rgb(0x00649d))
                                    .text_color(rgb(0xffffff))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(Icon::new(IconName::KeyRound).with_size(px(24.))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .items_start()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .w_full()
                                            .truncate()
                                            .text_size(px(14.))
                                            .font_weight(FontWeights::SEMIBOLD)
                                            .child(key.summary.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .truncate()
                                            .text_size(px(11.))
                                            .font_weight(FontWeights::MEDIUM)
                                            .text_color(p.muted)
                                            .child(subtitle),
                                    ),
                            )
                            .when(selected, |view| {
                                view.child(
                                    Icon::new(IconName::CircleCheck)
                                        .with_size(px(18.))
                                        .text_color(p.accent),
                                )
                            }),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.auth.selected != Some(id) {
                            this.passphrase
                                .update(cx, |input, cx| input.set_value("", window, cx));
                        }
                        this.auth.selected = Some(id);
                        this.error = None;
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }
    pub(super) fn authentication_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let can_continue = self.can_authenticate(cx);
        if self.requires_fido2_pin() || self.auth.binding.is_none() {
            return self
                .action("authenticate", "Continue", true)
                .disabled(!can_continue)
                .loading(self.auth.busy)
                .on_click(
                    cx.listener(|this, _, window, cx| {
                        this.submit_authentication(false, window, cx)
                    }),
                )
                .into_any_element();
        }
        let owner = cx.weak_entity();
        ButtonGroup::new()
            .controls()
            .button(
                self.action("authenticate-save", "Continue & Save", true)
                    .disabled(
                        !can_continue
                            || self
                                .auth
                                .catalog
                                .as_ref()
                                .is_none_or(|catalog| catalog.save_target.is_none()),
                    )
                    .loading(self.auth.busy)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.submit_authentication(true, window, cx)
                    })),
            )
            .dropdown(
                self.action("authenticate-menu", "", true)
                    .w(px(if self.compact_controls() { 24. } else { 30. }))
                    .px_0()
                    .icon(IconName::ChevronDown)
                    .disabled(!can_continue),
                move |menu, _, _| {
                    let owner = owner.clone();
                    menu.item(
                        MenuItem::localized("Continue").on_click(move |_, window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.submit_authentication(false, window, cx)
                            });
                        }),
                    )
                },
            )
            .into_any_element()
    }
}
