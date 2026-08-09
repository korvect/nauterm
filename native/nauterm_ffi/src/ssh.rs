// SSH setup and forwarding functions mirror protocol/FFI request fields. Keeping
// the arguments explicit avoids introducing partially initialized config bags at
// security-sensitive boundaries.
#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::fs::{self, OpenOptions};
use std::future::Future;
use std::io::{Read, Seek, SeekFrom, Write as _};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use encoding_rs::{Decoder, Encoding, UTF_8};
use russh::client::{self, AuthResult};
use russh::keys::agent::client::{AgentClient, AgentStream};
use russh::keys::known_hosts;
use russh::keys::{decode_secret_key, Error as KeyError, HashAlg, PrivateKeyWithHashAlg};
use russh::ChannelMsg;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use zeroize::Zeroize;

mod auth;
mod probe;
mod proxy;
mod sftp;

pub(crate) use auth::authenticate;
use probe::*;
pub(crate) use proxy::connect_ssh_tcp_stream;
#[cfg(test)]
use proxy::preferred_mosh_udp_host;
use proxy::{connect_ssh_with_timeout, connect_ssh_with_timeout_and_udp_host};
use sftp::{
    copy_sftp_path, delete_sftp_path, download_sftp_path, list_sftp_directory_entries,
    mkdir_sftp_path, move_sftp_path, open_sftp_session, open_sudo_sftp_session, upload_sftp_path,
};

use crate::output_queue::{append_output, clear_output, drain_output_chunk};
use crate::pty::WakeupCallback;
use crate::session::SessionEvent;
use crate::terminal::{TerminalGeometry, TerminalOptions};
use crate::{mosh::MoshBootstrap, mosh::MoshServerCommand};

const SSH_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(8);
const SSH_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const SSH_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const SSH_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(20);
const SSH_KEEPALIVE_MISSES: usize = 3;
const SUDO_SFTP_SESSION_LIFETIME: Duration = Duration::from_secs(15 * 60);
const SFTP_MAX_CHANNEL_WINDOW_SIZE: u32 = 16 * 1024 * 1024;

pub type ShellHistoryResult = Result<(Option<String>, String), String>;
pub type ShellHistoryReceiver = Receiver<ShellHistoryResult>;
const SSH_SESSION_CANCELLED: &str = "SSH session cancelled.";
const CELL_WIDTH_PIXELS: u32 = 8;
const CELL_HEIGHT_PIXELS: u32 = 16;

