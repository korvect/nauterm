//! OS file drops are planned on a worker, then preflighted one item at a time.
//! Same-named files from different folders retain independent conflict choices.
use crate::{
    file_model::Entry,
    native::Core,
    pane_transfer::{Batch, Endpoint},
};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
};

pub struct Uploads {
    pending: VecDeque<Batch>,
    pub reserved: HashSet<String>,
    pub skipped: usize,
}
impl Uploads {
    pub fn plan(
        paths: Vec<PathBuf>,
        target: Endpoint,
        owner: u64,
        core: Option<Arc<Core>>,
    ) -> Result<Self> {
        ensure!(
            matches!(target, Endpoint::Remote { .. }),
            "Drop files onto a connected remote pane."
        );
        let mut seen = HashSet::new();
        let mut pending = VecDeque::new();
        let mut skipped = 0;
        for path in paths {
            if !seen.insert(path.clone()) {
                continue;
            }
            ensure!(
                path.is_absolute(),
                "A dropped file needs an absolute path. No items were queued."
            );
            let text = path
                .to_str()
                .context("This filename cannot be represented by SFTP. No items were queued.")?;
            ensure!(
                !text.contains('\0'),
                "Invalid dropped filename. No items were queued."
            );
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                skipped += 1;
                continue;
            };
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    skipped += 1;
                    continue;
                }
                Err(error) => {
                    return Err(error)
                        .context("Could not inspect the dropped files. No items were queued.");
                }
            };
            // Match native upload's handling of a selected symlink, including
            // its displayed filename. Do not canonicalize or trim user paths.
            let link = metadata.file_type().is_symlink();
            let metadata = if link {
                std::fs::metadata(&path).unwrap_or(metadata)
            } else {
                metadata
            };
            // A broken selected link stays an ordinary failed upload, visible
            // in the queue; it must not discard later files in this drop.
            ensure!(
                metadata.is_file() || metadata.is_dir() || link,
                "Only files and folders can be uploaded. No items were queued."
            );
            let entry = Entry {
                native_path: Some(path.clone()),
                can_open: false,
                path: text.into(),
                name: name.into(),
                directory: metadata.is_dir(),
                parent: false,
                size: metadata.len(),
                modified: None,
                mode: None,
            };
            pending.push_back(Batch::plan(
                Endpoint::Local(
                    path.parent()
                        .context("Choose a file, not a filesystem root.")?
                        .to_owned(),
                ),
                target.clone(),
                vec![entry],
                owner,
                owner,
                core.clone(),
            )?);
        }
        Ok(Self {
            pending,
            reserved: HashSet::new(),
            skipped,
        })
    }
    pub fn next(&mut self) -> Option<Batch> {
        self.pending.pop_front()
    }
    pub fn reserve(&mut self, targets: impl IntoIterator<Item = String>) {
        self.reserved.extend(targets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{file_operation::Operation, native::SshOptions};

    fn target() -> Endpoint {
        Endpoint::Remote {
            directory: "/remote destination".into(),
            options: Arc::new(SshOptions::new(
                "fixture.invalid".into(),
                22,
                "fixture".into(),
                PathBuf::from("unused-known-hosts"),
            )),
        }
    }

    #[test]
    fn dropped_unicode_and_metacharacter_paths_remain_literal_transfer_arguments() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let directory = temp.path().join("本地 目录 & tools");
        std::fs::create_dir(&directory)?;
        let file = directory.join("报告 %PATH% & 1.txt");
        std::fs::write(&file, b"synthetic upload fixture")?;
        let mut uploads = Uploads::plan(
            vec![file.clone(), file.clone(), directory.join("missing.txt")],
            target(),
            1,
            None,
        )?;
        assert_eq!(uploads.skipped, 1);
        let batch = uploads.next().context("Missing upload plan")?;
        let operations = batch.operations();
        let [
            Operation::Upload {
                local_path,
                remote_path,
                ..
            },
        ] = operations.as_slice()
        else {
            anyhow::bail!("Expected a single upload operation");
        };
        assert_eq!(local_path, file.to_str().unwrap());
        assert_eq!(remote_path, "/remote destination/报告 %PATH% & 1.txt");
        assert!(uploads.next().is_none());
        Ok(())
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_drag_plan_preserves_drive_and_unc_paths_without_shell_quoting() -> Result<()> {
        // Planning is local and credential-free; the synthetic UNC spelling
        // verifies native path handling without contacting a network share.
        for path in [
            r"C:\Users\fixture\文件 & tools\报告 final.txt",
            r"\\server\共享 目录\文件 & tools\报告 final.txt",
            r"\\?\C:\Users\fixture\报告 final.txt",
        ] {
            let path = PathBuf::from(path);
            let entry = Entry {
                native_path: Some(path.clone()),
                can_open: false,
                path: path.to_str().unwrap().into(),
                name: path.file_name().unwrap().to_str().unwrap().into(),
                directory: false,
                parent: false,
                size: 0,
                modified: None,
                mode: None,
            };
            let batch = Batch::plan(
                Endpoint::Local(path.parent().unwrap().into()),
                target(),
                vec![entry],
                1,
                1,
                None,
            )?;
            let operations = batch.operations();
            assert_eq!(operations[0].source(), path.to_str().unwrap());
            assert_eq!(operations[0].target(), "/remote destination/报告 final.txt");
        }
        Ok(())
    }
}
