//! Cross-client liveness/maintenance lease for a recording, including sessions
//! with metadata only. Hold it through final metadata publication. Recovery,
//! deletion, compaction and orphan cleanup acquire the same lease first.
use std::{
    fs::{File, OpenOptions, TryLockError},
    io,
    path::Path,
};

pub struct RecordingLease {
    _file: File,
}

pub fn valid_recording_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

impl RecordingLease {
    /// None means a live writer or another maintenance operation owns the log.
    /// Locks are descriptor scoped (including within one process) and released
    /// automatically on process exit. Never unlink the stable sidecar file.
    pub fn try_acquire(directory: &Path, id: &str) -> io::Result<Option<Self>> {
        Self::acquire(directory, id, true)
    }

    /// Legacy sessions without a lease may still be owned by an older client.
    /// Their missing lease is not evidence that it is safe to recover them.
    pub fn try_acquire_existing(directory: &Path, id: &str) -> io::Result<Option<Self>> {
        Self::acquire(directory, id, false)
    }

    fn acquire(directory: &Path, id: &str, create: bool) -> io::Result<Option<Self>> {
        if !valid_recording_id(id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid recording ID",
            ));
        }
        if std::fs::symlink_metadata(directory)
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Recording directory must not be a symlink",
            ));
        }
        if create {
            crate::capture_crypto::prepare_capture_directory(directory)?;
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(create)
            .truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = match options.open(directory.join(format!(".nauterm-recording.{id}.lock"))) {
            Ok(file) => file,
            Err(error) if !create && error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
}

/// Only recognize files owned by the encrypted recording format. Unknown
/// files and lease sidecars must survive automatic directory cleanup.
pub fn recording_id_for_file(name: &str) -> Option<&str> {
    let id = name.get(..36).filter(|id| valid_recording_id(id))?;
    let mut suffix = name.get(36..)?.strip_prefix('.')?;
    if !suffix.starts_with("ntrcap") {
        let (revision, rest) = suffix.split_once('.')?;
        if revision.len() != 32 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        suffix = rest;
    }
    let tail = suffix.strip_prefix("ntrcap")?;
    if tail.is_empty() || tail == ".state" {
        return Some(id);
    }
    if let Some(token) = tail
        .strip_prefix(".state.")
        .and_then(|tail| tail.strip_suffix(".tmp"))
    {
        return (token.len() == 16 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then_some(id);
    }
    if let Some(tail) = tail.strip_prefix(".compact.") {
        let (token, extension) = tail.rsplit_once('.')?;
        return (!token.is_empty()
            && token.bytes().all(|byte| byte.is_ascii_digit())
            && matches!(extension, "tmp" | "bak"))
        .then_some(id);
    }
    None
}
