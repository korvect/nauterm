//! Downloads stay in a private directory on the destination filesystem until
//! complete. Publication never silently overwrites a race-created destination.
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

fn private_directory_in(parent: impl AsRef<Path>, prefix: &str) -> Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    Ok(builder.tempdir_in(parent)?)
}

#[derive(Debug, PartialEq, Eq)]
struct Stamp {
    kind: fs::FileType,
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64),
    #[cfg(unix)]
    permissions: u32,
}
fn stamp(path: &Path) -> io::Result<Option<Stamp>> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(Some(Stamp {
        kind: meta.file_type(),
        len: meta.len(),
        modified: meta.modified().ok(),
        #[cfg(unix)]
        identity: {
            use std::os::unix::fs::MetadataExt;
            (meta.dev(), meta.ino())
        },
        #[cfg(unix)]
        permissions: {
            use std::os::unix::fs::PermissionsExt;
            meta.permissions().mode() & 0o777
        },
    }))
}

pub struct DownloadTarget {
    directory: Mutex<Option<tempfile::TempDir>>,
    staged: PathBuf,
    target: PathBuf,
    expected: Option<Stamp>,
}
impl DownloadTarget {
    pub fn new(target: &Path, replace: bool) -> Result<Self> {
        Self::new_with_prefix(target, replace, ".nauterm-download-")
    }
    pub(crate) fn new_with_prefix(target: &Path, replace: bool, prefix: &str) -> Result<Self> {
        ensure!(
            !prefix.is_empty()
                && prefix
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)),
            "Invalid staging prefix."
        );
        ensure!(
            target.is_absolute(),
            "Choose an absolute download destination."
        );
        let name = target
            .file_name()
            .context("Cannot download over a filesystem root.")?;
        let parent = target
            .parent()
            .context("A destination folder is required.")?
            .canonicalize()?;
        let target = parent.join(name);
        let expected = stamp(&target)?;
        ensure!(
            replace || expected.is_none(),
            "The local destination already exists."
        );
        let directory = private_directory_in(parent, prefix)?;
        let staged = directory.path().join("content");
        Ok(Self {
            directory: Mutex::new(Some(directory)),
            staged,
            target,
            expected,
        })
    }
    pub fn staged(&self) -> &Path {
        &self.staged
    }
    pub fn publish(&self) -> Result<()> {
        self.publish_with(rename_new)
    }
    fn publish_with(&self, rename: impl Fn(&Path, &Path) -> io::Result<()>) -> Result<()> {
        let mut owned = self.directory.lock().unwrap();
        let directory = owned
            .as_ref()
            .context("Download staging was already released.")?;
        ensure!(
            stamp(&self.staged)?.is_some(),
            "The download produced no file or folder."
        );
        if self.expected.is_none() {
            return rename(&self.staged, &self.target).context(
                "The download finished, but its destination now exists or could not be published.",
            );
        }
        #[cfg(unix)]
        if let Some(expected) = &self.expected
            && stamp(&self.staged)?.is_some_and(|staged| staged.kind == expected.kind)
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                &self.staged,
                fs::Permissions::from_mode(expected.permissions),
            )
            .context("Could not preserve the destination permissions; it was not replaced.")?;
        }
        // The user's old item remains recoverable until the new one is in place.
        let backup = directory.path().join("previous-destination");
        rename(&self.target, &backup).context(
            "The destination changed or could not be backed up; it was not overwritten.",
        )?;
        let result = (|| {
            ensure!(
                stamp(&backup)? == self.expected,
                "The destination changed during the download. It was not replaced."
            );
            rename(&self.staged, &self.target).context("Could not publish the completed download.")
        })();
        if let Err(error) = result {
            if let Err(restore) = rename(&backup, &self.target) {
                let recovery = owned.take().unwrap().keep();
                bail!(
                    "{error} The previous destination is preserved in {} (restore failed: {restore}).",
                    recovery.display()
                );
            }
            return Err(error);
        }
        Ok(())
    }
}

/// Atomic no-replace rename for files and directories on supported platforms.
fn rename_new(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let from = CString::new(from.as_os_str().as_bytes())?;
        let to = CString::new(to.as_os_str().as_bytes())?;
        #[cfg(target_os = "macos")]
        let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        #[cfg(target_os = "linux")]
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Atomic no-replace publication is unavailable on this platform.",
        ));
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        // MoveFileW explicitly rejects an existing destination, for directories
        // as well as files: learn.microsoft.com/windows/win32/api/winbase/nf-winbase-movefilew
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileW(from: *const u16, to: *const u16) -> i32;
        }
        let encode = |path: &Path| -> io::Result<Vec<u16>> {
            let mut wide: Vec<_> = path.as_os_str().encode_wide().collect();
            if wide.contains(&0) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Path contains NUL",
                ));
            }
            wide.push(0);
            Ok(wide)
        };
        let from = encode(from)?;
        let to = encode(to)?;
        if unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) } != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(not(any(unix, windows)))]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Atomic publication is unavailable.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_publish_and_failed_rollback_keep_the_old_destination_recoverable() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("existing");
        fs::write(&path, b"old data").unwrap();
        let download = DownloadTarget::new(&path, true).unwrap();
        fs::write(download.staged(), b"new data").unwrap();
        let recovery = download.staged().parent().unwrap().to_owned();
        let calls = std::cell::Cell::new(0);
        let error = download
            .publish_with(|from, to| {
                let call = calls.replace(calls.get() + 1);
                if call == 0 {
                    return rename_new(from, to);
                }
                if call == 1 {
                    fs::write(to, b"a concurrent writer")?;
                }
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "simulated publication race",
                ))
            })
            .unwrap_err();
        assert!(error.to_string().contains("preserved"));
        drop(download);
        assert_eq!(
            fs::read(recovery.join("previous-destination")).unwrap(),
            b"old data"
        );
        assert_eq!(fs::read(path).unwrap(), b"a concurrent writer");
    }
    #[test]
    fn download_publication_preserves_existing_and_race_created_targets() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("结果.txt");
        let download = DownloadTarget::new(&path, false).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(download.staged().parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        fs::write(download.staged(), b"new").unwrap();
        fs::write(&path, b"arrived later").unwrap();
        assert!(download.publish().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"arrived later");
        assert!(DownloadTarget::new(&path, false).is_err());
        let replace = DownloadTarget::new(&path, true).unwrap();
        fs::write(replace.staged(), b"complete").unwrap();
        replace.publish().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"complete");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let replace = DownloadTarget::new(&path, true).unwrap();
            fs::write(replace.staged(), b"private replacement").unwrap();
            replace.publish().unwrap();
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn changed_destination_is_restored_and_directory_download_is_atomic() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("item");
        fs::write(&path, b"old").unwrap();
        let download = DownloadTarget::new(&path, true).unwrap();
        fs::write(download.staged(), b"new").unwrap();
        fs::write(&path, b"modified by someone else").unwrap();
        assert!(download.publish().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"modified by someone else");
        let directory = temp.path().join("folder");
        let download = DownloadTarget::new(&directory, false).unwrap();
        fs::create_dir(download.staged()).unwrap();
        fs::write(download.staged().join("child"), b"payload").unwrap();
        download.publish().unwrap();
        assert_eq!(fs::read(directory.join("child")).unwrap(), b"payload");
    }
}
