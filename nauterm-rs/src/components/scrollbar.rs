//! Product-owned overlay scrollbars. The wrapped element keeps its exact
//! layout, scroll handle and virtualization; no second scrolling viewport.
use gpui_kit::{component::ActiveTheme, prelude::*, *};
use std::cell::Cell;

/// Flutter's macOS embedder uses 40 logical pixels per non-precise wheel line,
/// rather than the text line height used by GPUI. Preserve the native viewport's
/// axis routing (including vertical-wheel input on a horizontal-only strip).
/// Pixel/trackpad events and other platforms retain their original movement.
fn wheel_offset(
    before: Point<Pixels>,
    native: Point<Pixels>,
    delta: ScrollDelta,
    macos: bool,
) -> Point<Pixels> {
    let ScrollDelta::Lines(lines) = delta else {
        return native;
    };
    if !macos || !lines.x.is_finite() || !lines.y.is_finite() {
        return native;
    }
    let routed = |start: Pixels, end: Pixels, primary: f32, fallback: f32| {
        if start == end {
            end
        } else {
            start + px(if primary == 0. { fallback } else { primary } * 40.)
        }
    };
    point(
        routed(before.x, native.x, lines.x, lines.y),
        routed(before.y, native.y, lines.y, lines.x),
    )
}

#[derive(Default)]
struct State {
    handle: ScrollHandle,
    drag: Option<(Axis, Pixels)>,
    hovered: Option<Axis>,
    viewport_hovered: bool,
    pointer_in_window: bool,
    _activation: Option<Subscription>,
}
impl State {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active()
                && (this.drag.is_some() || this.hovered.is_some() || this.viewport_hovered)
            {
                this.drag = None;
                this.hovered = None;
                this.viewport_hovered = false;
                cx.notify();
                window.refresh();
            }
        });
        Self {
            pointer_in_window: true,
            _activation: Some(activation),
            ..Default::default()
        }
    }
}

pub trait ScrollbarsExt: Sized + Element + StatefulInteractiveElement {
    fn scrollbars(self) -> ScrollArea<Self>;
    fn scrollbars_with(self, handle: &ScrollHandle) -> ScrollArea<Self>;
    fn scrollbars_hidden(self) -> ScrollArea<Self>;
}
impl<E: Element + StatefulInteractiveElement> ScrollbarsExt for E {
    fn scrollbars(self) -> ScrollArea<Self> {
        ScrollArea {
            content: self,
            handle: None,
            show_bars: true,
            hover_only: false,
        }
    }
    fn scrollbars_with(self, handle: &ScrollHandle) -> ScrollArea<Self> {
        ScrollArea {
            content: self,
            handle: Some(handle.clone()),
            show_bars: true,
            hover_only: false,
        }
    }
    fn scrollbars_hidden(self) -> ScrollArea<Self> {
        ScrollArea {
            content: self,
            handle: None,
            show_bars: false,
            hover_only: false,
        }
    }
}

#[derive(IntoElement)]
pub struct ScrollArea<E: Element + StatefulInteractiveElement> {
    content: E,
    handle: Option<ScrollHandle>,
    show_bars: bool,
    hover_only: bool,
}
impl<E: Element + StatefulInteractiveElement> ScrollArea<E> {
    /// Reveal on pointer hover, including while resting over a child option.
    pub fn hover_only(mut self) -> Self {
        self.hover_only = true;
        self
    }
}
impl<E: Element + StatefulInteractiveElement + Styled> Styled for ScrollArea<E> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.content.style()
    }
}
impl<E: Element + StatefulInteractiveElement + ParentElement> ParentElement for ScrollArea<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}
impl<E: Element + StatefulInteractiveElement> InteractiveElement for ScrollArea<E> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.content.interactivity()
    }
}
impl<E: Element + StatefulInteractiveElement> StatefulInteractiveElement for ScrollArea<E> {}
impl<E: Element + StatefulInteractiveElement> RenderOnce for ScrollArea<E> {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = Element::id(&self.content).expect("Scrollable div has an ID");
        let state = window.use_keyed_state((id, "scrollbars"), cx, State::new);
        let handle = self.handle.unwrap_or_else(|| state.read(cx).handle.clone());
        state.update(cx, |state, _| state.handle = handle.clone());
        let overflow = &self.content.interactivity().base_style.overflow;
        let axes = point(
            overflow.x == Some(Overflow::Scroll),
            overflow.y == Some(Overflow::Scroll),
        );
        ScrollElement {
            content: self.content.track_scroll(&handle),
            state,
            show_bars: self.show_bars,
            hover_only: self.hover_only,
            axes,
        }
    }
}

