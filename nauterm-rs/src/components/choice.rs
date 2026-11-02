//! Product-owned choice controls, composed only from GPUI and our Button.
use super::{
    Button,
    motion::{self, Transition},
    settings::Button as SettingsButton,
};
use crate::typography::FontWeights;
use gpui_kit::{
    component::{ActiveTheme, Disableable, Icon, Sizable},
    prelude::*,
    *,
};
use std::{rc::Rc, time::Instant};
type Handler<V> = Rc<dyn Fn(&V, &mut Window, &mut App)>;

fn animated_check(id: ElementId, value: bool, window: &mut Window, cx: &mut App) -> f32 {
    let state = window.use_keyed_state(id, cx, |_, _| Transition::new(f32::from(value)));
    let reduced = cx.reduce_motion();
    let (value, moving) = state.update(cx, |s, _| {
        s.set(f32::from(value), Instant::now(), motion::SELECTION, reduced)
    });
    if moving {
        window.request_animation_frame();
    }
    value
}
pub(super) fn next_radio(key: &str, current: usize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match key {
        "left" | "up" => Some((current + len - 1) % len),
        "right" | "down" => Some((current + 1) % len),
        "home" => Some(0),
        "end" => Some(len - 1),
        _ => None,
    }
}

#[derive(IntoElement)]
pub struct RadioGroup<V: Clone + PartialEq + 'static> {
    id: ElementId,
    options: Vec<(V, SharedString)>,
    selected: V,
    on_change: Option<Handler<V>>,
}
impl<V: Clone + PartialEq + 'static> RadioGroup<V> {
    pub fn new(
        id: impl Into<ElementId>,
        options: impl IntoIterator<Item = (V, impl Into<SharedString>)>,
        selected: V,
    ) -> Self {
        Self {
            id: id.into(),
            options: options.into_iter().map(|(v, l)| (v, l.into())).collect(),
            selected,
            on_change: None,
        }
    }
    pub fn on_change(mut self, f: impl Fn(&V, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}
impl<V: Clone + PartialEq + 'static> RenderOnce for RadioGroup<V> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let count = self.options.len();
        let focused = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
            (0..count).map(|_| cx.focus_handle()).collect::<Vec<_>>()
        });
        let focuses = focused.update(cx, |v, cx| {
            v.resize_with(count, || cx.focus_handle());
            v.clone()
        });
        let values: Rc<Vec<_>> = Rc::new(self.options.iter().map(|(v, _)| v.clone()).collect());
        let selected = self
            .options
            .iter()
            .position(|(v, _)| v == &self.selected)
            .unwrap_or(0);
        let theme = cx.theme();
        let accent = theme.primary;
        let border = theme.input;
        div()
            .id(self.id.clone())
            .role(Role::RadioGroup)
            .w_full()
            .flex()
            .items_center()
            .children(
                self.options
                    .into_iter()
                    .enumerate()
                    .map(|(i, (value, label))| {
                        let checked = value == self.selected;
                        let child_id: ElementId =
                            (self.id.clone(), SharedString::from(i.to_string())).into();
                        let amount = animated_check(child_id.clone(), checked, window, cx);
                        let select = self.on_change.clone();
                        let key_select = select.clone();
                        let values = values.clone();
                        let key_focus = focuses.clone();
                        SettingsButton::new(child_id)
                            .ghost()
                            .content_start()
                            .flex_1()
                            .min_w_0()
                            .h(px(34.))
                            .px_0()
                            .role(Role::RadioButton)
                            .checked(checked)
                            .accessibility_label(label.clone())
                            .track_focus(&focuses[i])
                            .tab_stop(i == selected)
                            .child(
                                div()
                                    .w(px(20.))
                                    .h(px(18.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .flex_shrink_0()
                                    .child(
                                        div()
                                            .size(px(16.))
                                            .rounded_full()
                                            .border_1()
                                            .border_color(if checked { accent } else { border })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                div()
                                                    .size(px(8. * amount))
                                                    .rounded_full()
                                                    .bg(accent),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::NORMAL)
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(crate::localization::view(label)),
                            )
                            .on_key_down(move |event, window, cx| {
                                if !event.keystroke.modifiers.modified()
                                    && let Some(next) =
                                        next_radio(&event.keystroke.key, i, values.len())
                                {
                                    key_focus[next].focus(window, cx);
                                    if let Some(f) = &key_select {
                                        f(&values[next], window, cx);
                                    }
                                    cx.stop_propagation();
                                    window.prevent_default();
                                }
                            })
                            .on_click(move |_, window, cx| {
                                if !checked && let Some(f) = &select {
                                    f(&value, window, cx);
                                }
                            })
                    }),
            )
    }
}
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    label: SharedString,
    checked: bool,
    disabled: bool,
    on_change: Option<Handler<bool>>,
    description: Option<SharedString>,
    list_row: bool,
}
impl Checkbox {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, checked: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked,
            disabled: false,
            on_change: None,
            description: None,
            list_row: false,
        }
    }
    pub fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }
    pub fn list_row(mut self) -> Self {
        self.list_row = true;
        self
    }
}
impl Disableable for Checkbox {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = crate::localization::label(cx, &self.label);
        let description = self
            .description
            .map(|description| crate::localization::text(cx, &description));
        let amount = animated_check(self.id.clone(), self.checked, window, cx);
        let accent: Rgba = cx.theme().primary.into();
        let border = cx.theme().input;
        Button::new(self.id)
            .disabled(self.disabled)
            .ghost()
            .content_start()
            .px_0()
            .when(self.list_row, |button| {
                button
                    .w_full()
                    .h_auto()
                    .min_h(px(44.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(0.))
            })
            .role(Role::CheckBox)
            .checked(self.checked)
            .accessibility_label(label.clone())
            .child(
                div()
                    .size(px(15.))
                    .flex_shrink_0()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(border)
                    .bg(motion::mix(rgba(0), accent, amount))
                    .child(
                        Icon::new(gpui_kit::assets::IconName::Check)
                            .with_size(px(13.))
                            .text_color(rgb(0xffffff))
                            .opacity(amount),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .when(self.list_row, |view| view.flex_1())
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(2.))
                    .child(
                        div()
                            .max_w_full()
                            .text_size(px(13.))
                            .font_weight(if self.list_row {
                                FontWeights::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            })
                            .when(self.list_row, |view| view.truncate())
                            .child(label),
                    )
                    .when_some(description, |view, description| {
                        view.child(
                            div()
                                .max_w_full()
                                .text_size(px(11.))
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(description),
                        )
                    }),
            )
            .on_click(move |_, w, cx| {
                if let Some(f) = &self.on_change {
                    f(&!self.checked, w, cx);
                }
            })
    }
}
#[derive(IntoElement)]
pub struct Toggle<const SETTINGS: bool> {
    id: ElementId,
    label: SharedString,
    checked: bool,
    disabled: bool,
    on_change: Option<Handler<bool>>,
}
pub type Switch = Toggle<true>;
impl<const SETTINGS: bool> Toggle<SETTINGS> {
    pub fn new(id: impl Into<ElementId>, label: &'static str, checked: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked,
            disabled: false,
            on_change: None,
        }
    }
    pub fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}
