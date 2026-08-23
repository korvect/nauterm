use futures_util::{stream, StreamExt};
use russh_sftp::{
    client::{Config as SftpClientConfig, SftpSession},
    protocol::OpenFlags,
};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use super::*;

static NEXT_STAGING_PATH_ID: AtomicU64 = AtomicU64::new(1);
const SFTP_TRANSFER_BUFFER_SIZE: usize = 256 * 1024;
const SFTP_MIN_DOWNLOAD_CHUNK_SIZE: u64 = 4 * 1024 * 1024;
const SFTP_PART_METADATA_VERSION: &str = "nauterm-sftp-part-v1";
const SFTP_DOWNLOAD_METADATA_VERSION: &str = "nauterm-sftp-download-v1";
const SFTP_DOWNLOAD_METADATA_FLUSH_BYTES: u64 = 4 * 1024 * 1024;
const SFTP_MAX_DIRECTORY_FILE_CONCURRENCY: usize = 8;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
struct DownloadPartMetadata {
    source: String,
    size: u64,
    modified: u64,
    ranges: Vec<DownloadRangeState>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
struct DownloadRangeState {
    start: u64,
    length: u64,
    completed: u64,
}

enum DownloadRangeEvent {
    Progress(u64),
    Durable { index: usize, completed: u64 },
}

pub(super) async fn list_sftp_directory_entries(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<&SshProxyConfig>,
    directory: &str,
    host_key_trust_mode: HostKeyTrustMode,
    cancel: Arc<AtomicBool>,
) -> SftpDirectoryEntries {
    let config = ssh_client_config();
    let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
    let output = Arc::new(Mutex::new(Vec::new()));
    let wakeup = Arc::new(Mutex::new(None));
    let handler = SshClientHandler {
        host: host.to_owned(),
        port,
        known_hosts_path: known_hosts_path.map(PathBuf::from),
        host_key_trust_mode,
        output: output.clone(),
        events: events.clone(),
        wakeup: wakeup.clone(),
    };
    let result = async {
        let mut handle = cancel_on_sftp_cancel(
            &cancel,
            connect_ssh_with_timeout(config, host, port, proxy, handler),
        )
        .await?;

        cancel_on_sftp_cancel(&cancel, async {
            authenticate(
                &mut handle,
                username,
                private_key,
                passphrase,
                password,
                &events,
                &wakeup,
            )
            .await
            .map_err(|error| format!("authentication failed: {error}"))
        })
        .await?;

        let channel = cancel_on_sftp_cancel(&cancel, async {
            handle
                .channel_open_session()
                .await
                .map_err(|error| format!("failed to open SFTP channel: {error}"))
        })
        .await?;
        let channel = cancel_on_sftp_cancel(&cancel, async {
            channel
                .request_subsystem(true, "sftp")
                .await
                .map_err(|error| format!("failed to request SFTP subsystem: {error}"))?;
            Ok(channel)
        })
        .await?;
        let sftp = cancel_on_sftp_cancel(&cancel, async {
            SftpSession::new(channel.into_stream())
                .await
                .map_err(|error| format!("failed to initialize SFTP session: {error}"))
        })
        .await?;
        let resolved_directory =
            cancel_on_sftp_cancel(&cancel, resolve_sftp_directory(&sftp, directory)).await?;
        let read_dir = cancel_on_sftp_cancel(&cancel, async {
            sftp.read_dir(resolved_directory.clone())
                .await
                .map_err(|error| format!("failed to read SFTP directory: {error}"))
        })
        .await?;
        let mut entries = Vec::new();
        for entry in read_dir {
            if cancel.load(Ordering::SeqCst) {
                return Err("SFTP listing cancelled.".to_owned());
            }
            let name = entry.file_name();
            if name.is_empty() || name == "." || name == ".." {
                continue;
            }
            let path = join_remote_path(&resolved_directory, &name);
            let metadata =
                cancel_on_sftp_cancel(&cancel, async { Ok(sftp.metadata(path).await.ok()) })
                    .await?;
            let is_directory = metadata
                .as_ref()
                .map(|metadata| metadata.is_dir())
                .unwrap_or_else(|| entry.file_type().is_dir());
            let modified = metadata.as_ref().and_then(|metadata| metadata.mtime);
            entries.push(SshDirectoryEntry {
                name,
                is_directory,
                size: metadata.and_then(|metadata| metadata.size).unwrap_or(0),
                modified: modified.map(u64::from),
            });
        }
        let _ = sftp.close().await;
        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        entries.sort_by(|a, b| {
            b.is_directory
                .cmp(&a.is_directory)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        entries.dedup_by(|a, b| a.name == b.name && a.is_directory == b.is_directory);
        Ok((resolved_directory, entries))
    }
    .await;
    let captured_events = events
        .lock()
        .map(|events| events.clone())
        .unwrap_or_default();
    match result {
        Ok((directory, entries)) => SftpDirectoryEntries {
            directory,
            entries,
            events: captured_events,
            error: None,
        },
        Err(error) => SftpDirectoryEntries {
            directory: directory.to_owned(),
            entries: Vec::new(),
            events: captured_events,
            error: Some(error),
        },
    }
}

pub(super) async fn open_sftp_session(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<&SshProxyConfig>,
    host_key_trust_mode: HostKeyTrustMode,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    transfer_threads: usize,
) -> Result<(client::Handle<SshClientHandler>, SftpSession), String> {
    let config = ssh_client_config_for_sftp(transfer_threads);
    let output = Arc::new(Mutex::new(Vec::new()));
    let wakeup = Arc::new(Mutex::new(None));
    let handler = SshClientHandler {
        host: host.to_owned(),
        port,
        known_hosts_path: known_hosts_path.map(PathBuf::from),
        host_key_trust_mode,
        output: output.clone(),
        events: events.clone(),
        wakeup: wakeup.clone(),
    };
    let mut handle = connect_ssh_with_timeout(config, host, port, proxy, handler).await?;
    authenticate(
        &mut handle,
        username,
        private_key,
        passphrase,
        password,
        &events,
        &wakeup,
    )
    .await
    .map_err(|error| format!("authentication failed: {error}"))?;
    let channel = handle
        .channel_open_session()
        .await
        .map_err(|error| format!("failed to open SFTP channel: {error}"))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|error| format!("failed to request SFTP subsystem: {error}"))?;
    let sftp =
        SftpSession::new_with_config(channel.into_stream(), sftp_client_config(transfer_threads))
            .await
            .map_err(|error| format!("failed to initialize SFTP session: {error}"))?;
    Ok((handle, sftp))
}

pub(super) async fn open_sudo_sftp_session(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<&SshProxyConfig>,
    host_key_trust_mode: HostKeyTrustMode,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    sudo_password: &str,
    transfer_threads: usize,
) -> Result<(client::Handle<SshClientHandler>, SftpSession), String> {
    const SUDO_SFTP_COMMAND: &str = concat!(
        "sudo -k -S -p '' sh -c '",
        "for p in ",
        "/usr/lib/openssh/sftp-server ",
        "/usr/lib/ssh/sftp-server ",
        "/usr/libexec/openssh/sftp-server ",
        "/usr/libexec/sftp-server; ",
        "do if [ -x \"$p\" ]; then exec \"$p\"; fi; done; ",
        "p=$(command -v sftp-server 2>/dev/null) && [ -n \"$p\" ] && exec \"$p\"; ",
        "exit 127'"
    );

    let config = ssh_client_config_for_sftp(transfer_threads);
    let output = Arc::new(Mutex::new(Vec::new()));
    let wakeup = Arc::new(Mutex::new(None));
    let handler = SshClientHandler {
        host: host.to_owned(),
        port,
        known_hosts_path: known_hosts_path.map(PathBuf::from),
        host_key_trust_mode,
        output: output.clone(),
        events: events.clone(),
        wakeup: wakeup.clone(),
    };
    let mut handle = connect_ssh_with_timeout(config, host, port, proxy, handler).await?;
    authenticate(
        &mut handle,
        username,
        private_key,
        passphrase,
        password,
        &events,
        &wakeup,
    )
    .await
    .map_err(|error| format!("authentication failed: {error}"))?;
    let channel = handle
        .channel_open_session()
        .await
        .map_err(|error| format!("failed to open sudo SFTP channel: {error}"))?;
    channel
        .exec(false, SUDO_SFTP_COMMAND)
        .await
        .map_err(|error| format!("failed to start sudo SFTP server: {error}"))?;
    let mut password_line = Vec::with_capacity(sudo_password.len() + 1);
    password_line.extend_from_slice(sudo_password.as_bytes());
    password_line.push(b'\n');
    let write_result = channel.data(&password_line[..]).await;
    password_line.zeroize();
    write_result.map_err(|error| format!("failed to send sudo password: {error}"))?;
    let sftp = tokio::time::timeout(
        Duration::from_secs(12),
        SftpSession::new_with_config(channel.into_stream(), sftp_client_config(transfer_threads)),
    )
    .await
    .map_err(|_| "sudo authentication failed or the privileged SFTP server timed out".to_owned())?
    .map_err(|error| {
        format!("sudo authentication failed or the privileged SFTP server is unavailable: {error}")
    })?;
    Ok((handle, sftp))
}

fn sftp_client_config(transfer_threads: usize) -> SftpClientConfig {
    SftpClientConfig {
        max_concurrent_writes: transfer_threads.clamp(1, 32),
        ..SftpClientConfig::default()
    }
}

impl SftpTaskProgress {
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    fn ensure_not_cancelled(&self) -> Result<(), String> {
        if self.is_cancelled() {
            Err("SFTP task cancelled.".to_owned())
        } else {
            Ok(())
        }
    }

    fn set_total(&mut self, total_bytes: u64, current_path: &str) {
        self.total_bytes = total_bytes;
        self.report(current_path);
    }

    fn add_bytes(&mut self, bytes: u64, current_path: &str) {
        self.transferred_bytes = self.transferred_bytes.saturating_add(bytes);
        if let Some(concurrent) = self.concurrent.as_ref() {
            concurrent.add_bytes(bytes);
            return;
        }
        self.report(current_path);
    }

    fn report(&self, current_path: &str) {
        let Some(callback) = self.callback else {
            return;
        };
        let _ = current_path;
        callback(
            self.user_data as *mut c_void,
            self.transferred_bytes,
            self.total_bytes,
            ptr::null(),
        );
    }
}

impl SftpConcurrentProgress {
    fn add_bytes(&self, bytes: u64) {
        let transferred = self
            .transferred_bytes
            .fetch_add(bytes, Ordering::SeqCst)
            .saturating_add(bytes);
        let Some(callback) = self.callback else {
            return;
        };
        callback(
            self.user_data as *mut c_void,
            transferred,
            self.total_bytes,
            ptr::null(),
        );
    }
}

fn local_path_total(path: &Path, cancel: &AtomicBool) -> Result<u64, String> {
    if cancel.load(Ordering::SeqCst) {
        return Err("SFTP task cancelled.".to_owned());
    }
    let metadata = fs::metadata(path)
        .map_err(|error| format!("local path not found {}: {error}", path.display()))?;
    if !metadata.is_dir() {
        return Ok(metadata.len());
    }
    let mut total = 0_u64;
    let mut directories = vec![path.to_path_buf()];
    while let Some(directory) = directories.pop() {
        if cancel.load(Ordering::SeqCst) {
            return Err("SFTP task cancelled.".to_owned());
        }
        let entries = fs::read_dir(&directory).map_err(|error| {
            format!(
                "failed to read local folder {}: {error}",
                directory.display()
            )
        })?;
        for entry in entries {
            if cancel.load(Ordering::SeqCst) {
                return Err("SFTP task cancelled.".to_owned());
            }
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read local folder {}: {error}",
                    directory.display()
                )
            })?;
            let metadata = entry.metadata().map_err(|error| {
                format!(
                    "failed to inspect local path {}: {error}",
                    entry.path().display()
                )
            })?;
            if metadata.is_dir() {
                directories.push(entry.path());
            } else {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    Ok(total)
}

async fn remote_path_total(
    sftp: &SftpSession,
    remote_path: &str,
    progress: &SftpTaskProgress,
) -> Result<u64, String> {
    progress.ensure_not_cancelled()?;
    let metadata = sftp
        .metadata(remote_path.to_owned())
        .await
        .map_err(|error| format!("remote path not found {remote_path}: {error}"))?;
    if !metadata.is_dir() {
        return Ok(metadata.size.unwrap_or(0));
    }
    let mut total = 0_u64;
    let mut directories = vec![remote_path.to_owned()];
    while let Some(remote_dir) = directories.pop() {
        progress.ensure_not_cancelled()?;
        let entries = sftp
            .read_dir(remote_dir.clone())
            .await
            .map_err(|error| format!("failed to read remote directory {remote_dir}: {error}"))?;
        for entry in entries {
            progress.ensure_not_cancelled()?;
            let name = entry.file_name();
            if name.is_empty() || name == "." || name == ".." {
                continue;
            }
            if entry.file_type().is_dir() {
                directories.push(join_remote_path(&remote_dir, &name));
            } else {
                total = total.saturating_add(entry.metadata().size.unwrap_or(0));
            }
        }
    }
    Ok(total)
}

pub(super) async fn download_sftp_path(
    sftp: &SftpSession,
    remote_path: &str,
    local_path: &Path,
    transfer_threads: usize,
    progress: &mut SftpTaskProgress,
) -> Result<(u64, String), String> {
    let metadata = sftp
        .metadata(remote_path.to_owned())
        .await
        .map_err(|error| format!("remote path not found {remote_path}: {error}"))?;
    if metadata.is_dir() {
        let mut total = 0_u64;
        let mut files = Vec::new();
        let mut local_directories = vec![PathBuf::new()];
        let mut tree_entries = Vec::new();
        let mut directories = vec![(remote_path.to_owned(), PathBuf::new())];
        while let Some((remote_dir, relative_dir)) = directories.pop() {
            progress.ensure_not_cancelled()?;
            let entries = sftp.read_dir(remote_dir.clone()).await.map_err(|error| {
                format!("failed to read remote directory {remote_dir}: {error}")
            })?;
            for entry in entries {
                progress.ensure_not_cancelled()?;
                let name = entry.file_name();
                if name.is_empty() || name == "." || name == ".." {
                    continue;
                }
                let local_name = safe_local_download_name(&name)?;
                let child_remote = join_remote_path(&remote_dir, &name);
                let child_relative = relative_dir.join(local_name);
                let entry_metadata = entry.metadata();
                let is_directory = entry.file_type().is_dir();
                let size = entry_metadata.size.unwrap_or(0);
                let modified = entry_metadata.mtime.map(u64::from).unwrap_or(0);
                tree_entries.push(format!(
                    "{}\0{}\0{size}\0{modified}",
                    child_relative.to_string_lossy(),
                    u8::from(is_directory),
                ));
                if is_directory {
                    local_directories.push(child_relative.clone());
                    directories.push((child_remote, child_relative));
                } else {
                    total = total.saturating_add(size);
                    files.push((child_remote, child_relative));
                }
            }
        }
        progress.set_total(total, remote_path);
        tree_entries.sort_unstable();
        let tree_fingerprint = sha256_hex(tree_entries.join("\n").as_bytes());
        let staging_path = local_download_staging_path(local_path);
        let metadata_path = local_part_metadata_path(&staging_path);
        let expected_metadata = transfer_part_metadata(
            &format!("{remote_path}#{tree_fingerprint}"),
            total,
            metadata.mtime.map(u64::from).unwrap_or(0),
        );
        prepare_local_directory_staging(&staging_path, &metadata_path, &expected_metadata)?;
        for directory in local_directories {
            let local_directory = staging_path.join(directory);
            fs::create_dir_all(&local_directory).map_err(|error| {
                format!(
                    "failed to create local folder {}: {error}",
                    local_directory.display()
                )
            })?;
        }
        let concurrency = transfer_threads.clamp(1, 32);
        let file_concurrency = concurrency.min(files.len().max(1));
        let per_file_concurrency = concurrency.div_ceil(file_concurrency);
        let cancel = progress.cancel.clone();
        let concurrent_progress = SftpConcurrentProgress {
            callback: progress.callback,
            user_data: progress.user_data,
            total_bytes: total,
            transferred_bytes: Arc::new(AtomicU64::new(0)),
        };
        let transfers = stream::iter(files.into_iter().map(|(remote_file, relative_file)| {
            let cancel = cancel.clone();
            let concurrent_progress = concurrent_progress.clone();
            let local_file = staging_path.join(relative_file);
            async move {
                let mut worker_progress = SftpTaskProgress {
                    callback: None,
                    user_data: 0,
                    cancel,
                    total_bytes: 0,
                    transferred_bytes: 0,
                    concurrent: Some(concurrent_progress),
                };
                download_sftp_file(
                    sftp,
                    &remote_file,
                    &local_file,
                    per_file_concurrency,
                    &mut worker_progress,
                )
                .await
            }
        }))
        .buffer_unordered(file_concurrency);
        tokio::pin!(transfers);
        let mut bytes = 0_u64;
        let mut first_error = None;
        while let Some(result) = transfers.next().await {
            match result {
                Ok(file_bytes) => bytes = bytes.saturating_add(file_bytes),
                Err(error) => {
                    first_error.get_or_insert(error);
                    progress.cancel.store(true, Ordering::SeqCst);
                }
            }
        }
        progress.transferred_bytes = concurrent_progress.transferred_bytes.load(Ordering::SeqCst);
        if let Some(error) = first_error {
            return Err(error);
        }
        commit_local_staging_path(&staging_path, local_path)?;
        let _ = fs::remove_file(metadata_path);
        Ok((bytes, "folder".to_owned()))
    } else {
        progress.set_total(metadata.size.unwrap_or(0), remote_path);
        let bytes =
            download_sftp_file(sftp, remote_path, local_path, transfer_threads, progress).await?;
        Ok((bytes, "file".to_owned()))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut value = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(value, "{byte:02x}");
    }
    value
}

async fn download_sftp_file(
    sftp: &SftpSession,
    remote_path: &str,
    local_path: &Path,
    transfer_threads: usize,
    progress: &mut SftpTaskProgress,
) -> Result<u64, String> {
    progress.ensure_not_cancelled()?;
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create local folder {}: {error}",
                parent.display()
            )
        })?;
    }
    let remote_metadata = sftp
        .metadata(remote_path.to_owned())
        .await
        .map_err(|error| format!("failed to inspect remote file {remote_path}: {error}"))?;
    let remote_size = remote_metadata.size.unwrap_or(0);
    let staging_path = local_download_staging_path(local_path);
    let metadata_path = local_part_metadata_path(&staging_path);
    let mut download_metadata = prepare_local_download_staging(
        &staging_path,
        &metadata_path,
        remote_path,
        remote_size,
        remote_metadata.mtime.map(u64::from).unwrap_or(0),
        transfer_threads,
    )?;
    let resumed_bytes = download_metadata
        .ranges
        .iter()
        .map(|range| range.completed)
        .sum();
    if resumed_bytes > 0 {
        progress.add_bytes(resumed_bytes, remote_path);
    }
    download_sftp_file_ranges(
        sftp,
        remote_path,
        &staging_path,
        &metadata_path,
        &mut download_metadata,
        progress,
    )
    .await?;
    commit_local_staging_path(&staging_path, local_path)?;
    let _ = fs::remove_file(metadata_path);
    Ok(remote_size)
}

