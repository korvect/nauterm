use super::*;
use crate::{
    avatar::{self, HostAvatar},
    components::menu::dropdown_surface,
};
use gpui_kit::{
    base::{ElementExt, Placement},
    component::Selectable,
};
use std::{cell::Cell, rc::Rc};

const TILE: f32 = 40.;
const GAP: f32 = 6.;
const WIDTH: f32 = 296.;
const HEIGHT: f32 = 396.;
const INSET: f32 = 9.;

pub(super) struct Picker {
    pub selected: Option<String>,
    search: Entity<InputState>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    open: bool,
    hovered: bool,
    _search_events: Subscription,
}

impl Picker {
    pub(super) fn new(
        selected: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx));
        let search_events = cx.subscribe_in(
            &search,
            window,
            |this, _, event: &InputEvent, _, cx| match event {
                InputEvent::Change => {
                    this.scroll.set_offset(Point::default());
                    cx.notify();
                }
                InputEvent::PressEnter { .. } if this.open => {
                    if let [choice] = this.choices(cx).as_slice() {
                        this.select(choice.0, cx);
                    }
                }
                _ => {}
            },
        );
        Self {
            selected,
            search,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            open: false,
            hovered: false,
            _search_events: search_events,
        }
    }

    fn choices(&self, cx: &App) -> Vec<(Option<&'static str>, &'static str)> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let automatic = crate::localization::text(cx, "workspace.action.useAutomaticIcon");
        std::iter::once((None, "workspace.action.useAutomaticIcon"))
            .filter(|_| {
                "auto automatic 自动".contains(&query) || automatic.to_lowercase().contains(&query)
            })
            .chain(avatar::HOST_ICON_NAMES.iter().filter_map(|&(slug, name)| {
                avatar::matches_host_icon(slug, name, &query).then_some((Some(slug), name))
            }))
            .collect()
    }

    fn select(&mut self, value: Option<&str>, cx: &mut Context<Self>) {
        self.selected = value.map(str::to_owned);
        self.open = false;
        self.hovered = false;
        cx.notify();
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = !self.open;
        self.hovered = false;
        if self.open {
            let placeholder = crate::localization::text(cx, "workspace.label.searchIcons");
            self.search.update(cx, |search, cx| {
                search.set_placeholder(placeholder, window, cx);
                search.set_value("", window, cx);
            });
            self.scroll.set_offset(Point::default());
        }
        cx.notify();
    }

    fn menu(&self, dark: bool, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::new(dark);
        let window_size = window.viewport_size();
        let anchor = self.bounds.get();
        let width = f32::from(window_size.width - px(16.)).clamp(1., WIDTH);
        let below = window_size.height - anchor.bottom() - px(16.);
        let above = anchor.top() - px(16.);
        let height = f32::from(below.max(above)).clamp(1., HEIGHT);
        let columns = (((width - INSET * 2. - 2. + GAP) / (TILE + GAP)).floor() as usize).max(1);
        let choices = self.choices(cx);
        div()
            .w(px((width - 2.).max(0.)))
            .h(px((height - 2.).max(0.)))
            .flex()
            .flex_col()
            .text_color(p.text)
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(INSET))
                    .pt(px(10.))
                    .pb(px(8.))
                    .text_size(px(13.))
                    .font_weight(FontWeights::SEMIBOLD)
                    .child(crate::localization::text(cx, "workspace.label.hostIcon")),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(INSET))
                    .pb(px(INSET))
                    .child(Input::new(&self.search).aria_label("workspace.label.searchIcons")),
            )
            .child(if choices.is_empty() {
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.))
                    .text_color(p.muted)
                    .child(crate::localization::text(
                        cx,
                        "workspace.label.noMatchingIcons",
                    ))
                    .into_any_element()
            } else {
                div()
                    .id("host-icon-grid")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .scrollbars_with(&self.scroll)
                    .hover_only()
                    .px(px(INSET))
                    .pb(px(INSET))
                    .flex()
                    .flex_col()
                    .gap(px(GAP))
                    .children(choices.chunks(columns).map(|row| {
                        div()
                            .flex()
                            .flex_shrink_0()
                            .gap(px(GAP))
                            .children(row.iter().map(|&(slug, name)| self.tile(slug, name, p, cx)))
                    }))
                    .into_any_element()
            })
            .into_any_element()
    }

    fn tile(
        &self,
        slug: Option<&'static str>,
        name: &'static str,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Button {
        let selected = self.selected.as_deref() == slug;
        let label = if slug.is_some() {
            name.into()
        } else {
            crate::localization::text(cx, name)
        };
        let graphic = if let Some(slug) = slug {
            avatar::host("")
                .editing()
                .custom_icon(Some(slug))
                .into_any_element()
        } else {
            div()
                .size(px(TILE))
                .rounded(px(9.))
                .bg(p.hover)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::RefreshCcw)
                        .with_size(px(27.))
                        .text_color(if selected { p.accent } else { p.muted }),
                )
                .into_any_element()
        };
        Button::new(SharedString::from(format!(
            "host-icon-choice-{}",
            slug.unwrap_or("automatic")
        )))
        .ghost()
        .ink_ripple(false)
        .selected(selected)
        .size(px(TILE))
        .flex_shrink_0()
        .rounded(px(9.))
        .p_0()
        .raw_tooltip(label.clone())
        .accessibility_label(label)
        .child(graphic)
        .when(selected, |button| {
            button.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(9.))
                    .border_2()
                    .border_color(p.accent),
            )
        })
        .on_click(cx.listener(move |this, _, _, cx| this.select(slug, cx)))
    }
}