pub struct SshTransport {
    sender: Sender<SshCommand>,
    exited: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

pub struct SshPump {
    pub output: Vec<u8>,
    pub events: Vec<SessionEvent>,
    pub exited: bool,
    pub has_more: bool,
}

#[allow(dead_code)]
pub(crate) struct MoshSshBootstrap {
    pub bootstrap: MoshBootstrap,
    pub events: Vec<SessionEvent>,
    pub udp_host: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MoshUdpHostSource {
    ConnectedPeerIp,
    OriginalHost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedMoshUdpHost {
    host: String,
    source: MoshUdpHostSource,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SshProxyConfig {
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SshDirectoryEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
    pub modified: Option<u64>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SshDirectoryEntries {
    pub directory: String,
    pub entries: Vec<SshDirectoryEntry>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SftpDirectoryEntries {
    pub directory: String,
    pub entries: Vec<SshDirectoryEntry>,
    pub events: Vec<SessionEvent>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct HostOsDetection {
    pub os: Option<String>,
    pub distro: Option<String>,
    pub events: Vec<SessionEvent>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct HostSystemInfo {
    pub hostname: Option<String>,
    pub os_name: Option<String>,
    pub kernel: Option<String>,
    pub architecture: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub load_average: Option<f64>,
    pub load_average_5: Option<f64>,
    pub load_average_15: Option<f64>,
    pub cpu_count: Option<u32>,
    pub cpu_usage_percent: Option<f64>,
    pub memory_total_bytes: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub swap_total_bytes: Option<u64>,
    pub swap_used_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub disk_used_bytes: Option<u64>,
    pub latency_ms: Option<f64>,
    pub processes: Vec<HostProcessInfo>,
    pub network_interfaces: Vec<HostNetworkInterface>,
    pub filesystems: Vec<HostFilesystemInfo>,
    pub events: Vec<SessionEvent>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SshPublicKeyExportResult {
    pub ok: bool,
    pub events: Vec<SessionEvent>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct HostProcessInfo {
    pub memory_bytes: u64,
    pub cpu_usage_percent: f64,
    pub command: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct HostNetworkInterface {
    pub name: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct HostFilesystemInfo {
    pub path: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Clone, Debug)]
struct SshCommandOutput {
    stdout: String,
    exit_status: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostKeyTrustMode {
    Strict,
    AcceptAndSave,
    AcceptOnce,
}

impl HostKeyTrustMode {
    pub(crate) fn from_u32(value: u32) -> Self {
        match value {
            1 => Self::AcceptAndSave,
            2 => Self::AcceptOnce,
            _ => Self::Strict,
        }
    }
}

pub fn proxy_config_from_json_ptr(proxy_json: *const c_char) -> Option<SshProxyConfig> {
    if proxy_json.is_null() {
        return None;
    }
    let json = unsafe { std::ffi::CStr::from_ptr(proxy_json) }
        .to_string_lossy()
        .trim()
        .to_owned();
    if json.is_empty() {
        return None;
    }
    serde_json::from_str::<SshProxyConfig>(&json)
        .ok()
        .and_then(normalize_proxy_config)
}

fn normalize_proxy_config(mut proxy: SshProxyConfig) -> Option<SshProxyConfig> {
    proxy.proxy_type = proxy.proxy_type.trim().to_ascii_lowercase();
    proxy.host = proxy.host.trim().to_owned();
    proxy.username = proxy
        .username
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    proxy.password = proxy.password.filter(|value| !value.is_empty());
    if proxy.host.is_empty() || proxy.port == 0 {
        return None;
    }
    match proxy.proxy_type.as_str() {
        "http" | "socks5" => Some(proxy),
        _ => None,
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum SftpTaskRequest {
    Download {
        remote_path: String,
        local_path: String,
        #[serde(default = "default_sftp_transfer_threads")]
        transfer_threads: usize,
        #[serde(default)]
        sudo_password: Option<String>,
        #[serde(default)]
        sudo_session_id: Option<String>,
    },
    Upload {
        local_path: String,
        remote_path: String,
        #[serde(default = "default_sftp_transfer_threads")]
        transfer_threads: usize,
        #[serde(default)]
        replace_existing: bool,
        #[serde(default)]
        sudo_password: Option<String>,
        #[serde(default)]
        sudo_session_id: Option<String>,
    },
    CleanupUpload {
        remote_path: String,
        #[serde(default)]
        sudo_password: Option<String>,
        #[serde(default)]
        sudo_session_id: Option<String>,
    },
    Move {
        source_path: String,
        target_path: String,
        #[serde(default)]
        replace_existing: bool,
        #[serde(default)]
        sudo_password: Option<String>,
        #[serde(default)]
        sudo_session_id: Option<String>,
    },
    Copy {
        source_path: String,
        target_path: String,
        #[serde(default)]
        replace_existing: bool,
        #[serde(default)]
        sudo_password: Option<String>,
        #[serde(default)]
        sudo_session_id: Option<String>,
    },
    Mkdir {
        target_path: String,
        #[serde(default)]
        sudo_password: Option<String>,
        #[serde(default)]
        sudo_session_id: Option<String>,
    },
    Delete {
        target_path: String,
        #[serde(default)]
        sudo_password: Option<String>,
        #[serde(default)]
        sudo_session_id: Option<String>,
    },
}

impl SftpTaskRequest {
    fn transfer_threads(&self) -> usize {
        match self {
            Self::Download {
                transfer_threads, ..
            }
            | Self::Upload {
                transfer_threads, ..
            } => (*transfer_threads).clamp(1, 32),
            _ => default_sftp_transfer_threads(),
        }
    }

    fn take_sudo_password(&mut self) -> Option<String> {
        match self {
            Self::Download { sudo_password, .. }
            | Self::Upload { sudo_password, .. }
            | Self::CleanupUpload { sudo_password, .. }
            | Self::Move { sudo_password, .. }
            | Self::Copy { sudo_password, .. }
            | Self::Mkdir { sudo_password, .. }
            | Self::Delete { sudo_password, .. } => sudo_password.take(),
        }
    }

    fn take_sudo_session_id(&mut self) -> Option<String> {
        match self {
            Self::Download {
                sudo_session_id, ..
            }
            | Self::Upload {
                sudo_session_id, ..
            }
            | Self::CleanupUpload {
                sudo_session_id, ..
            }
            | Self::Move {
                sudo_session_id, ..
            }
            | Self::Copy {
                sudo_session_id, ..
            }
            | Self::Mkdir {
                sudo_session_id, ..
            }
            | Self::Delete {
                sudo_session_id, ..
            } => sudo_session_id.take(),
        }
    }
}

const fn default_sftp_transfer_threads() -> usize {
    8
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SftpTaskResult {
    pub ok: bool,
    pub bytes: u64,
    pub item_kind: String,
    pub events: Vec<SessionEvent>,
    pub error: Option<String>,
}

pub type SftpTaskProgressCallback = Option<extern "C" fn(*mut c_void, u64, u64, *const c_char)>;

struct SftpTaskProgress {
    callback: SftpTaskProgressCallback,
    user_data: usize,
    cancel: Arc<AtomicBool>,
    total_bytes: u64,
    transferred_bytes: u64,
    concurrent: Option<SftpConcurrentProgress>,
}

#[derive(Clone)]
struct SftpConcurrentProgress {
    callback: SftpTaskProgressCallback,
    user_data: usize,
    total_bytes: u64,
    transferred_bytes: Arc<AtomicU64>,
}

static SFTP_TASK_CANCEL_FLAGS: OnceLock<Mutex<HashMap<u64, Arc<AtomicBool>>>> = OnceLock::new();

fn sftp_task_cancel_flags() -> &'static Mutex<HashMap<u64, Arc<AtomicBool>>> {
    SFTP_TASK_CANCEL_FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct CachedSudoSftpSession {
    handle: client::Handle<SshClientHandler>,
    sftp: russh_sftp::client::SftpSession,
    authenticated_at: Instant,
}

type CachedSudoSftpSessionHandle = Arc<tokio::sync::Mutex<CachedSudoSftpSession>>;

static SUDO_SFTP_RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
static SUDO_SFTP_SESSIONS: OnceLock<
    tokio::sync::Mutex<HashMap<String, CachedSudoSftpSessionHandle>>,
> = OnceLock::new();

fn sudo_sftp_runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    match SUDO_SFTP_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|error| format!("failed to start sudo SFTP runtime: {error}"))
    }) {
        Ok(runtime) => Ok(runtime),
        Err(error) => Err(error.clone()),
    }
}

fn sudo_sftp_sessions() -> &'static tokio::sync::Mutex<HashMap<String, CachedSudoSftpSessionHandle>>
{
    SUDO_SFTP_SESSIONS.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

pub fn cancel_sftp_task(task_id: u64) -> bool {
    let Ok(mut flags) = sftp_task_cancel_flags().lock() else {
        return false;
    };
    let flag = flags
        .entry(task_id)
        .or_insert_with(|| Arc::new(AtomicBool::new(false)))
        .clone();
    flag.store(true, Ordering::SeqCst);
    true
}

pub fn cancel_all_sftp_tasks() {
    if let Ok(flags) = sftp_task_cancel_flags().lock() {
        for flag in flags.values() {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

fn sftp_task_cancel_flag(task_id: u64) -> Arc<AtomicBool> {
    let Ok(mut flags) = sftp_task_cancel_flags().lock() else {
        return Arc::new(AtomicBool::new(false));
    };
    flags
        .entry(task_id)
        .or_insert_with(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

fn finish_sftp_task(task_id: u64) {
    if let Ok(mut flags) = sftp_task_cancel_flags().lock() {
        flags.remove(&task_id);
    }
}

pub fn list_directory_entries_blocking(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    directory: &str,
) -> Result<SshDirectoryEntries, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to start SSH completion runtime: {error}"))?;
    runtime.block_on(list_directory_entries(
        host,
        port,
        username,
        password,
        private_key,
        passphrase,
        known_hosts_path,
        directory,
    ))
}

pub fn detect_host_os_blocking_with_trust(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<SshProxyConfig>,
    host_key_trust_mode: HostKeyTrustMode,
) -> HostOsDetection {
    let runtime_result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(runtime) = runtime_result else {
        return HostOsDetection {
            os: None,
            distro: None,
            events: Vec::new(),
            error: Some(format!(
                "failed to start host OS detection runtime: {}",
                runtime_result.err().unwrap()
            )),
        };
    };
    runtime.block_on(detect_host_os(
        host,
        port,
        username,
        password,
        private_key,
        passphrase,
        known_hosts_path,
        proxy.as_ref(),
        host_key_trust_mode,
    ))
}

pub fn collect_host_system_info_blocking_with_trust(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<SshProxyConfig>,
    host_key_trust_mode: HostKeyTrustMode,
) -> HostSystemInfo {
    let runtime_result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(runtime) = runtime_result else {
        return HostSystemInfo {
            error: Some(format!(
                "failed to start system information runtime: {}",
                runtime_result.err().unwrap()
            )),
            ..HostSystemInfo::default()
        };
    };
    runtime.block_on(collect_host_system_info(
        host,
        port,
        username,
        password,
        private_key,
        passphrase,
        known_hosts_path,
        proxy.as_ref(),
        host_key_trust_mode,
    ))
}

pub fn export_public_key_blocking_with_trust(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<SshProxyConfig>,
    public_key: &str,
    location: &str,
    filename: &str,
    script: &str,
    host_key_trust_mode: HostKeyTrustMode,
) -> SshPublicKeyExportResult {
    let runtime_result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(runtime) = runtime_result else {
        return SshPublicKeyExportResult {
            ok: false,
            events: Vec::new(),
            error: Some(format!(
                "failed to start SSH key export runtime: {}",
                runtime_result.err().unwrap()
            )),
        };
    };
    runtime.block_on(export_public_key(
        host,
        port,
        username,
        password,
        private_key,
        passphrase,
        known_hosts_path,
        proxy.as_ref(),
        public_key,
        location,
        filename,
        script,
        host_key_trust_mode,
    ))
}

pub fn list_sftp_directory_entries_blocking_with_trust(
    request_id: u64,
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<SshProxyConfig>,
    directory: &str,
    host_key_trust_mode: HostKeyTrustMode,
) -> SftpDirectoryEntries {
    let cancel = sftp_task_cancel_flag(request_id);
    let runtime_result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(runtime) = runtime_result else {
        finish_sftp_task(request_id);
        return SftpDirectoryEntries {
            directory: directory.to_owned(),
            entries: Vec::new(),
            events: Vec::new(),
            error: Some(format!(
                "failed to start SFTP runtime: {}",
                runtime_result.err().unwrap()
            )),
        };
    };
    let result = runtime.block_on(list_sftp_directory_entries(
        host,
        port,
        username,
        password,
        private_key,
        passphrase,
        known_hosts_path,
        proxy.as_ref(),
        directory,
        host_key_trust_mode,
        cancel,
    ));
    finish_sftp_task(request_id);
    result
}

pub fn execute_sftp_task_blocking_with_trust(
    task_id: u64,
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<SshProxyConfig>,
    operation_json: &str,
    host_key_trust_mode: HostKeyTrustMode,
    progress_callback: SftpTaskProgressCallback,
    progress_user_data: *mut c_void,
) -> SftpTaskResult {
    let mut request = match serde_json::from_str::<SftpTaskRequest>(operation_json) {
        Ok(request) => request,
        Err(error) => {
            return SftpTaskResult {
                ok: false,
                bytes: 0,
                item_kind: "unknown".to_owned(),
                events: Vec::new(),
                error: Some(format!("invalid SFTP task: {error}")),
            };
        }
    };
    if let Some(sudo_session_id) = request.take_sudo_session_id() {
        let sudo_password = request.take_sudo_password();
        let result = execute_cached_sudo_sftp_task_blocking(
            host,
            port,
            username,
            password,
            private_key,
            passphrase,
            known_hosts_path,
            proxy,
            request,
            host_key_trust_mode,
            sudo_session_id,
            sudo_password,
            SftpTaskProgress {
                callback: progress_callback,
                user_data: progress_user_data as usize,
                cancel: sftp_task_cancel_flag(task_id),
                total_bytes: 0,
                transferred_bytes: 0,
                concurrent: None,
            },
        );
        finish_sftp_task(task_id);
        return result;
    }
    let runtime_result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(runtime) = runtime_result else {
        return SftpTaskResult {
            ok: false,
            bytes: 0,
            item_kind: "unknown".to_owned(),
            events: Vec::new(),
            error: Some(format!(
                "failed to start SFTP task runtime: {}",
                runtime_result.err().unwrap()
            )),
        };
    };
    let cancel = sftp_task_cancel_flag(task_id);
    let result = runtime.block_on(execute_sftp_task(
        host,
        port,
        username,
        password,
        private_key,
        passphrase,
        known_hosts_path,
        proxy,
        request,
        host_key_trust_mode,
        SftpTaskProgress {
            callback: progress_callback,
            user_data: progress_user_data as usize,
            cancel,
            total_bytes: 0,
            transferred_bytes: 0,
            concurrent: None,
        },
    ));
    finish_sftp_task(task_id);
    result
}

fn execute_cached_sudo_sftp_task_blocking(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<SshProxyConfig>,
    request: SftpTaskRequest,
    host_key_trust_mode: HostKeyTrustMode,
    sudo_session_id: String,
    mut sudo_password: Option<String>,
    mut progress: SftpTaskProgress,
) -> SftpTaskResult {
    let transfer_threads = request.transfer_threads();
    let runtime = match sudo_sftp_runtime() {
        Ok(runtime) => runtime,
        Err(error) => {
            if let Some(password) = sudo_password.as_mut() {
                password.zeroize();
            }
            return SftpTaskResult {
                ok: false,
                bytes: 0,
                item_kind: "unknown".to_owned(),
                events: Vec::new(),
                error: Some(error),
            };
        }
    };
    runtime.block_on(async {
        let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
        let mut cached = {
            let sessions = sudo_sftp_sessions().lock().await;
            sessions.get(&sudo_session_id).cloned()
        };
        if let Some(session) = cached.as_ref() {
            let expired =
                session.lock().await.authenticated_at.elapsed() >= SUDO_SFTP_SESSION_LIFETIME;
            if expired {
                let removed = {
                    let mut sessions = sudo_sftp_sessions().lock().await;
                    if sessions
                        .get(&sudo_session_id)
                        .is_some_and(|current| Arc::ptr_eq(current, session))
                    {
                        sessions.remove(&sudo_session_id)
                    } else {
                        None
                    }
                };
                if let Some(expired_session) = removed {
                    close_cached_sudo_sftp_session(expired_session).await;
                }
                cached = None;
                if sudo_password.is_none() {
                    return SftpTaskResult {
                        ok: false,
                        bytes: 0,
                        item_kind: "unknown".to_owned(),
                        events: Vec::new(),
                        error: Some("sudo SFTP session expired after 15 minutes".to_owned()),
                    };
                }
            }
        }
        let session = match cached {
            Some(session) => session,
            None => {
                let Some(sudo_password_value) = sudo_password.as_deref() else {
                    return SftpTaskResult {
                        ok: false,
                        bytes: 0,
                        item_kind: "unknown".to_owned(),
                        events: Vec::new(),
                        error: Some("sudo SFTP session is not authenticated".to_owned()),
                    };
                };
                let opened = open_sudo_sftp_session(
                    host,
                    port,
                    username,
                    password,
                    private_key,
                    passphrase,
                    known_hosts_path,
                    proxy.as_ref(),
                    host_key_trust_mode,
                    events.clone(),
                    sudo_password_value,
                    transfer_threads,
                )
                .await;
                if let Some(password) = sudo_password.as_mut() {
                    password.zeroize();
                }
                let (handle, sftp) = match opened {
                    Ok(session) => session,
                    Err(error) => {
                        return SftpTaskResult {
                            ok: false,
                            bytes: 0,
                            item_kind: "unknown".to_owned(),
                            events: events
                                .lock()
                                .map(|events| events.clone())
                                .unwrap_or_default(),
                            error: Some(error),
                        };
                    }
                };
                let session = Arc::new(tokio::sync::Mutex::new(CachedSudoSftpSession {
                    handle,
                    sftp,
                    authenticated_at: Instant::now(),
                }));
                sudo_sftp_sessions()
                    .lock()
                    .await
                    .insert(sudo_session_id.clone(), session.clone());
                session
            }
        };
        if let Some(password) = sudo_password.as_mut() {
            password.zeroize();
        }
        let result = {
            let session = session.lock().await;
            execute_sftp_request(&session.sftp, request, &mut progress).await
        };
        let result = match result {
            Err(error) if sudo_sftp_session_transport_failed(&error) => {
                let mut sessions = sudo_sftp_sessions().lock().await;
                if sessions
                    .get(&sudo_session_id)
                    .is_some_and(|cached| Arc::ptr_eq(cached, &session))
                {
                    sessions.remove(&sudo_session_id);
                }
                Err(format!("sudo SFTP session expired: {error}"))
            }
            result => result,
        };
        let captured_events = events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default();
        match result {
            Ok((bytes, item_kind)) => SftpTaskResult {
                ok: true,
                bytes,
                item_kind,
                events: captured_events,
                error: None,
            },
            Err(error) => SftpTaskResult {
                ok: false,
                bytes: 0,
                item_kind: "unknown".to_owned(),
                events: captured_events,
                error: Some(error),
            },
        }
    })
}

fn sudo_sftp_session_transport_failed(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("channel closed")
        || normalized.contains("connection closed")
        || normalized.contains("connection reset")
        || normalized.contains("broken pipe")
        || normalized.contains("unexpected eof")
        || normalized.contains("transport error")
}

pub fn close_sudo_sftp_session(session_id: &str) -> bool {
    if session_id.trim().is_empty() {
        return false;
    }
    let Ok(runtime) = sudo_sftp_runtime() else {
        return false;
    };
    runtime.block_on(async {
        let session = sudo_sftp_sessions().lock().await.remove(session_id);
        let Some(session) = session else {
            return false;
        };
        close_cached_sudo_sftp_session(session).await;
        true
    })
}

async fn close_cached_sudo_sftp_session(session: CachedSudoSftpSessionHandle) {
    let session = session.lock().await;
    let _ = session.sftp.close().await;
    let _ = session
        .handle
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await;
}

async fn execute_sftp_request(
    sftp: &russh_sftp::client::SftpSession,
    request: SftpTaskRequest,
    progress: &mut SftpTaskProgress,
) -> Result<(u64, String), String> {
    match request {
        SftpTaskRequest::Download {
            remote_path,
            local_path,
            transfer_threads,
            ..
        } => {
            download_sftp_path(
                sftp,
                &remote_path,
                Path::new(&local_path),
                transfer_threads.clamp(1, 32),
                progress,
            )
            .await
        }
        SftpTaskRequest::Upload {
            local_path,
            remote_path,
            transfer_threads,
            replace_existing,
            ..
        } => {
            upload_sftp_path(
                sftp,
                Path::new(&local_path),
                &remote_path,
                replace_existing,
                transfer_threads.clamp(1, 32),
                progress,
            )
            .await
        }
        SftpTaskRequest::CleanupUpload { remote_path, .. } => {
            sftp::cleanup_sftp_upload_parts(sftp, &remote_path).await;
            Ok((0, "unknown".to_owned()))
        }
        SftpTaskRequest::Move {
            source_path,
            target_path,
            replace_existing,
            ..
        } => move_sftp_path(sftp, &source_path, &target_path, replace_existing, progress).await,
        SftpTaskRequest::Copy {
            source_path,
            target_path,
            replace_existing,
            ..
        } => copy_sftp_path(sftp, &source_path, &target_path, replace_existing, progress).await,
        SftpTaskRequest::Mkdir { target_path, .. } => {
            mkdir_sftp_path(sftp, &target_path, progress).await
        }
        SftpTaskRequest::Delete { target_path, .. } => {
            delete_sftp_path(sftp, &target_path, progress).await
        }
    }
}

async fn execute_sftp_task(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<SshProxyConfig>,
    mut request: SftpTaskRequest,
    host_key_trust_mode: HostKeyTrustMode,
    mut progress: SftpTaskProgress,
) -> SftpTaskResult {
    let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
    let result = async {
        let transfer_threads = request.transfer_threads();
        let mut sudo_password = request.take_sudo_password();
        let session = match sudo_password.as_deref() {
            Some(sudo_password) => {
                open_sudo_sftp_session(
                    host,
                    port,
                    username,
                    password,
                    private_key,
                    passphrase,
                    known_hosts_path,
                    proxy.as_ref(),
                    host_key_trust_mode,
                    events.clone(),
                    sudo_password,
                    transfer_threads,
                )
                .await
            }
            None => {
                open_sftp_session(
                    host,
                    port,
                    username,
                    password,
                    private_key,
                    passphrase,
                    known_hosts_path,
                    proxy.as_ref(),
                    host_key_trust_mode,
                    events.clone(),
                    transfer_threads,
                )
                .await
            }
        };
        if let Some(password) = sudo_password.as_mut() {
            password.zeroize();
        }
        let (handle, sftp) = session?;
        let result = execute_sftp_request(&sftp, request, &mut progress).await;
        let _ = sftp.close().await;
        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        result
    }
    .await;
    let captured_events = events
        .lock()
        .map(|events| events.clone())
        .unwrap_or_default();
    match result {
        Ok((bytes, item_kind)) => SftpTaskResult {
            ok: true,
            bytes,
            item_kind,
            events: captured_events,
            error: None,
        },
        Err(error) => SftpTaskResult {
            ok: false,
            bytes: 0,
            item_kind: "unknown".to_owned(),
            events: captured_events,
            error: Some(error),
        },
    }
}

fn ssh_client_config() -> Arc<client::Config> {
    ssh_client_config_with_keepalive(SSH_KEEPALIVE_INTERVAL.as_secs() as u32)
}

fn ssh_client_config_for_sftp(transfer_concurrency: usize) -> Arc<client::Config> {
    let mut config = client::Config {
        nodelay: true,
        keepalive_interval: Some(SSH_KEEPALIVE_INTERVAL),
        keepalive_max: SSH_KEEPALIVE_MISSES,
        ..client::Config::default()
    };
    let desired_window = transfer_concurrency
        .clamp(1, 32)
        .saturating_mul(2 * 256 * 1024)
        .min(SFTP_MAX_CHANNEL_WINDOW_SIZE as usize);
    config.window_size = config.window_size.max(desired_window as u32);
    Arc::new(config)
}

fn ssh_client_config_with_keepalive(interval_seconds: u32) -> Arc<client::Config> {
    Arc::new(client::Config {
        nodelay: true,
        keepalive_interval: (interval_seconds > 0)
            .then(|| Duration::from_secs(u64::from(interval_seconds))),
        keepalive_max: SSH_KEEPALIVE_MISSES,
        ..client::Config::default()
    })
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) async fn start_mosh_server(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    proxy: Option<&SshProxyConfig>,
    host_key_trust_mode: HostKeyTrustMode,
    server_command: &MoshServerCommand,
    terminal_options: &TerminalOptions,
) -> Result<MoshSshBootstrap, (String, Vec<SessionEvent>)> {
    let config = ssh_client_config();
    let events = Arc::new(Mutex::new(vec![SessionEvent::new(
        "connect_start",
        format!("Connecting to {username}@{host}:{port}."),
    )
    .with_host_port(host, port)
    .with_username(username)]));
    let output = Arc::new(Mutex::new(Vec::new()));
    let wakeup = Arc::new(Mutex::new(None));
    let handler = SshClientHandler {
        host: host.to_owned(),
        port,
        known_hosts_path: known_hosts_path.map(PathBuf::from),
        host_key_trust_mode,
        output,
        events: events.clone(),
        wakeup: wakeup.clone(),
    };
    let result = async {
        let (mut handle, udp_host) =
            connect_ssh_with_timeout_and_udp_host(config, host, port, proxy, handler).await?;
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

        let command = server_command
            .render_for_terminal(terminal_options)
            .map_err(|error| error.to_string())?;
        let command_result = run_ssh_exec_command(&mut handle, &command).await;
        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        let command_result = command_result?;
        if command_result.exit_status != 0 {
            return Err(format!(
                "mosh-server exited with status {}.",
                command_result.exit_status
            ));
        }
        let mut bootstrap_output = command_result.stdout;
        let bootstrap_result =
            MoshBootstrap::parse(&bootstrap_output).map_err(|error| error.to_string());
        bootstrap_output.zeroize();
        let bootstrap = bootstrap_result?;
        push_event(
            &events,
            &wakeup,
            SessionEvent::new(
                "mosh_bootstrap_ready",
                format!(
                    "Mosh UDP transport is ready on {}:{} ({}).",
                    udp_host.host,
                    bootstrap.port,
                    match udp_host.source {
                        MoshUdpHostSource::ConnectedPeerIp => "using the SSH connected peer IP",
                        MoshUdpHostSource::OriginalHost => "using the original host target",
                    }
                ),
            )
            .with_host_port(&udp_host.host, bootstrap.port)
            .with_username(username),
        );
        Ok((bootstrap, udp_host))
    }
    .await;
    match result {
        Ok((bootstrap, udp_host)) => Ok(MoshSshBootstrap {
            bootstrap,
            udp_host: udp_host.host,
            events: events
                .lock()
                .map(|events| events.clone())
                .unwrap_or_default(),
        }),
        Err(error) => {
            push_event(
                &events,
                &wakeup,
                SessionEvent::new("error", error.clone())
                    .with_host_port(host, port)
                    .with_username(username),
            );
            let captured = events
                .lock()
                .map(|events| events.clone())
                .unwrap_or_default();
            Err((error, captured))
        }
    }
}

async fn list_directory_entries(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    passphrase: Option<&str>,
    known_hosts_path: Option<&str>,
    directory: &str,
) -> Result<SshDirectoryEntries, String> {
    let config = ssh_client_config();
    let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
    let output = Arc::new(Mutex::new(Vec::new()));
    let wakeup = Arc::new(Mutex::new(None));
    let handler = SshClientHandler {
        host: host.to_owned(),
        port,
        known_hosts_path: known_hosts_path.map(PathBuf::from),
        host_key_trust_mode: HostKeyTrustMode::Strict,
        output: output.clone(),
        events: events.clone(),
        wakeup: wakeup.clone(),
    };
    let mut handle = connect_ssh_with_timeout(config, host, port, None, handler).await?;

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

    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|error| format!("failed to open completion channel: {error}"))?;
    let command = directory_entries_command(directory);
    channel
        .exec(false, command)
        .await
        .map_err(|error| format!("failed to request completion command: {error}"))?;

    let mut bytes = Vec::new();
    let mut exit_status = 0;
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                bytes.extend_from_slice(&data);
            }
            ChannelMsg::ExitStatus {
                exit_status: status,
            } => {
                exit_status = status;
            }
            ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }
    let _ = channel.close().await;

    if exit_status != 0 {
        return Err(format!(
            "remote directory listing exited with status {exit_status}"
        ));
    }

    let mut resolved_directory = directory.to_owned();
    let mut entries = Vec::new();
    for record in bytes.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let Ok(record) = String::from_utf8(record.to_vec()) else {
            continue;
        };
        let Some((kind, name)) = record.split_once('\t') else {
            continue;
        };
        if kind == "p" {
            if !name.is_empty() {
                resolved_directory = name.to_owned();
            }
            continue;
        }
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }
        entries.push(SshDirectoryEntry {
            name: name.to_owned(),
            is_directory: kind == "d",
            size: 0,
            modified: None,
        });
    }
    entries.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| b.is_directory.cmp(&a.is_directory))
    });
    entries.dedup_by(|a, b| a.name == b.name && a.is_directory == b.is_directory);
    Ok(SshDirectoryEntries {
        directory: resolved_directory,
        entries,
    })
}

fn directory_entries_command(directory: &str) -> String {
    format!(
        "dir={}; cd -- \"$dir\" || exit; resolved=$(pwd -P) || exit; printf 'p\\t%s\\0' \"$resolved\"; for p in ./* ./.[!.]* ./..?*; do [ -e \"$p\" ] || continue; name=${{p##*/}}; [ \"$name\" = . ] && continue; [ \"$name\" = .. ] && continue; if [ -d \"$p\" ]; then kind=d; else kind=f; fi; printf '%s\\t%s\\0' \"$kind\" \"$name\"; done",
        remote_directory_value(directory)
    )
}

fn remote_directory_value(directory: &str) -> String {
    if directory == "~" {
        return "$HOME".to_owned();
    }
    if let Some(rest) = directory.strip_prefix("~/") {
        return format!("$HOME/{}", shell_quote(rest));
    }
    shell_quote(directory)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

enum SshCommand {
    Input(Vec<u8>),
    Resize(TerminalGeometry),
    ReadShellHistory(Sender<ShellHistoryResult>),
    Shutdown,
}

struct SshTarget {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    private_key: Option<String>,
    passphrase: Option<String>,
    known_hosts_path: Option<PathBuf>,
    host_key_trust_mode: HostKeyTrustMode,
    proxy: Option<SshProxyConfig>,
    geometry: TerminalGeometry,
    terminal_options: TerminalOptions,
    ssh_keepalive_interval_seconds: u32,
    encoding: String,
}

#[derive(Clone)]
struct SshClientHandler {
    host: String,
    port: u16,
    known_hosts_path: Option<PathBuf>,
    host_key_trust_mode: HostKeyTrustMode,
    output: Arc<Mutex<Vec<u8>>>,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
}

impl SshTransport {
    pub fn connect(
        geometry: TerminalGeometry,
        terminal_options: TerminalOptions,
        host: &str,
        port: u16,
        username: &str,
        password: Option<&str>,
        private_key: Option<&str>,
        passphrase: Option<&str>,
        known_hosts_path: Option<&str>,
        host_key_trust_mode: HostKeyTrustMode,
        proxy: Option<SshProxyConfig>,
        ssh_keepalive_interval_seconds: u32,
        encoding: &str,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let exited = Arc::new(AtomicBool::new(false));
        let output = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let wakeup = Arc::new(Mutex::new(None));
        let shutdown = Arc::new(AtomicBool::new(false));
        let target = SshTarget {
            host: host.to_owned(),
            port,
            username: username.to_owned(),
            password: password.map(str::to_owned),
            private_key: private_key.map(str::to_owned),
            passphrase: passphrase.map(str::to_owned),
            known_hosts_path: known_hosts_path.map(PathBuf::from),
            host_key_trust_mode,
            proxy,
            geometry,
            terminal_options,
            ssh_keepalive_interval_seconds,
            encoding: encoding.to_owned(),
        };
        let worker = spawn_ssh_worker(
            target,
            receiver,
            exited.clone(),
            output.clone(),
            events.clone(),
            wakeup.clone(),
            shutdown.clone(),
        )?;

        Ok(Self {
            sender,
            exited,
            output,
            events,
            wakeup,
            shutdown,
            worker: Some(worker),
        })
    }

    pub fn resize(&mut self, geometry: TerminalGeometry) {
        if self.exited.load(Ordering::Acquire) {
            return;
        }
        let _ = self.sender.send(SshCommand::Resize(geometry));
    }

    pub fn queue_input(&mut self, bytes: &[u8]) -> bool {
        if self.exited.load(Ordering::Acquire) {
            return false;
        }
        self.sender.send(SshCommand::Input(bytes.to_vec())).is_ok()
    }

    pub fn request_shell_history(&self) -> Result<ShellHistoryReceiver, String> {
        if self.exited.load(Ordering::Acquire) {
            return Err("SSH session has exited.".to_owned());
        }
        let (reply, receiver) = mpsc::channel();
        self.sender
            .send(SshCommand::ReadShellHistory(reply))
            .map_err(|_| "SSH session is unavailable.".to_owned())?;
        Ok(receiver)
    }

    pub fn set_wakeup_callback(&mut self, callback: Option<WakeupCallback>) {
        if let Ok(mut wakeup) = self.wakeup.lock() {
            *wakeup = callback;
        }
        notify_wakeup(&self.wakeup);
    }

    pub fn drain_output(&mut self) -> SshPump {
        let (output, has_more) = drain_output_chunk(&self.output);
        if has_more {
            notify_wakeup(&self.wakeup);
        }
        let events = self
            .events
            .lock()
            .map(|mut events| events.drain(..).collect())
            .unwrap_or_default();
        let exited = self.exited.load(Ordering::Acquire);

        SshPump {
            output,
            events,
            exited,
            has_more,
        }
    }

    pub fn clear_pending_output(&mut self) {
        clear_output(&self.output);
    }
}

impl Drop for SshTransport {
    fn drop(&mut self) {
        if let Ok(mut wakeup) = self.wakeup.lock() {
            *wakeup = None;
        }
        self.shutdown.store(true, Ordering::Release);
        let _ = self.sender.send(SshCommand::Shutdown);
        crate::pty::join_worker(&mut self.worker, "SSH");
    }
}

impl client::Handler for SshClientHandler {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> impl std::future::Future<Output = Result<bool, Self::Error>> + Send {
        let host = self.host.clone();
        let port = self.port;
        let known_hosts_path = self.known_hosts_path.clone();
        let host_key_trust_mode = self.host_key_trust_mode;
        let events = self.events.clone();
        let wakeup = self.wakeup.clone();
        let server_public_key = server_public_key.clone();

        async move {
            let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();
            push_event(
                &events,
                &wakeup,
                SessionEvent::new(
                    "known_host_check",
                    format!("Checking host key for {host}:{port}."),
                )
                .with_host_port(&host, port)
                .with_fingerprint(&fingerprint),
            );

            let Some(known_hosts_path) = known_hosts_path else {
                push_event(
                    &events,
                    &wakeup,
                    SessionEvent::new(
                        "known_host_store_missing",
                        "No known_hosts store is configured for this connection.",
                    )
                    .with_host_port(&host, port)
                    .with_fingerprint(&fingerprint),
                );
                return Ok(false);
            };

            match known_hosts::check_known_hosts_path(
                &host,
                port,
                &server_public_key,
                &known_hosts_path,
            ) {
                Ok(true) => {
                    push_event(
                        &events,
                        &wakeup,
                        SessionEvent::new(
                            "known_host_verified",
                            format!("Known host verified for {host}:{port}."),
                        )
                        .with_host_port(&host, port)
                        .with_fingerprint(&fingerprint),
                    );
                    Ok(true)
                }
                Ok(false) => match host_key_trust_mode {
                    HostKeyTrustMode::Strict => {
                        push_event(
                            &events,
                            &wakeup,
                            SessionEvent::new(
                                "host_key_unknown",
                                format!("Host key for {host}:{port} is not trusted yet."),
                            )
                            .with_host_port(&host, port)
                            .with_fingerprint(&fingerprint),
                        );
                        Ok(false)
                    }
                    HostKeyTrustMode::AcceptOnce => {
                        push_event(
                            &events,
                            &wakeup,
                            SessionEvent::new(
                                "host_key_accepted_for_session",
                                format!(
                                    "Trusted host key for {host}:{port} for this session only."
                                ),
                            )
                            .with_host_port(&host, port)
                            .with_fingerprint(&fingerprint),
                        );
                        Ok(true)
                    }
                    HostKeyTrustMode::AcceptAndSave => {
                        match learn_known_hosts_path_without_leading_blank(
                            &host,
                            port,
                            &server_public_key,
                            &known_hosts_path,
                        ) {
                            Ok(()) => {
                                push_event(
                                    &events,
                                    &wakeup,
                                    SessionEvent::new(
                                        "host_key_accepted",
                                        format!("Trusted and saved host key for {host}:{port}."),
                                    )
                                    .with_host_port(&host, port)
                                    .with_fingerprint(&fingerprint),
                                );
                                Ok(true)
                            }
                            Err(error) => {
                                push_event(
                                    &events,
                                    &wakeup,
                                    SessionEvent::new(
                                        "host_key_save_failed",
                                        format!(
                                            "Failed to save host key for {host}:{port}: {error}"
                                        ),
                                    )
                                    .with_host_port(&host, port)
                                    .with_fingerprint(&fingerprint),
                                );
                                Ok(false)
                            }
                        }
                    }
                },
                Err(error) => {
                    let kind = match &error {
                        KeyError::KeyChanged { .. } => "host_key_changed",
                        _ => "host_key_rejected",
                    };
                    push_event(
                        &events,
                        &wakeup,
                        SessionEvent::new(
                            kind,
                            format!("Known hosts rejected {host}:{port}: {error}"),
                        )
                        .with_host_port(&host, port)
                        .with_fingerprint(&fingerprint),
                    );
                    Ok(false)
                }
            }
        }
    }

    fn auth_banner(
        &mut self,
        banner: &str,
        _session: &mut client::Session,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send {
        let output = self.output.clone();
        let wakeup = self.wakeup.clone();
        let banner = banner.as_bytes().to_vec();
        async move {
            push_output(&output, &wakeup, &banner);
            Ok(())
        }
    }
}

fn spawn_ssh_worker(
    target: SshTarget,
    receiver: Receiver<SshCommand>,
    exited: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
    shutdown: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("nauterm-ssh".to_owned())
        .spawn(move || run_ssh_worker(target, receiver, exited, output, events, wakeup, shutdown))
}

fn run_ssh_worker(
    target: SshTarget,
    receiver: Receiver<SshCommand>,
    exited: Arc<AtomicBool>,
    output: Arc<Mutex<Vec<u8>>>,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
    shutdown: Arc<AtomicBool>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            push_event(
                &events,
                &wakeup,
                SessionEvent::new("error", format!("Failed to start SSH runtime: {error}")),
            );
            exited.store(true, Ordering::Release);
            notify_wakeup(&wakeup);
            return;
        }
    };

    runtime.block_on(async {
        if let Err(error) = run_ssh_session(
            target,
            receiver,
            output.clone(),
            events.clone(),
            wakeup.clone(),
            shutdown.clone(),
        )
        .await
        {
            if error != SSH_SESSION_CANCELLED {
                push_event(
                    &events,
                    &wakeup,
                    SessionEvent::new("error", format!("SSH session ended: {error}")),
                );
            }
        }
    });

    exited.store(true, Ordering::Release);
    notify_wakeup(&wakeup);
}

async fn run_ssh_session(
    target: SshTarget,
    receiver: Receiver<SshCommand>,
    output: Arc<Mutex<Vec<u8>>>,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: Arc<Mutex<Option<WakeupCallback>>>,
    shutdown: Arc<AtomicBool>,
) -> Result<(), String> {
    push_event(
        &events,
        &wakeup,
        SessionEvent::new(
            "connect_start",
            format!(
                "Connecting to {}@{}:{}.",
                target.username, target.host, target.port
            ),
        )
        .with_host_port(&target.host, target.port)
        .with_username(&target.username),
    );
    let config = ssh_client_config_with_keepalive(target.ssh_keepalive_interval_seconds);
    let handler = SshClientHandler {
        host: target.host.clone(),
        port: target.port,
        known_hosts_path: target.known_hosts_path.clone(),
        host_key_trust_mode: target.host_key_trust_mode,
        output: output.clone(),
        events: events.clone(),
        wakeup: wakeup.clone(),
    };
    let mut handle = cancel_on_ssh_shutdown(
        &receiver,
        &shutdown,
        connect_ssh_with_timeout(
            config,
            target.host.as_str(),
            target.port,
            target.proxy.as_ref(),
            handler,
        ),
    )
    .await?;

    match cancel_on_ssh_shutdown(
        &receiver,
        &shutdown,
        authenticate(
            &mut handle,
            &target.username,
            target.private_key.as_deref(),
            target.passphrase.as_deref(),
            target.password.as_deref(),
            &events,
            &wakeup,
        ),
    )
    .await
    {
        Ok(()) => {}
        Err(error) if error == SSH_SESSION_CANCELLED => return Err(error),
        Err(error) => return Err(format!("authentication failed: {error}")),
    }

    let remote_shell = match cancel_on_ssh_shutdown(
        &receiver,
        &shutdown,
        detect_remote_interactive_shell(&mut handle),
    )
    .await
    {
        Ok(shell) => shell,
        Err(error) if error == SSH_SESSION_CANCELLED => return Err(error),
        Err(_) => None,
    };

    let mut channel = cancel_on_ssh_shutdown(&receiver, &shutdown, async {
        handle
            .channel_open_session()
            .await
            .map_err(|error| format!("failed to open session channel: {error}"))
    })
    .await?;
    let geometry = target.geometry;
    cancel_on_ssh_shutdown(&receiver, &shutdown, async {
        channel
            .request_pty(
                false,
                target.terminal_options.terminal_type.term(),
                geometry.columns as u32,
                geometry.rows as u32,
                geometry.columns as u32 * CELL_WIDTH_PIXELS,
                geometry.rows as u32 * CELL_HEIGHT_PIXELS,
                &[],
            )
            .await
            .map_err(|error| format!("failed to request remote PTY: {error}"))
    })
    .await?;
    cancel_on_ssh_shutdown(&receiver, &shutdown, async {
        channel
            .set_env(false, "TERM", target.terminal_options.terminal_type.term())
            .await
            .map_err(|error| format!("failed to set TERM: {error}"))
    })
    .await?;
    if let Some(color_term) = target.terminal_options.color_term.env_value() {
        cancel_on_ssh_shutdown(&receiver, &shutdown, async {
            channel
                .set_env(false, "COLORTERM", color_term)
                .await
                .map_err(|error| format!("failed to set COLORTERM: {error}"))
        })
        .await?;
    }
    for entry in &target.terminal_options.environment {
        let variable = entry.variable.trim();
        if variable.is_empty() || variable.contains('=') {
            continue;
        }
        cancel_on_ssh_shutdown(&receiver, &shutdown, async {
            channel
                .set_env(false, variable, &entry.value)
                .await
                .map_err(|error| format!("failed to set {variable}: {error}"))
        })
        .await?;
    }
    if let Some(shell) = remote_shell {
        let command = remote_interactive_shell_command(&shell);
        cancel_on_ssh_shutdown(&receiver, &shutdown, async {
            channel
                .exec(false, command)
                .await
                .map_err(|error| format!("failed to start integrated remote shell: {error}"))
        })
        .await?;
    } else {
        cancel_on_ssh_shutdown(&receiver, &shutdown, async {
            channel
                .request_shell(false)
                .await
                .map_err(|error| format!("failed to start remote shell: {error}"))
        })
        .await?;
    }

    push_event(
        &events,
        &wakeup,
        SessionEvent::new(
            "connected",
            format!("Connected to {}@{}.", target.username, target.host),
        )
        .with_host_port(&target.host, target.port)
        .with_username(&target.username),
    );
    let mut command_tick = tokio::time::interval(SSH_COMMAND_POLL_INTERVAL);
    let latency_interval_seconds = target.ssh_keepalive_interval_seconds.max(1);
    let mut latency_tick =
        tokio::time::interval(Duration::from_secs(latency_interval_seconds as u64));
    latency_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let measure_latency = target.ssh_keepalive_interval_seconds > 0;
    let encoding = Encoding::for_label(target.encoding.as_bytes()).unwrap_or(UTF_8);
    let mut decoder = encoding.new_decoder();
    let mut exit_status_seen = false;
    let mut shutdown_requested = false;

    loop {
        tokio::select! {
            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let decoded = decode_ssh_output(&mut decoder, &data);
                        push_output(&output, &wakeup, &decoded);
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        exit_status_seen = true;
                        push_event(
                            &events,
                            &wakeup,
                            SessionEvent::new(
                                "exit_status",
                                format!("Remote process exited with status {exit_status}."),
                            )
                            .with_exit_status(exit_status),
                        );
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    Some(_) => {}
                }
            }
            _ = command_tick.tick() => {
                if !drain_ssh_commands(&receiver, &mut channel, &mut handle, encoding).await? {
                    shutdown_requested = true;
                    break;
                }
            }
            _ = latency_tick.tick(), if measure_latency => {
                let started = Instant::now();
                if tokio::time::timeout(Duration::from_secs(6), handle.send_ping())
                    .await
                    .is_ok_and(|result| result.is_ok())
                {
                    let latency_ms = started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
                    push_event(
                        &events,
                        &wakeup,
                        SessionEvent::new(
                            "ssh_latency_updated",
                            format!("SSH latency {latency_ms} ms."),
                        )
                        .with_latency_ms(latency_ms),
                    );
                }
            }
        }
    }

    if shutdown_requested || shutdown.load(Ordering::Acquire) {
        let graceful_shutdown = async {
            let _ = channel.close().await;
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
        };
        if tokio::time::timeout(SSH_SHUTDOWN_TIMEOUT, graceful_shutdown)
            .await
            .is_err()
        {
            eprintln!(
                "nauterm: SSH session did not finish graceful shutdown within {} ms",
                SSH_SHUTDOWN_TIMEOUT.as_millis()
            );
        }
        return Ok(());
    }
    let _ = channel.close().await;
    if exit_status_seen {
        push_event(
            &events,
            &wakeup,
            SessionEvent::new("session_closed", "SSH session closed.")
                .with_host_port(&target.host, target.port)
                .with_username(&target.username),
        );
    } else {
        push_event(
            &events,
            &wakeup,
            SessionEvent::new(
                "connection_lost",
                format!(
                    "SSH connection to {}:{} was lost.",
                    target.host, target.port
                ),
            )
            .with_host_port(&target.host, target.port)
            .with_username(&target.username),
        );
    }
    Ok(())
}

async fn detect_remote_interactive_shell(
    handle: &mut client::Handle<SshClientHandler>,
) -> Result<Option<String>, String> {
    const MARKER: &str = "__NAUTERM_SHELL__";
    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|error| format!("failed to open shell detection channel: {error}"))?;
    channel
        .exec(false, format!("printf '{MARKER}%s' \"$SHELL\""))
        .await
        .map_err(|error| format!("failed to detect remote shell: {error}"))?;

    let mut bytes = Vec::new();
    let mut exit_status = None;
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => {
                if bytes.len() < 4096 {
                    bytes.extend_from_slice(&data[..data.len().min(4096 - bytes.len())]);
                }
            }
            ChannelMsg::ExitStatus {
                exit_status: status,
            } => exit_status = Some(status),
            ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }
    let _ = channel.close().await;
    if exit_status.is_some_and(|status| status != 0) {
        return Ok(None);
    }

    let output = String::from_utf8_lossy(&bytes);
    let Some(shell) = output
        .split(MARKER)
        .nth(1)
        .map(str::trim)
        .filter(|shell| !shell.is_empty())
    else {
        return Ok(None);
    };
    let name = Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    Ok(matches!(
        name.as_str(),
        "zsh" | "bash" | "fish" | "ksh" | "ksh93" | "tcsh" | "csh" | "sh" | "dash" | "ash"
    )
    .then(|| shell.to_owned()))
}

fn remote_interactive_shell_command(shell: &str) -> String {
    let quoted = shell_quote(shell);
    let announce = format!("printf '\\033]4545;Shell;%s\\007' {quoted}; ");
    let name = Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match name.as_str() {
        "zsh" => format!("{announce}exec {quoted} --histignorespace -i -l"),
        "bash" => format!(
            "{announce}case :${{HISTCONTROL:-}}: in *:ignorespace:*|*:ignoreboth:*) ;; *) \
             HISTCONTROL=ignorespace${{HISTCONTROL:+:${{HISTCONTROL}}}} ;; esac; \
             export HISTCONTROL; exec {quoted} -i -l"
        ),
        "fish" => format!("{announce}exec {quoted} -i -l"),
        "ksh" | "ksh93" | "tcsh" | "csh" => format!("{announce}exec {quoted} -l"),
        _ => format!("{announce}exec {quoted} -i"),
    }
}

async fn drain_ssh_commands(
    receiver: &Receiver<SshCommand>,
    channel: &mut russh::Channel<client::Msg>,
    handle: &mut client::Handle<SshClientHandler>,
    encoding: &'static Encoding,
) -> Result<bool, String> {
    loop {
        match receiver.try_recv() {
            Ok(SshCommand::Input(bytes)) => channel
                .data(&encode_ssh_input(&bytes, encoding)[..])
                .await
                .map_err(|error| format!("failed to send input: {error}"))?,
            Ok(SshCommand::Resize(geometry)) => channel
                .window_change(
                    geometry.columns as u32,
                    geometry.rows as u32,
                    geometry.columns as u32 * CELL_WIDTH_PIXELS,
                    geometry.rows as u32 * CELL_HEIGHT_PIXELS,
                )
                .await
                .map_err(|error| format!("failed to resize remote PTY: {error}"))?,
            Ok(SshCommand::ReadShellHistory(reply)) => {
                let result = run_ssh_exec_command(handle, &shell_history_command())
                    .await
                    .and_then(|command| parse_shell_history_output(&command.stdout));
                let _ = reply.send(result);
            }
            Ok(SshCommand::Shutdown) | Err(TryRecvError::Disconnected) => return Ok(false),
            Err(TryRecvError::Empty) => return Ok(true),
        }
    }
}

fn decode_ssh_output(decoder: &mut Decoder, bytes: &[u8]) -> Vec<u8> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let Some(capacity) = decoder.max_utf8_buffer_length(bytes.len()) else {
        return bytes.to_vec();
    };
    let mut output = String::with_capacity(capacity);
    let _ = decoder.decode_to_string(bytes, &mut output, false);
    output.into_bytes()
}

fn encode_ssh_input(bytes: &[u8], encoding: &'static Encoding) -> Vec<u8> {
    if std::ptr::eq(encoding, UTF_8) {
        return bytes.to_vec();
    }
    let text = String::from_utf8_lossy(bytes);
    encoding.encode(&text).0.into_owned()
}

async fn cancel_on_ssh_shutdown<T, F>(
    receiver: &Receiver<SshCommand>,
    shutdown: &AtomicBool,
    future: F,
) -> Result<T, String>
where
    F: Future<Output = Result<T, String>>,
{
    tokio::select! {
        result = future => result,
        () = wait_for_ssh_shutdown(receiver, shutdown) => {
            Err(SSH_SESSION_CANCELLED.to_owned())
        }
    }
}

async fn cancel_on_sftp_cancel<T, F>(cancel: &AtomicBool, future: F) -> Result<T, String>
where
    F: Future<Output = Result<T, String>>,
{
    tokio::pin!(future);
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("SFTP listing cancelled.".to_owned());
        }
        tokio::select! {
            result = &mut future => return result,
            _ = tokio::time::sleep(SSH_COMMAND_POLL_INTERVAL) => {}
        }
    }
}

async fn wait_for_ssh_shutdown(receiver: &Receiver<SshCommand>, shutdown: &AtomicBool) {
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }

