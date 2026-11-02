//! Remote editing owns private local copies, immutable save snapshots and
//! captured SSH options. No credentials or file contents enter public UI state.
use crate::{
    file_commit::DownloadTarget,
    file_operation::Operation,
    native::{Core, SshOptions},
    sftp_client::{Cancellation, Listing, Request},
    transfer_queue::{Client, GuardRejected, Payload, Spec},
};
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

mod worker;
#[cfg(target_os = "macos")]
pub use worker::close_window;
pub use worker::{Event, Handle, Resolution, prepare_shutdown};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub size: u64,
    pub modified: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fingerprint {
    size: u64,
    modified: Option<SystemTime>,
    digest: [u8; 32],
}

pub struct Connection {
    core: Arc<Core>,
    options: Arc<SshOptions>,
    sudo: Mutex<Option<Arc<crate::sftp_client::Sudo>>>,
    cancelled: AtomicBool,
    active: Mutex<Option<Cancellation>>,
    transfers: Client,
}
impl Connection {
    pub fn new_queued(core: Arc<Core>, options: Arc<SshOptions>, transfers: Client) -> Arc<Self> {
        Self::new_queued_authorized(core, options, transfers, None)
    }
    pub fn new_queued_authorized(
        core: Arc<Core>,
        options: Arc<SshOptions>,
        transfers: Client,
        sudo: Option<Arc<crate::sftp_client::Sudo>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            core,
            options: Arc::new(options.for_new_connection()),
            sudo: Mutex::new(sudo),
            cancelled: AtomicBool::new(false),
            active: Mutex::default(),
            transfers,
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.transfers.cancel();
        if let Some(active) = self.active.lock().unwrap().as_ref() {
            active.cancel();
        }
    }
    fn request<T>(&self, action: impl FnOnce(Request) -> Result<T>) -> Result<T> {
        {
            let mut active = self.active.lock().unwrap();
            ensure!(
                !self.cancelled.load(Ordering::SeqCst),
                "Remote editing was cancelled."
            );
            ensure!(active.is_none(), "An edit request is already running.");
            let (request, cancel) = Request::new(self.core.clone())?;
            *active = Some(cancel);
            drop(active);
            let result = action(request);
            self.active.lock().unwrap().take();
            result
        }
    }
    fn listing(&self, path: &str) -> Result<Listing> {
        let sudo = self.sudo.lock().unwrap().clone();
        let listing = self.request(|request| {
            request.list_authorized(&self.options, path, sudo.as_deref(), None)
        })?;
        ensure!(
            listing.error.is_none(),
            "{}",
            listing.error.as_deref().unwrap_or_default()
        );
        Ok(listing)
    }
    fn version(&self, path: &str) -> Result<Option<Version>> {
        let (directory, name) = parent(path)?;
        let listing = self.listing(directory)?;
        if let Some(entry) = listing.entries.iter().find(|entry| entry.name == name) {
            ensure!(
                !entry.is_directory && entry.mode.is_none_or(|mode| mode & 0o170000 == 0o100000),
                "The remote path is no longer a regular file. Local edits were retained; it was not replaced."
            );
        }
        Ok(listing
            .entries
            .iter()
            .find(|entry| entry.name == name && !entry.is_directory)
            .map(|entry| Version {
                size: entry.size,
                modified: entry.modified,
            }))
    }

