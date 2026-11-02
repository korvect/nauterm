//! Nauterm's Input presentation over gpui-base's retained editing/IME engine.
//! Multiline is a mode of this control, never a separately styled Textarea.
use super::{
    ControlMetrics,
    button::Button,
    menu::{self, Menu, MenuItem, MenuState},
    motion::{self, Transition},
};
use crate::palette::Palette;
use gpui_kit::base::input::{
    Copy, Cut, Input as BaseText, InputContextMenuCapabilities, InputEditorStyle, InputFilter,
    InputPresentation, InputState, NativeMenu, NativeMenuItem, Paste, SelectAll,
    Textarea as BaseMultiline, TextareaState,
};
use gpui_kit::base::{InputBase as BaseFrame, StyledExt};
use gpui_kit::{
    assets::IconName,
    component::{ActiveTheme, Disableable, Sizable, Size},
    prelude::*,
    *,
};
use std::{rc::Rc, time::Instant};

#[derive(Clone)]
enum InputType {
    Text(Entity<InputState>),
    Multiline(Entity<TextareaState>),
}
macro_rules! dispatch {
    ($self:expr,|$state:ident|$body:expr) => {
        match $self {
            InputType::Text($state) => $body,
            InputType::Multiline($state) => $body,
        }
    };
}
impl InputType {
    fn id(&self) -> EntityId {
        dispatch!(self, |state| state.entity_id())
    }
    fn presentation(&self, cx: &App) -> InputPresentation {
        dispatch!(self, |state| state.read(cx).presentation())
    }
    fn set_placeholder(&self, placeholder: SharedString, window: &mut Window, cx: &mut App) {
        dispatch!(self, |state| state.update(cx, |state, cx| {
            state.set_placeholder(placeholder.clone(), window, cx)
        }));
    }
    fn nonempty(&self, cx: &App) -> bool {
        dispatch!(self, |state| state.read(cx).text().len() > 0)
    }
    fn public_value(&self, cx: &App) -> String {
        dispatch!(self, |state| state.read(cx).text().to_string())
    }
    fn focus(&self, window: &mut Window, cx: &mut App) {
        dispatch!(self, |state| state
            .update(cx, |state, cx| state.focus(window, cx)));
    }
    fn clear(&self, window: &mut Window, cx: &mut App) {
        dispatch!(self, |state| state.update(cx, |state, cx| {
            if state.is_editable() {
                state.replace_all("", window, cx);
                state.focus(window, cx);
            }
        }));
    }
    fn set_accessible_value(
        &self,
        data: Option<&accesskit::ActionData>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(accesskit::ActionData::Value(value)) = data {
            dispatch!(self, |state| state.update(cx, |state, cx| {
                if state.is_editable() {
                    state.replace_all(value.to_string(), window, cx);
                }
            }));
        }
    }
    fn toggle_mask(&self, window: &mut Window, cx: &mut App) {
        if let Self::Text(state) = self {
            state.update(cx, |state, cx| state.toggle_masked(window, cx));
        }
    }
    fn has_selection_focus(&self, window: &Window, cx: &App) -> bool {
        dispatch!(self, |state| state.read(cx).has_selection_focus(window, cx))
    }
    fn configure(
        &self,
        policy: Policy,
        style: InputEditorStyle,
        align: TextAlign,
        menu: &Entity<MenuState>,
        settings: bool,
        cx: &mut App,
    ) {
        dispatch!(self, |state| {
            let editor = state.downgrade();
            let menu = menu.downgrade();
            state.update(cx, |state, cx| {
                state.set_disabled(policy.disabled, cx);
                state.set_readonly(policy.readonly, cx);
                state.set_text_align(align, cx);
                state.set_editor_style(style);
                state.set_input_filter(if policy.digits_only {
                    InputFilter::AsciiDigits
                } else {
                    InputFilter::None
                });
                // Padding belongs to our frame, including the multiline viewport.
                state.set_editor_paddings(Edges::default());
                state.on_context_menu(Rc::new(move |model, capabilities, position, window, cx| {
                    let (Some(input), Some(host)) = (editor.upgrade(), menu.upgrade()) else {
                        return;
                    };
                    let origin = input.read(cx).focus_handle(cx);
                    let model = context_menu(model, capabilities);
                    let mut content = Menu::default()
                        .id(("input-menu-content", input.entity_id()))
                        .width(220.)
                        .settings(settings);
                    for item in model.items {
                        content = match item {
                            NativeMenuItem::Separator => content.separator(),
                            NativeMenuItem::Action {
                                label,
                                disabled,
                                action,
                            } => {
                                let target = editor.clone();
                                let mut item = MenuItem::new(crate::localization::text(cx, &label))
                                    .disabled(disabled);
                                let command = policy
                                    .terminal_shortcuts
                                    .then(|| terminal_editing_command(action.as_ref()))
                                    .flatten();
                                if let Some(command) = command {
                                    item = crate::shortcuts::current(cx).menu(item, command);
                                } else {
                                    for binding in
                                        window.bindings_for_action_in(action.as_ref(), &origin)
                                    {
                                        if let [key] = binding.keystrokes() {
                                            item = item.accelerator(key.clone());
                                        }
                                    }
                                }
                                content.item(item.on_click(move |_, window, cx| {
                                    let Some(input) = target.upgrade() else {
                                        return;
                                    };
                                    // Permissions/selection can change while the menu is open.
                                    // Never dispatch against a different editor or stale policy.
                                    if action_allowed(
                                        input.read(cx).context_menu_capabilities(),
                                        action.as_ref(),
                                    ) {
                                        let focus = input.read(cx).focus_handle(cx);
                                        focus.focus(window, cx);
                                        focus.dispatch_action(action.as_ref(), window, cx);
                                    }
                                }))
                            }
                        };
                    }
                    let focus = host.read(cx).focus_handle(cx);
                    input.update(cx, |input, cx| input.set_selection_focus(Some(focus), cx));
                    host.update(cx, |host, cx| {
                        host.show_at(content, position, origin, window, cx)
                    });
                }));
            });
        });
    }
    fn editor(&self) -> AnyElement {
        match self {
            Self::Text(state) => BaseText::new(state).into_any_element(),
            Self::Multiline(state) => BaseMultiline::new(state).into_any_element(),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Policy {
    disabled: bool,
    readonly: bool,
    sensitive: bool,
    multiline: bool,
    digits_only: bool,
    terminal_shortcuts: bool,
}
impl Policy {
    fn editable(self) -> bool {
        !self.disabled && !self.readonly
    }
    fn clear_visible(self, requested: bool, focused: bool, nonempty: bool, loading: bool) -> bool {
        requested && focused && nonempty && self.editable() && !self.sensitive && !loading
    }
    fn exposes_value(self, masked: bool) -> bool {
        !self.sensitive && !masked
    }
    fn role(self) -> Role {
        if self.sensitive {
            Role::PasswordInput
        } else if self.multiline {
            Role::MultilineTextInput
        } else {
            Role::TextInput
        }
    }
}
fn context_menu(menu: NativeMenu, capabilities: InputContextMenuCapabilities) -> NativeMenu {
    let enabled = !capabilities.is_disabled();
    let copy = enabled && capabilities.is_copyable();
    menu.menu_with_disabled("Cut", !(capabilities.is_editable() && copy), Box::new(Cut))
        .menu_with_disabled("Copy", !copy, Box::new(Copy))
        .menu_with_disabled("Paste", !capabilities.is_editable(), Box::new(Paste))
        .separator()
        .menu_with_disabled("Select All", !enabled, Box::new(SelectAll))
}
fn action_allowed(capabilities: InputContextMenuCapabilities, action: &dyn Action) -> bool {
    if capabilities.is_disabled() {
        return false;
    }
    if action.as_any().is::<Cut>() {
        capabilities.is_editable() && capabilities.is_copyable()
    } else if action.as_any().is::<Copy>() {
        capabilities.is_copyable()
    } else if action.as_any().is::<Paste>() {
        capabilities.is_editable()
    } else {
        action.as_any().is::<SelectAll>()
    }
}

fn terminal_editing_command(action: &dyn Action) -> Option<crate::shortcuts::Command> {
    use crate::shortcuts::Command;
    if action.as_any().is::<Copy>() {
        Some(Command::Copy)
    } else if action.as_any().is::<Paste>() {
        Some(Command::Paste)
    } else if action.as_any().is::<SelectAll>() {
        Some(Command::SelectAll)
    } else {
        None
    }
}

#[derive(IntoElement)]
pub struct Input<const SETTINGS: bool> {
    state: InputType,
    palette: Option<Palette>,
    caret_color: Option<Rgba>,
    editor_cursor: Option<CursorStyle>,
    style: StyleRefinement,
    size: Size,
    invalid: bool,
    appearance: bool,
    bordered: bool,
    disabled: bool,
    readonly: bool,
    focused: bool,
    cleanable: bool,
    mask_toggle: bool,
    digits_only: bool,
    terminal_shortcuts: bool,
    label: Option<SharedString>,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
}
impl<const S: bool> Input<S> {
    fn from_type(state: InputType) -> Self {
        Self {
            state,
            palette: None,
            caret_color: None,
            editor_cursor: None,
            style: StyleRefinement::default(),
            size: Size::Medium,
            invalid: false,
            appearance: true,
            bordered: true,
            disabled: false,
            readonly: false,
            focused: false,
            cleanable: false,
            mask_toggle: false,
            digits_only: false,
            terminal_shortcuts: false,
            label: None,
            prefix: None,
            suffix: None,
        }
    }
    pub fn new(state: &Entity<InputState>) -> Self {
        Self::from_type(InputType::Text(state.clone()))
    }
    pub fn digits_only(mut self) -> Self {
        self.digits_only = true;
        self
    }
    /// The terminal owns copy/paste/select-all bindings even while its composer
    /// or search editor has focus. Its menu must advertise the same chords.
    pub fn terminal_shortcuts(mut self) -> Self {
        self.terminal_shortcuts = true;
        self
    }
    /// Terminal colors can differ from the app. Project them into the retained
    /// editor too, rather than changing only the frame's background.
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = Some(palette);
        self
    }
    /// Compact terminal-tool skin shared by text, multiline and Select editors.
    pub fn terminal_control(self, palette: Palette) -> Self {
        let multiline = matches!(self.state, InputType::Multiline(_));
        self.small()
            .palette(palette)
            .bg(palette.control)
            .px(px(10.))
            .text_size(px(11.5))
            .line_height(px(18.))
            .font_weight(crate::typography::FontWeights::MEDIUM)
            .when(!multiline, |input| input.h(px(34.)))
            .when(multiline, |input| input.py(px(8.)))
    }
    pub fn caret_color(mut self, color: Rgba) -> Self {
        self.caret_color = Some(color);
        self
    }
    /// Set only the text viewport's cursor; affixes keep their own pointer.
    pub fn editor_cursor(mut self, cursor: CursorStyle) -> Self {
        self.editor_cursor = Some(cursor);
        self
    }
    pub fn multiline(state: &Entity<TextareaState>) -> Self {
        Self::from_type(InputType::Multiline(state.clone()))
    }
    pub fn h(self, height: impl Into<DefiniteLength>) -> Self {
        Styled::h(self, height.into())
    }
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
    pub fn appearance(mut self, value: bool) -> Self {
        self.appearance = value;
        self
    }
    pub fn bordered(mut self, value: bool) -> Self {
        self.bordered = value;
        self
    }
    pub fn readonly(mut self, value: bool) -> Self {
        self.readonly = value;
        self
    }
    /// An open select keeps its active frame while its overlay owns focus.
    pub fn focused(mut self, value: bool) -> Self {
        self.focused = value;
        self
    }
    pub fn cleanable(mut self, value: bool) -> Self {
        self.cleanable = value;
        self
    }
    pub fn mask_toggle(mut self) -> Self {
        self.mask_toggle = true;
        self
    }
    pub fn invalid(mut self, value: bool) -> Self {
        self.invalid = value;
        self
    }
    pub fn prefix(mut self, element: impl IntoElement) -> Self {
        self.prefix = Some(element.into_any_element());
        self
    }
    pub fn suffix(mut self, element: impl IntoElement) -> Self {
        self.suffix = Some(element.into_any_element());
        self
    }
}
impl<const S: bool> Styled for Input<S> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl<const S: bool> Sizable for Input<S> {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}
impl<const S: bool> Disableable for Input<S> {
    fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
}

impl<const S: bool> RenderOnce for Input<S> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let dark = cx.theme().is_dark();
        let p = self.palette.unwrap_or_else(|| {
            if S {
                Palette::settings(dark)
            } else {
                Palette::new(dark)
            }
        });
        let metrics = if S {
            ControlMetrics::settings_input(self.size)
        } else {
            ControlMetrics::input(self.size)
        };
        let multiline = matches!(self.state, InputType::Multiline(_));
        let fixed_height = !matches!(self.style.size.height, None | Some(Length::Auto));
        let policy = Policy {
            disabled: self.disabled,
            readonly: self.readonly,
            sensitive: self.mask_toggle,
            multiline,
            digits_only: self.digits_only && !multiline,
            terminal_shortcuts: self.terminal_shortcuts,
        };
        let before = self.state.presentation(cx);
        let policy = Policy {
            sensitive: policy.sensitive || before.is_masked(),
            ..policy
        };
        let editor_style = InputEditorStyle {
            foreground: p.text.into(),
            muted_foreground: if S {
                Palette::settings_faint(dark)
            } else {
                p.muted.opacity(0.55)
            }
            .into(),
            background: p.surface.into(),
            border: p.border.into(),
            caret: caret(self.caret_color.unwrap_or(p.accent)),
            selection: p.accent.opacity(0.3).into(),
            ..Default::default()
        };
        let context_menu = window.use_keyed_state(("input-menu", self.state.id()), cx, |_, cx| {
            MenuState::new(cx)
        });
        self.state.configure(
            policy,
            editor_style,
            self.style.text.text_align.unwrap_or(TextAlign::Left),
            &context_menu,
            S,
            cx,
        );
        let placeholder = self.state.presentation(cx).placeholder().clone();
        let translated_placeholder = crate::localization::text(cx, &placeholder);
        if translated_placeholder != placeholder {
            self.state
                .set_placeholder(translated_placeholder, window, cx);
        }
        let presentation = self.state.presentation(cx);
        let frame_focus = window
            .use_keyed_state(("input-frame", self.state.id()), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        let focused = !self.disabled
            && (self.focused
                || self.state.has_selection_focus(window, cx)
                || frame_focus.contains_focused(window, cx));
        let hover = window.use_keyed_state(("input-hover", self.state.id()), cx, |_, _| false);
        let hovered = *hover.read(cx) && !self.disabled;
        let motion = window.use_keyed_state(("input-focus", self.state.id()), cx, |_, _| {
            Transition::new(f32::from(focused))
        });
        let reduced = cx.reduce_motion();
        let (amount, moving) = motion.update(cx, |m, _| {
            m.set(f32::from(focused), Instant::now(), motion::OVERLAY, reduced)
        });
        if moving {
            window.request_animation_frame();
        }
        let normal_border = if self.invalid {
            rgb(0xe5453d)
        } else {
            p.border
        };
        let border = if self.invalid {
            normal_border
        } else {
            motion::mix(
                normal_border,
                if S { p.accent } else { p.accent.opacity(0.68) },
                amount,
            )
        };
        let hover_motion =
            window.use_keyed_state(("input-hover-motion", self.state.id()), cx, |_, _| {
                Transition::new(0.)
            });
        let (hover_amount, moving) = hover_motion.update(cx, |m, _| {
            m.set(
                f32::from(S && hovered && !focused),
                Instant::now(),
                motion::OVERLAY,
                reduced,
            )
        });
        if moving {
            window.request_animation_frame();
        }
        let fill = motion::mix(
            p.surface,
            rgb(if dark { 0x3a3a3a } else { 0xf8fafc }),
            hover_amount,
        );
        let label = self
            .label
            .map(|label| crate::localization::label(cx, &label))
            .or_else(|| {
                (!presentation.placeholder().is_empty()).then(|| presentation.placeholder().clone())
            });
        // Never snapshot a password rope, even while its glyphs are revealed.
        let accessible = (window.is_a11y_active()
            && policy.exposes_value(presentation.is_masked()))
        .then(|| self.state.public_value(cx));
        let show_clear = policy.clear_visible(
            self.cleanable,
            focused,
            self.state.nonempty(cx),
            presentation.is_loading(),
        );
        let reserve_clear = self.cleanable && !policy.sensitive && self.suffix.is_none();
        let clear_amount = if reserve_clear {
            super::affix::clear_transition(self.state.id(), show_clear, window, cx)
        } else {
            0.
        };
        let show_mask = self.mask_toggle && !multiline;
        let focus_target = self.state.clone();
        let a11y_target = self.state.clone();
        let value_target = self.state.clone();
        let mut frame = BaseFrame::new(("product-input", self.state.id()))
            .role(policy.role())
            .focused(focused)
            .disabled(self.disabled)
            .relative()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .flex()
            .items_center()
            .when(multiline, |f| f.items_start())
            .gap(px(4.))
            .text_size(px(metrics.font))
            .font_weight(FontWeight::NORMAL)
            .line_height(px(metrics.line_height))
            .text_color(p.text)
            .when(!multiline, |f| f.h(px(metrics.height)))
            .when(multiline, |f| f.h_auto())
            .when(self.appearance, |f| {
                // Reserve the old 1px content inset as padding, so an absolute
                // stroke covers the outer box rather than its inner border box.
                let gutter = f32::from(!S && self.bordered);
                f.px(px(metrics.padding + gutter))
                    .py(px(metrics.vertical_padding + gutter))
                    .rounded(px(if S { 7. } else { 8. }))
                    .bg(fill)
                    .when(self.bordered, |f| {
                        if S {
                            f.border_1().border_color(border)
                        } else {
                            f.border_0()
                        }
                    })
                    .when(S && self.bordered && amount > 0., |f| {
                        f.shadow(vec![BoxShadow {
                            color: rgb(0x2563eb).opacity(0.1 * amount).into(),
                            offset: Point::default(),
                            blur_radius: px(0.),
                            spread_radius: px(2.),
                            inset: false,
                        }])
                    })
            })
            .when(!self.disabled, |f| {
                f.track_focus(&frame_focus)
                    .on_mouse_down(MouseButton::Left, move |_, w, cx| {
                        focus_target.focus(w, cx);
                        // This frame also has an automatic focus binding. A
                        // padding/affix click belongs to the editor, not that
                        // frame handle; do not let its default overwrite it.
                        w.prevent_default();
                    })
            })
            .when(self.disabled, |f| {
                f.capture_any_mouse_down(|_, _, cx| cx.stop_propagation())
            })
            .on_hover(move |over, _, cx| {
                hover.update(cx, |state, cx| {
                    if *state != *over {
                        *state = *over;
                        cx.notify();
                    }
                });
            })
            .when_some(label, |f, label| f.accessibility_label(label))
            .when(!presentation.placeholder().is_empty(), |f| {
                f.aria_placeholder(presentation.placeholder().clone())
            })
            .when_some(accessible, |f, value| f.aria_value(value))
            .on_a11y_action(AccessibleAction::Focus, move |_, w, cx| {
                if !a11y_target.presentation(cx).is_disabled() {
                    a11y_target.focus(w, cx);
                }
            })
            .when(policy.editable(), |f| {
                f.on_a11y_action(AccessibleAction::SetValue, move |data, w, cx| {
                    value_target.set_accessible_value(data, w, cx)
                })
            })
            .refine_style(&self.style);
        let editor = div()
            .min_w_0()
            .min_h_0()
            .flex_1()
            // A percentage-height wrapper inside an intrinsic-height frame
            // erases the Textarea's minimum/auto-grow rows. Only fill a frame
            // when its caller supplied an actual height.
            .when(multiline && fixed_height, |v| v.h_full())
            .child(super::cursor::override_cursor(
                self.state.editor(),
                self.editor_cursor,
            ));
        frame = frame
            .when_some(self.prefix, |f, prefix| {
                f.child(
                    div()
                        .flex_shrink_0()
                        .when(self.disabled, |v| v.opacity(0.5))
                        .child(prefix),
                )
            })
            .child(editor);
        if show_mask || reserve_clear || self.suffix.is_some() {
            let clear = self.state.clone();
            let mask = self.state.clone();
            let affixes = div()
                .flex()
                .items_center()
                .gap(px(4.))
                .flex_shrink_0()
                .when(multiline, |v| {
                    v.mt(px(((metrics.line_height - 16.) / 2.).max(0.)))
                })
                .when(show_mask, |v| {
                    v.child(
                        Button::<S>::new(("input-mask", self.state.id()))
                            .ghost()
                            .size(px(if S { 28. } else { 16. }))
                            .icon_size(px(14.))
                            .icon(if presentation.is_masked() {
                                IconName::EyeOff
                            } else {
                                IconName::Eye
                            })
                            .focusable(false)
                            .tab_stop(false)
                            .disabled(self.disabled)
                            .tooltip(if presentation.is_masked() {
                                "Show password"
                            } else {
                                "Hide password"
                            })
                            .on_click(move |_, w, cx| mask.toggle_mask(w, cx)),
                    )
                })
                .when(reserve_clear, |v| {
                    v.child(div().size(px(16.)).when(clear_amount > 0., |v| {
                        v.child(
                            super::affix::clear_button::<S>(
                                ("input-clear", self.state.id()),
                                p,
                                self.palette.is_some(),
                                cx,
                            )
                            .opacity(clear_amount)
                            .disabled(!show_clear)
                            .tooltip("Clear input")
                            .on_click(move |_, w, cx| clear.clear(w, cx)),
                        )
                    }))
                })
                .children(self.suffix);
            frame = frame.child(affixes);
        }
        // Native style painting keeps the stroke and fill on the same resolved
        // corners, including caller overrides and clamping on compact inputs.
        // The positioned border does not change the editor's content insets.
        frame = frame.when(!S && self.appearance && self.bordered, |f| {
            f.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(8.))
                    .refine_style(&StyleRefinement {
                        corner_radii: self.style.corner_radii,
                        ..Default::default()
                    })
                    .border(px(if self.invalid { 1.2 } else { 1. + 0.6 * amount }))
                    .border_color(border),
            )
        });
        // A zero-size child hosts the pointer-positioned overlay without
        // wrapping the editor or altering its flex/viewport/label geometry.
        frame.child(
            div()
                .absolute()
                .top(px(0.))
                .left(px(0.))
                .size(px(0.))
                .child(menu::surface(&context_menu, None, cx)),
        )
    }
}
/// gpui-base resolves a zero-alpha caret as "unset" and paints the theme
/// caret instead, so a transparent caret keeps the smallest nonzero alpha:
/// still unpainted, but no longer replaced.
fn caret(color: Rgba) -> Hsla {
    let mut caret = Hsla::from(color);
    if caret.a == 0. {
        caret.a = f32::MIN_POSITIVE;
    }
    caret
}

