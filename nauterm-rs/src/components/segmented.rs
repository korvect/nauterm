//! Workspace segmented choices share the ordinary Button interaction policy.
use super::{Button, ButtonCustomVariant, motion};
use crate::{palette::Palette, typography::FontWeights};
use gpui_kit::{
    assets::IconName,
    component::{ActiveTheme, Disableable, Selectable, Sizable},
    prelude::*,
    *,
};
use std::{rc::Rc, time::Instant};

type ChangeHandler<V> = Rc<dyn Fn(&V, &mut Window, &mut App)>;

struct Segment<V> {
    value: V,
    label: SharedString,
    icon: Option<IconName>,
}

#[derive(IntoElement)]
pub struct Segmented<V: Clone + PartialEq + 'static> {
    id: ElementId,
    value: V,
    options: Vec<Segment<V>>,
    sliding: bool,
    disabled: bool,
    height: f32,
    change: Option<ChangeHandler<V>>,
}
impl<V: Clone + PartialEq + 'static> Segmented<V> {
    pub fn new(
        id: impl Into<ElementId>,
        value: V,
        options: impl IntoIterator<Item = (V, SharedString)>,
    ) -> Self {
        Self {
            id: id.into(),
            value,
            options: options
                .into_iter()
                .map(|(value, label)| Segment {
                    value,
                    label,
                    icon: None,
                })
                .collect(),
            sliding: false,
            disabled: false,
            height: 32.,
            change: None,
        }
    }
    /// A raised card slides beneath transparent segments, as in auth tabs.
    pub fn sliding(mut self) -> Self {
        self.sliding = true;
        self
    }
    pub fn icon(mut self, value: V, icon: IconName) -> Self {
        if let Some(option) = self.options.iter_mut().find(|option| option.value == value) {
            option.icon = Some(icon);
        }
        self
    }
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
    pub fn on_change(mut self, change: impl Fn(&V, &mut Window, &mut App) + 'static) -> Self {
        self.change = Some(Rc::new(change));
        self
    }
}
impl<V: Clone + PartialEq + 'static> Disableable for Segmented<V> {
    fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
}
impl<V: Clone + PartialEq + 'static> RenderOnce for Segmented<V> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let dark = cx.theme().is_dark();
        let p = Palette::new(dark);
        let count = self.options.len();
        let focused = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
            (0..count).map(|_| cx.focus_handle()).collect::<Vec<_>>()
        });
        let focuses = focused.update(cx, |focuses, cx| {
            focuses.resize_with(count, || cx.focus_handle());
            focuses.clone()
        });
        let values = Rc::new(
            self.options
                .iter()
                .map(|option| option.value.clone())
                .collect::<Vec<_>>(),
        );
        let selected_index = values.iter().position(|value| *value == self.value);
        let active = selected_index.unwrap_or(0);
        let indicator = if selected_index.is_some() {
            let motion = window.use_keyed_state((self.id.clone(), "indicator"), cx, |_, _| {
                motion::Transition::new(active as f32)
            });
            let reduced = cx.reduce_motion();
            let (position, moving) = motion.update(cx, |motion, _| {
                motion.set(active as f32, Instant::now(), motion::SEGMENTED, reduced)
            });
            if moving {
                window.request_animation_frame();
            }
            Some(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(position / count as f32))
                    .w(relative(1. / count as f32))
                    .rounded(px(6.))
                    .bg(if self.sliding { p.surface } else { p.accent })
                    .when(!self.sliding && self.disabled, |view| view.opacity(0.45))
                    .when(self.sliding, |view| {
                        view.border_1()
                            .border_color(if dark { p.border } else { rgb(0xd3dfe2) })
                            .shadow(vec![BoxShadow {
                                color: black().opacity(if dark { 0.18 } else { 0.07 }),
                                offset: point(px(0.), px(1.)),
                                blur_radius: px(3.),
                                spread_radius: px(0.),
                                inset: false,
                            }])
                    }),
            )
        } else {
            None
        };
        div()
            .id(self.id.clone())
            .role(Role::RadioGroup)
            .h(px(self.height))
            .flex_shrink_0()
            .min_w_0()
            .p(px(1.))
            .when(!self.sliding, |view| view.border_1().border_color(p.border))
            .rounded(px(if self.sliding { 8. } else { 7. }))
            .bg(if self.sliding {
                if dark { p.hover } else { p.border }
            } else {
                p.background
            })
            .child(
                div()
                    .relative()
                    .size_full()
                    .min_w_0()
                    .flex()
                    .when_some(indicator, |view, indicator| view.child(indicator))
                    .children(self.options.into_iter().enumerate().map(
                        |(index, Segment { value, label, icon })| {
                            let selected = self.value == value;
                            let background = rgba(0);
                            let color_motion = window.use_keyed_state(
                                (
                                    self.id.clone(),
                                    SharedString::from(format!("label-{index}")),
                                ),
                                cx,
                                |_, _| motion::Transition::new(f32::from(selected)),
                            );
                            let reduced = cx.reduce_motion();
                            let (amount, moving) = color_motion.update(cx, |motion, _| {
                                motion.set(
                                    f32::from(selected),
                                    Instant::now(),
                                    motion::SEGMENTED,
                                    reduced,
                                )
                            });
                            if moving {
                                window.request_animation_frame();
                            }
                            let foreground = motion::mix(
                                p.muted,
                                if self.sliding {
                                    p.accent
                                } else {
                                    rgb(0xffffff)
                                },
                                amount,
                            );
                            let change = self.change.clone();
                            let key_change = change.clone();
                            let values = values.clone();
                            let key_focus = focuses.clone();
                            let disabled = self.disabled;
                            Button::new((self.id.clone(), SharedString::from(index.to_string())))
                                .small()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .px(px(2.))
                                .rounded(px(6.))
                                .text_size(px(12.))
                                .font_weight(if self.sliding {
                                    FontWeights::SEMIBOLD
                                } else {
                                    FontWeights::MEDIUM
                                })
                                .label(label)
                                .when_some(icon, |button, icon| {
                                    button.icon(icon).icon_size(px(14.))
                                })
                                .selected(selected)
                                .disabled(self.disabled)
                                .role(Role::RadioButton)
                                .checked(selected)
                                .track_focus(&focuses[index])
                                .tab_stop(index == active)
                                .custom(if self.sliding {
                                    ButtonCustomVariant::new(cx)
                                        .color(rgba(0).into())
                                        .foreground(foreground.into())
                                        .hover(
                                            if selected {
                                                rgba(0)
                                            } else {
                                                p.accent.alpha(if dark { 0.10 } else { 0.055 })
                                            }
                                            .into(),
                                        )
                                        .active(
                                            p.accent.alpha(if dark { 0.18 } else { 0.12 }).into(),
                                        )
                                        .highlight(rgba(0).into())
                                } else {
                                    ButtonCustomVariant::new(cx)
                                        .color(background.into())
                                        .foreground(foreground.into())
                                        .hover(if selected { rgba(0) } else { p.hover }.into())
                                        .active(
                                            if selected {
                                                rgb(0xffffff).alpha(0.18)
                                            } else {
                                                p.accent.alpha(0.12)
                                            }
                                            .into(),
                                        )
                                        .highlight(rgba(0).into())
                                })
                                .bg(background)
                                .on_key_down(move |event, window, cx| {
                                    if !disabled
                                        && !event.keystroke.modifiers.modified()
                                        && let Some(next) = super::choice::next_radio(
                                            &event.keystroke.key,
                                            index,
                                            values.len(),
                                        )
                                    {
                                        key_focus[next].focus(window, cx);
                                        if let Some(change) = &key_change {
                                            change(&values[next], window, cx);
                                        }
                                        cx.stop_propagation();
                                    }
                                })
                                .on_click(move |_, window, cx| {
                                    if let Some(change) = &change {
                                        change(&value, window, cx);
                                    }
                                })
                        },
                    )),
            )
    }
}
