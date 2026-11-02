use super::{
    ControlMetrics,
    ink::{self, Highlight, Ink},
    motion::mix,
};
use crate::typography::FontWeights;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::{
    component::{ActiveTheme, Disableable, Icon, Selectable, Sizable, Size, StyledExt},
    prelude::*,
    *,
};
use std::{rc::Rc, time::Instant};
type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type HoverHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

#[derive(Clone, Copy)]
pub struct ButtonCustomVariant {
    color: Rgba,
    foreground: Rgba,
    hover: Rgba,
    active: Rgba,
    highlight: Option<Rgba>,
}
impl ButtonCustomVariant {
    pub fn for_surface(p: crate::palette::Palette) -> Self {
        let pressed = mix(p.hover, p.text, 0.08);
        Self {
            color: rgba(0),
            foreground: p.text,
            hover: p.hover,
            active: pressed,
            highlight: Some(pressed),
        }
    }
    pub fn new(cx: &App) -> Self {
        Self {
            color: rgba(0),
            foreground: cx.theme().foreground.into(),
            hover: rgba(0),
            active: rgba(0),
            highlight: None,
        }
    }
    pub fn color(mut self, v: Hsla) -> Self {
        self.color = v.into();
        self
    }
    pub fn foreground(mut self, v: Hsla) -> Self {
        self.foreground = v.into();
        self
    }
    pub fn hover(mut self, v: Hsla) -> Self {
        self.hover = v.into();
        self
    }
    pub fn active(mut self, v: Hsla) -> Self {
        self.active = v.into();
        self
    }
    /// Override the held highlight independently of the expanding ink color.
    pub fn highlight(mut self, v: Hsla) -> Self {
        self.highlight = Some(v.into());
        self
    }
}
#[derive(Clone, Copy, Default)]
enum Variant {
    #[default]
    Secondary,
    Primary,
    Accent,
    Danger,
    Ghost,
    Custom(ButtonCustomVariant),
}
#[derive(Clone, Copy)]
struct ButtonColors {
    background: Rgba,
    foreground: Rgba,
    hover: Rgba,
    highlight: Rgba,
    splash: Rgba,
}
impl Variant {
    fn colors(self, settings: bool, dark: bool) -> ButtonColors {
        let skin = if settings {
            crate::palette::Palette::settings(dark)
        } else {
            crate::palette::Palette::new(dark)
        };
        if let Self::Custom(v) = self {
            let splash = if v.active.a > 0. {
                v.active
            } else {
                ink::faded(v.foreground, 0.14)
            };
            return ButtonColors {
                background: v.color,
                foreground: v.foreground,
                hover: v.hover,
                highlight: v.highlight.unwrap_or(splash),
                splash,
            };
        }
        let solid = matches!(self, Self::Primary | Self::Accent | Self::Danger);
        let base = match self {
            Self::Primary if !settings => rgb(0x075e92),
            Self::Accent if !settings => rgb(0x168df2),
            Self::Primary | Self::Accent => skin.accent,
            Self::Danger => rgb(if settings { 0xe5453d } else { 0xef4444 }),
            _ => rgb(0x62747b),
        };
        let (hover, splash) = if settings {
            // Settings has its own restrained state colors, not workspace ink.
            if solid {
                (
                    ink::faded(rgb(0xffffff), 0.08),
                    ink::faded(rgb(0xffffff), 0.10),
                )
            } else {
                (ink::faded(skin.text, 0.06), ink::faded(skin.accent, 0.10))
            }
        } else if solid {
            // _WorkspaceButtonPalette.solid: opaque tints of this button's
            // semantic base, including darker (not white) pressed ink.
            (
                mix(base, rgb(0xffffff), 0.08),
                mix(base, rgb(0x000000), 0.08),
            )
        } else if dark {
            (rgb(0x444444), rgb(0x484848))
        } else {
            // Default filled/text controls use neutral gray, never blue accent.
            (
                mix(rgb(0xffffff), base, 0.14),
                mix(rgb(0xffffff), base, 0.22),
            )
        };
        ButtonColors {
            background: if solid {
                base
            } else if matches!(self, Self::Ghost) {
                rgba(0)
            } else {
                skin.control
            },
            foreground: if solid { rgb(0xffffff) } else { skin.text },
            hover,
            highlight: splash,
            splash,
        }
    }
}
struct Interaction {
    focus: FocusHandle,
    hovered: bool,
    pressed: bool,
    key: Option<SharedString>,
    inside: bool,
    bounds: Bounds<Pixels>,
    hover: Highlight,
    press: Highlight,
    ink: Ink,
    activation: Option<u64>,
}
impl Interaction {
    fn origin(&self, position: Point<Pixels>) -> Point<f32> {
        point(
            (f32::from(position.x - self.bounds.left())
                / f32::from(self.bounds.size.width).max(1.))
            .clamp(0., 1.),
            (f32::from(position.y - self.bounds.top())
                / f32::from(self.bounds.size.height).max(1.))
            .clamp(0., 1.),
        )
    }
    fn release(&mut self, confirmed: bool, now: Instant) {
        self.ink.finish(self.activation, confirmed, now);
        self.pressed = false;
        self.key = None;
    }
}

