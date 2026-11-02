//! The connection pipeline follows workspace_connection_page.dart: stage travel
//! takes 300ms, inserting an intervention redistributes nodes over 320ms.
use crate::{
    connection_state::{ConnectionState, Intervention, Phase},
    palette::Palette,
};
use gpui_kit::{
    assets::IconName,
    base::animation::cubic_bezier,
    component::{Icon, Sizable},
    prelude::*,
    *,
};
use std::{
    f32::consts::PI,
    time::{Duration, Instant},
};

pub(super) const TRAVEL: Duration = Duration::from_millis(300);
const LAYOUT: Duration = Duration::from_millis(320);

#[derive(Clone)]
struct Stage {
    icons: Vec<IconName>,
    reached: usize,
    spinning: Option<usize>,
    moving: Option<usize>,
    traveling: Option<usize>,
    failed: bool,
}
impl Stage {
    fn new(
        state: &ConnectionState,
        protocol: IconName,
        destination: IconName,
        choosing: bool,
    ) -> Self {
        let mut icons = vec![protocol];
        icons.extend(
            state
                .interventions
                .iter()
                .map(|intervention| match intervention {
                    Intervention::HostKey => IconName::FingerprintPattern,
                    Intervention::Authentication => IconName::KeyRound,
                }),
        );
        icons.push(destination);
        let last = icons.len() - 1;
        let mut stage = Self {
            icons,
            reached: last - 1,
            spinning: None,
            moving: None,
            traveling: None,
            failed: matches!(state.phase, Phase::Failed | Phase::Exited),
        };
        if choosing {
            stage.reached = 0;
        } else {
            match state.phase {
                Phase::Connecting => stage.spinning = Some(stage.reached),
                Phase::Completing => {
                    stage.moving = Some(stage.reached);
                    stage.traveling = Some(last);
                }
                Phase::HostKey | Phase::Authentication => {
                    let active = if state.phase == Phase::HostKey {
                        Intervention::HostKey
                    } else {
                        Intervention::Authentication
                    };
                    let step = state
                        .interventions
                        .iter()
                        .position(|item| *item == active)
                        .map_or(1, |index| index + 1);
                    stage.reached = step - 1;
                    stage.moving = Some(step - 1);
                    stage.traveling = Some(step);
                }
                Phase::Failed | Phase::Exited => {}
            }
        }
        stage
    }
}

