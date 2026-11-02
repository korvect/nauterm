use super::{Button, ButtonCustomVariant, ScrollbarsExt};
use crate::{palette::Palette, typography::FontWeights};
use gpui_kit::{
    assets::IconName,
    component::{Icon, Sizable},
    prelude::*,
    *,
};
use std::rc::Rc;

type SelectHandler<V> = Rc<dyn Fn(&V, &mut Window, &mut App)>;

/// Collection paths share the reference's single row and noninteractive leaf.
#[derive(IntoElement)]
pub struct Breadcrumb<V: Clone + 'static> {
    id: ElementId,
    entries: Vec<(V, SharedString)>,
    palette: Palette,
    select: Option<SelectHandler<V>>,
}

impl<V: Clone + 'static> Breadcrumb<V> {
    pub fn new(
        id: impl Into<ElementId>,
        entries: impl IntoIterator<Item = (V, SharedString)>,
        palette: Palette,
    ) -> Self {
        Self {
            id: id.into(),
            entries: entries.into_iter().collect(),
            palette,
            select: None,
        }
    }

    pub fn on_select(mut self, select: impl Fn(&V, &mut Window, &mut App) + 'static) -> Self {
        self.select = Some(Rc::new(select));
        self
    }
}

impl<V: Clone + 'static> RenderOnce for Breadcrumb<V> {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let p = self.palette;
        let count = self.entries.len();
        let colors = ButtonCustomVariant::for_surface(p).foreground(p.muted.into());
        let mut row = div()
            .id(self.id)
            .role(Role::Navigation)
            .h(px(28.))
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .overflow_x_scroll()
            .scrollbars_hidden()
            .flex()
            .items_center()
            .on_click(|_, _, cx| cx.stop_propagation());
        for (index, (value, label)) in self.entries.into_iter().enumerate() {
            if index > 0 {
                row = row.child(
                    Icon::new(IconName::ChevronRight)
                        .with_size(px(17.))
                        .mx(px(3.))
                        .flex_shrink_0()
                        .text_color(p.muted),
                );
            }
            row = if index + 1 == count {
                row.child(
                    div()
                        .id(("breadcrumb-current", index))
                        .role(Role::Label)
                        .h(px(28.))
                        .px(px(9.))
                        .flex_shrink_0()
                        .rounded(px(7.))
                        .bg(p.hover)
                        .flex()
                        .items_center()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .whitespace_nowrap()
                        .font_weight(FontWeights::SEMIBOLD)
                        .text_color(p.text)
                        .child(label),
                )
            } else {
                let select = self.select.clone();
                row.child(
                    Button::new(("breadcrumb-ancestor", index))
                        .xsmall()
                        .custom(colors)
                        .h(px(28.))
                        .px(px(9.))
                        .flex_shrink_0()
                        .rounded(px(7.))
                        .text_size(px(12.))
                        .raw_label(label)
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            if let Some(select) = &select {
                                select(&value, window, cx);
                            }
                        }),
                )
            };
        }
        row
    }
}
