//! Shared management cards and explicit grid tracks. Cards never decide when
//! to wrap from independently rounded widths during a window resize.
use super::{Button, ButtonCustomVariant, ink};
use crate::{layout, palette::Palette, typography::FontWeights};
use gpui_kit::{assets::IconName, component::Sizable, prelude::*, *};
use std::time::Duration;

pub(super) const TRANSITION: Duration = Duration::from_millis(120);

#[derive(Clone, Copy)]
pub(super) struct Surface {
    pub dark: bool,
    pub list: bool,
    pub selected: bool,
}
impl Surface {
    pub fn colors(self) -> ButtonCustomVariant {
        let p = Palette::new(self.dark);
        ButtonCustomVariant::for_surface(p)
            .color(p.surface.into())
            .hover(rgb(if self.dark { 0x343434 } else { 0xfbfdfd }).into())
            .active(if self.dark {
                rgb(0x484848).into()
            } else {
                ink::faded(rgb(0xdbe8eb), 0.42).into()
            })
            .highlight(if self.dark {
                ink::faded(rgb(0x383838), 0.72).into()
            } else {
                ink::faded(rgb(0xe7eff1), 0.36).into()
            })
    }

    pub fn shadows(self, hover: f32) -> Vec<BoxShadow> {
        // Match the reference's 2 -> 5 card elevation (1 -> 3 in list mode)
        // and its blue-gray shadow, without moving the card's layout bounds.
        let (rest, raised, alpha, hover_alpha) = if self.list {
            (1., 3., 0.10, 0.18)
        } else {
            (2., 5., 0.14, 0.22)
        };
        let elevation = rest + (raised - rest) * hover;
        vec![BoxShadow {
            color: ink::faded(rgb(0x6d858c), alpha + (hover_alpha - alpha) * hover).into(),
            offset: point(px(0.), px(elevation / 2.)),
            blur_radius: px(elevation * 2.),
            spread_radius: px(0.),
            inset: false,
        }]
    }
}

pub fn card_grid(layout: layout::CardGrid) -> Div {
    div()
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .grid()
        .grid_cols(layout.columns)
        .items_start()
        .gap_x(px(layout::CARD_GAP))
        .gap_y(px(12.))
        .when_some(layout.max_width, |grid, width| grid.max_w(px(width)))
}

pub fn item_card(id: impl Into<ElementId>, dark: bool, selected: bool, list: bool) -> Button {
    Button::new(id)
        .card_surface(Surface {
            dark,
            list,
            selected,
        })
        .focusable(false)
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .h(px(if list { 52. } else { 58. }))
        .px(px(11.))
        .content_start()
        .content_gap(px(if list { 11. } else { 10. }))
        .rounded(px(if list { 8. } else { 12. }))
        // Ink and the selection outline share the full rounded surface. A
        // transparent layout border leaves an unpainted rim around the ink.
        .border_0()
        .aria_selected(selected)
}

/// `_WorkspaceCardEditButton`: a circular action above the card's own ink.
pub fn card_edit_button(id: impl Into<ElementId>, dark: bool) -> Button {
    let p = Palette::new(dark);
    let pressed = rgb(0x484848);
    Button::new(id)
        .xsmall()
        .size(px(26.))
        .p_0()
        .rounded_full()
        .focusable(false)
        .custom(
            ButtonCustomVariant::for_surface(p)
                .color(
                    if dark {
                        pressed.alpha(0.72)
                    } else {
                        rgb(0xedf3f5)
                    }
                    .into(),
                )
                .foreground(p.accent.into())
                .hover(
                    if dark {
                        rgb(0x383838).alpha(0.78)
                    } else {
                        rgb(0xdce8ec)
                    }
                    .into(),
                )
                .active(if dark { pressed } else { rgb(0xcbdde2) }.into())
                .highlight(if dark { pressed } else { rgb(0xd3e2e6) }.into()),
        )
        .icon(IconName::Pencil)
        .icon_size(px(15.))
        .tooltip("Edit")
}

pub fn card_text(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    dark: bool,
) -> Div {
    let detail = detail.into();
    let p = Palette::new(dark);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .text_size(px(13.))
                .line_height(px(18.))
                .font_weight(FontWeights::MEDIUM)
                .truncate()
                .child(title.into()),
        )
        .when(!detail.trim().is_empty(), |text| {
            text.child(
                div()
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .font_weight(FontWeights::MEDIUM)
                    .text_color(p.muted)
                    .truncate()
                    .child(detail),
            )
        })
}
