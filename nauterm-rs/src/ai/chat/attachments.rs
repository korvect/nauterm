use super::*;
use crate::ai::attachment::{Attachment, MAX_COUNT, Summary};

pub(super) fn badge(summary: &Summary, palette: Palette) -> Div {
    let name = if summary.redacted {
        crate::localization::view(format!("{} (redacted)", summary.name)).into_any_element()
    } else {
        summary.name.clone().into_any_element()
    };
    div()
        .max_w(px(210.))
        .min_w_0()
        .h(px(29.))
        .px(px(7.))
        .flex()
        .items_center()
        .gap(px(5.))
        .rounded(px(6.))
        .border_1()
        .border_color(palette.border)
        .bg(palette.background)
        .child(
            gpui_kit::component::Icon::new(if summary.image {
                IconName::Image
            } else {
                IconName::FileText
            })
            .with_size(px(13.))
            .text_color(palette.accent),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(11.))
                .child(name),
        )
}
impl Chat {
    fn pick_attachments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy()
            || self.history_loading
            || self.history.is_some()
            || self.picking_attachments
            || self.editor.is_some()
            || self
                .session
                .as_ref()
                .is_some_and(|session| session.has_unresolved())
        {
            return;
        }
        let remaining = MAX_COUNT.saturating_sub(self.attachments.len());
        if remaining == 0 {
            return;
        }
        self.picking_attachments = true;
        self.error = None;
        let session = self.session.clone();
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(crate::localization::text(cx, "Attach files")),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = match picker.await {
                Ok(Ok(Some(paths))) => {
                    cx.background_spawn(async move {
                        let mut attachments = vec![];
                        let mut errors = vec![];
                        for path in paths {
                            if attachments.len() == remaining {
                                errors.push("Attach at most five files per message.".into());
                                break;
                            }
                            match Attachment::read(&path) {
                                Ok(attachment) => attachments.push(attachment),
                                Err(error) => errors.push(error.to_string()),
                            }
                        }
                        Ok((attachments, errors))
                    })
                    .await
                }
                Ok(Ok(None)) => Ok((vec![], vec![])),
                _ => Err(anyhow::anyhow!("Could not open the attachment picker.")),
            };
            let _ = this.update(cx, |this, cx| {
                this.picking_attachments = false;
                // A conversation can be released while the native picker is open.
                let same = session
                    .as_ref()
                    .zip(this.session.as_ref())
                    .is_some_and(|(old, current)| Arc::ptr_eq(old, current));
                if same {
                    match result {
                        Ok((attachments, errors)) => {
                            this.attachments.extend(attachments);
                            this.error = (!errors.is_empty()).then(|| errors.join("\n"));
                        }
                        Err(error) => this.error = Some(error.to_string()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn attach_button(&self, cx: &mut Context<Self>) -> Button {
        Button::new("ai-attach-files")
            .ghost()
            .custom(ButtonCustomVariant::for_surface(self.palette()))
            .size(px(30.))
            .rounded_full()
            .icon_size(px(17.))
            .text_color(self.palette().muted)
            .icon(crate::assets::icon(IconName::Plus))
            .tooltip(if self.picking_attachments {
                "Loading attachments…"
            } else {
                "Attach files"
            })
            .disabled(
                self.picking_attachments
                    || self.busy()
                    || self.history_loading
                    || self.editor.is_some()
                    || self.session.is_none()
                    || self
                        .session
                        .as_ref()
                        .is_some_and(|session| session.has_unresolved())
                    || self.attachments.len() >= MAX_COUNT,
            )
            .on_click(cx.listener(|this, _, window, cx| this.pick_attachments(window, cx)))
    }
    pub(super) fn pending_attachments(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.attachments.is_empty() {
            return None;
        }
        let palette = self.palette();
        Some(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_wrap()
                .gap(px(5.))
                .children(self.attachments.iter().map(|attachment| {
                    let id = attachment.id.clone();
                    let summary = Summary {
                        name: attachment.name.clone(),
                        image: attachment.kind == crate::ai::attachment::Kind::Image,
                        redacted: attachment.redacted,
                    };
                    badge(&summary, palette).pr(px(2.)).child(
                        Button::new(SharedString::from(format!("ai-remove-attachment-{id}")))
                            .xsmall()
                            .ghost()
                            .custom(ButtonCustomVariant::for_surface(palette))
                            .icon(crate::assets::icon(IconName::X))
                            .tooltip(format!("Remove {}", attachment.name))
                            .disabled(self.picking_attachments || self.busy())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.attachments.retain(|a| a.id != id);
                                cx.notify();
                            })),
                    )
                }))
                .into_any_element(),
        )
    }
}
