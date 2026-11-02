use super::{BaseColors, Palette};
use nauterm_core::paths::ApplicationPaths;
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UserTheme {
    pub id: String,
    pub name: String,
    pub palette: Palette,
}
pub struct Loaded {
    pub(super) themes: Vec<UserTheme>,
    pub warnings: Vec<String>,
}

pub fn load(paths: &ApplicationPaths) -> Loaded {
    let directories: Vec<_> = std::iter::once(paths.themes())
        .chain(paths.additional_theme_directories.iter().cloned())
        .collect();
    load_directories(&directories)
}

fn load_directories(directories: &[PathBuf]) -> Loaded {
    let mut loaded = Loaded {
        themes: vec![],
        warnings: vec![],
    };
    for directory in directories {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                loaded
                    .warnings
                    .push(format!("{}: {error}", directory.display()));
                continue;
            }
        };
        let mut files = vec![];
        for entry in entries {
            match entry {
                Ok(entry)
                    if entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "toml") =>
                {
                    files.push(entry.path())
                }
                Ok(_) => {}
                Err(error) => loaded
                    .warnings
                    .push(format!("{}: {error}", directory.display())),
            }
        }
        files.sort();
        for path in files {
            match read_theme(&path) {
                Ok(Some(theme)) => loaded.themes.push(theme),
                Ok(None) => {}
                Err(error) => loaded.warnings.push(format!("{}: {error}", path.display())),
            }
        }
    }
    loaded
}

fn read_theme(path: &Path) -> Result<Option<UserTheme>, String> {
    let Some(id) = path.file_stem().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    if matches!(id, "default" | "nysa-light" | "nysa-dark") || id.chars().any(char::is_control) {
        return Ok(None);
    }
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Ok(None);
    }
    let mut source = String::new();
    file.take(512 * 1024 + 1)
        .read_to_string(&mut source)
        .map_err(|error| error.to_string())?;
    if source.len() > 512 * 1024 {
        return Err("Theme file exceeds 512 KiB".into());
    }
    let default = Palette::nysa_light();
    let mut defaults = [0; 23];
    defaults[..7].copy_from_slice(&[
        default.base.foreground,
        default.base.background,
        default.base.cursor,
        default.accent,
        default.selection,
        default.selection_text,
        default.cursor_text,
    ]);
    defaults[7..].copy_from_slice(&default.ansi);
    let parsed = crate::theme_resource_parser::parse_user(id, &source, defaults)?;
    let colors = parsed.colors;
    Ok(Some(UserTheme {
        id: id.into(),
        name: parsed.name,
        palette: Palette {
            base: BaseColors {
                foreground: colors[0],
                background: colors[1],
                cursor: colors[2],
            },
            accent: colors[3],
            selection: colors[4],
            selection_text: colors[5],
            cursor_text: colors[6],
            ansi: colors[7..].try_into().unwrap(),
            dark: parsed.dark,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_user_themes_load_in_directory_order_with_defaults_and_isolated_errors()
    -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let primary = root.path().join("primary");
        let extra = root.path().join("extra");
        fs::create_dir_all(&primary)?;
        fs::create_dir_all(&extra)?;
        fs::write(
            primary.join("自定义.toml"),
            "name='Personal'\ntype='dark'\n[colors.primary]\nbackground='#010203'\nforeground='bad'\n",
        )?;
        fs::write(
            extra.join("自定义.toml"),
            "name='Override'\n[colors.primary]\nbackground='#ffabcdef'\n",
        )?;
        fs::write(
            primary.join("nysa-light.toml"),
            "this is intentionally ignored",
        )?;
        fs::write(primary.join("broken.toml"), "[broken")?;
        let loaded = load_directories(&[root.path().join("missing"), primary, extra]);
        assert_eq!(loaded.themes.len(), 2);
        assert_eq!(loaded.warnings.len(), 1);
        assert_eq!(loaded.themes[0].name, "Personal");
        assert!(loaded.themes[0].palette.dark);
        assert_eq!(loaded.themes[0].palette.base.background, 0x010203);
        assert_eq!(
            loaded.themes[0].palette.base.foreground,
            Palette::nysa_light().base.foreground
        );
        assert_eq!(loaded.themes[1].name, "Override");
        assert_eq!(loaded.themes[1].palette.base.background, 0xabcdef);
        Ok(())
    }
}