async fn download_sftp_file_ranges(
    sftp: &SftpSession,
    remote_path: &str,
    staging_path: &Path,
    metadata_path: &Path,
    metadata: &mut DownloadPartMetadata,
    progress: &mut SftpTaskProgress,
) -> Result<(), String> {
    let mut opened_ranges = Vec::new();
    for (index, range) in metadata.ranges.iter().enumerate() {
        if range.completed >= range.length {
            continue;
        }
        progress.ensure_not_cancelled()?;
        let remote_file = sftp
            .open(remote_path.to_owned())
            .await
            .map_err(|error| format!("failed to open remote file {remote_path}: {error}"))?;
        opened_ranges.push((index, range.clone(), remote_file));
    }

    let (progress_sender, mut progress_receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut tasks = tokio::task::JoinSet::new();
    for (index, range, mut remote_file) in opened_ranges {
        let cancel = progress.cancel.clone();
        let progress_sender = progress_sender.clone();
        let source_path = remote_path.to_owned();
        let staging_path = staging_path.to_path_buf();
        tasks.spawn(async move {
            let offset = range.start.saturating_add(range.completed);
            remote_file
                .seek(SeekFrom::Start(offset))
                .await
                .map_err(|error| format!("failed to seek remote file {source_path}: {error}"))?;
            let mut staging_file =
                OpenOptions::new()
                    .write(true)
                    .open(&staging_path)
                    .map_err(|error| {
                        format!(
                            "failed to open partial download {}: {error}",
                            staging_path.display()
                        )
                    })?;
            staging_file
                .seek(SeekFrom::Start(offset))
                .map_err(|error| {
                    format!(
                        "failed to seek partial download {}: {error}",
                        staging_path.display()
                    )
                })?;
            let mut remaining = range.length.saturating_sub(range.completed);
            let mut range_completed = range.completed;
            let mut pending_durable_bytes = 0_u64;
            let mut buffer = vec![0_u8; SFTP_TRANSFER_BUFFER_SIZE];
            while remaining > 0 {
                if cancel.load(Ordering::SeqCst) {
                    return Err("SFTP task cancelled.".to_owned());
                }
                let requested = remaining.min(buffer.len() as u64) as usize;
                let read = remote_file
                    .read(&mut buffer[..requested])
                    .await
                    .map_err(|error| {
                        format!("failed to read remote file {source_path}: {error}")
                    })?;
                if read == 0 {
                    return Err(format!(
                        "remote file {source_path} ended before the expected size"
                    ));
                }
                staging_file.write_all(&buffer[..read]).map_err(|error| {
                    format!(
                        "failed to write partial download {}: {error}",
                        staging_path.display()
                    )
                })?;
                remaining -= read as u64;
                range_completed = range_completed.saturating_add(read as u64);
                pending_durable_bytes = pending_durable_bytes.saturating_add(read as u64);
                let _ = progress_sender.send(DownloadRangeEvent::Progress(read as u64));
                if pending_durable_bytes >= SFTP_DOWNLOAD_METADATA_FLUSH_BYTES || remaining == 0 {
                    if remaining == 0 {
                        staging_file.sync_all()
                    } else {
                        staging_file.sync_data()
                    }
                    .map_err(|error| {
                        format!(
                            "failed to flush partial download {}: {error}",
                            staging_path.display()
                        )
                    })?;
                    let _ = progress_sender.send(DownloadRangeEvent::Durable {
                        index,
                        completed: range_completed,
                    });
                    pending_durable_bytes = 0;
                }
            }
            remote_file
                .shutdown()
                .await
                .map_err(|error| format!("failed to close remote file {source_path}: {error}"))?;
            Ok::<(), String>(())
        });
    }
    drop(progress_sender);

    let mut first_error = None;
    let mut remaining_tasks = tasks.len();
    while remaining_tasks > 0 {
        tokio::select! {
            range_event = progress_receiver.recv(), if !progress_receiver.is_closed() => {
                if let Some(event) = range_event {
                    apply_download_range_event(
                        event,
                        metadata_path,
                        metadata,
                        remote_path,
                        progress,
                    )?;
                }
            }
            joined = tasks.join_next() => {
                remaining_tasks -= 1;
                match joined {
                    Some(Ok(Ok(()))) => {}
                    Some(Ok(Err(error))) => {
                        first_error.get_or_insert(error);
                    }
                    Some(Err(error)) => {
                        first_error.get_or_insert_with(|| {
                            format!("download worker failed for {remote_path}: {error}")
                        });
                    }
                    None => break,
                };
            }
        }
    }
    while let Ok(event) = progress_receiver.try_recv() {
        apply_download_range_event(event, metadata_path, metadata, remote_path, progress)?;
    }
    write_local_download_metadata(metadata_path, metadata)?;
    if let Some(error) = first_error {
        return Err(error);
    }
    if metadata
        .ranges
        .iter()
        .any(|range| range.completed != range.length)
    {
        return Err(format!(
            "download did not complete every range for {remote_path}"
        ));
    }
    OpenOptions::new()
        .write(true)
        .open(staging_path)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            format!(
                "failed to flush partial download {}: {error}",
                staging_path.display()
            )
        })
}

