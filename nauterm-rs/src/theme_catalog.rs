//! Client-owned user themes overlay compiled assets. Files are read at startup or on
//! background refresh, never during rendering. Slot indices remain stable.
use crate::terminal_appearance::{BaseColors, Palette, ThemeId};
use std::{
    collections::BTreeMap,
    sync::{OnceLock, RwLock},
};
mod users;
pub use users::{Loaded, load};
#[derive(Clone, Copy)]
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub palette: Palette,
}
include!(concat!(env!("OUT_DIR"), "/terminal_themes.rs"));

#[derive(Clone, PartialEq, Eq)]
struct Entry {
    id: &'static str,
    name: String,
    palette: Palette,
    visible: bool,
}
struct Registry {
    entries: Vec<Entry>,
    by_id: BTreeMap<&'static str, u16>,
    order: Vec<u16>,
}
impl Registry {
    fn new() -> Self {
        Self {
            entries: THEMES
                .iter()
                .map(|theme| Entry {
                    id: theme.id,
                    name: theme.name.into(),
                    palette: theme.palette,
                    visible: true,
                })
                .collect(),
            by_id: THEMES
                .iter()
                .enumerate()
                .map(|(index, theme)| (theme.id, index as u16))
                .collect(),
            order: (0..THEMES.len()).map(|index| index as u16).collect(),
        }
    }
    fn publish(&mut self, themes: Vec<users::UserTheme>) -> bool {
        let mut desired: BTreeMap<_, _> = THEMES
            .iter()
            .map(|theme| (theme.id.to_owned(), (theme.name.to_owned(), theme.palette)))
            .collect();
        for theme in themes {
            if !matches!(theme.id.as_str(), "default" | "nysa-light" | "nysa-dark") {
                desired.insert(theme.id, (theme.name, theme.palette));
            }
        }
        let mut changed = false;
        for entry in &mut self.entries {
            let next = desired.remove(entry.id);
            let visible = next.is_some();
            let (name, palette) = next.unwrap_or_else(|| (entry.id.into(), Palette::nysa_light()));
            changed |= entry.name != name || entry.palette != palette || entry.visible != visible;
            entry.name = name;
            entry.palette = palette;
            entry.visible = visible;
        }
        for (id, (name, palette)) in desired {
            let Ok(index) = u16::try_from(self.entries.len()) else {
                break;
            };
            // One interned ID per stable slot, bounded by the u16 index space.
            // Labels and palettes are replaced normally and do not leak on reload.
            let id: &'static str = Box::leak(id.into_boxed_str());
            self.entries.push(Entry {
                id,
                name,
                palette,
                visible: true,
            });
            self.by_id.insert(id, index);
            changed = true;
        }
        self.order = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.visible)
            .map(|(index, _)| index as u16)
            .collect();
        self.order.sort_by_key(|index| {
            let id = self.entries[usize::from(*index)].id;
            (
                match id {
                    "nysa-light" => 0,
                    "nysa-dark" => 1,
                    _ => 2,
                },
                id,
            )
        });
        changed
    }
    fn index(&self, value: &str) -> Option<u16> {
        let name = value.trim().rsplit(['/', '\\']).next()?;
        let id = name.strip_suffix(".toml").unwrap_or(name);
        let id = if id == "default" { "nysa-light" } else { id };
        self.by_id
            .get(id)
            .copied()
            .filter(|index| self.entries[usize::from(*index)].visible)
    }
}
fn registry() -> &'static RwLock<Registry> {
    static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(Registry::new()))
}
pub fn publish(loaded: Loaded) -> bool {
    registry().write().unwrap().publish(loaded.themes)
}
pub fn indices() -> Vec<u16> {
    registry().read().unwrap().order.clone()
}
pub fn index(id: &str) -> Option<u16> {
    registry().read().unwrap().index(id)
}
pub fn id(index: u16) -> &'static str {
    registry().read().unwrap().entries[usize::from(index)].id
}
pub fn label(index: u16) -> String {
    registry().read().unwrap().entries[usize::from(index)]
        .name
        .clone()
}
pub fn palette(index: u16) -> Palette {
    registry().read().unwrap().entries[usize::from(index)].palette
}

