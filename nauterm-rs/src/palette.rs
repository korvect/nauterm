use gpui_kit::{Rgba, rgb};

pub fn apply_theme(dark: bool, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    use gpui_kit::component::{Theme, ThemeMode};
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        Some(window),
        cx,
    );
    let p = Palette::new(dark);
    Theme::update(cx, |theme| {
        theme.colors.background = p.surface.into();
        theme.colors.foreground = p.text.into();
        theme.colors.primary = p.accent.into();
        theme.colors.primary_foreground = rgb(0xffffff).into();
        theme.colors.primary_hover = rgb(0x36a0ff).into();
        theme.colors.primary_active = rgb(0x0878d5).into();
        // Workspace buttons use a darker semantic primary than progress/focus
        // indicators (workspace_form_controls.dart::_buttonTypeColor).
        theme.colors.button_primary = rgb(0x075e92).into();
        theme.colors.button_primary_foreground = rgb(0xffffff).into();
        theme.colors.button_primary_hover = rgb(0x1b6b9b).into();
        theme.colors.button_primary_active = rgb(0x065786).into();
        // Editor Save / Generate actions use the blue informational accent.
        // Use semantic tokens, so disabled/hover/loading states stay upstream.
        theme.colors.button_info = p.accent.into();
        theme.colors.button_info_foreground = rgb(0xffffff).into();
        theme.colors.button_info_hover = rgb(0x36a0ff).into();
        theme.colors.button_info_active = rgb(0x0878d5).into();
        theme.colors.button_secondary = p.control.into();
        theme.colors.button_secondary_foreground = p.text.into();
        theme.colors.button_secondary_hover = if dark {
            rgb(0x444444).into()
        } else {
            rgb(0xe9ebed).into()
        };
        theme.colors.button_secondary_active = if dark {
            rgb(0x444444).into()
        } else {
            rgb(0xdce0e2).into()
        };
        theme.colors.border = p.border.into();
        theme.colors.input = p.border.into();
        theme.colors.muted_foreground = p.muted.into();
        theme.colors.ring = p.accent.into();
        theme.focus_ring = false;
    });
}

#[derive(Clone, Copy, PartialEq)]
pub struct Palette {
    pub control: Rgba,
    pub background: Rgba,
    pub surface: Rgba,
    pub sidebar: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
    pub border: Rgba,
    pub hover: Rgba,
    pub accent: Rgba,
    pub titlebar: Rgba,
}

/// `_SidebarItemButton` has a different state palette from ordinary buttons.
#[derive(Clone, Copy)]
pub struct SidebarButtonColors {
    pub background: Rgba,
    pub hover: Rgba,
    pub highlight: Rgba,
    pub splash: Rgba,
}

impl Palette {
    pub fn sidebar_button(dark: bool, selected: bool) -> SidebarButtonColors {
        let p = Self::new(dark);
        let hover = rgb(if dark { 0x383838 } else { 0xedf4f5 });
        let pressed = rgb(if dark { 0x444444 } else { 0xc7d5d8 });
        let splash = if dark {
            pressed.blend(p.text.alpha(0.22))
        } else {
            pressed
        };
        SidebarButtonColors {
            background: if selected {
                if dark { hover } else { rgb(0xe6eeee) }
            } else {
                p.sidebar
            },
            hover: if selected {
                if dark {
                    hover.blend(pressed.alpha(0.36))
                } else {
                    rgb(0xdde9ea)
                }
            } else {
                hover
            },
            highlight: Rgba { a: 0.58, ..splash },
            splash,
        }
    }
    /// Settings is its own reference design, not the workspace palette.
    pub fn settings(dark: bool) -> Self {
        if dark {
            return Self::new(true);
        }
        Self {
            control: rgb(0xf9fafb),
            background: rgb(0xffffff),
            surface: rgb(0xffffff),
            sidebar: rgb(0xf9fafb),
            text: rgb(0x111827),
            muted: rgb(0x4b5563),
            border: rgb(0xe5e7eb),
            hover: rgb(0xf8fafc),
            accent: rgb(0x2563eb),
            titlebar: rgb(0xf9fafb),
        }
    }
    /// Settings row subtitles are fainter than intro text and inactive navigation.
    pub fn settings_faint(dark: bool) -> Rgba {
        rgb(if dark { 0x858585 } else { 0x9ca3af })
    }
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                control: rgb(0x383838),
                background: rgb(0x202020),
                surface: rgb(0x2c2c2c),
                sidebar: rgb(0x282828),
                text: rgb(0xf2f2f2),
                muted: rgb(0xb8b8b8),
                border: rgb(0x464646),
                hover: rgb(0x383838),
                accent: rgb(0x0a84ff),
                titlebar: rgb(0x282828),
            }
        } else {
            Self {
                control: rgb(0xe0e4e6),
                background: rgb(0xedf3f3),
                surface: rgb(0xffffff),
                sidebar: rgb(0xf6f9f9),
                text: rgb(0x151927),
                muted: rgb(0x8ca0a6),
                border: rgb(0xdbe6e8),
                hover: rgb(0xe5eef0),
                accent: rgb(0x168df2),
                titlebar: rgb(0x363c51),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_and_workspace_keep_distinct_reference_tokens() {
        let settings = Palette::settings(false);
        let workspace = Palette::new(false);
        assert_eq!(settings.accent, rgb(0x2563eb));
        assert_eq!(workspace.accent, rgb(0x168df2));
        assert_eq!(settings.text, rgb(0x111827));
        assert_ne!(settings.muted, workspace.muted);
        assert_eq!(Palette::settings(true).accent, Palette::new(true).accent);
    }
    #[test]
    fn sidebar_keeps_selected_hover_and_translucent_hold_separate_from_splash() {
        let idle = Palette::sidebar_button(false, false);
        let selected = Palette::sidebar_button(false, true);
        assert_eq!(idle.background, rgb(0xf6f9f9));
        assert_eq!(selected.background, rgb(0xe6eeee));
        assert_eq!(idle.hover, rgb(0xedf4f5));
        assert_eq!(selected.hover, rgb(0xdde9ea));
        for dark in [false, true] {
            let idle = Palette::sidebar_button(dark, false);
            let selected = Palette::sidebar_button(dark, true);
            assert_ne!(idle.background, selected.background);
            assert_ne!(idle.hover, selected.hover);
            assert_eq!(idle.splash, selected.splash);
            assert_eq!(idle.highlight, selected.highlight);
            assert_eq!(idle.splash.a, 1.);
            assert_eq!(idle.highlight.a, 0.58);
            assert_eq!(idle.highlight.r, idle.splash.r);
            assert_eq!(idle.highlight.g, idle.splash.g);
            assert_eq!(idle.highlight.b, idle.splash.b);
        }
        assert_eq!(idle.splash, rgb(0xc7d5d8));
        let dark = Palette::sidebar_button(true, true);
        assert_eq!(dark.background, rgb(0x383838));
        assert!((dark.hover.r * 255. - 60.32).abs() < 0.001);
        assert!((dark.splash.r * 255. - 106.28).abs() < 0.001);
    }
}
