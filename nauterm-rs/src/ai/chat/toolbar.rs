use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
struct ToolbarState {
    history: bool,
    blocked: bool,
    empty: bool,
}
impl Chat {
    fn toolbar_state(&self) -> ToolbarState {
        ToolbarState {
            history: self.history.is_some(),
            blocked: self.busy() || self.history_loading || self.picking_attachments,
            empty: self.attachments.is_empty()
                && self
                    .session
                    .as_ref()
                    .is_none_or(|session| session.is_empty()),
        }
    }
    /// Repaint the host toolbar only when its controls change, not on each token.
    pub fn observe_toolbar<T: 'static>(view: &Entity<Self>, cx: &mut Context<T>) -> Subscription {
        let mut previous = view.read(cx).toolbar_state();
        cx.observe(view, move |_, view, cx| {
            let next = view.read(cx).toolbar_state();
            if next != previous {
                previous = next;
                cx.notify();
            }
        })
    }
    /// Shared by the terminal mode strip and the workspace assistant header.
    pub fn toolbar_actions(view: &Entity<Self>, cx: &App) -> AnyElement {
        let this = view.read(cx);
        let p = this.palette();
        let state = this.toolbar_state();
        div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(Self::history_button(view, state, p))
            .child(Self::secondary_button(view, state, p))
            .into_any_element()
    }
    pub fn workspace_toolbar(view: &Entity<Self>, cx: &App) -> AnyElement {
        let this = view.read(cx);
        let p = this.palette();
        let state = this.toolbar_state();
        div()
            .h(px(48.))
            .flex_shrink_0()
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(8.))
            .when(state.history, |bar| {
                bar.child(Self::history_button(view, state, p))
            })
            .when(!state.history, |bar| {
                bar.child(
                    crate::assets::icon(IconName::Sparkles)
                        .size(px(18.))
                        .text_color(p.accent),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.))
                    .font_weight(crate::typography::FontWeights::MEDIUM)
                    .child(crate::localization::view(if state.history {
                        "Conversation history"
                    } else {
                        "AI Assistant"
                    })),
            )
            .when(!state.history, |bar| {
                bar.child(Self::history_button(view, state, p))
            })
            .child(Self::secondary_button(view, state, p))
            .into_any_element()
    }
    fn history_button(view: &Entity<Self>, state: ToolbarState, p: Palette) -> Button {
        let owner = view.downgrade();
        Button::new("ai-history-button")
            .ghost()
            .custom(ButtonCustomVariant::for_surface(p))
            .size(px(27.))
            .icon_size(px(16.))
            .text_color(p.muted)
            .icon(if state.history {
                IconName::ArrowLeft
            } else {
                IconName::RotateCcwClock
            })
            .tooltip(if state.history {
                "Back to conversation"
            } else {
                "Conversation history"
            })
            .disabled(state.blocked)
            .on_click(move |_, window, cx| {
                let _ = owner.update(cx, |this, cx| this.show_history(window, cx));
            })
    }
    fn secondary_button(view: &Entity<Self>, state: ToolbarState, p: Palette) -> Button {
        let owner = view.downgrade();
        Button::new("ai-secondary-action")
            .ghost()
            .custom(ButtonCustomVariant::for_surface(p))
            .size(px(27.))
            .icon_size(px(16.))
            .text_color(p.muted)
            .icon(if state.history {
                IconName::RefreshCw
            } else {
                IconName::MessageSquarePlus
            })
            .tooltip(if state.history {
                "Refresh conversations"
            } else {
                "New conversation"
            })
            .disabled(state.blocked || (!state.history && state.empty))
            .on_click(move |_, window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    if this.history.is_some() {
                        this.refresh_history(window, cx);
                    } else {
                        this.new_conversation(window, cx);
                    }
                });
            })
    }
}
