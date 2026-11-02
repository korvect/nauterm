//! Opaque RGB editor using our Input/Button and window-owned overlay layer.
//! HSV changes are drafts until dismissal, matching the reference color overlay.
use super::overlay::Overlay;
use super::settings::{Button, Input};
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::{component::Sizable, prelude::*, *};

#[derive(Clone, Copy, Debug)]
struct Hsv {
    h: f32,
    s: f32,
    v: f32,
}
impl Hsv {
    fn from_rgb(color: u32) -> Self {
        let [r, g, b] = [16, 8, 0].map(|shift| ((color >> shift) & 255) as f32 / 255.);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let hue = if delta == 0. {
            0.
        } else if max == r {
            ((g - b) / delta).rem_euclid(6.)
        } else if max == g {
            (b - r) / delta + 2.
        } else {
            (r - g) / delta + 4.
        };
        Self {
            h: hue / 6.,
            s: if max == 0. { 0. } else { delta / max },
            v: max,
        }
    }
    fn rgb(self) -> u32 {
        let hue = self.h.rem_euclid(1.) * 6.;
        let chroma = self.v * self.s;
        let x = chroma * (1. - (hue.rem_euclid(2.) - 1.).abs());
        let (r, g, b) = match hue as u32 {
            0 => (chroma, x, 0.),
            1 => (x, chroma, 0.),
            2 => (0., chroma, x),
            3 => (0., x, chroma),
            4 => (x, 0., chroma),
            _ => (chroma, 0., x),
        };
        let component =
            |value: f32| ((value + self.v - chroma).clamp(0., 1.) * 255.).round() as u32;
        (component(r) << 16) | (component(g) << 8) | component(b)
    }
}
fn parse_hex(value: &str) -> Option<u32> {
    let value = value.trim();
    let value = value.strip_prefix('#').unwrap_or(value);
    (value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
}

pub struct RgbChange {
    pub color: u32,
    pub revision: u64,
}
pub struct RgbState {
    committed: u32,
    draft: Hsv,
    input: Entity<InputState>,
    pub revision: u64,
    open: bool,
    dragging: Option<bool>,
    sv: Bounds<Pixels>,
    hue: Bounds<Pixels>,
    _input_events: Subscription,
}
impl EventEmitter<RgbChange> for RgbState {}
impl RgbState {
    pub fn new(color: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(format!("{color:06X}")));
        let events = cx.subscribe_in(
            &input,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if let Some(color) = parse_hex(&input.read(cx).value()) {
                    if color != this.draft.rgb() {
                        this.draft = Hsv::from_rgb(color);
                    }
                    if matches!(event, InputEvent::PressEnter { shift: false, .. }) {
                        this.close(cx);
                    }
                }
                let _ = window;
                cx.notify();
            },
        );
        Self {
            committed: color,
            draft: Hsv::from_rgb(color),
            input,
            revision: 0,
            open: false,
            dragging: None,
            sv: Bounds::default(),
            hue: Bounds::default(),
            _input_events: events,
        }
    }
    pub fn set_value(&mut self, color: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.revision = self.revision.wrapping_add(1);
        self.committed = color;
        self.draft = Hsv::from_rgb(color);
        self.open = false;
        self.dragging = None;
        self.sync_hex(window, cx);
        cx.notify();
    }
    fn sync_hex(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hex = format!("{:06X}", self.draft.rgb());
        if self.input.read(cx).value().as_str() != hex {
            self.input
                .update(cx, |input, cx| input.set_value(hex, window, cx));
        }
    }
    fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.dragging = None;
        let color = self.draft.rgb();
        if color != self.committed {
            self.committed = color;
            cx.emit(RgbChange {
                color,
                revision: self.revision,
            });
        }
        cx.notify();
    }
    fn drag(
        &mut self,
        position: Point<Pixels>,
        hue: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = if hue { self.hue } else { self.sv };
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
            return;
        }
        let x = ((position.x - bounds.left()) / bounds.size.width).clamp(0., 1.);
        if hue {
            self.draft.h = x;
        } else {
            self.draft.s = x;
            self.draft.v = 1. - ((position.y - bounds.top()) / bounds.size.height).clamp(0., 1.);
        }
        self.sync_hex(window, cx);
        cx.notify();
    }
    fn area(&self, hue: bool, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let mut surface = div()
            .id(if hue { "hue" } else { "sv" })
            .w(px(180.))
            .h(px(if hue { 14. } else { 120. }))
            .relative()
            .overflow_hidden()
            .rounded(px(4.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.dragging = Some(hue);
                    this.drag(event.position, hue, window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if let Some(hue) = this.dragging {
                    this.drag(event.position, hue, window, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    if let Some(hue) = this.dragging.take() {
                        this.drag(event.position, hue, window, cx);
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    if let Some(hue) = this.dragging.take() {
                        this.drag(event.position, hue, window, cx);
                    }
                }),
            );
        if hue {
            surface = surface
                .flex()
                .children((0..6).map(|index| {
                    div().flex_1().h_full().bg(linear_gradient(
                        90.,
                        linear_color_stop(
                            rgb(Hsv {
                                h: index as f32 / 6.,
                                s: 1.,
                                v: 1.,
                            }
                            .rgb()),
                            0.,
                        ),
                        linear_color_stop(
                            rgb(Hsv {
                                h: (index + 1) as f32 / 6.,
                                s: 1.,
                                v: 1.,
                            }
                            .rgb()),
                            1.,
                        ),
                    ))
                }))
                .child(
                    div()
                        .absolute()
                        .left(px(self.draft.h * 176.))
                        .top_0()
                        .w(px(4.))
                        .h_full()
                        .border_1()
                        .border_color(rgb(0xffffff))
                        .rounded(px(2.)),
                );
        } else {
            surface = surface
                .bg(rgb(Hsv {
                    h: self.draft.h,
                    s: 1.,
                    v: 1.,
                }
                .rgb()))
                .child(div().absolute().size_full().bg(linear_gradient(
                    90.,
                    linear_color_stop(rgba(0xffffffff), 0.),
                    linear_color_stop(rgba(0xffffff00), 1.),
                )))
                .child(div().absolute().size_full().bg(linear_gradient(
                    180.,
                    linear_color_stop(rgba(0x00000000), 0.),
                    linear_color_stop(rgba(0x000000ff), 1.),
                )))
                .child(
                    div()
                        .absolute()
                        .left(px((self.draft.s * 180. - 4.).clamp(0., 172.)))
                        .top(px(((1. - self.draft.v) * 120. - 4.).clamp(0., 112.)))
                        .size(px(8.))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0xffffff))
                        .shadow_sm(),
                );
        }
        surface
            .child(
                canvas(
                    move |bounds, _, cx| {
                        view.update(cx, |this, _| {
                            if hue {
                                this.hue = bounds;
                            } else {
                                this.sv = bounds;
                            }
                        })
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
    }
}
impl Render for RgbState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .w(px(180.))
            .child(self.area(false, cx))
            .child(self.area(true, cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .size(px(18.))
                            .rounded(px(4.))
                            .bg(rgb(self.draft.rgb())),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.input)
                                .small()
                                .aria_label(crate::localization::text(cx, "Hex color"))
                                .invalid(parse_hex(&self.input.read(cx).value()).is_none()),
                        ),
                    ),
            )
    }
}

