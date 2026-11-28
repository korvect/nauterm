//! Explicit, cancellable download of the reviewed RS asset. Only a sealed
//! verified artifact can leave staging; failed/cancelled partials are removed.
use super::releases::{Asset, Release};
use crate::cancel::Cancel;
use anyhow::{Context, Result, ensure};
use futures_util::future::{Either, select};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tempfile::TempDir;
use url::Url;
const MAX_PACKAGE: u64 = 4 * 1024 * 1024 * 1024;
const MAX_CHECKSUMS: u64 = 512 * 1024;

#[derive(Clone)]
pub struct Plan {
    pub(super) package: Asset,
    checksums: Asset,
    pub version: String,
}
impl Plan {
    pub fn new(release: &Release) -> Result<Self> {
        ensure!(
            release.newer,
            "This release is not newer than the installed version."
        );
        let package = release
            .asset
            .clone()
            .context("This release has no compatible Nauterm RS package.")?;
        let checksums = release
            .checksums
            .clone()
            .context("This release has no checksum file for an in-app download.")?;
        ensure!(
            super::releases::Target::current()
                .asset_rank(&release.version, &package.name)
                .is_some()
                && release.tag.strip_prefix('v').unwrap_or(&release.tag) == release.version,
            "The reviewed Nauterm RS package does not match this version or platform."
        );
        ensure!(
            package.url == release.repository.download_url(&release.tag, &package.name)
                && checksums.name == "SHA256SUMS.txt"
                && checksums.url
                    == release
                        .repository
                        .download_url(&release.tag, &checksums.name),
            "The reviewed release package or checksum destination changed."
        );
        ensure!(
            package.size > 0
                && package.size <= MAX_PACKAGE
                && checksums.size > 0
                && checksums.size <= MAX_CHECKSUMS,
            "The release package or checksum file exceeds the download limit."
        );
        ensure!(
            !package.name.contains(['/', '\\', '\0'])
                && package.name != "."
                && package.name != "..",
            "Invalid update package name."
        );
        Ok(Self {
            package,
            checksums,
            version: release.version.clone(),
        })
    }
}
pub enum Event {
    Checksums,
    Progress { received: u64, total: u64 },
    Verifying,
    Ready(Verified),
    Failed(String),
}
pub struct Flow {
    pub events: async_channel::Receiver<Event>,
    pub cancel: Cancel,
}
struct Worker {
    cancel: Cancel,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}