        match receiver.try_recv() {
            Ok(SshCommand::Shutdown) | Err(TryRecvError::Disconnected) => return,
            Ok(SshCommand::Input(_)) | Ok(SshCommand::Resize(_)) => {}
            Ok(SshCommand::ReadShellHistory(reply)) => {
                let _ = reply.send(Err("SSH session is not connected yet.".to_owned()));
            }
            Err(TryRecvError::Empty) => {
                tokio::time::sleep(SSH_COMMAND_POLL_INTERVAL).await;
            }
        }
    }
}

fn push_output(
    output: &Arc<Mutex<Vec<u8>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    bytes: &[u8],
) {
    if append_output(output, bytes) {
        notify_wakeup(wakeup);
    }
}

fn learn_known_hosts_path_without_leading_blank(
    host: &str,
    port: u16,
    pubkey: &russh::keys::ssh_key::PublicKey,
    path: &PathBuf,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    let metadata_len = file.metadata().map_err(|error| error.to_string())?.len();
    let needs_separator = if metadata_len == 0 {
        false
    } else {
        let mut last_byte = [0; 1];
        file.seek(SeekFrom::End(-1))
            .map_err(|error| error.to_string())?;
        file.read_exact(&mut last_byte)
            .map_err(|error| error.to_string())?;
        last_byte[0] != b'\n'
    };

    file.seek(SeekFrom::End(0))
        .map_err(|error| error.to_string())?;
    let mut file = std::io::BufWriter::new(file);
    if needs_separator {
        file.write_all(b"\n").map_err(|error| error.to_string())?;
    }
    if port != 22 {
        write!(file, "[{host}]:{port} ").map_err(|error| error.to_string())?;
    } else {
        write!(file, "{host} ").map_err(|error| error.to_string())?;
    }
    let encoded_key = pubkey.to_openssh().map_err(|error| error.to_string())?;
    file.write_all(encoded_key.as_bytes())
        .map_err(|error| error.to_string())?;
    file.write_all(b"\n").map_err(|error| error.to_string())?;

    Ok(())
}

