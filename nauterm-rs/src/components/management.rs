//! Workspace management surfaces, shared without imposing the Settings skin.
use super::{
    Button, Input, ScrollbarsExt,
    menu::{DropdownMenu, Menu},
};
use crate::{palette::Palette, typography::FontWeights};
use gpui_kit::{
    assets::IconName,
    base::input::InputState,
    component::{ActiveTheme, Icon, Sizable},
    prelude::*,
    *,
};
use std::rc::Rc;
type EmptyAction = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub const COLLECTION_SECTION_GAP: f32 = 28.;
pub const COLLECTION_TITLE_GAP: f32 = 12.;

/// `_workspacePanePadding` is shared by every management collection.
pub fn collection_padding<T: Styled>(view: T) -> T {
    view.px(px(26.)).pt(px(22.)).pb(px(30.))
}

pub fn section_title(title: &str, cx: &App) -> Div {
    div()
        .flex_shrink_0()
        .text_size(px(13.))
        .line_height(px(18.))
        .font_weight(FontWeights::SEMIBOLD)
        .child(crate::localization::text(cx, title))
}

pub fn collection_section(title: &str, cx: &App) -> Div {
    div()
        .min_w_0()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(COLLECTION_TITLE_GAP))
        .child(section_title(title, cx))
}

#[derive(IntoElement)]
pub struct EmptyState {
    icon: IconName,
    title: SharedString,
    description: Option<SharedString>,
    action: Option<AnyElement>,
    dark: bool,
    click: Option<EmptyAction>,
}
impl EmptyState {
    pub fn new(icon: IconName, title: impl Into<SharedString>, dark: bool) -> Self {
        Self {
            icon,
            title: title.into(),
            description: None,
            action: None,
            dark,
            click: None,
        }
    }
    pub fn description(mut self, value: impl Into<SharedString>) -> Self {
        self.description = Some(value.into());
        self
    }
    pub fn action(mut self, action: Button) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
    pub fn on_click(
        mut self,
        callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.click = Some(Rc::new(callback));
        self
    }
}
impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let title = crate::localization::text(cx, &self.title);
        let description = self
            .description
            .map(|description| crate::localization::text(cx, &description));
        let p = Palette::new(self.dark);
        div()
            .id("management-empty-state")
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(32.))
            .when_some(self.click, |view, callback| {
                view.on_click(move |event, window, cx| callback(event, window, cx))
            })
            .child(
                div()
                    .w_full()
                    .max_w(px(360.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_center()
                    .child(
                        div()
                            .size(px(48.))
                            .rounded(px(12.))
                            .bg(p.hover)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(p.muted)
                            .child(Icon::new(self.icon).with_size(px(23.))),
                    )
                    .child(
                        div()
                            .mt(px(14.))
                            .text_size(px(16.))
                            .font_weight(FontWeights::MEDIUM)
                            .text_color(p.text)
                            .child(title),
                    )
                    .when_some(description, |view, text| {
                        view.child(
                            div()
                                .mt(px(6.))
                                .text_size(px(13.))
                                .line_height(relative(1.4))
                                .text_color(p.muted)
                                .child(text),
                        )
                    })
                    .when_some(self.action, |view, action| {
                        view.child(div().mt(px(18.)).child(action))
                    }),
            )
    }
}

pub fn toolbar(dark: bool) -> Div {
    let p = Palette::new(dark);
    div()
        .h(px(48.))
        .min_w_0()
        .flex_shrink_0()
        .px(px(10.))
        .py(px(8.))
        .flex()
        .items_center()
        .gap(px(6.))
        .bg(p.surface)
        .border_b_1()
        .border_color(p.border)
}

/// Measure the full labels against the collection's actual width, including
/// its drawer. Icon actions and an expanded search retain their own space.
/// Creation groups keep their labels; include them here to reserve that space
/// when deciding whether standalone actions should collapse.
pub fn toolbar_labels_collapsed(
    width: f32,
    labels: &[(&str, bool)],
    icons: usize,
    search_open: bool,
    window: &Window,
) -> bool {
    let controls = labels.len() + icons + usize::from(search_open);
    // Include the flexible spacer's gap. A joined button group needs slightly
    // less room, so this also leaves a few pixels at its collapse boundary.
    let mut required = 20. + controls as f32 * 6. + icons as f32 * 30.;
    if search_open {
        required += 180.;
    }
    let mut style = window.text_style();
    style.font_weight = FontWeights::MEDIUM;
    for &(label, icon) in labels {
        let text: SharedString = label.to_owned().into();
        let run = style.to_run(text.len());
        let measured = window.text_system().shape_line(text, px(12.), &[run], None);
        required += f32::from(measured.width()).ceil() + 20. + if icon { 22. } else { 0. };
    }
    width < required
}