fn apply_download_range_event(
    event: DownloadRangeEvent,
    metadata_path: &Path,
    metadata: &mut DownloadPartMetadata,
    remote_path: &str,
    progress: &mut SftpTaskProgress,
) -> Result<(), String> {
    match event {
        DownloadRangeEvent::Progress(bytes) => {
            progress.add_bytes(bytes, remote_path);
            Ok(())
        }
        DownloadRangeEvent::Durable { index, completed } => {
            metadata.ranges[index].completed = completed.min(metadata.ranges[index].length);
            write_local_download_metadata(metadata_path, metadata)
        }
    }
}

fn download_chunk_ranges(offset: u64, total: u64, transfer_threads: usize) -> Vec<(u64, u64)> {
    let remaining = total.saturating_sub(offset);
    if remaining == 0 {
        return Vec::new();
    }
    let desired = transfer_threads.clamp(1, 32) as u64;
    let count = desired.min(remaining.div_ceil(SFTP_MIN_DOWNLOAD_CHUNK_SIZE).max(1));
    let base_length = remaining / count;
    let extra = remaining % count;
    let mut ranges = Vec::with_capacity(count as usize);
    let mut start = offset;
    for index in 0..count {
        let length = base_length + u64::from(index < extra);
        ranges.push((start, length));
        start += length;
    }
    ranges
}

