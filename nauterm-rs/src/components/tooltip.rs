use crate::typography::FontWeights;
use gpui_kit::{component::ActiveTheme, prelude::*, *};
mod host;
pub(super) use host::{anchor_changed, hover};
pub(crate) use host::{hide_owner, init};
pub struct Tooltip {
    text: SharedString,
}
impl Tooltip {
    pub fn new(text: SharedString) -> Self {
        Self { text }
    }
}
impl Render for Tooltip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut style = window.text_style();
        style.font_family = cx.theme().font_family.clone();
        style.font_weight = FontWeights::REGULAR;
        style.font_style = FontStyle::Normal;
        let natural_width = self.text.lines().fold(0_f32, |width, line| {
            let text: SharedString = line.to_owned().into();
            let run = style.to_run(text.len());
            width.max(f32::from(
                window
                    .text_system()
                    .shape_line(text, px(12.), &[run], None)
                    .width(),
            ))
        });
        let maximum_width = (f32::from(window.viewport_size().width) - 16.).clamp(1., 480.);
        div()
            .id("product-tooltip")
            .role(Role::Tooltip)
            // GPUI measures tooltips with MinContent. An explicit natural width
            // keeps ordinary hints on one line while long text can still wrap.
            .w(px((natural_width.ceil() + 16.).min(maximum_width)))
            .max_w(px(maximum_width))
            .min_h(px(24.))
            .rounded(px(4.))
            .px(px(8.))
            .py(px(4.))
            .bg(rgb(0x616161).alpha(0.9))
            .text_color(rgb(0xffffff))
            .font_family(style.font_family)
            .font_weight(FontWeights::REGULAR)
            .not_italic()
            .text_size(px(12.))
            .line_height(px(18.))
            .whitespace_normal()
            .child(self.text.clone())
    }
}
