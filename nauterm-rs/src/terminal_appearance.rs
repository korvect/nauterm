//! Public terminal presentation preferences and the reference Nysa palettes.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const DEFAULT_WORD_BOUNDARIES: &str = " \t'\"│`|:;,()[]{}<>$";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Beam,
    HollowBlock,
}
impl CursorShape {
    pub const ALL: [Self; 4] = [Self::Block, Self::Underline, Self::Beam, Self::HollowBlock];
    pub fn label(self) -> &'static str {
        match self {
            Self::Block => "Block",
            Self::Underline => "Underline",
            Self::Beam => "Beam",
            Self::HollowBlock => "Hollow Block",
        }
    }
    pub fn native(self) -> u32 {
        match self {
            Self::Block => 0,
            Self::Underline => 1,
            Self::Beam => 2,
            Self::HollowBlock => 3,
        }
    }
}

pub fn format_word_boundaries(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\t', "\\t")
}
pub fn parse_word_boundaries(value: &str) -> String {
    let mut chars = value.chars().peekable();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek() {
                Some('t') => {
                    chars.next();
                    result.push('\t');
                    continue;
                }
                Some('\\') => {
                    chars.next();
                    result.push('\\');
                    continue;
                }
                _ => {}
            }
        }
        result.push(ch);
    }
    result
}

