//! Family aliases and locale ordering for system-installed Noto fonts.

const CJK_FAMILIES: [&str; 5] = [
    "Noto Sans CJK SC",
    "Noto Sans CJK TC",
    "Noto Sans CJK JP",
    "Noto Sans CJK KR",
    "Noto Sans CJK HK",
];

// Installed faces can expose weights as separate families. Group those
// entries when their regular family is also installed.
const WEIGHT_ALIASES: &[(&str, &str)] = &[
    ("Noto Sans Medium", "Noto Sans"),
    ("Noto Sans SemiBold", "Noto Sans"),
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

pub(crate) fn cjk_fallbacks() -> [&'static str; 5] {
    prefer_locale(crate::localization::system_locale())
}

fn prefer_locale(locale: &str) -> [&'static str; 5] {
    let mut families = CJK_FAMILIES;
    let mut parts = locale.split(['-', '_', '.', '@']);
    let language = parts.next().unwrap_or_default();
    let preferred = if language.eq_ignore_ascii_case("ja") {
        2
    } else if language.eq_ignore_ascii_case("ko") {
        3
    } else if language.eq_ignore_ascii_case("zh")
        && parts
            .clone()
            .any(|part| part.eq_ignore_ascii_case("HK") || part.eq_ignore_ascii_case("MO"))
    {
        4
    } else if language.eq_ignore_ascii_case("zh")
        && parts.any(|part| part.eq_ignore_ascii_case("Hant") || part.eq_ignore_ascii_case("TW"))
    {
        1
    } else {
        0
    };
    // Han code points are shared, so the matching regional glyph comes first.
    families[..=preferred].rotate_right(1);
    families
}
