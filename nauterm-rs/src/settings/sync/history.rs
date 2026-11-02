//! A history browser is a content Modal; restoring is a separate decision Dialog.
use super::*;
use crate::sync::{
    Provider,
    history::{self as model, RestorePlan, Snapshot},
};
use std::sync::Arc;

pub(super) struct History {
    target: Provider,
    snapshot: Option<Arc<Snapshot>>,
    confirm: Option<RestorePlan>,
    focus: FocusHandle,
    confirm_focus: FocusHandle,
    scroll: ScrollHandle,
}
impl Panel {
    pub(super) fn open_history(
        &mut self,
        target: Provider,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() || !target.connected || !target.supports_history {
            return;
        }
        self.history = Some(History {
            target,
            snapshot: None,
            confirm: None,
            focus: cx.focus_handle(),
            confirm_focus: cx.focus_handle(),
            scroll: ScrollHandle::default(),
        });
        self.error = None;
        self.validation_error = None;
        self.notice = None;
        self.read_history(window, cx);
    }
    fn read_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(history) = &self.history else {
            return;
        };
        let scope = history.target.id.clone();
        let Some(service) = self.service.clone() else {
            return;
        };
        self.loading = true;
        self.error = None;
        self.validation_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.history(scope.clone()).await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.loading = false;
                let success = result.is_ok();
                if let Some(history) = this
                    .history
                    .as_mut()
                    .filter(|history| history.target.id == scope)
                {
                    match result {
                        Ok(snapshot) => history.snapshot = Some(snapshot),
                        Err(error) => this.error = Some(error.to_string()),
                    }
                }
                cx.emit(Event::Settled(success));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn run_version_restore(
        &mut self,
        plan: RestorePlan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            self.operation_failed("Sync storage is unavailable.".into(), cx);
            return;
        };
        self.mutating = true;
        self.error = None;
        self.validation_error = None;
        self.notice = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.restore_version(plan).await;
            let _ = this.update_in(cx, |this, window, cx| {
                let completion = this.finish_sync_attempt(result, true, window, cx);
                if completion.applied
                    && let Some(history) = &mut this.history
                {
                    Modal::complete(&history.confirm_focus, window, cx);
                    history.confirm = None;
                    history.snapshot = None;
                }
                if completion.clean {
                    this.read_history(window, cx);
                } else {
                    cx.emit(Event::Settled(false));
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn history_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let history = self.history.as_ref()?;
        let p = Palette::settings(self.dark);
        let title = format!("{} History", history.target.name);
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().text_size(px(12.)).text_color(p.muted).child(
            crate::localization::text(cx, "Encrypted versions retained by this provider. Up to 100 recent versions are shown."),
        ));
        if self.loading {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Spinner::new())
                    .child(crate::localization::text(cx, "rs.ui.copy.loadingHistory")),
            );
        }
        if let Some(snapshot) = &history.snapshot {
            if !snapshot.can_restore {
                body = body.child(div().text_size(px(12.)).text_color(p.muted).child(
                    crate::localization::text(cx, "rs.ui.copy.completeSyncBeforeRestore"),
                ));
            }
            let mut list = div()
                .id("cloud-sync-history-list")
                .min_h_0()
                .max_h(px(350.))
                .overflow_y_scroll()
                .track_scroll(&history.scroll)
                .scrollbars_with(&history.scroll)
                .flex()
                .flex_col()
                .border_1()
                .border_color(p.border)
                .rounded(px(8.));
            if snapshot.versions.is_empty() {
                list = list.child(
                    div()
                        .py(px(24.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .text_color(p.muted)
                        .child(Icon::new(IconName::Clock))
                        .child(crate::localization::text(
                            cx,
                            "rs.ui.copy.noEncryptedVersions",
                        )),
                );
            }
            for (index, version) in snapshot.versions.iter().enumerate() {
                let version = version.clone();
                let descriptor = version.clone();
                let snapshot = snapshot.clone();
                let mut details = model::date(&version);
                if let Some(bytes) = version.bytes {
                    details.push_str(&format!(" · {:.1} KB", bytes as f64 / 1024.));
                }
                if !version.author.is_empty() {
                    details.push_str(&format!(" · {}", version.author));
                }
                list = list.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(12.))
                        .py(px(7.))
                        .when(index > 0, |row| row.border_t_1().border_color(p.border))
                        .when(version.current, |row| row.bg(p.sidebar))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().font_weight(FontWeights::MEDIUM).child(
                                    crate::localization::view(if version.current {
                                        "Current"
                                    } else {
                                        "Version"
                                    }),
                                ))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(p.muted)
                                        .truncate()
                                        .child(details),
                                )
                                .when(!version.message.is_empty(), |view| {
                                    view.child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(p.muted)
                                            .truncate()
                                            .child(version.message.clone()),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(p.muted)
                                .child(model::short_id(&version)),
                        )
                        .when(!version.current, |row| {
                            row.child(
                                Button::new(("restore-cloud-version", index))
                                    .outline()
                                    .label("Restore")
                                    .disabled(self.busy() || !snapshot.can_restore)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.busy() {
                                            return;
                                        }
                                        match RestorePlan::new(
                                            snapshot.clone(),
                                            descriptor.clone(),
                                            this.preferences.backup_count,
                                        ) {
                                            Ok(plan) => {
                                                if let Some(history) = &mut this.history {
                                                    history.confirm = Some(plan);
                                                }
                                                this.error = None;
                                                this.validation_error = None;
                                            }
                                            Err(error) => this.error = Some(error.to_string()),
                                        }
                                        cx.notify();
                                    })),
                            )
                        }),
                );
            }
            body = body.child(list);
        }
        if history.confirm.is_none()
            && let Some(notice) = &self.notice
        {
            body = body.child(
                div()
                    .text_size(px(12.))
                    .text_color(p.accent)
                    .child(crate::localization::message(cx, notice)),
            );
        }
        Some(
            Modal::new("cloud-sync-history", title.clone())
                .focus_scope(&history.focus)
                .return_focus(&self.focus)
                .dismissible(!self.busy())
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.history = None;
                    this.error = None;
                    this.validation_error = None;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("cloud-sync-history-frame", title)
                        .colors(self.colors())
                        .width(620.)
                        .content(body)
                        .action(
                            Button::new("refresh-cloud-history")
                                .outline()
                                .label("Refresh")
                                .disabled(self.busy())
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.read_history(window, cx)
                                    }),
                                ),
                        )
                        .action(
                            Button::new("close-cloud-history")
                                .label("Done")
                                .disabled(self.busy())
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        ),
                )
                .into_any_element(),
        )
    }
    pub(super) fn version_restore_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let history = self.history.as_ref()?;
        let plan = history.confirm.as_ref()?;
        let body = div().flex().flex_col().gap(px(12.))
            .child(crate::localization::message(cx, &format!("Restore version {} from {}? This makes it current in the cloud and replaces local synced data.", model::short_id(&plan.version), history.target.name)))
            .child(crate::localization::message(cx, if plan.backup_count == 0 { "Local backups are disabled in settings." } else { "A local backup will be created before restoring." }));
        Some(
            Dialog::new("restore-cloud-version-confirm", "Restore Cloud Version")
                .focus_scope(&history.confirm_focus)
                .return_focus(&history.focus)
                .dismissible(!self.busy())
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    if let Some(history) = &mut this.history {
                        history.confirm = None;
                    }
                    this.error = None;
                    this.validation_error = None;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("restore-cloud-version-frame", "Restore Cloud Version")
                        .colors(self.colors())
                        .content(body)
                        .action(
                            Button::new("cancel-cloud-version-restore")
                                .outline()
                                .label("Cancel")
                                .disabled(self.busy())
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        )
                        .action(
                            Button::new("confirm-cloud-version-restore")
                                .primary()
                                .label(if self.mutating {
                                    "Restoring…"
                                } else {
                                    "Restore"
                                })
                                .disabled(self.busy())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if this.busy() {
                                        return;
                                    }
                                    if let Some(plan) = this
                                        .history
                                        .as_ref()
                                        .and_then(|history| history.confirm.clone())
                                    {
                                        this.mutating = true;
                                        this.error = None;
                                        this.validation_error = None;
                                        this.notice = None;
                                        cx.emit(Event::MutationRequested(
                                            Operation::RestoreVersion(plan),
                                        ));
                                        cx.notify();
                                    }
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}