pub(super) async fn upload_sftp_path(
    sftp: &SftpSession,
    local_path: &Path,
    remote_path: &str,
    replace_existing: bool,
    transfer_concurrency: usize,
    progress: &mut SftpTaskProgress,
) -> Result<(u64, String), String> {
    let total = local_path_total(local_path, &progress.cancel)?;
    progress.set_total(total, &local_path.to_string_lossy());
    let metadata = fs::metadata(local_path)
        .map_err(|error| format!("local path not found {}: {error}", local_path.display()))?;
    if !replace_existing && sftp.metadata(remote_path.to_owned()).await.is_ok() {
        return Err(format!("remote target already exists: {remote_path}"));
    }
    let staging_path = if metadata.is_dir() {
        unique_remote_staging_path(sftp, remote_path, "upload").await?
    } else {
        remote_upload_staging_path(remote_path)
    };
    let upload_result: Result<(u64, String), String> = async {
        if metadata.is_dir() {
            ensure_sftp_directory(sftp, &staging_path).await?;
            let mut files = Vec::new();
            let mut directories = vec![(local_path.to_path_buf(), staging_path.clone())];
            while let Some((local_dir, remote_dir)) = directories.pop() {
                progress.ensure_not_cancelled()?;
                let entries = fs::read_dir(&local_dir).map_err(|error| {
                    format!(
                        "failed to read local folder {}: {error}",
                        local_dir.display()
                    )
                })?;
                for entry in entries {
                    progress.ensure_not_cancelled()?;
                    let entry = entry.map_err(|error| {
                        format!(
                            "failed to read local folder {}: {error}",
                            local_dir.display()
                        )
                    })?;
                    let child_local = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    let child_remote = join_remote_path(&remote_dir, &name);
                    let child_metadata = entry.metadata().map_err(|error| {
                        format!(
                            "failed to inspect local path {}: {error}",
                            child_local.display()
                        )
                    })?;
                    if child_metadata.is_dir() {
                        ensure_sftp_directory(sftp, &child_remote).await?;
                        directories.push((child_local, child_remote));
                    } else {
                        files.push((child_local, child_remote));
                    }
                }
            }
            let concurrency = transfer_concurrency
                .clamp(1, 32)
                .min(SFTP_MAX_DIRECTORY_FILE_CONCURRENCY);
            let cancel = progress.cancel.clone();
            let concurrent_progress = SftpConcurrentProgress {
                callback: progress.callback,
                user_data: progress.user_data,
                total_bytes: total,
                transferred_bytes: Arc::new(AtomicU64::new(0)),
            };
            let transfers = stream::iter(files.into_iter().map(|(local_file, remote_file)| {
                let cancel = cancel.clone();
                let concurrent_progress = concurrent_progress.clone();
                async move {
                    let mut worker_progress = SftpTaskProgress {
                        callback: None,
                        user_data: 0,
                        cancel,
                        total_bytes: 0,
                        transferred_bytes: 0,
                        concurrent: Some(concurrent_progress),
                    };
                    upload_sftp_file(sftp, &local_file, &remote_file, false, &mut worker_progress)
                        .await
                }
            }))
            .buffer_unordered(concurrency);
            tokio::pin!(transfers);
            let mut bytes = 0_u64;
            let mut first_error = None;
            while let Some(result) = transfers.next().await {
                match result {
                    Ok(file_bytes) => bytes = bytes.saturating_add(file_bytes),
                    Err(error) => {
                        first_error.get_or_insert(error);
                        progress.cancel.store(true, Ordering::SeqCst);
                    }
                }
            }
            progress.transferred_bytes =
                concurrent_progress.transferred_bytes.load(Ordering::SeqCst);
            if let Some(error) = first_error {
                return Err(error);
            }
            Ok((bytes, "folder".to_owned()))
        } else {
            let bytes = upload_sftp_file(sftp, local_path, &staging_path, true, progress).await?;
            Ok((bytes, "file".to_owned()))
        }
    }
    .await;
    let uploaded = match upload_result {
        Ok(uploaded) => uploaded,
        Err(error) => {
            if metadata.is_dir() {
                let _ = remove_remote_path_quiet(sftp, &staging_path).await;
            }
            return Err(error);
        }
    };
    if let Err(error) =
        commit_remote_staging_path(sftp, &staging_path, remote_path, replace_existing).await
    {
        let _ = remove_remote_path_quiet(sftp, &staging_path).await;
        if !metadata.is_dir() {
            let _ = sftp
                .remove_file(remote_part_metadata_path(&staging_path))
                .await;
        }
        return Err(error);
    }
    if !metadata.is_dir() {
        let _ = sftp
            .remove_file(remote_part_metadata_path(&staging_path))
            .await;
    }
    Ok(uploaded)
}