#[cfg(test)]
mod tests {
    use super::{Policy, caret, context_menu};
    use gpui_kit::Role;
    use gpui_kit::base::input::{InputContextMenuCapabilities, NativeMenu, NativeMenuItem};
    #[test]
    fn readonly_keeps_reading_but_never_offers_clear() {
        let p = Policy {
            readonly: true,
            ..Default::default()
        };
        assert!(!p.editable());
        assert!(p.exposes_value(false));
        assert!(!p.clear_visible(true, true, true, false));
    }
    #[test]
    fn a_transparent_caret_stays_hidden_instead_of_resolving_to_the_theme() {
        let hidden = caret(gpui_kit::rgba(0));
        assert!(hidden.a > 0. && hidden.a < 1. / 255.);
        assert_eq!(caret(gpui_kit::rgb(0x336699)).a, 1.);
    }
    #[test]
    fn disabled_is_not_editable_or_clearable() {
        let p = Policy {
            disabled: true,
            ..Default::default()
        };
        assert!(!p.editable());
        assert!(!p.clear_visible(true, true, true, false));
    }
    #[test]
    fn revealing_a_password_never_publishes_its_accessible_value() {
        let p = Policy {
            sensitive: true,
            ..Default::default()
        };
        assert_eq!(p.role(), Role::PasswordInput);
        assert!(!p.exposes_value(false));
        assert!(!p.exposes_value(true));
        assert!(!p.clear_visible(true, true, true, false));
    }
    #[test]
    fn clear_is_focus_only_nonempty_and_not_loading() {
        let p = Policy::default();
        assert!(p.clear_visible(true, true, true, false));
        for (requested, focused, nonempty, loading) in [
            (false, true, true, false),
            (true, false, true, false),
            (true, true, false, false),
            (true, true, true, true),
        ] {
            assert!(!p.clear_visible(requested, focused, nonempty, loading));
        }
    }
    #[test]
    fn multiline_is_an_input_mode_with_matching_accessible_role() {
        assert_eq!(
            Policy {
                multiline: true,
                ..Default::default()
            }
            .role(),
            Role::MultilineTextInput
        );
        assert_eq!(Policy::default().role(), Role::TextInput);
    }
    fn actions(capabilities: InputContextMenuCapabilities) -> Vec<(String, bool)> {
        context_menu(NativeMenu::new(), capabilities)
            .items
            .into_iter()
            .filter_map(|item| match item {
                NativeMenuItem::Action {
                    label, disabled, ..
                } => Some((label.to_string(), disabled)),
                NativeMenuItem::Separator => None,
            })
            .collect()
    }
    #[test]
    fn readonly_context_menu_keeps_copy_and_selection_but_blocks_writes() {
        assert_eq!(
            actions(
                InputContextMenuCapabilities::new()
                    .readonly(true)
                    .selection(true)
            ),
            vec![
                ("Cut".into(), true),
                ("Copy".into(), false),
                ("Paste".into(), true),
                ("Select All".into(), false)
            ]
        );
    }
    #[test]
    fn masked_context_menu_never_offers_copy_or_cut() {
        let values = actions(
            InputContextMenuCapabilities::new()
                .selection(true)
                .masked(true),
        );
        assert_eq!((values[0].1, values[1].1, values[2].1), (true, true, false));
    }
    #[test]
    fn disabled_context_menu_cannot_read_or_write() {
        assert!(
            actions(
                InputContextMenuCapabilities::new()
                    .selection(true)
                    .disabled(true)
            )
            .iter()
            .all(|(_, disabled)| *disabled)
        );
    }
    #[test]
    fn empty_selection_still_offers_paste_without_reading_clipboard() {
        let values = actions(InputContextMenuCapabilities::new());
        assert_eq!((values[0].1, values[1].1, values[2].1), (true, true, false));
    }
}