impl<const SETTINGS: bool> Disableable for Toggle<SETTINGS> {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
impl<const SETTINGS: bool> RenderOnce for Toggle<SETTINGS> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = crate::localization::label(cx, &self.label);
        let amount = animated_check(self.id.clone(), self.checked, window, cx);
        let dark = cx.theme().is_dark();
        let track = motion::mix(
            if dark { rgb(0x28323d) } else { rgb(0xe5e7eb) },
            if dark { rgb(0x238c68) } else { rgb(0x10b981) },
            amount,
        );
        super::button::Button::<SETTINGS>::new(self.id)
            .ghost()
            .disabled(self.disabled)
            .w(px(42.))
            .h(px(25.))
            .p_0()
            .rounded_full()
            .role(Role::Switch)
            .checked(self.checked)
            .accessibility_label(label)
            .child(
                div()
                    .relative()
                    .w_full()
                    .h_full()
                    .rounded_full()
                    .bg(track)
                    .child(
                        div()
                            .absolute()
                            .left(px(2. + 17. * amount))
                            .top(px(2.))
                            .size(px(21.))
                            .rounded_full()
                            .bg(if dark {
                                motion::mix(rgb(0x9aa8b3), rgb(0xffffff), amount)
                            } else {
                                rgb(0xffffff)
                            }),
                    ),
            )
            .on_click(move |_, w, cx| {
                if let Some(f) = &self.on_change {
                    f(&!self.checked, w, cx);
                }
            })
    }
}
#[cfg(test)]
mod tests {
    use super::next_radio;
    #[test]
    fn radio_arrows_wrap_and_home_end_are_bounded() {
        assert_eq!(next_radio("left", 0, 3), Some(2));
        assert_eq!(next_radio("right", 2, 3), Some(0));
        assert_eq!(next_radio("home", 1, 3), Some(0));
        assert_eq!(next_radio("end", 0, 3), Some(2));
        assert_eq!(next_radio("right", 0, 0), None);
        assert_eq!(next_radio("a", 0, 3), None);
    }
}
