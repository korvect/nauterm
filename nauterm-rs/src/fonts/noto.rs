//! Fonts shared by the Windows and Linux clients.
use gpui_kit::{
    App, Context, Div, Empty, FontFallbacks, IntoElement, Render, Stateful, Styled, Window,
    base::{Root, RootPlugin},
};
use std::borrow::Cow;

pub(crate) const UI_FAMILY: &str = "Noto Sans";
pub(crate) const MONO_FAMILY: &str = "Noto Sans Mono";

const UI_CJK: [&str; 4] = [
    "Noto Sans CJK SC",
    "Noto Sans CJK TC",
    "Noto Sans CJK JP",
    "Noto Sans CJK KR",
];
const MONO_CJK: [&str; 4] = [
    "Noto Sans Mono CJK SC",
    "Noto Sans Mono CJK TC",
    "Noto Sans Mono CJK JP",
    "Noto Sans Mono CJK KR",
];

// Legacy family names in the bundled static faces can expose weights as
// separate families. Keep the corresponding typographic family in the picker.
const WEIGHT_ALIASES: &[(&str, &str)] = &[
    ("Noto Sans Medium", UI_FAMILY),
    ("Noto Sans SemiBold", UI_FAMILY),
    ("Noto Sans CJK SC Medium", "Noto Sans CJK SC"),
    ("Noto Sans CJK TC Medium", "Noto Sans CJK TC"),
    ("Noto Sans CJK JP Medium", "Noto Sans CJK JP"),
    ("Noto Sans CJK KR Medium", "Noto Sans CJK KR"),
    ("Noto Sans CJK HK Medium", "Noto Sans CJK HK"),
];

pub(crate) fn weight_alias_family(name: &str) -> Option<&'static str> {
    WEIGHT_ALIASES
        .iter()
        .find_map(|(alias, family)| name.eq_ignore_ascii_case(alias).then_some(*family))
}

pub(super) fn merge_weight_aliases(names: &mut Vec<String>) {
    let available_families: Vec<_> = WEIGHT_ALIASES
        .iter()
        .map(|(_, family)| *family)
        .filter(|family| names.iter().any(|name| name.eq_ignore_ascii_case(family)))
        .collect();
    names.retain(|name| {
        weight_alias_family(name).is_none_or(|family| !available_families.contains(&family))
    });
}

const FACES: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/noto/NotoSans-Regular.ttf"),
    include_bytes!("../../assets/fonts/noto/NotoSans-Medium.ttf"),
    include_bytes!("../../assets/fonts/noto/NotoSans-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/noto/NotoSans-Bold.ttf"),
    include_bytes!("../../assets/fonts/noto/NotoSans-Italic.ttf"),
    include_bytes!("../../assets/fonts/noto/NotoSansMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/noto/NotoSansMono-Bold.ttf"),
    include_bytes!("../../assets/fonts/noto/NotoSansCJK-Regular.ttc"),
    include_bytes!("../../assets/fonts/noto/NotoSansCJK-Medium.ttc"),
    include_bytes!("../../assets/fonts/noto/NotoSansCJK-Bold.ttc"),
];

pub(super) fn install(cx: &mut App) -> anyhow::Result<()> {
    cx.text_system()
        .add_fonts(FACES.iter().copied().map(Cow::Borrowed).collect())?;
    Root::register_plugin(cx, |_, _| {
        let families = prefer_locale(UI_CJK, crate::localization::system_locale());
        UiFallbacks(FontFallbacks::from_fonts(
            families.into_iter().map(str::to_owned).collect(),
        ))
    });
    Ok(())
}

pub(crate) fn mono_cjk_fallbacks() -> [&'static str; 4] {
    prefer_locale(MONO_CJK, crate::localization::system_locale())
}

fn prefer_locale(mut families: [&'static str; 4], locale: &str) -> [&'static str; 4] {
    let mut parts = locale.split(['-', '_', '.', '@']);
    let language = parts.next().unwrap_or_default();
    let preferred = if language.eq_ignore_ascii_case("ja") {
        2
    } else if language.eq_ignore_ascii_case("ko") {
        3
    } else if language.eq_ignore_ascii_case("zh")
        && parts.any(|part| {
            ["Hant", "TW", "HK", "MO"]
                .iter()
                .any(|code| part.eq_ignore_ascii_case(code))
        })
    {
        1
    } else {
        0
    };
    // Han code points are shared, so the matching regional glyph comes first.
    families[..=preferred].rotate_right(1);
    families
}

struct UiFallbacks(FontFallbacks);

impl Render for UiFallbacks {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl RootPlugin for UiFallbacks {
    fn style(&self, surface: &mut Stateful<Div>, _: &mut Window, _: &mut App) {
        surface.text_style().font_fallbacks = Some(self.0.clone());
    }
}
