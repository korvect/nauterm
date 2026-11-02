use super::{
    Button, ButtonCustomVariant, ControlMetrics, Input,
    motion::{self, Transition},
};
use crate::typography::FontWeights;
use gpui_kit::base::input::{InputState, TextareaState};
use gpui_kit::{
    component::{ActiveTheme, Disableable, Sizable, Size},
    prelude::*,
    *,
};
use std::time::Instant;

/// Cross-field validation stays with the form; operation failures use Message.
pub fn validation_message(error: Option<&str>, cx: &App) -> Option<AnyElement> {
    error.map(|error| {
        div()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .text_size(px(12.))
            .line_height(px(18.))
            .text_color(rgb(0xe5453d))
            .child(crate::localization::message(cx, error))
            .into_any_element()
    })
}

/// External label shared by terminal appearance controls and inline tool forms.
pub fn terminal_field(
    label: &'static str,
    control: impl IntoElement,
    palette: crate::palette::Palette,
) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(
            div()
                .text_size(px(11.5))
                .line_height(px(18.))
                .text_color(palette.muted)
                .child(crate::localization::view(label)),
        )
        .child(control)
        .into_any_element()
}

enum Content {
    Input(Entity<InputState>),
    Multiline(Entity<TextareaState>, Option<f32>),
    Trigger(Box<Button>, bool),
    Control(AnyElement, FocusHandle, bool),
}
/// Workspace floating-label/error presentation, not the settings label layout.
/// Never clones the input text to decide label position, including passwords.
#[derive(IntoElement)]
pub struct Field {
    content: Content,
    label: SharedString,
    secret: bool,
    digits_only: bool,
    disabled: bool,
    error: Option<SharedString>,
    palette: Option<crate::palette::Palette>,
    size: Size,
}
impl Field {
    pub(crate) fn control(
        control: impl IntoElement,
        label: impl Into<SharedString>,
        focus: FocusHandle,
        nonempty: bool,
    ) -> Self {
        Self {
            content: Content::Control(control.into_any_element(), focus, nonempty),
            label: label.into(),
            secret: false,
            digits_only: false,
            disabled: false,
            error: None,
            palette: None,
            size: Size::Large,
        }
    }
    /// An input-shaped selector using the same label/error frame. The supplied
    /// Button retains keyboard, focus, disabled and accessibility semantics.
    pub fn trigger(button: Button, label: impl Into<SharedString>, has_content: bool) -> Self {
        Self {
            content: Content::Trigger(Box::new(button), has_content),
            label: label.into(),
            secret: false,
            digits_only: false,
            disabled: false,
            error: None,
            palette: None,
            size: Size::Large,
        }
    }
    pub fn input(state: &Entity<InputState>, label: impl Into<SharedString>) -> Self {
        Self {
            content: Content::Input(state.clone()),
            label: label.into(),
            secret: false,
            digits_only: false,
            disabled: false,
            error: None,
            palette: None,
            size: Size::Large,
        }
    }
    pub fn multiline(
        state: &Entity<TextareaState>,
        label: impl Into<SharedString>,
        height: f32,
    ) -> Self {
        Self {
            content: Content::Multiline(state.clone(), Some(height)),
            label: label.into(),
            secret: false,
            digits_only: false,
            disabled: false,
            error: None,
            palette: None,
            size: Size::Large,
        }
    }
    /// Intrinsic multiline height, using the editor state's row/auto-grow policy.
    /// Wrapping, selection, scrolling and IME still belong to the base editor.
    pub fn multiline_auto(state: &Entity<TextareaState>, label: impl Into<SharedString>) -> Self {
        Self {
            content: Content::Multiline(state.clone(), None),
            label: label.into(),
            secret: false,
            digits_only: false,
            disabled: false,
            error: None,
            palette: None,
            size: Size::Large,
        }
    }
    pub(crate) fn with_palette(mut self, palette: Option<crate::palette::Palette>) -> Self {
        self.palette = palette;
        self
    }
    pub fn secret(mut self, secret: bool) -> Self {
        self.secret = secret;
        self
    }
    pub fn digits_only(mut self) -> Self {
        self.digits_only = true;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn error(mut self, error: Option<SharedString>) -> Self {
        self.error = error;
        self
    }
}
impl Sizable for Field {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}
impl RenderOnce for Field {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Only the field's UI caption is localized; the editor state is data.
        self.label = crate::localization::label(cx, &self.label);
        let (focus, nonempty, multiline) = match &self.content {
            Content::Input(state) => (
                Some(state.read(cx).focus_handle(cx)),
                state.read(cx).text().len() > 0,
                false,
            ),
            Content::Multiline(state, _) => (
                Some(state.read(cx).focus_handle(cx)),
                state.read(cx).text().len() > 0,
                true,
            ),
            Content::Trigger(_, nonempty) => (None, *nonempty, false),
            Content::Control(_, focus, nonempty) => (Some(focus.clone()), *nonempty, false),
        };
        let focused = !self.disabled
            && match &self.content {
                Content::Input(state) => state.read(cx).has_selection_focus(window, cx),
                Content::Multiline(state, _) => state.read(cx).has_selection_focus(window, cx),
                _ => focus.as_ref().is_some_and(|focus| focus.is_focused(window)),
            };
        let floating = nonempty || focused;
        let key: ElementId = match &self.content {
            Content::Input(state) => ("field-label", state.entity_id()).into(),
            Content::Multiline(state, _) => ("field-label", state.entity_id()).into(),
            Content::Trigger(button, _) => (button.element_id(), "field-label").into(),
            Content::Control(_, focus, _) => (ElementId::from(focus), "field-label").into(),
        };
        let motion = window.use_keyed_state((key.clone(), "motion"), cx, |_, _| {
            Transition::new(f32::from(floating))
        });
        let reduced = cx.reduce_motion();
        let (amount, moving) = motion.update(cx, |state, _| {
            state.set(
                f32::from(floating),
                Instant::now(),
                motion::OVERLAY,
                reduced,
            )
        });
        if moving {
            window.request_animation_frame();
        }
        let palette = self
            .palette
            .unwrap_or_else(|| crate::palette::Palette::new(cx.theme().is_dark()));
        let fill = if self.palette.is_some() {
            palette.control
        } else {
            palette.surface
        };
        let invalid = self.error.is_some();
        let label_color = if invalid {
            rgb(0xe5453d)
        } else {
            palette.muted
        };
        let layout = LabelLayout::at(multiline, amount, self.size);
        let label = div()
            .id(key)
            .absolute()
            .left(px(layout.left))
            .top(px(layout.top))
            .px(px(3. * amount))
            .h(px(layout.line_height))
            .line_height(px(layout.line_height))
            .max_w(relative(0.9))
            .text_ellipsis()
            .bg(fill)
            .text_color(label_color)
            .text_size(px(layout.font))
            .font_weight(FontWeights::MEDIUM)
            .child(self.label.clone())
            .when_some(
                focus.filter(|_| !self.disabled && !matches!(self.content, Content::Control(..))),
                |label, focus| {
                    label.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        focus.focus(window, cx)
                    })
                },
            );
        let control = match self.content {
            Content::Input(state) => Input::new(&state)
                .when(self.digits_only, |input| input.digits_only())
                .with_size(self.size)
                .when_some(self.palette, |input, p| input.palette(p).bg(p.control))
                .aria_label(self.label.clone())
                .cleanable(!self.secret)
                .disabled(self.disabled)
                .invalid(invalid)
                .when(self.secret, |field| field.mask_toggle())
                .into_any_element(),
            Content::Multiline(state, height) => Input::multiline(&state)
                .with_size(self.size)
                .when_some(self.palette, |input, p| input.palette(p).bg(p.control))
                .when_some(height, |field, height| field.h(px(height)))
                .aria_label(self.label.clone())
                .disabled(self.disabled)
                .invalid(invalid)
                .into_any_element(),
            Content::Trigger(button, _) => (*button)
                .with_size(self.size)
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(fill.into())
                        .foreground(palette.text.into())
                        .highlight(rgba(0).into()),
                )
                .ink_ripple(false)
                .outline()
                .border_color(palette.border)
                .rounded(px(8.))
                .h(px(ControlMetrics::input(self.size).height))
                .content_start()
                .w_full()
                .px(px(ControlMetrics::input(self.size).padding + 1.))
                .relative()
                .disabled(self.disabled)
                .accessibility_label(self.label.clone())
                .text_size(px(ControlMetrics::input(self.size).font))
                .line_height(px(ControlMetrics::input(self.size).line_height))
                .font_weight(FontWeights::REGULAR)
                .when(invalid, |button| button.border_color(rgb(0xe5453d)))
                .into_any_element(),
            Content::Control(control, _, _) => control,
        };
        div()
            .min_w_0()
            .when(multiline, |field| field.flex_grow_1().flex_shrink_0())
            .when(!multiline, |field| field.flex_1())
            .child(div().relative().child(control).child(label))
            .when_some(self.error, |view, error| {
                view.child(
                    div()
                        .mt(px(4.))
                        .text_size(px(11.))
                        .text_color(rgb(0xe5453d))
                        .child(crate::localization::message(cx, &error)),
                )
            })
    }
}

/// Empty labels share the editor's actual line box, not an inherited page line
/// height. Multiline labels stay on its first line. Flutter's outline decorator
/// scales its 12px floating style by .75; its line box straddles the top stroke.
struct LabelLayout {
    left: f32,
    top: f32,
    font: f32,
    line_height: f32,
}
impl LabelLayout {
    fn at(multiline: bool, amount: f32, size: Size) -> Self {
        let m = ControlMetrics::input(size);
        let empty_top = if multiline {
            1. + m.vertical_padding
        } else {
            (m.height - m.line_height) / 2.
        };
        let floating_font = 12. * 0.75;
        let floating_line = 18. * 0.75;
        let mix = |from: f32, to: f32| from + (to - from) * amount;
        Self {
            // The border lies outside frame padding; label's animated gap must
            // not change the glyph's start relative to the editor text.
            left: 1. + m.padding - 3. * amount,
            top: mix(empty_top, 0.5 - floating_line / 2.),
            font: mix(m.font, floating_font),
            line_height: mix(m.line_height, floating_line),
        }
    }
}