#[derive(Clone, Default)]
pub(super) struct Pipeline {
    stage: Option<Stage>,
    previous_icons: Vec<IconName>,
    layout_started: Option<Instant>,
    travel_started: Option<Instant>,
    travel_value: f32,
}
struct Node {
    icon: IconName,
    position: f32,
    active: bool,
    spinning: bool,
}
struct Frame {
    nodes: Vec<Node>,
    traveler: Option<Node>,
    progress: f32,
    moving: bool,
    failed: bool,
}
fn elapsed(start: Option<Instant>, now: Instant, duration: Duration) -> f32 {
    start.map_or(1., |start| {
        (now.saturating_duration_since(start).as_secs_f32() / duration.as_secs_f32()).min(1.)
    })
}
fn stage_curve(value: f32) -> f32 {
    cubic_bezier(0.55, 0., 0.30, 1.)(value)
}
impl Pipeline {
    fn sample(&mut self, next: Stage, now: Instant, reduced: bool) -> Frame {
        if let Some(old) = &self.stage {
            if old.icons != next.icons {
                self.previous_icons = old.icons.clone();
                self.layout_started = Some(now);
                // A newly inserted node travels with the layout. Do not also
                // animate a duplicate traveler along its moving segment.
                self.travel_started = None;
                self.travel_value = f32::from(next.moving.is_some());
            } else if old.moving != next.moving || old.traveling != next.traveling {
                let layout_active = elapsed(self.layout_started, now, LAYOUT) < 1.;
                if self.travel_started.is_some() {
                    self.travel_value = elapsed(self.travel_started, now, TRAVEL);
                }
                self.travel_started = (!layout_active && next.moving.is_some()).then_some(now);
                if self.travel_started.is_some() {
                    self.travel_value = 0.;
                }
            }
        } else {
            self.previous_icons = next.icons.clone();
            self.travel_started = next.moving.is_some().then_some(now);
        }
        let layout = if reduced {
            1.
        } else {
            elapsed(self.layout_started, now, LAYOUT)
        };
        let travel = if reduced {
            1.
        } else if self.travel_started.is_some() {
            elapsed(self.travel_started, now, TRAVEL)
        } else {
            self.travel_value
        };
        let node_position = |index: usize| {
            let destination = index as f32 / (next.icons.len() - 1).max(1) as f32;
            if layout >= 1. {
                return destination;
            }
            let origin = (0..=index).rev().find_map(|anchor| {
                self.previous_icons
                    .iter()
                    .position(|icon| *icon == next.icons[anchor])
            });
            origin.map_or(destination, |index| {
                let from = index as f32 / (self.previous_icons.len() - 1).max(1) as f32;
                from + (destination - from) * stage_curve(layout)
            })
        };
        let progress = if next.failed {
            1.
        } else if let Some(from) = next.moving {
            let from = node_position(from);
            let to = node_position(next.traveling.unwrap_or(next.reached));
            from + (to - from) * stage_curve(travel)
        } else {
            node_position(next.reached)
        };
        let nodes = next
            .icons
            .iter()
            .enumerate()
            .filter(|(index, _)| Some(*index) != next.traveling || layout < 1.)
            .map(|(index, icon)| {
                let spinning =
                    Some(index) == next.spinning || (Some(index) == next.traveling && layout < 1.);
                Node {
                    icon: *icon,
                    position: node_position(index),
                    active: next.failed || index <= next.reached || spinning,
                    spinning,
                }
            })
            .collect();
        let traveler = next.traveling.filter(|_| layout >= 1.).map(|index| Node {
            icon: next.icons[index],
            position: progress,
            active: true,
            spinning: true,
        });
        let frame = Frame {
            nodes,
            traveler,
            progress,
            moving: layout < 1. || (self.travel_started.is_some() && travel < 1.),
            failed: next.failed,
        };
        self.stage = Some(next);
        frame
    }
    pub fn render(
        &mut self,
        state: &ConnectionState,
        endpoints: (IconName, IconName),
        choosing: bool,
        dark: bool,
        window: &mut Window,
        cx: &App,
    ) -> AnyElement {
        let frame = self.sample(
            Stage::new(state, endpoints.0, endpoints.1, choosing),
            Instant::now(),
            cx.reduce_motion(),
        );
        if frame.moving {
            window.request_animation_frame();
        }
        let p = Palette::new(dark);
        let color = if frame.failed {
            rgb(0xef4444)
        } else {
            p.accent
        };
        let reduced = cx.reduce_motion();
        div()
            .relative()
            .w_full()
            .h(px(36.))
            .child(
                div()
                    .absolute()
                    .left(px(18.))
                    .right(px(18.))
                    .top(px(16.))
                    .h(px(4.))
                    .bg(p.border)
                    .child(div().h_full().w(relative(frame.progress)).bg(color)),
            )
            .children(frame.nodes.into_iter().enumerate().map(|(index, node)| {
                dot(node, ("connection-step", index), color, p.muted, reduced)
            }))
            .when_some(frame.traveler, |view, node| {
                view.child(dot(
                    node,
                    ("connection-traveler", 0),
                    color,
                    p.muted,
                    reduced,
                ))
            })
            .into_any_element()
    }
}
fn dot(
    node: Node,
    id: (&'static str, usize),
    active: Rgba,
    inactive: Rgba,
    reduced: bool,
) -> impl IntoElement {
    let color = if node.active { active } else { inactive };
    div()
        .id(id)
        .absolute()
        .left(relative(node.position))
        .ml(px(-36. * node.position))
        .size(px(36.))
        .flex()
        .items_center()
        .justify_center()
        .when(node.spinning, |view| view.child(ring(color, reduced)))
        .child(
            div()
                .size(px(24.))
                .rounded_full()
                .bg(color)
                .text_color(rgb(0xffffff))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(node.icon).with_size(px(14.))),
        )
}