async fn upload_sftp_file(
    sftp: &SftpSession,
    local_path: &Path,
    remote_path: &str,
    resume_existing: bool,
    progress: &mut SftpTaskProgress,
) -> Result<u64, String> {
    progress.ensure_not_cancelled()?;
    if let Some(parent) = remote_parent_path(remote_path) {
        ensure_sftp_directory(sftp, &parent).await?;
    }
    if !resume_existing && sftp.metadata(remote_path.to_owned()).await.is_ok() {
        return Err(format!("remote target already exists: {remote_path}"));
    }
    let local_metadata = fs::metadata(local_path).map_err(|error| {
        format!(
            "failed to inspect local file {}: {error}",
            local_path.display()
        )
    })?;
    let local_size = local_metadata.len();
    let modified = local_metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let resume_offset = if resume_existing {
        let metadata_path = remote_part_metadata_path(remote_path);
        // Do not expose the user's local path in a metadata file on the server.
        let expected_metadata = transfer_part_metadata("local-source", local_size, modified);
        let offset = reusable_remote_part_size(
            sftp,
            remote_path,
            &metadata_path,
            &expected_metadata,
            local_size,
        )
        .await?;
        write_remote_part_metadata(sftp, &metadata_path, &expected_metadata).await?;
        offset
    } else {
        0
    };

    let mut local_file = fs::File::open(local_path).map_err(|error| {
        format!(
            "failed to open local file {}: {error}",
            local_path.display()
        )
    })?;
    if resume_offset > 0 {
        local_file
            .seek(SeekFrom::Start(resume_offset))
            .map_err(|error| {
                format!(
                    "failed to resume local file {}: {error}",
                    local_path.display()
                )
            })?;
    }
    let mut remote_file = if resume_offset == 0 {
        sftp.create(remote_path.to_owned()).await
    } else {
        sftp.open_with_flags(remote_path.to_owned(), OpenFlags::CREATE | OpenFlags::WRITE)
            .await
    }
    .map_err(|error| format!("failed to open partial upload {remote_path}: {error}"))?;
    if resume_offset > 0 {
        remote_file
            .seek(SeekFrom::Start(resume_offset))
            .await
            .map_err(|error| format!("failed to resume partial upload {remote_path}: {error}"))?;
        progress.add_bytes(resume_offset, &local_path.to_string_lossy());
    }
    let mut buffer = vec![0_u8; SFTP_TRANSFER_BUFFER_SIZE];
    loop {
        progress.ensure_not_cancelled()?;
        let read = local_file.read(&mut buffer).map_err(|error| {
            format!(
                "failed to read local file {}: {error}",
                local_path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        remote_file
            .write_all(&buffer[..read])
            .await
            .map_err(|error| format!("failed to write remote file {remote_path}: {error}"))?;
        progress.add_bytes(read as u64, &local_path.to_string_lossy());
    }
    remote_file
        .sync_all()
        .await
        .map_err(|error| format!("failed to flush remote file {remote_path}: {error}"))?;
    remote_file
        .shutdown()
        .await
        .map_err(|error| format!("failed to close remote file {remote_path}: {error}"))?;
    Ok(local_size)
}

fn local_download_staging_path(target_path: &Path) -> PathBuf {
    let name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("download");
    target_path.with_file_name(format!(".{name}.nauterm-download.part"))
}

fn local_part_metadata_path(staging_path: &Path) -> PathBuf {
    let name = staging_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".nauterm-download.part");
    staging_path.with_file_name(format!("{name}.meta"))
}

fn remote_upload_staging_path(target_path: &str) -> String {
    let parent = remote_parent_path(target_path).unwrap_or_else(|| ".".to_owned());
    let name = target_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("upload");
    join_remote_path(&parent, &format!(".{name}.nauterm-upload.part"))
}

fn remote_part_metadata_path(staging_path: &str) -> String {
    format!("{staging_path}.meta")
}

pub(super) async fn cleanup_sftp_upload_parts(sftp: &SftpSession, target_path: &str) {
    let staging_path = remote_upload_staging_path(target_path);
    let metadata_path = remote_part_metadata_path(&staging_path);
    let owned = read_remote_part_metadata(sftp, &metadata_path)
        .await
        .as_deref()
        .is_some_and(is_sftp_part_metadata);
    if owned && remove_remote_path_quiet(sftp, &staging_path).await.is_ok() {
        let _ = sftp.remove_file(metadata_path).await;
    }
}

fn transfer_part_metadata(source: &str, size: u64, modified: impl std::fmt::Display) -> String {
    format!("{SFTP_PART_METADATA_VERSION}\n{size}\n{modified}\n{source}")
}

fn is_sftp_part_metadata(value: &str) -> bool {
    value
        .strip_prefix(SFTP_PART_METADATA_VERSION)
        .is_some_and(|suffix| suffix.starts_with('\n'))
}

fn is_local_download_metadata(value: &str) -> bool {
    is_sftp_part_metadata(value)
        || value
            .strip_prefix(SFTP_DOWNLOAD_METADATA_VERSION)
            .is_some_and(|suffix| suffix.starts_with('\n'))
}

fn prepare_local_download_staging(
    staging_path: &Path,
    metadata_path: &Path,
    source: &str,
    size: u64,
    modified: u64,
    transfer_concurrency: usize,
) -> Result<DownloadPartMetadata, String> {
    let existing_metadata = fs::read_to_string(metadata_path).ok();
    let owned = existing_metadata
        .as_deref()
        .is_some_and(is_local_download_metadata);
    if existing_metadata.is_some() && !owned {
        return Err(format!(
            "refusing to overwrite unowned download metadata path {}",
            metadata_path.display()
        ));
    }
    let reusable = existing_metadata
        .as_deref()
        .and_then(parse_local_download_metadata)
        .filter(|metadata| {
            metadata.source == source
                && metadata.size == size
                && metadata.modified == modified
                && valid_download_ranges(&metadata.ranges, size)
                && staging_path.is_file()
                && fs::metadata(staging_path).is_ok_and(|staging| staging.len() == size)
        });
    if let Some(metadata) = reusable {
        return Ok(metadata);
    }
    if staging_path.exists() {
        if !owned {
            return Err(format!(
                "refusing to overwrite unowned download staging path {}",
                staging_path.display()
            ));
        }
        remove_local_path(staging_path)?;
    }
    if owned {
        let _ = fs::remove_file(metadata_path);
    }
    let metadata = DownloadPartMetadata {
        source: source.to_owned(),
        size,
        modified,
        ranges: download_chunk_ranges(0, size, transfer_concurrency)
            .into_iter()
            .map(|(start, length)| DownloadRangeState {
                start,
                length,
                completed: 0,
            })
            .collect(),
    };
    write_local_download_metadata(metadata_path, &metadata)?;
    let staging = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(staging_path)
        .map_err(|error| {
            format!(
                "failed to create partial download {}: {error}",
                staging_path.display()
            )
        })?;
    staging.set_len(size).map_err(|error| {
        format!(
            "failed to allocate partial download {}: {error}",
            staging_path.display()
        )
    })?;
    Ok(metadata)
}

fn parse_local_download_metadata(value: &str) -> Option<DownloadPartMetadata> {
    let payload = value
        .strip_prefix(SFTP_DOWNLOAD_METADATA_VERSION)?
        .strip_prefix('\n')?;
    serde_json::from_str(payload).ok()
}

fn write_local_download_metadata(
    metadata_path: &Path,
    metadata: &DownloadPartMetadata,
) -> Result<(), String> {
    let payload = serde_json::to_string(metadata)
        .map_err(|error| format!("failed to encode download resume metadata: {error}"))?;
    fs::write(
        metadata_path,
        format!("{SFTP_DOWNLOAD_METADATA_VERSION}\n{payload}"),
    )
    .map_err(|error| {
        format!(
            "failed to write download resume metadata {}: {error}",
            metadata_path.display()
        )
    })
}

fn valid_download_ranges(ranges: &[DownloadRangeState], size: u64) -> bool {
    if size == 0 {
        return ranges.is_empty();
    }
    let mut expected_start = 0;
    for range in ranges {
        if range.start != expected_start || range.length == 0 || range.completed > range.length {
            return false;
        }
        let Some(end) = range.start.checked_add(range.length) else {
            return false;
        };
        expected_start = end;
    }
    expected_start == size
}

fn prepare_local_directory_staging(
    staging_path: &Path,
    metadata_path: &Path,
    expected_metadata: &str,
) -> Result<(), String> {
    let existing_metadata = fs::read_to_string(metadata_path).ok();
    let owned = existing_metadata
        .as_deref()
        .is_some_and(is_sftp_part_metadata);
    if existing_metadata.is_some() && !owned {
        return Err(format!(
            "refusing to overwrite unowned download metadata path {}",
            metadata_path.display()
        ));
    }
    let reusable = existing_metadata.as_deref() == Some(expected_metadata) && staging_path.is_dir();
    if staging_path.exists() && !reusable {
        if !owned {
            return Err(format!(
                "refusing to overwrite unowned download staging path {}",
                staging_path.display()
            ));
        }
        remove_local_path(staging_path)?;
    }
    if owned && !reusable {
        let _ = fs::remove_file(metadata_path);
    }
    fs::create_dir_all(staging_path).map_err(|error| {
        format!(
            "failed to create partial download folder {}: {error}",
            staging_path.display()
        )
    })?;
    fs::write(metadata_path, expected_metadata).map_err(|error| {
        format!(
            "failed to write download resume metadata {}: {error}",
            metadata_path.display()
        )
    })
}

fn remove_local_path(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to inspect partial download {}: {error}",
            path.display()
        )
    })?;
    let result = if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    result.map_err(|error| {
        format!(
            "failed to remove partial download {}: {error}",
            path.display()
        )
    })
}

