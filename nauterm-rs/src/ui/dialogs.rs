use super::*;
use crate::components::{DialogColors, DialogFrame, Modal, Spinner};
use crate::localization::{format_for, language, text};

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
            let title = self.close_title(target, cx);
            return Some(
                crate::components::Dialog::new("close-confirmation-modal", title.clone())
                    .return_focus(&self.focus)
                    .on_dismiss(cx.listener(|this, _, _, cx| {
                        this.cancel_close(cx);
                    }))
                    .content(
                        DialogFrame::new("close-confirmation-dialog", title)
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
                                    .label(match target {
                                        CloseTarget::Restart => "Restart",
                                        CloseTarget::Quit => "common.action.quit",
                                        _ => "common.action.close",
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
        let kind = self.selection_kind?;
        let (single, multiple, description) = match kind {
            ItemKind::Host => (
                "workspace.dialog.deleteHost",
                "workspace.dialog.deleteHosts",
                "workspace.dialog.deleteHosts.description",
            ),
            ItemKind::Group => (
                "workspace.dialog.deleteGroup",
                "workspace.dialog.deleteGroups",
                "workspace.dialog.deleteGroups.description",
            ),
            ItemKind::Snippet => (
                "workspace.dialog.deleteSnippet",
                "workspace.dialog.deleteSnippets",
                "workspace.dialog.deleteSnippets.description",
            ),
            ItemKind::SnippetPackage => (
                "workspace.dialog.deleteSnippetPackage",
                "workspace.dialog.deleteSnippetPackages",
                "workspace.dialog.deleteSnippetPackages.description",
            ),
            ItemKind::Forward => (
                "workspace.dialog.deleteForward",
                "workspace.dialog.deleteForwards",
                "workspace.dialog.deleteForwards.description",
            ),
            ItemKind::Proxy => (
                "workspace.dialog.deleteProxy",
                "workspace.dialog.deleteProxies",
                "workspace.dialog.deleteProxies.description",
            ),
        };
        let count = self.selection.len();
        let title = text(cx, if count == 1 { single } else { multiple });
        let message = if count == 1 {
            text(
                cx,
                match kind {
                    ItemKind::Group => "workspace.dialog.deleteGroup.description",
                    ItemKind::SnippetPackage => "workspace.dialog.deleteSnippetPackage.description",
                    ItemKind::Forward => "workspace.dialog.deleteForward.description",
                    _ => "workspace.dialog.deleteItem.description",
                },
            )
        } else {
            format_for(language(cx), description, &[("count", &count.to_string())])
        };
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
                        .content(message)
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
                                .label(if self.forward_delete_busy {
                                    "Deleting…"
                                } else {
                                    "Delete"
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Some(kind) = this.selection_kind {
                                        if kind == ItemKind::Forward {
                                            match this.delete_forward_selection(window, cx) {
                                                Ok(true) => {}
                                                Ok(false) => return,
                                                Err(error) => {
                                                    this.notice = Some(error.to_string());
                                                    cx.notify();
                                                    return;
                                                }
                                            }
                                        } else if matches!(
                                            kind,
                                            ItemKind::Snippet | ItemKind::SnippetPackage
                                        ) {
                                            this.delete_snippets(
                                                kind == ItemKind::SnippetPackage,
                                                window,
                                                cx,
                                            );
                                            return;
                                        } else {
                                            if let Err(error) = this.delete_resources(kind, cx) {
                                                this.notice =
                                                    Some(format!("Could not delete: {error}"));
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
