//! Workspace dialog presentation; Modal separately owns scope and dismissal.
//! Source: workspace_dialogs.dart::_WorkspaceDialogFrame. Not a settings skin.
use super::{Modal, ScrollbarsExt};
use crate::{palette::Palette, typography::FontWeights};
use gpui_kit::{
    AnyElement, App, BoxShadow, ElementId, FocusHandle, FontWeight, IntoElement, RenderOnce, Rgba,
    SharedString, Window, component::ActiveTheme, div, prelude::*, px, relative, rgb, size,
};

/// A decision/confirmation dialog, not an arbitrary modal workspace. The typed
/// frame enforces its title/content/action layout and exposes alert semantics.
#[derive(IntoElement)]
pub struct Dialog {
    modal: Modal,
}
impl Dialog {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            modal: Modal::new(id, label).alert().dismiss_on_backdrop(false),
        }
    }
    pub fn content(mut self, frame: DialogFrame) -> Self {
        self.modal = self.modal.content(frame);
        self
    }
    pub fn return_focus(mut self, focus: &FocusHandle) -> Self {
        self.modal = self.modal.return_focus(focus);
        self
    }
    pub fn focus_scope(mut self, focus: &FocusHandle) -> Self {
        self.modal = self.modal.focus_scope(focus);
        self
    }
    pub fn dismissible(mut self, value: bool) -> Self {
        self.modal = self.modal.dismissible(value);
        self
    }
    pub fn on_dismiss(mut self, callback: impl Fn(&(), &mut Window, &mut App) + 'static) -> Self {
        self.modal = self.modal.on_dismiss(callback);
        self
    }
}
impl RenderOnce for Dialog {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.modal
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialogColors {
    pub surface: Rgba,
    pub header: Rgba,
    pub border: Rgba,
    pub foreground: Rgba,
    pub dark: bool,
}
impl DialogColors {
    pub fn workspace(dark: bool) -> Self {
        let p = Palette::new(dark);
        Self {
            surface: p.surface,
            header: p.sidebar,
            border: p.border,
            foreground: p.text,
            dark,
        }
    }
    fn shadow(self) -> BoxShadow {
        let mut color = rgb(0x000000);
        color.a = if self.dark { 0.38 } else { 0.14 };
        gpui_kit::base::box_shadow(px(0.), px(14.), px(24.), px(0.), color.into())
    }
}

enum Title {
    Text(SharedString),
    Custom(AnyElement),
}
#[derive(IntoElement)]
pub struct DialogFrame {
    id: ElementId,
    title: Title,
    content: Option<AnyElement>,
    actions: Vec<AnyElement>,
    width: f32,
    colors: Option<DialogColors>,
}
impl DialogFrame {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: Title::Text(title.into()),
            content: None,
            actions: Vec::new(),
            width: 420.,
            colors: None,
        }
    }
    /// A header with extra controls; the caller translates its own copy.
    pub fn with_header(id: impl Into<ElementId>, header: impl IntoElement) -> Self {
        Self {
            title: Title::Custom(header.into_any_element()),
            ..Self::new(id, "")
        }
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
    /// Allows terminal-themed dialogs without changing other windows' tokens.
    pub fn colors(mut self, colors: DialogColors) -> Self {
        self.colors = Some(colors);
        self
    }
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
    pub fn actions(mut self, actions: impl IntoIterator<Item = AnyElement>) -> Self {
        self.actions.extend(actions);
        self
    }
}
impl RenderOnce for DialogFrame {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = self
            .colors
            .unwrap_or_else(|| DialogColors::workspace(cx.theme().is_dark()));
        let available = window.viewport_size() - size(px(48.), px(48.));
        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w(available.width.max(px(0.)))
            .max_h(available.height.max(px(0.)))
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(colors.surface)
            .text_color(colors.foreground)
            .text_size(px(13.))
            .line_height(relative(1.35))
            .font_weight(FontWeight::NORMAL)
            .rounded(px(12.))
            .border_1()
            .border_color(colors.border)
            .shadow(vec![colors.shadow()])
            .overflow_hidden()
            .child(
                div()
                    .id((self.id.clone(), "header"))
                    .flex_shrink_0()
                    .min_w_0()
                    .px(px(16.))
                    .pt(px(14.))
                    .pb(px(12.))
                    .bg(colors.header)
                    // GPUI overflow masks are rectangular. Keep the tinted
                    // child inside the frame's 12px radius and 1px border.
                    .rounded_t(px(11.))
                    .border_b_1()
                    .border_color(colors.border)
                    .text_size(px(15.))
                    .line_height(relative(1.2))
                    .font_weight(FontWeights::MEDIUM)
                    .child(match self.title {
                        Title::Text(title) => {
                            crate::localization::text(cx, &title).into_any_element()
                        }
                        Title::Custom(header) => header,
                    }),
            )
            .child(
                div()
                    .id((self.id.clone(), "body-scroll"))
                    .min_h_0()
                    .min_w_0()
                    .flex_shrink_1()
                    .overflow_y_scroll()
                    .scrollbars()
                    .px(px(16.))
                    .pt(px(16.))
                    .pb(px(14.))
                    .child(
                        div()
                            .id((self.id.clone(), "content"))
                            .min_h(px(56.))
                            .w_full()
                            .children(self.content),
                    ),
            )
            .child(
                div()
                    .id((self.id, "footer"))
                    .flex_shrink_0()
                    .min_w_0()
                    .px(px(16.))
                    .pt(px(8.))
                    .pb(px(10.))
                    .border_t_1()
                    .border_color(colors.border)
                    .flex()
                    .items_center()
                    .justify_end()
                    .flex_wrap()
                    .gap(px(8.))
                    .children(self.actions),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::point;
    #[test]
    fn dialog_tokens_and_shadow_follow_workspace_not_settings() {
        for dark in [false, true] {
            let colors = DialogColors::workspace(dark);
            let p = Palette::new(dark);
            assert_eq!(colors.surface, p.surface);
            assert_eq!(colors.header, p.sidebar);
            assert_eq!(colors.foreground, p.text);
            assert_eq!(colors.border, p.border);
            let shadow = colors.shadow();
            assert_eq!(shadow.offset, point(px(0.), px(14.)));
            assert_eq!(shadow.blur_radius, px(24.));
            assert_eq!(shadow.color.a, if dark { 0.38 } else { 0.14 });
        }
        assert_ne!(
            DialogColors::workspace(false).header,
            Palette::settings(false).sidebar
        );
    }
}
