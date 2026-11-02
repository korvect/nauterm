//! Shallow record-owned capture cleanup, serialized with writes/checkpoints.
use super::*;

pub(super) const COMPACTION_PREFIX: &str = ".nauterm-recording-compact-";
fn owned_capture(name: &str) -> bool {
    name.get(..36)
        .is_some_and(|id| valid_identifier(id) && capture_name(id, name))
}
fn state_capture(name: &str) -> Option<&str> {
    if let Some(capture) = name.strip_suffix(".state") {
        return owned_capture(capture).then_some(capture);
    }
    let (capture, suffix) = name.rsplit_once(".state.")?;
    let token = suffix.strip_suffix(".tmp")?;
    (owned_capture(capture)
        && token.len() == 16
        && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then_some(capture)
}
fn directory_entries(directory: &Path) -> Result<Vec<fs::DirEntry>> {
    match fs::symlink_metadata(directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error.into()),
        Ok(metadata) => ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "Recording directory must be a real directory."
        ),
    }
    Ok(fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?)
}
pub(super) fn disk_usage(directory: &Path) -> Result<u64> {
    directory_entries(directory)?
        .into_iter()
        .try_fold(0u64, |bytes, entry| {
            let metadata = fs::symlink_metadata(entry.path())?;
            Ok(bytes.saturating_add(
                if metadata.is_file() && !metadata.file_type().is_symlink() {
                    metadata.len()
                } else {
                    0
                },
            ))
        })
}
fn stage_recording_id(name: &str) -> Option<&str> {
    let rest = name.strip_prefix(COMPACTION_PREFIX)?;
    let id = rest.get(..36).filter(|id| valid_identifier(id))?;
    let token = rest.get(36..)?.strip_prefix('-')?;
    (token.len() == 6 && token.bytes().all(|byte| byte.is_ascii_alphanumeric())).then_some(id)
}
fn abandoned_stage(path: &Path, name: &str) -> Result<bool> {
    if stage_recording_id(name).is_none() {
        return Ok(false);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        let name = entry.file_name();
        let name = name.to_str().unwrap_or("");
        let allowed = name == "content"
            || name == "content.state"
            || name
                .strip_prefix("content.state.")
                .and_then(|name| name.strip_suffix(".tmp"))
                .is_some_and(|token| {
                    token.len() == 16 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
                });
        if !allowed || !metadata.is_file() || metadata.file_type().is_symlink() {
            return Ok(false);
        }
    }
    Ok(true)
}
fn sweep(
    directory: &Path,
    references: &BTreeMap<String, bool>,
    stages: bool,
    mut lookup: impl FnMut(&str) -> Result<Option<bool>>,
) -> Result<usize> {
    for name in references.keys() {
        ensure!(
            owned_capture(name),
            "Invalid referenced recording filename."
        );
    }
    let mut removed = 0;
    for entry in directory_entries(directory)? {
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if metadata.is_file() {
            let (capture, state) = if owned_capture(name) {
                (name, false)
            } else if let Some(capture) = state_capture(name) {
                (capture, true)
            } else {
                continue;
            };
            let id = nauterm_core::recording_lease::recording_id_for_file(capture)
                .context("Invalid recording filename")?;
            let lease = if references.get(capture) == Some(&true) {
                RecordingLease::try_acquire_existing(directory, id)?
            } else {
                RecordingLease::try_acquire(directory, id)?
            };
            let Some(_lease) = lease else {
                continue;
            };
            // A writer may have published after the first directory/catalog
            // snapshot. Refresh its exact reference only after acquiring it.
            let reference = lookup(capture)?;
            if if state {
                reference != Some(true)
            } else {
                reference.is_none()
            } {
                fs::remove_file(entry.path())?;
                removed += 1;
            }
        } else if stages
            && metadata.is_dir()
            && let Some(id) = stage_recording_id(name)
        {
            let Some(_lease) = RecordingLease::try_acquire(directory, id)? else {
                continue;
            };
            if abandoned_stage(&entry.path(), name)? {
                fs::remove_dir_all(entry.path())?;
                removed += 1;
            }
        }
    }
    Ok(removed)
}
impl Runtime {
    pub(super) fn cleanup_orphans(&mut self, stages: bool) -> Result<()> {
        let mut references = BTreeMap::new();
        let mut entries = self.entries()?;
        entries.extend(self.active.values().map(|active| active.entry.clone()));
        for entry in entries {
            if entry.capture_file.is_empty() {
                continue;
            }
            self.capture_path(&entry)?;
            references
                .entry(entry.capture_file)
                .and_modify(|unfinished| *unfinished |= entry.capture_sha256.is_none())
                .or_insert(entry.capture_sha256.is_none());
        }
        if sweep(&self.directory, &references, stages, |capture| {
            let id = nauterm_core::recording_lease::recording_id_for_file(capture)
                .context("Invalid recording filename")?;
            Ok(self
                .entry(id)?
                .filter(|entry| entry.capture_file == capture)
                .map(|entry| entry.capture_sha256.is_none()))
        })? > 0
        {
            let _ = self.notify.try_send(());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_preserves_references_recovery_state_and_unrelated_items() -> Result<()> {
        let root = tempfile::tempdir()?;
        let directory = root.path().join("recordings");
        fs::create_dir(&directory)?;
        let active = format!("{}.ntrcap", identifier());
        let completed = format!("{}.{:032x}.ntrcap", identifier(), 9);
        let orphan = format!("{}.{:032x}.ntrcap", identifier(), 10);
        let active_state = format!("{active}.state");
        let active_temp = format!("{active}.state.0123456789abcdef.tmp");
        let completed_state = format!("{completed}.state");
        let orphan_state = format!("{orphan}.state");
        for name in [
            &active,
            &completed,
            &orphan,
            &active_state,
            &active_temp,
            &completed_state,
            &orphan_state,
        ] {
            fs::write(directory.join(name), b"fixture")?;
        }
        fs::write(directory.join("notes.txt"), b"untouched")?;
        let outside = root.path().join("outside");
        fs::write(&outside, b"outside sentinel")?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, directory.join(format!("{}.ntrcap", identifier())))?;
        let stage = directory.join(format!("{COMPACTION_PREFIX}{}-Ab123x", identifier()));
        fs::create_dir(&stage)?;
        fs::write(stage.join("content"), b"encrypted staging fixture")?;
        let download = directory.join(".nauterm-download-Ab123x");
        fs::create_dir(&download)?;
        fs::write(download.join("content"), b"unrelated download")?;
        let unknown = directory.join(format!("{COMPACTION_PREFIX}{}-Cd456y", identifier()));
        fs::create_dir(&unknown)?;
        fs::write(unknown.join("unknown-file"), b"untouched")?;
        assert!(
            sweep(
                &directory,
                &BTreeMap::from([("../invalid".into(), false)]),
                true,
                |_| Ok(None)
            )
            .is_err()
        );
        assert!(
            directory.join(&orphan).exists(),
            "Invalid catalog must abort before deleting anything"
        );
        let references = BTreeMap::from([(active.clone(), true), (completed.clone(), false)]);
        sweep(&directory, &references, false, |file| {
            Ok(references.get(file).copied())
        })?;
        for name in [&active, &completed, &active_state, &active_temp] {
            assert!(directory.join(name).is_file());
        }
        for name in [&orphan, &completed_state, &orphan_state] {
            assert!(!directory.join(name).exists());
        }
        assert!(
            stage.exists(),
            "Live maintenance does not remove staging directories"
        );
        sweep(&directory, &references, true, |file| {
            Ok(references.get(file).copied())
        })?;
        assert!(!stage.exists());
        assert!(download.join("content").is_file() && unknown.join("unknown-file").is_file());
        assert_eq!(fs::read(directory.join("notes.txt"))?, b"untouched");
        assert_eq!(fs::read(&outside)?, b"outside sentinel");
        let expected = fs::read_dir(&directory)?
            .filter_map(|entry| {
                let metadata = fs::symlink_metadata(entry.ok()?.path()).ok()?;
                metadata.is_file().then_some(metadata.len())
            })
            .sum::<u64>();
        assert_eq!(disk_usage(&directory)?, expected);
        assert_eq!(disk_usage(&root.path().join("missing"))?, 0);
        #[cfg(unix)]
        {
            let alias = root.path().join("alias");
            std::os::unix::fs::symlink(&directory, &alias)?;
            assert!(disk_usage(&alias).is_err());
            assert!(
                sweep(&alias, &references, true, |file| Ok(references
                    .get(file)
                    .copied()))
                .is_err()
            );
        }
        Ok(())
    }
}