/// Connected segments keep independent button focus, ink and activation.
/// A dropdown segment anchors its overlay before the group is laid out.
#[derive(IntoElement)]
pub struct ButtonGroup {
    segments: Vec<(Button, Option<MenuBuilder>)>,
    toolbar: bool,
}
impl Default for ButtonGroup {
    fn default() -> Self {
        Self {
            segments: vec![],
            toolbar: true,
        }
    }
}
type MenuBuilder = Rc<dyn Fn(Menu, &mut Window, &mut App) -> Menu>;
impl ButtonGroup {
    pub fn new() -> Self {
        Self::default()
    }
    /// Preserve the caller's sizing outside management toolbars.
    pub fn controls(mut self) -> Self {
        self.toolbar = false;
        self
    }
    pub fn button(mut self, button: Button) -> Self {
        self.segments.push((button, None));
        self
    }
    pub fn dropdown(
        mut self,
        button: Button,
        menu: impl Fn(Menu, &mut Window, &mut App) -> Menu + 'static,
    ) -> Self {
        self.segments.push((button, Some(Rc::new(menu))));
        self
    }
}
impl RenderOnce for ButtonGroup {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let count = self.segments.len();
        div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(2.))
            .children(
                self.segments
                    .into_iter()
                    .enumerate()
                    .map(|(index, (button, menu))| {
                        let button = button
                            .when(self.toolbar, Button::toolbar)
                            .rounded(px(0.))
                            .when(index == 0, |button| button.rounded_l(px(8.)))
                            .when(index + 1 == count, |button| button.rounded_r(px(8.)));
                        match menu {
                            Some(menu) => button
                                .dropdown_menu(move |surface, window, cx| menu(surface, window, cx))
                                .into_any_element(),
                            None => button.into_any_element(),
                        }
                    }),
            )
    }
}

pub fn toolbar_search(
    id: &'static str,
    state: &Entity<InputState>,
    open: bool,
    toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let state = state.clone();
    let p = Palette::new(cx.theme().is_dark());
    let mut search_palette = p;
    search_palette.surface = p.hover;
    if open || !state.read(cx).value().is_empty() {
        let clear = state.clone();
        div()
            .w(px(180.))
            .min_w(px(84.))
            .flex_shrink_1()
            .child(
                Input::new(&state)
                    .palette(search_palette)
                    .small()
                    .h(px(30.))
                    .pl(px(8.))
                    .pr(px(2.))
                    .py_0()
                    .rounded(px(7.))
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .font_weight(FontWeights::MEDIUM)
                    .prefix(
                        Icon::new(IconName::Search)
                            .with_size(px(16.))
                            .text_color(p.text),
                    )
                    .aria_label(crate::localization::text(cx, "Search"))
                    .suffix(
                        Button::new((ElementId::from(id), "clear"))
                            .ghost()
                            .size(px(26.))
                            .p_0()
                            .rounded_full()
                            .icon(crate::assets::icon(IconName::X))
                            .icon_size(px(15.))
                            .focusable(false)
                            .tab_stop(false)
                            .custom(
                                super::ButtonCustomVariant::for_surface(search_palette)
                                    .foreground(p.muted.into()),
                            )
                            .tooltip("Close search")
                            .on_click(move |_, window, cx| {
                                clear.update(cx, |field, cx| field.set_value("", window, cx));
                                toggle(false, window, cx);
                            }),
                    ),
            )
            .into_any_element()
    } else {
        Button::new(id)
            .toolbar()
            .ghost()
            .icon(IconName::Search)
            .tooltip("Search")
            .on_click(move |_, window, cx| toggle(true, window, cx))
            .into_any_element()
    }
}

/// Standard editor drawer. Nested selection/gallery pages can use their own
/// navigation, while first-level editors share geometry and Save placement.
/// Shared by form drawers and workflows whose body manages its own scrolling.
pub(crate) fn drawer_header(
    title: impl Into<SharedString>,
    dark: bool,
    leading: Vec<AnyElement>,
    actions: Vec<AnyElement>,
) -> Div {
    let p = Palette::new(dark);
    div()
        .h(px(48.))
        .flex_shrink_0()
        .px(px(16.))
        .bg(p.surface)
        .border_b_1()
        .border_color(p.border)
        .flex()
        .items_center()
        .gap(px(6.))
        .children(leading)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(14.))
                .font_weight(FontWeights::MEDIUM)
                .truncate()
                .child(title.into()),
        )
        .children(actions)
}

#[derive(IntoElement)]
pub struct DrawerFrame {
    id: ElementId,
    title: SharedString,
    dark: bool,
    leading: Vec<AnyElement>,
    actions: Vec<AnyElement>,
    sections: Vec<AnyElement>,
    footer: Option<AnyElement>,
    scroll: Option<ScrollHandle>,
}
impl DrawerFrame {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, dark: bool) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            dark,
            leading: vec![],
            actions: vec![],
            sections: vec![],
            footer: None,
            scroll: None,
        }
    }
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
    pub fn scroll_handle(mut self, scroll: &ScrollHandle) -> Self {
        self.scroll = Some(scroll.clone());
        self
    }
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading.push(element.into_any_element());
        self
    }
    pub fn actions(mut self, actions: impl IntoIterator<Item = AnyElement>) -> Self {
        self.actions.extend(actions);
        self
    }
    pub fn section(mut self, section: impl IntoElement) -> Self {
        self.sections.push(section.into_any_element());
        self
    }
    pub fn sections(mut self, sections: impl IntoIterator<Item = AnyElement>) -> Self {
        self.sections.extend(sections);
        self
    }
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }
}
impl RenderOnce for DrawerFrame {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let p = Palette::new(self.dark);
        div()
            .w(px(crate::layout::DRAWER_WIDTH))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(p.background)
            .border_l_1()
            .border_color(p.border)
            .child(drawer_header(
                self.title,
                self.dark,
                self.leading,
                self.actions,
            ))
            .child(
                div()
                    .id((self.id, "scroll"))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .map(|view| match self.scroll {
                        Some(handle) => view.scrollbars_with(&handle),
                        None => view.scrollbars(),
                    })
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .children(self.sections),
            )
            .when_some(self.footer, |view, footer| {
                view.child(
                    div()
                        .h(px(64.))
                        .flex_shrink_0()
                        .px(px(16.))
                        .py(px(10.))
                        .border_t_1()
                        .border_color(p.border)
                        .child(footer),
                )
            })
    }
}
