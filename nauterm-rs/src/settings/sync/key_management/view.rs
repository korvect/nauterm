use super::*;
impl Panel {
    pub(in crate::settings::sync) fn key_management_modal(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        Some(self.render_key_modal(self.management.as_ref()?, false, cx))
    }

    pub(in crate::settings::sync) fn change_key_modal(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let management = self.management.as_ref().filter(|state| state.changing)?;
        Some(self.render_key_modal(management, true, cx))
    }

    fn render_key_modal(
        &self,
        management: &Management,
        changing: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = Palette::settings(self.dark);
        let title = if changing {
            "settings.label.changeMasterKey"
        } else {
            "settings.label.syncKey"
        };
        let info = management.info.as_ref();
        let mut body = div().flex().flex_col().gap(px(14.));
        if self.loading {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Spinner::new())
                    .child(crate::localization::text(
                        cx,
                        "rs.ui.copy.loadingKeyDetails",
                    )),
            );
        }
        if changing {
            body = body.child(div().text_size(px(12.)).text_color(p.muted).child(
                crate::localization::text(cx, "settings.sync.keyRotation.warning"),
            ));
            if management.attempted {
                body = body.child(div().text_size(px(12.)).text_color(p.accent).child(
                    crate::localization::text(cx, "Keep both keys until every provider is updated. Retry with this same pair; a failed upload may still have reached the provider.")));
            }
            let field = |label: &'static str, state: &Entity<InputState>| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .child(crate::localization::view(label)),
                    )
                    .child(
                        Input::new(state)
                            .aria_label(crate::localization::text_for(self.language, label))
                            .mask_toggle()
                            .disabled(self.busy())
                            .readonly(management.attempted),
                    )
            };
            let value = Zeroizing::new(management.new.read(cx).value().to_string());
            body = body
                .child(field(
                    "settings.label.currentMasterKey",
                    &management.current,
                ))
                .child(
                    div()
                        .border_t_1()
                        .border_color(p.border)
                        .pt(px(14.))
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(
                            field("settings.label.newMasterKey", &management.new)
                                .child(key::strength(&value, p, self.language)),
                        )
                        .child(field(
                            "settings.label.confirmNewMasterKey",
                            &management.confirm,
                        )),
                );
            body = body.child(div().text_size(px(11.)).text_color(p.muted).child(
                crate::localization::view(if self.preferences.backup_count == 0 {
                    "Local backups are disabled. Providers are updated one at a time."
                } else {
                    "A local backup is created first. Providers are updated one at a time."
                }),
            ));
        } else if let Some(info) = info {
            body = body.child(div().p(px(12.)).flex().items_start().gap(px(10.))
                .border_1().border_color(p.border).rounded(px(8.)).bg(p.sidebar)
                .child(Icon::new(IconName::KeyRound).with_size(px(18.)).text_color(p.accent))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(4.))
                    .child(div().font_weight(FontWeights::MEDIUM).child(crate::localization::view(if info.state.has_key { "Sync DEK is cached on this device" } else { "No Sync DEK cached on this device" })))
                    .child(div().text_size(px(12.)).text_color(p.muted).child(
                        crate::localization::text(cx, "Your Master Key is never stored; it only wraps the random Sync DEK that encrypts cloud data. This cache belongs to Nauterm RS.")))));
        }
        if let Some(info) = info {
            let mut list = div().flex().flex_col().gap(px(7.)).child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeights::MEDIUM)
                    .text_color(p.muted)
                    .child(crate::localization::view(format!(
                        "CONNECTED PROVIDERS · {}",
                        info.targets().len()
                    ))),
            );
            if !info.can_rotate() {
                list = list.child(div().text_size(px(12.)).text_color(p.muted).child(
                    crate::localization::text(cx, "rs.ui.copy.noConnectedProviders"),
                ));
            }
            for target in info.targets() {
                let phase = management.phases.get(&target.scope);
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().flex_1().min_w_0().child(target.name))
                        .when(phase == Some(&Phase::Updating), |row| {
                            row.child(Spinner::new())
                        })
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(if phase == Some(&Phase::Failed) {
                                    rgb(0xef4444)
                                } else {
                                    p.muted
                                })
                                .child(crate::localization::text_for(
                                    self.language,
                                    match phase {
                                        Some(Phase::Updating) => "rs.ui.copy.updating",
                                        Some(Phase::Updated) => "rs.ui.copy.updated2",
                                        Some(Phase::Failed) => "common.label.failed",
                                        None if changing => "settings.sync.keyRotation.pending",
                                        None => "common.label.connected",
                                    },
                                )),
                        ),
                );
            }
            body = body.child(list);
        }
        if changing && !management.completed.is_empty() {
            body = body.child(div().text_size(px(12.)).text_color(p.accent).child(
                crate::localization::view(model::completed_message(&management.completed)),
            ));
        }
        if let Some(notice) = &self.notice {
            body = body.child(
                div()
                    .text_size(px(12.))
                    .text_color(p.accent)
                    .child(crate::localization::message(cx, notice)),
            );
        }
        body = body.children(crate::components::validation_message(
            self.validation_error.as_deref().or(self.error.as_deref()),
            cx,
        ));
        let frame = DialogFrame::new(
            if changing {
                "change-master-key-frame"
            } else {
                "sync-key-management-frame"
            },
            title,
        )
        .colors(self.colors())
        .width(560.)
        .content(body)
        .action(
            Button::new("reload-sync-key-details")
                .outline()
                .label("Reload")
                .disabled(self.busy())
                .on_click(cx.listener(|this, _, window, cx| this.read_key_management(window, cx))),
        );
        if changing {
            let frame = frame
                .action(
                    Button::new("cancel-sync-key-change")
                        .outline()
                        .label("Cancel")
                        .disabled(self.busy())
                        .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                )
                .action(
                    Button::new("apply-sync-key-change")
                        .primary()
                        .label(if self.mutating {
                            "Updating…"
                        } else if management.attempted {
                            "Retry"
                        } else {
                            "settings.label.changeMasterKey"
                        })
                        .disabled(self.busy() || info.is_none_or(|info| !info.can_rotate()))
                        .on_click(cx.listener(|this, _, _, cx| this.request_key_rotation(cx))),
                );
            return Modal::new("change-master-key", title)
                .focus_scope(&management.change_focus)
                .initial_focus(&management.current.read(cx).focus_handle(cx))
                .return_focus(&management.focus)
                .dismiss_on_backdrop(false)
                .dismissible(!self.busy())
                .on_dismiss(cx.listener(|this, _, window, cx| {
                    if let Some(management) = &mut this.management {
                        management.clear_inputs(window, cx);
                        management.changing = false;
                        management.phases.clear();
                        management.completed.clear();
                        // Re-entering a change needs a new reviewed snapshot.
                        management.info = None;
                    }
                    this.error = None;
                    this.validation_error = None;
                    this.read_key_management(window, cx);
                }))
                .content(frame)
                .into_any_element();
        }

        let frame = frame
            .action(
                Button::new("forget-sync-key")
                    .outline()
                    .label("settings.label.forgetMasterKey")
                    .disabled(
                        self.busy()
                            || info.is_none_or(|info| {
                                !info.state.has_key && self.key.pending.is_none()
                            }),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(management) = &mut this.management {
                            management.confirm_forget = true;
                        }
                        this.error = None;
                        this.validation_error = None;
                        cx.notify();
                    })),
            )
            .action(
                Button::new("change-sync-master-key")
                    .outline()
                    .label("settings.label.changeMasterKey")
                    .disabled(self.busy() || info.is_none_or(|info| !info.can_rotate()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(management) = &mut this.management {
                            management.changing = true;
                        }
                        this.error = None;
                        this.validation_error = None;
                        this.notice = None;
                        cx.notify();
                    })),
            )
            .action(
                Button::new("close-sync-key-details")
                    .label("Done")
                    .disabled(self.busy())
                    .on_click(|_, window, cx| Modal::dismiss(window, cx)),
            );
        Modal::new("sync-key-management", title)
            .focus_scope(&management.focus)
            .return_focus(&self.focus)
            .dismissible(!self.busy() && !management.changing && !management.confirm_forget)
            .on_dismiss(cx.listener(|this, _, window, cx| {
                if let Some(management) = &mut this.management {
                    management.clear_inputs(window, cx);
                }
                this.management = None;
                this.error = None;
                this.validation_error = None;
                cx.notify();
            }))
            .content(frame)
            .into_any_element()
    }
    pub(in crate::settings::sync) fn forget_key_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let management = self
            .management
            .as_ref()
            .filter(|management| management.confirm_forget)?;
        let title = "settings.sync.forgetKey.title";
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(crate::localization::text(
                cx,
                "settings.sync.forgetKey.summary",
            ))
            .child(crate::localization::text(
                cx,
                "settings.sync.forgetKey.description",
            ))
            .children(crate::components::validation_message(
                self.error.as_deref(),
                cx,
            ));
        Some(
            Dialog::new("forget-sync-key", title)
                .focus_scope(&management.forget_focus)
                .return_focus(&management.focus)
                .dismissible(!self.busy())
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    if let Some(management) = &mut this.management {
                        management.confirm_forget = false;
                    }
                    this.error = None;
                    this.validation_error = None;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("forget-shared-sync-key-frame", title)
                        .colors(self.colors())
                        .content(body)
                        .action(
                            Button::new("cancel-forget-sync-key")
                                .outline()
                                .label("Cancel")
                                .disabled(self.busy())
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        )
                        .action(
                            Button::new("confirm-forget-sync-key")
                                .danger()
                                .loading(self.mutating)
                                .label("settings.label.forgetMasterKey")
                                .disabled(self.busy())
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.forget_key(window, cx)),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
