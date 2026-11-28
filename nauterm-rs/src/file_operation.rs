//! Public, credential-free task payloads matching the reference native API.
use anyhow::{Result, ensure};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    Mkdir {
        target_path: String,
    },
    Move {
        source_path: String,
        target_path: String,
        replace_existing: bool,
    },
    Copy {
        source_path: String,
        target_path: String,
        replace_existing: bool,
    },
    Delete {
        target_path: String,
    },
    Upload {
        local_path: String,
        remote_path: String,
        replace_existing: bool,
        transfer_threads: usize,
    },
    Download {
        remote_path: String,
        local_path: String,
        transfer_threads: usize,
    },
}
impl Operation {
    /// Serialize overlapping trees, including local download/upload paths,
    /// while allowing unrelated transfers to use the queue's parallel slots.
    pub fn conflicts(&self, other: &Self, same_server: bool) -> bool {
        fn overlaps(a: &str, b: &str) -> bool {
            a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
        }
        fn remote(operation: &Operation) -> Vec<&str> {
            match operation {
                Operation::Copy {
                    source_path,
                    target_path,
                    ..
                }
                | Operation::Move {
                    source_path,
                    target_path,
                    ..
                } => vec![source_path, target_path],
                Operation::Upload { remote_path, .. } | Operation::Download { remote_path, .. } => {
                    vec![remote_path]
                }
                Operation::Mkdir { target_path } | Operation::Delete { target_path } => {
                    vec![target_path]
                }
            }
        }
        fn local(operation: &Operation) -> Option<&str> {
            match operation {
                Operation::Upload { local_path, .. } | Operation::Download { local_path, .. } => {
                    Some(local_path)
                }
                _ => None,
            }
        }
        fn local_overlaps(a: &str, b: &str) -> bool {
            #[cfg(target_os = "windows")]
            {
                fn normalized(path: &str) -> String {
                    let path = path.replace('\\', "/").to_ascii_lowercase();
                    let path = if let Some(unc) = path.strip_prefix("//?/unc/") {
                        format!("//{unc}")
                    } else {
                        path.strip_prefix("//?/").unwrap_or(&path).to_owned()
                    };
                    path.trim_end_matches('/').to_owned()
                }
                overlaps(&normalized(a), &normalized(b))
            }
            #[cfg(not(target_os = "windows"))]
            overlaps(a, b)
        }
        (same_server
            && remote(self)
                .iter()
                .any(|a| remote(other).iter().any(|b| overlaps(a, b))))
            || local(self)
                .zip(local(other))
                .is_some_and(|(a, b)| local_overlaps(a, b))
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Mkdir { .. } => "New Folder",
            Self::Move { .. } => "Move",
            Self::Copy { .. } => "Copy",
            Self::Delete { .. } => "Delete",
            Self::Upload { .. } => "Upload",
            Self::Download { .. } => "Download",
        }
    }
    pub fn target(&self) -> &str {
        match self {
            Self::Mkdir { target_path }
            | Self::Delete { target_path }
            | Self::Move { target_path, .. }
            | Self::Copy { target_path, .. } => target_path,
            Self::Upload { remote_path, .. } => remote_path,
            Self::Download { local_path, .. } => local_path,
        }
    }
    pub fn source(&self) -> &str {
        match self {
            Self::Move { source_path, .. } | Self::Copy { source_path, .. } => source_path,
            Self::Upload { local_path, .. } => local_path,
            Self::Download { remote_path, .. } => remote_path,
            _ => self.target(),
        }
    }
    pub fn pausable(&self) -> bool {
        matches!(self, Self::Upload { .. } | Self::Download { .. })
    }
    pub fn set_transfer_threads(&mut self, threads: usize) {
        if let Self::Upload {
            transfer_threads, ..
        }
        | Self::Download {
            transfer_threads, ..
        } = self
        {
            *transfer_threads = threads;
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Mkdir { target_path } | Self::Delete { target_path } => {
                remote_path(target_path, false)?
            }
            Self::Move {
                source_path,
                target_path,
                ..
            }
            | Self::Copy {
                source_path,
                target_path,
                ..
            } => {
                remote_path(source_path, false)?;
                remote_path(target_path, false)?;
                ensure!(
                    source_path != target_path,
                    "Source and destination are the same."
                );
                ensure!(
                    !target_path.starts_with(&format!("{source_path}/")),
                    "A folder cannot be placed inside itself."
                );
                ensure!(
                    !source_path.starts_with(&format!("{target_path}/")),
                    "A parent cannot be replaced by one of its children."
                );
            }
            Self::Upload {
                local_path: path,
                remote_path: remote,
                transfer_threads,
                ..
            }
            | Self::Download {
                local_path: path,
                remote_path: remote,
                transfer_threads,
            } => {
                remote_path(remote, false)?;
                ensure!(
                    Path::new(path).is_absolute()
                        && !path.contains('\0')
                        && Path::new(path).file_name().is_some(),
                    "An absolute local file path is required."
                );
                ensure!(
                    (1..=32).contains(transfer_threads),
                    "Invalid transfer concurrency."
                );
            }
        }
        Ok(())
    }
}
fn remote_path(path: &str, root: bool) -> Result<()> {
    ensure!(
        path.starts_with('/') && path.len() <= 64 * 1024 && !path.contains('\0'),
        "An absolute remote path is required."
    );
    ensure!(
        !path.split('/').any(|part| [".", ".."].contains(&part)),
        "Resolve relative path components before starting a task."
    );
    ensure!(
        root || !path.trim_end_matches('/').is_empty(),
        "This operation cannot target the filesystem root."
    );
    Ok(())
}
pub fn filename(name: &str) -> Result<()> {
    ensure!(
        !name.trim().is_empty()
            && ![".", ".."].contains(&name)
            && !name.contains(['/', '\0', '\r', '\n']),
        "Enter a valid name."
    );
    Ok(())
}
pub fn destination(input: &str, base: &str) -> Result<String> {
    ensure!(
        !input.trim().is_empty() && !input.contains('\0'),
        "Enter a destination path."
    );
    let input = if input.starts_with('/') {
        input.to_owned()
    } else {
        format!("{base}/{input}")
    };
    let mut parts = vec![];
    for part in input.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let path = format!("/{}", parts.join("/"));
    remote_path(&path, false)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "windows")]
    #[test]
    fn native_local_tree_conflicts_preserve_drive_unc_and_verbatim_path_semantics() {
        let upload = |path: &str| Operation::Upload {
            remote_path: "/source".into(),
            local_path: path.into(),
            replace_existing: false,
            transfer_threads: 8,
        };
        let download = |path: &str| Operation::Download {
            remote_path: "/target".into(),
            local_path: path.into(),
            transfer_threads: 8,
        };
        for (parent, child) in [
            (
                r"C:\Data\中文 & folder",
                r"c:/data/中文 & folder/report.txt",
            ),
            (r"\\server\share\目录", r"\\SERVER\share\目录\report.txt"),
            (r"\\?\C:\Data", r"C:\Data\report.txt"),
            (
                r"\\?\UNC\server\share\目录",
                r"\\server\share\目录\report.txt",
            ),
        ] {
            assert!(upload(parent).conflicts(&download(child), false));
            assert!(download(child).conflicts(&upload(parent), false));
        }
        assert!(!upload(r"C:\Data").conflicts(&download(r"D:\Data\file"), false));
        assert!(!upload(r"C:\Data").conflicts(&download(r"C:\Database\file"), false));
        assert!(!upload(r"\\server\share\a").conflicts(&download(r"\\server\other\a\b"), false));
        assert!(
            !Operation::Delete {
                target_path: "/Data".into()
            }
            .conflicts(
                &Operation::Delete {
                    target_path: "/data/child".into()
                },
                true
            )
        );
    }
    #[test]
    fn overlapping_remote_trees_and_shared_local_targets_are_serialized() {
        let deletion = Operation::Delete {
            target_path: "/folder".into(),
        };
        let upload = Operation::Upload {
            remote_path: "/folder/child".into(),
            local_path: "/local/file".into(),
            replace_existing: false,
            transfer_threads: 8,
        };
        assert!(deletion.conflicts(&upload, true));
        assert!(!deletion.conflicts(&upload, false));
        let download = Operation::Download {
            remote_path: "/unrelated".into(),
            local_path: "/local/file".into(),
            transfer_threads: 8,
        };
        assert!(download.conflicts(&upload, false));
        assert!(!deletion.conflicts(&download, true));
    }
    #[test]
    fn task_payloads_are_typed_public_only_and_fail_closed_on_root_or_self_targets() {
        let operation = Operation::Move {
            source_path: "/home/fixture/a".into(),
            target_path: "/home/fixture/b".into(),
            replace_existing: false,
        };
        operation.validate().unwrap();
        let wire = serde_json::to_value(&operation).unwrap();
        assert_eq!(wire["op"], "move");
        assert_eq!(wire["replace_existing"], false);
        for root in ["/", "////", "/a/../", "relative"] {
            assert!(
                Operation::Delete {
                    target_path: root.into()
                }
                .validate()
                .is_err()
            );
        }
        for target in ["/a", "/a/b"] {
            assert!(
                Operation::Copy {
                    source_path: "/a".into(),
                    target_path: target.into(),
                    replace_existing: true
                }
                .validate()
                .is_err()
            );
        }
        assert_eq!(
            destination("../中文", "/home/fixture").unwrap(),
            "/home/中文"
        );
        for name in ["", ".", "..", "a/b", "a\0b"] {
            assert!(filename(name).is_err());
        }
    }
}
