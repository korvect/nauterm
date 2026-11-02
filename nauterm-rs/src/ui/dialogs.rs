use super::*;
use crate::components::{DialogColors, DialogFrame, Modal, Spinner};

impl Nauterm {
    pub(super) fn confirmation_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let colors = DialogColors::workspace(self.data.dark);
        if let Some(action) = self.exiting {
            let title = if action == crate::settings::ExitAction::Restart {
                "Restarting Nauterm RS"
            } else {
                "Closing Nauterm RS"
            };
            return Some(
                crate::components::Dialog::new("application-exit-progress", title)
                    .dismissible(false)
                    .return_focus(&self.focus)
                    .content(
                        DialogFrame::new("application-exit-progress-frame", title)
                            .colors(colors)
                            .content(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(Spinner::new())
                                    .child(crate::localization::text(
                                        cx,
                                        "rs.ui.copy.waitingForBackgroundWork",
                                    )),
                            ),
                    )
                    .into_any_element(),
            );
        }
        if let Some(target) = self.close_pending {
            return Some(
                crate::components::Dialog::new(
                    "close-confirmation-modal",
                    self.close_title(target),
                )
                .return_focus(&self.focus)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.cancel_close(cx);
                }))
                .content(
                    DialogFrame::new("close-confirmation-dialog", self.close_title(target))
                        .width(380.)
                        .colors(colors)
                        .content(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(10.))
                                .child(self.close_message(target, cx)),
                        )
                        .action(
                            Button::new("keep-terminal")
                                .ghost()
                                .label("Cancel")
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        )
                        .action(
                            Button::new("close-terminal-confirm")
                                .danger()
                                .label(if matches!(target, CloseTarget::Restart) {
                                    "Restart"
                                } else {
                                    "Close"
                                })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.confirm_close(target, window, cx)
                                })),
                        ),
                )
                .into_any_element(),
            );
        }
        if !self.delete_pending {
            return None;
        }
        let title = format!("Delete {} selected item(s)?", self.selection.len());
        let snippet_busy = self.snippet_state.busy
            && matches!(
                self.selection_kind,
                Some(ItemKind::Snippet | ItemKind::SnippetPackage)
            );
        let deleting = snippet_busy || self.forward_delete_busy;
        Some(
            crate::components::Dialog::new("delete-confirmation-modal", title.clone())
                .dismissible(!deleting)
                .return_focus(&self.focus)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.delete_pending = false;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("delete-confirmation-dialog", title)
                        .width(380.)
                        .colors(colors)
                        .content(div().flex().flex_col().gap(px(8.))
                            .child(crate::localization::view(if self.selection_kind==Some(ItemKind::SnippetPackage){"Deleting packages keeps their snippets and removes only the package assignment."}else if self.selection_kind==Some(ItemKind::Forward){"This stops the selected forwarding rules and removes their saved configuration."}else{"This removes saved configuration. Active connections are handled separately."})))
                        .action(
                            Button::new("cancel-delete")
                                .disabled(deleting)
                                .ghost()
                                .label("Cancel")
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        )
                        .action(
                            Button::new("confirm-delete")
                                .disabled(deleting)
                                .loading(self.forward_delete_busy)
                                .danger()
                                .label(if self.forward_delete_busy { "Deleting…" } else { "Delete" })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Some(kind) = this.selection_kind {
                                        if kind == ItemKind::Forward {
                                            match this.delete_forward_selection(window, cx) {
                                                Ok(true) => {},
                                                Ok(false) => return,
                                                Err(error) => { this.notice = Some(error.to_string()); cx.notify(); return; }
                                            }
                                        } else if matches!(kind,ItemKind::Snippet|ItemKind::SnippetPackage) {
                                            this.delete_snippets(kind==ItemKind::SnippetPackage,window,cx);
                                            return;
                                        } else {
                                            if let Err(error) = this.delete_resources(kind, cx) {
                                                this.notice = Some(format!("Could not delete: {error}"));
                                                cx.notify();
                                                return;
                                            }
                                        }
                                    }
                                    this.selection.clear();
                                    this.retire_editor();
                                    this.delete_pending = false;
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}
