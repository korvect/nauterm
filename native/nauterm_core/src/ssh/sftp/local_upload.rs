//! Inspect a local upload once. Nested links are copied as links, not followed.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) struct Plan {
    pub directory: bool,
    pub total: u64,
    pub directories: Vec<String>,
    pub files: Vec<(PathBuf, String)>,
    /// Relative destination and the exact link text, including relative links.
    pub links: Vec<(String, String)>,
}

fn check_cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::SeqCst) {
        Err("SFTP task cancelled.".into())
    } else {
        Ok(())
    }
}

impl Plan {
    pub fn read(path: &Path, cancel: &AtomicBool) -> Result<Self, String> {
        check_cancelled(cancel)?;
        // Preserve the existing chooser policy: follow a link selected as the
        // upload root, while preserving links contained inside a directory.
        let metadata = fs::metadata(path)
            .map_err(|error| format!("local path not found {}: {error}", path.display()))?;
        if !metadata.is_dir() && !metadata.is_file() {
            return Err(format!(
                "local upload is not a regular file or folder: {}",
                path.display()
            ));
        }
        let mut plan = Self {
            directory: metadata.is_dir(),
            total: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
            directories: vec![],
            files: vec![],
            links: vec![],
        };
        if !plan.directory {
            return Ok(plan);
        }
        let mut pending = vec![(path.to_path_buf(), String::new())];
        while let Some((directory, relative)) = pending.pop() {
            check_cancelled(cancel)?;
            let entries = fs::read_dir(&directory).map_err(|error| {
                format!(
                    "failed to read local folder {}: {error}",
                    directory.display()
                )
            })?;
            for entry in entries {
                check_cancelled(cancel)?;
                let entry = entry.map_err(|error| {
                    format!(
                        "failed to read local folder {}: {error}",
                        directory.display()
                    )
                })?;
                let local = entry.path();
                let name = entry.file_name().into_string().map_err(|_| {
                    format!(
                        "local filename cannot be represented by SFTP: {}",
                        local.display()
                    )
                })?;
                let destination = if relative.is_empty() {
                    name
                } else {
                    format!("{relative}/{name}")
                };
                let metadata = entry.metadata().map_err(|error| {
                    format!("failed to inspect local path {}: {error}", local.display())
                })?;
                if metadata.file_type().is_symlink() {
                    let target = fs::read_link(&local).map_err(|error| {
                        format!(
                            "failed to read local symbolic link {}: {error}",
                            local.display()
                        )
                    })?;
                    let target = target.into_os_string().into_string().map_err(|_| {
                        format!(
                            "symbolic link target cannot be represented by SFTP: {}",
                            local.display()
                        )
                    })?;
                    plan.links.push((destination, target));
                } else if metadata.is_dir() {
                    plan.directories.push(destination.clone());
                    pending.push((local, destination));
                } else if metadata.is_file() {
                    plan.total = plan.total.saturating_add(metadata.len());
                    plan.files.push((local, destination));
                } else {
                    return Err(format!(
                        "local upload is not a regular file or folder: {}",
                        local.display()
                    ));
                }
            }
        }
        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_files_directories_and_cancellation() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        fs::create_dir(root.join("empty")).unwrap();
        fs::create_dir(root.join("nested")).unwrap();
        fs::write(root.join("nested/文件.txt"), b"hello").unwrap();
        let cancel = AtomicBool::new(false);
        let plan = Plan::read(root, &cancel).unwrap();
        assert!(plan.directory);
        assert_eq!(plan.total, 5);
        assert_eq!(
            plan.files,
            vec![(root.join("nested/文件.txt"), "nested/文件.txt".into())]
        );
        assert!(plan.directories.contains(&"empty".into()));
        assert!(plan.links.is_empty());
        let file = Plan::read(&root.join("nested/文件.txt"), &cancel).unwrap();
        assert!(!file.directory);
        assert_eq!(file.total, 5);
        cancel.store(true, Ordering::SeqCst);
        assert_eq!(
            Plan::read(root, &cancel).err().as_deref(),
            Some("SFTP task cancelled.")
        );
    }

    #[cfg(unix)]
    #[test]
    fn framework_links_keep_their_targets_without_duplication_or_recursion() {
        use std::os::unix::fs::symlink;
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("Sparkle.framework");
        fs::create_dir_all(root.join("Versions/B/PrivateHeaders")).unwrap();
        fs::write(root.join("Versions/B/Sparkle"), b"binary").unwrap();
        fs::write(root.join("Versions/B/PrivateHeaders/header.h"), b"header").unwrap();
        symlink("B", root.join("Versions/Current")).unwrap();
        symlink(
            "Versions/Current/PrivateHeaders",
            root.join("PrivateHeaders"),
        )
        .unwrap();
        symlink("Versions/Current/Sparkle", root.join("Sparkle")).unwrap();
        symlink(".", root.join("cycle")).unwrap();
        symlink("missing", root.join("broken")).unwrap();
        let cancel = AtomicBool::new(false);
        let plan = Plan::read(&root, &cancel).unwrap();
        assert_eq!(plan.total, 12);
        assert_eq!(plan.files.len(), 2);
        assert_eq!(plan.links.len(), 5);
        assert!(plan.links.contains(&(
            "PrivateHeaders".into(),
            "Versions/Current/PrivateHeaders".into()
        )));
        assert!(plan
            .links
            .contains(&("Versions/Current".into(), "B".into())));
        assert!(plan.links.contains(&("broken".into(), "missing".into())));
        assert!(plan.links.contains(&("cycle".into(), ".".into())));
        assert!(!plan
            .directories
            .iter()
            .any(|path| path.contains("Current") || path == "PrivateHeaders"));
        let alias = temporary.path().join("selected-alias");
        symlink(&root, &alias).unwrap();
        let selected = Plan::read(&alias, &cancel).unwrap();
        assert!(selected.directory);
        assert_eq!(selected.total, plan.total);
        assert_eq!(selected.links.len(), plan.links.len());
    }
}
