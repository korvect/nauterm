//! File-browser data, navigation and selection independent of rendering.
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
pub fn format_size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024. && unit < units.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    let digits = usize::from(unit != 0 && value < 100.);
    format!("{value:.digits$} {}", units[unit])
}

#[derive(Clone, Debug)]
pub struct Entry {
    /// Exact local path; display text is never used as a lossy filesystem path.
    pub native_path: Option<PathBuf>,
    pub can_open: bool,
    pub path: String,
    pub name: String,
    pub directory: bool,
    pub parent: bool,
    pub size: u64,
    pub modified: Option<u64>,
    pub mode: Option<u32>,
}
impl Entry {
    pub fn compact_subtitle(&self) -> String {
        if self.parent {
            return String::new();
        }
        let mut parts = Vec::new();
        if let Some(mode) = self.mode {
            parts.push(permissions(mode));
        }
        if !self.directory {
            parts.push(format_size(self.size));
        }
        if let Some(time) = self
            .modified
            .and_then(|time| i64::try_from(time).ok())
            .and_then(|time| chrono::DateTime::from_timestamp(time, 0))
        {
            parts.push(
                time.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string(),
            );
        }
        parts.join(" · ")
    }
    pub fn kind(&self) -> String {
        if self.directory {
            "Folder".into()
        } else {
            Path::new(&self.name)
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .map(|extension| format!("{} File", extension.to_uppercase()))
                .unwrap_or("File".into())
        }
    }
}
fn permissions(mode: u32) -> String {
    let kind = match mode & 0o170000 {
        0o040000 => 'd',
        0o120000 => 'l',
        0o020000 => 'c',
        0o060000 => 'b',
        0o010000 => 'p',
        0o140000 => 's',
        _ => '-',
    };
    let mut result = String::from(kind);
    for (shift, special) in [(6, 0o4000), (3, 0o2000), (0, 0o1000)] {
        result.push(if mode & (4 << shift) != 0 { 'r' } else { '-' });
        result.push(if mode & (2 << shift) != 0 { 'w' } else { '-' });
        let executable = mode & (1 << shift) != 0;
        result.push(if mode & special != 0 {
            match (shift, executable) {
                (0, true) => 't',
                (0, false) => 'T',
                (_, true) => 's',
                _ => 'S',
            }
        } else if executable {
            'x'
        } else {
            '-'
        });
    }
    result
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Name,
    Modified,
    Size,
    Kind,
}
pub fn visible(
    entries: &[Entry],
    query: &str,
    hidden: bool,
    sort: Sort,
    ascending: bool,
) -> Vec<Entry> {
    let query = query.trim().to_lowercase();
    let mut result: Vec<_> = entries
        .iter()
        .filter(|entry| {
            entry.parent
                || ((hidden || !entry.name.starts_with('.'))
                    && entry.name.to_lowercase().contains(&query))
        })
        .cloned()
        .collect();
    result.sort_by(|a, b| {
        b.parent
            .cmp(&a.parent)
            .then_with(|| b.directory.cmp(&a.directory))
            .then_with(|| {
                let comparison = match sort {
                    Sort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    Sort::Modified => a.modified.cmp(&b.modified),
                    Sort::Size => a.size.cmp(&b.size),
                    Sort::Kind => a.kind().cmp(&b.kind()),
                }
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                if ascending {
                    comparison
                } else {
                    comparison.reverse()
                }
            })
    });
    result
}
pub fn remote_entries(listing: &crate::sftp_client::Listing) -> Vec<Entry> {
    let mut entries: Vec<_> = listing
        .entries
        .iter()
        .map(|entry| Entry {
            native_path: None,
            can_open: !entry.is_directory
                && entry.mode.is_some_and(|mode| {
                    mode & 0o170000 == 0o100000
                        && can_open_with_system(&entry.name, mode & 0o111 != 0)
                }),
            path: format!("{}/{}", listing.directory.trim_end_matches('/'), entry.name),
            name: entry.name.clone(),
            directory: entry.is_directory,
            parent: false,
            size: entry.size,
            modified: entry.modified,
            mode: entry.mode,
        })
        .collect();
    if let Some(parent) = remote_parent(&listing.directory) {
        entries.insert(0, parent_entry(parent));
    }
    entries
}
pub fn remote_parent(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    if path.is_empty() || !path.starts_with('/') {
        return None;
    }
    Some(
        path.rsplit_once('/')
            .map(|(parent, _)| if parent.is_empty() { "/" } else { parent })
            .unwrap_or("/")
            .into(),
    )
}
fn parent_entry(path: String) -> Entry {
    Entry {
        native_path: None,
        can_open: false,
        path,
        name: "..".into(),
        directory: true,
        parent: true,
        size: 0,
        modified: None,
        mode: None,
    }
}
pub fn local_path(input: &str, home: &Path, current: &Path) -> Result<PathBuf> {
    // Native Windows verbatim paths preserve literal trailing spaces/dots.
    // Navigation from a listed entry must not reinterpret its exact name.
    let input = if cfg!(windows) && input.starts_with(r"\\?\") {
        input
    } else {
        input.trim()
    };
    ensure!(!input.contains('\0'), "Invalid directory path");
    let path = if input.is_empty() || input == "~" {
        home.to_owned()
    } else if let Some(relative) = input.strip_prefix("~/") {
        home.join(relative)
    } else {
        let path = PathBuf::from(input);
        if path.is_absolute() {
            path
        } else {
            current.join(path)
        }
    };
    path.canonicalize()
        .with_context(|| format!("Cannot open directory: {}", path.display()))
}
pub fn list_local(path: &Path) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    if let Some(parent) = path.parent() {
        let mut entry = parent_entry(parent.to_string_lossy().into_owned());
        entry.native_path = Some(parent.to_owned());
        entries.push(entry);
    }
    for entry in std::fs::read_dir(path)
        .with_context(|| format!("Cannot read directory: {}", path.display()))?
    {
        let entry = entry?;
        // Follow a symlink for browsing, but keep its displayed path. Broken
        // links remain visible rather than failing the entire directory.
        let metadata = std::fs::metadata(entry.path()).ok();
        entries.push(Entry {
            native_path: Some(entry.path()),
            can_open: can_open_local(&entry.path()).unwrap_or(false),
            path: entry.path().to_string_lossy().into_owned(),
            name: entry.file_name().to_string_lossy().into_owned(),
            directory: metadata.as_ref().is_some_and(|metadata| metadata.is_dir()),
            parent: false,
            size: metadata
                .as_ref()
                .map(|metadata| metadata.len())
                .unwrap_or(0),
            #[cfg(unix)]
            mode: metadata.as_ref().map(|metadata| {
                use std::os::unix::fs::MetadataExt;
                metadata.mode()
            }),
            #[cfg(not(unix))]
            mode: None,
            modified: metadata
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs()),
        });
    }
    Ok(entries)
}

