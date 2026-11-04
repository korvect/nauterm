//! Resolve desktop-entry icons against XDG icon themes, including inherited
//! themes and absolute icon files. All I/O happens on the catalog worker.
use super::*;
use gpui_kit::ImageFormat;

const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;
fn value<'a>(text: &'a str, group: &str, key: &str) -> Option<&'a str> {
    let mut current = "";
    for line in text.lines().map(str::trim) {
        if line.starts_with(['#', ';']) {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            current = section;
        } else if current == group
            && let Some((name, value)) = line.split_once('=')
            && name.trim() == key
        {
            return Some(value.trim());
        }
    }
    None
}
fn safe_name(name: &str) -> bool {
    !name.is_empty() && !matches!(name, "." | "..") && !name.contains(['/', '\\', '\0'])
}
fn read(path: &Path) -> Option<(ImageFormat, Vec<u8>)> {
    let format = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => ImageFormat::Png,
        "svg" => ImageFormat::Svg,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        _ => return None,
    };
    if !fs::metadata(path).ok()?.is_file() {
        return None;
    }
    // Bound both the metadata and actual read in case the file changes.
    use std::io::Read;
    if fs::metadata(path).ok()?.len() > MAX_ICON_BYTES {
        return None;
    }
    let mut bytes = vec![];
    fs::File::open(path)
        .ok()?
        .take(MAX_ICON_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_ICON_BYTES).then_some((format, bytes))
}
fn candidate(base: &Path, name: &str) -> Option<(ImageFormat, Vec<u8>)> {
    read(&base.join(name)).or_else(|| {
        ["png", "svg", "jpg"]
            .into_iter()
            .find_map(|ext| read(&base.join(format!("{name}.{ext}"))))
    })
}
fn theme_directories(index: &str) -> Vec<&str> {
    let mut sizes = std::collections::HashMap::new();
    let mut section = "";
    // Parse section sizes once. Looking up Size by rescanning the entire file
    // from a sort comparator makes large themes take seconds per application.
    for line in index.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = name;
        } else if let Some((key, size)) = line.split_once('=')
            && key.trim() == "Size"
            && let Ok(size) = size.trim().parse::<i32>()
        {
            sizes.entry(section).or_insert(size);
        }
    }
    let mut dirs: Vec<_> = ["Directories", "ScaledDirectories"]
        .into_iter()
        .filter_map(|key| value(index, "Icon Theme", key))
        .flat_map(|dirs| dirs.split(','))
        .filter(|dir| {
            !dir.is_empty()
                && Path::new(dir)
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
        })
        .take(4096)
        .collect();
    dirs.sort_by_key(|dir| sizes.get(dir).copied().unwrap_or(32).abs_diff(32));
    dirs
}
fn theme_icon(
    roots: &[PathBuf],
    theme: &str,
    name: &str,
    visited: &mut HashSet<String>,
) -> Option<(ImageFormat, Vec<u8>)> {
    if !safe_name(theme) || visited.len() >= 32 || !visited.insert(theme.to_owned()) {
        return None;
    }
    let mut inherited = vec![];
    for root in roots {
        let base = root.join(theme);
        let index_path = base.join("index.theme");
        if fs::metadata(&index_path).is_ok_and(|metadata| metadata.len() > MAX_ICON_BYTES) {
            continue;
        }
        let index = fs::read_to_string(index_path).unwrap_or_default();
        // Prefer the closest nominal size to the menu's 32-device-pixel image.
        for dir in theme_directories(&index) {
            if let Some(icon) = candidate(&base.join(dir), name) {
                return Some(icon);
            }
        }
        // Some application installers create a sparse hicolor tree without
        // installing an index.theme in that particular XDG data directory.
        for size in [
            "32x32", "48x48", "scalable", "64x64", "24x24", "16x16", "256x256", "512x512",
        ] {
            if let Some(icon) = candidate(&base.join(size).join("apps"), name) {
                return Some(icon);
            }
        }
        if let Some(parents) = value(&index, "Icon Theme", "Inherits") {
            inherited.extend(parents.split(',').map(str::trim).map(str::to_owned));
        }
    }
    inherited
        .iter()
        .find_map(|parent| theme_icon(roots, parent, name, visited))
}
fn roots() -> (Vec<PathBuf>, Vec<PathBuf>) {
    let home = directories::UserDirs::new().map(|dirs| dirs.home_dir().to_owned());
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.as_ref().map(|home| home.join(".local/share")));
    let mut data: Vec<_> = data_home.into_iter().collect();
    data.extend(
        std::env::split_paths(
            &std::env::var_os("XDG_DATA_DIRS")
                .unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
        )
        .filter(|path| path.is_absolute()),
    );
    let mut icons: Vec<_> = home.map(|home| home.join(".icons")).into_iter().collect();
    icons.extend(data.iter().map(|root| root.join("icons")));
    (
        icons,
        data.into_iter().map(|root| root.join("pixmaps")).collect(),
    )
}
pub(in crate::external_application) fn load(path: &Path) -> Option<(ImageFormat, Vec<u8>)> {
    let name = icon_name(path)?;
    if Path::new(&name).is_absolute() {
        return read(Path::new(&name));
    }
    if !safe_name(&name) {
        return None;
    }
    let (roots, pixmaps) = roots();
    let config = directories::BaseDirs::new().map(|dirs| dirs.config_dir().to_owned());
    let configured = config
        .as_ref()
        .and_then(|config| fs::read_to_string(config.join("gtk-3.0/settings.ini")).ok())
        .and_then(|text| value(&text, "Settings", "gtk-icon-theme-name").map(str::to_owned))
        .or_else(|| {
            config
                .and_then(|config| fs::read_to_string(config.join("kdeglobals")).ok())
                .and_then(|text| value(&text, "Icons", "Theme").map(str::to_owned))
        })
        .or_else(icon_theme);
    let mut visited = HashSet::new();
    configured
        .as_deref()
        .into_iter()
        .chain(["hicolor", "Adwaita"])
        .find_map(|theme| theme_icon(&roots, theme, &name, &mut visited))
        .or_else(|| {
            roots
                .iter()
                .chain(pixmaps.iter())
                .find_map(|root| candidate(root, &name))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn theme_directory_order_keeps_nearest_sizes_and_rejects_escaping_paths() {
        let index = "[Icon Theme]\nDirectories=large/apps,small/apps,exact/apps,unspecified/apps,../outside,/absolute,\nScaledDirectories=scaled/apps\n[large/apps]\nSize=256\n[small/apps]\nSize=16\n[exact/apps]\nSize=32\n[scaled/apps]\nSize=48\n";
        assert_eq!(
            theme_directories(index),
            [
                "exact/apps",
                "unspecified/apps",
                "small/apps",
                "scaled/apps",
                "large/apps"
            ]
        );
    }

    #[test]
    fn themed_icons_follow_inheritance_and_user_override_without_enumerating_the_desktop()
    -> Result<()> {
        let temp = tempfile::tempdir()?;
        let user = temp.path().join("user");
        let system = temp.path().join("system");
        for root in [&user, &system] {
            fs::create_dir_all(root.join("base/32x32/apps"))?;
            fs::write(
                root.join("base/index.theme"),
                "[Icon Theme]\nDirectories=32x32/apps\n[32x32/apps]\nSize=32\n",
            )?;
            fs::write(
                root.join("base/32x32/apps/editor.svg"),
                root.to_string_lossy().as_bytes(),
            )?;
        }
        fs::create_dir_all(user.join("custom"))?;
        fs::write(
            user.join("custom/index.theme"),
            "[Icon Theme]\nInherits=base,custom\n",
        )?;
        let roots = [user.clone(), system.clone()];
        let (_, bytes) = theme_icon(&roots, "custom", "editor", &mut HashSet::new()).unwrap();
        assert_eq!(bytes, user.to_string_lossy().as_bytes());
        assert!(theme_icon(&roots, "custom", "missing", &mut HashSet::new()).is_none());
        assert!(!safe_name("../outside"));
        assert!(!safe_name(".."));
        assert!(read(&user.join("custom/index.theme")).is_none());
        Ok(())
    }
}