/// In-memory index only; metadata always stores the stable resource ID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThemeId(u16);
impl ThemeId {
    #[allow(non_upper_case_globals)]
    pub const NysaLight: Self = Self(0);
    #[allow(non_upper_case_globals)]
    pub const NysaDark: Self = Self(1);
    pub fn all() -> impl Iterator<Item = Self> {
        [Self::NysaLight, Self::NysaDark].into_iter().chain(
            crate::theme_catalog::indices()
                .into_iter()
                .skip(2)
                .map(Self),
        )
    }
    pub fn from_id(id: &str) -> Option<Self> {
        crate::theme_catalog::index(id).map(Self)
    }
    pub fn id(self) -> &'static str {
        crate::theme_catalog::id(self.0)
    }
    pub fn label(self) -> String {
        crate::theme_catalog::label(self.0)
    }
}
impl Serialize for ThemeId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.id())
    }
}
impl<'de> Deserialize<'de> for ThemeId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let id = String::deserialize(deserializer)?;
        Self::from_id(&id)
            .ok_or_else(|| serde::de::Error::custom(format!("Unavailable terminal theme: {id}")))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub theme: ThemeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_palette: Option<Palette>,
    pub family: String,
    pub cjk_family: Option<String>,
    pub enable_ligatures: bool,
    pub size: f32,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub weight: u16,
    pub bold_weight: u16,
    pub cursor_blink: bool,
    pub cursor_shape: CursorShape,
    pub scrollbar: bool,
    pub bell: crate::terminal_bell::Preferences,
    pub ssh_prediction: crate::terminal_prediction::Mode,
    pub padding: crate::terminal_padding::Padding,
    pub scrollback_lines: u32,
    pub report_mouse_events: bool,
    pub use_option_as_meta_key: bool,
    pub navigation_keys_scroll: bool,
    pub command_click_opens_filename_or_url: bool,
    pub option_click_moves_cursor: bool,
    pub copy_on_select: bool,
    pub select_command_block_on_click: bool,
    pub word_boundaries: String,
    pub composer_autocomplete: bool,
    pub composer_enabled: bool,
    pub multiple_tabs: bool,
    pub composer_max_suggestions: usize,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: ThemeId::NysaLight,
            custom_palette: None,
            family: "monospace".into(),
            cjk_family: None,
            enable_ligatures: false,
            size: 12.,
            line_height: 1.18,
            letter_spacing: 0.,
            weight: 400,
            bold_weight: 700,
            cursor_blink: true,
            cursor_shape: CursorShape::Block,
            scrollbar: true,
            bell: Default::default(),
            ssh_prediction: Default::default(),
            padding: Default::default(),
            scrollback_lines: 10_000,
            report_mouse_events: true,
            use_option_as_meta_key: true,
            navigation_keys_scroll: true,
            command_click_opens_filename_or_url: true,
            option_click_moves_cursor: true,
            copy_on_select: false,
            select_command_block_on_click: true,
            word_boundaries: DEFAULT_WORD_BOUNDARIES.into(),
            composer_autocomplete: false,
            composer_enabled: true,
            multiple_tabs: false,
            composer_max_suggestions: 999,
        }
    }
}
impl Appearance {
    pub fn runtime_options(&self) -> nauterm_core::terminal::TerminalRuntimeOptions {
        nauterm_core::terminal::TerminalRuntimeOptions {
            scrollback_lines: self.scrollback_lines as usize,
            cursor_shape: self.cursor_shape.native(),
            cursor_blinking: self.cursor_blink,
        }
    }
    /// Autocomplete is internal-only, not an available user feature. Preserve
    /// earlier experimental metadata without letting it enable suggestions or
    /// directory readers in the application.
    pub fn composer_autocomplete_enabled(&self) -> bool {
        false
    }
    pub fn validate(&self) -> Result<()> {
        self.padding.validate()?;
        ensure!(
            self.word_boundaries.len() <= 4096,
            "Word boundaries are too long"
        );
        ensure!(
            (1..=999).contains(&self.composer_max_suggestions),
            "Invalid composer suggestion limit"
        );
        if let Some(palette) = self.custom_palette {
            palette.validate()?;
        }
        ensure!(
            !self.family.trim().is_empty()
                && self.family.len() <= 256
                && !self.family.chars().any(char::is_control),
            "Invalid terminal font family"
        );
        ensure!(
            self.cjk_family.as_ref().is_none_or(|family| {
                !family.trim().is_empty()
                    && family.len() <= 256
                    && !family.chars().any(char::is_control)
            }),
            "Invalid terminal CJK font family"
        );
        ensure!(
            self.size.is_finite() && (9. ..=288.).contains(&self.size),
            "Invalid terminal font size"
        );
        ensure!(
            self.line_height.is_finite() && (1. ..=1.6).contains(&self.line_height),
            "Invalid terminal line height"
        );
        ensure!(
            self.letter_spacing.is_finite() && (0. ..=2.).contains(&self.letter_spacing),
            "Invalid terminal letter spacing"
        );
        ensure!(
            [300, 400, 500].contains(&self.weight),
            "Invalid terminal font weight"
        );
        ensure!(
            [600, 700, 800, 900].contains(&self.bold_weight),
            "Invalid terminal bold weight"
        );
        Ok(())
    }
    pub fn resolved_family(&self) -> &str {
        if self.family.trim().eq_ignore_ascii_case("monospace") {
            platform_family()
        } else {
            self.family.trim()
        }
    }
    pub fn fallback_families(&self) -> Vec<String> {
        let mut families = Vec::<String>::new();
        for candidate in self.cjk_family.as_deref().into_iter().chain([
            crate::fonts::SYMBOLS_FAMILY,
            "Consolas",
            "Menlo",
            "DejaVu Sans Mono",
            "Noto Sans Mono",
            "Liberation Mono",
            "Courier New",
        ]) {
            let candidate = candidate.trim();
            if !candidate.is_empty()
                && !candidate.eq_ignore_ascii_case(self.resolved_family())
                && !families
                    .iter()
                    .any(|family| family.eq_ignore_ascii_case(candidate))
            {
                families.push(candidate.to_owned());
            }
        }
        families
    }
    pub fn cell_height(&self) -> f32 {
        (self.size * self.line_height).ceil()
    }
    pub fn palette(&self) -> Palette {
        self.custom_palette
            .unwrap_or_else(|| Palette::new(self.theme))
    }
    pub fn select_theme(&mut self, theme: ThemeId) {
        self.theme = theme;
        self.custom_palette = None;
    }
    pub fn change_color(&mut self, index: usize, color: u32) -> Result<()> {
        ensure!(index < 23 && color <= 0xffffff, "Invalid terminal color");
        let mut palette = self.palette();
        match index {
            0 => palette.accent = color,
            1 => palette.base.foreground = color,
            2 => palette.base.background = color,
            3 => palette.base.cursor = color,
            4 => palette.cursor_text = color,
            5 => palette.selection = color,
            6 => palette.selection_text = color,
            _ => palette.ansi[index - 7] = color,
        }
        self.custom_palette = Some(palette);
        Ok(())
    }
}
pub fn platform_family() -> &'static str {
    if cfg!(target_os = "windows") {
        "Consolas"
    } else if cfg!(target_os = "macos") {
        "Menlo"
    } else {
        "DejaVu Sans Mono"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseColors {
    pub foreground: u32,
    pub background: u32,
    pub cursor: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    pub base: BaseColors,
    pub accent: u32,
    pub selection: u32,
    pub selection_text: u32,
    pub cursor_text: u32,
    pub ansi: [u32; 16],
    pub dark: bool,
}
// Keep the terminal-theme adapter with the terminal palette. Core product
// controls/palette remain usable without the compiled terminal theme catalog.
impl crate::palette::Palette {
    pub fn terminal(colors: Palette) -> Self {
        use gpui_kit::rgb;
        let bg = colors.base.background;
        let fg = colors.base.foreground;
        let mut p = Self::new(colors.dark);
        p.background = rgb(bg);
        p.surface = rgb(bg);
        p.text = rgb(fg);
        p.muted = rgb(blend(bg, fg, 0.62));
        p.hover = rgb(blend(bg, fg, 0.07));
        p.control = rgb(blend(bg, fg, 0.06));
        p.border = rgb(blend(bg, fg, 0.13));
        p.accent = rgb(colors.accent);
        p.sidebar = rgb(blend(bg, fg, 0.035));
        p
    }
}
const LIGHT_ANSI: [u32; 16] = [
    0x343842, 0xd95f56, 0x43a46f, 0xb88416, 0x1f73d8, 0x9d52a8, 0x1298aa, 0x68707d, 0x555b68,
    0xe46f67, 0x63b987, 0xd6a73d, 0x4b9cf0, 0xb86ac6, 0x47b7c6, 0x858d99,
];
impl Default for Palette {
    fn default() -> Self {
        Self::new(ThemeId::NysaLight)
    }
}
impl Palette {
    pub fn new(theme: ThemeId) -> Self {
        crate::theme_catalog::palette(theme.0)
    }
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.native_colors()
                .into_iter()
                .chain([
                    self.accent,
                    self.selection,
                    self.selection_text,
                    self.cursor_text
                ])
                .all(|color| color <= 0xffffff),
            "Invalid terminal RGB color"
        );
        Ok(())
    }
    pub fn editor_colors(self) -> [u32; 23] {
        let mut colors = [0; 23];
        colors[..7].copy_from_slice(&[
            self.accent,
            self.base.foreground,
            self.base.background,
            self.base.cursor,
            self.cursor_text,
            self.selection,
            self.selection_text,
        ]);
        colors[7..].copy_from_slice(&self.ansi);
        colors
    }
    pub(crate) const fn nysa_light() -> Self {
        Self {
            base: BaseColors {
                foreground: 0x343842,
                background: 0xfbfbf8,
                cursor: 0x9ec6ee,
            },
            accent: 0x3fa36f,
            selection: 0xd7eaf6,
            selection_text: 0x343842,
            cursor_text: 0x343842,
            ansi: LIGHT_ANSI,
            dark: false,
        }
    }
    pub(crate) const fn nysa_dark() -> Self {
        Self {
            base: BaseColors {
                foreground: 0xbcc1c9,
                background: 0x24262a,
                cursor: 0x929dad,
            },
            accent: 0x7fba89,
            selection: 0x383d45,
            selection_text: 0xbcc1c9,
            cursor_text: 0x24262a,
            ansi: [
                0x24262a, 0xdf7176, 0x80ba8a, 0xd7ad5f, 0x5798df, 0xb772c8, 0x50b0bf, 0xbcc1c9,
                0x4b5058, 0xe58489, 0x92cb9b, 0xe2c078, 0x70adeb, 0xc985d8, 0x6ac6d1, 0xd2d6dd,
            ],
            dark: true,
        }
    }
    pub fn native_colors(self) -> [u32; 19] {
        let mut values = [0; 19];
        values[..3].copy_from_slice(&[
            self.base.foreground,
            self.base.background,
            self.base.cursor,
        ]);
        values[3..].copy_from_slice(&self.ansi);
        values
    }
}
/// The native engine resolves color identity. Never guess roles from equal RGB.
pub fn cell_colors(mut foreground: u32, background: u32, flags: u16) -> (u32, u32) {
    if flags & 1 != 0 {
        return (background, foreground);
    }
    if flags & 0x80 != 0 {
        foreground = blend(background, foreground, 0.62);
    }
    (foreground, background)
}
pub fn blend(background: u32, foreground: u32, fraction: f32) -> u32 {
    let channel = |shift: u32| {
        let background = ((background >> shift) & 255) as f32;
        let foreground = ((foreground >> shift) & 255) as f32;
        (background + (foreground - background) * fraction).round() as u32
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_preferences_preserve_reference_defaults_unicode_and_empty_boundaries() -> Result<()>
    {
        let mut appearance: Appearance = serde_json::from_str("{}")?;
        assert!(appearance.report_mouse_events);
        assert!(appearance.command_click_opens_filename_or_url);
        assert!(appearance.option_click_moves_cursor);
        appearance.option_click_moves_cursor = false;
        appearance.command_click_opens_filename_or_url = false;
        assert!(!appearance.copy_on_select);
        assert!(appearance.select_command_block_on_click);
        appearance.select_command_block_on_click = false;
        assert_eq!(appearance.word_boundaries, DEFAULT_WORD_BOUNDARIES);
        for boundaries in [DEFAULT_WORD_BOUNDARIES, "", "中/\\t\\\t", "\\q"] {
            assert_eq!(
                parse_word_boundaries(&format_word_boundaries(boundaries)),
                boundaries
            );
            appearance.word_boundaries = boundaries.into();
            assert_eq!(
                serde_json::from_str::<Appearance>(&serde_json::to_string(&appearance)?)?,
                appearance
            );
        }
        assert_eq!(parse_word_boundaries("\\q\\"), "\\q\\");
        Ok(())
    }
    #[test]
    fn reference_defaults_are_independent_of_application_theme() -> Result<()> {
        let appearance: Appearance = serde_json::from_str("{}")?;
        assert_eq!(appearance, Appearance::default());
        assert_eq!((appearance.size, appearance.cell_height()), (12., 15.));
        assert_eq!(appearance.palette().base.background, 0xfbfbf8);
        assert_eq!(appearance.palette().base.cursor, 0x9ec6ee);
        assert_eq!(appearance.palette().accent, 0x3fa36f);
        appearance.validate()
    }
    #[test]
    fn native_palette_and_resolved_cells_do_not_alias_truecolors() {
        let light = Palette::new(ThemeId::NysaLight);
        let dark = Palette::new(ThemeId::NysaDark);
        assert_eq!(
            &dark.native_colors()[..3],
            &[dark.base.foreground, dark.base.background, dark.base.cursor]
        );
        assert_eq!(&dark.native_colors()[3..], &dark.ansi);
        for color in light.native_colors().into_iter().chain([0x123456]) {
            assert_eq!(
                cell_colors(color, dark.base.background, 0),
                (color, dark.base.background)
            );
        }
        assert_eq!(
            cell_colors(dark.base.foreground, dark.base.background, 1),
            (dark.base.background, dark.base.foreground)
        );
        assert_ne!(
            cell_colors(dark.base.foreground, dark.base.background, 0x80).0,
            dark.base.foreground
        );
        assert_eq!(
            cell_colors(dark.base.foreground, dark.base.background, 0x81),
            (dark.base.background, dark.base.foreground)
        );
    }
    #[test]
    fn invalid_public_preferences_do_not_reach_layout() {
        for invalid in [
            Appearance {
                size: f32::NAN,
                ..Appearance::default()
            },
            Appearance {
                line_height: 0.,
                ..Appearance::default()
            },
            Appearance {
                letter_spacing: -1.,
                ..Appearance::default()
            },
            Appearance {
                family: "bad\nfont".into(),
                ..Appearance::default()
            },
            Appearance {
                cjk_family: Some("bad\nfont".into()),
                ..Appearance::default()
            },
            Appearance {
                cjk_family: Some(" ".into()),
                ..Appearance::default()
            },
            Appearance {
                weight: 999,
                ..Appearance::default()
            },
            Appearance {
                composer_max_suggestions: 0,
                ..Appearance::default()
            },
            Appearance {
                composer_max_suggestions: 1000,
                ..Appearance::default()
            },
        ] {
            assert!(invalid.validate().is_err());
        }
    }
    #[test]
    fn cjk_fallback_precedes_platform_fonts_without_duplicates_or_primary_family() {
        let mut appearance = Appearance {
            family: "Menlo".into(),
            cjk_family: Some(" Noto Sans Mono ".into()),
            ..Appearance::default()
        };
        let fallbacks = appearance.fallback_families();
        assert_eq!(fallbacks[0], "Noto Sans Mono");
        assert_eq!(
            fallbacks.iter().filter(|f| *f == "Noto Sans Mono").count(),
            1
        );
        assert!(!fallbacks.iter().any(|f| f.eq_ignore_ascii_case("Menlo")));
        appearance.cjk_family = Some("menlo".into());
        assert_eq!(
            appearance.fallback_families()[0],
            crate::fonts::SYMBOLS_FAMILY
        );
        appearance.cjk_family = None;
        assert_eq!(
            appearance.fallback_families()[0],
            crate::fonts::SYMBOLS_FAMILY
        );
    }
    #[test]
    fn composer_preferences_default_off_and_round_trip_without_migrating_old_metadata() -> Result<()>
    {
        let old: Appearance = serde_json::from_str("{}")?;
        assert!(!old.composer_autocomplete);
        assert!(!old.composer_autocomplete_enabled());
        assert_eq!(old.composer_max_suggestions, 999);
        old.validate()?;
        let enabled = Appearance {
            composer_autocomplete: true,
            composer_max_suggestions: 1,
            ..old
        };
        enabled.validate()?;
        assert!(!enabled.composer_autocomplete_enabled());
        assert_eq!(
            serde_json::from_str::<Appearance>(&serde_json::to_string(&enabled)?)?,
            enabled
        );
        Ok(())
    }
    #[test]
    fn custom_colors_round_trip_and_selecting_a_theme_clears_all_overrides() -> Result<()> {
        let mut appearance = Appearance::default();
        appearance.select_theme(ThemeId::from_id("ayu-dark").unwrap());
        let base = appearance.palette();
        for index in 0..23 {
            appearance.change_color(index, 0x123400 + index as u32)?;
        }
        assert_eq!(
            appearance.palette().editor_colors(),
            std::array::from_fn(|index| 0x123400 + index as u32)
        );
        let stored = serde_json::to_string(&appearance)?;
        assert!(stored.contains("\"theme\":\"ayu-dark\""));
        assert_eq!(serde_json::from_str::<Appearance>(&stored)?, appearance);
        let previous = appearance.clone();
        assert!(appearance.change_color(23, 0).is_err());
        assert!(appearance.change_color(0, 0x1000000).is_err());
        assert_eq!(appearance, previous);
        appearance.select_theme(appearance.theme);
        assert!(appearance.custom_palette.is_none());
        assert_eq!(appearance.palette(), base);
        let mut invalid = base;
        invalid.selection_text = 0x1000000;
        appearance.custom_palette = Some(invalid);
        assert!(appearance.validate().is_err());
        Ok(())
    }
}