pub fn filter(query: &str) -> Vec<ThemeId> {
    let query = query.trim().to_lowercase();
    ThemeId::all()
        .filter(|id| {
            id.id().to_lowercase().contains(&query) || id.label().to_lowercase().contains(&query)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn user_theme_overrides_reload_and_removal_preserve_stable_slot_ids() {
        let mut catalog = Registry::new();
        let mut custom = users::UserTheme {
            id: "个人主题".into(),
            name: "Personal".into(),
            palette: Palette::nysa_dark(),
        };
        assert!(catalog.publish(vec![custom.clone()]));
        let selected = catalog.index("个人主题").unwrap();
        assert_eq!(catalog.index("/some/path/个人主题.toml"), Some(selected));
        assert_eq!(catalog.index("default"), Some(0));
        custom.palette.base.background = 0x123456;
        custom.name = "Updated".into();
        assert!(catalog.publish(vec![custom.clone()]));
        assert_eq!(catalog.index("个人主题"), Some(selected));
        assert_eq!(
            catalog.entries[usize::from(selected)]
                .palette
                .base
                .background,
            0x123456
        );
        assert!(!catalog.publish(vec![custom.clone()]));
        assert!(catalog.publish(vec![]));
        assert_eq!(catalog.index("个人主题"), None);
        assert_eq!(catalog.entries[usize::from(selected)].id, "个人主题");
        assert!(catalog.publish(vec![custom]));
        assert_eq!(catalog.index("个人主题"), Some(selected));
        let mut override_theme = users::UserTheme {
            id: THEMES[2].id.into(),
            name: "Override".into(),
            palette: Palette::nysa_dark(),
        };
        catalog.publish(vec![override_theme.clone()]);
        assert_eq!(catalog.index(THEMES[2].id), Some(2));
        assert_eq!(catalog.entries[2].name, "Override");
        override_theme.id = "nysa-light".into();
        catalog.publish(vec![override_theme]);
        assert_eq!(catalog.entries[0].name, "Nysa Light");
        assert_eq!(catalog.entries[2].name, THEMES[2].name);
    }
    #[test]
    fn catalog_preserves_ids_order_and_every_reference_resource_color() {
        let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/themes");
        let mut count = 0;
        for file in std::fs::read_dir(resources).unwrap() {
            let path = file.unwrap().path();
            if path.extension().is_none_or(|extension| extension != "toml") {
                continue;
            }
            count += 1;
            let id = ThemeId::from_id(path.file_stem().unwrap().to_str().unwrap()).unwrap();
            let parsed = crate::theme_resource_parser::parse(
                id.id(),
                &std::fs::read_to_string(path).unwrap(),
            )
            .unwrap();
            let colors = Palette::new(id);
            assert_eq!(id.label(), parsed.name);
            assert_eq!(
                [
                    colors.base.foreground,
                    colors.base.background,
                    colors.base.cursor,
                    colors.accent,
                    colors.selection,
                    colors.selection_text,
                    colors.cursor_text
                ],
                parsed.colors[..7]
            );
            assert_eq!(colors.ansi, parsed.colors[7..]);
            assert_eq!(colors.dark, parsed.dark);
            let encoded = serde_json::to_string(&id).unwrap();
            assert_eq!(encoded, format!("\"{}\"", id.id()));
            assert_eq!(serde_json::from_str::<ThemeId>(&encoded).unwrap(), id);
        }
        assert_eq!(THEMES.len(), count + 2);
        assert_eq!(THEMES[0].id, "nysa-light");
        assert_eq!(THEMES[1].id, "nysa-dark");
        assert!(THEMES[2..].windows(2).all(|pair| pair[0].id < pair[1].id));
    }
    #[test]
    fn search_matches_ids_and_names_case_insensitively_without_changing_selection() {
        assert_eq!(filter(""), ThemeId::all().collect::<Vec<_>>());
        assert_eq!(
            filter("  NYSA  "),
            vec![ThemeId::NysaLight, ThemeId::NysaDark]
        );
        assert_eq!(filter("Ayu Dark"), filter("AYU-DARK"));
        assert!(filter("no such theme 12345").is_empty());
        assert!(serde_json::from_str::<ThemeId>("\"no such theme\"").is_err());
    }
}
