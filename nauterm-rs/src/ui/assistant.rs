use super::*;
use crate::{
    ai::chat::{Chat, Source},
    components::motion::Transition,
};
use std::time::{Duration, Instant};

pub(super) struct WorkspaceAssistant {
    open: bool,
    width: f32,
    amount: f32,
    motion: Transition,
    chat: Option<Entity<Chat>>,
    _events: Option<Subscription>,
    _toolbar: Option<Subscription>,
}
impl Default for WorkspaceAssistant {
    fn default() -> Self {
        Self {
            open: false,
            width: 360.,
            amount: 0.,
            motion: Transition::ease_in_out(0.),
            chat: None,
            _events: None,
            _toolbar: None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Geometry {
    width: f32,
    separate: bool,
}
fn geometry(viewport: f32, requested: f32) -> Geometry {
    let viewport = if viewport.is_finite() {
        viewport.max(0.)
    } else {
        920.
    };
    // The workspace list keeps its width. Only the preview area can make room
    // for the assistant; narrow previews keep their layout under the overlay.
    let available = (viewport - crate::workspace::SIDEBAR_WIDTH).max(0.);
    let separate = available >= 900.;
    let maximum = if separate {
        (available - 360.).clamp(280., 640.)
    } else {
        (available - 24.).clamp(240., 640.)
    }
    .min(available);
    let requested = if requested.is_finite() {
        requested
    } else {
        360.
    };
    Geometry {
        width: requested.clamp(280_f32.min(maximum), maximum),
        separate,
    }
}
impl Nauterm {
    pub(super) fn terminal_ai_source(session: &SessionView, data: &AppState) -> Source {
        let host = match &session.restore_target {
            Some(
                RestoreTarget::Host { id, .. }
                | RestoreTarget::MoshHost { id, .. }
                | RestoreTarget::TelnetHost { id, .. },
            ) => data
                .hosts
                .iter()
                .find(|host| host.id == *id)
                .and_then(|host| host.uuid.clone()),
            _ => None,
        };
        let view = match &session.content {
            TabContent::Terminal(view) if session.replay.is_none() => Some(view),
            _ => None,
        };
        Source::terminal(view, host, session.appearance(data).palette())
    }
    pub(super) fn workspace_overview_visible(&self) -> bool {
        self.page == Page::Sessions
            && self.data.general.workspace_enabled
            && (self.overview || self.active_terminal().is_none())
    }
    pub(super) fn assistant_available(&self) -> bool {
        self.workspace_overview_visible()
            || (self.page == Page::Sessions
                && !self.overview
                && self
                    .terminals
                    .iter()
                    .find(|tab| Some(tab.id) == self.active_terminal())
                    .is_some_and(|tab| !tab.show_sftp))
    }
    pub(super) fn assistant_open(&self) -> bool {
        if self.workspace_overview_visible() {
            self.workspace_assistants
                .get(&self.data.selected_workspace)
                .is_some_and(|state| state.open)
        } else {
            self.assistant_available()
                && self
                    .terminals
                    .iter()
                    .find(|tab| Some(tab.id) == self.active_terminal())
                    .is_some_and(|tab| tab.tools.ai_open(tab.show_sftp))
        }
    }
    pub(super) fn assistant_accent(&self) -> Rgba {
        if self.workspace_overview_visible() {
            Palette::new(self.data.dark).accent
        } else {
            self.terminals
                .iter()
                .find(|tab| Some(tab.id) == self.active_terminal())
                .map(|tab| rgb(tab.active().appearance(&self.data).palette().accent))
                .unwrap_or(Palette::new(self.data.dark).accent)
        }
    }
    pub(super) fn toggle_assistant(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.assistant_available() {
            return;
        }
        if self.workspace_overview_visible() {
            let state = self
                .workspace_assistants
                .entry(self.data.selected_workspace)
                .or_default();
            state.open = !state.open;
            self.workspace_assistant_drag = None;
            // Rendering advances the transition and requests animation frames.
            // GPUI cannot request a frame for a current view during input dispatch.
        } else {
            let selected = self.active_terminal();
            let mut closed = false;
            if let Some(tab) = self
                .terminals
                .iter_mut()
                .find(|tab| Some(tab.id) == selected)
            {
                tab.tools.toggle_ai();
                closed = !tab.tools.ai_open(tab.show_sftp);
            }
            self.refresh_terminal_files(window, cx);
            if closed && let Some(tab) = self.terminals.iter().find(|tab| Some(tab.id) == selected)
            {
                tab.focus(window, cx);
            }
        }
        cx.notify();
    }
    /// Render phase only: advancing a transition can request an animation frame.
    pub(super) fn refresh_workspace_assistants(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids: BTreeSet<_> = self
            .data
            .workspaces
            .iter()
            .map(|workspace| workspace.id)
            .collect();
        self.workspace_assistants.retain(|id, _| ids.contains(id));
        let visible = self.workspace_overview_visible();
        if !visible
            || !window.is_window_active()
            || crate::components::overlay::is_modal_open(window, cx)
        {
            self.workspace_assistant_drag = None;
        }
        let now = Instant::now();
        let reduced = cx.reduce_motion();
        for (id, state) in &mut self.workspace_assistants {
            let selected = visible && *id == self.data.selected_workspace;
            let (amount, moving) = state.motion.set(
                if state.open { 1. } else { 0. },
                now,
                Duration::from_millis(180),
                reduced,
            );
            state.amount = amount;
            if moving && selected && window.is_window_active() {
                window.request_animation_frame();
            }
            if selected && state.open && state.chat.is_none() {
                let service = self.store.ai_providers().map_err(|error| error.to_string());
                let view = cx.new(|cx| {
                    Chat::new(
                        service,
                        Source::workspace(self.data.dark),
                        self.data.ai.clone(),
                        window,
                        cx,
                    )
                });
                let events = cx.subscribe_in(
                    &view,
                    window,
                    |this, _, _: &crate::ai::chat::Configure, window, cx| {
                        this.open_settings_at(Some(crate::settings::Page::Ai), window, cx);
                    },
                );
                state._toolbar = Some(Chat::observe_toolbar(&view, cx));
                state.chat = Some(view);
                state._events = Some(events);
            }
            if let Some(chat) = &state.chat {
                chat.update(cx, |chat, cx| {
                    chat.sync(
                        selected && state.open && window.is_window_active(),
                        &self.data.ai,
                        Source::workspace(self.data.dark),
                        window,
                        cx,
                    )
                });
            }
        }
    }
    pub(super) fn workspace_assistant_inset(&self, window: &Window) -> f32 {
        if !self.workspace_overview_visible() {
            return 0.;
        }
        let Some(state) = self.workspace_assistants.get(&self.data.selected_workspace) else {
            return 0.;
        };
        let geometry = geometry(f32::from(window.viewport_size().width), state.width);
        if geometry.separate {
            geometry.width * state.amount
        } else {
            0.
        }
    }
    pub(super) fn workspace_with_assistant(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let body = self.workspace_page(window, cx);
        let Some(state) = self
            .workspace_assistants
            .get(&self.data.selected_workspace)
            .filter(|state| state.open || state.amount > 0.)
        else {
            return body;
        };
        let Some(chat) = state.chat.clone() else {
            return body;
        };
        let p = Palette::new(self.data.dark);
        let geometry = geometry(f32::from(window.viewport_size().width), state.width);
        let id = self.data.selected_workspace;
        let width = geometry.width;
        let dragging = self
            .workspace_assistant_drag
            .is_some_and(|(workspace, _, _)| workspace == id);
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .right(px(self.workspace_assistant_inset(window)))
                    .flex()
                    .min_w_0()
                    .min_h_0()
                    .child(body),
            )
            .child(
                div()
                    .id("workspace-ai-panel")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(px(-width * (1. - state.amount)))
                    .w(px(width))
                    .flex()
                    .min_h_0()
                    .bg(p.surface)
                    .occlude()
                    .border_l_1()
                    .border_color(if dragging { p.accent } else { p.border })
                    .child(
                        div()
                            .id("workspace-ai-divider")
                            .w(px(6.))
                            .h_full()
                            .flex_shrink_0()
                            .cursor(CursorStyle::ResizeLeftRight)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    this.split_drag = None;
                                    this.terminal_tools_drag = None;
                                    this.workspace_assistant_drag =
                                        Some((id, event.position.x, width));
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .child(Chat::workspace_toolbar(&chat, cx))
                            .child(div().flex_1().min_h_0().child(chat)),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn resize_workspace_assistant(
        &mut self,
        event: &MouseMoveEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, start, width)) = self.workspace_assistant_drag else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left)
            || !self.workspace_overview_visible()
            || self.data.selected_workspace != id
        {
            self.workspace_assistant_drag = None;
            cx.notify();
            return;
        }
        if let Some(state) = self.workspace_assistants.get_mut(&id) {
            state.width = geometry(
                f32::from(window.viewport_size().width),
                width + f32::from(start - event.position.x),
            )
            .width;
            state.motion.set(1., Instant::now(), Duration::ZERO, true);
            state.amount = 1.;
            cx.notify();
        }
        cx.stop_propagation();
    }
}

#[cfg(test)]
mod tests {
    use super::geometry;
    #[test]
    fn workspace_ai_overlays_narrow_windows_and_preserves_wide_workspace_space() {
        assert!(!geometry(750., 360.).separate);
        assert_eq!(geometry(750., 360.).width, 360.);
        assert!(!geometry(920., 360.).separate);
        assert!(geometry(1158., 360.).separate);
        assert_eq!(geometry(1158., 900.).width, 540.);
        assert_eq!(geometry(1600., 900.).width, 640.);
        assert_eq!(geometry(300., 360.).width, 42.);
        assert_eq!(geometry(258., 360.).width, 0.);
        assert!(geometry(f32::NAN, f32::NAN).width.is_finite());
    }
    #[test]
    fn assistant_never_covers_the_workspace_list_or_squeezes_a_docked_preview() {
        for viewport in [0., 250., 300., 750., 920., 1158., 1600.] {
            let available = (viewport - crate::workspace::SIDEBAR_WIDTH).max(0.);
            for requested in [-20., 280., 360., 640., 900., f32::NAN, f32::INFINITY] {
                let panel = geometry(viewport, requested);
                assert!(panel.width.is_finite() && panel.width >= 0.);
                assert!(panel.width <= available);
                if panel.separate {
                    assert!(available - panel.width >= 360.);
                }
            }
        }
    }
}