#[cfg(test)]
thread_local! {
    static OBSERVED: std::cell::RefCell<std::collections::HashMap<(WindowId, ElementId), WeakEntity<Interaction>>>
        = std::cell::RefCell::new(std::collections::HashMap::new());
}
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn button_snapshot(
    id: impl Into<ElementId>,
    window: &mut Window,
    cx: &mut App,
) -> (bool, bool, usize) {
    let state = OBSERVED
        .with(|states| {
            states
                .borrow()
                .get(&(window.window_handle().window_id(), id.into()))
                .and_then(WeakEntity::upgrade)
        })
        .expect("Button was not mounted");
    state.update(cx, |s, _| {
        (
            s.pressed,
            s.key.is_some(),
            s.ink.sample(Instant::now(), false).0.len(),
        )
    })
}
/// Our visual button over gpui-base's unstyled interaction/focus primitive.
/// The workspace and settings skins do not inherit Kit component styles.
#[derive(IntoElement)]
pub struct Button<const SETTINGS: bool> {
    base: BaseButton,
    id: ElementId,
    size: Size,
    variant: Variant,
    icon: Option<Icon>,
    trailing_icon: Option<Icon>,
    label: Option<SharedString>,
    label_localized: bool,
    accessible: Option<SharedString>,
    accessible_localized: bool,
    tooltip: Option<SharedString>,
    tooltip_localized: bool,
    children: Vec<AnyElement>,
    hover_action: Option<Box<Self>>,
    list_menu_hint: bool,
    loading: bool,
    disabled: bool,
    selected: bool,
    open: bool,
    outline: bool,
    toolbar: bool,
    content_start: bool,
    icon_size: Option<Pixels>,
    content_gap: Pixels,
    click: Option<Handler>,
    hover_changed: Option<HoverHandler>,
    role: Role,
    checked: Option<bool>,
    focus: Option<FocusHandle>,
    tab_stop: bool,
    focusable: bool,
    ink_ripple: bool,
    card: Option<super::card::Surface>,
    selection_outline: Option<(Rgba, Pixels)>,
    reveal: bool,
}
impl<const S: bool> Button<S> {
    pub(super) fn element_id(&self) -> ElementId {
        self.id.clone()
    }
    pub(super) fn inactive(&self) -> bool {
        self.disabled || self.loading
    }
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: BaseButton::new(id.clone()),
            id,
            size: Size::Medium,
            variant: Variant::default(),
            icon: None,
            trailing_icon: None,
            label: None,
            label_localized: true,
            accessible: None,
            accessible_localized: false,
            tooltip: None,
            tooltip_localized: true,
            children: vec![],
            hover_action: None,
            list_menu_hint: false,
            loading: false,
            disabled: false,
            selected: false,
            open: false,
            outline: false,
            toolbar: false,
            content_start: false,
            icon_size: None,
            content_gap: px(6.),
            click: None,
            hover_changed: None,
            role: Role::Button,
            checked: None,
            focus: None,
            tab_stop: true,
            focusable: true,
            ink_ripple: true,
            card: None,
            selection_outline: None,
            reveal: false,
        }
    }
    pub fn label(mut self, v: impl Into<SharedString>) -> Self {
        self.label = Some(v.into());
        self.label_localized = true;
        self
    }
    /// Resource names, shell labels and other user-owned display values are literal.
    pub fn raw_label(mut self, value: impl Into<SharedString>) -> Self {
        self.label = Some(value.into());
        self.label_localized = false;
        self
    }
    pub fn raw_tooltip(mut self, value: impl Into<SharedString>) -> Self {
        self.tooltip = Some(value.into());
        self.tooltip_localized = false;
        self
    }
    /// Keep an action's name available when a narrow toolbar shows its icon.
    pub fn label_collapsed(mut self, collapsed: bool) -> Self {
        if collapsed
            && self.icon.is_some()
            && let Some(label) = self.label.take()
        {
            self.accessible.get_or_insert_with(|| label.clone());
            self.tooltip.get_or_insert(label);
        }
        self
    }
    /// Reveal a keyboard-selected item once, using its nearest scroll parent.
    pub fn reveal_on_layout(mut self, reveal: bool) -> Self {
        self.reveal = reveal;
        self
    }
    pub fn icon(mut self, v: impl Into<Icon>) -> Self {
        self.icon = Some(v.into());
        self
    }
    pub fn trailing_icon(mut self, v: impl Into<Icon>) -> Self {
        self.trailing_icon = Some(v.into());
        self
    }
    pub fn icon_size(mut self, v: Pixels) -> Self {
        self.icon_size = Some(v);
        self
    }
    pub fn content_gap(mut self, v: Pixels) -> Self {
        self.content_gap = v;
        self
    }
    pub fn tooltip(mut self, v: impl Into<SharedString>) -> Self {
        self.tooltip = Some(v.into());
        self
    }
    pub fn accessibility_label(mut self, v: impl Into<SharedString>) -> Self {
        self.accessible = Some(v.into());
        self.accessible_localized = false;
        self
    }
    pub fn localized_accessibility_label(mut self, v: impl Into<SharedString>) -> Self {
        self.accessible = Some(v.into());
        self.accessible_localized = true;
        self
    }
    pub fn content_start(mut self) -> Self {
        self.content_start = true;
        self
    }
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }
    /// Keep hover/highlight/activation, but omit expanding ink on resizing tabs.
    pub fn ink_ripple(mut self, enabled: bool) -> Self {
        self.ink_ripple = enabled;
        self
    }
    pub(super) fn card_surface(mut self, surface: super::card::Surface) -> Self {
        self.variant = Variant::Custom(surface.colors());
        self.card = Some(surface);
        self
    }
    /// Paint a selection around the full surface without reserving a layout rim.
    pub fn selection_outline(mut self, color: Rgba, width: Pixels) -> Self {
        self.selection_outline = Some((color, width));
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.click = Some(Rc::new(f));
        self
    }
    pub fn on_hover(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.hover_changed = Some(Rc::new(f));
        self
    }
    /// Reserve an action's space while fading it with this surface's hover.
    /// Hidden actions cannot receive pointer/keyboard activation.
    pub fn hover_action(mut self, action: Self) -> Self {
        self.hover_action = Some(Box::new(action));
        self
    }
    /// List rows use a passive menu hint; the whole row owns the context menu.
    pub fn list_menu_hint(mut self, enabled: bool) -> Self {
        self.list_menu_hint = enabled;
        self
    }
    pub fn loading(mut self, v: bool) -> Self {
        self.loading = v;
        self
    }
    pub fn toolbar(mut self) -> Self {
        self.size = Size::XSmall;
        self.toolbar = true;
        self.base = self.base.flex_shrink_0();
        self
    }
    pub fn primary(mut self) -> Self {
        self.variant = Variant::Primary;
        self
    }
    pub fn secondary(mut self) -> Self {
        self.variant = Variant::Secondary;
        self
    }
    pub fn accent(mut self) -> Self {
        self.variant = Variant::Accent;
        self
    }
    pub fn danger(mut self) -> Self {
        self.variant = Variant::Danger;
        self
    }
    pub fn ghost(mut self) -> Self {
        self.variant = Variant::Ghost;
        self
    }
    pub fn custom(mut self, v: ButtonCustomVariant) -> Self {
        self.variant = Variant::Custom(v);
        self
    }
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }
    pub fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }
    pub fn checked(mut self, value: bool) -> Self {
        self.checked = Some(value);
        self
    }
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }
    pub fn tab_stop(mut self, value: bool) -> Self {
        self.tab_stop = value;
        self
    }
}
impl<const S: bool> Styled for Button<S> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}
impl<const S: bool> InteractiveElement for Button<S> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}
impl<const S: bool> StatefulInteractiveElement for Button<S> {}
impl<const S: bool> ParentElement for Button<S> {
    fn extend(&mut self, v: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(v);
    }
}
impl<const S: bool> Sizable for Button<S> {
    fn with_size(mut self, v: impl Into<Size>) -> Self {
        self.size = v.into();
        self
    }
}
impl<const S: bool> Disableable for Button<S> {
    fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
}
impl<const S: bool> Selectable for Button<S> {
    fn selected(mut self, v: bool) -> Self {
        self.selected = v;
        self
    }
    fn is_selected(&self) -> bool {
        self.selected
    }
    fn open(mut self, v: bool) -> Self {
        self.open = v;
        self
    }
    fn is_open(&self) -> bool {
        self.open
    }
}
impl<const S: bool> RenderOnce for Button<S> {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.label_localized {
            self.label = self
                .label
                .take()
                .map(|label| crate::localization::text(cx, &label));
        }
        if self.tooltip_localized {
            self.tooltip = self
                .tooltip
                .take()
                .map(|tooltip| crate::localization::text(cx, &tooltip));
        }
        if self.accessible_localized {
            self.accessible = self
                .accessible
                .take()
                .map(|label| crate::localization::label(cx, &label));
        }
        let mut m = ControlMetrics::button(self.size);
        if self.toolbar {
            m.height = 30.;
            m.padding = 10.;
        }
        if S && !self.toolbar {
            m = ControlMetrics::settings_button();
        }
        let mut overrides = std::mem::take(self.base.style());
        let inert = self.disabled || self.loading;
        let state = window.use_keyed_state(self.id.clone(), cx, |window, cx| {
            let owner = cx.entity_id();
            let window_id = window.window_handle().window_id();
            cx.on_release(move |_, cx| super::tooltip::hide_owner(window_id, owner, cx))
                .detach();
            Interaction {
                focus: cx.focus_handle(),
                hovered: false,
                pressed: false,
                key: None,
                inside: false,
                bounds: Bounds::default(),
                hover: Highlight::new(),
                press: Highlight::new(),
                ink: Ink::default(),
                activation: None,
            }
        });
        #[cfg(test)]
        OBSERVED.with(|states| {
            states.borrow_mut().insert(
                (window.window_handle().window_id(), self.id.clone()),
                state.downgrade(),
            );
        });
        let reduced = cx.reduce_motion();
        let reduce_ink = reduced || !self.ink_ripple;
        let focus = self
            .focus
            .unwrap_or_else(|| state.read(cx).focus.clone())
            .tab_stop(self.tab_stop);
        let (hover, press, samples, moving) = state.update(cx, |s, _| {
            let now = Instant::now();
            if inert || !window.is_window_active() {
                s.hovered = false;
                s.release(false, now);
                s.ink.clear();
                s.activation = None;
            }
            if !focus.is_focused(window) && s.key.is_some() {
                s.release(false, now);
            }
            let (h, a) = s.hover.set(
                f32::from(!inert && (s.hovered || self.open)),
                now,
                if self.card.is_some() {
                    super::card::TRANSITION
                } else {
                    ink::HOVER
                },
                reduced,
            );
            let (p, b) = s.press.set(
                f32::from(!inert && ((s.pressed && s.inside) || s.key.is_some())),
                now,
                ink::HIGHLIGHT,
                reduced,
            );
            let (samples, c) = s.ink.sample(now, reduce_ink);
            (h, p, samples, a || b || c)
        });
        if moving {
            window.request_animation_frame();
        }
        let action_visible = !inert && state.read(cx).hovered;
        if let Some(surface) = self.card {
            overrides.box_shadow = Some(surface.shadows(hover));
        }
        let theme = cx.theme();
        let dark = theme.is_dark();
        let colors = self.variant.colors(S, dark);
        // A caller's selected/card/tab background remains the base surface.
        // Ink is alpha-composited over it, never mixed toward unrelated tokens.
        let bg = overrides
            .background
            .as_ref()
            .and_then(Fill::color)
            .and_then(|v| v.as_solid())
            .map(Rgba::from)
            .unwrap_or(colors.background);
        overrides.background = None;
        let rem = window.rem_size();
        let corners = Corners {
            top_left: overrides
                .corner_radii
                .top_left
                .unwrap_or(px(7.).into())
                .to_pixels(rem),
            top_right: overrides
                .corner_radii
                .top_right
                .unwrap_or(px(7.).into())
                .to_pixels(rem),
            bottom_right: overrides
                .corner_radii
                .bottom_right
                .unwrap_or(px(7.).into())
                .to_pixels(rem),
            bottom_left: overrides
                .corner_radii
                .bottom_left
                .unwrap_or(px(7.).into())
                .to_pixels(rem),
        };
        let content = self.label.is_some() || !self.children.is_empty();
        let icon_size =
            self.icon_size
                .unwrap_or(px(if self.size == Size::XSmall { 16. } else { 18. }));
        let name = self
            .accessible
            .or_else(|| self.label.clone())
            .or_else(|| self.tooltip.clone());
        let hover_action = self.hover_action.filter(|_| !self.list_menu_hint);
        let has_hover_action = hover_action.is_some();
        let children = if (has_hover_action || self.list_menu_hint) && !self.children.is_empty() {
            // Keep avatar/text spacing independent of the trailing affordance.
            vec![
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(self.content_gap)
                    .children(self.children)
                    .into_any_element(),
            ]
        } else {
            self.children
        };
        let mut base = self
            .base
            .disabled(inert)
            .focusable(self.focusable)
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .line_height(relative(1.))
            .h(px(m.height))
            .px(px(if content { m.padding } else { 0. }))
            .when(!content, |b| b.w(px(m.height)))
            .rounded(px(7.))
            .bg(bg)
            .text_color(colors.foreground)
            .text_size(px(m.font))
            .font_weight(FontWeights::MEDIUM)
            .role(self.role)
            .when_some(name, |b, n| b.aria_label(n))
            .when_some(self.checked, |b, v| {
                b.aria_toggled(if v {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                })
            })
            .when(self.selected, |b| b.aria_selected(true))
            .when(self.open, |b| b.aria_expanded(true))
            .when(self.outline, |b| b.border_1().border_color(theme.border))
            .when(!inert, |b| {
                b.cursor_pointer().when(self.focusable, |b| {
                    b.track_focus(&focus)
                        .focus_visible(|s| s.border_1().border_color(theme.ring))
                })
            })
            .when(inert, |b| b.opacity(0.45).cursor_default())
            .refine_style(&overrides);
        let hover_changed = self.hover_changed;
        let tooltip_owner = state.entity_id();
        let has_tooltip = self.tooltip.is_some();
        if inert || !window.is_window_active() || !has_tooltip {
            super::tooltip::hide_owner(window.window_handle().window_id(), tooltip_owner, cx);
        }
        let tooltip = self.tooltip;
        if !inert {
            let hover_changed = hover_changed.clone();
            let h = state.clone();
            let d = state.clone();
            let kd = state.clone();
            let ku = state.clone();
            let click_state = state.clone();
            let pointer_focus = focus.clone();
            let key_focus = focus.clone();
            let focusable = self.focusable;
            base = base
                .on_hover(move |v, w, cx| {
                    let (effective, changed, bounds) = h.update(cx, |s, cx| {
                        // Base hover temporarily reports false while a pointer
                        // is down; that is not a real pointer exit.
                        let effective = *v || (s.pressed && s.inside);
                        let changed = s.hovered != effective;
                        s.hovered = effective;
                        if changed {
                            cx.notify();
                        }
                        (effective, changed, s.bounds)
                    });
                    if changed && let Some(text) = &tooltip {
                        super::tooltip::hover(
                            tooltip_owner,
                            text.clone(),
                            bounds,
                            effective,
                            w,
                            cx,
                        );
                    }
                    if changed && let Some(callback) = &hover_changed {
                        callback(&effective, w, cx);
                    }
                })
                .on_mouse_down(MouseButton::Left, move |event, w, cx| {
                    if focusable {
                        pointer_focus.focus(w, cx);
                    }
                    let reduced = cx.reduce_motion() || reduce_ink;
                    d.update(cx, |s, cx| {
                        s.release(false, Instant::now());
                        s.activation =
                            s.ink
                                .begin(s.origin(event.position), Instant::now(), reduced);
                        s.pressed = true;
                        s.inside = true;
                        cx.notify();
                    });
                    // The deepest button owns the press, not its card/ancestor.
                    // GPUI has already recorded this node's pending click.
                    cx.stop_propagation();
                })
                .capture_key_down(move |event, w, cx| {
                    if !key_focus.is_focused(w) {
                        return;
                    }
                    let activation = matches!(event.keystroke.key.as_str(), "enter" | "space")
                        && !event.keystroke.modifiers.modified();
                    let reduced = cx.reduce_motion() || reduce_ink;
                    kd.update(cx, |s, cx| {
                        if activation && s.key.is_none() && !event.is_held {
                            s.activation = s.ink.begin(point(0.5, 0.5), Instant::now(), reduced);
                            s.key = Some(event.keystroke.key.clone().into());
                        } else if !activation {
                            s.release(false, Instant::now());
                        }
                        cx.notify();
                    });
                })
                .capture_key_up(move |_, _, cx| {
                    ku.update(cx, |s, cx| {
                        if s.key.is_some() {
                            s.release(false, Instant::now());
                            cx.notify();
                        }
                    });
                })
                .when_some(self.click, |b, callback| {
                    b.on_click(move |event, w, cx| {
                        let reduced = cx.reduce_motion() || reduce_ink;
                        click_state.update(cx, |s, cx| {
                            if s.activation.is_none() {
                                let origin = match event {
                                    ClickEvent::Keyboard(_) => point(0.5, 0.5),
                                    _ => s.origin(event.position()),
                                };
                                s.activation = s.ink.begin(origin, Instant::now(), reduced);
                            }
                            s.release(true, Instant::now());
                            s.activation = None;
                            cx.notify();
                        });
                        cx.stop_propagation();
                        callback(event, w, cx);
                    })
                });
        } else {
            base = base
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| cx.stop_propagation());
        }
        let measure = state.clone();
        let events = state.downgrade();
        let selection_outline = self.selection_outline.or_else(|| {
            self.card
                .filter(|surface| surface.selected)
                .map(|surface| (crate::palette::Palette::new(surface.dark).accent, px(2.)))
        });
        let reveal = std::cell::Cell::new(self.reveal);
        let overlay = div().absolute().inset_0().child(
            canvas(
                move |bounds, window, cx| {
                    measure.update(cx, |s, _| s.bounds = bounds);
                    if has_tooltip {
                        super::tooltip::anchor_changed(
                            window.window_handle().window_id(),
                            tooltip_owner,
                            bounds,
                            cx,
                        );
                    }
                    if reveal.replace(false) {
                        window.request_autoscroll(bounds);
                    }
                },
                move |bounds, _, window, _| {
                    for (color, opacity) in [(colors.hover, hover), (colors.highlight, press)] {
                        if opacity > 0. {
                            let mut quad = fill(bounds, ink::faded(color, opacity));
                            quad.corner_radii = corners;
                            window.paint_quad(quad);
                        }
                    }
                    for sample in samples {
                        ink::paint(bounds, corners, sample, colors.splash, window);
                    }
                    if let Some((color, width)) = selection_outline {
                        let mut border = outline(bounds, color, BorderStyle::Solid);
                        border.corner_radii = corners;
                        border.border_widths = width.into();
                        window.paint_quad(border);
                    }
                    let up = events.clone();
                    let up_hover = hover_changed.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                            let changed = up.update(cx, |s, cx| {
                                if s.pressed {
                                    let inside = s.bounds.contains(&event.position);
                                    let changed = s.hovered != inside;
                                    s.inside = inside;
                                    s.hovered = inside;
                                    s.release(inside, Instant::now());
                                    cx.notify();
                                    return changed.then_some(inside);
                                }
                                None
                            });
                            if let Ok(Some(hovered)) = changed
                                && let Some(callback) = &up_hover
                            {
                                callback(&hovered, window, cx);
                            }
                        }
                    });
                    let moved = events.clone();
                    let move_hover = hover_changed.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture {
                            let changed = moved.update(cx, |s, cx| {
                                if s.pressed {
                                    let inside = s.bounds.contains(&event.position);
                                    let changed = s.hovered != inside;
                                    if !inside {
                                        s.ink.finish(s.activation, false, Instant::now());
                                    }
                                    if inside != s.inside || changed {
                                        s.inside = inside;
                                        s.hovered = inside;
                                        cx.notify();
                                    }
                                    return changed.then_some(inside);
                                }
                                None
                            });
                            if let Ok(Some(hovered)) = changed
                                && let Some(callback) = &move_hover
                            {
                                callback(&hovered, window, cx);
                            }
                        }
                    });
                    let exited = events.clone();
                    let exit_hover = hover_changed.clone();
                    window.on_mouse_event(move |_: &MouseExitEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture {
                            let changed = exited.update(cx, |s, cx| {
                                let hovered = s.hovered;
                                if s.pressed || s.hovered {
                                    s.hovered = false;
                                    s.inside = false;
                                    s.release(false, Instant::now());
                                    cx.notify();
                                }
                                hovered
                            });
                            if matches!(changed, Ok(true))
                                && let Some(callback) = &exit_hover
                            {
                                callback(&false, window, cx);
                            }
                        }
                    });
                },
            )
            .size_full(),
        );
        base.child(overlay).child(
            div()
                .size_full()
                .min_w_0()
                .flex()
                .items_center()
                .when(self.content_start, |v| v.justify_start())
                .when(!self.content_start, |v| v.justify_center())
                .gap(if self.list_menu_hint {
                    px(12.)
                } else if has_hover_action {
                    px(6.)
                } else {
                    self.content_gap
                })
                .when(self.loading, |v| {
                    v.child(super::spinner::Spinner::new().with_size(icon_size))
                })
                .when_some(self.icon.filter(|_| !self.loading), |v, i| {
                    // Labels may be clipped during resize. Never let flex
                    // compress the leading icon along with them.
                    v.child(i.with_size(icon_size).flex_shrink_0())
                })
                .when_some(self.label, |v, l| {
                    v.child(div().min_w_0().whitespace_nowrap().text_ellipsis().child(l))
                })
                .children(children)
                .when(self.list_menu_hint, |content| {
                    content.child(
                        Icon::new(gpui_kit::assets::IconName::Ellipsis)
                            .with_size(px(18.))
                            .flex_shrink_0()
                            .text_color(mix(rgb(0xb0c0c6), rgb(0x71848c), hover)),
                    )
                })
                .when_some(hover_action, |content, action| {
                    let disabled = action.disabled || !action_visible;
                    content.child(
                        div()
                            .flex_shrink_0()
                            .opacity(hover)
                            .when(hover <= 0., |view| view.invisible())
                            .child((*action).focusable(false).disabled(disabled).opacity(1.)),
                    )
                })
                .when_some(self.trailing_icon, |v, i| {
                    v.child(i.with_size(icon_size).ml_auto())
                }),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::{ButtonCustomVariant, Variant};
    use crate::components::Button;
    use gpui_kit::{
        Rgba,
        assets::IconName,
        component::{Sizable, Size},
        px, rgb, rgba,
    };
    #[test]
    fn compound_buttons_share_alignment_and_icon_metrics() {
        let b = Button::new("navigation")
            .large()
            .content_start()
            .icon_size(px(16.))
            .trailing_icon(IconName::ChevronDown)
            .content_gap(px(20.));
        assert!(b.content_start);
        assert_eq!(b.size, Size::Large);
        assert_eq!(b.icon_size, Some(px(16.)));
        assert!(b.trailing_icon.is_some());
        assert_eq!(b.content_gap, px(20.));
    }

    fn near_color(actual: Rgba, expected: u32) {
        let expected = rgb(expected);
        for (a, b) in [
            (actual.r, expected.r),
            (actual.g, expected.g),
            (actual.b, expected.b),
        ] {
            assert!((a - b).abs() <= 0.5 / 255., "{actual:?} != {expected:?}");
        }
        assert_eq!(actual.a, 1.);
    }

    #[test]
    fn workspace_neutral_buttons_use_gray_reference_ink_in_both_themes() {
        for variant in [Variant::Secondary, Variant::Ghost] {
            let light = variant.colors(false, false);
            near_color(light.hover, 0xe9eced);
            near_color(light.splash, 0xdce0e2);
            assert_eq!(light.highlight, light.splash);
            let dark = variant.colors(false, true);
            near_color(dark.hover, 0x444444);
            near_color(dark.splash, 0x484848);
            assert_eq!(dark.highlight, dark.splash);
        }
    }

    #[test]
    fn workspace_solid_ink_darkens_its_semantic_base_instead_of_whitening() {
        for dark in [false, true] {
            for (variant, base, hover, pressed) in [
                (Variant::Primary, 0x075e92, 0x1b6b9b, 0x065686),
                (Variant::Accent, 0x168df2, 0x2996f3, 0x1482df),
                (Variant::Danger, 0xef4444, 0xf05353, 0xdc3f3f),
            ] {
                let colors = variant.colors(false, dark);
                near_color(colors.background, base);
                near_color(colors.hover, hover);
                near_color(colors.splash, pressed);
                assert_eq!(colors.highlight, colors.splash);
            }
        }
    }

    #[test]
    fn workspace_color_correction_does_not_change_settings_feedback() {
        for dark in [false, true] {
            let p = crate::palette::Palette::settings(dark);
            for variant in [Variant::Secondary, Variant::Ghost] {
                let colors = variant.colors(true, dark);
                assert_eq!(colors.hover, super::ink::faded(p.text, 0.06));
                assert_eq!(colors.splash, super::ink::faded(p.accent, 0.10));
                assert_eq!(colors.highlight, colors.splash);
            }
            for variant in [Variant::Primary, Variant::Accent, Variant::Danger] {
                let colors = variant.colors(true, dark);
                assert_eq!(colors.hover, super::ink::faded(rgb(0xffffff), 0.08));
                assert_eq!(colors.splash, super::ink::faded(rgb(0xffffff), 0.10));
                assert_eq!(colors.highlight, colors.splash);
            }
        }
    }

    #[test]
    fn custom_held_highlight_is_independent_of_splash_including_transparent() {
        let custom = ButtonCustomVariant {
            color: rgba(0),
            foreground: rgb(0x151927),
            hover: rgb(0xedf4f5),
            active: rgb(0xc7d5d8),
            highlight: None,
        };
        let default = Variant::Custom(custom).colors(false, false);
        assert_eq!(default.highlight, default.splash);
        for alpha in [0., 0.58] {
            let colors = Variant::Custom(custom.highlight(custom.active.alpha(alpha).into()))
                .colors(false, false);
            assert_eq!(colors.splash, custom.active);
            assert!((colors.highlight.a - alpha).abs() < 0.00001);
        }
    }
}
