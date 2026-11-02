//! One passive terminal-theme renderer for tools, host drawers and settings.
//! Actions/selection ownership stay in each caller's own workspace/settings skin.
use crate::terminal_appearance::{Palette, blend, platform_family};
use crate::typography::FontWeights;
use gpui_kit::{prelude::*, *};

#[derive(IntoElement)]
pub struct ThemePreview {
    title: SharedString,
    colors: Palette,
    compact: bool,
    selected: bool,
}
impl ThemePreview {
    // 2px border + 20px padding + 16px title + 9px gap + 34px sample
    // + 8px gap + 16px ANSI swatches, matching the reference's natural height.
    pub const COMPACT_HEIGHT: f32 = 105.;

    pub fn new(title: impl Into<SharedString>, colors: Palette) -> Self {
        Self {
            title: title.into(),
            colors,
            compact: false,
            selected: false,
        }
    }
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}
impl RenderOnce for ThemePreview {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self {
            title,
            colors,
            compact,
            selected,
        } = self;
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .relative()
            .px(px(12.))
            .pt(px(if compact { 10. } else { 11. }))
            .pb(px(if compact { 10. } else { 12. }))
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(blend(
                colors.base.background,
                if selected {
                    colors.accent
                } else {
                    colors.base.foreground
                },
                if selected { 0.72 } else { 0.13 },
            )))
            .bg(rgb(colors.base.background))
            .text_color(rgb(colors.base.foreground))
            .font_weight(FontWeights::MEDIUM)
            .shadow(vec![gpui_kit::base::box_shadow(
                px(0.),
                px(5.),
                px(if selected { 16. } else { 10. }),
                px(0.),
                rgb(0)
                    .alpha(if colors.dark {
                        0.16
                    } else if selected {
                        0.08
                    } else {
                        0.045
                    })
                    .into(),
            )])
            .child(
                canvas(
                    move |bounds, window, cx| {
                        // Use actual cell width, including when a drawer/panel is resized.
                        let sample_width = (f32::from(bounds.size.width) - 22.).max(0.);
                        let selection = sample_width >= 64.;
                        let cursor = sample_width >= if selection { 28. } else { 12. };
                        let swatch_gap = (f32::from(bounds.size.width) / 40.).clamp(0., 4.);
                        let swatch_width =
                            ((f32::from(bounds.size.width) - swatch_gap * 7.) / 8.).clamp(0., 12.);
                        let mut content = div()
                            .size_full()
                            .min_w_0()
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .w_full()
                                    .h(px(if compact { 16. } else { 31. }))
                                    .flex_shrink_0()
                                    .pr(px(14.))
                                    .overflow_hidden()
                                    .text_size(px(if compact { 13. } else { 12. }))
                                    .line_height(px(if compact { 14.04 } else { 12.96 }))
                                    .font_weight(FontWeights::SEMIBOLD)
                                    .when(compact, |v| v.whitespace_nowrap().text_ellipsis())
                                    .child(title),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(if compact { 34. } else { 38. }))
                                    .flex_shrink_0()
                                    .mt(px(9.))
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(rgb(blend(
                                        colors.base.background,
                                        colors.base.foreground,
                                        0.08,
                                    )))
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .gap(px(5.))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .items_center()
                                            .gap(px(5.))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .text_ellipsis()
                                                    .font_family(platform_family())
                                                    // The system UI's macOS 530 adapter
                                                    // selects Bold in Menlo. Keep the
                                                    // reference's numeric monospace 500.
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_size(px(11.))
                                                    .line_height(px(14.))
                                                    .child("$ ls -la"),
                                            )
                                            .when(cursor, |v| {
                                                v.child(
                                                    div()
                                                        .w(px(6.))
                                                        .h(px(13.))
                                                        .flex_shrink_0()
                                                        .rounded(px(2.))
                                                        .bg(rgb(colors.base.cursor)),
                                                )
                                            }),
                                    )
                                    .when(selection, |v| {
                                        v.child(
                                            div()
                                                .flex_shrink_0()
                                                .px(px(7.))
                                                .py(px(3.))
                                                .rounded(px(5.))
                                                .bg(rgb(colors.selection))
                                                .text_color(rgb(colors.selection_text))
                                                .text_size(px(10.))
                                                .line_height(px(14.))
                                                .child("sel"),
                                        )
                                    }),
                            )
                            .child(div().mt(px(8.)).flex().flex_col().gap(px(4.)).children(
                                colors.ansi.chunks(8).map(|row| {
                                    div().flex().gap(px(swatch_gap)).children(row.iter().map(
                                        |color| {
                                            div()
                                                .w(px(swatch_width))
                                                .h(px(6.))
                                                .flex_shrink_0()
                                                .rounded(px(3.))
                                                .bg(rgb(*color))
                                        },
                                    ))
                                }),
                            ))
                            .into_any_element();
                        content.layout_as_root(
                            bounds.size.map(AvailableSpace::Definite),
                            window,
                            cx,
                        );
                        content.prepaint_at(bounds.origin, window, cx);
                        content
                    },
                    |_, mut content, window, cx| content.paint(window, cx),
                )
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .right(px(12.))
                    .top(px(12.))
                    .size(px(if selected { 11. } else { 9. }))
                    .rounded_full()
                    .bg(rgb(colors.accent))
                    .border_1()
                    .border_color(rgb(blend(colors.base.background, colors.accent, 0.25))),
            )
    }
}
