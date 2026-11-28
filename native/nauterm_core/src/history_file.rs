//! Coordination for the display history file shared by Flutter and RS.
//! Dart RandomAccessFile.lock uses POSIX record locks on macOS/Linux, whereas
//! std::fs::File::lock uses flock there. Match Dart's record lock explicitly.
//! https://github.com/dart-lang/sdk/blob/main/runtime/bin/file_macos.cc
//! https://github.com/dart-lang/sdk/blob/main/runtime/bin/file_linux.cc
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::Mutex,
};

static PROCESS_WRITER: Mutex<()> = Mutex::new(());

/// Hold the stable sidecar lock across read/merge/atomic replacement. POSIX
/// locks are process scoped, so serialize in-process callers as well. The lock
/// file is never renamed or removed; closing it releases the OS lock on exit.
pub fn with_lock<T, E: From<io::Error>>(
    path: &Path,
    work: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let _process = PROCESS_WRITER
        .lock()
        .map_err(|_| io::Error::other("Shell history writer is unavailable"))?;
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    let lock_path = PathBuf::from(name);
    if let Some(parent) = lock_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(lock_path)?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        loop {
            // SAFETY: the descriptor is owned by `file`, remains open through
            // `work`, and lockf takes no pointers. Offset zero/length zero
            // covers the same whole-file range as Dart's default lock call.
            if unsafe { libc::lockf(file.as_raw_fd(), libc::F_LOCK, 0) } == 0 {
                break;
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error.into());
            }
        }
    }
    #[cfg(not(unix))]
    file.lock()?;
    work()
}