// Match Material's independent 1333ms head/tail cycle and 2222ms rotation.
// Draw the changing arc itself, rather than rotate a static loader SVG.
fn ring(color: Rgba, reduced: bool) -> impl IntoElement {
    let element = div().absolute().inset_0();
    if reduced {
        return element.child(arc(color, 0.25 / 2222.)).into_any_element();
    }
    element
        .with_animation(
            "connection-progress-ring",
            Animation::new(Duration::from_millis(1333 * 2222))
                .repeat()
                .with_easing(|value| value),
            move |view, value| view.child(arc(color, value)),
        )
        .into_any_element()
}
fn arc(color: Rgba, value: f32) -> impl IntoElement {
    canvas(
        |bounds, _, _| bounds,
        move |_, bounds, window, _| {
            let cycle = (value * 2222.).fract();
            let fast = cubic_bezier(0.4, 0., 0.2, 1.);
            let head = fast((cycle * 2.).clamp(0., 1.));
            let tail = fast((cycle * 2. - 1.).clamp(0., 1.));
            let start =
                -PI / 2. + tail * 1.5 * PI + (value * 1333.).fract() * 2. * PI + cycle * PI / 2.;
            let sweep = ((head - tail) * 1.5 * PI).max(0.001);
            let radius = (bounds.size.width.min(bounds.size.height) - px(3.)) / 2.;
            let center = bounds.center();
            let position = |angle: f32| center + point(radius * angle.cos(), radius * angle.sin());
            let mut path = PathBuilder::stroke(px(3.));
            path.move_to(position(start));
            path.arc_to(
                point(radius, radius),
                px(0.),
                sweep > PI,
                true,
                position(start + sweep),
            );
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::{LAYOUT, Pipeline, Stage, TRAVEL};
    use crate::connection_state::{ConnectionState, Intervention, Phase};
    use gpui_kit::assets::IconName;
    use std::time::Instant;
    fn stage(state: &ConnectionState) -> Stage {
        Stage::new(state, IconName::Power, IconName::SquareTerminal, false)
    }
    #[test]
    fn inserted_interventions_follow_encounter_order_and_move_from_the_previous_anchor() {
        let now = Instant::now();
        let mut pipeline = Pipeline::default();
        let mut state = ConnectionState::default();
        pipeline.sample(stage(&state), now, false);
        state.phase = Phase::Authentication;
        state.remember_intervention(Intervention::Authentication);
        let start = pipeline.sample(stage(&state), now, false);
        assert_eq!(start.nodes[1].position, 0.);
        assert!(start.nodes[1].spinning);
        assert!(start.traveler.is_none());
        let middle = pipeline.sample(stage(&state), now + LAYOUT / 2, false);
        assert!(middle.nodes[1].position > 0. && middle.nodes[1].position < 0.5);
        let end = pipeline.sample(stage(&state), now + LAYOUT, false);
        assert_eq!(end.progress, 0.5);
        assert_eq!(end.traveler.unwrap().position, 0.5);
        state.remember_intervention(Intervention::HostKey);
        state.remember_intervention(Intervention::Authentication);
        state.phase = Phase::HostKey;
        let next = stage(&state);
        assert_eq!(
            next.icons,
            vec![
                IconName::Power,
                IconName::KeyRound,
                IconName::FingerprintPattern,
                IconName::SquareTerminal
            ]
        );
        assert_eq!(next.traveling, Some(2));
    }
    #[test]
    fn completion_travels_once_and_failure_or_reduced_motion_settles_immediately() {
        let now = Instant::now();
        let mut pipeline = Pipeline::default();
        let mut state = ConnectionState::default();
        pipeline.sample(stage(&state), now, false);
        state.phase = Phase::Completing;
        let start = pipeline.sample(stage(&state), now, false);
        assert_eq!(start.progress, 0.);
        assert_eq!(start.nodes.len(), 1);
        let middle = pipeline.sample(stage(&state), now + TRAVEL / 2, false);
        assert!(middle.progress > 0. && middle.progress < 1.);
        let end = pipeline.sample(stage(&state), now + TRAVEL, false);
        assert_eq!(end.progress, 1.);
        assert!(!end.moving);
        state.phase = Phase::Failed;
        let failed = pipeline.sample(stage(&state), now + TRAVEL, false);
        assert_eq!(failed.progress, 1.);
        assert!(
            failed
                .nodes
                .iter()
                .all(|node| node.active && !node.spinning)
        );
        assert!(failed.traveler.is_none());
        state.phase = Phase::Completing;
        let reduced = pipeline.sample(stage(&state), now + TRAVEL, true);
        assert_eq!(reduced.progress, 1.);
        assert!(!reduced.moving);
    }
}
