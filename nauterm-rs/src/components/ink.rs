//! Product ink feedback, matching Flutter InkRipple's timeline. No timers.
use gpui_kit::{Bounds, Corners, PathBuilder, Pixels, Point, Rgba, Window, point, px};
use std::time::{Duration, Instant};

pub(super) const HOVER: Duration = Duration::from_millis(50);
pub(super) const HIGHLIGHT: Duration = Duration::from_millis(200);
const FADE_IN: f32 = 0.075;
const EXPAND: f32 = 0.225;
const CONFIRM: f32 = 0.375;
const CANCEL: f32 = 0.075;

#[derive(Clone, Copy)]
pub(super) struct Highlight {
    from: f32,
    target: f32,
    started: Instant,
    duration: Duration,
}
impl Highlight {
    pub fn new() -> Self {
        Self {
            from: 0.,
            target: 0.,
            started: Instant::now(),
            duration: HOVER,
        }
    }
    fn value(&self, now: Instant) -> f32 {
        let t = (now.saturating_duration_since(self.started).as_secs_f32()
            / self.duration.as_secs_f32())
        .clamp(0., 1.);
        self.from + (self.target - self.from) * t
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
        if target != self.target {
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
struct Release {
    at: Instant,
    progress: f32,
    alpha: f32,
    confirmed: bool,
}
#[derive(Clone, Copy)]
struct Ripple {
    id: u64,
    origin: Point<f32>,
    born: Instant,
    release: Option<Release>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Sample {
    pub center: Point<f32>,
    pub progress: f32,
    pub alpha: f32,
}
impl Ripple {
    fn progress(&self, now: Instant) -> f32 {
        if let Some(release) = self.release.filter(|r| r.confirmed) {
            (release.progress + now.saturating_duration_since(release.at).as_secs_f32() / EXPAND)
                .min(1.)
        } else {
            now.saturating_duration_since(self.born)
                .as_secs_f32()
                .min(1.)
        }
    }
    fn sample(&self, now: Instant) -> (Sample, bool) {
        let progress = self.progress(now);
        let eased = ease(progress);
        let age = now.saturating_duration_since(self.born).as_secs_f32();
        let (alpha, moving) = if let Some(r) = self.release {
            let elapsed = now.saturating_duration_since(r.at).as_secs_f32();
            if r.confirmed {
                let out = ((elapsed - EXPAND) / (CONFIRM - EXPAND)).clamp(0., 1.);
                ((age / FADE_IN).min(1.) * (1. - out), elapsed < CONFIRM)
            } else {
                (
                    r.alpha * (1. - (elapsed / CANCEL).min(1.)),
                    elapsed < CANCEL,
                )
            }
        } else {
            ((age / FADE_IN).min(1.), age < 1.)
        };
        (
            Sample {
                center: point(
                    self.origin.x + (0.5 - self.origin.x) * eased,
                    self.origin.y + (0.5 - self.origin.y) * eased,
                ),
                progress: eased,
                alpha,
            },
            moving,
        )
    }
}
#[derive(Default)]
pub(super) struct Ink {
    next: u64,
    ripples: Vec<Ripple>,
}
impl Ink {
    pub fn begin(&mut self, origin: Point<f32>, now: Instant, reduced: bool) -> Option<u64> {
        if reduced {
            self.clear();
            return None;
        }
        self.next = self.next.wrapping_add(1);
        // Retain overlapping quick taps, but bound transient paint work.
        if self.ripples.len() == 8 {
            self.ripples.remove(0);
        }
        self.ripples.push(Ripple {
            id: self.next,
            origin,
            born: now,
            release: None,
        });
        Some(self.next)
    }
    pub fn finish(&mut self, id: Option<u64>, confirmed: bool, now: Instant) {
        if let Some(ripple) = self.ripples.iter_mut().find(|r| Some(r.id) == id) {
            if ripple.release.is_some_and(|r| r.confirmed || !confirmed) {
                return;
            }
            let progress = ripple.progress(now);
            let alpha = ripple.sample(now).0.alpha;
            ripple.release = Some(Release {
                at: now,
                progress,
                alpha,
                confirmed,
            });
        }
    }
    pub fn clear(&mut self) {
        self.ripples.clear();
    }
    pub fn sample(&mut self, now: Instant, reduced: bool) -> (Vec<Sample>, bool) {
        if reduced {
            self.clear();
            return (Vec::new(), false);
        }
        self.ripples.retain(|r| {
            r.release.is_none_or(|release| {
                now.saturating_duration_since(release.at).as_secs_f32()
                    < if release.confirmed { CONFIRM } else { CANCEL }
            })
        });
        let mut moving = false;
        let samples = self
            .ripples
            .iter()
            .map(|r| {
                let (sample, active) = r.sample(now);
                moving |= active;
                sample
            })
            .collect();
        (samples, moving)
    }
}
// Flutter Curves.ease: cubic-bezier(.25, .1, .25, 1).
fn ease(x: f32) -> f32 {
    let bezier = |t: f32, a: f32, b: f32| {
        3. * (1. - t).powi(2) * t * a + 3. * (1. - t) * t * t * b + t.powi(3)
    };
    let (mut low, mut high) = (0., 1.);
    for _ in 0..18 {
        let mid = (low + high) / 2.;
        if bezier(mid, 0.25, 0.25) < x {
            low = mid;
        } else {
            high = mid;
        }
    }
    bezier((low + high) / 2., 0.1, 1.)
}

pub(super) fn faded(mut color: Rgba, opacity: f32) -> Rgba {
    color.a *= opacity;
    color
}

/// GPUI's content masks are rectangular. Intersect two convex contours before
/// tessellation, so ink really clips to each caller-provided rounded corner.
fn clipped_circle(
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    sample: Sample,
) -> Vec<Point<f32>> {
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    if w <= 0. || h <= 0. {
        return Vec::new();
    }
    let radius = w.hypot(h) * 0.5 * (0.3 + 0.7 * sample.progress) + 5. * sample.progress;
    let center = point(sample.center.x * w, sample.center.y * h);
    let mut polygon: Vec<_> = (0..128)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 128.;
            point(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect();
    let mut clip = Vec::with_capacity(36);
    for (cx, cy, r, angle) in [
        (w, 0., corners.top_right, -90.),
        (w, h, corners.bottom_right, 0.),
        (0., h, corners.bottom_left, 90.),
        (0., 0., corners.top_left, 180.),
    ] {
        let r = f32::from(r).max(0.).min(w.min(h) * 0.5);
        let cx = if cx == 0. { r } else { w - r };
        let cy = if cy == 0. { r } else { h - r };
        for step in 0..=8 {
            let a = (angle + step as f32 * 90. / 8.).to_radians();
            clip.push(point(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    let cross = |a: Point<f32>, b: Point<f32>, p: Point<f32>| {
        (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
    };
    for i in 0..clip.len() {
        if polygon.is_empty() {
            break;
        }
        let a = clip[i];
        let b = clip[(i + 1) % clip.len()];
        if (b.x - a.x).abs() + (b.y - a.y).abs() < 0.0001 {
            continue;
        }
        let input = std::mem::take(&mut polygon);
        let mut previous = *input.last().unwrap();
        let mut dp = cross(a, b, previous);
        for current in input {
            let dc = cross(a, b, current);
            if (dp >= 0.) != (dc >= 0.) {
                let t = dp / (dp - dc);
                polygon.push(point(
                    previous.x + (current.x - previous.x) * t,
                    previous.y + (current.y - previous.y) * t,
                ));
            }
            if dc >= 0. {
                polygon.push(current);
            }
            previous = current;
            dp = dc;
        }
    }
    polygon
}
pub(super) fn paint(
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    sample: Sample,
    color: Rgba,
    window: &mut Window,
) {
    if sample.alpha <= 0. {
        return;
    }
    let vertices = clipped_circle(bounds, corners, sample);
    if vertices.len() < 3 {
        return;
    }
    let mut path = PathBuilder::fill();
    for (i, p) in vertices.into_iter().enumerate() {
        let p = bounds.origin + point(px(p.x), px(p.y));
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, faded(color, sample.alpha));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::size;
    #[test]
    fn material_ripple_expands_from_origin_then_centers_and_fades_after_release() {
        let now = Instant::now();
        let mut ink = Ink::default();
        let id = ink.begin(point(0.1, 0.8), now, false);
        let start = ink.sample(now, false).0[0];
        assert!((start.center.x - 0.1).abs() < 0.001);
        assert_eq!(start.alpha, 0.);
        ink.finish(id, true, now);
        let expanded = ink.sample(now + Duration::from_millis(225), false).0[0];
        assert!((expanded.center.x - 0.5).abs() < 0.001);
        assert!((expanded.progress - 1.).abs() < 0.001);
        assert_eq!(expanded.alpha, 1.);
        assert!(
            (ink.sample(now + Duration::from_millis(300), false).0[0].alpha - 0.5).abs() < 0.001
        );
        assert!(
            ink.sample(now + Duration::from_millis(375), false)
                .0
                .is_empty()
        );
    }
    #[test]
    fn cancellation_preserves_current_alpha_and_quick_taps_overlap_boundedly() {
        let now = Instant::now();
        let mut ink = Ink::default();
        let id = ink.begin(point(0.5, 0.5), now, false);
        let release = now + Duration::from_millis(30);
        let alpha = ink.sample(release, false).0[0].alpha;
        ink.finish(id, false, release);
        assert_eq!(ink.sample(release, false).0[0].alpha, alpha);
        assert!(
            ink.sample(release + Duration::from_millis(75), false)
                .0
                .is_empty()
        );
        for _ in 0..20 {
            ink.begin(point(0.5, 0.5), now, false);
        }
        assert_eq!(ink.sample(now, false).0.len(), 8);
        assert!(ink.sample(now, true).0.is_empty());
        assert!(ink.begin(point(0., 0.), now, true).is_none());
    }
    #[test]
    fn hover_interruption_is_linear_and_does_not_restart_on_unrelated_frames() {
        let now = Instant::now();
        let mut h = Highlight::new();
        h.set(1., now, HOVER, false);
        assert_eq!(h.set(1., now + HOVER / 2, HOVER, false), (0.5, true));
        assert_eq!(h.set(0., now + HOVER / 2, HOVER, false), (0.5, true));
        assert_eq!(h.set(0., now + HOVER * 2, HOVER, false), (0., false));
    }
    #[test]
    fn ripple_geometry_never_leaks_beyond_rounded_or_rectangular_bounds() {
        let bounds = Bounds::new(point(px(20.), px(30.)), size(px(120.), px(32.)));
        for radius in [0., 7., 16.] {
            let vertices = clipped_circle(
                bounds,
                Corners::all(px(radius)),
                Sample {
                    center: point(0.5, 0.5),
                    progress: 1.,
                    alpha: 1.,
                },
            );
            assert!(!vertices.is_empty());
            for p in vertices {
                assert!(p.x >= -0.001 && p.x <= 120.001 && p.y >= -0.001 && p.y <= 32.001);
                let nearest = point(
                    p.x.clamp(radius, 120. - radius),
                    p.y.clamp(radius, 32. - radius),
                );
                assert!((p.x - nearest.x).hypot(p.y - nearest.y) <= radius + 0.001);
            }
        }
    }
}
