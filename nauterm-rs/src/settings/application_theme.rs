use super::*;
use gpui_kit::component::Selectable;

fn miniature(dark: bool) -> AnyElement {
    let background = rgb(if dark { 0x242424 } else { 0xfafcfd });
    let detail = rgb(if dark { 0x424242 } else { 0xe7edf0 });
    let content = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(div().w(relative(0.65)).h(px(4.)).rounded(px(2.)).bg(detail))
        .child(div().w_full().h(px(4.)).rounded(px(2.)).bg(detail))
        .child(div().flex().gap(px(4.)).children((0..2).map(|_| {
            div()
                .flex_1()
                .h(px(18.))
                .border_1()
                .border_color(detail)
                .rounded(px(2.))
        })));
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .p(px(7.))
        .bg(background)
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(
            div().flex().gap(px(3.)).children(
                [0xff6058, 0xffbd2e, 0x28c840]
                    .map(|color| div().size(px(5.)).rounded_full().bg(rgb(color))),
            ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .gap(px(5.))
                .child(div().w(px(13.)).h_full().rounded(px(2.)).bg(detail))
                .child(content),
        )
        .into_any_element()
}

impl Settings {
    pub(super) fn theme_choices(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::settings(self.values.dark);
        div()
            .w_full()
            .max_w(px(1000.))
            .flex()
            .gap(px(12.))
            .children(
                [
                    (AppTheme::Light, "Light"),
                    (AppTheme::System, "System"),
                    (AppTheme::Dark, "Dark"),
                ]
                .map(|(theme, label)| {
                    let selected = self.values.theme == theme;
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(7.))
                        .child(
                            Button::new(label)
                                .ghost()
                                .selected(selected)
                                .localized_accessibility_label(format!("{label} application theme"))
                                .w_full()
                                .h_auto()
                                .aspect_ratio(1.72)
                                .p(px(if selected { 5. } else { 6. }))
                                .rounded(px(8.))
                                .border_1()
                                .border_color(if selected { p.accent } else { p.border })
                                .when(selected, |v| v.border_2())
                                .child(
                                    div()
                                        .size_full()
                                        .relative()
                                        .rounded(px(4.))
                                        .overflow_hidden()
                                        .flex()
                                        .when(theme != AppTheme::System, |v| {
                                            v.child(miniature(theme == AppTheme::Dark))
                                        })
                                        .when(theme == AppTheme::System, |v| {
                                            v.child(miniature(false)).child(miniature(true))
                                        })
                                        .when(selected, |v| {
                                            v.child(
                                                div()
                                                    .absolute()
                                                    .top(px(2.))
                                                    .right(px(2.))
                                                    .size(px(18.))
                                                    .rounded_full()
                                                    .bg(p.surface)
                                                    .child(
                                                        Icon::new(IconName::CircleCheck)
                                                            .with_size(px(18.))
                                                            .text_color(p.accent),
                                                    ),
                                            )
                                        }),
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.values.theme = theme;
                                    this.values.dark = theme.dark(window.appearance());
                                    this.changed(window, cx);
                                })),
                        )
                        .child(
                            SettingsText::Caption
                                .apply(div())
                                .text_center()
                                .text_color(if selected { p.accent } else { p.text })
                                .child(self.text(label)),
                        )
                }),
            )
            .into_any_element()
    }
}
