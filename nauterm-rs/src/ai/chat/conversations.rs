use super::*;
use crate::components::{Dialog, DialogColors, DialogFrame, Modal};

impl Chat {
    fn prepare_delete(&mut self, uuid: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.history_loading || self.delete_pending.is_some() {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.history_loading = true;
        let scope = self.scope;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.prepare_conversation_delete(uuid, scope).await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.history_loading = false;
                match result {
                    Ok(target) => {
                        this.delete_pending = Some(target);
                        this.delete_error = None;
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn confirm_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.deleting || self.delete_error.is_some() {
            return;
        }
        let Some(target) = self.delete_pending.clone() else {
            return;
        };
        let Some(service) = self.service.clone() else {
            return;
        };
        self.deleting = true;
        let uuid = target.uuid.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.delete_conversation(target).await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(()) => {
                        let mut rows = this.history.take();
                        if let Some(rows) = &mut rows {
                            rows.remove(&uuid);
                        }
                        if this.session.as_ref().is_some_and(|session| {
                            session.view().entry.uuid.as_deref() == Some(uuid.as_str())
                        }) && let Some(service) = this.service.clone()
                        {
                            this.install(
                                Session::new(service, this.scope, this.host_uuid.clone()),
                                window,
                                cx,
                            );
                        }
                        this.history = rows;
                        this.error = None;
                        // Completion addresses this dialog, even if another
                        // modal became topmost while the database was busy.
                        if !Modal::complete(&this.delete_focus, window, cx) {
                            this.delete_pending = None;
                            this.deleting = false;
                        }
                    }
                    Err(error) => {
                        this.deleting = false;
                        this.delete_error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn delete_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let target = self.delete_pending.as_ref()?;
        let palette = self.palette();
        let colors = DialogColors {
            surface: palette.surface,
            header: palette.sidebar,
            border: palette.border,
            foreground: palette.text,
            dark: palette.background.r * 0.2126
                + palette.background.g * 0.7152
                + palette.background.b * 0.0722
                < 0.5,
        };
        let focus = self.history_search.read(cx).focus_handle(cx);
        Some(
            Dialog::new("ai-delete-conversation-modal", "Delete Conversation?")
                .focus_scope(&self.delete_focus)
                .return_focus(&focus)
                .dismissible(!self.deleting)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.delete_pending = None;
                    this.delete_error = None;
                    this.deleting = false;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("ai-delete-conversation-dialog", "Delete Conversation?")
                        .width(400.)
                        .colors(colors)
                        .content(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(10.))
                                .text_size(px(13.))
                                .child(crate::localization::view(format!(
                                    "Delete \"{}\" from conversation history?",
                                    target.title
                                )))
                                .when_some(self.delete_error.clone(), |body, error| {
                                    body.child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(palette.muted)
                                            .child(crate::localization::message(cx, &error)),
                                    )
                                }),
                        )
                        .action(
                            Button::new("ai-cancel-delete")
                                .ghost()
                                .custom(ButtonCustomVariant::for_surface(palette))
                                .label("Cancel")
                                .disabled(self.deleting)
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        )
                        .action(
                            Button::new("ai-confirm-delete")
                                .danger()
                                .label("Delete")
                                .disabled(self.deleting || self.delete_error.is_some())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_delete(window, cx)
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
    fn history_row(&self, index: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let row = self.history.as_ref()?.row(index)?;
        let palette = self.palette();
        let uuid = row.uuid.clone();
        let delete_uuid = uuid.clone();
        let date = row
            .updated_at
            .and_then(chrono::DateTime::<chrono::Utc>::from_timestamp_millis)
            .map(|date| {
                date.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            });
        Some(
            div()
                .w_full()
                .min_w_0()
                .h(px(56.))
                .px(px(8.))
                .pb(px(6.))
                .child(
                    div()
                        .id(SharedString::from(format!("ai-history-row-{uuid}")))
                        .relative()
                        .h(px(50.))
                        .rounded(px(8.))
                        .bg(palette.control)
                        .hover(|row| row.bg(palette.hover))
                        .child(
                            Button::new(SharedString::from(format!("ai-conversation-{uuid}")))
                                .ghost()
                                .custom(
                                    ButtonCustomVariant::for_surface(palette).hover(rgba(0).into()),
                                )
                                .h(px(50.))
                                .w_full()
                                .pr(px(36.))
                                .min_w_0()
                                .content_start()
                                .disabled(self.history_loading || self.deleting)
                                .child(
                                    div()
                                        .min_w_0()
                                        .w_full()
                                        .flex()
                                        .flex_col()
                                        .gap(px(3.))
                                        .text_size(px(12.))
                                        .line_height(relative(1.5))
                                        .child(
                                            div()
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .child(row.title.clone()),
                                        )
                                        .when_some(date, |body, date| {
                                            body.child(
                                                div()
                                                    .text_size(px(10.))
                                                    .text_color(palette.muted)
                                                    .child(date),
                                            )
                                        }),
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_history(uuid.clone(), window, cx)
                                })),
                        )
                        .child(
                            Button::new(SharedString::from(format!(
                                "ai-delete-history-{delete_uuid}"
                            )))
                            .absolute()
                            .right(px(4.))
                            .top(px(13.))
                            .xsmall()
                            .ghost()
                            .custom(ButtonCustomVariant::for_surface(palette))
                            .icon(IconName::Trash)
                            .tooltip("Delete conversation")
                            .disabled(self.history_loading || self.busy() || self.deleting)
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.prepare_delete(delete_uuid.clone(), window, cx)
                                },
                            )),
                        ),
                )
                .into_any_element(),
        )
    }
    pub(super) fn history_body(
        &self,
        rows: &history_list::Catalog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette();
        let query_empty = self.history_search.read(cx).value().is_empty();
        let body = if rows.visible_len() == 0 {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .text_center()
                .text_color(palette.muted)
                .child(crate::assets::icon(IconName::RotateCcwClock).size(px(22.)))
                .child(div().text_size(px(13.)).text_color(palette.text).child(
                    crate::localization::view(if query_empty {
                        "No conversation history"
                    } else {
                        "No matching conversations"
                    }),
                ))
                .into_any_element()
        } else {
            crate::components::uniform_scrollbar(
                "ai-history-scrollbar",
                uniform_list(
                    "ai-history-list",
                    rows.visible_len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .filter_map(|index| this.history_row(index, cx))
                            .collect()
                    }),
                )
                .track_scroll(&self.history_scroll)
                .size_full()
                .pb(px(8.)),
                &self.history_scroll,
            )
            .hover_only()
            .into_any_element()
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(10.))
            .pt(px(10.))
            .when(
                !rows.is_empty() || !query_empty || matches!(self.source, Source::Terminal { .. }),
                |body| {
                    body.child(
                        div()
                            .px(px(10.))
                            .pb(px(10.))
                            .border_b_1()
                            .border_color(palette.border)
                            .child(
                                Input::new(&self.history_search)
                                    .palette(palette)
                                    .prefix(
                                        crate::assets::icon(IconName::Search)
                                            .size(px(14.))
                                            .text_color(palette.muted),
                                    )
                                    .aria_label(crate::localization::text(
                                        cx,
                                        "Search conversations",
                                    )),
                            ),
                    )
                },
            )
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}