    /// Hooks execute inside the same queue reservation as the data transfer,
    /// including a fresh precondition check on every resume/retry.
    fn transfer_checked(
        self: &Arc<Self>,
        operation: Operation,
        expected: Option<Version>,
        payload: Payload,
        display_local: String,
        label: String,
    ) -> Result<Version> {
        let download = matches!(operation, Operation::Download { .. });
        let remote_path = if download {
            operation.source()
        } else {
            operation.target()
        }
        .to_owned();
        let mut spec = Spec::new(0, self.options.clone(), operation);
        if !download {
            spec.kind = Some(crate::transfer_queue::Kind::Edit);
        }
        spec.core = Some(self.core.clone());
        spec.sudo = self.sudo.lock().unwrap().clone();
        spec.label = Some(if spec.sudo.is_some() {
            format!("{label} with sudo")
        } else {
            label
        });
        spec.payload = Some(payload);
        spec.refresh_local = false;
        spec.inherit_transfer_threads = true;
        spec.refresh_owner = !download;
        if download {
            spec.display_target = Some(display_local);
        } else {
            spec.display_source = Some(display_local);
        }
        let connection = Arc::downgrade(self);
        let before_path = remote_path.clone();
        let before_version = expected.clone();
        spec.before = Some(Arc::new(move || {
            let connection = connection
                .upgrade()
                .context("The edit session has closed.")?;
            if connection.version(&before_path)? != before_version {
                return Err(GuardRejected(
                    "The remote file changed while this transfer was queued or paused.".into(),
                )
                .into());
            }
            Ok(())
        }));
        let verified = Arc::new(Mutex::new(None));
        let result_version = verified.clone();
        let connection = Arc::downgrade(self);
        spec.after = Some(Arc::new(move || {
            let connection = connection
                .upgrade()
                .context("The edit session has closed.")?;
            let version = connection.version(&remote_path)?;
            ensure!(
                !download || version == expected,
                "The remote file changed during download."
            );
            *result_version.lock().unwrap() =
                Some(version.context("The remote file could not be verified.")?);
            Ok(())
        }));
        let result = self.transfers.execute(spec)?;
        ensure!(
            result.ok,
            "{}",
            result.error.as_deref().unwrap_or("SFTP transfer failed.")
        );
        let version = verified
            .lock()
            .unwrap()
            .take()
            .context("Missing transfer verification.")?;
        Ok(version)
    }
    fn key(&self, path: &str) -> String {
        format!(
            "{}@{}:{}:{path}",
            self.options.username, self.options.host, self.options.port
        )
    }
    fn authorization_target(&self, path: &str) -> Result<AuthorizationTarget> {
        ensure!(
            self.sudo.lock().unwrap().is_some(),
            "This edit was not opened with sudo."
        );
        ensure!(
            !self.cancelled.load(Ordering::SeqCst),
            "The edit session closed."
        );
        Ok(AuthorizationTarget {
            options: self.options.clone(),
            directory: parent(path)?.0.into(),
            sudo: crate::sftp_client::Sudo::new(self.core.clone())?,
        })
    }
    fn renew_sudo(&self, sudo: Arc<crate::sftp_client::Sudo>) -> Result<()> {
        ensure!(
            !self.cancelled.load(Ordering::SeqCst),
            "The edit session closed."
        );
        ensure!(sudo.current(), "Sudo authorization already expired.");
        let mut current = self.sudo.lock().unwrap();
        ensure!(
            current.is_some(),
            "An ordinary edit cannot silently switch to sudo."
        );
        *current = Some(sudo);
        Ok(())
    }
}

/// Private transient connection data, not a serializable UI/task snapshot.
pub use crate::sftp_client::AuthorizationTarget;

fn parent(path: &str) -> Result<(&str, &str)> {
    ensure!(
        path.starts_with('/')
            && !path.contains(['\0', '\r', '\n'])
            && !path.split('/').any(|part| matches!(part, "." | "..")),
        "Invalid remote edit path."
    );
    let (directory, name) = path.rsplit_once('/').context("Choose a remote file.")?;
    ensure!(
        !name.is_empty()
            && !name.contains('\\')
            && Path::new(name).components().count() == 1
            && Path::new(name).file_name().is_some(),
        "This filename cannot be safely represented on the local platform."
    );
    Ok((if directory.is_empty() { "/" } else { directory }, name))
}
fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .context("The edit path cannot be encoded by SFTP.")
}
fn lock_for(key: String) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Weak<Mutex<()>>>>> = OnceLock::new();
    let mut locks = LOCKS.get_or_init(Mutex::default).lock().unwrap();
    locks.retain(|_, value| value.strong_count() > 0);
    if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(key, Arc::downgrade(&lock));
    lock
}

