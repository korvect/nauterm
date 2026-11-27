//! The Master Key unlocks management locally and is retained only in memory
//! until the first encrypted sync. It is never a database encryption password.
use super::*;
use crate::localization::{self, Language};
use crate::sync::MasterKeyValidationError;
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
                .placeholder("settings.sync.masterKey.hint")
        });
        let confirm = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("settings.label.repeatMasterKey")
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
            field.update(cx, |field, cx| {
                field.set_value("", window, cx);
                if !field.presentation().is_masked() {
                    field.toggle_masked(window, cx);
                }
            });
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
            self.validation_error = Some(master_key_validation_message(
                self.language,
                "common.label.masterKey",
                error,
            ));
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
        let text = |key| localization::text_for(self.language, key);
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
                Input::new(&self.key.input)
                    .mask_toggle()
                    .aria_label(text("common.label.masterKey")),
            )
            .child(strength(&value, p, self.language));
        let confirm = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                Input::new(&self.key.confirm)
                    .mask_toggle()
                    .aria_label(text("settings.label.confirmMasterKey"))
                    .invalid(error.is_some())
                    .when(checked, |input| {
                        input.suffix(
                            div()
                                .id("sync-master-key-confirmation")
                                .role(Role::Status)
                                .aria_label(text(
                                    error.unwrap_or("settings.sync.masterKey.confirmation.matches"),
                                ))
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
                "common.label.masterKey",
                "settings.sync.masterKey.requirements",
                field,
            ))
            .child(self.row(
                "settings.label.confirmMasterKey",
                "settings.sync.masterKey.confirm.description",
                confirm,
            ))
            .child(
                div().flex().justify_end().child(
                    Button::new("unlock-sync-management")
                        .outline()
                        .label("common.action.continue")
                        .disabled(self.busy())
                        .on_click(cx.listener(|this, _, window, cx| this.unlock_sync(window, cx))),
                ),
            )
            .children(crate::components::validation_message(
                self.validation_error.as_deref(),
                cx,
            ));
        sections::frame(
            "settings.label.unlockSync",
            text("settings.label.unlockSync"),
            IconName::Key,
            p,
            content,
            None,
        )
    }
}

fn confirmation_error(value: &str, confirmation: &str) -> Option<&'static str> {
    if confirmation.is_empty() {
        Some("settings.sync.masterKey.error.confirmationRequired")
    } else if value != confirmation {
        Some("settings.sync.masterKey.error.mismatch")
    } else {
        None
    }
}

pub(super) fn master_key_validation_message(
    language: Language,
    subject_key: &str,
    error: MasterKeyValidationError,
) -> String {
    let key = match error {
        MasterKeyValidationError::Length => "settings.sync.masterKey.error.length",
        MasterKeyValidationError::Characters => "settings.sync.masterKey.error.characters",
        MasterKeyValidationError::CharacterClasses => {
            "settings.sync.masterKey.error.characterClasses"
        }
    };
    let subject = localization::text_for(language, subject_key);
    localization::format_for(language, key, &[("subject", &subject)]).to_string()
}

/// Both initial setup and rotation use the same policy and strength feedback.
pub(super) fn strength(value: &str, p: Palette, language: Language) -> AnyElement {
    let strength = if crate::sync::validate_master_key(value).is_err() {
        0
    } else if value.len() >= 20 {
        3
    } else if value.len() >= 16 {
        2
    } else {
        1
    };
    let colors = [rgb(0xdc2626), rgb(0xd97706), rgb(0x2563eb), rgb(0x16a34a)];
    let color = colors[strength];
    let text = |key| localization::text_for(language, key);
    let level = text(
        [
            "settings.sync.masterKey.strength.weak",
            "settings.sync.masterKey.strength.fair",
            "settings.sync.masterKey.strength.good",
            "settings.sync.masterKey.strength.strong",
        ][strength],
    );
    let caption = format!(
        "{}: {level}",
        text("settings.sync.masterKey.strength.label")
    );
    let requirements = [
        "settings.sync.masterKey.requirement.length",
        "settings.sync.masterKey.requirement.uppercase",
        "settings.sync.masterKey.requirement.lowercase",
        "settings.sync.masterKey.requirement.number",
        "settings.sync.masterKey.requirement.symbol",
    ];
    let satisfied =
        std::iter::once(value.chars().count() >= 12).chain(crate::sync::master_key_classes(value));
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(7.))
        .when(!value.is_empty(), |view| {
            view.child(
                div()
                    .id("sync-master-key-strength")
                    .role(Role::Status)
                    .aria_label(caption.clone())
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .children((0..colors.len()).map(|index| {
                                div()
                                    .flex_1()
                                    .h(px(3.))
                                    .rounded(px(2.))
                                    .bg(if index <= strength { color } else { p.border })
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeights::MEDIUM)
                            .text_right()
                            .text_color(color)
                            .child(caption),
                    ),
            )
        })
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_x(px(10.))
                .gap_y(px(5.))
                .children(requirements.into_iter().zip(satisfied).map(|(label, met)| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .text_size(px(10.))
                        .font_weight(FontWeights::MEDIUM)
                        .text_color(if met { colors[3] } else { p.muted })
                        .child(
                            Icon::new(if met {
                                IconName::CircleCheck
                            } else {
                                IconName::Circle
                            })
                            .with_size(px(12.)),
                        )
                        .child(text(label))
                })),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{Language, MasterKeyValidationError, master_key_validation_message};

    #[test]
    fn master_key_validation_localizes_the_subject_and_reason() {
        assert_eq!(
            master_key_validation_message(
                Language::Chinese,
                "settings.label.currentMasterKey",
                MasterKeyValidationError::Length,
            ),
            "当前主密钥必须至少包含 12 个字符。"
        );
    }
}
