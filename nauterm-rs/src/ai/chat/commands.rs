use super::*;
use crate::components::{motion::mix, selectable_text};
use crate::typography::FontWeights;
use crate::{ai::history::View, native::CommandOwner};
use nauterm_core::AiCommandBlockEntry;

fn command_duration(start: i64, end: i64) -> String {
    let seconds = end.saturating_sub(start).max(0) / 1000;
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

fn command_action(id: String, label: &str, primary: bool, disabled: bool, p: Palette) -> Button {
    let idle = mix(p.background, p.control, 0.55);
    let foreground = if disabled {
        p.muted.alpha(0.42)
    } else if primary {
        p.accent
    } else {
        p.muted
    };
    Button::new(SharedString::from(id))
        .custom(
            ButtonCustomVariant::for_surface(p)
                .color(
                    if disabled {
                        idle.alpha(0.46)
                    } else if primary {
                        p.accent.alpha(0.12)
                    } else {
                        idle
                    }
                    .into(),
                )
                .foreground(foreground.into())
                .hover(
                    if primary {
                        p.accent.alpha(0.18)
                    } else {
                        mix(p.background, p.control, 0.85)
                    }
                    .into(),
                )
                .active(if primary { p.accent } else { p.muted }.alpha(0.18).into())
                .highlight(
                    if primary {
                        p.accent.alpha(0.14)
                    } else {
                        p.control
                    }
                    .into(),
                ),
        )
        .h(px(28.))
        .px(px(10.))
        .rounded(px(9.))
        .border_1()
        .border_color(if primary {
            p.accent.alpha(if disabled { 0.08 } else { 0.22 })
        } else {
            p.border.alpha(if disabled { 0.34 } else { 0.76 })
        })
        .text_size(px(11.))
        .line_height(px(15.))
        .font_weight(FontWeights::SEMIBOLD)
        .content_gap(px(4.))
        .icon_size(px(14.))
        .label(label)
        // The custom skin already supplies the reference's disabled colors.
        .opacity(1.)
        .disabled(disabled)
}

impl Chat {
    pub(super) fn command_owner(&self, cx: &App) -> Option<CommandOwner> {
        if self.scope != Scope::Terminal
            || self
                .session
                .as_ref()
                .is_none_or(|session| !session.matches_host(self.host_uuid.as_deref()))
        {
            return None;
        }
        self.source.owner(cx)
    }
    fn run_command(&mut self, id: String, cx: &mut Context<Self>) {
        if self.busy() || self.history_loading || self.editor.is_some() {
            return;
        }
        let result = (|| {
            let session = self
                .session
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("The conversation is unavailable."))?;
            let (owner, _) = session.review(&id, self.command_owner(cx))?;
            let target = self.source.prepare(&owner, cx)?;
            session.execute(id, target)
        })();
        self.error = result.err().map(|error| error.to_string());
        cx.notify();
    }
    fn skip_command(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.history_loading || self.editor.is_some() {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        self.history_loading = true;
        cx.spawn_in(window, async move |this, cx| {
            let result = session.skip(id).await;
            let _ = this.update(cx, |this, cx| {
                this.history_loading = false;
                this.error = result.err().map(|error| error.to_string());
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn sync_command_clock(&mut self, cx: &mut Context<Self>) {
        let running = self
            .session
            .as_ref()
            .is_some_and(|session| session.has_running_command());
        if !running {
            self.command_clock = None;
            return;
        }
        if self.command_clock.is_some() {
            return;
        }
        self.command_clock = Some(cx.spawn(async move |this, cx| {
            loop {
                smol::Timer::after(Duration::from_secs(1)).await;
                let active = this
                    .update(cx, |this, cx| {
                        let active = this.active
                            && this
                                .session
                                .as_ref()
                                .is_some_and(|session| session.has_running_command());
                        if active {
                            cx.notify();
                        } else {
                            this.command_clock = None;
                        }
                        active
                    })
                    .unwrap_or(false);
                if !active {
                    break;
                }
            }
        }));
    }
    pub(super) fn command_card(
        &mut self,
        block: &AiCommandBlockEntry,
        view: &View,
        current: Option<&CommandOwner>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let id = block.tool_call_id.clone();
        let running = block.status == "running";
        let another_running = view
            .entry
            .command_blocks
            .iter()
            .any(|candidate| candidate.tool_call_id != id && candidate.status == "running");
        let busy = view.busy
            || self.history_loading
            || self.editor.is_some()
            || !view.supported
            || running
            || another_running;
        let mine = view.active_command.as_deref() == Some(id.as_str());
        let matches = current.is_some_and(|current| {
            view.command_owner
                .as_ref()
                .is_none_or(|owner| owner.same(current) && owner.validate().is_ok())
        });
        let available = matches && view.entry.host_uuid == self.host_uuid;
        let status = match block.status.as_str() {
            "pending" => "Awaiting approval".into(),
            "running" => if view.stopping { "Stopping" } else { "Running" }.into(),
            "submitted" => "Submitted · result not tracked".into(),
            "cancelled" => "Stopped".into(),
            "skipped" => "Skipped".into(),
            _ => block
                .exit_code
                .map_or_else(|| "Failed".into(), |code| format!("Exited {code}")),
        };
        let elapsed = block.started_at.map(|start| {
            let end = block
                .finished_at
                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
            command_duration(start, end)
        });
        let code = block.command.clone();
        let mut actions = div().flex().items_center().flex_wrap().gap(px(6.));
        if block.status == "pending" {
            let skip = id.clone();
            let run = id.clone();
            actions = actions
                .child(
                    command_action(format!("ai-skip-{id}"), "Skip", false, busy, p).on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.skip_command(skip.clone(), window, cx)
                        }),
                    ),
                )
                .child(
                    command_action(
                        format!("ai-run-{id}"),
                        if !available {
                            "No active terminal"
                        } else if another_running {
                            "Command running"
                        } else {
                            "Run"
                        },
                        true,
                        busy || !available,
                        p,
                    )
                    .icon(IconName::Play)
                    .on_click(cx.listener(move |this, _, _, cx| this.run_command(run.clone(), cx))),
                );
        } else if running {
            actions = actions.child(
                command_action(
                    format!("ai-stop-{id}"),
                    if view.stopping { "Stopping" } else { "Stop" },
                    false,
                    !mine || view.stopping,
                    p,
                )
                .icon(crate::assets::Glyph::Stop.icon())
                .tooltip(if mine {
                    "Stop command"
                } else {
                    "Command belongs to another session"
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(session) = &this.session {
                        session.stop();
                    }
                    cx.notify();
                })),
            );
        }
        let owner = view
            .command_owner
            .as_ref()
            .or(current)
            .map(|owner| owner.label.clone());
        let copy_id = format!("command-{id}");
        let copied = self.copied_text.as_ref() == Some(&copy_id);
        let successful = matches!(block.status.as_str(), "succeeded" | "submitted");
        let status_color = if running || successful {
            p.accent
        } else {
            p.muted
        };
        let status_icon = match block.status.as_str() {
            "succeeded" => Some(IconName::Check),
            "submitted" => Some(IconName::Send),
            "failed" => Some(IconName::CircleAlert),
            "cancelled" => Some(IconName::SquareStop),
            "skipped" => Some(IconName::Ban),
            _ => None,
        };
        let mut card = div()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(12.))
            .rounded(px(12.))
            .border_1()
            .border_color(if running || successful {
                mix(p.border, p.accent, 0.32)
            } else {
                p.border
            })
            .bg(mix(p.background, p.control, 0.72))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .size(px(26.))
                            .rounded(px(8.))
                            .bg(status_color.alpha(0.1))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                gpui_kit::component::Icon::new(IconName::SquareTerminal)
                                    .with_size(px(14.))
                                    .text_color(status_color),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.5))
                            .font_weight(FontWeights::SEMIBOLD)
                            .child(crate::localization::text(cx, "common.label.command")),
                    )
                    .child(
                        Button::new(SharedString::from(format!("ai-copy-command-{id}")))
                            .xsmall()
                            .ghost()
                            .custom(ButtonCustomVariant::for_surface(p))
                            .size(px(20.))
                            .p_0()
                            .icon_size(px(13.))
                            .icon(if copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .text_color(if copied { p.accent } else { p.muted })
                            .tooltip(if copied { "Copied" } else { "Copy command" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.copy_text(copy_id.clone(), code.clone(), cx)
                            })),
                    )
                    .when(running, |row| row.child(Spinner::new().with_size(px(14.))))
                    .when_some(status_icon, |row, icon| {
                        row.child(
                            gpui_kit::component::Icon::new(icon)
                                .with_size(px(16.))
                                .text_color(status_color),
                        )
                    }),
            )
            .when_some(owner, |card, owner| {
                card.child(
                    div()
                        .text_size(px(10.))
                        .text_color(p.muted)
                        .child(selectable_text(
                            SharedString::from(format!("ai-command-owner-{id}")),
                            owner,
                            p,
                        )),
                )
            })
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .px(px(10.))
                    .py(px(9.))
                    .rounded(px(9.))
                    .border_1()
                    .border_color(p.border)
                    .bg(mix(p.control, p.background, 0.55))
                    .border_1()
                    .border_color(p.border.alpha(0.72))
                    .font_family("monospace")
                    .text_size(px(12.))
                    .line_height(relative(1.4))
                    .child(selectable_text(
                        SharedString::from(format!("ai-command-text-{id}")),
                        block.command.clone(),
                        p,
                    )),
            );
        if let Some(output) = &block.output
            && !output.is_empty()
        {
            let scroll = self.command_scroll.entry(id.clone()).or_default().clone();
            card = card.child(
                div()
                    .id(SharedString::from(format!("ai-output-{id}")))
                    .max_h(px(154.))
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .scrollbars_with(&scroll)
                    .hover_only()
                    .p(px(8.))
                    .rounded(px(9.))
                    .bg(p.background)
                    .font_family("monospace")
                    .text_size(px(11.))
                    .line_height(relative(1.4))
                    .child(selectable_text(
                        SharedString::from(format!("ai-command-output-{id}")),
                        output.clone(),
                        p,
                    )),
            );
        }
        card.when_some(block.error.clone(), |card, error| {
            card.child(
                div()
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .child(selectable_text(
                        SharedString::from(format!("ai-command-error-{id}")),
                        error,
                        p,
                    )),
            )
        })
        .child(
            div()
                .flex()
                .items_center()
                .flex_wrap()
                .gap(px(6.))
                .text_size(px(11.))
                .font_weight(FontWeights::MEDIUM)
                .text_color(if successful { p.accent } else { p.muted })
                .justify_end()
                .when(block.status != "pending", |row| {
                    row.child(selectable_text(
                        SharedString::from(format!("ai-command-status-{id}")),
                        crate::localization::text(cx, &status),
                        p,
                    ))
                })
                .when_some(elapsed, |row, elapsed| {
                    row.child(div().text_color(p.muted).child(if running {
                        elapsed
                    } else {
                        format!("· {elapsed}")
                    }))
                })
                .when(block.status == "pending" || running, |row| {
                    row.child(actions)
                }),
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::command_duration;
    #[test]
    fn elapsed_command_time_matches_the_reference_clock() {
        for (millis, clock) in [
            (-1, "0:00"),
            (999, "0:00"),
            (9_000, "0:09"),
            (61_000, "1:01"),
            (3_661_000, "1:01:01"),
        ] {
            assert_eq!(command_duration(1_000, 1_000 + millis), clock);
        }
    }
}
