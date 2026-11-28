//! Interruptible frame-driven motion. No polling timers; reduce-motion settles.
use gpui_kit::Rgba;
use std::time::{Duration, Instant};
pub const FEEDBACK: Duration = Duration::from_millis(100);
pub const SELECTION: Duration = Duration::from_millis(160);
pub const OVERLAY: Duration = Duration::from_millis(120);
pub const MENU: Duration = Duration::from_millis(84);
pub const SEGMENTED: Duration = Duration::from_millis(180);
pub const TAB_COLOR: Duration = Duration::from_millis(180);
pub const TAB_SIZE: Duration = Duration::from_millis(280);
pub const COMPOSER_EXPAND: Duration = Duration::from_millis(180);
pub const COMPOSER_COLLAPSE: Duration = Duration::from_millis(160);
// Modal surfaces should settle promptly and release their barrier on cancel.
pub const MODAL_ENTER: Duration = Duration::from_millis(200);
pub const MODAL_EXIT: Duration = Duration::from_millis(160);
#[derive(Clone, Copy)]
pub struct Transition {
    from: f32,
    target: f32,
    started: Instant,
    duration: Duration,
    curve: Curve,
}
#[derive(Clone, Copy)]
enum Curve {
    OutCubic,
    InCubic,
    OutExponential,
    InOutCubic,
}
impl Transition {
    pub fn new(value: f32) -> Self {
        Self {
            from: value,
            target: value,
            started: Instant::now(),
            duration: FEEDBACK,
            curve: Curve::OutCubic,
        }
    }
    pub fn exponential(value: f32) -> Self {
        Self {
            curve: Curve::OutExponential,
            ..Self::new(value)
        }
    }
    pub fn ease_in_out(value: f32) -> Self {
        Self {
            curve: Curve::InOutCubic,
            ..Self::new(value)
        }
    }
    pub fn ease_in(value: f32) -> Self {
        Self {
            curve: Curve::InCubic,
            ..Self::new(value)
        }
    }
    pub fn value(&self, now: Instant) -> f32 {
        let t = (now.saturating_duration_since(self.started).as_secs_f32()
            / self.duration.as_secs_f32())
        .clamp(0., 1.);
        let amount = match self.curve {
            Curve::OutExponential if t >= 1. => 1.,
            Curve::OutExponential => 1. - 2_f32.powf(-10. * t),
            Curve::OutCubic => 1. - (1. - t).powi(3),
            Curve::InCubic => t.powi(3),
            Curve::InOutCubic if t < 0.5 => 4. * t.powi(3),
            Curve::InOutCubic => 1. - (-2. * t + 2.).powi(3) / 2.,
        };
        self.from + (self.target - self.from) * amount
    }
    pub fn set(
        &mut self,
        target: f32,
        now: Instant,
        duration: Duration,
        reduced: bool,
    ) -> (f32, bool) {
        if reduced {
            self.from = target;
            self.target = target;
            return (target, false);
        }
        if self.target != target {
            self.from = self.value(now);
            self.target = target;
            self.started = now;
            self.duration = duration;
        }
        (
            self.value(now),
            self.from != self.target && now.saturating_duration_since(self.started) < self.duration,
        )
    }
}

#[derive(Clone, Copy)]
pub struct ColorTransition([Transition; 4]);
impl ColorTransition {
    pub fn new(value: Rgba) -> Self {
        Self([value.r, value.g, value.b, value.a].map(Transition::new))
    }
    pub fn set(
        &mut self,
        target: Rgba,
        now: Instant,
        duration: Duration,
        reduced: bool,
    ) -> (Rgba, bool) {
        let mut moving = false;
        let mut values = [0.; 4];
        for ((channel, target), value) in self
            .0
            .iter_mut()
            .zip([target.r, target.g, target.b, target.a])
            .zip(&mut values)
        {
            let (next, active) = channel.set(target, now, duration, reduced);
            *value = next;
            moving |= active;
        }
        (
            Rgba {
                r: values[0],
                g: values[1],
                b: values[2],
                a: values[3],
            },
            moving,
        )
    }
}

pub fn animated_color(
    id: impl Into<gpui_kit::ElementId>,
    target: Rgba,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
) -> Rgba {
    let state = window.use_keyed_state(id, cx, |_, _| ColorTransition::new(target));
    let reduced = cx.reduce_motion();
    let (value, moving) = state.update(cx, |state, _| {
        state.set(target, Instant::now(), TAB_COLOR, reduced)
    });
    if moving {
        window.request_animation_frame();
    }
    value
}
pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0., 1.);
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_motion_preserves_value_and_stops() {
        let now = Instant::now();
        let mut m = Transition::new(0.);
        m.set(1., now, FEEDBACK, false);
        let middle = now + Duration::from_millis(40);
        let prior = m.value(middle);
        assert_eq!(m.set(0., middle, FEEDBACK, false), (prior, true));
        assert_eq!(m.set(0., middle + FEEDBACK, FEEDBACK, false), (0., false));
        assert_eq!(m.set(1., middle + FEEDBACK, FEEDBACK, true), (1., false));
    }
    #[test]
    fn unrelated_renders_do_not_restart_motion() {
        let now = Instant::now();
        let mut m = Transition::new(0.);
        m.set(1., now, SELECTION, false);
        for i in 1..20 {
            m.set(1., now + Duration::from_millis(i * 10), SELECTION, false);
        }
        assert_eq!(m.value(now + SELECTION), 1.);
    }
}
