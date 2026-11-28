//! Font samples use direct glyph queries, never a fallback-shaped text width.
use crate::components::SelectOption;
use gpui_kit::{TextSystem, font, px};

#[cfg(any(target_os = "linux", target_os = "windows"))]
mod noto;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) use noto::{MONO_FAMILY, UI_FAMILY, mono_cjk_fallbacks, weight_alias_family};

pub(crate) const SYMBOLS_FAMILY: &str = "Symbols Nerd Font";
const SYMBOLS_DATA: &[u8] = include_bytes!("../../assets/fonts/SymbolsNerdFont-Regular.ttf");
#[cfg(target_os = "macos")]
mod macos;

pub(crate) fn install(cx: &mut gpui_kit::App) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    macos::register(SYMBOLS_DATA)?;
    cx.text_system()
        .add_fonts(vec![std::borrow::Cow::Borrowed(SYMBOLS_DATA)])?;
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    noto::install(cx)?;
    Ok(())
}

/// Font settings use standard numeric weights, independently of UI role aliases.
pub(crate) fn weight_options(values: &[u16]) -> Vec<SelectOption<String>> {
    values
        .iter()
        .map(|value| {
            let name = match value {
                100 => "Thin",
                200 => "Extra Light",
                300 => "Light",
                400 => "Regular",
                500 => "Medium",
                600 => "Semi Bold",
                700 => "Bold",
                800 => "Extra Bold",
                900 => "Black",
                _ => return SelectOption::new(value.to_string(), value.to_string()),
            };
            SelectOption::new(value.to_string(), name)
        })
        .collect()
}

#[derive(Clone)]
pub(crate) struct Options {
    primary: Vec<SelectOption<String>>,
    cjk: Vec<SelectOption<String>>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            primary: vec![
                SelectOption::new("monospace".into(), "monospace")
                    .font_sample(crate::terminal_appearance::platform_family(), "Aa 0123"),
            ],
            cjk: vec![SelectOption::localized(
                String::new(),
                "settings.terminal.cjkFont.auto",
            )],
        }
    }
}

pub(crate) fn primary_options(fonts: &Options, selected: &str) -> Vec<SelectOption<String>> {
    with_saved_family(&fonts.primary, selected, |title| {
        let family = if selected.eq_ignore_ascii_case("monospace") {
            crate::terminal_appearance::platform_family()
        } else {
            selected
        };
        SelectOption::new(selected.to_owned(), title.to_owned()).font_sample(family, "Aa 0123")
    })
}

pub(crate) fn cjk_options(fonts: &Options, selected: &str) -> Vec<SelectOption<String>> {
    with_saved_family(&fonts.cjk, selected, |title| {
        SelectOption::new(selected.to_owned(), title.to_owned())
    })
}

fn with_saved_family(
    available: &[SelectOption<String>],
    selected: &str,
    option_for_selected: impl FnOnce(&str) -> SelectOption<String>,
) -> Vec<SelectOption<String>> {
    let mut options = available.to_vec();
    if options
        .iter()
        .any(|option| option.value().as_deref() == Some(selected))
    {
        return options;
    }
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    if let Some(family) = noto::weight_alias_family(selected)
        && let Some(option) = options.iter_mut().find(|option| {
            option
                .value()
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(family))
        })
    {
        // Keep the saved value selected without exposing the same family twice.
        *option = option_for_selected(family);
        return options;
    }
    options.push(option_for_selected(selected));
    options
}

pub(crate) fn size_options(selected: f32) -> Vec<SelectOption<String>> {
    let mut sizes = vec![
        9., 10., 11., 12., 13., 14., 15., 16., 17., 18., 24., 36., 48., 64., 72., 96., 144., 288.,
    ];
    sizes.push(selected);
    sizes.sort_by(f32::total_cmp);
    sizes.dedup();
    sizes
        .into_iter()
        .map(|size| {
            let value = size.to_string();
            SelectOption::new(value.clone(), value)
        })
        .collect()
}