fn push_event(
    events: &Arc<Mutex<Vec<SessionEvent>>>,
    wakeup: &Arc<Mutex<Option<WakeupCallback>>>,
    event: SessionEvent,
) {
    if let Ok(mut events) = events.lock() {
        events.push(event);
    }
    notify_wakeup(wakeup);
}

fn notify_wakeup(wakeup: &Arc<Mutex<Option<WakeupCallback>>>) {
    if let Ok(wakeup) = wakeup.lock() {
        if let Some(callback) = *wakeup {
            callback.call();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::keys::ssh_key::PublicKey;
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_PUBLIC_KEY: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti";

    #[test]
    fn sftp_task_request_accepts_optional_sudo_password() {
        let mut privileged = serde_json::from_str::<SftpTaskRequest>(
            r#"{"op":"delete","target_path":"/root/item","sudo_password":"secret","sudo_session_id":"pane-1"}"#,
        )
        .unwrap();
        let mut ordinary =
            serde_json::from_str::<SftpTaskRequest>(r#"{"op":"delete","target_path":"/tmp/item"}"#)
                .unwrap();

        assert_eq!(privileged.take_sudo_password().as_deref(), Some("secret"));
        assert_eq!(privileged.take_sudo_session_id().as_deref(), Some("pane-1"));
        assert_eq!(ordinary.take_sudo_password(), None);
        assert_eq!(ordinary.take_sudo_session_id(), None);
    }

    #[test]
    fn sftp_task_request_bounds_transfer_threads() {
        let default_request = serde_json::from_str::<SftpTaskRequest>(
            r#"{"op":"upload","local_path":"a","remote_path":"b"}"#,
        )
        .unwrap();
        let oversized_request = serde_json::from_str::<SftpTaskRequest>(
            r#"{"op":"download","remote_path":"a","local_path":"b","transfer_threads":99}"#,
        )
        .unwrap();

        assert_eq!(default_request.transfer_threads(), 8);
        assert_eq!(oversized_request.transfer_threads(), 32);
    }

    #[test]
    fn remote_shell_history_header_is_removed_and_normalized() {
        let (shell, content) = parse_shell_history_output(
            "Welcome to the server\nprofile warning\n\
             __NAUTERM_SHELL__/usr/bin/zsh\r\n\
             : 1720000000:0;echo one\necho two\n",
        )
        .unwrap();

        assert_eq!(shell.as_deref(), Some("/usr/bin/zsh"));
        assert_eq!(content, ": 1720000000:0;echo one\necho two\n");
    }

    #[test]
    fn remote_shell_history_requires_a_detected_shell() {
        let error = parse_shell_history_output("__NAUTERM_SHELL__\ncommand\n").unwrap_err();

        assert_eq!(error, "remote login shell could not be detected");
    }

    #[test]
    fn remote_shell_history_collector_runs_through_posix_sh() {
        let command = shell_history_command();

        assert!(command.starts_with("sh -c '"));
        assert!(command.contains("__NAUTERM_SHELL__"));
        assert!(!command.contains("sh -c ''"));
    }

    #[test]
    fn integrated_remote_zsh_enables_history_ignore_space_before_startup() {
        assert_eq!(
            remote_interactive_shell_command("/usr/bin/zsh"),
            "printf '\\033]4545;Shell;%s\\007' '/usr/bin/zsh'; \
             exec '/usr/bin/zsh' --histignorespace -i -l"
        );
    }

    #[test]
    fn integrated_remote_bash_preserves_history_policy_and_adds_ignore_space() {
        let command = remote_interactive_shell_command("/bin/bash");

        assert!(command.contains("HISTCONTROL=ignorespace"));
        assert!(command.contains("export HISTCONTROL"));
        assert!(command.starts_with("printf '\\033]4545;Shell;%s\\007' '/bin/bash'; "));
        assert!(command.ends_with("exec '/bin/bash' -i -l"));
    }

    #[cfg(all(feature = "integration-tests", not(windows)))]
    #[test]
    fn remote_shell_history_collector_reads_a_bash_history_file() {
        let home = std::env::temp_dir().join(format!(
            "nauterm-ssh-history-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(".bash_history"), "echo first\necho second\n").unwrap();

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(shell_history_command())
            .env("HOME", &home)
            .env("SHELL", "/bin/bash")
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let (shell, content) = parse_shell_history_output(&stdout).unwrap();

        assert!(output.status.success());
        assert_eq!(shell.as_deref(), Some("bash"));
        assert_eq!(content, "echo first\necho second\n");
        let _ = fs::remove_dir_all(home);
    }

    #[cfg(all(feature = "integration-tests", not(windows)))]
    #[test]
    fn remote_shell_history_collector_resolves_custom_bash_histfile() {
        let home = std::env::temp_dir().join(format!(
            "nauterm-ssh-custom-history-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&home).unwrap();
        fs::write(
            home.join(".bashrc"),
            "HISTFILE=\"$HOME/.custom_bash_history\"\n",
        )
        .unwrap();
        fs::write(
            home.join(".custom_bash_history"),
            "echo custom first\necho custom second\n",
        )
        .unwrap();

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(shell_history_command())
            .env("HOME", &home)
            .env("SHELL", "/bin/bash")
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let (shell, content) = parse_shell_history_output(&stdout).unwrap();

        assert!(output.status.success());
        assert_eq!(shell.as_deref(), Some("bash"));
        assert_eq!(content, "echo custom first\necho custom second\n");
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn learning_known_host_does_not_start_empty_file_with_blank_line() {
        let path = temp_known_hosts_path("empty");
        let public_key = test_public_key();

        learn_known_hosts_path_without_leading_blank("example.com", 22, &public_key, &path)
            .unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.starts_with('\n'));
        assert!(text.starts_with("example.com ssh-ed25519 "));
        assert!(text.ends_with('\n'));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn learning_known_host_separates_existing_unterminated_line() {
        let path = temp_known_hosts_path("unterminated");
        fs::write(&path, "old.example.com ssh-ed25519 AAAA").unwrap();
        let public_key = test_public_key();

        learn_known_hosts_path_without_leading_blank("example.com", 2222, &public_key, &path)
            .unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\n[example.com]:2222 ssh-ed25519 "));
        assert!(!text.starts_with('\n'));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn ssh_client_config_uses_keepalive_policy() {
        let config = ssh_client_config();

        assert_eq!(config.keepalive_interval, Some(SSH_KEEPALIVE_INTERVAL));
        assert_eq!(config.keepalive_max, SSH_KEEPALIVE_MISSES);
        assert!(config.nodelay);
    }

    #[test]
    fn ssh_client_config_can_disable_keepalive() {
        let config = ssh_client_config_with_keepalive(0);

        assert_eq!(config.keepalive_interval, None);
        assert_eq!(config.keepalive_max, SSH_KEEPALIVE_MISSES);
    }

    #[test]
    fn sftp_channel_window_scales_with_transfer_concurrency() {
        let single = ssh_client_config_for_sftp(1);
        let eight = ssh_client_config_for_sftp(8);
        let oversized = ssh_client_config_for_sftp(99);

        assert_eq!(single.window_size, client::Config::default().window_size);
        assert_eq!(eight.window_size, 4 * 1024 * 1024);
        assert_eq!(oversized.window_size, SFTP_MAX_CHANNEL_WINDOW_SIZE);
    }

    #[test]
    fn direct_ssh_connect_prefers_the_connected_peer_ip_for_udp() {
        let peer = "203.0.113.9:22".parse().unwrap();
        assert_eq!(
            preferred_mosh_udp_host("ssh-alias", None, Some(peer)),
            SelectedMoshUdpHost {
                host: "203.0.113.9".to_owned(),
                source: MoshUdpHostSource::ConnectedPeerIp,
            }
        );
    }

    #[test]
    fn proxied_ssh_connect_keeps_the_original_host_for_udp() {
        let proxy = SshProxyConfig {
            proxy_type: "socks5".to_owned(),
            host: "proxy.example".to_owned(),
            port: 1080,
            username: None,
            password: None,
        };
        let peer = "203.0.113.9:22".parse().unwrap();
        assert_eq!(
            preferred_mosh_udp_host("ssh-alias", Some(&proxy), Some(peer)),
            SelectedMoshUdpHost {
                host: "ssh-alias".to_owned(),
                source: MoshUdpHostSource::OriginalHost,
            }
        );
    }

    #[test]
    fn proxied_ssh_connect_normalizes_bracketed_ipv6_hosts_for_udp() {
        let proxy = SshProxyConfig {
            proxy_type: "http".to_owned(),
            host: "proxy.example".to_owned(),
            port: 8080,
            username: None,
            password: None,
        };
        assert_eq!(
            preferred_mosh_udp_host("[2001:db8::10]", Some(&proxy), None),
            SelectedMoshUdpHost {
                host: "2001:db8::10".to_owned(),
                source: MoshUdpHostSource::OriginalHost,
            }
        );
    }

    #[test]
    fn os_release_ids_are_normalized_for_supported_linux_distributions() {
        let fixtures = [
            ("ID=alpine\nNAME=Alpine Linux", "alpine"),
            ("ID=almalinux\nNAME=AlmaLinux", "alma"),
            ("ID=amzn\nNAME=Amazon Linux", "amazon"),
            ("ID=arch\nNAME=Arch Linux", "arch"),
            ("ID=raspbian\nNAME=Raspbian GNU/Linux", "raspbian"),
            ("ID=opensuse-leap\nNAME=openSUSE Leap", "suse"),
            ("ID=rhel\nNAME=Red Hat Enterprise Linux", "redhat"),
        ];

        for (contents, expected) in fixtures {
            assert_eq!(parse_os_release(Some(contents)).as_deref(), Some(expected));
        }
    }

    #[test]
    fn unknown_linux_release_uses_generic_linux_distribution() {
        assert_eq!(
            parse_os_release(Some("ID=custom\nNAME=Custom Linux")).as_deref(),
            Some("linux")
        );
    }

    #[test]
    fn unix_names_are_normalized_as_operating_system_families() {
        let output = SshCommandOutput {
            stdout: "Darwin".to_owned(),
            exit_status: 0,
        };

        assert_eq!(
            detected_os_from_unix_probe(Some(&output)),
            Some(DetectedHostPlatform {
                os: "macos".to_owned(),
                distro: None,
            })
        );
    }

    #[test]
    fn host_system_information_is_parsed_and_cpu_is_clamped() {
        let info = parse_host_system_info(
            "hostname=server-01\n\
             os_name=Debian GNU/Linux 13\n\
             kernel=Linux 6.12\n\
             architecture=x86_64\n\
             uptime_seconds=86461\n\
             load_average=0.42\n\
             cpu_count=8\n\
             cpu_usage_percent=104.5\n\
             memory_total_bytes=17179869184\n\
             memory_used_bytes=8589934592\n\
             swap_total_bytes=4294967296\n\
             swap_used_bytes=1073741824\n\
             disk_total_bytes=107374182400\n\
             disk_used_bytes=26843545600\n\
             process=134217728|2.5|sshd\n\
             network=eth0|1024|512\n\
             filesystem=/|107374182400|26843545600\n",
        );

        assert_eq!(info.hostname.as_deref(), Some("server-01"));
        assert_eq!(info.os_name.as_deref(), Some("Debian GNU/Linux 13"));
        assert_eq!(info.cpu_count, Some(8));
        assert_eq!(info.cpu_usage_percent, Some(100.0));
        assert_eq!(info.memory_used_bytes, Some(8_589_934_592));
        assert_eq!(info.disk_total_bytes, Some(107_374_182_400));
        assert_eq!(info.swap_used_bytes, Some(1_073_741_824));
        assert_eq!(info.processes[0].command, "sshd");
        assert_eq!(info.network_interfaces[0].name, "eth0");
        assert_eq!(info.filesystems[0].path, "/");
    }

    #[test]
    fn public_key_export_accepts_one_authorized_keys_line() {
        assert_eq!(
            normalize_public_key_for_export(TEST_PUBLIC_KEY).unwrap(),
            TEST_PUBLIC_KEY
        );
        assert!(
            normalize_public_key_for_export(&format!("{TEST_PUBLIC_KEY}\n{TEST_PUBLIC_KEY}"))
                .is_err()
        );
    }

    #[test]
    fn public_key_export_quotes_shell_arguments_and_rejects_path_filenames() {
        let command = build_public_key_export_command(
            EXPORT_PUBLIC_KEY_SCRIPT,
            TEST_PUBLIC_KEY,
            ".ssh/team's keys",
            "authorized_keys",
        );

        assert!(command.contains(r#"'.ssh/team'\''s keys'"#));
        assert!(command.contains("'authorized_keys'"));
        assert!(command.contains("'if test ! -e \"$1\"; then"));
        assert!(validate_export_filename("../authorized_keys").is_err());
        assert!(validate_export_filename("authorized_keys").is_ok());
        assert!(validate_export_script("").is_err());
        assert!(validate_export_script(EXPORT_PUBLIC_KEY_SCRIPT).is_ok());
    }

    #[test]
    fn cancellable_ssh_future_stops_on_shutdown_command() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let (sender, receiver) = mpsc::channel();
        let shutdown = AtomicBool::new(false);
        sender.send(SshCommand::Shutdown).unwrap();

        let result = runtime.block_on(cancel_on_ssh_shutdown(&receiver, &shutdown, async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            Ok::<(), String>(())
        }));

        assert_eq!(result, Err(SSH_SESSION_CANCELLED.to_owned()));
    }

    #[test]
    fn repeated_transport_shutdown_does_not_leave_workers_running() {
        for _ in 0..32 {
            let transport = SshTransport::connect(
                TerminalGeometry::new(80, 24),
                TerminalOptions::default(),
                "127.0.0.1",
                9,
                "test",
                None,
                None,
                None,
                None,
                HostKeyTrustMode::Strict,
                None,
                0,
                "UTF-8",
            )
            .unwrap();
            let exited = transport.exited.clone();

            drop(transport);

            assert!(exited.load(Ordering::Acquire));
        }
    }

    fn test_public_key() -> PublicKey {
        PublicKey::from_openssh(TEST_PUBLIC_KEY).unwrap()
    }

    fn temp_known_hosts_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "nauterm_known_hosts_{name}_{}_{}",
            std::process::id(),
            nanos
        ))
    }
}
