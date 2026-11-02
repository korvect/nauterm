//! Plain reading text joins the same selection order as product Markdown.
use crate::palette::Palette;
use gpui_kit::{ElementId, SharedString, base::SelectableText};

pub fn selectable_text(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    palette: Palette,
) -> SelectableText {
    SelectableText::new(id, text)
        .in_paint_order()
        .selection_color(palette.accent.alpha(0.24).into())
}
