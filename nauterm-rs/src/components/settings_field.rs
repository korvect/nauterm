//! Settings labels are external, never the workspace's animated outline label.
use crate::{palette::Palette, typography::FontWeights};
use gpui_kit::{component::ActiveTheme, prelude::*, *};

#[derive(IntoElement)]
pub struct Field {
    label: SharedString,
    control: AnyElement,
    focus: FocusHandle,
}
impl Field {
    pub fn new(
        label: impl Into<SharedString>,
        control: impl IntoElement,
        focus: FocusHandle,
    ) -> Self {
        Self {
            label: label.into(),
            control: control.into_any_element(),
            focus,
        }
    }
}
impl RenderOnce for Field {
    fn render(mut self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        self.label = crate::localization::label(cx, &self.label);
        let p = Palette::settings(cx.theme().is_dark());
        div()
            .id((ElementId::from(&self.focus), "settings-field"))
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .id((ElementId::from(&self.focus), "settings-label"))
                    .role(Role::Label)
                    .aria_label(self.label.clone())
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .font_weight(FontWeights::MEDIUM)
                    .text_color(p.text)
                    .child(self.label)
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        self.focus.focus(window, cx);
                        window.prevent_default();
                    }),
            )
            .child(self.control)
    }
}
