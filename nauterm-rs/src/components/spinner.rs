//! Our loading control uses the reference's vector asset, not Kit Spinner.
use gpui_kit::{
    assets::IconName,
    component::{Sizable, Size},
    prelude::*,
    *,
};
use std::time::Duration;
#[derive(IntoElement)]
pub struct Spinner {
    size: Pixels,
    icon: IconName,
    color: Option<Hsla>,
}
impl Spinner {
    pub fn new() -> Self {
        Self {
            size: px(16.),
            icon: IconName::Loader,
            color: None,
        }
    }
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}
impl Sizable for Spinner {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = match size.into() {
            Size::XSmall => px(12.),
            Size::Small => px(14.),
            Size::Medium => px(16.),
            Size::Large => px(24.),
            Size::Size(v) => v,
        };
        self
    }
}
impl RenderOnce for Spinner {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        svg()
            .path(self.icon.path())
            .size(self.size)
            .flex_shrink_0()
            .text_color(self.color.unwrap_or(window.text_style().color))
            .with_animation(
                "spinner-rotation",
                Animation::new(Duration::from_millis(800))
                    .repeat_synced()
                    .with_easing(ease_in_out),
                |icon, t| icon.with_transformation(Transformation::rotate(percentage(t))),
            )
    }
}