/// Decorate a virtual list without changing its virtualization or item layout.
pub fn uniform_scrollbar(
    id: impl Into<ElementId>,
    content: impl IntoElement,
    handle: &UniformListScrollHandle,
) -> UniformScrollbar {
    let handle = handle.0.borrow().base_handle.clone();
    UniformScrollbar {
        id: id.into(),
        content: content.into_any_element(),
        handle,
        hover_only: false,
    }
}
#[derive(IntoElement)]
pub struct UniformScrollbar {
    id: ElementId,
    content: AnyElement,
    handle: ScrollHandle,
    hover_only: bool,
}
impl UniformScrollbar {
    /// Keep menu tracks hidden until the pointer enters the option viewport.
    /// A grabbed thumb remains visible until its drag ends.
    pub fn hover_only(mut self) -> Self {
        self.hover_only = true;
        self
    }
}
impl RenderOnce for UniformScrollbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, State::new);
        state.update(cx, |state, _| state.handle = self.handle);
        ScrollElement {
            content: self.content,
            state,
            show_bars: true,
            hover_only: self.hover_only,
            axes: point(false, true),
        }
    }
}

struct Bar {
    axis: Axis,
    track: Bounds<Pixels>,
    thumb: Bounds<Pixels>,
    hitbox: Hitbox,
}
struct ScrollElement<E: Element> {
    content: E,
    state: Entity<State>,
    show_bars: bool,
    hover_only: bool,
    axes: Point<bool>,
}
fn scroll_maximum(handle: &ScrollHandle, axes: Point<bool>) -> Point<Pixels> {
    let maximum = handle.max_offset();
    point(
        if axes.x { maximum.x } else { px(0.) },
        if axes.y { maximum.y } else { px(0.) },
    )
}
impl<E: Element> IntoElement for ScrollElement<E> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl<E: Element> Element for ScrollElement<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = (E::PrepaintState, Vec<Bar>, Hitbox);
    fn id(&self) -> Option<ElementId> {
        self.content.id()
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.content.source_location()
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.content.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Insert before descendants so their overlays/occlusion remain above
        // this viewport. This hitbox owns no pointer interaction of its own.
        let viewport = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let inner = self
            .content
            .prepaint(id, inspector, bounds, layout, window, cx);
        let handle = &self.state.read(cx).handle;
        let maximum = scroll_maximum(handle, self.axes);
        let offset = handle.offset();
        let mut bars = vec![];
        let vertical = maximum.y > px(0.) && bounds.size.height >= px(28.);
        let horizontal = maximum.x > px(0.) && bounds.size.width >= px(28.);
        for axis in [Axis::Vertical, Axis::Horizontal] {
            if !self.show_bars {
                break;
            }
            let (extent, max, position) = match axis {
                Axis::Vertical => (bounds.size.height, maximum.y, -offset.y),
                Axis::Horizontal => (bounds.size.width, maximum.x, -offset.x),
            };
            if max <= px(0.) || extent < px(28.) {
                continue;
            }
            let corner = match axis {
                Axis::Vertical => horizontal,
                Axis::Horizontal => vertical,
            };
            let length = extent - px(4.) - if corner { px(10.) } else { px(0.) };
            let thumb_length = (length * (extent / (extent + max)))
                .max((length / 2.).min(px(24.)))
                .min(length);
            let start = (length - thumb_length) * (position / max).clamp(0., 1.);
            let (track, thumb) = match axis {
                Axis::Vertical => (
                    Bounds::new(
                        point(bounds.right() - px(10.), bounds.top() + px(2.)),
                        size(px(10.), length),
                    ),
                    Bounds::new(
                        point(bounds.right() - px(8.), bounds.top() + px(2.) + start),
                        size(px(6.), thumb_length),
                    ),
                ),
                Axis::Horizontal => (
                    Bounds::new(
                        point(bounds.left() + px(2.), bounds.bottom() - px(10.)),
                        size(length, px(10.)),
                    ),
                    Bounds::new(
                        point(bounds.left() + px(2.) + start, bounds.bottom() - px(8.)),
                        size(thumb_length, px(6.)),
                    ),
                ),
            };
            bars.push(Bar {
                axis,
                track,
                thumb,
                // Do not hover/click a row behind the thumb. Wheel input still
                // belongs to the existing viewport, including over the bar.
                hitbox: window.insert_hitbox(track, HitboxBehavior::BlockMouseExceptScroll),
            });
        }
        (inner, bars, viewport)
    }
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let viewport = prepaint.2.clone();
        let handle = self.state.read(cx).handle.clone();
        let trace = crate::scroll_trace::enabled()
            .then(|| format!("viewport {:?}", Element::id(&self.content)));
        let before = Cell::new(None);
        let axes = self.axes;
        // Register before painting the existing viewport. Bubble dispatch then
        // lets its native/virtual-list handler choose the axes before this
        // boundary normalizes macOS wheel distance and prevents a second scroll
        // in an ancestor. Descendants win; precise gestures stay native.
        window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
            let maximum = scroll_maximum(&handle, axes);
            if phase == DispatchPhase::Capture {
                before.set(Some(handle.offset()));
            }
            if phase == DispatchPhase::Bubble
                && viewport.should_handle_scroll(window)
                && (maximum.x > px(0.) || maximum.y > px(0.))
            {
                let offset = handle.offset();
                let start = before.take();
                let normalized = start
                    .map(|start| {
                        wheel_offset(start, offset, event.delta, cfg!(target_os = "macos"))
                    })
                    .unwrap_or(offset);
                let clamped = point(
                    normalized.x.clamp(-maximum.x, px(0.)),
                    normalized.y.clamp(-maximum.y, px(0.)),
                );
                if clamped != offset {
                    handle.set_offset(clamped);
                    window.refresh();
                }
                if let Some(scope) = &trace {
                    crate::scroll_trace::record(
                        scope,
                        event,
                        format_args!("offset={start:?} native={offset:?} applied={clamped:?}"),
                    );
                }
                cx.stop_propagation();
            }
        });
        self.content
            .paint(id, inspector, bounds, layout, &mut prepaint.0, window, cx);
        let viewport = prepaint.2.clone();
        // Aggregate pointer presence for the viewport, independently of GPUI's
        // keyboard modality and child BlockMouseExceptScroll hitboxes. A real
        // occluding overlay still removes this viewport from the hit test.
        let viewport_hovered = window.is_window_active()
            && window.is_window_hovered()
            && self.state.read(cx).pointer_in_window
            && viewport.should_handle_scroll(window);
        if self.hover_only {
            // Opening under a stationary pointer already paints a visible bar.
            // Remember that state so the first pointer exit also repaints it.
            self.state
                .update(cx, |state, _| state.viewport_hovered = viewport_hovered);
            let moved = self.state.clone();
            window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    let over = window.is_window_active()
                        && window.is_window_hovered()
                        && viewport.should_handle_scroll(window);
                    moved.update(cx, |state, _| {
                        state.pointer_in_window = true;
                        if state.viewport_hovered != over {
                            state.viewport_hovered = over;
                            window.refresh();
                        }
                    });
                }
            });
        }
        for bar in &prepaint.1 {
            let hovered = bar.hitbox.is_hovered(window);
            let active = self
                .state
                .read(cx)
                .drag
                .is_some_and(|(axis, _)| axis == bar.axis);
            let mut color = crate::palette::Palette::new(cx.theme().is_dark()).muted;
            color.a = if hovered || active { 0.85 } else { 0.45 };
            let mut quad = fill(bar.thumb, color);
            quad.corner_radii = Corners::all(px(3.));
            if !self.hover_only || viewport_hovered || active {
                window.paint_quad(quad);
            }
            window.set_cursor_style(CursorStyle::Arrow, &bar.hitbox);
            let down = self.state.clone();
            let axis = bar.axis;
            let track = bar.track;
            let thumb = bar.thumb;
            let hitbox = bar.hitbox.clone();
            let hover_hitbox = hitbox.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture
                    || event.button != MouseButton::Left
                    || !hitbox.is_hovered(window)
                    || !track.contains(&event.position)
                {
                    return;
                }
                down.update(cx, |state, _| {
                    let coord = if axis == Axis::Vertical {
                        event.position.y
                    } else {
                        event.position.x
                    };
                    let begin = if axis == Axis::Vertical {
                        thumb.top()
                    } else {
                        thumb.left()
                    };
                    let length = if axis == Axis::Vertical {
                        thumb.size.height
                    } else {
                        thumb.size.width
                    };
                    let grab = if thumb.contains(&event.position) {
                        coord - begin
                    } else {
                        length / 2.
                    };
                    state.drag = Some((axis, grab));
                    move_thumb(&state.handle, axis, track, length, coord, grab);
                });
                window.prevent_default();
                window.refresh();
                cx.stop_propagation();
            });
            let moved = self.state.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                moved.update(cx, |state, cx| {
                    let over = window.is_window_active()
                        && track.contains(&event.position)
                        && hover_hitbox.is_hovered(window);
                    if over && state.hovered != Some(axis) {
                        state.hovered = Some(axis);
                        window.refresh();
                    } else if !over && state.hovered == Some(axis) {
                        state.hovered = None;
                        window.refresh();
                    }
                    if let Some((drag_axis, grab)) = state.drag
                        && drag_axis == axis
                    {
                        if !event.dragging() {
                            state.drag = None;
                            window.refresh();
                            return;
                        }
                        let coord = if axis == Axis::Vertical {
                            event.position.y
                        } else {
                            event.position.x
                        };
                        let length = if axis == Axis::Vertical {
                            thumb.size.height
                        } else {
                            thumb.size.width
                        };
                        move_thumb(&state.handle, axis, track, length, coord, grab);
                        window.refresh();
                        cx.stop_propagation();
                    }
                });
            });
        }
        let up = self.state.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                up.update(cx, |state, cx| {
                    if state.drag.take().is_some() {
                        window.refresh();
                        cx.stop_propagation();
                    }
                });
            }
        });
        let exit = self.state.clone();
        window.on_mouse_event(move |_: &MouseExitEvent, phase, window, cx| {
            if phase == DispatchPhase::Capture {
                exit.update(cx, |state, _| {
                    state.pointer_in_window = false;
                    let was_hovered = state.viewport_hovered;
                    state.viewport_hovered = false;
                    if state.hovered.take().is_some() || was_hovered {
                        window.refresh();
                    }
                });
            }
        });
    }
}
fn move_thumb(
    handle: &ScrollHandle,
    axis: Axis,
    track: Bounds<Pixels>,
    length: Pixels,
    coord: Pixels,
    grab: Pixels,
) {
    let (start, total, max) = if axis == Axis::Vertical {
        (track.top(), track.size.height, handle.max_offset().y)
    } else {
        (track.left(), track.size.width, handle.max_offset().x)
    };
    let fraction = ((coord - start - grab) / (total - length).max(px(1.))).clamp(0., 1.);
    let mut offset = handle.offset();
    if axis == Axis::Vertical {
        offset.y = -max * fraction;
    } else {
        offset.x = -max * fraction;
    }
    handle.set_offset(offset);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn macos_wheel_distance_matches_flutter_without_font_height_or_rounding() {
        let before = point(px(0.), px(-100.));
        let delta = ScrollDelta::Lines(point(0., -1.));
        for native in [point(px(0.), px(-118.)), point(px(0.), px(-124.))] {
            assert_eq!(
                wheel_offset(before, native, delta, true),
                point(px(0.), px(-140.))
            );
            assert_eq!(wheel_offset(before, native, delta, false), native);
        }
        assert_eq!(
            wheel_offset(
                before,
                point(px(0.), px(-98.)),
                ScrollDelta::Lines(point(0., 0.125)),
                true
            ),
            point(px(0.), px(-95.)),
        );
    }

    #[test]
    fn precise_motion_and_native_axis_routing_are_preserved() {
        let before = point(px(-60.), px(-100.));
        let native = point(px(-60.), px(-109.5));
        assert_eq!(
            wheel_offset(
                before,
                native,
                ScrollDelta::Pixels(point(px(0.), px(-9.5))),
                true
            ),
            native,
        );
        let vertical_wheel = ScrollDelta::Lines(point(0., -1.));
        assert_eq!(
            wheel_offset(before, point(px(-84.), before.y), vertical_wheel, true),
            point(px(-100.), before.y),
        );
        assert_eq!(wheel_offset(before, before, vertical_wheel, true), before);
        assert_eq!(
            wheel_offset(
                before,
                point(before.x, px(-124.)),
                ScrollDelta::Lines(point(-0.5, -1.)),
                true
            ),
            point(before.x, px(-140.)),
        );
    }
}
