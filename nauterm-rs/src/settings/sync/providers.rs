//! Provider forms keep credentials write-only and retain drafts after failures.
use super::*;
use crate::sync::provider::{Draft, Field, Kind};
use zeroize::Zeroizing;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum GistStep {
    Connect,
    Authorized,
    Settings,
}

pub(super) struct Editor {
    pub(super) draft: Draft,
    pub(super) generation: String,
    pub(super) authorized: Option<nauterm_core::SyncProviderCredentials>,
    pub(super) authorized_account: Option<String>,
    pub(super) authorizing: Option<super::authorization::Authorization>,
    pub(super) gist_step: Option<GistStep>,
    name: Entity<InputState>,
    fields: Vec<(Field, Entity<InputState>)>,
}
impl Editor {
    fn new(draft: Draft, window: &mut Window, cx: &mut Context<Panel>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(draft.name.clone()));
        let fields = draft
            .kind
            .fields()
            .into_iter()
            .map(|field| {
                let value = draft.value(field);
                let hint = if field.credential && draft.has_credentials() {
                    "Saved; leave blank to keep"
                } else if field.credential {
                    "Enter credential"
                } else if field.key == "gist_id" {
                    "settings.label.createAutomatically"
                } else {
                    field.default
                };
                let hint = crate::localization::text(cx, hint);
                (
                    field,
                    cx.new(|cx| {
                        InputState::new(window, cx)
                            .default_value(value)
                            .placeholder(hint)
                            .masked(field.masked)
                    }),
                )
            })
            .collect();
        let gist_step = (draft.kind == Kind::Gist).then(|| {
            if draft.has_credentials() {
                GistStep::Settings
            } else {
                GistStep::Connect
            }
        });
        Self {
            draft,
            generation: uuid::Uuid::new_v4().to_string(),
            authorized: None,
            authorized_account: None,
            authorizing: None,
            gist_step,
            name,
            fields,
        }
    }
    fn clear(mut self, window: &mut Window, cx: &mut Context<Panel>) {
        self.authorizing = None;
        self.authorized = None;
        for input in std::iter::once(&self.name).chain(self.fields.iter().map(|(_, input)| input)) {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
    }
}
impl Panel {
    fn install_provider_editor(
        &mut self,
        draft: Draft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(previous) = self.editor.take() {
            previous.clear(window, cx);
        }
        self.editor = Some(Editor::new(draft, window, cx));
        self.error = None;
        self.validation_error = None;
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.gist_step == Some(GistStep::Connect))
        {
            self.start_authorization(window, cx);
        }
        cx.notify();
    }
    pub(super) fn open_provider_editor(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        let kind = Kind::BUILT_IN
            .into_iter()
            .find(|kind| kind.fixed_id() == Some(id.as_str()));
        let id = id.strip_prefix("cloud:").unwrap_or(&id).to_owned();
        self.loading = true;
        self.error = None;
        self.validation_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.provider(id, kind).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.loading = false;
                let success = result.is_ok();
                match result {
                    Ok(draft) => this.install_provider_editor(draft, window, cx),
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.emit(Event::Settled(success));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn choose_provider(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        Modal::complete(&self.catalog_focus, window, cx);
        self.catalog = false;
        if let Some(id) = kind.fixed_id() {
            self.open_provider_editor(id.into(), window, cx);
        } else {
            self.install_provider_editor(Draft::new(kind), window, cx);
        }
    }
    fn close_provider_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.storage_busy() {
            return;
        }
        if let Some(editor) = self.editor.take() {
            editor.clear(window, cx);
        }
        self.remove_provider = None;
        self.error = None;
        self.validation_error = None;
        cx.notify();
    }
    fn save_provider_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(editor) = &self.editor else {
            return;
        };
        let update = editor.draft.update_authorized(
            &editor.name.read(cx).value(),
            editor
                .fields
                .iter()
                .map(|(field, input)| (*field, Zeroizing::new(input.read(cx).value().to_string())))
                .collect(),
            editor.authorized.clone(),
        );
        let update = match update {
            Ok(update) => update,
            Err(error) => {
                self.validation_error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let expected = editor.draft.expected.clone();
        self.validation_error = None;
        let Some(service) = self.service.clone() else {
            return;
        };
        self.loading = true;
        self.error = None;
        self.validation_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.save_provider(expected, update).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.loading = false;
                let success = result.is_ok();
                match result {
                    Ok((_, overview)) => {
                        this.overview = Some(overview);
                        Modal::complete(&this.provider_focus, window, cx);
                        this.close_provider_editor(window, cx);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.emit(Event::Settled(success));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn confirm_provider_removal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(disconnect) = self.remove_provider else {
            return;
        };
        let Some(expected) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.draft.expected.clone())
        else {
            return;
        };
        let Some(service) = self.service.clone() else {
            return;
        };
        self.loading = true;
        self.error = None;
        self.validation_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service.remove_provider(expected, disconnect).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.loading = false;
                let success = result.is_ok();
                match result {
                    Ok(overview) => {
                        this.overview = Some(overview);
                        Modal::complete(&this.remove_focus, window, cx);
                        Modal::complete(&this.provider_focus, window, cx);
                        this.close_provider_editor(window, cx);
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.emit(Event::Settled(success));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn provider_catalog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.catalog {
            return None;
        }
        let p = Palette::settings(self.dark);
        let mut body = div().flex().flex_col().gap(px(8.)).child(
            div()
                .text_size(px(12.))
                .text_color(p.muted)
                .child(crate::localization::text(
                    cx,
                    "settings.sync.provider.description",
                )),
        );
        let kinds: Vec<_> = Kind::CATALOG.into_iter().enumerate().collect();
        for pair in kinds.chunks(2) {
            let mut line = div().w_full().flex().gap(px(8.));
            for &(index, kind) in pair {
                line = line.child(
                    div().flex_1().min_w_0().child(
                        Button::new(("new-sync-provider-kind", index))
                            .outline()
                            .content_start()
                            .w_full()
                            .h(px(40.))
                            .icon(IconName::Cloud)
                            .label(kind.label())
                            .disabled(self.busy())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.choose_provider(kind, window, cx)
                            })),
                    ),
                );
            }
            body = body.child(line);
        }
        Some(
            Modal::new("add-sync-provider", "settings.label.addProvider")
                .focus_scope(&self.catalog_focus)
                .return_focus(&self.focus)
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.catalog = false;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("add-sync-provider-frame", "settings.label.addProvider")
                        .colors(self.colors())
                        .width(400.)
                        .content(body)
                        .action(
                            Button::new("cancel-add-provider")
                                .outline()
                                .label("common.action.cancel")
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        ),
                )
                .into_any_element(),
        )
    }
    fn provider_fields(&self, editor: &Editor, cx: &mut Context<Self>) -> Div {
        let p = Palette::settings(self.dark);
        let mut body = div().w_full().flex().flex_col().gap(px(14.));
        let location =
            |key: &str| matches!(key, "prefix" | "root" | "filename" | "path" | "branch");
        let wide = |key: &str| matches!(key, "endpoint" | "repository_url");
        let mut groups: [(&'static str, &'static str, Vec<AnyElement>); 3] = [
            ("CONNECTION", "", Vec::new()),
            (
                "CREDENTIALS",
                if editor.draft.has_credentials() {
                    "Leave blank to keep saved values"
                } else {
                    "Encrypted on this device"
                },
                Vec::new(),
            ),
            ("LOCATION", "", Vec::new()),
        ];
        let mut pending: [Vec<AnyElement>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        let flush = |row: &mut Vec<AnyElement>, out: &mut Vec<AnyElement>| {
            if row.is_empty() {
                return;
            }
            let mut line = div().w_full().flex().gap(px(10.));
            for cell in row.drain(..) {
                line = line.child(div().flex_1().min_w_0().child(cell));
            }
            out.push(line.into_any_element());
        };
        if editor.draft.kind.fixed_id().is_none() {
            groups[0].2.push(
                self.field(
                    "settings.label.displayName",
                    "",
                    Input::new(&editor.name)
                        .aria_label(crate::localization::text(cx, "settings.label.displayName")),
                ),
            );
        }
        for (field, input) in &editor.fields {
            let index = if field.credential {
                1
            } else if location(field.key) {
                2
            } else {
                0
            };
            let cell = self.field(
                field.label,
                "",
                Input::new(input)
                    .readonly(field.readonly)
                    .aria_label(crate::localization::text(cx, field.label)),
            );
            if wide(field.key) {
                flush(&mut pending[index], &mut groups[index].2);
                groups[index].2.push(cell);
            } else {
                pending[index].push(cell);
                if pending[index].len() == 2 {
                    flush(&mut pending[index], &mut groups[index].2);
                }
            }
        }
        for (index, (title, hint, rows)) in groups.into_iter().enumerate() {
            let mut rows = rows;
            flush(&mut pending[index], &mut rows);
            if rows.is_empty() {
                continue;
            }
            let mut group = div().w_full().flex().flex_col().gap(px(10.)).child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(11.))
                    .child(
                        div()
                            .font_weight(FontWeights::MEDIUM)
                            .text_color(p.muted)
                            .child(crate::localization::view(title)),
                    )
                    .when(!hint.is_empty(), |view| {
                        view.child(
                            div()
                                .text_color(p.muted)
                                .child(crate::localization::view(hint)),
                        )
                    }),
            );
            for row in rows {
                group = group.child(row);
            }
            body = body.child(group);
        }
        if editor.draft.kind.oauth().is_some() {
            body = body.child(self.authorization_control(cx));
        } else if editor.fields.iter().all(|(field, _)| !field.credential) {
            body = body.child(div().text_size(px(12.)).text_color(p.muted).child(
                crate::localization::view(if editor.draft.has_credentials() {
                    "settings.sync.authorization.saved.description"
                } else {
                    "settings.sync.authorization.validation.required"
                }),
            ));
        }
        body
    }

    fn gist_field(&self, field: Field, input: &Entity<InputState>, cx: &App) -> AnyElement {
        let is_gist_id = field.key == "gist_id";
        self.field(
            field.label,
            if is_gist_id {
                "settings.sync.gist.id.description"
            } else {
                ""
            },
            Input::new(input)
                .readonly(field.readonly)
                .cleanable(is_gist_id)
                .aria_label(crate::localization::text(cx, field.label)),
        )
    }

    fn gist_authorized_frame(&self, editor: &Editor, cx: &mut Context<Self>) -> DialogFrame {
        let p = Palette::settings(self.dark);
        let green = rgb(if self.dark { 0x34d399 } else { 0x10b981 });
        let status = div()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(48.))
                    .rounded_full()
                    .bg(green.opacity(0.10))
                    .child(
                        Icon::new(IconName::CircleCheck)
                            .size(px(23.))
                            .text_color(green),
                    ),
            )
            .child(
                div()
                    .mt(px(12.))
                    .text_size(px(14.))
                    .font_weight(FontWeights::SEMIBOLD)
                    .child(crate::localization::view("settings.label.githubConnected")),
            )
            .when_some(editor.authorized_account.as_ref(), |view, account| {
                view.child(
                    div()
                        .mt(px(5.))
                        .text_center()
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .child(crate::localization::format(
                            cx,
                            "settings.sync.github.authorized",
                            &[("account", account)],
                        )),
                )
            });
        let body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(status)
            .children(
                editor
                    .fields
                    .iter()
                    .filter(|(field, _)| field.key == "gist_id")
                    .map(|(field, input)| self.gist_field(*field, input, cx)),
            )
            .children(crate::components::validation_message(
                self.validation_error.as_deref(),
                cx,
            ))
            .child(
                Button::new("save-authorized-gist-settings")
                    .primary()
                    .w_full()
                    .h(px(32.))
                    .label(if self.loading {
                        "common.label.saving"
                    } else {
                        "settings.label.saveGistSettings"
                    })
                    .disabled(self.busy())
                    .on_click(
                        cx.listener(|this, _, window, cx| this.save_provider_editor(window, cx)),
                    ),
            );
        self.gist_flow_frame(cx).content(body)
    }

    pub(super) fn provider_editor(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let editor = self.editor.as_ref()?;
        let flow_frame = match editor.gist_step {
            Some(GistStep::Connect) => Some(self.gist_authorization_frame(editor, cx)),
            Some(GistStep::Authorized) => Some(self.gist_authorized_frame(editor, cx)),
            _ => None,
        };
        if let Some(frame) = flow_frame {
            return Some(self.provider_modal("settings.label.connectToGithub", frame, cx));
        }
        let is_gist = editor.draft.kind == Kind::Gist;
        let body = if is_gist {
            div().w_full().flex().flex_col().gap(px(14.)).children(
                editor
                    .fields
                    .iter()
                    .map(|(field, input)| self.gist_field(*field, input, cx)),
            )
        } else {
            self.provider_fields(editor, cx)
        }
        .children(crate::components::validation_message(
            self.validation_error.as_deref(),
            cx,
        ));
        let title = editor.draft.kind.label();
        let mut frame = DialogFrame::new("sync-provider-editor-frame", title)
            .colors(self.colors())
            .width(560.)
            .content(
                div()
                    .id("sync-provider-editor-body")
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .child(body),
            );
        if editor.draft.expected.is_some() {
            let disconnect = editor.draft.kind.fixed_id().is_some();
            frame = frame.action(
                Button::new("remove-sync-provider")
                    .outline()
                    .label(if disconnect {
                        "common.action.disconnect"
                    } else {
                        "settings.ai.provider.delete"
                    })
                    .disabled(self.busy())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_provider = Some(disconnect);
                        this.error = None;
                        this.validation_error = None;
                        cx.notify();
                    })),
            );
            frame = if is_gist {
                frame.action(
                    Button::new("reconnect-gist")
                        .outline()
                        .label("common.label.reconnect")
                        .disabled(self.busy())
                        .on_click(cx.listener(|this, _, window, cx| {
                            if !this.busy()
                                && let Some(editor) = &mut this.editor
                            {
                                editor.gist_step = Some(GistStep::Connect);
                                this.start_authorization(window, cx);
                            }
                        })),
                )
            } else {
                let id = editor.draft.id.clone();
                frame.action(
                    Button::new("reload-sync-provider")
                        .outline()
                        .label("common.action.refresh")
                        .disabled(self.busy())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_provider_editor(id.clone(), window, cx)
                        })),
                )
            };
        }
        frame = frame
            .action(
                Button::new("cancel-sync-provider-editor")
                    .outline()
                    .label("common.action.cancel")
                    .disabled(self.storage_busy())
                    .on_click(|_, window, cx| Modal::dismiss(window, cx)),
            )
            .action(
                Button::new("save-sync-provider")
                    .primary()
                    .label(if self.loading {
                        "common.label.saving"
                    } else if is_gist {
                        "settings.label.saveGistSettings"
                    } else {
                        "settings.sync.action.saveSettings"
                    })
                    .disabled(self.busy())
                    .on_click(
                        cx.listener(|this, _, window, cx| this.save_provider_editor(window, cx)),
                    ),
            );
        Some(self.provider_modal(title, frame, cx))
    }

    fn provider_modal(
        &self,
        title: &'static str,
        frame: DialogFrame,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        Modal::new("sync-provider-editor", title)
            .focus_scope(&self.provider_focus)
            .return_focus(&self.focus)
            .dismissible(!self.storage_busy())
            .dismiss_on_backdrop(false)
            .on_dismiss(cx.listener(|this, _, window, cx| this.close_provider_editor(window, cx)))
            .content(frame)
            .into_any_element()
    }
    pub(super) fn remove_provider_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let disconnect = self.remove_provider?;
        let editor = self.editor.as_ref()?;
        let title = if disconnect {
            "settings.sync.provider.disconnect.title"
        } else {
            "settings.sync.provider.delete.title"
        };
        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(crate::localization::format(
                cx,
                if disconnect {
                    "settings.sync.provider.disconnect.description"
                } else {
                    "settings.sync.provider.delete.description"
                },
                &[("name", &editor.draft.name)],
            ));
        Some(
            Dialog::new("remove-sync-provider-confirm", title)
                .focus_scope(&self.remove_focus)
                .return_focus(&self.provider_focus)
                .dismissible(!self.busy())
                .on_dismiss(cx.listener(|this, _, _, cx| {
                    this.remove_provider = None;
                    this.error = None;
                    this.validation_error = None;
                    cx.notify();
                }))
                .content(
                    DialogFrame::new("remove-sync-provider-frame", title)
                        .colors(self.colors())
                        .content(body)
                        .action(
                            Button::new("cancel-remove-sync-provider")
                                .outline()
                                .label("common.action.cancel")
                                .disabled(self.busy())
                                .on_click(|_, window, cx| Modal::dismiss(window, cx)),
                        )
                        .action(
                            Button::new("confirm-remove-sync-provider")
                                .primary()
                                .label(if self.loading {
                                    "common.status.working"
                                } else if disconnect {
                                    "common.action.disconnect"
                                } else {
                                    "common.action.delete"
                                })
                                .disabled(self.busy())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_provider_removal(window, cx)
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}
