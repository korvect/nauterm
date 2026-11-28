//! Event-driven bell feedback. The paint-only flash never intercepts input.
use super::*;
use crate::{components::motion::Transition, terminal_bell::Counter};

pub struct Observed;
impl EventEmitter<Observed> for Terminal {}
pub struct Indicator;
impl EventEmitter<Indicator> for Terminal {}

pub(super) struct State {
    counter: Counter,
    until: Option<Instant>,
    expiry: Option<Task<()>>,
    opacity: Transition,
}
impl State {
    pub fn new(count: u64) -> Self {
        Self {
            counter: Counter::new(count),
            until: None,
            expiry: None,
            opacity: Transition::new(0.),
        }
    }
}

impl Terminal {
    pub(super) fn observe_bell(&mut self, cx: &mut Context<Self>) {
        if self
            .bell
            .counter
            .observe(self.frame.bell_count, !self.session.is_replay())
        {
            cx.emit(Observed);
        }
    }
    pub(crate) fn deliver_bell(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.bell.counter.take() || self.session.is_replay() {
            return;
        }
        let preferences = self.appearance.bell;
        if preferences.sound {
            window.play_system_bell();
        }
        if preferences.visual {
            let deadline = Instant::now() + Duration::from_millis(160);
            self.bell.until = Some(deadline);
            self.bell.expiry = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(deadline.saturating_duration_since(Instant::now()))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.bell.until == Some(deadline) {
                        this.bell.until = None;
                        this.bell.expiry = None;
                        cx.notify();
                    }
                });
            }));
            cx.notify();
        }
        if preferences.tab_indicator {
            cx.emit(Indicator);
        }
    }
    pub(super) fn clear_visual_bell(&mut self) {
        self.bell.until = None;
        self.bell.expiry = None;
        self.bell.opacity = Transition::new(0.);
    }
    pub(super) fn bell_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let visible = self.appearance.bell.visual
            && self.bell.until.is_some_and(|until| until > Instant::now());
        let (opacity, moving) = self.bell.opacity.set(
            f32::from(visible),
            Instant::now(),
            Duration::from_millis(70),
            cx.reduce_motion(),
        );
        if moving {
            window.request_animation_frame();
        }
        if opacity <= 0. {
            return None;
        }
        let color = rgb(self.appearance.palette().base.foreground).alpha(0.14 * opacity);
        Some(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    window.paint_quad(fill(bounds, color));
                },
            )
            .absolute()
            .inset_0()
            .into_any_element(),
        )
    }
}