struct EditDirectory {
    temporary: Mutex<Option<tempfile::TempDir>>,
}
pub struct Document {
    directory: Option<Arc<EditDirectory>>,
    directory_identity: fs::Metadata,
    root: PathBuf,
    pub local_path: PathBuf,
    pub remote_path: String,
    pub connection: Arc<Connection>,
    version: Version,
    uploaded: Fingerprint,
    opened: bool,
}
impl Document {
    pub fn download(
        connection: Arc<Connection>,
        remote_path: String,
        explicit: bool,
    ) -> Result<Self> {
        let (parent, name) = parent(&remote_path)?;
        let listing = connection.listing(parent)?;
        let entry = listing
            .entries
            .iter()
            .find(|entry| entry.name == name && !entry.is_directory)
            .context("The remote file no longer exists.")?;
        ensure!(
            entry.mode.is_none_or(|mode| mode & 0o170000 == 0o100000),
            "Only regular remote files can be edited."
        );
        ensure!(
            explicit
                || entry.mode.is_some_and(|mode| mode & 0o170000 == 0o100000
                    && mode & 0o111 == 0
                    && crate::file_model::can_open_with_system(name, false)),
            "Choose an explicit application for this remote file."
        );
        let version = Version {
            size: entry.size,
            modified: entry.modified,
        };
        let mut builder = tempfile::Builder::new();
        builder.prefix("nauterm-rs-edit-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(fs::Permissions::from_mode(0o700));
        }
        let directory = builder.tempdir()?;
        let directory_identity = fs::symlink_metadata(directory.path())?;
        let root = directory.path().canonicalize()?;
        let directory = Arc::new(EditDirectory {
            temporary: Mutex::new(Some(directory)),
        });
        let local_path = root.join(name);
        let download = DownloadTarget::new(&local_path, false)?;
        connection.transfer_checked(
            Operation::Download {
                remote_path: remote_path.clone(),
                local_path: path_text(download.staged())?,
                transfer_threads: 8,
            },
            Some(version.clone()),
            directory.clone(),
            path_text(&local_path)?,
            format!("Open {name}"),
        )?;
        download.publish()?;
        let uploaded = fingerprint(&local_path, &root, None)?;
        Ok(Self {
            directory: Some(directory),
            directory_identity,
            root,
            local_path,
            remote_path,
            connection,
            version,
            uploaded,
            opened: false,
        })
    }
    pub fn mark_opened(&mut self) {
        self.opened = true;
    }
    fn directory(&self) -> &Path {
        &self.root
    }
    fn fingerprint(&self) -> Result<Fingerprint> {
        let metadata = fs::symlink_metadata(self.directory())?;
        ensure!(
            metadata.is_dir() && same_identity(&metadata, &self.directory_identity),
            "The private edit directory was replaced."
        );
        fingerprint(&self.local_path, self.directory(), None)
    }
    fn snapshot(&self) -> Result<Arc<Snapshot>> {
        self.fingerprint()?;
        let mut file = tempfile::Builder::new()
            .prefix(".nauterm-rs-upload-")
            .tempfile_in(self.directory())?;
        let fingerprint =
            fingerprint(&self.local_path, self.directory(), Some(file.as_file_mut()))?;
        ensure!(
            self.fingerprint()? == fingerprint,
            "The editor is still writing the file."
        );
        Ok(Arc::new(Snapshot {
            file,
            fingerprint,
            _directory: self.directory.as_ref().unwrap().clone(),
        }))
    }
    fn changed(&self) -> bool {
        !self.fingerprint().is_ok_and(|value| value == self.uploaded)
    }
    fn retain(&mut self) -> Option<PathBuf> {
        self.directory.take().and_then(|directory| {
            directory
                .temporary
                .lock()
                .unwrap()
                .take()
                .map(|directory| directory.keep())
        })
    }
    fn finish(&mut self) -> Option<PathBuf> {
        let dirty = self.opened && self.changed();
        self.opened = false;
        if dirty { self.retain() } else { None }
    }
    /// Caller owns the per-target lock until resolution/transfer completes.
    fn save(
        &mut self,
        snapshot: &Arc<Snapshot>,
        resolution: Option<(Resolution, Option<Version>)>,
    ) -> Result<Save> {
        let current = self.connection.version(&self.remote_path)?;
        if resolution
            .as_ref()
            .is_some_and(|(_, approved)| approved != &current)
        {
            return Ok(Save::Conflict(current));
        }
        if current.as_ref() != Some(&self.version) && resolution.is_none() {
            return Ok(Save::Conflict(current));
        }
        let resolution = resolution.map(|(resolution, _)| resolution);
        match resolution {
            Some(Resolution::Cancel) => return Ok(Save::Paused),
            Some(Resolution::KeepRemote) => {
                let version = current.context("The remote file no longer exists.")?;
                let target = DownloadTarget::new(&self.local_path, true)?;
                let result = self.connection.transfer_checked(
                    Operation::Download {
                        remote_path: self.remote_path.clone(),
                        local_path: path_text(target.staged())?,
                        transfer_threads: 8,
                    },
                    Some(version.clone()),
                    snapshot.clone(),
                    path_text(&self.local_path)?,
                    format!("Keep remote {}", self.remote_path),
                );
                if result
                    .as_ref()
                    .err()
                    .is_some_and(|error| error.is::<GuardRejected>())
                {
                    return Ok(Save::Conflict(self.connection.version(&self.remote_path)?));
                }
                result?;
                ensure!(
                    self.fingerprint()? == snapshot.fingerprint,
                    "The editor changed the local file during Keep Remote. Both versions were preserved."
                );
                target.publish()?;
                self.uploaded = self.fingerprint()?;
                self.version = version;
                return Ok(Save::Saved);
            }
            _ => {}
        }
        let copy = resolution == Some(Resolution::SaveCopy);
        let destination = if copy {
            let (directory, name) = parent(&self.remote_path)?;
            let listing = self.connection.listing(directory)?;
            let (stem, extension) = name
                .rsplit_once('.')
                .filter(|(stem, _)| !stem.is_empty())
                .map(|(stem, ext)| (stem, format!(".{ext}")))
                .unwrap_or((name, String::new()));
            (1..)
                .map(|index| format!("{stem} ({index}){extension}"))
                .find(|candidate| !listing.entries.iter().any(|entry| entry.name == *candidate))
                .map(|name| format!("{}/{name}", directory.trim_end_matches('/')))
                .unwrap()
        } else {
            self.remote_path.clone()
        };
        let result = self.connection.transfer_checked(
            Operation::Upload {
                local_path: path_text(snapshot.file.path())?,
                remote_path: destination.clone(),
                replace_existing: !copy,
                transfer_threads: 8,
            },
            if copy { None } else { current },
            snapshot.clone(),
            path_text(&self.local_path)?,
            format!("Save {destination}"),
        );
        if result
            .as_ref()
            .err()
            .is_some_and(|error| error.is::<GuardRejected>())
        {
            return Ok(Save::Conflict(self.connection.version(&self.remote_path)?));
        }
        // The write may already have succeeded; don't automatically retry a
        // verification failure and never pretend its previous version is current.
        let version = result?;
        self.remote_path = destination;
        self.version = version;
        self.uploaded = snapshot.fingerprint.clone();
        Ok(Save::Saved)
    }
}
impl Drop for Document {
    fn drop(&mut self) {
        // A cancelled frontend future may drop a copy after asking the OS to
        // open it. Only a managed, explicit finish may delete an opened copy.
        if self.opened
            && let Some(path) = self.retain()
        {
            eprintln!("Unsaved remote edits retained at {}", path.display());
        }
    }
}
struct Snapshot {
    file: tempfile::NamedTempFile,
    fingerprint: Fingerprint,
    _directory: Arc<EditDirectory>,
}
enum Save {
    Saved,
    Paused,
    Conflict(Option<Version>),
}

