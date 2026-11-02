//! Product menus. No styled Kit menu/popover participates in this implementation.
//! Commands run before one dismissal event; only enabled actions are invokable.
use super::ScrollbarsExt;
use super::{button::Button, motion, overlay::Overlay};
use crate::{palette::Palette, typography::FontWeights};
use gpui_kit::base::{ElementExt, Placement};
use gpui_kit::{
    assets::IconName,
    component::{ActiveTheme, Disableable, Icon, Selectable},
    prelude::*,
    *,
};
use std::{
    collections::BTreeMap,
    rc::Rc,
    time::{Duration, Instant},
};

type Invoke = Rc<dyn Fn(&MenuInvocation, &mut Window, &mut App)>;
type Builder = Rc<dyn Fn(Menu, &mut Window, &mut Context<MenuState>) -> Menu>;
type Enabled = Rc<dyn Fn(&mut Window, &mut App) -> bool>;
pub struct MenuInvocation;

#[derive(Clone)]
pub struct MenuItem {
    label: SharedString,
    localized: bool,
    caption: Option<SharedString>,
    icon: Option<IconName>,
    image: Option<std::sync::Arc<Image>>,
    disabled: bool,
    destructive: bool,
    checked: Option<bool>,
    shortcut: Option<SharedString>,
    accelerators: Vec<KeybindingKeystroke>,
    command: Option<Invoke>,
    children: Option<Submenu>,
}
#[derive(Clone)]
enum Submenu {
    Ready(Box<Menu>),
    Lazy(Builder),
}
impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            localized: false,
            caption: None,
            icon: None,
            image: None,
            disabled: false,
            destructive: false,
            checked: None,
            shortcut: None,
            accelerators: vec![],
            command: None,
            children: None,
        }
    }
    /// Product copy is explicit; application names, host/model IDs and other
    /// user-owned labels continue to use new() without translation.
    pub fn localized(label: impl Into<SharedString>) -> Self {
        let mut item = Self::new(label);
        item.localized = true;
        item
    }
    fn display_for(&self, language: crate::localization::Language) -> SharedString {
        if self.localized {
            crate::localization::text_for(language, &self.label)
        } else {
            self.label.clone()
        }
    }
    fn display(&self, cx: &App) -> SharedString {
        self.display_for(crate::localization::language(cx))
    }
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn image(mut self, image: std::sync::Arc<Image>) -> Self {
        self.image = Some(image);
        self
    }
    /// Secondary context above the action's label, such as its AI provider.
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
    /// A single-stroke accelerator resolved in the originating control's scope.
    /// Matching uses GPUI's layout-aware key matcher, not display strings.
    pub fn accelerator(mut self, key: KeybindingKeystroke) -> Self {
        self.accelerators.push(key);
        self
    }
    pub fn submenu(mut self, menu: Menu) -> Self {
        self.children = Some(Submenu::Ready(Box::new(menu)));
        self
    }
    /// Build only when opened, so native catalogs need not block the UI or load
    /// for every visible row. Async results update that exact child, never reopen it.
    pub fn submenu_with(
        mut self,
        build: impl Fn(Menu, &mut Window, &mut Context<MenuState>) -> Menu + 'static,
    ) -> Self {
        self.children = Some(Submenu::Lazy(Rc::new(build)));
        self
    }
    pub fn on_click(
        mut self,
        command: impl Fn(&MenuInvocation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.command = Some(Rc::new(command));
        self
    }
}
#[derive(Clone)]
enum Entry {
    Action(Box<MenuItem>),
    Separator,
}
#[derive(Clone)]
pub struct Menu {
    id: Option<ElementId>,
    entries: Vec<Entry>,
    width: f32,
    max_visible_rows: Option<usize>,
    settings: bool,
    card: bool,
    palette: Option<Palette>,
}
impl Default for Menu {
    fn default() -> Self {
        Self {
            id: None,
            entries: vec![],
            width: 220.,
            max_visible_rows: Some(7),
            settings: false,
            card: false,
            palette: None,
        }
    }
}
impl Menu {
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = Some(palette);
        self
    }
    fn inherit_palette(mut self, palette: Option<Palette>) -> Self {
        self.palette = self.palette.or(palette);
        self
    }
    fn colors(&self, cx: &App) -> Palette {
        let mut palette = self.palette.unwrap_or_else(|| {
            if self.settings {
                Palette::settings(cx.theme().is_dark())
            } else {
                Palette::new(cx.theme().is_dark())
            }
        });
        if self.card {
            let dark = cx.theme().is_dark();
            palette.hover = rgb(if dark { 0x3e3e3e } else { 0xf0f4f5 });
            if !dark {
                palette.border = rgb(0xd9e3e6);
                palette.muted = rgb(0xc2d0d5);
            }
        }
        palette
    }
    pub(super) fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn when(self, condition: bool, build: impl FnOnce(Self) -> Self) -> Self {
        if condition { build(self) } else { self }
    }
    pub fn item(mut self, item: MenuItem) -> Self {
        self.entries.push(Entry::Action(Box::new(item)));
        self
    }
    pub fn separator(mut self) -> Self {
        if matches!(self.entries.last(), Some(Entry::Action(_))) {
            self.entries.push(Entry::Separator);
        }
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.clamp(100., 480.);
        self
    }
    /// Dropdown height cap. None fits all rows within the window's bounds.
    pub fn max_visible_rows(mut self, rows: Option<usize>) -> Self {
        self.max_visible_rows = rows.map(|rows| rows.max(1));
        self
    }
    pub fn settings(mut self, settings: bool) -> Self {
        self.settings = settings;
        self
    }
    /// Workspace resource menus have the reference's roomier rows and shortcut
    /// badges, independently of compact dropdowns and Settings menus.
    pub fn card_menu(mut self) -> Self {
        self.card = true;
        self.width = 286.;
        self
    }
    fn enabled(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                matches!(entry, Entry::Action(item) if !item.disabled).then_some(i)
            })
            .collect()
    }
    fn initial_active(&self) -> Option<usize> {
        self.entries.iter().position(|entry| {
            matches!(entry, Entry::Action(item) if !item.disabled && item.checked == Some(true))
        })
    }
}
gpui_kit::actions!(
    menu,
    [Next, Previous, First, Last, Activate, Cancel, Forward, Back]
);
pub(super) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("down", Next, Some("NautermMenu")),
        KeyBinding::new("up", Previous, Some("NautermMenu")),
        KeyBinding::new("home", First, Some("NautermMenu")),
        KeyBinding::new("end", Last, Some("NautermMenu")),
        KeyBinding::new("enter", Activate, Some("NautermMenu")),
        KeyBinding::new("space", Activate, Some("NautermMenu")),
        KeyBinding::new("escape", Cancel, Some("NautermMenu")),
        KeyBinding::new("left", Back, Some("NautermMenu")),
        KeyBinding::new("right", Forward, Some("NautermMenu")),
    ]);
}
struct Child {
    state: Entity<MenuState>,
    _changed: Subscription,
}
pub struct MenuState {
    menu: Menu,
    builder: Option<Builder>,
    open: bool,
    focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
    action_focus: Option<FocusHandle>,
    origin_focus: Option<FocusHandle>,
    position: Option<Point<Pixels>>,
    active: Option<usize>,
    hovered: Option<usize>,
    pointer_mode: bool,
    pointer_inside: bool,
    root: WeakEntity<MenuState>,
    child: bool,
    children: BTreeMap<usize, Child>,
    typeahead: String,
    typed_at: Instant,
    scroll: ScrollHandle,
    settings: bool,
    dropdown: bool,
    bounds: Bounds<Pixels>,
    row_bounds: BTreeMap<usize, Bounds<Pixels>>,
    submenu_origin: Option<Point<Pixels>>,
    pending_hover: Option<usize>,
    hover_delay: Option<Task<()>>,
    _accelerators: Subscription,
}
impl EventEmitter<DismissEvent> for MenuState {}
impl Focusable for MenuState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl MenuState {
    pub fn child_menu(&self, index: usize) -> Option<&Entity<MenuState>> {
        self.children.get(&index).map(|child| &child.state)
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn replace_items_if_open(&mut self, menu: Menu, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        let selected = self
            .active
            .and_then(|index| match self.menu.entries.get(index) {
                Some(Entry::Action(item)) => Some(item.label.clone()),
                _ => None,
            });
        self.menu = menu
            .settings(self.menu.settings)
            .inherit_palette(self.menu.palette);
        self.children.clear();
        self.hovered = None;
        self.active = self.menu.enabled().into_iter().find(|index| {
            matches!(&self.menu.entries[*index], Entry::Action(item) if Some(&item.label) == selected.as_ref())
        }).or_else(|| self.menu.initial_active());
        if let Some(active) = self.active {
            self.scroll.scroll_to_item(active);
        }
        cx.notify();
    }
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let owner = cx.weak_entity();
        let accelerators = cx.intercept_keystrokes(move |event, window, cx| {
            let Some(owner) = owner.upgrade() else {
                return;
            };
            let state = owner.read(cx);
            if !state.open || !state.focus.is_focused(window) {
                return;
            }
            let index = state.menu.entries.iter().position(|entry| match entry {
                Entry::Action(item) => item
                    .accelerators
                    .iter()
                    .any(|key| event.keystroke.should_match(key)),
                Entry::Separator => false,
            });
            if let Some(index) = index {
                // Also consume disabled accelerators: they must not fall through
                // and act on an unrelated document or terminal behind the menu.
                owner.update(cx, |state, cx| state.activate(index, window, cx));
                window.prevent_default();
                cx.stop_propagation();
            }
        });
        Self {
            menu: Menu::default(),
            builder: None,
            open: false,
            focus: cx.focus_handle(),
            previous_focus: None,
            action_focus: None,
            origin_focus: None,
            position: None,
            active: None,
            hovered: None,
            pointer_mode: false,
            pointer_inside: false,
            root: cx.weak_entity(),
            child: false,
            children: BTreeMap::new(),
            typeahead: String::new(),
            typed_at: Instant::now(),
            scroll: ScrollHandle::new(),
            settings: false,
            dropdown: false,
            bounds: Bounds::default(),
            row_bounds: BTreeMap::new(),
            submenu_origin: None,
            pending_hover: None,
            hover_delay: None,
            _accelerators: accelerators,
        }
    }
    fn open(
        &mut self,
        position: Option<Point<Pixels>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let replacing = self.open;
        if !replacing || !self.focus.contains_focused(window, cx) {
            self.previous_focus = window.focused(cx);
        }
        self.action_focus = self.previous_focus.clone();
        self.origin_focus = None;
        self.position = position;
        self.menu = self
            .builder
            .clone()
            .map(|builder| builder(Menu::default().settings(self.settings), window, cx))
            .unwrap_or_default();
        self.children.clear();
        self.hover_delay = None;
        self.pending_hover = None;
        self.submenu_origin = None;
        self.row_bounds.clear();
        self.active = self.menu.initial_active();
        self.hovered = None;
        self.pointer_mode = false;
        self.pointer_inside = false;
        self.typeahead.clear();
        self.open = true;
        if replacing {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }
    /// Editors must transfer focus synchronously so base can retain their
    /// selection. The original focus also identifies any parent suggestions
    /// overlay when our surface is painted on the following frame.
    pub(super) fn show_at(
        &mut self,
        menu: Menu,
        position: Point<Pixels>,
        origin: FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.previous_focus = Some(origin.clone());
        self.action_focus = Some(origin.clone());
        self.origin_focus = Some(origin);
        self.position = Some(position);
        self.menu = menu;
        self.children.clear();
        self.hover_delay = None;
        self.pending_hover = None;
        self.submenu_origin = None;
        self.row_bounds.clear();
        self.active = self.menu.initial_active();
        self.hovered = None;
        self.pointer_mode = false;
        self.pointer_inside = false;
        self.typeahead.clear();
        self.open = true;
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn dismiss(&mut self, restore: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.hovered = None;
        self.pointer_inside = false;
        self.hover_delay = None;
        self.pending_hover = None;
        if restore && let Some(focus) = &self.previous_focus {
            focus.focus(window, cx);
        }
        cx.emit(DismissEvent);
        cx.notify();
    }
    fn dismiss_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss(false, window, cx);
        if self.child {
            let _ = self
                .root
                .update(cx, |root, cx| root.dismiss(false, window, cx));
        }
    }
    fn navigate(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.pointer_mode = false;
        self.hovered = None;
        self.hover_delay = None;
        self.pending_hover = None;
        self.submenu_origin = None;
        self.active = next(&self.menu.enabled(), self.active, delta);
        if let Some(active) = self.active {
            self.scroll.scroll_to_item(active);
        }
        cx.notify();
    }
    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.hover_delay = None;
        self.pending_hover = None;
        if !self.open {
            return;
        }
        let Some(Entry::Action(item)) = self.menu.entries.get(index).cloned() else {
            return;
        };
        if item.disabled {
            return;
        }
        if item.children.is_some() {
            self.open_child(index, window, cx);
            return;
        }
        if let Some(focus) = &self.action_focus {
            focus.focus(window, cx);
        }
        if let Some(command) = &item.command {
            command(&MenuInvocation, window, cx);
        }
        // Keep this after the command: Delete may set a confirmation flag and
        // Edit may change which card should be restored by the dismissal observer.
        self.dismiss_all(window, cx);
    }
    fn open_child(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        let Some(Entry::Action(item)) = self.menu.entries.get(index) else {
            return;
        };
        let Some(menu) = item.children.clone().filter(|_| !item.disabled) else {
            return;
        };
        let settings = self.menu.settings;
        let palette = self.menu.palette;
        let already_open = self
            .children
            .get(&index)
            .is_some_and(|child| child.state.read(cx).open);
        if already_open {
            return;
        }
        self.focus.focus(window, cx);
        for child in self.children.values() {
            child
                .state
                .update(cx, |state, cx| state.dismiss(false, window, cx));
        }
        if let Some(child) = self.children.get(&index) {
            let previous = self.focus.clone();
            child.state.update(cx, |state, cx| {
                state.menu = match menu {
                    Submenu::Ready(menu) => (*menu).settings(settings).inherit_palette(palette),
                    Submenu::Lazy(build) => build(
                        Menu::default().settings(settings).inherit_palette(palette),
                        window,
                        cx,
                    )
                    .inherit_palette(palette),
                };
                state.children.clear();
                state.open = true;
                state.previous_focus = Some(previous);
                state.active = state.menu.initial_active();
                cx.notify();
            });
        } else {
            let previous = self.focus.clone();
            let action_focus = self.action_focus.clone();
            let root = self.root.clone();
            let state = cx.new(|cx| {
                let mut state = Self::new(cx);
                state.menu = match menu {
                    Submenu::Ready(menu) => (*menu).settings(settings).inherit_palette(palette),
                    Submenu::Lazy(build) => build(
                        Menu::default().settings(settings).inherit_palette(palette),
                        window,
                        cx,
                    )
                    .inherit_palette(palette),
                };
                state.open = true;
                state.child = true;
                state.root = root;
                state.previous_focus = Some(previous);
                state.action_focus = action_focus;
                state.active = state.menu.initial_active();
                state
            });
            let changed = cx.observe(&state, |_, _, cx| cx.notify());
            self.children.insert(
                index,
                Child {
                    state,
                    _changed: changed,
                },
            );
        }
        cx.notify();
    }
    fn hovered(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open
            || !matches!(self.menu.entries.get(index), Some(Entry::Action(item)) if !item.disabled)
        {
            return;
        }
        self.pointer_mode = true;
        let point = window.mouse_position();
        if self.active == Some(index) {
            self.submenu_origin = Some(point);
            self.pending_hover = None;
            self.hover_delay = None;
        } else if self.aiming_at_submenu(point, cx) {
            self.pending_hover = Some(index);
            if self.hover_delay.is_none() {
                self.hover_delay = Some(cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
                    let _ = this.update_in(cx, |this, window, cx| {
                        this.hover_delay = None;
                        let Some(index) = this.pending_hover.take() else {
                            return;
                        };
                        let point = window.mouse_position();
                        // Entering the child cancels the pending sibling. Its
                        // actual resolved bounds also cover left-flipped menus.
                        if this.open
                            && this
                                .row_bounds
                                .get(&index)
                                .is_some_and(|bounds| bounds.contains(&point))
                        {
                            this.apply_hover(index, window, cx);
                        }
                    });
                }));
            }
            return;
        } else {
            self.hover_delay = None;
            self.pending_hover = None;
            self.submenu_origin = Some(point);
        }
        self.apply_hover(index, window, cx);
    }
    fn leave_hover(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.pending_hover == Some(index) {
            self.pending_hover = None;
            self.hover_delay = None;
        }
        if self.hovered == Some(index) {
            self.hovered = None;
            cx.notify();
        }
    }
    fn highlighted(&self, cx: &App) -> Option<usize> {
        if !self.pointer_mode {
            return self.active;
        }
        // A parent stays expanded while interacting with its child or a
        // protected sibling crossing, but leaving the whole branch clears ink.
        self.hovered.or_else(|| {
            self.active.filter(|index| {
                self.children.get(index).is_some_and(|child| {
                    let child = child.state.read(cx);
                    child.open && (child.pointer_in_branch(cx) || self.pending_hover.is_some())
                })
            })
        })
    }
    fn pointer_in_branch(&self, cx: &App) -> bool {
        self.open
            && (self.pointer_inside
                || self
                    .children
                    .values()
                    .any(|child| child.state.read(cx).pointer_in_branch(cx)))
    }
    fn aiming_at_submenu(&self, point: Point<Pixels>, cx: &App) -> bool {
        let Some(origin) = self.submenu_origin else {
            return false;
        };
        let Some(child) = self
            .active
            .and_then(|index| self.children.get(&index))
            .map(|child| child.state.read(cx))
            .filter(|child| child.open)
        else {
            return false;
        };
        let bounds = child.bounds;
        if bounds.size.width <= px(0.) {
            return false;
        }
        let right = bounds.left() >= self.bounds.right() - px(8.);
        let edge = if right { bounds.left() } else { bounds.right() };
        // Only motion toward the child is protected, never movement away.
        if (right && point.x < origin.x) || (!right && point.x > origin.x) {
            return false;
        }
        in_triangle(
            point,
            origin,
            point_at(edge, bounds.top() - px(8.)),
            point_at(edge, bounds.bottom() + px(8.)),
        )
    }
    fn apply_hover(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.hovered != Some(index) {
            self.hovered = Some(index);
            cx.notify();
        }
        if self.active == Some(index) {
            if matches!(self.menu.entries.get(index), Some(Entry::Action(item)) if item.children.is_some())
            {
                self.open_child(index, window, cx);
            }
            return;
        }
        self.active = Some(index);
        let submenu = matches!(self.menu.entries.get(index), Some(Entry::Action(item)) if item.children.is_some());
        if submenu {
            self.open_child(index, window, cx);
        } else {
            for child in self.children.values() {
                child
                    .state
                    .update(cx, |state, cx| state.dismiss(false, window, cx));
            }
            self.focus.focus(window, cx);
        }
        cx.notify();
    }
    fn typeahead(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let key = event
            .keystroke
            .key_char
            .as_ref()
            .unwrap_or(&event.keystroke.key);
        if key.chars().count() != 1 || key.chars().all(char::is_control) {
            return;
        }
        self.hover_delay = None;
        self.pending_hover = None;
        self.submenu_origin = None;
        self.pointer_mode = false;
        self.hovered = None;
        let now = Instant::now();
        if now.saturating_duration_since(self.typed_at) > Duration::from_millis(500) {
            self.typeahead.clear();
        }
        self.typeahead.push_str(&key.to_lowercase());
        self.typed_at = now;
        if let Some((index, _)) = self.menu.entries.iter().enumerate().find(|(_, entry)| matches!(entry, Entry::Action(item) if !item.disabled && item.display(cx).to_lowercase().starts_with(&self.typeahead))) {
            self.active = Some(index); self.scroll.scroll_to_item(index); cx.notify();
        }
    }
    fn row<const S: bool>(
        &self,
        index: usize,
        item: &MenuItem,
        p: Palette,
        cx: &Context<Self>,
    ) -> AnyElement {
        let active = self.highlighted(cx) == Some(index);
        let hover = cx.weak_entity();
        let pick = cx.weak_entity();
        let measured = cx.weak_entity();
        let moved = cx.weak_entity();
        let fg = if item.destructive {
            rgb(0xef4444)
        } else {
            p.text
        };
        let row = Button::<S>::new(item_id(cx.entity().entity_id(), index))
            .ink_ripple(false)
            .ghost()
            .h(px(if item.caption.is_some() {
                44.
            } else if self.menu.card {
                35.
            } else {
                32.
            }))
            .w_full()
            .px(px(if self.menu.card { 8. } else { 10. }))
            .rounded(px(if self.menu.card { 7. } else { 6. }))
            .focusable(false)
            .tab_stop(false)
            .disabled(item.disabled)
            .role(if item.checked.is_some() {
                Role::MenuItemCheckBox
            } else {
                Role::MenuItem
            })
            .accessibility_label(
                item.caption
                    .as_ref()
                    .map(|caption| SharedString::from(format!("{caption}: {}", item.display(cx))))
                    .unwrap_or_else(|| item.display(cx)),
            )
            .when_some(item.checked, |row, checked| row.checked(checked))
            .when(active, |row| row.aria_active_descendant())
            .text_size(px(if self.menu.card { 13. } else { 12. }))
            // Button's tight default line box can clip font descenders in
            // truncated labels. Keep full glyph height within the same row.
            .line_height(relative(1.5))
            .font_weight(if self.menu.card {
                FontWeights::SEMIBOLD
            } else {
                FontWeights::MEDIUM
            })
            .when(
                self.menu.card || self.menu.palette.is_some() || item.destructive,
                |row| {
                    // Apply the surface and semantic role together: a terminal
                    // palette must not replace a red item's hover with neutral ink.
                    let mut colors = super::ButtonCustomVariant::for_surface(p).hover(
                        if item.destructive {
                            rgba(0xef44441f)
                        } else {
                            p.hover
                        }
                        .into(),
                    );
                    if self.menu.card || item.destructive {
                        colors = colors.highlight(rgba(0).into());
                    }
                    row.custom(colors)
                },
            )
            .text_color(fg)
            .bg(if active {
                if item.destructive {
                    rgba(0xef44441f)
                } else {
                    p.hover
                }
            } else {
                rgba(0)
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(if self.menu.card {
                        14.
                    } else if item.image.is_some() {
                        8.
                    } else {
                        10.
                    }))
                    .w_full()
                    .when_some(item.image.clone(), |row, image| {
                        row.child(
                            img(image)
                                .size(px(18.))
                                .object_fit(ObjectFit::Contain)
                                .rounded(px(4.))
                                .flex_shrink_0(),
                        )
                    })
                    .when_some(item.icon.filter(|_| item.image.is_none()), |row, icon| {
                        row.child(crate::assets::icon(icon).size(px(if self.menu.card {
                            17.
                        } else {
                            16.
                        })))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .when_some(item.caption.clone(), |label, caption| {
                                label.child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(p.muted)
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(caption),
                                )
                            })
                            .child(
                                div()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(item.display(cx)),
                            ),
                    )
                    .when_some(
                        item.shortcut.clone().or_else(|| {
                            item.accelerators
                                .last()
                                .map(|key| key.display(cx.platform_info().os).to_string().into())
                        }),
                        |row, hint| {
                            row.child(
                                div()
                                    .ml(px(4.))
                                    .text_size(px(11.))
                                    .text_color(if item.destructive { fg } else { p.muted })
                                    .when(self.menu.card, |badge| {
                                        badge
                                            .h(px(24.))
                                            .min_w(px(38.))
                                            .px(px(7.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded(px(6.))
                                            .bg(if item.destructive {
                                                rgba(0xef3f371f)
                                            } else {
                                                p.hover
                                            })
                                    })
                                    .child(hint),
                            )
                        },
                    )
                    .when(item.checked == Some(true), |row| {
                        row.child(
                            Icon::new(IconName::Check)
                                .size(px(14.))
                                .text_color(p.accent),
                        )
                    })
                    .when(item.children.is_some(), |row| {
                        row.child(Icon::new(IconName::ChevronRight).size(px(16.)))
                    }),
            )
            .on_hover(move |over, window, cx| {
                if *over {
                    let _ = hover.update(cx, |this, cx| this.hovered(index, window, cx));
                } else {
                    let _ = hover.update(cx, |this, cx| this.leave_hover(index, cx));
                }
            })
            .on_mouse_move(move |_, window, cx| {
                let _ = moved.update(cx, |this, cx| this.hovered(index, window, cx));
            })
            .on_prepaint(move |bounds, _, cx| {
                let _ = measured.update(cx, |this, _| {
                    this.row_bounds.insert(index, bounds);
                });
            })
            .on_click(move |event, window, cx| {
                let _ = pick.update(cx, |this, cx| {
                    // Never execute on press or by dragging into a sibling.
                    // A mouse activation must start and end on this same row;
                    // keyboard activation continues to use Activate above.
                    if let ClickEvent::Mouse(event) = event {
                        let Some(bounds) = this.row_bounds.get(&index) else {
                            return;
                        };
                        if event.down.button != MouseButton::Left
                            || !bounds.contains(&event.down.position)
                            || !bounds.contains(&event.up.position)
                        {
                            return;
                        }
                    }
                    this.activate(index, window, cx)
                });
            });
        if let Some(child) = self.child_menu(index) {
            surface(child, Some(Placement::Right), cx)
                .trigger_element(row)
                .into_any_element()
        } else {
            row.into_any_element()
        }
    }
}
pub fn item_id(state: EntityId, index: usize) -> ElementId {
    (
        ElementId::from(("menu-item", state)),
        SharedString::from(index.to_string()),
    )
        .into()
}
fn point_at(x: Pixels, y: Pixels) -> Point<Pixels> {
    point(x, y)
}
fn in_triangle(p: Point<Pixels>, a: Point<Pixels>, b: Point<Pixels>, c: Point<Pixels>) -> bool {
    let cross = |a: Point<Pixels>, b: Point<Pixels>, p: Point<Pixels>| {
        f32::from(b.x - a.x) * f32::from(p.y - a.y) - f32::from(b.y - a.y) * f32::from(p.x - a.x)
    };
    let values = [cross(a, b, p), cross(b, c, p), cross(c, a, p)];
    !values.iter().all(|v| v.abs() < f32::EPSILON)
        && (values.iter().all(|v| *v >= 0.) || values.iter().all(|v| *v <= 0.))
}
fn next(enabled: &[usize], active: Option<usize>, delta: isize) -> Option<usize> {
    if enabled.is_empty() {
        return None;
    }
    let Some(index) = active.and_then(|active| enabled.iter().position(|i| *i == active)) else {
        return if delta < 0 {
            enabled.last()
        } else {
            enabled.first()
        }
        .copied();
    };
    Some(enabled[(index as isize + delta).rem_euclid(enabled.len() as isize) as usize])
}
impl Render for MenuState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.menu.colors(cx);
        let width = self
            .menu
            .width
            .min((f32::from(window.viewport_size().width) - 18.).max(1.));
        let max_height = (f32::from(window.viewport_size().height) - 30.).max(32.);
        let max_height = self
            .menu
            .max_visible_rows
            .filter(|_| self.dropdown)
            .map_or(max_height, |rows| max_height.min(rows as f32 * 32. + 12.));
        // Never leave a trailing separator or an invisible separator Tab stop.
        let count = self.menu.entries.len()
            - usize::from(matches!(self.menu.entries.last(), Some(Entry::Separator)));
        div()
            .id(self
                .menu
                .id
                .clone()
                .unwrap_or_else(|| ("menu-content", cx.entity().entity_id()).into()))
            .on_prepaint({
                let owner = cx.weak_entity();
                move |bounds, _, cx| {
                    let _ = owner.update(cx, |this, _| this.bounds = bounds);
                }
            })
            .role(Role::Menu)
            .key_context("NautermMenu")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .w(px(width - 2.))
            .max_h(px(max_height))
            .on_hover(cx.listener(|this, over: &bool, _, cx| {
                if this.pointer_inside != *over {
                    this.pointer_inside = *over;
                    cx.notify();
                }
                if !*over {
                    this.pending_hover = None;
                    this.hover_delay = None;
                    if let Some(index) = this.hovered {
                        this.leave_hover(index, cx);
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &Next, _, cx| {
                this.navigate(1, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &Previous, _, cx| {
                this.navigate(-1, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &First, _, cx| {
                this.pointer_mode = false;
                this.hovered = None;
                this.hover_delay = None;
                this.pending_hover = None;
                this.active = this.menu.enabled().first().copied();
                if let Some(i) = this.active {
                    this.scroll.scroll_to_item(i);
                }
                cx.notify();
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &Last, _, cx| {
                this.pointer_mode = false;
                this.hovered = None;
                this.hover_delay = None;
                this.pending_hover = None;
                this.active = this.menu.enabled().last().copied();
                if let Some(i) = this.active {
                    this.scroll.scroll_to_item(i);
                }
                cx.notify();
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &Activate, w, cx| {
                if let Some(i) = this.highlighted(cx) {
                    this.activate(i, w, cx);
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &Forward, w, cx| {
                this.pointer_mode = false;
                this.hovered = None;
                this.hover_delay = None;
                this.pending_hover = None;
                if let Some(i) = this.active {
                    this.open_child(i, w, cx);
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &Cancel, w, cx| {
                this.dismiss(true, w, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &Back, w, cx| {
                if this.child {
                    this.dismiss(true, w, cx);
                }
                cx.stop_propagation();
            }))
            .on_key_down(cx.listener(|this, event, _, cx| this.typeahead(event, cx)))
            .child(
                div()
                    .id("menu-rows")
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .scrollbars_with(&self.scroll)
                    .hover_only()
                    .py(px(if self.menu.card { 10. } else { 6. }))
                    .px(px(if self.menu.card { 9. } else { 5. }))
                    .children(self.menu.entries.iter().take(count).enumerate().map(
                        |(i, entry)| {
                            match entry {
                                Entry::Separator => div()
                                    .h(px(if self.menu.card { 13. } else { 9. }))
                                    .flex()
                                    .items_center()
                                    .px(px(if self.menu.card { 0. } else { 5. }))
                                    .child(div().h(px(1.)).w_full().bg(p.border))
                                    .into_any_element(),
                                Entry::Action(item) => {
                                    if self.menu.settings {
                                        self.row::<true>(i, item, p, cx)
                                    } else {
                                        self.row::<false>(i, item, p, cx)
                                    }
                                }
                            }
                        },
                    )),
            )
    }
}
/// Chrome shared by ordinary menus and custom tag/filter dropdowns.
pub(crate) fn dropdown_surface(id: impl Into<ElementId>, p: Palette, dark: bool) -> Overlay {
    Overlay::new(id)
        .duration(motion::MENU)
        .rounded(px(11.))
        .p_0()
        .bg(p.surface)
        .border_color(p.border)
        .shadow(vec![
            gpui_kit::base::box_shadow(
                px(0.),
                px(10.),
                px(22.),
                px(0.),
                rgba(if dark { 0x00000057 } else { 0x0000001f }).into(),
            ),
            gpui_kit::base::box_shadow(
                px(0.),
                px(1.),
                px(3.),
                px(0.),
                rgba(if dark { 0x0000002e } else { 0x0000000d }).into(),
            ),
        ])
}

pub(super) fn surface(state: &Entity<MenuState>, side: Option<Placement>, cx: &App) -> Overlay {
    let this = state.read(cx);
    let focus = this.focus.clone();
    let menu = state.clone();
    let changed = state.downgrade();
    let p = this.menu.colors(cx);
    dropdown_surface(("menu-overlay", state.entity_id()), p, cx.theme().is_dark())
        .open(this.open)
        .content_focus(&focus)
        .when_some(this.origin_focus.clone(), |overlay, focus| {
            overlay.origin_focus(&focus)
        })
        .restore_focus(false)
        .protect_trigger(side.is_some())
        .rounded(px(if this.menu.card { 12. } else { 11. }))
        .when_some(side, |overlay, side| overlay.side(side))
        .when_some(this.position, |overlay, position| {
            overlay.position(position)
        })
        .on_open_change(move |open, window, cx| {
            if !open {
                let _ = changed.update(cx, |this, cx| this.dismiss(false, window, cx));
            }
        })
        .content(move |_, _| menu.clone().into_any_element())
}

pub trait ContextMenuExt:
    InteractiveElement + ParentElement + Styled + IntoElement + Sized + 'static
{
    #[track_caller]
    fn context_menu(
        mut self,
        build: impl Fn(Menu, &mut Window, &mut Context<MenuState>) -> Menu + 'static,
    ) -> ContextMenu<Self> {
        let id = self
            .interactivity()
            .element_id
            .clone()
            .unwrap_or_else(|| ElementId::CodeLocation(*std::panic::Location::caller()));
        ContextMenu {
            element: self,
            id: (id, "context-menu").into(),
            builder: Rc::new(build),
            enabled: None,
        }
    }
}
impl<E: InteractiveElement + ParentElement + Styled + IntoElement + 'static> ContextMenuExt for E {}
#[derive(IntoElement)]
pub struct ContextMenu<E: InteractiveElement + ParentElement + Styled + IntoElement + 'static> {
    element: E,
    id: ElementId,
    builder: Builder,
    enabled: Option<Enabled>,
}
impl<E: InteractiveElement + ParentElement + Styled + IntoElement + 'static> ContextMenu<E> {
    /// Decide at the gesture whether the product menu owns this right click.
    pub fn enabled_when(
        mut self,
        enabled: impl Fn(&mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.enabled = Some(Rc::new(enabled));
        self
    }
}
impl<E: InteractiveElement + ParentElement + Styled + IntoElement + 'static> Styled
    for ContextMenu<E>
{
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}
impl<E: InteractiveElement + ParentElement + Styled + IntoElement + 'static> ParentElement
    for ContextMenu<E>
{
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.element.extend(elements);
    }
}
impl<E: InteractiveElement + ParentElement + Styled + IntoElement + 'static> RenderOnce
    for ContextMenu<E>
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, |_, cx| MenuState::new(cx));
        state.update(cx, |state, _| state.builder = Some(self.builder));
        let open = state.downgrade();
        let element = self
            .element
            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                if window.default_prevented()
                    || self
                        .enabled
                        .as_ref()
                        .is_some_and(|enabled| !enabled(window, cx))
                {
                    return;
                }
                // GPUI dispatches same-element mouse listeners newest first.
                // Claim the context gesture, but let the card's selection/focus
                // listeners finish before building against current application state.
                window.prevent_default();
                let position = event.position + point(px(6.), px(6.));
                let open = open.clone();
                window.defer(cx, move |window, cx| {
                    let _ = open.update(cx, |state, cx| state.open(Some(position), window, cx));
                });
            });
        surface(&state, None, cx).attach_to(element)
    }
}
pub trait DropdownMenu: Sized {
    type Element: IntoElement;
    fn dropdown_menu(
        self,
        build: impl Fn(Menu, &mut Window, &mut Context<MenuState>) -> Menu + 'static,
    ) -> Self::Element;
}
impl<const S: bool> DropdownMenu for Button<S> {
    type Element = Dropdown<S>;
    fn dropdown_menu(
        self,
        build: impl Fn(Menu, &mut Window, &mut Context<MenuState>) -> Menu + 'static,
    ) -> Self::Element {
        Dropdown {
            button: self,
            builder: Rc::new(build),
        }
    }
}
#[derive(IntoElement)]
pub struct Dropdown<const S: bool> {
    button: Button<S>,
    builder: Builder,
}
impl<const S: bool> RenderOnce for Dropdown<S> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = (self.button.element_id(), "dropdown-menu");
        let state = window.use_keyed_state(id, cx, |_, cx| MenuState::new(cx));
        if self.button.inactive() {
            state.update(cx, |state, cx| state.dismiss(false, window, cx));
        }
        state.update(cx, |state, _| {
            state.builder = Some(self.builder);
            state.settings = S;
            state.dropdown = true;
        });
        let open = state.read(cx).is_open();
        let toggle = state.downgrade();
        let trigger = self.button.open(open).on_click(move |_, window, cx| {
            let _ = toggle.update(cx, |state, cx| {
                if state.open {
                    state.dismiss(true, window, cx);
                } else {
                    state.open(None, window, cx);
                }
            });
        });
        surface(&state, Some(Placement::Bottom), cx).trigger_element(trigger)
    }
}

#[cfg(test)]
mod tests {
    use super::{Menu, MenuItem, next};
    #[test]
    fn product_menu_copy_translates_but_user_owned_labels_and_identity_stay_literal() {
        use crate::localization::Language;
        let action = MenuItem::localized("Copy");
        let named = MenuItem::new(String::from("Copy"));
        assert_eq!(action.display_for(Language::Chinese), "复制");
        assert_eq!(action.display_for(Language::English), "Copy");
        assert_eq!(named.display_for(Language::Chinese), "Copy");
        assert_eq!(action.label, named.label);
        let model = MenuItem::new("provider/model:latest".to_owned()).caption("Provider");
        assert_eq!(
            model.display_for(Language::Chinese),
            "provider/model:latest"
        );
        assert_eq!(model.caption.as_deref(), Some("Provider"));
    }
    #[test]
    fn navigation_wraps_and_never_selects_disabled_or_separator_rows() {
        let menu = Menu::default()
            .separator()
            .item(MenuItem::new("A"))
            .separator()
            .item(MenuItem::new("Disabled").disabled(true))
            .item(MenuItem::new("B"));
        assert_eq!(menu.enabled(), vec![0, 3]);
        assert_eq!(next(&menu.enabled(), Some(0), 1), Some(3));
        assert_eq!(next(&menu.enabled(), Some(3), 1), Some(0));
        assert_eq!(next(&menu.enabled(), Some(0), -1), Some(3));
        assert_eq!(next(&menu.enabled(), None, 1), Some(0));
        assert_eq!(next(&menu.enabled(), None, -1), Some(3));
        assert_eq!(next(&[], None, 1), None);
    }
    #[test]
    fn opening_only_highlights_a_checked_enabled_item() {
        let menu = Menu::default().item(MenuItem::new("First")).item(
            MenuItem::new("Checked but disabled")
                .checked(true)
                .disabled(true),
        );
        assert_eq!(menu.initial_active(), None);
        assert_eq!(
            menu.item(MenuItem::new("Current").checked(true))
                .initial_active(),
            Some(2)
        );
    }
}
