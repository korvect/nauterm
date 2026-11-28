//! A passive themed overlay. One deadline hides Restored; there is no polling.
use super::*;
use crate::{
    components::motion::{self, Transition},
    mosh::network::{Network, State},
    typography::FontWeights,
};

pub(super) struct Status {
    enabled: bool,
    model: Network,
    displayed: State,
    opacity: Transition,
    expiry: Option<Task<()>>,
}
impl Status {
    pub fn new(enabled: bool, logs: &[LogEntry]) -> Self {
        let mut status = Self {
            enabled,
            model: Network::default(),
            displayed: State::Stable,
            opacity: Transition::new(0.),
            expiry: None,
        };
        // The connection page may already have consumed these native events.
        // Carry its last network state into the terminal instead of losing it.
        for log in logs {
            status.apply(&log.event.kind);
        }
        status
    }
    pub fn apply(&mut self, kind: &str) -> bool {
        self.enabled && self.model.apply(kind, Instant::now())
    }
}

impl Terminal {
    pub(super) fn sync_network_expiry(&mut self, cx: &mut Context<Self>) {
        self.network.expiry = None;
        if let Some(deadline) = self.network.model.restored_until() {
            self.network.expiry = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(deadline.saturating_duration_since(Instant::now()))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.network.model.restored_until() == Some(deadline)
                        && this.network.model.expire(Instant::now())
                    {
                        this.network.expiry = None;
                        cx.notify();
                    }
                });
            }));
        }
        cx.notify();
    }
    pub(super) fn network_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.network.model.state();
        let visible = self.network.enabled
            && state != State::Stable
            && !self.frame.exited
            && !matches!(self.phase(), Phase::Failed | Phase::Exited);
        if visible {
            self.network.displayed = state;
        }
        let (opacity, moving) = self.network.opacity.set(
            if visible { 1. } else { 0. },
            Instant::now(),
            motion::SELECTION,
            cx.reduce_motion(),
        );
        if moving {
            window.request_animation_frame();
        }
        if opacity <= 0. {
            return None;
        }
        let palette = self.appearance.palette();
        let restored = self.network.displayed == State::Restored;
        let foreground = rgb(if restored {
            palette.ansi[2]
        } else {
            palette.base.foreground
        });
        let background = rgb(palette.base.background).blend(foreground.alpha(0.11));
        Some(
            div()
                .absolute()
                .top(px(10.))
                .left(px(10.))
                .right(px(18.))
                .flex()
                .justify_end()
                // No hitbox, cursor, occlusion, focus or pointer handlers:
                // selection and mouse reporting still reach the terminal.
                .child(
                    div()
                        .id("mosh-network-status")
                        .role(Role::Status)
                        .aria_label(crate::localization::text(
                            cx,
                            self.network.displayed.label(),
                        ))
                        .max_w(px(300.))
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(foreground.alpha(0.24))
                        .bg(background)
                        .shadow_md()
                        .text_color(foreground)
                        .text_size(px(11.5))
                        .font_weight(FontWeights::MEDIUM)
                        .opacity(opacity)
                        .child(
                            div().flex_shrink_0().child(
                                gpui_kit::component::Icon::new(if restored {
                                    IconName::CircleCheck
                                } else {
                                    IconName::RadioTower
                                })
                                .with_size(px(14.)),
                            ),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .text_ellipsis()
                                .child(crate::localization::view(self.network.displayed.label())),
                        ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{State, Status};
    use crate::connection_state::LogEntry;

    #[test]
    fn initial_connection_events_are_retained_only_for_mosh_terminals() {
        let logs: Vec<_> = [
            "connected",
            "mosh_network_switching",
            "mosh_network_degraded",
        ]
        .into_iter()
        .enumerate()
        .map(|(sequence, kind)| LogEntry {
            sequence: sequence as u64,
            time: String::new(),
            category: "transport",
            event: crate::native::SessionEvent {
                kind: kind.into(),
                ..Default::default()
            },
        })
        .collect();
        let mosh = Status::new(true, &logs);
        assert_eq!(mosh.model.state(), State::Degraded);
        let mut other = Status::new(false, &logs);
        assert_eq!(other.model.state(), State::Stable);
        assert!(!other.apply("mosh_network_restored"));
        assert_eq!(other.model.restored_until(), None);
    }
}
