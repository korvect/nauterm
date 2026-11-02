//! Font samples use direct glyph queries, never a fallback-shaped text width.
use crate::components::SelectOption;
use gpui_kit::{TextSystem, font, px};

pub(crate) const SYMBOLS_FAMILY: &str = "Symbols Nerd Font";
const SYMBOLS_DATA: &[u8] = include_bytes!("../../assets/fonts/SymbolsNerdFont-Regular.ttf");
#[cfg(target_os = "macos")]
mod macos;

pub(crate) fn install(cx: &gpui_kit::App) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    macos::register(SYMBOLS_DATA)?;
    cx.text_system()
        .add_fonts(vec![std::borrow::Cow::Borrowed(SYMBOLS_DATA)])
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
            SelectOption::localized(
                value.to_string(),
                format!(
                    "settings.terminal.fontWeight.{}",
                    name.to_lowercase().replace(' ', "")
                ),
            )
        })
        .collect()
}

#[derive(Clone)]
pub(crate) struct Options {
    pub primary: Vec<SelectOption<String>>,
    pub cjk: Vec<SelectOption<String>>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            primary: vec![
                SelectOption::new("monospace".into(), "monospace")
                    .font_preview(crate::terminal_appearance::platform_family()),
            ],
            cjk: vec![SelectOption::localized(String::new(), "Auto")],
        }
    }
}

pub(crate) fn primary_options(fonts: &Options, selected: &str) -> Vec<SelectOption<String>> {
    let mut options = fonts.primary.clone();
    if !options
        .iter()
        .any(|option| option.value().as_deref() == Some(selected))
    {
        let family = if selected.eq_ignore_ascii_case("monospace") {
            crate::terminal_appearance::platform_family()
        } else {
            selected
        };
        options.push(
            SelectOption::new(selected.to_owned(), selected.to_owned())
                .font_preview(family.to_owned()),
        );
    }
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
    let mut primary = vec!["monospace".to_owned()];
    let mut cjk = vec![SelectOption::localized(String::new(), "Auto")];
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
                SelectOption::new(name.clone(), name.clone()).font_preview(resolved.to_owned())
            })
            .collect(),
        cjk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
