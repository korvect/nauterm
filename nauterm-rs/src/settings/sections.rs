//! The Settings section rhythm is shared across pages, including About.
use super::*;

pub(super) fn stack() -> Div {
    div().w_full().min_w_0().flex().flex_col().gap(px(30.))
}

pub(super) fn frame(
    title: &'static str,
    caption: SharedString,
    icon: IconName,
    palette: Palette,
    content: impl IntoElement,
    action: Option<AnyElement>,
) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(8.))
                .pb(px(9.))
                .border_b_1()
                .border_color(palette.border)
                .child(
                    Icon::new(icon)
                        .with_size(px(20.))
                        .text_color(palette.accent),
                )
                .child(
                    SettingsText::Section
                        .apply(div().id(SharedString::from(format!(
                            "settings-section-caption-{title}"
                        ))))
                        .role(Role::Heading)
                        .aria_label(caption.clone())
                        .text_color(rgb(0x6b7280))
                        .child(TrackedText::new(caption, px(0.8))),
                )
                .when_some(action, |header, action| {
                    header.child(div().flex_1()).child(action)
                }),
        )
        .child(content)
        .into_any_element()
}