#[derive(IntoElement)]
pub(super) struct Trigger {
    pub state: Entity<Picker>,
    pub avatar: HostAvatar,
    pub dark: bool,
    pub disabled: bool,
}

impl RenderOnce for Trigger {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = Palette::new(self.dark);
        let state = self.state.read(cx);
        let open = state.open;
        let highlighted = (open || state.hovered) && !self.disabled;
        let anchor = state.bounds.clone();
        let focus = state.focus.clone();
        let search_focus = state.search.read(cx).focus_handle(cx);
        let toggle = self.state.downgrade();
        let hover = toggle.clone();
        let dismiss = toggle.clone();
        let menu = self.state.downgrade();
        let dark = self.dark;
        let button = Button::new("edit-host-icon")
            .ghost()
            .ink_ripple(false)
            .size(px(TILE))
            .rounded(px(10.))
            .p_0()
            .disabled(self.disabled)
            .open(open)
            .track_focus(&focus)
            .localized_accessibility_label("workspace.action.changeIcon")
            .when(!open, |button| {
                button.tooltip("workspace.action.changeIcon")
            })
            .child(self.avatar)
            .when(highlighted, |button| {
                button.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(px(10.))
                        .border_2()
                        .border_color(p.accent),
                )
            })
            .child(
                div()
                    .absolute()
                    .top(px(-4.))
                    .right(px(-4.))
                    .size(px(17.))
                    .rounded_full()
                    .border_1()
                    .border_color(if highlighted { p.accent } else { p.border })
                    .bg(p.surface)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::Pencil)
                            .with_size(px(10.))
                            .text_color(if highlighted { p.accent } else { p.muted }),
                    ),
            )
            .on_hover(move |hovered, _, cx| {
                let _ = hover.update(cx, |state, cx| {
                    if state.hovered != *hovered {
                        state.hovered = *hovered;
                        cx.notify();
                    }
                });
            })
            .on_click(move |_, window, cx| {
                let _ = toggle.update(cx, |state, cx| state.toggle(window, cx));
            });
        dropdown_surface(("host-icon-picker", self.state.entity_id()), p, dark)
            .open(open && !self.disabled)
            .side(Placement::Bottom)
            .origin_focus(&focus)
            .content_focus(&search_focus)
            .on_open_change(move |open, _, cx| {
                let _ = dismiss.update(cx, |state, cx| {
                    if state.open != *open {
                        state.open = *open;
                        state.hovered = false;
                        cx.notify();
                    }
                });
            })
            .content(move |window, cx| {
                menu.update(cx, |state, cx| state.menu(dark, window, cx))
                    .unwrap_or_else(|_| div().into_any_element())
            })
            .attach_to(
                div()
                    .size(px(TILE))
                    .flex_shrink_0()
                    .relative()
                    .on_prepaint(move |bounds, _, _| anchor.set(bounds))
                    .child(button),
            )
    }
}
