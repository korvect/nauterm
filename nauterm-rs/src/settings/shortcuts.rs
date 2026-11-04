use super::*;
use crate::shortcuts::{Binding, Command, Scope};
use crate::typography::FontWeights;
use gpui_kit::component::Disableable;

impl Settings {
    pub(super) fn shortcut_recorder(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (FocusHandle, Subscription, Subscription) {
        let focus = cx.focus_handle();
        let capture = focus.clone();
        let owner = cx.weak_entity();
        let window_id = window.window_handle().window_id();
        let keys = cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle().window_id() != window_id || !capture.is_focused(window) {
                return;
            }
            let _ = owner.update(cx, |this, cx| {
                let Some(command) = this.shortcut_recording else {
                    return;
                };
                if event.keystroke.key == "escape" {
                    this.shortcut_recording = None;
                    this.focus.focus(window, cx);
                    cx.notify();
                } else if let Some(binding) =
                    Binding::record(&event.keystroke, command.scope() == Scope::Terminal)
                {
                    this.values.shortcuts.set(command, &binding);
                    this.shortcut_recording = None;
                    this.focus.focus(window, cx);
                    this.changed(window, cx);
                }
            });
            // Recording owns all keys, including Escape, Cmd/Ctrl-W and Quit.
            // No application action or text editor sees the recorded chord.
            window.prevent_default();
            cx.stop_propagation();
        });
        let blur = cx.on_blur(&focus, window, |this, _, cx| {
            if this.shortcut_recording.take().is_some() {
                cx.notify();
            }
        });
        (focus, keys, blur)
    }

    pub(super) fn shortcut_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::settings(self.values.dark);
        let faint = Palette::settings_faint(self.values.dark);
        let field_hover = if self.values.dark {
            rgb(0x3a3a3a)
        } else {
            p.hover
        };
        let column = |weight| div().flex_1().flex_grow(weight).min_w_0();
        let heading = |title: &'static str, weight| {
            column(weight)
                .id(SharedString::from(format!("shortcut-heading-{title}")))
                .role(Role::ColumnHeader)
                .aria_label(title)
                .text_size(px(10.))
                .line_height(px(14.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x6b7280))
                .child(TrackedText::new(title, px(0.7)))
        };
        let mut table = div()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .rounded(px(9.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .shadow(vec![gpui_kit::base::box_shadow(
                px(0.),
                px(2.),
                px(8.),
                px(0.),
                rgba(0x0000000a).into(),
            )])
            .child(
                div()
                    .h(px(44.))
                    .flex_shrink_0()
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .bg(if self.values.dark {
                        rgb(0x343434)
                    } else {
                        p.sidebar
                    })
                    .rounded_t(px(8.))
                    .child(heading("SCOPE", 2.))
                    .child(heading("ACTION", 4.))
                    .child(heading("SHORTCUT", 3.))
                    .child(div().w(px(72.)).flex_shrink_0()),
            );
        for (index, command) in Command::ALL.into_iter().enumerate() {
            let recording = self.shortcut_recording == Some(command);
            let scope = match command {
                Command::Tab(_) | Command::PreviousTab | Command::NextTab | Command::CloseTab => {
                    "Tabs"
                }
                Command::Copy
                | Command::Paste
                | Command::SelectAll
                | Command::Search
                | Command::SplitRight
                | Command::SplitDown
                | Command::NewLocalTerminal => "Terminal",
                _ => "Workspace",
            };
            let raw = self.values.shortcuts.raw(command);
            let label = command.label();
            let recorder = Button::new(("record-shortcut", index))
                .w_full()
                .min_w_0()
                .h(px(28.))
                .px(px(9.))
                .rounded(px(6.))
                .border_1()
                .border_color(if recording {
                    p.accent.alpha(0.40)
                } else {
                    p.border
                })
                .content_start()
                .text_size(px(11.))
                .line_height(px(16.))
                .font_family(crate::terminal_appearance::platform_family())
                // Numeric monospace 500 must not inherit the macOS system-face adapter.
                .font_weight(FontWeights::MEDIUM)
                .custom(
                    crate::components::ButtonCustomVariant::for_surface(p)
                        .color(
                            if recording {
                                p.accent.alpha(0.14)
                            } else {
                                field_hover
                            }
                            .into(),
                        )
                        .foreground(if recording { p.accent } else { p.muted }.into())
                        .hover(
                            if recording {
                                p.accent.alpha(0.14)
                            } else {
                                field_hover
                            }
                            .into(),
                        )
                        .active(p.accent.alpha(0.12).into())
                        .highlight(p.accent.alpha(0.08).into()),
                )
                .localized_accessibility_label(format!("Change {label} shortcut"))
                .tooltip("Click to record a shortcut. Escape cancels recording.")
                .when(recording, |button| {
                    button
                        .track_focus(&self.shortcut_focus)
                        .on_mouse_down_out(cx.listener(
                            move |this, event: &MouseDownEvent, window, cx| {
                                if event.button == MouseButton::Left
                                    && this.shortcut_recording == Some(command)
                                {
                                    this.shortcut_recording = None;
                                    this.focus.focus(window, cx);
                                    cx.notify();
                                }
                            },
                        ))
                })
                .label(if recording {
                    "Press shortcut…".into()
                } else {
                    self.values.shortcuts.label(command).replace(" + ", " ")
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    if this.shortcut_recording == Some(command) {
                        this.shortcut_recording = None;
                        this.focus.focus(window, cx);
                    } else {
                        this.shortcut_recording = Some(command);
                        this.shortcut_focus.focus(window, cx);
                    }
                    cx.notify();
                }));
            let row_action = |id, icon, enabled| {
                Button::new((id, index))
                    .w(px(28.))
                    .h(px(28.))
                    .p_0()
                    .rounded(px(6.))
                    .icon(icon)
                    .icon_size(px(15.))
                    .custom(
                        crate::components::ButtonCustomVariant::for_surface(p)
                            .foreground(if enabled { p.muted } else { faint }.into())
                            .hover(field_hover.into())
                            .active(p.accent.alpha(0.12).into()),
                    )
                    .disabled(!enabled)
            };
            table = table.child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .bg(p.surface)
                    .border_t_1()
                    .border_color(p.border)
                    .when(index + 1 == Command::ALL.len(), |row| row.rounded_b(px(8.)))
                    .child(
                        column(2.)
                            .text_size(px(11.))
                            .line_height(px(16.))
                            .font_weight(FontWeights::MEDIUM)
                            .text_color(faint)
                            .truncate()
                            .child(crate::localization::view(scope)),
                    )
                    .child(
                        column(4.)
                            .text_size(px(13.))
                            .line_height(px(18.))
                            .font_weight(FontWeights::MEDIUM)
                            .truncate()
                            .child(crate::localization::view(label)),
                    )
                    .child(column(3.).child(recorder))
                    .child(
                        div()
                            .w(px(72.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(4.))
                            .child(
                                row_action("disable-shortcut", IconName::Ban, !raw.is_empty())
                                    .tooltip("Disable shortcut")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.shortcut_recording = None;
                                        this.values.shortcuts.set(command, "");
                                        this.changed(window, cx);
                                    })),
                            )
                            .child(
                                row_action(
                                    "reset-shortcut",
                                    IconName::RotateCcw,
                                    raw != command.default_binding(),
                                )
                                .tooltip("Restore default")
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.shortcut_recording = None;
                                        this.values
                                            .shortcuts
                                            .set(command, &command.default_binding());
                                        this.changed(window, cx);
                                    },
                                )),
                            ),
                    ),
            );
        }
        let reset = Button::new("reset-all-shortcuts")
            .ghost()
            .h(px(26.))
            .px(px(8.))
            .rounded(px(6.))
            .content_gap(px(5.))
            .icon(IconName::RotateCcw)
            .icon_size(px(13.))
            .text_size(px(11.))
            .line_height(px(16.))
            .text_color(p.muted)
            .label("Reset all")
            .tooltip("Restore all default shortcuts")
            .on_click(cx.listener(|this, _, window, cx| {
                this.shortcut_recording = None;
                this.values.shortcuts.reset();
                this.changed(window, cx);
            }));
        sections::stack()
            .child(
                self.section(
                    Section::Keyboard,
                    IconName::Keyboard,
                    self.row(
                        if cfg!(target_os = "macos") {
                            "Use Option as Meta Key"
                        } else {
                            "Use Alt as Meta Key"
                        },
                        "Send Option/Alt key combinations with an Escape prefix.",
                        Switch::new(
                            "option-as-meta",
                            "Use Option/Alt as Meta Key",
                            self.values.terminal.use_option_as_meta_key,
                        )
                        .on_change(cx.listener(
                            |this, value, window, cx| {
                                this.values.terminal.use_option_as_meta_key = *value;
                                this.changed(window, cx);
                            },
                        )),
                    ),
                ),
            )
            .child(self.section_with_action(
                Section::Bindings,
                IconName::Keyboard,
                table,
                Some(reset.into_any_element()),
            ))
            .into_any_element()
    }
}
