//! Titlebar tabs use the same Button input/focus engine, with their own chrome
//! motion. The leading icon has a fixed inset while the trailing label resizes.
use super::{
    Button, ButtonCustomVariant,
    motion::{self, ColorTransition, Transition},
};
use crate::typography::FontWeights;
use gpui_kit::{assets::IconName, component::Selectable, prelude::*, *};
use std::{rc::Rc, time::Instant};

type Click = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
struct State {
    width: Transition,
    foreground: ColorTransition,
    background: ColorTransition,
}

#[derive(IntoElement)]
pub struct Tab {
    id: ElementId,
    label: SharedString,
    icon: IconName,
    width: Option<f32>,
    selected: bool,
    bell_indicator: bool,
    collapsed: bool,
    reveal_label: bool,
    background: Rgba,
    foreground: Rgba,
    click: Option<Click>,
    close: Option<Click>,
}
impl Tab {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, icon: IconName) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon,
            width: None,
            selected: false,
            bell_indicator: false,
            collapsed: false,
            reveal_label: false,
            background: rgba(0),
            foreground: rgb(0xffffff),
            click: None,
            close: None,
        }
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self.reveal_label = true;
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn bell_indicator(mut self, visible: bool) -> Self {
        self.bell_indicator = visible;
        self
    }
    pub fn colors(mut self, background: Rgba, foreground: Rgba) -> Self {
        self.background = background;
        self.foreground = foreground;
        self
    }
    pub fn on_click(
        mut self,
        click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.click = Some(Rc::new(click));
        self
    }
    pub fn on_close(
        mut self,
        click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.close = Some(Rc::new(click));
        self
    }
}
impl RenderOnce for Tab {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut text_style = window.text_style();
        text_style.font_weight = FontWeights::MEDIUM;
        let run = text_style.to_run(self.label.len());
        let label_width = window
            .text_system()
            .shape_line(self.label.clone(), px(13.), &[run], None)
            .width();
        let target_width = if self.collapsed {
            30.
        } else if let Some(width) = self.width {
            width
        } else {
            (f32::from(label_width) + 39.).ceil().min(160.)
        };
        let state = window.use_keyed_state((self.id.clone(), "chrome-motion"), cx, |_, _| State {
            width: Transition::exponential(target_width),
            foreground: ColorTransition::new(self.foreground),
            background: ColorTransition::new(self.background),
        });
        let reduced = cx.reduce_motion();
        let now = Instant::now();
        let (width, foreground, background, moving) = state.update(cx, |state, _| {
            let (width, w) = state
                .width
                .set(target_width, now, motion::TAB_SIZE, reduced);
            let (foreground, f) =
                state
                    .foreground
                    .set(self.foreground, now, motion::TAB_COLOR, reduced);
            let (background, b) =
                state
                    .background
                    .set(self.background, now, motion::TAB_COLOR, reduced);
            (width, foreground, background, w || f || b)
        });
        if moving {
            window.request_animation_frame();
        }
        let hover = motion::mix(background, foreground, 0.08);
        let held = motion::mix(background, foreground, 0.16);
        let close_padding = if self.close.is_some() { 33. } else { 8. };
        let label = if self.reveal_label {
            // Keep the whole label at its natural width and constant opacity.
            // Only this viewport's right edge moves. There is no ellipsis,
            // shrinking text, opacity tween, or absolutely positioned icon.
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .child(
                    div()
                        .w(label_width.ceil())
                        .flex_shrink_0()
                        .whitespace_nowrap()
                        .child(self.label.clone()),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .text_ellipsis()
                .child(self.label.clone())
                .into_any_element()
        };
        let label = div()
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .child(label)
            .when(self.bell_indicator, |row| {
                row.child(
                    crate::assets::icon(IconName::Bell)
                        .size(px(12.))
                        .ml(px(5.))
                        .flex_shrink_0(),
                )
            });
        let mut button = Button::new(self.id.clone())
            .ink_ripple(false)
            .toolbar()
            .content_start()
            .content_gap(px(7.))
            .w_full()
            .px(px(8.))
            .pr(px(close_padding))
            .flex_shrink_0()
            .overflow_hidden()
            .role(Role::Tab)
            .selected(self.selected)
            .accessibility_label(self.label.clone())
            .text_size(px(13.))
            .icon(self.icon)
            .icon_size(px(14.))
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(background.into())
                    .foreground(foreground.into())
                    .hover(hover.into())
                    .highlight(held.into()),
            )
            .raw_tooltip(self.label.clone())
            .child(label);
        if let Some(click) = self.click {
            button = button.on_click(move |event, window, cx| click(event, window, cx));
        }
        // Sibling hit targets keep closing an inactive tab independent from
        // its selection button, including the initial mouse press.
        let mut tab = div()
            .id((self.id.clone(), "tab-container"))
            .relative()
            .w(px(width))
            .h(px(30.))
            .flex_shrink_0()
            .overflow_hidden()
            .child(button);
        if let Some(close) = self.close {
            tab = tab.child(
                div()
                    .absolute()
                    .right(px(8.))
                    .top(px(6.))
                    .size(px(18.))
                    .child(
                        Button::new((self.id, "close"))
                            .ink_ripple(false)
                            .size(px(18.))
                            .rounded(px(5.))
                            .flex_shrink_0()
                            .focusable(false)
                            .tab_stop(false)
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .foreground(foreground.into())
                                    .hover(foreground.opacity(0.12).into())
                                    .highlight(foreground.opacity(0.18).into()),
                            )
                            .icon(crate::assets::icon(IconName::X))
                            .icon_size(px(12.))
                            .tooltip("Close tab")
                            .on_click(move |event, window, cx| {
                                cx.stop_propagation();
                                close(event, window, cx);
                            }),
                    ),
            );
        }
        tab
    }
}