async fn reusable_remote_part_size(
    sftp: &SftpSession,
    staging_path: &str,
    metadata_path: &str,
    expected_metadata: &str,
    source_size: u64,
) -> Result<u64, String> {
    let existing_metadata = read_remote_part_metadata(sftp, metadata_path).await;
    let owned = existing_metadata
        .as_deref()
        .is_some_and(is_sftp_part_metadata);
    if existing_metadata.is_some() && !owned {
        return Err(format!(
            "refusing to overwrite unowned upload metadata path {metadata_path}"
        ));
    }
    let partial_size = sftp
        .metadata(staging_path.to_owned())
        .await
        .ok()
        .and_then(|metadata| metadata.size)
        .unwrap_or(0);
    if existing_metadata.as_deref() == Some(expected_metadata) && partial_size <= source_size {
        return Ok(partial_size);
    }
    if sftp.metadata(staging_path.to_owned()).await.is_ok() {
        if !owned {
            return Err(format!(
                "refusing to overwrite unowned upload staging path {staging_path}"
            ));
        }
        sftp.remove_file(staging_path.to_owned())
            .await
            .map_err(|error| format!("failed to reset partial upload {staging_path}: {error}"))?;
    }
    if owned {
        let _ = sftp.remove_file(metadata_path.to_owned()).await;
    }
    Ok(0)
}

async fn read_remote_part_metadata(sftp: &SftpSession, path: &str) -> Option<String> {
    let mut file = sftp.open(path.to_owned()).await.ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).await.ok()?;
    String::from_utf8(bytes).ok()
}

async fn write_remote_part_metadata(
    sftp: &SftpSession,
    path: &str,
    metadata: &str,
) -> Result<(), String> {
    let mut file = sftp
        .create(path.to_owned())
        .await
        .map_err(|error| format!("failed to create upload resume metadata {path}: {error}"))?;
    file.write_all(metadata.as_bytes())
        .await
        .map_err(|error| format!("failed to write upload resume metadata {path}: {error}"))?;
    file.shutdown()
        .await
        .map_err(|error| format!("failed to close upload resume metadata {path}: {error}"))
}

fn commit_local_staging_path(staging_path: &Path, target_path: &Path) -> Result<(), String> {
    if !target_path.exists() {
        return fs::rename(staging_path, target_path).map_err(|error| {
            format!(
                "failed to publish download as {}: {error}",
                target_path.display()
            )
        });
    }
    let backup_path = target_path.with_file_name(format!(
        ".{}.nauterm-download-backup-{}-{}.tmp",
        target_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("download"),
        std::process::id(),
        NEXT_STAGING_PATH_ID.fetch_add(1, AtomicOrdering::Relaxed),
    ));
    fs::rename(target_path, &backup_path).map_err(|error| {
        format!(
            "failed to preserve existing download {}: {error}",
            target_path.display()
        )
    })?;
    if let Err(error) = fs::rename(staging_path, target_path) {
        let restore_error = fs::rename(&backup_path, target_path).err();
        return Err(format!(
            "failed to publish download as {}: {error}{}",
            target_path.display(),
            restore_error
                .map(|restore| format!("; restoring the previous file also failed: {restore}"))
                .unwrap_or_default(),
        ));
    }
    let _ = remove_local_path(&backup_path);
    Ok(())
}

pub(super) async fn move_sftp_path(
    sftp: &SftpSession,
    source_path: &str,
    target_path: &str,
    replace_existing: bool,
    progress: &mut SftpTaskProgress,
) -> Result<(u64, String), String> {
    progress.ensure_not_cancelled()?;
    if same_remote_path(source_path, target_path) {
        return Err("source and target paths are the same".to_owned());
    }
    let source_metadata = sftp
        .metadata(source_path.to_owned())
        .await
        .map_err(|error| format!("remote path not found {source_path}: {error}"))?;
    if source_metadata.is_dir() && remote_path_is_child(target_path, source_path) {
        return Err("cannot move a folder into itself".to_owned());
    }
    let item_kind = if source_metadata.is_dir() {
        "folder".to_owned()
    } else {
        "file".to_owned()
    };
    if sftp.metadata(target_path.to_owned()).await.is_ok() {
        if !replace_existing {
            return Err(format!("remote target already exists: {target_path}"));
        }
        if remote_path_is_child(source_path, target_path) {
            return Err("cannot replace a path with one of its children".to_owned());
        }
        delete_sftp_path(sftp, target_path, progress).await?;
    }
    progress.set_total(0, source_path);
    if let Some(parent) = remote_parent_path(target_path) {
        ensure_sftp_directory(sftp, &parent).await?;
    }
    sftp.rename(source_path.to_owned(), target_path.to_owned())
        .await
        .map_err(|error| format!("failed to move {source_path} to {target_path}: {error}"))?;
    progress.report(target_path);
    Ok((0, item_kind))
}

pub(super) async fn copy_sftp_path(
    sftp: &SftpSession,
    source_path: &str,
    target_path: &str,
    replace_existing: bool,
    progress: &mut SftpTaskProgress,
) -> Result<(u64, String), String> {
    progress.ensure_not_cancelled()?;
    if same_remote_path(source_path, target_path) {
        return Err("source and target paths are the same".to_owned());
    }
    let metadata = sftp
        .metadata(source_path.to_owned())
        .await
        .map_err(|error| format!("remote path not found {source_path}: {error}"))?;
    if metadata.is_dir() && remote_path_is_child(target_path, source_path) {
        return Err("cannot copy a folder into itself".to_owned());
    }
    if sftp.metadata(target_path.to_owned()).await.is_ok() {
        if !replace_existing {
            return Err(format!("remote target already exists: {target_path}"));
        }
        if remote_path_is_child(source_path, target_path) {
            return Err("cannot replace a path with one of its children".to_owned());
        }
        delete_sftp_path(sftp, target_path, progress).await?;
    }
    let total = remote_path_total(sftp, source_path, progress).await?;
    progress.set_total(total, source_path);
    if metadata.is_dir() {
        ensure_sftp_directory(sftp, target_path).await?;
        let mut bytes = 0_u64;
        let mut directories = vec![(source_path.to_owned(), target_path.to_owned())];
        while let Some((source_dir, target_dir)) = directories.pop() {
            progress.ensure_not_cancelled()?;
            let entries = sftp.read_dir(source_dir.clone()).await.map_err(|error| {
                format!("failed to read remote directory {source_dir}: {error}")
            })?;
            for entry in entries {
                progress.ensure_not_cancelled()?;
                let name = entry.file_name();
                if name.is_empty() || name == "." || name == ".." {
                    continue;
                }
                let child_source = join_remote_path(&source_dir, &name);
                let child_target = join_remote_path(&target_dir, &name);
                if entry.file_type().is_dir() {
                    ensure_sftp_directory(sftp, &child_target).await?;
                    directories.push((child_source, child_target));
                } else {
                    bytes = bytes.saturating_add(
                        copy_sftp_file(sftp, &child_source, &child_target, progress).await?,
                    );
                }
            }
        }
        Ok((bytes, "folder".to_owned()))
    } else {
        let bytes = copy_sftp_file(sftp, source_path, target_path, progress).await?;
        Ok((bytes, "file".to_owned()))
    }
}

async fn copy_sftp_file(
    sftp: &SftpSession,
    source_path: &str,
    target_path: &str,
    progress: &mut SftpTaskProgress,
) -> Result<u64, String> {
    progress.ensure_not_cancelled()?;
    if let Some(parent) = remote_parent_path(target_path) {
        ensure_sftp_directory(sftp, &parent).await?;
    }
    let mut source_file = sftp
        .open(source_path.to_owned())
        .await
        .map_err(|error| format!("failed to open remote file {source_path}: {error}"))?;
    let mut target_file = sftp
        .create(target_path.to_owned())
        .await
        .map_err(|error| format!("failed to create remote file {target_path}: {error}"))?;
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut bytes = 0_u64;
    loop {
        progress.ensure_not_cancelled()?;
        let read = source_file
            .read(&mut buffer)
            .await
            .map_err(|error| format!("failed to read remote file {source_path}: {error}"))?;
        if read == 0 {
            break;
        }
        target_file
            .write_all(&buffer[..read])
            .await
            .map_err(|error| format!("failed to write remote file {target_path}: {error}"))?;
        bytes = bytes.saturating_add(read as u64);
        progress.add_bytes(read as u64, source_path);
    }
    target_file
        .shutdown()
        .await
        .map_err(|error| format!("failed to close remote file {target_path}: {error}"))?;
    Ok(bytes)
}

