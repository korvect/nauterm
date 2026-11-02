//! The Master Key unlocks management locally and is retained only in memory
//! until the first encrypted sync. It is never a database encryption password.
use super::*;
use zeroize::Zeroizing;

pub(super) struct Key {
    input: Entity<InputState>,
    confirm: Entity<InputState>,
    confirmation_checked: bool,
    pub pending: Option<Zeroizing<String>>,
    _events: Vec<Subscription>,
}
impl Key {
    pub fn new(window: &mut Window, cx: &mut Context<Panel>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("At least 12 characters")
        });
        let confirm = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Repeat Master Key")
        });
        let events = [&input, &confirm]
            .into_iter()
            .enumerate()
            .map(|(index, input)| {
                cx.subscribe_in(
                    input,
                    window,
                    move |this, _, event: &InputEvent, window, cx| {
                        if index == 1
                            && matches!(event, InputEvent::Blur)
                            && this.requires_master_key()
                        {
                            this.key.confirmation_checked = true;
                        }
                        if matches!(event, InputEvent::Change) && this.requires_master_key() {
                            this.validation_error = None;
                        }
                        if matches!(event, InputEvent::PressEnter { .. }) && index == 1 {
                            this.unlock_sync(window, cx);
                        }
                        cx.notify();
                    },
                )
            })
            .collect();
        Self {
            input,
            confirm,
            confirmation_checked: false,
            pending: None,
            _events: events,
        }
    }
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Panel>) {
        self.pending = None;
        self.confirmation_checked = false;
        for field in [&self.input, &self.confirm] {
            field.update(cx, |field, cx| field.set_value("", window, cx));
        }
    }
}
impl Panel {
    pub fn requires_master_key(&self) -> bool {
        self.overview
            .as_ref()
            .is_some_and(|overview| !overview.has_key)
            && self.key.pending.is_none()
    }
    fn unlock_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let value = Zeroizing::new(self.key.input.read(cx).value().to_string());
        let confirmation = Zeroizing::new(self.key.confirm.read(cx).value().to_string());
        self.key.confirmation_checked = true;
        if let Err(error) = crate::sync::validate_master_key(&value) {
            self.validation_error = Some(error.to_string());
            cx.notify();
            return;
        }
        if confirmation_error(&value, &confirmation).is_some() {
            self.validation_error = None;
            cx.notify();
            return;
        }
        self.key.clear(window, cx);
        self.key.pending = Some(value);
        self.error = None;
        self.validation_error = None;
        cx.emit(Event::Unlocked);
        cx.notify();
    }
    pub(super) fn master_key_gate(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::settings(self.dark);
        let value = Zeroizing::new(self.key.input.read(cx).value().to_string());
        let confirmation = Zeroizing::new(self.key.confirm.read(cx).value().to_string());
        let checked = self.key.confirmation_checked;
        let error = checked
            .then(|| confirmation_error(&value, &confirmation))
            .flatten();
        let field = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                Input::new(&self.key.input).aria_label(crate::localization::text(cx, "Master Key")),
            )
            .child(strength(&value, &p));
        let confirm = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                Input::new(&self.key.confirm)
                    .aria_label(crate::localization::text(cx, "Confirm Master Key"))
                    .invalid(error.is_some())
                    .when(checked, |input| {
                        input.suffix(
                            div()
                                .id("sync-master-key-confirmation")
                                .role(Role::Status)
                                .aria_label(error.unwrap_or("Master Keys match."))
                                .flex()
                                .items_center()
                                .child(
                                    Icon::new(if error.is_some() {
                                        IconName::CircleAlert
                                    } else {
                                        IconName::CircleCheck
                                    })
                                    .with_size(px(16.))
                                    .text_color(
                                        if error.is_some() {
                                            rgb(0xe5453d)
                                        } else {
                                            rgb(0x22a55b)
                                        },
                                    ),
                                ),
                        )
                    }),
            )
            .children(crate::components::validation_message(error, cx));
        let content = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(18.))
            .child(self.row(
                "Master Key",
                "Use at least 12 printable ASCII characters and at least 3 character types.",
                field,
            ))
            .child(self.row(
                "Confirm Master Key",
                "Enter the same Master Key again.",
                confirm,
            ))
            .child(
                div().flex().justify_end().child(
                    Button::new("unlock-sync-management")
                        .outline()
                        .label("Continue")
                        .disabled(self.busy())
                        .on_click(cx.listener(|this, _, window, cx| this.unlock_sync(window, cx))),
                ),
            )
            .children(crate::components::validation_message(
                self.validation_error.as_deref(),
                cx,
            ));
        sections::frame(
            "UNLOCK SYNC",
            "UNLOCK SYNC".into(),
            IconName::Key,
            p,
            content,
            None,
        )
    }
}

fn confirmation_error(value: &str, confirmation: &str) -> Option<&'static str> {
    if confirmation.is_empty() {
        Some("Confirm your Master Key.")
    } else if value != confirmation {
        Some("Master Key confirmation does not match.")
    } else {
        None
    }
}

/// Both initial setup and rotation use the same policy and strength feedback.
pub(super) fn strength(value: &str, p: &Palette) -> AnyElement {
    let strength = if crate::sync::validate_master_key(value).is_err() {
        0
    } else if value.len() >= 20 {
        3
    } else if value.len() >= 16 {
        2
    } else {
        1
    };
    let colors = [rgb(0xef4444), rgb(0xf59e0b), p.accent, rgb(0x22a55b)];
    let mut requirements = div().flex().flex_wrap().gap(px(8.));
    for (index, label) in ["Uppercase", "Lowercase", "Numbers", "Symbols"]
        .into_iter()
        .enumerate()
    {
        requirements = requirements.child(
            div()
                .text_size(px(10.))
                .text_color(if crate::sync::master_key_classes(value)[index] {
                    colors[3]
                } else {
                    p.muted
                })
                .child(crate::localization::view(label)),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .when(!value.is_empty(), |view| {
            view.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div().flex_1().h(px(3.)).rounded_full().bg(p.border).child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(colors[strength])
                                .w(relative((strength + 1) as f32 / 4.)),
                        ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(colors[strength])
                            .child(["Weak", "Fair", "Good", "Strong"][strength]),
                    ),
            )
        })
        .child(requirements)
        .into_any_element()
}
