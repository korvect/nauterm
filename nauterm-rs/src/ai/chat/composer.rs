use super::*;
use crate::components::motion::mix;

impl Chat {
    pub(super) fn composer(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let busy = self.busy();
        let stopping = self
            .session
            .as_ref()
            .is_some_and(|session| session.stopping());
        let waiting = self
            .session
            .as_ref()
            .is_some_and(|session| session.has_unresolved());
        let can_stop = busy || waiting;
        let focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let dark =
            p.background.r * 0.2126 + p.background.g * 0.7152 + p.background.b * 0.0722 < 0.5;
        let fill = if matches!(self.source, Source::Workspace { .. }) {
            Palette::new(dark).background
        } else {
            mix(p.background, p.text, if dark { 0.04 } else { 0.03 })
        };
        let send_disabled = stopping
            || !can_stop
                && ((self.input.read(cx).value().trim().is_empty() && self.attachments.is_empty())
                    || self.picking_attachments
                    || waiting
                    || self.history_loading
                    || self.editor.is_some()
                    || self.provider.is_none()
                    || self.service.is_none());
        let focus = self.input.read(cx).focus_handle(cx);
        let context = self.context_controls(cx);
        let attachments = self.pending_attachments(cx);
        let has_chips = context.is_some() || attachments.is_some();
        div()
            .flex_shrink_0()
            .px(px(12.))
            .pt(px(8.))
            .pb(px(12.))
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .min_h(px(92.))
                    .max_h(px(224.))
                    .flex()
                    .flex_col()
                    .rounded(px(18.))
                    .border_1()
                    .border_color(if focused {
                        p.accent.opacity(0.72)
                    } else {
                        p.border
                    })
                    .bg(fill)
                    // A 3-sigma tail must fade inside the 12px panel insets.
                    .shadow(vec![gpui_kit::base::box_shadow(
                        px(0.),
                        px(2.),
                        px(4.),
                        px(-3.),
                        rgb(0).opacity(if dark { 0.16 } else { 0.06 }).into(),
                    )])
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        if !waiting {
                            focus.focus(window, cx);
                        }
                    })
                    .when(has_chips, |card| {
                        card.child(
                            div()
                                .id("ai-composer-chips")
                                .min_h_0()
                                .max_h(px(72.))
                                .overflow_y_scroll()
                                .scrollbars()
                                .hover_only()
                                .children(context)
                                .children(
                                    attachments
                                        .map(|chips| div().px(px(8.)).pt(px(8.)).child(chips)),
                                ),
                        )
                    })
                    .child(
                        div()
                            .flex_shrink_0()
                            .px(px(14.))
                            .pt(px(10.))
                            .pb(px(5.))
                            .child(
                                Input::multiline(&self.input)
                                    .palette(p)
                                    .appearance(false)
                                    .bordered(false)
                                    .disabled(waiting)
                                    .aria_label(crate::localization::text(cx, "AI message")),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex_shrink_0()
                            .px(px(8.))
                            .pb(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(self.attach_button(cx))
                            .children(self.restore_context_button(cx))
                            .child(self.provider_controls(busy || self.history_loading, cx))
                            .child(
                                Button::new("ai-send")
                                    .ghost()
                                    .custom(
                                        ButtonCustomVariant::for_surface(p)
                                            .color(p.accent.into())
                                            .foreground(p.background.into())
                                            .hover(mix(p.accent, p.text, 0.1).into())
                                            .active(mix(p.accent, p.text, 0.16).into()),
                                    )
                                    .size(px(24.))
                                    .rounded_full()
                                    .icon_size(px(16.))
                                    .icon(if can_stop {
                                        crate::assets::Glyph::Stop.icon()
                                    } else {
                                        crate::assets::icon(IconName::ArrowUp)
                                    })
                                    .tooltip(if stopping {
                                        "Stopping"
                                    } else if can_stop {
                                        "Stop conversation"
                                    } else {
                                        "Send"
                                    })
                                    .disabled(send_disabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.busy()
                                            || this
                                                .session
                                                .as_ref()
                                                .is_some_and(|session| session.has_unresolved())
                                        {
                                            if let Some(session) = &this.session {
                                                this.error = session
                                                    .stop_turn()
                                                    .err()
                                                    .map(|error| error.to_string());
                                            }
                                            cx.notify();
                                        } else {
                                            this.send(window, cx);
                                        }
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}
