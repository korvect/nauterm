//! Authorization belongs to one editor generation. Cancellation/discard closes
//! its flow; late tokens cannot be applied to a reloaded or different provider.
use super::providers::GistStep;
use super::*;
use crate::{cancel::Cancel, sync::oauth};

enum Stage {
    Starting,
    Device {
        code: String,
        url: String,
        copied: bool,
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
    pub(super) fn start_authorization(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                if editor.gist_step == Some(GistStep::Connect) {
                    self.validation_error = Some(error.to_string());
                } else {
                    self.error = Some(error.to_string());
                }
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
                    .update_in(cx, |this, window, cx| {
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
                            oauth::Event::Device { code, url } => {
                                authorization.stage = Stage::Device {
                                    code,
                                    url,
                                    copied: false,
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
                                if editor.gist_step == Some(GistStep::Connect) {
                                    this.save_gist_authorization(window, cx);
                                } else {
                                    cx.emit(Event::Settled(true));
                                }
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

    fn save_gist_authorization(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some((editor, service)) = self.editor.as_ref().zip(self.service.clone()) else {
            return;
        };
        let Some(credentials) = editor.authorized.clone() else {
            return;
        };
        // Persist the sign-in independently of any unsaved Gist ID edits.
        let update =
            match editor
                .draft
                .update_authorized(&editor.draft.name, Vec::new(), Some(credentials))
            {
                Ok(update) => update,
                Err(error) => {
                    self.validation_error = Some(error.to_string());
                    cx.notify();
                    return;
                }
            };
        let expected = editor.draft.expected.clone();
        self.loading = true;
        self.validation_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.save_provider(expected, update).await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.loading = false;
                let success = result.is_ok();
                match result {
                    Ok((saved, overview)) => {
                        this.overview = Some(overview);
                        if let Some(editor) = &mut this.editor {
                            editor.draft = saved;
                            editor.authorized = None;
                            editor.authorizing = None;
                            editor.gist_step = Some(GistStep::Authorized);
                        }
                    }
                    Err(error) => this.validation_error = Some(error.to_string()),
                }
                cx.emit(Event::Settled(success));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn gist_authorization_frame(
        &self,
        editor: &super::providers::Editor,
        cx: &mut Context<Self>,
    ) -> DialogFrame {
        let running = self.authorization_running();
        let mut body = div().w_full().flex().flex_col();
        if self.loading {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Spinner::new())
                    .child(crate::localization::view("common.label.saving")),
            );
        } else if self.validation_error.is_none() {
            body = body.children(
                editor
                    .authorizing
                    .as_ref()
                    .map(|authorization| self.authorization_progress(authorization, cx)),
            );
        }
        body = body.children(crate::components::validation_message(
            self.validation_error.as_deref(),
            cx,
        ));
        let mut frame = self.gist_flow_frame(cx).content(body);
        if !running && !self.storage_busy() {
            let save = editor.authorized.is_some();
            frame = frame.action(
                Button::new("retry-gist-authorization")
                    .primary()
                    .label(if save {
                        "common.action.retrySave"
                    } else {
                        "settings.label.tryAgain"
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if save {
                            this.save_gist_authorization(window, cx);
                        } else {
                            this.start_authorization(window, cx);
                        }
                    })),
            );
        }
        frame
    }

    pub(super) fn gist_flow_frame(&self, cx: &App) -> DialogFrame {
        let p = Palette::settings(self.dark);
        DialogFrame::with_header(
            "github-gist-flow-frame",
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .child(crate::localization::text(
                    cx,
                    "settings.label.connectToGithub",
                ))
                .child(
                    Button::new("close-github-gist-flow")
                        .ghost()
                        .size(px(24.))
                        .p_0()
                        .icon_size(px(14.))
                        .icon(IconName::X)
                        .text_color(p.muted)
                        .tooltip("common.action.close")
                        .disabled(self.storage_busy())
                        .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                ),
        )
        .colors(self.colors())
        .width(460.)
        .hide_empty_footer()
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
            "settings.sync.authorization.browser.description"
        } else if pending {
            "settings.sync.authorization.pending.description"
        } else if saved {
            "settings.sync.authorization.saved.description"
        } else {
            "settings.sync.authorization.required.description"
        };
        let control = if running {
            Button::new("cancel-sync-provider-sign-in")
                .outline()
                .label("settings.sync.authorization.action.cancel")
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
                    "common.label.reconnect"
                } else {
                    "common.action.connect"
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
            .child(self.row("settings.label.authorization", caption, control));
        if let Some(account) = editor.authorized_account.as_ref().filter(|_| pending) {
            body = body.child(div().text_size(px(12.)).text_color(p.muted).child(
                crate::localization::format(
                    cx,
                    "settings.sync.github.authorized",
                    &[("account", account)],
                ),
            ));
        }
        if let Some(authorization) = &editor.authorizing {
            body = body.child(self.authorization_progress(authorization, cx));
        }
        body.into_any_element()
    }

    fn authorization_progress(&self, authorization: &Authorization, cx: &mut Context<Self>) -> Div {
        let p = Palette::settings(self.dark);
        match &authorization.stage {
            Stage::Starting => div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(Spinner::new())
                .child(crate::localization::text(
                    cx,
                    "settings.sync.authorization.status.starting",
                )),
            Stage::Exchanging => div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(Spinner::new())
                .child(crate::localization::text(
                    cx,
                    "settings.sync.authorization.status.exchanging",
                )),
            Stage::Device { code, url, copied } => {
                let flow_id = authorization.id.clone();
                let browser = url.clone();
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_center()
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .mb(px(4.))
                            .child(crate::localization::view(
                                "settings.sync.github.deviceCode.description",
                            )),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.))
                            .px(px(16.))
                            .py(px(16.))
                            .rounded(px(10.))
                            .bg(p.control)
                            .border_1()
                            .border_color(p.border)
                            .child(
                                div()
                                    .w_full()
                                    .text_center()
                                    .text_ellipsis()
                                    .font_weight(FontWeights::SEMIBOLD)
                                    .text_size(px(23.))
                                    .child(code.clone()),
                            )
                            .child(
                                Button::new("copy-github-device-code")
                                    .ghost()
                                    .small()
                                    .icon_size(px(14.))
                                    .text_size(px(11.))
                                    .label(if *copied {
                                        "common.status.copied"
                                    } else {
                                        "common.action.copyCode"
                                    })
                                    .icon(if *copied {
                                        IconName::Check
                                    } else {
                                        IconName::Copy
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(flow) = this
                                            .editor
                                            .as_mut()
                                            .and_then(|editor| editor.authorizing.as_mut())
                                            .filter(|flow| flow.id == flow_id)
                                            && let Stage::Device { code, copied, .. } =
                                                &mut flow.stage
                                        {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                code.clone(),
                                            ));
                                            *copied = true;
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
                    .child(
                        Button::new("open-github-device-sign-in")
                            .primary()
                            .w_full()
                            .h(px(36.))
                            .icon(IconName::ExternalLink)
                            .icon_size(px(14.))
                            .label("settings.label.openGithub")
                            .on_click(move |_, _, cx| cx.open_url(&browser)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(8.))
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .child(Spinner::new().with_size(px(13.)))
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
                        crate::localization::text(
                            cx,
                            "settings.sync.authorization.status.waitingForBrowser",
                        ),
                    ))
                    .child(
                        Button::new("reopen-storage-sign-in")
                            .outline()
                            .label("settings.sync.authorization.action.openBrowser")
                            .on_click(move |_, _, cx| cx.open_url(&browser)),
                    )
            }
            Stage::Failed(error) => div()
                .text_size(px(12.))
                .text_color(rgb(0xef4444))
                .child(crate::localization::message(cx, error)),
            Stage::Ready => div(),
        }
    }
}
