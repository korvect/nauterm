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
