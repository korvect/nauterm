use super::*;
use crate::{
    components::{
        Button, ButtonCustomVariant, Dialog, DialogColors, DialogFrame, DrawerFrame, EmptyState,
        Field, Modal, Segmented, Spinner, WorkspaceSwitch,
    },
    palette::Palette,
    typography::FontWeights,
};
use gpui_kit::{
    assets::IconName,
    component::{Disableable, Icon, Selectable, Sizable},
};

fn section(title: &'static str, p: Palette) -> Div {
    div()
        .p(px(16.))
        .rounded(px(10.))
        .bg(p.surface)
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .font_weight(FontWeights::MEDIUM)
                .child(crate::localization::view(title)),
        )
}
fn toggle(
    label: &'static str,
    description: &'static str,
    switch: WorkspaceSwitch,
    p: Palette,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(crate::localization::view(label))
                .when(!description.is_empty(), |view| {
                    view.child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .child(crate::localization::view(description)),
                    )
                }),
        )
        .child(switch)
}
impl Wizard {
    fn device_step(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        if self.loading {
            return div()
                .flex_1()
                .min_h(px(240.))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .child(Spinner::new().color(p.muted.into()))
                .child(crate::localization::text(
                    cx,
                    "rs.ui.copy.scanningFidoDevices",
                ))
                .into_any_element();
        }
        if self.devices.is_empty() {
            return EmptyState::new(IconName::Usb, "Insert FIDO2 device", self.dark)
                .description("Connect your FIDO2 device to show here.")
                .action(
                    Button::new("refresh-fido2")
                        .secondary()
                        .icon(IconName::RefreshCw)
                        .label("Refresh")
                        .on_click(cx.listener(|view, _, _, cx| view.refresh(cx))),
                )
                .into_any_element();
        }
        section("Device", p)
            .child(div().text_color(p.muted).child(crate::localization::text(
                cx,
                "rs.ui.copy.selectFidoAuthenticator",
            )))
            .children(self.devices.iter().map(|device| {
                let selected = self.device.as_ref() == Some(&device.id);
                let background = if selected {
                    crate::components::motion::mix(p.surface, p.accent, 0.10)
                } else {
                    p.surface
                };
                let id = device.id.clone();
                Button::new(SharedString::from(format!("fido2-device-{id}")))
                    .w_full()
                    .min_w_0()
                    .h_auto()
                    .p(px(12.))
                    .rounded(px(9.))
                    .selected(selected)
                    .role(Role::RadioButton)
                    .checked(selected)
                    .border_1()
                    .border_color(if selected { p.accent } else { p.border })
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(background.into())
                            .foreground(p.text.into())
                            .hover(p.hover.into())
                            .active(p.hover.into()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(11.))
                            .w_full()
                            .child(
                                div()
                                    .size(px(38.))
                                    .rounded(px(9.))
                                    .bg(p.accent)
                                    .text_color(rgb(0xffffff))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(Icon::new(IconName::Usb).with_size(px(21.))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(FontWeights::MEDIUM)
                                            .child(device.name.clone()),
                                    )
                                    .child(div().text_size(px(12.)).text_color(p.muted).child(
                                        format!(
                                            "{:04X}:{:04X}{}",
                                            device.vendor_id,
                                            device.product_id,
                                            if device.has_pin { " · PIN set" } else { "" }
                                        ),
                                    )),
                            )
                            .when(selected, |view| {
                                view.child(Icon::new(IconName::Check).with_size(px(16.)))
                            }),
                    )
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if view.busy() {
                            return;
                        }
                        view.device = Some(id.clone());
                        view.require_pin = false;
                        view.pin
                            .update(cx, |field, cx| field.set_value("", window, cx));
                        view.signature_pin
                            .update(cx, |field, cx| field.set_value("", window, cx));
                        view.error = None;
                        view.validation = None;
                        cx.notify();
                    }))
            }))
            .child(
                Button::new("rescan-fido2")
                    .ghost()
                    .icon(IconName::RefreshCw)
                    .label("Refresh")
                    .on_click(cx.listener(|view, _, _, cx| view.refresh(cx))),
            )
            .into_any_element()
    }
    fn options(&self, p: Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let disabled = self.busy();
        let protected = !self.passphrase.read(cx).value().is_empty();
        vec![
            section("Key", p)
                .child(
                    Field::input(&self.label, "Label *")
                        .disabled(disabled)
                        .error(self.validation_hint(ValidationError::Label)),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeights::MEDIUM)
                        .child(crate::localization::text(cx, "workspace.keys.field.type")),
                )
                .child(
                    Segmented::new(
                        "fido2-key-type",
                        self.key_type,
                        [KeyType::Ecdsa, KeyType::Ed25519].map(|kind| (kind, kind.label().into())),
                    )
                    .disabled(disabled)
                    .on_change(cx.listener(|view, kind, _, cx| {
                        view.key_type = *kind;
                        cx.notify();
                    })),
                )
                .into_any_element(),
            section("Security", p)
                .child(toggle(
                    "Require user presence",
                    "Touch the security key for every SSH signature.",
                    WorkspaceSwitch::new(
                        "fido2-user-presence",
                        "Require user presence",
                        self.presence,
                    )
                    .disabled(disabled)
                    .on_change(cx.listener(|view, value, _, cx| {
                        view.presence = *value;
                        cx.notify();
                    })),
                    p,
                ))
                .when(
                    self.selected().is_some_and(|device| device.has_pin),
                    |view| {
                        view.child(toggle(
                            "Require PIN",
                            "Require the authenticator PIN for every SSH signature.",
                            WorkspaceSwitch::new(
                                "fido2-require-pin",
                                "Require PIN",
                                self.require_pin,
                            )
                            .disabled(disabled)
                            .on_change(cx.listener(
                                |view, value, window, cx| {
                                    view.require_pin = *value;
                                    if !*value {
                                        view.signature_pin.update(cx, |field, cx| {
                                            field.set_value("", window, cx)
                                        });
                                        if view.validation == Some(ValidationError::Pin) {
                                            view.validation = None;
                                        }
                                    }
                                    cx.notify();
                                },
                            )),
                            p,
                        ))
                    },
                )
                .when(self.require_pin, |view| {
                    view.child(
                        Field::input(&self.signature_pin, "PIN")
                            .secret(true)
                            .disabled(disabled)
                            .error(self.validation_hint(ValidationError::Pin)),
                    )
                })
                .into_any_element(),
            section("Private key protection", p)
                .child(
                    Field::input(&self.passphrase, "Passphrase")
                        .secret(true)
                        .disabled(disabled),
                )
                .when(protected, |view| {
                    view.child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeights::MEDIUM)
                            .child(crate::localization::text(cx, "rs.ui.copy.cipher")),
                    )
                    .child(
                        Segmented::new(
                            "fido2-cipher",
                            self.cipher,
                            [
                                ("aes256-ctr", "AES-256".into()),
                                ("aes128-ctr", "AES-128".into()),
                            ],
                        )
                        .disabled(disabled)
                        .on_change(cx.listener(|view, value, _, cx| {
                            view.cipher = *value;
                            if view.validation == Some(ValidationError::Cipher) {
                                view.validation = None;
                            }
                            cx.notify();
                        })),
                    )
                    .child(
                        Field::input(&self.rounds, "Rounds")
                            .digits_only()
                            .disabled(disabled)
                            .error(self.validation_hint(ValidationError::Rounds)),
                    )
                    .child(div().text_size(px(11.)).text_color(p.muted).child(
                        crate::localization::text(cx, "rs.ui.copy.privateKeyStrength"),
                    ))
                    .child(toggle(
                        "Save passphrase",
                        "",
                        WorkspaceSwitch::new(
                            "fido2-save-passphrase",
                            "Save passphrase",
                            self.save_passphrase,
                        )
                        .disabled(disabled)
                        .on_change(cx.listener(|view, value, _, cx| {
                            view.save_passphrase = *value;
                            cx.notify();
                        })),
                        p,
                    ))
                })
                .into_any_element(),
        ]
    }
}
impl Render for Wizard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(self.dark);
        let can_continue = self.work == Work::Idle
            && (self.pending.is_some()
                || match self.step {
                    Step::Device => self.device.is_some() && !self.loading,
                    Step::Pin => !self.pin.read(cx).value().is_empty(),
                    Step::Options => {
                        !self.require_pin || !self.signature_pin.read(cx).value().is_empty()
                    }
                });
        let sections = match self.step {
            Step::Device => vec![self.device_step(p, cx)],
            Step::Pin => vec![
                section("Unlock FIDO2 security key", p)
                    .child(
                        div()
                            .text_color(p.muted)
                            .child(crate::localization::text(cx, "rs.ui.copy.enterDevicePin")),
                    )
                    .child(
                        Field::input(&self.pin, "PIN")
                            .secret(true)
                            .disabled(self.busy())
                            .error(self.validation_hint(ValidationError::Pin)),
                    )
                    .into_any_element(),
            ],
            Step::Options => self.options(p, cx),
        };
        let form_error = self
            .validation
            .filter(|error| matches!(error, ValidationError::Device | ValidationError::Cipher))
            .map(ValidationError::message);
        div().h_full().min_h_0().relative().track_focus(&self.focus)
            .child(DrawerFrame::new("fido2-editor", "Generate FIDO2 key", self.dark)
                .scroll_handle(&self.scroll)
                .action(Button::new("close-fido2-editor").toolbar().ghost().icon(IconName::ArrowRightToLine).tooltip("Close editor").disabled(self.work != Work::Idle)
                    .on_click(cx.listener(|view, _, _, cx| view.request_close(cx))))
                .sections(sections)
                .when(self.work == Work::Generating, |frame| frame.section(section("Authenticator", p).child(crate::localization::text(cx, "rs.ui.copy.confirmFidoDevice"))))
                .when(self.pending.is_some() && self.work == Work::Idle, |frame| frame.section(section("Generated key", p).child(crate::localization::text(cx, "rs.ui.copy.keyWaitingToSave"))))
                .when_some(crate::components::validation_message(form_error, cx), |frame, error| frame.section(error))
                .when(self.step != Step::Device || !self.devices.is_empty(), |frame| frame.footer(div().flex().gap(px(10.))
                    .when(self.step != Step::Device, |view| view.child(Button::new("fido2-back").large().h(px(38.)).label("Back").disabled(self.busy()).on_click(cx.listener(|view, _, window, cx| view.back(window, cx)))))
                    .child(Button::new("fido2-primary").large().h(px(38.)).flex_1().min_w_0().primary().disabled(!can_continue).loading(self.work != Work::Idle)
                        .label(match self.work { Work::Unlocking => "Unlocking…", Work::Generating => "Waiting for security key…", Work::Saving => "Saving…", Work::Idle if self.pending.is_some() => "Retry save", Work::Idle if self.step == Step::Options => "Generate", Work::Idle => "Continue" })
                        .on_click(cx.listener(|view, _, window, cx| view.primary(window, cx)))))))
            .when(self.discard, |view| view.child(Dialog::new("discard-generated-key", "Discard generated key?").return_focus(&self.focus)
                .on_dismiss(cx.listener(|view, _, _, cx| { view.discard = false; cx.notify(); }))
                .content(DialogFrame::new("discard-generated-key-frame", "Discard generated key?").width(380.).colors(DialogColors::workspace(self.dark))
                    .content("This key was generated on the authenticator but has not been saved. Discarding it loses this app's key handle.")
                    .action(Button::new("keep-generated-key").ghost().label("Keep key").on_click(|_, window, cx| Modal::dismiss(window, cx)))
                    .action(Button::new("confirm-discard-generated-key").danger().label("Discard").on_click(cx.listener(|view, _, _, cx| { view.pending = None; view.discard = false; cx.emit(Event::Close); cx.notify(); }))))))
    }
}