#[derive(IntoElement)]
pub struct RgbPicker {
    state: Entity<RgbState>,
    label: SharedString,
}
impl RgbPicker {
    pub fn new(state: &Entity<RgbState>, label: impl Into<SharedString>) -> Self {
        Self {
            state: state.clone(),
            label: label.into(),
        }
    }
}
impl RenderOnce for RgbPicker {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let color = state.committed;
        let open = state.open;
        let editor = self.state.clone();
        let changes = self.state.clone();
        Overlay::new(("rgb-picker", self.state.entity_id()))
            .open(open)
            .w(px(202.))
            .on_open_change(move |open, window, cx| {
                changes.update(cx, |state, cx| {
                    if *open {
                        state.open = true;
                        state.draft = Hsv::from_rgb(state.committed);
                        state.sync_hex(window, cx);
                        cx.notify();
                    } else {
                        state.close(cx);
                    }
                });
            })
            .trigger(
                Button::new(("rgb-swatch", self.state.entity_id()))
                    .ghost()
                    .w(px(22.))
                    .h(px(22.))
                    .p_0()
                    .rounded_full()
                    .border_1()
                    .border_color(rgba(0x8ca0a655))
                    .bg(rgb(color))
                    .tooltip(format!("#{color:06X}"))
                    .accessibility_label(self.label),
            )
            .content(move |_, _| editor.clone().into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::{Hsv, parse_hex};
    #[test]
    fn hsv_round_trip_and_hex_validation_preserve_rgb() {
        for color in [
            0, 0xffffff, 0xff0000, 0x00ff00, 0x0000ff, 0xffff00, 0xff00ff, 0x00ffff, 0x343842,
            0x010101, 0x123456, 0xd95f56,
        ] {
            assert_eq!(Hsv::from_rgb(color).rgb(), color);
        }
        assert_eq!(parse_hex(" #00ffAA "), Some(0x00ffaa));
        for value in [
            "",
            "#123",
            "#gggggg",
            "#ff123456",
            "颜色颜色",
            "1234567",
            "##123456",
        ] {
            assert!(parse_hex(value).is_none());
        }
    }
}