async fn unique_remote_staging_path(
    sftp: &SftpSession,
    target_path: &str,
    purpose: &str,
) -> Result<String, String> {
    let parent = remote_parent_path(target_path).unwrap_or_else(|| ".".to_owned());
    let name = target_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("item");
    for _ in 0..100 {
        let id = NEXT_STAGING_PATH_ID.fetch_add(1, AtomicOrdering::Relaxed);
        let candidate = join_remote_path(
            &parent,
            &format!(".{name}.nauterm-{purpose}-{}-{id}.tmp", std::process::id()),
        );
        if sftp.metadata(candidate.clone()).await.is_err() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "failed to allocate a temporary path beside {target_path}"
    ))
}

async fn commit_remote_staging_path(
    sftp: &SftpSession,
    staging_path: &str,
    target_path: &str,
    replace_existing: bool,
) -> Result<(), String> {
    let target_exists = sftp.metadata(target_path.to_owned()).await.is_ok();
    if !target_exists {
        return sftp
            .rename(staging_path.to_owned(), target_path.to_owned())
            .await
            .map_err(|error| {
                format!("failed to publish temporary upload as {target_path}: {error}")
            });
    }
    if !replace_existing {
        return Err(format!("remote target already exists: {target_path}"));
    }

    // Servers implementing POSIX rename semantics replace the destination here
    // atomically. SFTP v3-only servers commonly reject this and use the guarded
    // backup fallback below.
    if sftp
        .rename(staging_path.to_owned(), target_path.to_owned())
        .await
        .is_ok()
    {
        return Ok(());
    }

    let backup_path = unique_remote_staging_path(sftp, target_path, "backup").await?;
    sftp.rename(target_path.to_owned(), backup_path.clone())
        .await
        .map_err(|error| format!("failed to preserve existing target {target_path}: {error}"))?;
    if let Err(error) = sftp
        .rename(staging_path.to_owned(), target_path.to_owned())
        .await
    {
        let restore_error = sftp
            .rename(backup_path.clone(), target_path.to_owned())
            .await
            .err();
        return Err(match restore_error {
            Some(restore_error) => format!(
                "failed to publish {target_path}: {error}; restoring the previous target also failed: {restore_error}"
            ),
            None => format!("failed to publish {target_path}: {error}"),
        });
    }
    let _ = remove_remote_path_quiet(sftp, &backup_path).await;
    Ok(())
}

async fn remove_remote_path_quiet(sftp: &SftpSession, target_path: &str) -> Result<(), String> {
    let metadata = match sftp.metadata(target_path.to_owned()).await {
        Ok(metadata) => metadata,
        Err(_) => return Ok(()),
    };
    if !metadata.is_dir() {
        return sftp
            .remove_file(target_path.to_owned())
            .await
            .map_err(|error| format!("failed to remove temporary file {target_path}: {error}"));
    }
    let mut directories = Vec::new();
    let mut pending = vec![target_path.to_owned()];
    while let Some(directory) = pending.pop() {
        directories.push(directory.clone());
        let entries = sftp
            .read_dir(directory.clone())
            .await
            .map_err(|error| format!("failed to inspect temporary folder {directory}: {error}"))?;
        for entry in entries {
            let name = entry.file_name();
            if name.is_empty() || name == "." || name == ".." {
                continue;
            }
            let child = join_remote_path(&directory, &name);
            if entry.file_type().is_dir() {
                pending.push(child);
            } else {
                sftp.remove_file(child.clone())
                    .await
                    .map_err(|error| format!("failed to remove temporary file {child}: {error}"))?;
            }
        }
    }
    for directory in directories.into_iter().rev() {
        sftp.remove_dir(directory.clone())
            .await
            .map_err(|error| format!("failed to remove temporary folder {directory}: {error}"))?;
    }
    Ok(())
}

pub(super) async fn delete_sftp_path(
    sftp: &SftpSession,
    target_path: &str,
    progress: &mut SftpTaskProgress,
) -> Result<(u64, String), String> {
    progress.ensure_not_cancelled()?;
    let total = remote_path_total(sftp, target_path, progress).await?;
    progress.set_total(total, target_path);
    let metadata = sftp
        .metadata(target_path.to_owned())
        .await
        .map_err(|error| format!("remote path not found {target_path}: {error}"))?;
    if metadata.is_dir() {
        let mut bytes = 0_u64;
        let mut directories = Vec::new();
        let mut pending = vec![target_path.to_owned()];
        while let Some(remote_dir) = pending.pop() {
            progress.ensure_not_cancelled()?;
            directories.push(remote_dir.clone());
            let entries = sftp.read_dir(remote_dir.clone()).await.map_err(|error| {
                format!("failed to read remote directory {remote_dir}: {error}")
            })?;
            for entry in entries {
                progress.ensure_not_cancelled()?;
                let name = entry.file_name();
                if name.is_empty() || name == "." || name == ".." {
                    continue;
                }
                let child_path = join_remote_path(&remote_dir, &name);
                if entry.file_type().is_dir() {
                    pending.push(child_path);
                } else {
                    let size = entry.metadata().size.unwrap_or(0);
                    sftp.remove_file(child_path.clone())
                        .await
                        .map_err(|error| {
                            format!("failed to delete remote file {child_path}: {error}")
                        })?;
                    bytes = bytes.saturating_add(size);
                    if size == 0 {
                        progress.report(&child_path);
                    } else {
                        progress.add_bytes(size, &child_path);
                    }
                }
            }
        }
        for remote_dir in directories.iter().rev() {
            progress.ensure_not_cancelled()?;
            sftp.remove_dir(remote_dir.clone())
                .await
                .map_err(|error| format!("failed to delete remote folder {remote_dir}: {error}"))?;
            progress.report(remote_dir);
        }
        Ok((bytes, "folder".to_owned()))
    } else {
        let size = metadata.size.unwrap_or(0);
        sftp.remove_file(target_path.to_owned())
            .await
            .map_err(|error| format!("failed to delete remote file {target_path}: {error}"))?;
        if size == 0 {
            progress.report(target_path);
        } else {
            progress.add_bytes(size, target_path);
        }
        Ok((size, "file".to_owned()))
    }
}

pub(super) async fn mkdir_sftp_path(
    sftp: &SftpSession,
    target_path: &str,
    progress: &mut SftpTaskProgress,
) -> Result<(u64, String), String> {
    progress.ensure_not_cancelled()?;
    progress.set_total(0, target_path);
    if sftp.metadata(target_path.to_owned()).await.is_ok() {
        return Err(format!("remote target already exists: {target_path}"));
    }
    if let Some(parent) = remote_parent_path(target_path) {
        ensure_sftp_directory(sftp, &parent).await?;
    }
    sftp.create_dir(target_path.to_owned())
        .await
        .map_err(|error| format!("failed to create remote folder {target_path}: {error}"))?;
    progress.report(target_path);
    Ok((0, "folder".to_owned()))
}

async fn ensure_sftp_directory(sftp: &SftpSession, path: &str) -> Result<(), String> {
    let normalized = path.trim().trim_end_matches('/');
    if normalized.is_empty() || normalized == "/" || normalized == "." {
        return Ok(());
    }
    if let Ok(metadata) = sftp.metadata(normalized.to_owned()).await {
        if metadata.is_dir() {
            return Ok(());
        }
        return Err(format!(
            "remote path exists and is not a directory: {normalized}"
        ));
    }
    let mut current = if normalized.starts_with('/') {
        "/".to_owned()
    } else {
        String::new()
    };
    for part in normalized.split('/').filter(|part| !part.is_empty()) {
        current = join_remote_path(&current, part);
        if let Ok(metadata) = sftp.metadata(current.clone()).await {
            if metadata.is_dir() {
                continue;
            }
            return Err(format!(
                "remote path exists and is not a directory: {current}"
            ));
        }
        match sftp.create_dir(current.clone()).await {
            Ok(()) => {}
            Err(_) => {
                if !sftp
                    .metadata(current.clone())
                    .await
                    .map(|metadata| metadata.is_dir())
                    .unwrap_or(false)
                {
                    return Err(format!("failed to create remote directory {current}"));
                }
            }
        }
    }
    Ok(())
}

