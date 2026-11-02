//! Follow the latest reply with Flutter's 120 ms ease-out cubic transition.
//! All coordinates use GPUI's negative vertical scroll offsets.
use std::time::{Duration, Instant};

const DURATION: Duration = Duration::from_millis(120);
const EPSILON: f32 = 0.5;

#[derive(Default)]
pub(super) struct Latest {
    motion: Option<Motion>,
    written: Option<f32>,
}
struct Motion {
    from: f32,
    target: f32,
    started: Instant,
}
impl Latest {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    /// A scrollbar drag or keyboard scroll is not an animation tick. Layout
    /// clamping after a viewport/content shrink must not count as user input.
    pub fn moved(&self, actual: f32, extent: f32) -> bool {
        self.written
            .is_some_and(|written| (actual - written.clamp(-extent.max(0.), 0.)).abs() > EPSILON)
    }
    pub fn step(&mut self, actual: f32, extent: f32, now: Instant, reduced: bool) -> (f32, bool) {
        let target = -extent.max(0.);
        let actual = actual.clamp(target, 0.);
        if reduced || (target - actual).abs() <= EPSILON {
            self.motion = None;
            self.written = Some(target);
            return (target, false);
        }
        if self
            .motion
            .as_ref()
            .is_none_or(|motion| (target - motion.target).abs() > EPSILON)
        {
            self.motion = Some(Motion {
                from: actual,
                target,
                started: now,
            });
        }
        let motion = self.motion.as_ref().unwrap();
        let t = (now.saturating_duration_since(motion.started).as_secs_f32()
            / DURATION.as_secs_f32())
        .clamp(0., 1.);
        let offset = motion.from + (motion.target - motion.from) * (1. - (1. - t).powi(3));
        self.written = Some(offset);
        if t >= 1. {
            self.motion = None;
        }
        (offset, t < 1.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn follows_with_reference_timing_without_treating_frames_as_user_scroll() {
        let mut latest = Latest::default();
        let start = Instant::now();
        assert_eq!(latest.step(0., 1000., start, false), (0., true));
        assert!(!latest.moved(0., 1000.));
        assert_eq!(
            latest.step(0., 1000., start + DURATION / 2, false),
            (-875., true)
        );
        assert!(!latest.moved(-875., 1000.));
        assert!(latest.moved(-800., 1000.));
        latest.clear();
        assert!(!latest.moved(-800., 1000.));
    }
    #[test]
    fn new_content_retargets_from_the_current_position_and_finishes() {
        let mut latest = Latest::default();
        let start = Instant::now();
        latest.step(0., 1000., start, false);
        let (offset, _) = latest.step(0., 1000., start + DURATION / 2, false);
        assert_eq!(
            latest.step(offset, 1200., start + DURATION / 2, false),
            (offset, true)
        );
        assert_eq!(
            latest.step(offset, 1200., start + DURATION * 2, false),
            (-1200., false)
        );
    }
    #[test]
    fn resize_clamps_and_reduced_motion_do_not_restart_the_transition() {
        let mut latest = Latest::default();
        let now = Instant::now();
        latest.step(0., 1000., now, false);
        latest.step(0., 1000., now + DURATION / 2, false);
        assert!(!latest.moved(-500., 500.));
        assert_eq!(
            latest.step(-500., 500., now + DURATION / 2, false),
            (-500., false)
        );
        assert_eq!(latest.step(-500., 1500., now, true), (-1500., false));
    }
}