fn fingerprint(path: &Path, directory: &Path, mut copy: Option<&mut File>) -> Result<Fingerprint> {
    ensure!(
        fs::symlink_metadata(directory)?.is_dir()
            && !fs::symlink_metadata(directory)?.file_type().is_symlink(),
        "The private edit directory was replaced."
    );
    ensure!(
        path.parent() == Some(directory),
        "The edit file escaped its private directory."
    );
    let before = fs::symlink_metadata(path)?;
    ensure!(
        before.is_file() && !before.file_type().is_symlink(),
        "The edit file is not a regular private file."
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(before.nlink() == 1, "Refusing a hard-linked edit file.");
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let input = options.open(path)?;
    let metadata = input.metadata()?;
    ensure!(metadata.is_file(), "The edit file is not regular.");
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        ensure!(
            (metadata.dev(), metadata.ino(), metadata.nlink()) == (before.dev(), before.ino(), 1),
            "The edit file changed while reading."
        );
        if metadata.permissions().mode() & 0o777 != 0o600 {
            input.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
    }
    let mut input = input.take(metadata.len().saturating_add(1));
    let mut hash = Sha256::new();
    let mut buffer = zeroize::Zeroizing::new(vec![0u8; 64 * 1024]);
    let mut size = 0u64;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        if let Some(copy) = copy.as_mut() {
            copy.write_all(&buffer[..count])?;
        }
        size += count as u64;
    }
    let after = input.get_ref().metadata()?;
    ensure!(
        size == metadata.len()
            && after.len() == metadata.len()
            && after.modified().ok() == metadata.modified().ok(),
        "The editor is still writing the file."
    );
    Ok(Fingerprint {
        size,
        modified: after.modified().ok(),
        digest: hash.finalize().into(),
    })
}

fn same_identity(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (a.dev(), a.ino()) == (b.dev(), b.ino())
    }
    #[cfg(not(unix))]
    {
        a.created().ok() == b.created().ok()
    }
}