pub(crate) fn options(text: &TextSystem) -> Options {
    const SAMPLE: &str = "汉字 Aa";
    let mut names = text.all_font_names();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    noto::merge_weight_aliases(&mut names);
    let mut primary = vec!["monospace".to_owned()];
    let mut cjk = vec![SelectOption::localized(
        String::new(),
        "settings.terminal.cjkFont.auto",
    )];
    for name in names {
        let face = font(name.clone());
        let id = text.resolve_font(&face);
        let widths: Option<Vec<f32>> = ['W', 'i', 'M', '.', '0', '@', ' ']
            .into_iter()
            .map(|ch| {
                text.advance(id, px(12.), ch)
                    .ok()
                    .map(|size| f32::from(size.width))
            })
            .collect();
        if widths.as_ref().is_some_and(|widths| {
            widths[0] > 0. && widths.iter().all(|width| (*width - widths[0]).abs() < 0.02)
        }) {
            primary.push(name.clone());
        }
        // resolve_font can substitute a default face if a font cannot load.
        // Suppress previews in that case, even if the substitute covers Han.
        let own_face = text
            .get_font_for_id(id)
            .is_some_and(|resolved| resolved.family == face.family);
        let covered = own_face
            && SAMPLE
                .chars()
                .all(|ch| text.advance(id, px(13.), ch).is_ok());
        let option = SelectOption::new(name.clone(), name.clone());
        cjk.push(if covered {
            option.font_sample(name, SAMPLE)
        } else {
            option
        });
    }
    primary.sort_by_key(|name| name.to_lowercase());
    primary.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    Options {
        primary: primary
            .into_iter()
            .map(|name| {
                let resolved = if name.eq_ignore_ascii_case("monospace") {
                    crate::terminal_appearance::platform_family()
                } else {
                    &name
                };
                SelectOption::new(name.clone(), name.clone()).font_sample(resolved, "Aa 0123")
            })
            .collect(),
        cjk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn noto_weights_merge_only_into_available_families() {
        let families = [
            "Noto Sans",
            "Noto Sans Mono",
            "Noto Sans CJK SC",
            "Noto Sans CJK TC",
            "Noto Sans CJK JP",
            "Noto Sans CJK KR",
            "Noto Sans CJK HK",
            "Noto Sans Mono CJK SC",
            "Noto Sans Mono CJK TC",
            "Noto Sans Mono CJK JP",
            "Noto Sans Mono CJK KR",
            "Noto Sans Mono CJK HK",
            "Other Font",
            "Other Font Medium",
        ];
        let mut names = families
            .into_iter()
            .chain([
                "Noto Sans Medium",
                "Noto Sans SemiBold",
                "Noto Sans CJK SC Medium",
                "Noto Sans CJK TC Medium",
                "Noto Sans CJK JP Medium",
                "Noto Sans CJK KR Medium",
                "Noto Sans CJK HK Medium",
            ])
            .map(str::to_owned)
            .collect();
        noto::merge_weight_aliases(&mut names);
        assert_eq!(names, families);

        let mut alias_only = vec!["Noto Sans CJK SC Medium".to_owned()];
        noto::merge_weight_aliases(&mut alias_only);
        assert_eq!(alias_only, ["Noto Sans CJK SC Medium"]);
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn saved_noto_alias_uses_one_row_with_the_original_value() {
        let family = "Noto Sans CJK HK";
        let alias = "Noto Sans CJK HK Medium";
        let fonts = Options {
            cjk: vec![SelectOption::new(family.to_owned(), family)],
            ..Options::default()
        };

        assert_eq!(
            cjk_options(&fonts, alias),
            [SelectOption::new(alias.to_owned(), family)]
        );
        assert_eq!(
            cjk_options(&fonts, "Custom CJK"),
            [
                SelectOption::new(family.to_owned(), family),
                SelectOption::new("Custom CJK".to_owned(), "Custom CJK"),
            ]
        );
        assert_eq!(cjk_options(&fonts, family), fonts.cjk);
    }

    #[test]
    fn saved_custom_fonts_remain_selectable_while_inventory_is_loading() {
        let pending = Options::default();
        let custom = primary_options(&pending, "Synthetic Mono");
        assert!(
            custom
                .iter()
                .any(|option| option.value().as_deref() == Some("Synthetic Mono"))
        );
        let loaded = Options {
            primary: custom.clone(),
            ..pending
        };
        assert_eq!(primary_options(&loaded, "Synthetic Mono"), custom);
    }

    #[test]
    fn size_choices_preserve_fractional_values_in_numeric_order_without_duplicates() {
        for selected in [9., 12.5, 96., 288.] {
            let values: Vec<f32> = size_options(selected)
                .iter()
                .map(|option| option.value().as_ref().unwrap().parse().unwrap())
                .collect();
            assert!(values.contains(&selected));
            assert!(values.windows(2).all(|pair| pair[0] < pair[1]));
            assert_eq!(values.first(), Some(&9.));
            assert_eq!(values.last(), Some(&288.));
        }
    }
}
