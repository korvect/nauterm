use super::{Client, Device, Generate, KeyType, ValidationError};
use crate::{
    native::Core,
    vault::{KeyRecord, Vault},
};
use gpui_kit::{
    base::input::{InputEvent, InputState},
    prelude::*,
    *,
};
use std::sync::Arc;
use zeroize::Zeroizing;
mod view;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Device,
    Pin,
    Options,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Work {
    Idle,
    Unlocking,
    Generating,
    Saving,
}
#[derive(Clone)]
struct Pending {
    token: String,
    key: KeyRecord,
}
pub enum Event {
    Close,
    Saved(i64),
}
pub struct Wizard {
    scroll: ScrollHandle,
    vault: Vault,
    client: Option<Arc<Client>>,
    dark: bool,
    step: Step,
    work: Work,
    devices: Vec<Device>,
    device: Option<String>,
    loading: bool,
    generation: u64,
    pending: Option<Pending>,
    discard: bool,
    label: Entity<InputState>,
    pin: Entity<InputState>,
    require_pin: bool,
    signature_pin: Entity<InputState>,
    presence: bool,
    key_type: KeyType,
    passphrase: Entity<InputState>,
    save_passphrase: bool,
    was_empty: bool,
    cipher: &'static str,
    rounds: Entity<InputState>,
    error: Option<String>,
    validation: Option<ValidationError>,
    focus: FocusHandle,
    _inputs: Vec<Subscription>,
}
impl EventEmitter<Event> for Wizard {}
impl Focusable for Wizard {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Wizard {
    pub fn new(vault: Vault, dark: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::components::message::watch(window, cx, |view: &Self| {
            view.error
                .clone()
                .map(crate::components::message::Message::error)
        });
        let input = |value: &str, secret, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value.to_owned())
                    .masked(secret)
            })
        };
        let label = input("FIDO2 key", false, window, cx);
        let pin = input("", true, window, cx);
        let signature_pin = input("", true, window, cx);
        let passphrase = input("", true, window, cx);
        let rounds = input("100", false, window, cx);
        let inputs = [&label, &pin, &signature_pin, &passphrase, &rounds]
            .into_iter()
            .map(|field| {
                cx.subscribe_in(
                    field,
                    window,
                    |this: &mut Self, field, event: &InputEvent, window, cx| {
                        if matches!(event, InputEvent::Change) && !this.busy() {
                            this.error = None;
                            this.validation = None;
                            if field == &this.passphrase {
                                let empty = field.read(cx).value().is_empty();
                                if empty {
                                    this.save_passphrase = false;
                                } else if this.was_empty {
                                    this.save_passphrase = true;
                                }
                                this.was_empty = empty;
                            }
                        }
                        if matches!(event, InputEvent::PressEnter { .. })
                            && (field == &this.pin || field == &this.signature_pin)
                        {
                            this.primary(window, cx);
                        }
                        cx.notify();
                    },
                )
            })
            .collect();
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut view = Self {
            scroll: ScrollHandle::new(),
            vault,
            client: None,
            dark,
            step: Step::Device,
            work: Work::Idle,
            devices: vec![],
            device: None,
            loading: false,
            generation: 0,
            pending: None,
            discard: false,
            label,
            pin,
            require_pin: false,
            signature_pin,
            presence: true,
            key_type: Default::default(),
            passphrase,
            save_passphrase: false,
            was_empty: true,
            cipher: "aes256-ctr",
            rounds,
            error: None,
            validation: None,
            focus,
            _inputs: inputs,
        };
        view.refresh(cx);
        view
    }
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.dark != dark {
            self.dark = dark;
            cx.notify();
        }
    }
    pub fn busy(&self) -> bool {
        self.work != Work::Idle || self.pending.is_some()
    }
    pub fn request_close(&mut self, cx: &mut Context<Self>) {
        if self.work != Work::Idle {
            return;
        }
        if self.pending.is_some() {
            self.discard = true;
        } else {
            cx.emit(Event::Close);
        }
        cx.notify();
    }
    fn selected(&self) -> Option<&Device> {
        self.devices
            .iter()
            .find(|device| Some(&device.id) == self.device.as_ref())
    }
    fn validation_hint(&self, error: ValidationError) -> Option<SharedString> {
        (self.validation == Some(error)).then(|| error.message().into())
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.busy() || self.step != Step::Device {
            return;
        }
        self.loading = true;
        self.error = None;
        self.validation = None;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    let client = Arc::new(Core::discover()?.fido2()?);
                    let devices = client.devices();
                    Ok::<_, anyhow::Error>((client, devices))
                })
                .await;
            let _ = view.update(cx, |view, cx| {
                if generation != view.generation || view.step != Step::Device {
                    return;
                }
                view.loading = false;
                match result {
                    Ok((client, devices)) => {
                        view.device = (devices.len() == 1).then(|| devices[0].id.clone());
                        view.client = Some(client);
                        view.devices = devices;
                    }
                    Err(error) => {
                        view.devices.clear();
                        view.device = None;
                        view.error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.work != Work::Idle {
            return;
        }
        if self.pending.is_some() {
            self.save(window, cx);
            return;
        }
        match self.step {
            Step::Device => {
                if self.loading {
                    return;
                }
                let Some(device) = self.selected() else {
                    return;
                };
                self.step = if device.has_pin {
                    Step::Pin
                } else {
                    Step::Options
                };
                self.error = None;
                self.validation = None;
                let input = if self.step == Step::Pin {
                    &self.pin
                } else {
                    &self.label
                };
                input.update(cx, |field, cx| field.focus(window, cx));
            }
            Step::Pin => self.unlock(window, cx),
            Step::Options => self.generate(window, cx),
        }
        cx.notify();
    }
    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.step = match self.step {
            Step::Device | Step::Pin => Step::Device,
            Step::Options if self.selected().is_some_and(|device| device.has_pin) => Step::Pin,
            Step::Options => Step::Device,
        };
        self.error = None;
        self.validation = None;
        self.pin
            .update(cx, |field, cx| field.set_value("", window, cx));
        self.signature_pin
            .update(cx, |field, cx| field.set_value("", window, cx));
        self.require_pin = false;
        if self.step == Step::Pin {
            self.pin.update(cx, |field, cx| field.focus(window, cx));
        } else {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }
    fn unlock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(client), Some(device)) = (self.client.clone(), self.device.clone()) else {
            return;
        };
        let pin = Zeroizing::new(self.pin.read(cx).value().to_string());
        if pin.is_empty() {
            self.validation = Some(ValidationError::Pin);
            return;
        }
        self.work = Work::Unlocking;
        self.error = None;
        self.validation = None;
        cx.spawn_in(window, async move |view, cx| {
            let result = cx
                .background_spawn(async move { client.verify(&device, pin) })
                .await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.work = Work::Idle;
                match result {
                    Ok(()) => {
                        view.pin
                            .update(cx, |field, cx| field.set_value("", window, cx));
                        view.step = Step::Options;
                        view.label.update(cx, |field, cx| field.focus(window, cx));
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn generate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(client), Some(device_id)) = (self.client.clone(), self.device.clone()) else {
            return;
        };
        let passphrase = Zeroizing::new(self.passphrase.read(cx).value().to_string());
        let rounds = self.rounds.read(cx).value().parse::<u32>();
        if !passphrase.is_empty()
            && !rounds
                .as_ref()
                .is_ok_and(|value| (1..=1_000_000).contains(value))
        {
            self.validation = Some(ValidationError::Rounds);
            return;
        }
        let request = Generate {
            device_id,
            label: self.label.read(cx).value().trim().to_owned(),
            key_type: self.key_type,
            pin: Zeroizing::new(self.signature_pin.read(cx).value().to_string()),
            require_presence: self.presence,
            require_pin: self.require_pin,
            passphrase,
            cipher: self.cipher,
            rounds: rounds.unwrap_or(100),
            save_passphrase: self.save_passphrase,
        };
        if let Err(error) = request.validate() {
            self.validation = Some(error);
            return;
        }
        self.work = Work::Generating;
        self.error = None;
        self.validation = None;
        let token = format!("{:032x}", rand::random::<u128>());
        cx.spawn_in(window, async move |view, cx| {
            let result = cx
                .background_spawn(async move { client.generate(request) })
                .await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.work = Work::Idle;
                match result {
                    Ok(key) => {
                        view.pending = Some(Pending { token, key });
                        for field in [&view.pin, &view.signature_pin, &view.passphrase] {
                            field.update(cx, |field, cx| field.set_value("", window, cx));
                        }
                        view.save(window, cx);
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pending) = self.pending.clone() else {
            return;
        };
        let vault = self.vault.clone();
        self.work = Work::Saving;
        self.error = None;
        self.validation = None;
        cx.spawn_in(window, async move |view, cx| {
            let result = vault.save_generated_key(pending.token, pending.key).await;
            let _ = view.update_in(cx, |view, _, cx| {
                view.work = Work::Idle;
                match result {
                    Ok(id) => { view.pending = None; cx.emit(Event::Saved(id)); }
                    Err(error) => view.error = Some(format!("The key was generated but could not be saved: {error}. Retry save to keep this key.")),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}
