//! Authorization belongs to one editor generation. Cancellation/discard closes
//! its flow; late tokens cannot be applied to a reloaded or different provider.
use super::*;
use crate::{cancel::Cancel, sync::oauth};

enum Stage {
    Starting,
    Device {
        code: String,
        url: String,
        expires_in: u64,
    },
    Browser(String),
    Exchanging,
    Ready,
    Failed(String),
}
pub(super) struct Authorization {
    id: String,
    pub(super) running: bool,
    stage: Stage,
    cancel: Cancel,
    _task: Task<()>,
}
impl Drop for Authorization {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
impl Panel {
    pub(super) fn authorization_running(&self) -> bool {
        self.editor
            .as_ref()
            .and_then(|editor| editor.authorizing.as_ref())
            .is_some_and(|authorization| authorization.running)
    }
    pub fn cancel_authorization(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.authorization_running() {
            return false;
        }
        if let Some(editor) = &mut self.editor {
            editor.authorizing = None;
        }
        cx.notify();
        true
    }
    fn start_authorization(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(vendor) = editor.draft.kind.oauth() else {
            return;
        };
        let generation = editor.generation.clone();
        let flow = match oauth::start(vendor, crate::localization::language(cx)) {
            Ok(flow) => flow,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let cancel = flow.cancel.clone();
        let id = uuid::Uuid::new_v4().to_string();
        let flow_id = id.clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = flow.events.recv().await {
                let terminal = matches!(
                    &event,
                    oauth::Event::Authorized(_) | oauth::Event::Failed(_)
                );
                let live = this
                    .update_in(cx, |this, _, cx| {
                        let Some(editor) = this
                            .editor
                            .as_mut()
                            .filter(|editor| editor.generation == generation)
                        else {
                            return false;
                        };
                        let Some(authorization) =
                            editor.authorizing.as_mut().filter(|authorization| {
                                authorization.id == flow_id && authorization.running
                            })
                        else {
                            return false;
                        };
                        match event {
                            oauth::Event::Device {
                                code,
                                url,
                                expires_in,
                            } => {
                                authorization.stage = Stage::Device {
                                    code,
                                    url,
                                    expires_in,
                                }
                            }
                            oauth::Event::Browser(url) => {
                                cx.open_url(&url);
                                authorization.stage = Stage::Browser(url);
                            }
                            oauth::Event::Exchanging => authorization.stage = Stage::Exchanging,
                            oauth::Event::Authorized(authorized) => {
                                editor.authorized = Some(authorized.credentials);
                                editor.authorized_account = authorized.account;
                                authorization.running = false;
                                authorization.stage = Stage::Ready;
                                cx.emit(Event::Settled(true));
                            }
                            oauth::Event::Failed(error) => {
                                authorization.running = false;
                                authorization.stage = Stage::Failed(error);
                                cx.emit(Event::Settled(false));
                            }
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if terminal || !live {
                    break;
                }
            }
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(authorization) = this
                    .editor
                    .as_mut()
                    .filter(|editor| editor.generation == generation)
                    .and_then(|editor| editor.authorizing.as_mut())
                    .filter(|authorization| authorization.id == flow_id && authorization.running)
                {
                    authorization.running = false;
                    authorization.stage =
                        Stage::Failed("Authorization stopped. Try signing in again.".into());
                    cx.emit(Event::Settled(false));
                    cx.notify();
                }
            });
        });
        if let Some(editor) = &mut self.editor {
            editor.authorizing = Some(Authorization {
                id,
                running: true,
                stage: Stage::Starting,
                cancel,
                _task: task,
            });
        }
        self.error = None;
        self.validation_error = None;
        cx.notify();
    }
    pub(super) fn authorization_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.editor else {
            return div().into_any_element();
        };
        let p = Palette::settings(self.dark);
        let pending = editor.authorized.is_some();
        let saved = editor.draft.has_credentials();
        let running = self.authorization_running();
        let caption = if running {
            "Complete sign-in in your browser."
        } else if pending {
            "Authorization is ready. Save settings to keep it."
        } else if saved {
            "Authorization is saved on this device."
        } else {
            "Sign in to connect this storage provider."
        };
        let control = if running {
            Button::new("cancel-sync-provider-sign-in")
                .outline()
                .label("Cancel sign-in")
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.cancel_authorization(cx) {
                        cx.emit(Event::Settled(true));
                    }
                }))
                .into_any_element()
        } else {
            Button::new("sync-provider-sign-in")
                .outline()
                .label(if pending || saved {
                    "Authorize again"
                } else {
                    "Sign in"
                })
                .disabled(self.storage_busy())
                .on_click(cx.listener(|this, _, window, cx| this.start_authorization(window, cx)))
                .into_any_element()
        };
        let mut body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.row("Authorization", caption, control));
        if let Some(account) = editor.authorized_account.as_ref().filter(|_| pending) {
            body = body.child(div().text_size(px(12.)).text_color(p.muted).child(
                crate::localization::message(cx, &format!("Authorized as @{account}.")),
            ));
        }
        if let Some(authorization) = &editor.authorizing {
            let progress = match &authorization.stage {
                Stage::Starting => div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Spinner::new())
                    .child(crate::localization::text(cx, "rs.ui.copy.startingSignIn")),
                Stage::Exchanging => div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Spinner::new())
                    .child(crate::localization::text(
                        cx,
                        "rs.ui.copy.completingAuthorization",
                    )),
                Stage::Device {
                    code,
                    url,
                    expires_in,
                } => {
                    let copy = code.clone();
                    let browser = url.clone();
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .child(
                            div()
                                .font_weight(FontWeights::MEDIUM)
                                .text_size(px(20.))
                                .child(code.clone()),
                        )
                        .child(div().text_size(px(12.)).text_color(p.muted).child(
                            crate::localization::message(
                                cx,
                                &format!(
                                    "Enter this code on GitHub. It expires after {} minutes.",
                                    expires_in.div_ceil(60)
                                ),
                            ),
                        ))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    Button::new("copy-github-device-code")
                                        .outline()
                                        .label("Copy code")
                                        .icon(IconName::Copy)
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                copy.clone(),
                                            ))
                                        }),
                                )
                                .child(
                                    Button::new("open-github-device-sign-in")
                                        .primary()
                                        .label("Open GitHub")
                                        .on_click(move |_, _, cx| cx.open_url(&browser)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(Spinner::new())
                                .child(crate::localization::text(
                                    cx,
                                    "settings.label.waitingForAuthorization",
                                )),
                        )
                }
                Stage::Browser(url) => {
                    let browser = url.clone();
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(Spinner::new())
                        .child(div().flex_1().text_size(px(12.)).text_color(p.muted).child(
                            crate::localization::text(cx, "rs.ui.copy.waitingForBrowser"),
                        ))
                        .child(
                            Button::new("reopen-storage-sign-in")
                                .outline()
                                .label("Open browser")
                                .on_click(move |_, _, cx| cx.open_url(&browser)),
                        )
                }
                Stage::Failed(error) => div()
                    .text_size(px(12.))
                    .text_color(rgb(0xef4444))
                    .child(crate::localization::message(cx, error)),
                Stage::Ready => div(),
            };
            body = body.child(progress);
        }
        body.into_any_element()
    }
}
