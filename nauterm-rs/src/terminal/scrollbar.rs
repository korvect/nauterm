use super::*;

fn thumb_length(track: Pixels, extent: f32, history: f32) -> Pixels {
    (track * (extent / (extent + history).max(1.)))
        .max((track / 2.).min(px(24.)))
        .min(track)
}

impl Terminal {
    fn scroll_to_pointer(&mut self, position: Pixels, grab: Pixels, cx: &mut Context<Self>) {
        let track = (self.viewport_bounds.size.height - px(4.)).max(px(1.));
        let extent = (self.viewport_bounds.size.height / self.cell_height.max(px(1.))).max(1.);
        let history = self.frame.history_lines as f32;
        let thumb = thumb_length(track, extent, history);
        let fraction = ((position - self.viewport_bounds.top() - px(2.) - grab)
            / (track - thumb).max(px(1.)))
        .clamp(0., 1.);
        let offset = (history * (1. - fraction)).round() as i64;
        let delta = (offset - i64::from(self.frame.display_offset))
            .clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        if delta != 0 {
            self.session.scroll(delta);
            self.refresh(cx);
        }
    }
    pub(super) fn scrollbar(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.appearance.scrollbar || self.frame.history_lines == 0 {
            return div().into_any_element();
        }
        let view = cx.entity();
        let events = view.clone();
        div()
            .id("terminal-scrollbar")
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(px(10.))
            .cursor_default()
            .block_mouse_except_scroll()
            .on_hover(cx.listener(|_, _, window, cx| {
                window.refresh();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    let track = (this.viewport_bounds.size.height - px(4.)).max(px(1.));
                    let extent =
                        (this.viewport_bounds.size.height / this.cell_height.max(px(1.))).max(1.);
                    let history = this.frame.history_lines as f32;
                    let thumb = thumb_length(track, extent, history);
                    let start = this.viewport_bounds.top()
                        + px(2.)
                        + (track - thumb)
                            * (1. - this.frame.display_offset as f32 / history.max(1.));
                    let grab = if event.position.y >= start && event.position.y <= start + thumb {
                        event.position.y - start
                    } else {
                        thumb / 2.
                    };
                    this.selecting = None;
                    this.selection_drag = None;
                    this.scroll_drag = Some(grab);
                    this.scroll_to_pointer(event.position.y, grab, cx);
                    window.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        let this = view.read(cx);
                        let extent = (this.viewport_bounds.size.height
                            / this.cell_height.max(px(1.)))
                        .max(1.);
                        let history = this.frame.history_lines as f32;
                        let track = (bounds.size.height - px(4.)).max(px(1.));
                        let thumb = thumb_length(track, extent, history);
                        let start = (track - thumb)
                            * (1. - this.frame.display_offset as f32 / history.max(1.))
                                .clamp(0., 1.);
                        let mut color = rgb(this.appearance.palette().base.foreground);
                        color.a = if this.scroll_drag.is_some()
                            || bounds.contains(&window.mouse_position())
                        {
                            0.65
                        } else {
                            0.30
                        };
                        let mut quad = fill(
                            Bounds::new(
                                point(bounds.left() + px(2.), bounds.top() + px(2.) + start),
                                size(px(6.), thumb),
                            ),
                            color,
                        );
                        quad.corner_radii = Corners::all(px(3.));
                        window.paint_quad(quad);
                        let moved = events.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                moved.update(cx, |this, cx| {
                                    if let Some(grab) = this.scroll_drag {
                                        if event.dragging() {
                                            this.scroll_to_pointer(event.position.y, grab, cx);
                                            cx.stop_propagation();
                                        } else {
                                            this.scroll_drag = None;
                                            cx.notify();
                                        }
                                    }
                                });
                            }
                        });
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture && event.button == MouseButton::Left
                            {
                                events.update(cx, |this, cx| {
                                    if this.scroll_drag.take().is_some() {
                                        cx.stop_propagation();
                                        cx.notify();
                                    }
                                });
                            }
                        });
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }
}