#[derive(Default)]
struct Workers {
    closed: bool,
    active: Vec<Arc<Worker>>,
}
fn workers() -> &'static Mutex<Workers> {
    static WORKERS: OnceLock<Mutex<Workers>> = OnceLock::new();
    WORKERS.get_or_init(Mutex::default)
}
pub fn prepare_shutdown() {
    let active = {
        let mut workers = workers().lock().unwrap();
        workers.closed = true;
        std::mem::take(&mut workers.active)
    };
    for worker in &active {
        worker.cancel.cancel();
    }
    for worker in active {
        if let Some(thread) = worker.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
}
pub fn digest(value: &str) -> Result<[u8; 32]> {
    ensure!(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid SHA-256 checksum."
    );
    let mut output = [0; 32];
    for (index, slot) in output.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)?;
    }
    Ok(output)
}
fn checksum(bytes: &[u8], name: &str) -> Result<[u8; 32]> {
    ensure!(
        bytes.len() as u64 <= MAX_CHECKSUMS,
        "The checksum file is too large."
    );
    let text = std::str::from_utf8(bytes).context("The checksum file is not UTF-8.")?;
    let mut expected = None;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        ensure!(
            line.len() >= 67
                && line.as_bytes()[64] == b' '
                && matches!(line.as_bytes()[65], b' ' | b'*'),
            "Invalid SHA256SUMS.txt format."
        );
        let hash = digest(line.get(..64).context("Invalid checksum entry.")?)?;
        let filename = line.get(66..).context("Invalid checksum filename.")?;
        ensure!(
            !filename.is_empty()
                && !filename.contains(['/', '\\', '\0'])
                && filename != "."
                && filename != "..",
            "Invalid checksum filename."
        );
        if filename == name {
            ensure!(
                expected.replace(hash).is_none(),
                "The checksum file has duplicate entries for this package."
            );
        }
    }
    expected.context("The checksum file has no entry for this Nauterm RS package.")
}
fn redirect_allowed(url: &Url, hops: usize) -> bool {
    hops < 5
        && url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
}
fn network_error(error: reqwest::Error) -> anyhow::Error {
    anyhow::anyhow!(if error.is_timeout() {
        "Update download timed out. Try again."
    } else {
        "Update download failed. Check your connection and try again."
    })
}
fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(format!("Nauterm-RS/{}", super::VERSION))
        .connect_timeout(Duration::from_secs(8))
        .read_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(30 * 60))
        // GitHub may stream a release asset or redirect it to object storage.
        // The request has no credentials, and the final bytes must match the
        // reviewed digest plus the same release's checksum manifest.
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if redirect_allowed(attempt.url(), attempt.previous().len()) {
                attempt.follow()
            } else {
                attempt.error("Unsupported update download redirect")
            }
        }))
        .build()?)
}
pub fn start(plan: Plan, root: PathBuf) -> Result<Flow> {
    let mut workers = workers().lock().unwrap();
    ensure!(!workers.closed, "The application is shutting down.");
    workers.active.retain(|worker| {
        let mut thread = worker.thread.lock().unwrap();
        if thread.as_ref().is_some_and(|thread| thread.is_finished()) {
            if let Some(thread) = thread.take() {
                let _ = thread.join();
            }
            false
        } else {
            true
        }
    });
    let (sender, events) = async_channel::bounded(4);
    let cancel = Cancel::new();
    let stop = cancel.clone();
    let thread = std::thread::Builder::new()
        .name("nauterm-rs-update-download".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => {
                    let _ = sender
                        .try_send(Event::Failed("Could not start the update download.".into()));
                    return;
                }
            };
            runtime.block_on(async {
                let outcome = match select(
                    Box::pin(download(plan, &root, &sender)),
                    Box::pin(stop.wait()),
                )
                .await
                {
                    Either::Left((result, _)) => Some(result),
                    Either::Right(_) => None,
                };
                if let Some(result) = outcome {
                    let event = match result {
                        Ok(verified) => Event::Ready(verified),
                        Err(error) => Event::Failed(error.to_string()),
                    };
                    let _ = select(Box::pin(sender.send(event)), Box::pin(stop.wait())).await;
                }
            });
            runtime.shutdown_timeout(Duration::from_millis(250));
        })
        .context("Could not start the update download.")?;
    workers.active.push(Arc::new(Worker {
        cancel: cancel.clone(),
        thread: Mutex::new(Some(thread)),
    }));
    Ok(Flow { events, cancel })
}
async fn response(client: &reqwest::Client, asset: &Asset) -> Result<reqwest::Response> {
    let response = client
        .get(&asset.url)
        .header("Accept-Encoding", "identity")
        .send()
        .await
        .map_err(network_error)?;
    ensure!(
        response.status() == reqwest::StatusCode::OK,
        "Update download failed (HTTP {}).",
        response.status().as_u16()
    );
    if let Some(length) = response.content_length() {
        ensure!(
            length == asset.size,
            "The release asset changed. Check for updates again."
        );
    }
    Ok(response)
}
async fn download(
    plan: Plan,
    root: &Path,
    sender: &async_channel::Sender<Event>,
) -> Result<Verified> {
    let client = client()?;
    let _ = sender.try_send(Event::Checksums);
    let mut manifest = response(&client, &plan.checksums).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = manifest.chunk().await.map_err(network_error)? {
        ensure!(
            (bytes.len() as u64).saturating_add(chunk.len() as u64) <= plan.checksums.size,
            "The checksum file changed or exceeded its size limit."
        );
        bytes.extend_from_slice(&chunk);
    }
    ensure!(
        bytes.len() as u64 == plan.checksums.size,
        "The checksum download is incomplete."
    );
    let manifest_hash: [u8; 32] = Sha256::digest(&bytes).into();
    if let Some(expected) = plan.checksums.digest {
        ensure!(
            expected == manifest_hash,
            "The checksum file changed. Check for updates again."
        );
    }
    let expected = checksum(&bytes, &plan.package.name)?;
    if let Some(reviewed) = plan.package.digest {
        ensure!(
            reviewed == expected,
            "The package digest and checksum manifest disagree. Check for updates again."
        );
    }
    let mut staged = Staged::new(root, &plan.package.name, plan.package.size)?;
    let mut package = response(&client, &plan.package).await?;
    let mut hash = Sha256::new();
    let mut received = 0u64;
    while let Some(chunk) = package.chunk().await.map_err(network_error)? {
        received = received
            .checked_add(chunk.len() as u64)
            .context("The update package is too large.")?;
        ensure!(
            received <= plan.package.size,
            "The package changed or exceeded its size limit. Check for updates again."
        );
        staged
            .file
            .write_all(&chunk)
            .context("Could not write the update package.")?;
        hash.update(&chunk);
        let _ = sender.try_send(Event::Progress {
            received,
            total: plan.package.size,
        });
    }
    ensure!(
        received == plan.package.size,
        "The update package download is incomplete."
    );
    let _ = sender.try_send(Event::Verifying);
    let actual: [u8; 32] = hash.finalize().into();
    ensure!(
        actual == expected,
        "The downloaded update failed SHA-256 verification."
    );
    staged
        .file
        .sync_all()
        .context("Could not finish saving the update package.")?;
    staged.seal(expected, plan.version)
}
struct Staged {
    file: File,
    directory: TempDir,
    path: PathBuf,
    size: u64,
}
impl Staged {
    fn new(root: &Path, name: &str, size: u64) -> Result<Self> {
        fs::create_dir_all(root).context("Could not create the update cache.")?;
        let metadata = fs::symlink_metadata(root)?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "The update cache must be an ordinary directory."
        );
        let mut builder = tempfile::Builder::new();
        builder.prefix("nauterm-rs-update-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(fs::Permissions::from_mode(0o700));
        }
        let directory = builder
            .tempdir_in(root.canonicalize()?)
            .context("Could not prepare private update storage.")?;
        let path = directory.path().join(name);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = options
            .open(&path)
            .context("Could not create the update package.")?;
        Ok(Self {
            file,
            directory,
            path,
            size,
        })
    }
    fn seal(self, digest: [u8; 32], version: String) -> Result<Verified> {
        let original = self.file.metadata()?;
        ensure!(
            original.is_file() && original.len() == self.size,
            "The update package is incomplete."
        );
        // A write-capable handle can prevent Windows from loading an EXE.
        // Keep an owned read-only handle after finishing the package.
        let Self {
            file,
            directory,
            path,
            size,
        } = self;
        drop(file);
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options
            .open(&path)
            .context("Could not seal the verified update package.")?;
        let current = file.metadata()?;
        ensure!(
            current.is_file() && current.len() == size,
            "The update package changed before verification completed."
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            ensure!(
                original.dev() == current.dev() && original.ino() == current.ino(),
                "The update package was replaced before verification completed."
            );
        }
        Ok(Verified {
            file: Mutex::new(file),
            directory: Mutex::new(Some(directory)),
            path,
            size,
            digest,
            version,
        })
    }
}
pub struct Verified {
    file: Mutex<File>,
    directory: Mutex<Option<TempDir>>,
    path: PathBuf,
    size: u64,
    digest: [u8; 32],
    pub version: String,
}
impl Verified {
    pub fn retained(&self) -> bool {
        self.directory.lock().unwrap().is_none()
    }
    pub fn filename(&self) -> String {
        self.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
    pub fn revalidate(&self) -> Result<()> {
        let metadata =
            fs::symlink_metadata(&self.path).context("The verified update package is missing.")?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() == self.size,
            "The verified update package changed. Download it again."
        );
        let mut source = self.file.lock().unwrap();
        let mut current =
            File::open(&self.path).context("The verified update package is unavailable.")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let original = source.metadata()?;
            let current = current.metadata()?;
            ensure!(
                original.dev() == current.dev() && original.ino() == current.ino(),
                "The verified update package was replaced. Download it again."
            );
        }
        let original = hash_file(&mut source, self.size)?;
        let current = hash_file(&mut current, self.size)?;
        ensure!(
            original == self.digest && current == self.digest,
            "The verified update package failed SHA-256 verification. Download it again."
        );
        Ok(())
    }
    /// A requested handoff retains the private package for the system installer,
    /// even if an asynchronous system acknowledgment is later interrupted.
    /// Until this is called, dropping the artifact removes the owned directory.
    pub fn retain_for_handoff(&self) -> Result<PathBuf> {
        self.revalidate()?;
        if let Some(directory) = self.directory.lock().unwrap().take() {
            let _ = directory.keep();
        }
        Ok(self.path.clone())
    }
}
fn hash_file(file: &mut File, size: u64) -> Result<[u8; 32]> {
    file.seek(SeekFrom::Start(0))?;
    let mut source = file.take(size.saturating_add(1));
    let mut hash = Sha256::new();
    let mut count = 0;
    let mut buffer = [0; 65536];
    loop {
        let length = source.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        count += length as u64;
        hash.update(&buffer[..length]);
    }
    ensure!(
        count == size,
        "The verified update package size changed. Download it again."
    );
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    #[test]
    fn checksum_entries_accept_standard_binary_markers_and_reject_missing_or_ambiguous_names()
    -> Result<()> {
        assert_eq!(
            checksum(
                format!("{ABC}  synthetic.pkg\n").as_bytes(),
                "synthetic.pkg"
            )?,
            digest(ABC)?
        );
        assert_eq!(
            checksum(
                format!("{} *synthetic.pkg\r\n", ABC.to_uppercase()).as_bytes(),
                "synthetic.pkg"
            )?,
            digest(ABC)?
        );
        for content in [
            format!("{ABC}  different.pkg"),
            format!("{ABC}  synthetic.pkg\n{ABC}  synthetic.pkg"),
            format!("{ABC}  ../synthetic.pkg"),
            "not a checksum".into(),
        ] {
            assert!(checksum(content.as_bytes(), "synthetic.pkg").is_err());
        }
        Ok(())
    }
    #[test]
    fn cancellation_and_failed_staging_remove_only_the_owned_temporary_directory() -> Result<()> {
        let root = tempfile::tempdir()?;
        let existing = root.path().join("retained-user-file");
        fs::write(&existing, b"unrelated")?;
        let mut cancelled = Staged::new(root.path(), "cancelled.pkg", 3)?;
        cancelled.file.write_all(b"a")?;
        let cancelled_path = cancelled.path.clone();
        drop(cancelled);
        assert!(!cancelled_path.exists());
        let mut staged = Staged::new(root.path(), "synthetic.pkg", 3)?;
        staged.file.write_all(b"ab")?;
        let partial = staged.path.clone();
        assert!(staged.seal(digest(ABC)?, "synthetic".into()).is_err());
        assert!(!partial.exists());
        assert_eq!(fs::read(existing)?, b"unrelated");
        Ok(())
    }
    #[test]
    fn verified_artifacts_detect_tampering_and_release_owned_storage_when_discarded() -> Result<()>
    {
        let root = tempfile::tempdir()?;
        let mut staged = Staged::new(root.path(), "synthetic.pkg", 3)?;
        staged.file.write_all(b"abc")?;
        let verified = staged.seal(digest(ABC)?, "synthetic".into())?;
        let path = verified.path.clone();
        verified.revalidate()?;
        fs::write(&path, b"abd")?;
        assert!(verified.revalidate().is_err());
        drop(verified);
        assert!(!path.exists());
        Ok(())
    }
    #[test]
    fn redirects_must_remain_bounded_https_without_embedded_credentials() -> Result<()> {
        assert!(redirect_allowed(
            &Url::parse("https://objects.example.invalid/asset?signature=synthetic")?,
            1
        ));
        for (url, hops) in [
            ("http://objects.example.invalid/asset", 1),
            ("https://user:pass@objects.example.invalid/asset", 1),
            ("https://objects.example.invalid/asset", 5),
            ("https://objects.example.invalid:8443/asset", 1),
        ] {
            assert!(!redirect_allowed(&Url::parse(url)?, hops));
        }
        Ok(())
    }
    #[test]
    fn managed_downloads_require_the_reviewed_checksum_file_and_unchanged_release_destinations()
    -> Result<()> {
        use super::super::releases::{Repository, Target, decode};
        use serde_json::json;
        let names = [
            "Nauterm-RS-0.4.0-macos-arm64.dmg",
            "Nauterm-RS-0.4.0-macos-x86_64.dmg",
            "Nauterm-RS-0.4.0-windows-arm64-setup.exe",
            "Nauterm-RS-0.4.0-windows-x86_64-setup.exe",
            "Nauterm-RS-0.4.0-linux-arm64.AppImage.tar.gz",
            "Nauterm-RS-0.4.0-linux-x86_64.AppImage.tar.gz",
            "SHA256SUMS.txt",
        ];
        let assets: Vec<_> = names.into_iter().map(|name| json!({"name":name, "state":"uploaded", "size":3,
            "browser_download_url":format!("https://github.com/example/nauterm/releases/download/v0.4.0/{name}"), "digest":format!("sha256:{ABC}")})).collect();
        let bytes = serde_json::to_vec(
            &json!({"tag_name":"v0.4.0", "html_url":"https://github.com/example/nauterm/releases/tag/v0.4.0", "draft":false,"prerelease":false,"assets":assets}),
        )?;
        let release = decode(
            &Repository::parse("example/nauterm")?,
            "0.3.9",
            Target::current(),
            &bytes,
        )?;
        assert!(Plan::new(&release).is_ok());
        let mut changed = release.clone();
        changed.checksums = None;
        assert!(Plan::new(&changed).is_err());
        changed = release.clone();
        changed.asset.as_mut().unwrap().url = "https://other.invalid/package".into();
        assert!(Plan::new(&changed).is_err());
        changed = release.clone();
        changed.checksums.as_mut().unwrap().size = MAX_CHECKSUMS + 1;
        assert!(Plan::new(&changed).is_err());
        changed = release;
        changed.version = "0.5.0".into();
        assert!(Plan::new(&changed).is_err());
        Ok(())
    }
}