pub fn new_folder_name(index: usize) -> String {
    if index == 1 {
        "New Folder".into()
    } else {
        format!("New Folder {index}")
    }
}
pub fn create_local_folder(directory: &Path) -> Result<PathBuf> {
    let directory = directory.canonicalize()?;
    ensure!(directory.is_dir(), "Choose a directory first.");
    for index in 1.. {
        let target = directory.join(new_folder_name(index));
        match std::fs::create_dir(&target) {
            Ok(()) => return Ok(target),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("Could not create the local folder"),
        }
    }
    unreachable!()
}
pub fn can_open_with_system(name: &str, executable: bool) -> bool {
    if executable {
        return false;
    }
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    // Keep aligned with terminal/external_editor_catalog.dart in the reference.
    ![
        "app",
        "appimage",
        "appref-ms",
        "appx",
        "appxbundle",
        "apk",
        "bash",
        "bat",
        "cmd",
        "com",
        "command",
        "cpl",
        "csh",
        "deb",
        "desktop",
        "exe",
        "fish",
        "gadget",
        "hta",
        "jar",
        "js",
        "jse",
        "ksh",
        "lnk",
        "msi",
        "msp",
        "msix",
        "msixbundle",
        "mpkg",
        "package",
        "pif",
        "pkg",
        "pl",
        "ps1",
        "py",
        "pyw",
        "rb",
        "reg",
        "rpm",
        "run",
        "scf",
        "scr",
        "sh",
        "url",
        "vb",
        "vbe",
        "vbs",
        "workflow",
        "wsf",
        "wsh",
        "zsh",
    ]
    .contains(&extension.as_str())
}
pub fn can_open_local(path: &Path) -> Result<bool> {
    let metadata = std::fs::metadata(path)?;
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let executable = false;
    Ok(metadata.is_file()
        && can_open_with_system(
            &path.file_name().unwrap_or_default().to_string_lossy(),
            executable,
        ))
}
#[derive(Clone, Copy)]
pub enum Navigation {
    Visit,
    Back,
    Forward,
    Refresh,
}
#[derive(Clone, Default)]
pub struct History {
    pub path: String,
    pub back: Vec<String>,
    pub forward: Vec<String>,
}
impl History {
    pub fn commit(&mut self, path: String, navigation: Navigation) {
        if path == self.path {
            return;
        }
        match navigation {
            Navigation::Visit => {
                if !self.path.is_empty() {
                    self.back.push(self.path.clone());
                }
                self.forward.clear();
            }
            Navigation::Back => {
                self.back.pop();
                self.forward.push(self.path.clone());
            }
            Navigation::Forward => {
                self.forward.pop();
                self.back.push(self.path.clone());
            }
            Navigation::Refresh => {}
        }
        self.path = path;
    }
}
#[derive(Clone, Default)]
pub struct Selection {
    pub paths: BTreeSet<String>,
    pub primary: Option<String>,
    anchor: Option<String>,
}
impl Selection {
    pub fn single(path: String) -> Self {
        Self {
            paths: [path.clone()].into(),
            primary: Some(path.clone()),
            anchor: Some(path),
        }
    }
    pub fn select(&mut self, path: &str, entries: &[Entry], additive: bool, range: bool) {
        let parent = entries
            .iter()
            .any(|entry| entry.path == path && entry.parent);
        if range && !parent {
            let anchor = self
                .anchor
                .as_deref()
                .or(self.primary.as_deref())
                .unwrap_or(path);
            let selectable: Vec<_> = entries.iter().filter(|entry| !entry.parent).collect();
            let selected = selectable
                .iter()
                .position(|entry| entry.path == anchor)
                .zip(selectable.iter().position(|entry| entry.path == path))
                .map(|(a, b)| {
                    selectable[a.min(b)..=a.max(b)]
                        .iter()
                        .map(|entry| entry.path.clone())
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_else(|| BTreeSet::from([path.into()]));
            if additive {
                self.paths.extend(selected);
            } else {
                self.paths = selected;
            }
        } else if additive && !parent {
            if !self.paths.remove(path) {
                self.paths.insert(path.into());
            }
            self.anchor = Some(path.into());
        } else {
            self.paths = BTreeSet::from([path.into()]);
            self.anchor = (!parent).then(|| path.into());
        }
        self.primary = Some(path.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(name: &str, directory: bool) -> Entry {
        Entry {
            native_path: None,
            can_open: false,
            path: format!("/test/{name}"),
            name: name.into(),
            directory,
            parent: false,
            size: 0,
            modified: None,
            mode: None,
        }
    }
    #[test]
    fn local_folder_creation_skips_existing_files_and_default_open_matches_reference_policy() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("New Folder"), b"do not change").unwrap();
        let folder = super::create_local_folder(temp.path()).unwrap();
        assert_eq!(folder.file_name().unwrap(), "New Folder 2");
        assert_eq!(
            std::fs::read(temp.path().join("New Folder")).unwrap(),
            b"do not change"
        );
        assert!(super::can_open_with_system("notes.txt", false));
        for extension in [
            "EXE", "command", "sh", "py", "appimage", "desktop", "pkg", "workflow", "ps1",
        ] {
            assert!(!super::can_open_with_system(
                &format!("file.{extension}"),
                false
            ));
        }
        assert!(!super::can_open_with_system("notes.txt", true));
        let source = include_str!("../../lib/terminal/external_editor_catalog.dart");
        let list = source
            .split("_unsafeSystemDefaultApplicationExtensions = <String>{")
            .nth(1)
            .unwrap()
            .split("};")
            .next()
            .unwrap();
        for line in list
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with('\''))
        {
            let extension = line.trim_matches([',', '\'']);
            assert!(
                !super::can_open_with_system(&format!("file.{extension}"), false),
                "{extension}"
            );
        }
    }
    #[test]
    fn compact_metadata_keeps_permissions_and_omits_directory_size_and_parent_metadata() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_size(100 * 1024), "100 KB");
        let mut file = entry("notes.txt", false);
        file.mode = Some(0o100644);
        file.size = 1536;
        assert_eq!(file.compact_subtitle(), "-rw-r--r-- · 1.5 KB");
        file.directory = true;
        file.mode = Some(0o041775);
        assert_eq!(file.compact_subtitle(), "drwxrwxr-t");
        file.parent = true;
        assert!(file.compact_subtitle().is_empty());
        assert_eq!(permissions(0o104700), "-rws------");
        assert_eq!(permissions(0o102600), "-rw---S---");
    }
    #[test]
    fn filter_and_sort_keep_parent_and_directories_first_in_both_directions() {
        let entries = vec![
            entry("z.txt", false),
            entry("Alpha", true),
            entry(".hidden", false),
            parent_entry("/".into()),
        ];
        let names = |entries: Vec<Entry>| {
            entries
                .into_iter()
                .map(|entry| entry.name)
                .collect::<Vec<_>>()
        };
        for ascending in [true, false] {
            assert_eq!(
                names(visible(&entries, "", false, Sort::Name, ascending)),
                vec!["..", "Alpha", "z.txt"]
            );
        }
        assert_eq!(
            names(visible(&entries, "Z.TXT", true, Sort::Name, true)),
            vec!["..", "z.txt"]
        );
        assert_eq!(visible(&entries, "", true, Sort::Name, true).len(), 4);
    }
    #[test]
    fn history_commits_only_successful_navigation_and_preserves_forward_after_refresh() {
        let mut history = History::default();
        history.commit("/a".into(), Navigation::Visit);
        history.commit("/b".into(), Navigation::Visit);
        history.commit("/a".into(), Navigation::Back);
        history.commit("/a".into(), Navigation::Refresh);
        assert_eq!(history.forward, vec!["/b"]);
        history.commit("/b".into(), Navigation::Forward);
        assert_eq!(history.back, vec!["/a"]);
        history.commit("/c".into(), Navigation::Visit);
        assert!(history.forward.is_empty());
    }
    #[test]
    fn modifier_selection_and_range_never_include_parent_in_bulk_selection() {
        let entries = vec![
            parent_entry("/".into()),
            entry("a", false),
            entry("b", false),
            entry("c", false),
        ];
        let mut selection = Selection::default();
        selection.select("/test/a", &entries, false, false);
        selection.select("/test/c", &entries, false, true);
        assert_eq!(selection.paths.len(), 3);
        selection.select("/test/b", &entries, true, false);
        assert_eq!(selection.paths.len(), 2);
        selection.select("/", &entries, true, true);
        assert_eq!(selection.paths, BTreeSet::from(["/".into()]));
    }
    #[test]
    fn actual_local_listing_navigation_and_unicode_paths() -> Result<()> {
        let temp = tempfile::tempdir()?;
        std::fs::create_dir(temp.path().join("中文"))?;
        std::fs::write(temp.path().join("file.txt"), "fixture")?;
        let path = local_path("~", temp.path(), temp.path())?;
        let entries = list_local(&path)?;
        assert!(
            entries
                .iter()
                .any(|entry| entry.name == "中文" && entry.directory)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.name == "file.txt" && entry.size == 7)
        );
        assert_eq!(
            local_path("中文", temp.path(), temp.path())?,
            temp.path().join("中文").canonicalize()?
        );
        assert!(local_path("missing", temp.path(), temp.path()).is_err());
        assert_eq!(remote_parent("/"), None);
        assert_eq!(remote_parent("/home/fixture/"), Some("/home".into()));
        Ok(())
    }
    #[test]
    fn remote_default_open_requires_regular_nonexecutable_permission_metadata() {
        for (name, mode, expected) in [
            ("file.txt", None, false),
            ("file.txt", Some(0o100644), true),
            ("file.txt", Some(0o100755), false),
            ("file.sh", Some(0o100644), false),
            ("fifo.txt", Some(0o010644), false),
        ] {
            let mut wire = serde_json::json!({ "directory": "/", "entries": [{"name": name, "is_directory": false, "size": 0, "modified": null}], "events": [], "error": null });
            if let Some(mode) = mode {
                wire["entries"][0]["mode"] = serde_json::json!(mode);
            }
            let listing: crate::sftp_client::Listing = serde_json::from_value(wire).unwrap();
            assert_eq!(
                super::remote_entries(&listing)[0].can_open,
                expected,
                "{name} {mode:?}"
            );
        }
    }
}
