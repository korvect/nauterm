//! Layout participates in the animation; contents retain their full width.
//! Retiring models stay paintable but are detached from the live editor slot.
use super::motion::Transition;
use gpui_kit::{prelude::*, *};
use std::time::{Duration, Instant};

pub const ENTER: Duration = Duration::from_millis(180);
pub const EXIT: Duration = Duration::from_millis(160);
pub const SIDEBAR: Duration = Duration::from_millis(160);

pub struct Presence<T> {
    retired: Option<T>,
    motion: Transition,
    amount: f32,
}
impl<T> Default for Presence<T> {
    fn default() -> Self {
        Self {
            retired: None,
            motion: Transition::new(0.),
            amount: 0.,
        }
    }
}
impl<T> Presence<T> {
    pub fn retire(&mut self, value: T) {
        self.retired = Some(value);
    }
    pub fn take_retired(&mut self) -> Option<T> {
        self.retired.take()
    }
    pub fn amount(&self) -> f32 {
        self.amount
    }
    pub fn sample(&mut self, open: bool, now: Instant, reduced: bool) -> (f32, bool) {
        if open {
            self.retired = None;
        }
        let (amount, moving) = self.motion.set(
            f32::from(open),
            now,
            if open { ENTER } else { EXIT },
            reduced,
        );
        self.amount = amount;
        if !open && !moving {
            self.retired = None;
        }
        (amount, moving)
    }
    pub fn frame(&mut self, open: bool, window: &Window, cx: &App) -> f32 {
        let (amount, moving) = self.sample(open, Instant::now(), cx.reduce_motion());
        if moving {
            window.request_animation_frame();
        }
        amount
    }
}

pub fn width_transition(
    id: impl Into<ElementId>,
    target: f32,
    initial: f32,
    duration: Duration,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let state = window.use_keyed_state(id, cx, |_, _| Transition::new(initial));
    let reduced = cx.reduce_motion();
    let (width, moving) = state.update(cx, |motion, _| {
        motion.set(target, Instant::now(), duration, reduced)
    });
    if moving {
        window.request_animation_frame();
    }
    width
}

pub fn slide(
    id: impl Into<ElementId>,
    width: f32,
    amount: f32,
    interactive: bool,
    content: impl IntoElement,
) -> AnyElement {
    div()
        .id(id)
        .relative()
        .w(px(width * amount))
        .h_full()
        .min_h_0()
        .flex_shrink_0()
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(width))
                .child(content),
        )
        // The overlay is painted above the retiring subtree. Child hitboxes
        // cannot invoke an action against the owner's now-empty live slot.
        .when(!interactive, |view| {
            view.child(div().absolute().inset_0().occlude().capture_any_mouse_down(
                |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                },
            ))
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{ENTER, EXIT, Presence};
    use std::time::Instant;
    #[test]
    fn retiring_content_survives_exit_and_reversal_is_continuous() {
        let mut panel = Presence::default();
        let now = Instant::now();
        panel.sample(true, now, false);
        panel.sample(true, now + ENTER, false);
        panel.retire("old editor");
        panel.sample(false, now + ENTER, false);
        let halfway = now + ENTER + EXIT / 2;
        let (before, _) = panel.sample(false, halfway, false);
        assert!(before > 0. && before < 1.);
        assert_eq!(panel.retired, Some("old editor"));
        let (after, _) = panel.sample(true, halfway, false);
        assert_eq!(before, after);
        assert!(panel.retired.is_none());
        panel.retire("second editor");
        panel.sample(false, halfway, false);
        let (amount, moving) = panel.sample(false, halfway + EXIT, false);
        assert_eq!(amount, 0.);
        assert!(!moving);
        assert!(panel.retired.is_none());
    }
    #[test]
    fn reduced_motion_releases_retiring_models_immediately() {
        let mut panel = Presence::default();
        panel.sample(true, Instant::now(), true);
        panel.retire("editor");
        assert_eq!(panel.sample(false, Instant::now(), true), (0., false));
        assert!(panel.retired.is_none());
    }
}