fn remote_parent_path(path: &str) -> Option<String> {
    let trimmed = path.trim().trim_end_matches('/');
    let index = trimmed.rfind('/')?;
    if index == 0 {
        Some("/".to_owned())
    } else {
        Some(trimmed[..index].to_owned())
    }
}

fn same_remote_path(left: &str, right: &str) -> bool {
    normalize_remote_path_for_compare(left) == normalize_remote_path_for_compare(right)
}

fn remote_path_is_child(path: &str, parent: &str) -> bool {
    let path = normalize_remote_path_for_compare(path);
    let parent = normalize_remote_path_for_compare(parent);
    if parent == "/" {
        return path != "/";
    }
    path.starts_with(&format!("{parent}/"))
}

fn normalize_remote_path_for_compare(path: &str) -> String {
    let trimmed = path.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        ".".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn join_remote_path(left: &str, right: &str) -> String {
    let clean_right = right.trim_matches('/');
    if left.is_empty() || left == "." {
        return clean_right.to_owned();
    }
    if left == "/" {
        return format!("/{clean_right}");
    }
    format!("{}/{}", left.trim_end_matches('/'), clean_right)
}

fn safe_local_download_name(name: &str) -> Result<&str, String> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(format!("unsafe remote file name: {name:?}"));
    }
    Ok(name)
}

async fn resolve_sftp_directory(sftp: &SftpSession, directory: &str) -> Result<String, String> {
    let target = if directory == "~" {
        ".".to_owned()
    } else if let Some(rest) = directory.strip_prefix("~/") {
        let home = sftp
            .canonicalize(".")
            .await
            .map_err(|error| format!("failed to resolve SFTP home directory: {error}"))?;
        if rest.is_empty() {
            home
        } else {
            format!("{}/{}", home.trim_end_matches('/'), rest)
        }
    } else {
        directory.to_owned()
    };
    sftp.canonicalize(target.clone())
        .await
        .map_err(|error| format!("failed to resolve SFTP directory {target}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nauterm-sftp-{name}-{}-{}",
            std::process::id(),
            NEXT_STAGING_PATH_ID.fetch_add(1, AtomicOrdering::Relaxed),
        ))
    }

    #[test]
    fn transfer_staging_paths_are_hidden_and_deterministic() {
        assert_eq!(
            local_download_staging_path(Path::new("/tmp/archive.tar")),
            PathBuf::from("/tmp/.archive.tar.nauterm-download.part"),
        );
        assert_eq!(
            remote_upload_staging_path("/srv/archive.tar"),
            "/srv/.archive.tar.nauterm-upload.part",
        );
    }

    #[test]
    fn remote_names_cannot_escape_the_download_staging_directory() {
        assert_eq!(safe_local_download_name("photo.jpg").unwrap(), "photo.jpg");
        assert!(safe_local_download_name("../photo.jpg").is_err());
        assert!(safe_local_download_name("folder/photo.jpg").is_err());
        assert!(safe_local_download_name("folder\\photo.jpg").is_err());
    }

    #[test]
    fn local_download_resumes_only_when_source_metadata_matches() {
        let directory = test_directory("resume");
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("archive.tar");
        let staging = local_download_staging_path(&target);
        let metadata = local_part_metadata_path(&staging);
        let mut initial =
            prepare_local_download_staging(&staging, &metadata, "remote", 20, 5, 4).unwrap();
        initial.ranges[0].completed = initial.ranges[0].length;
        write_local_download_metadata(&metadata, &initial).unwrap();

        let resumed =
            prepare_local_download_staging(&staging, &metadata, "remote", 20, 5, 8).unwrap();
        assert_eq!(resumed, initial);

        let reset =
            prepare_local_download_staging(&staging, &metadata, "remote", 20, 6, 8).unwrap();
        assert!(reset.ranges.iter().all(|range| range.completed == 0));
        assert_eq!(fs::metadata(&staging).unwrap().len(), 20);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn download_staging_never_overwrites_an_unowned_path() {
        let directory = test_directory("unowned");
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("archive.tar");
        let staging = local_download_staging_path(&target);
        let metadata = local_part_metadata_path(&staging);
        fs::write(&staging, b"user data").unwrap();

        let error =
            prepare_local_download_staging(&staging, &metadata, "remote", 100, 0, 4).unwrap_err();

        assert!(error.contains("refusing to overwrite unowned"));
        assert_eq!(fs::read(&staging).unwrap(), b"user data");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn directory_download_uses_an_owned_staging_directory() {
        let directory = test_directory("directory-staging");
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("photos");
        let staging = local_download_staging_path(&target);
        let metadata = local_part_metadata_path(&staging);
        let expected = format!("{SFTP_PART_METADATA_VERSION}\nphotos");

        prepare_local_directory_staging(&staging, &metadata, &expected).unwrap();
        fs::write(staging.join("image.png"), b"image").unwrap();
        commit_local_staging_path(&staging, &target).unwrap();
        fs::remove_file(&metadata).unwrap();

        assert_eq!(fs::read(target.join("image.png")).unwrap(), b"image");
        assert!(!staging.exists());
        assert!(!metadata.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn completed_download_replaces_existing_target_without_losing_fallback() {
        let directory = test_directory("commit");
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("archive.tar");
        let staging = local_download_staging_path(&target);
        fs::write(&target, b"old").unwrap();
        fs::write(&staging, b"new").unwrap();

        commit_local_staging_path(&staging, &target).unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert!(!staging.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn download_ranges_cover_the_remaining_file_without_gaps() {
        let mib = 1024 * 1024;
        let ranges = download_chunk_ranges(2 * mib, 22 * mib, 4);

        assert_eq!(
            ranges,
            vec![
                (2 * mib, 5 * mib),
                (7 * mib, 5 * mib),
                (12 * mib, 5 * mib),
                (17 * mib, 5 * mib)
            ]
        );
    }

    #[test]
    fn download_range_progress_round_trips_without_chunk_files() {
        let directory = test_directory("range-progress");
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("archive.tar");
        let staging = local_download_staging_path(&target);
        let metadata_path = local_part_metadata_path(&staging);
        let mut metadata = prepare_local_download_staging(
            &staging,
            &metadata_path,
            "remote",
            20 * 1024 * 1024,
            7,
            4,
        )
        .unwrap();
        metadata.ranges[1].completed = 1024;
        write_local_download_metadata(&metadata_path, &metadata).unwrap();

        let encoded = fs::read_to_string(&metadata_path).unwrap();
        assert_eq!(parse_local_download_metadata(&encoded), Some(metadata));
        assert_eq!(fs::metadata(staging).unwrap().len(), 20 * 1024 * 1024);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn visible_download_progress_does_not_wait_for_metadata_checkpoints() {
        let directory = test_directory("visible-progress");
        fs::create_dir_all(&directory).unwrap();
        let metadata_path = directory.join("download.meta");
        let mut metadata = DownloadPartMetadata {
            source: "remote".to_owned(),
            size: 4096,
            modified: 0,
            ranges: vec![DownloadRangeState {
                start: 0,
                length: 4096,
                completed: 0,
            }],
        };
        let mut progress = SftpTaskProgress {
            callback: None,
            user_data: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            total_bytes: 4096,
            transferred_bytes: 0,
            concurrent: None,
        };

        apply_download_range_event(
            DownloadRangeEvent::Progress(1024),
            &metadata_path,
            &mut metadata,
            "remote",
            &mut progress,
        )
        .unwrap();
        assert_eq!(progress.transferred_bytes, 1024);
        assert_eq!(metadata.ranges[0].completed, 0);
        assert!(!metadata_path.exists());

        apply_download_range_event(
            DownloadRangeEvent::Durable {
                index: 0,
                completed: 1024,
            },
            &metadata_path,
            &mut metadata,
            "remote",
            &mut progress,
        )
        .unwrap();
        assert_eq!(metadata.ranges[0].completed, 1024);
        assert!(metadata_path.exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
