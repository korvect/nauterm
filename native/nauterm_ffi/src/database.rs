use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::ffi::{c_char, CStr, CString};
use std::fmt::Write as _;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::ptr;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, types::Type, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use zeroize::Zeroize;

mod migrations;
mod repositories;
mod schema;

use schema::create_schema;

const SCHEMA_VERSION: i32 = 4;
#[cfg(test)]
const DEFAULT_MOSH_SERVER_COMMAND: &str = "mosh-server new -s -l LANG=en_US.UTF-8";
const DEVICE_ID_METADATA_KEY: &str = "device_id";
const DEVICE_TRACKED_TABLES: &[&str] = &[
    "groups",
    "keys",
    "identities",
    "hosts",
    "tags",
    "port_forwards",
    "proxies",
    "sftp_favorites",
    "snippet_packages",
    "snippets",
];
#[derive(Debug)]
pub struct NautermDatabase {
    connection: Connection,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostGroup {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<i64>,
    #[serde(default)]
    pub parent_uuid: Option<String>,
    #[serde(default)]
    pub identity_id: Option<i64>,
    #[serde(default)]
    pub identity_uuid: Option<String>,
    #[serde(default)]
    pub proxy_id: Option<i64>,
    #[serde(default)]
    pub proxy_uuid: Option<String>,
    #[serde(default)]
    pub port: Option<i64>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub theme_id: Option<String>,
    #[serde(default)]
    pub startup_snippet_id: Option<i64>,
    #[serde(default)]
    pub startup_snippet_uuid: Option<String>,
    #[serde(default)]
    pub ssh_enabled: Option<bool>,
    #[serde(default)]
    pub mosh_enabled: Option<bool>,
    #[serde(default)]
    pub mosh_server_command: Option<String>,
    #[serde(default)]
    pub telnet_enabled: Option<bool>,
    #[serde(default)]
    pub telnet_identity_id: Option<i64>,
    #[serde(default)]
    pub telnet_identity_uuid: Option<String>,
    #[serde(default)]
    pub telnet_username: Option<String>,
    #[serde(default)]
    pub telnet_password: Option<String>,
    #[serde(default)]
    pub telnet_port: Option<i64>,
    #[serde(default)]
    pub telnet_theme_id: Option<String>,
    #[serde(default)]
    pub environment_variables: Vec<HostEnvironmentVariable>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub telnet_encoding: Option<String>,
    #[serde(default)]
    pub key_id: Option<i64>,
    #[serde(default)]
    pub key_uuid: Option<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub name: String,
    #[serde(default)]
    pub private_key: Option<String>,
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default)]
    pub certificate: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub name: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub key_id: Option<i64>,
    #[serde(default)]
    pub key_uuid: Option<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub name: String,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostType {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl HostType {
    fn as_str(self) -> &'static str {
        match self {
            HostType::Local => "local",
            HostType::Remote => "remote",
        }
    }

    fn from_str(value: &str) -> rusqlite::Result<Self> {
        match value {
            "local" => Ok(HostType::Local),
            "remote" => Ok(HostType::Remote),
            _ => Err(rusqlite::Error::InvalidParameterName(format!(
                "unknown host type: {value}"
            ))),
        }
    }
}

#[cfg(test)]
fn default_mosh_server_command() -> String {
    DEFAULT_MOSH_SERVER_COMMAND.to_string()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostEnvironmentVariable {
    pub variable: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub name: String,
    #[serde(default)]
    pub group_id: Option<i64>,
    #[serde(default)]
    pub group_uuid: Option<String>,
    pub identity_id: Option<i64>,
    #[serde(default)]
    pub identity_uuid: Option<String>,
    #[serde(default)]
    pub proxy_id: Option<i64>,
    #[serde(default)]
    pub proxy_uuid: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<i64>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub theme_id: Option<String>,
    #[serde(default)]
    pub startup_snippet_id: Option<i64>,
    #[serde(default)]
    pub startup_snippet_uuid: Option<String>,
    #[serde(default)]
    pub ssh_enabled: Option<bool>,
    #[serde(default)]
    pub mosh_enabled: Option<bool>,
    #[serde(default)]
    pub mosh_server_command: Option<String>,
    #[serde(default)]
    pub telnet_enabled: Option<bool>,
    #[serde(default)]
    pub telnet_identity_id: Option<i64>,
    #[serde(default)]
    pub telnet_identity_uuid: Option<String>,
    #[serde(default)]
    pub telnet_username: Option<String>,
    #[serde(default)]
    pub telnet_password: Option<String>,
    #[serde(default)]
    pub telnet_port: Option<i64>,
    #[serde(default)]
    pub telnet_theme_id: Option<String>,
    #[serde(default)]
    pub environment_variables: Vec<HostEnvironmentVariable>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub telnet_encoding: Option<String>,
    #[serde(rename = "type")]
    pub host_type: HostType,
    #[serde(default)]
    pub key_id: Option<i64>,
    #[serde(default)]
    pub key_uuid: Option<String>,
    #[serde(default)]
    pub shell_path: Option<String>,
    #[serde(default)]
    pub work_dir: Option<String>,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub distro: Option<String>,
    #[serde(default)]
    pub tag_uuids: Vec<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortForwardEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub name: String,
    pub r#type: String,
    pub bind_address: String,
    pub bind_port: i64,
    pub destination_host: String,
    pub destination_port: i64,
    pub connection_id: i64,
    #[serde(default)]
    pub host_uuid: Option<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub name: String,
    pub r#type: String,
    pub host: String,
    pub port: i64,
    #[serde(default)]
    pub identity_id: Option<i64>,
    #[serde(default)]
    pub identity_uuid: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiProviderEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_ai_provider_config")]
    pub config: Map<String, Value>,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

fn default_ai_provider_config() -> Map<String, Value> {
    Map::new()
}

fn normalize_ai_provider_protocol(value: &str) -> rusqlite::Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "openai" => Ok("openai"),
        "anthropic" => Ok("anthropic"),
        "google" | "googleai" | "gemini" => Ok("google"),
        "ollama" => Ok("ollama"),
        value => Err(rusqlite::Error::InvalidParameterName(format!(
            "unsupported AI provider protocol: {value}"
        ))),
    }
}

fn normalize_port_forward_type(value: &str) -> rusqlite::Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "local" => Ok("local"),
        "remote" => Ok("remote"),
        "dynamic" => Ok("dynamic"),
        value => Err(rusqlite::Error::InvalidParameterName(format!(
            "unsupported port forward type: {value}"
        ))),
    }
}

fn normalize_proxy_type(value: &str) -> rusqlite::Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "http" => Ok("http"),
        "socks5" => Ok("socks5"),
        value => Err(rusqlite::Error::InvalidParameterName(format!(
            "unsupported proxy type: {value}"
        ))),
    }
}

fn normalize_ai_conversation_scope(value: &str) -> rusqlite::Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "terminal" => Ok("terminal"),
        "workspace" => Ok("workspace"),
        value => Err(rusqlite::Error::InvalidParameterName(format!(
            "unsupported AI conversation scope: {value}"
        ))),
    }
}

fn normalize_ai_message_role(value: &str) -> rusqlite::Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "system" => Ok("system"),
        "user" => Ok("user"),
        "assistant" => Ok("assistant"),
        "tool" => Ok("tool"),
        value => Err(rusqlite::Error::InvalidParameterName(format!(
            "unsupported AI message role: {value}"
        ))),
    }
}

fn normalize_ai_command_status(value: &str) -> rusqlite::Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pending" => Ok("pending"),
        "running" => Ok("running"),
        "succeeded" => Ok("succeeded"),
        "failed" => Ok("failed"),
        "cancelled" => Ok("cancelled"),
        "skipped" => Ok("skipped"),
        value => Err(rusqlite::Error::InvalidParameterName(format!(
            "unsupported AI command status: {value}"
        ))),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SftpFavoritePathEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub scope: String,
    #[serde(default)]
    pub host_id: Option<i64>,
    #[serde(default)]
    pub host_uuid: Option<String>,
    pub path: String,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SftpTaskHistoryEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub host_uuid: Option<String>,
    #[serde(rename = "type")]
    pub task_type: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub username: String,
    #[serde(default = "default_sftp_port")]
    pub port: i64,
    pub status: String,
    pub display_name: String,
    pub source_path: String,
    pub target_path: String,
    pub created_at: i64,
    pub finished_at: i64,
    #[serde(default)]
    pub bytes: i64,
    #[serde(default)]
    pub total_bytes: i64,
    #[serde(default)]
    pub item_kind: String,
    #[serde(default)]
    pub error: Option<String>,
}

fn default_sftp_port() -> i64 {
    22
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetPackageEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub name: String,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub package_id: Option<i64>,
    #[serde(default)]
    pub package_uuid: Option<String>,
    #[serde(default = "default_snippet_scope")]
    pub scope: String,
    pub description: String,
    pub script: String,
    #[serde(default)]
    pub target_group_ids: Vec<i64>,
    #[serde(default)]
    pub target_host_ids: Vec<i64>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

fn default_snippet_scope() -> String {
    "global".to_string()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalLogEntry {
    pub id: String,
    #[serde(default)]
    pub local_id: Option<i64>,
    pub title: String,
    #[serde(default)]
    pub theme_id: Option<String>,
    #[serde(default)]
    pub host_id: Option<i64>,
    #[serde(default)]
    pub host_uuid: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<i64>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub shell_path: Option<String>,
    #[serde(default)]
    pub work_dir: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    pub capture_file: String,
    #[serde(default)]
    pub capture_bytes: i64,
    #[serde(default)]
    pub capture_sha256: Option<String>,
    #[serde(default)]
    pub columns: Option<i64>,
    #[serde(default)]
    pub rows: Option<i64>,
    pub started_at: i64,
    #[serde(default)]
    pub ended_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalLogEvent {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub log_id: Option<String>,
    #[serde(default)]
    pub log_uuid: Option<String>,
    pub timestamp: i64,
    #[serde(rename = "type")]
    pub event_type: String,
    pub message: String,
    #[serde(default)]
    pub connection_kind: Option<String>,
    #[serde(default)]
    pub data: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiConversationEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub title: String,
    #[serde(default)]
    pub preview: Option<String>,
    pub scope: String,
    #[serde(default)]
    pub host_uuid: Option<String>,
    #[serde(default)]
    pub provider_uuid: Option<String>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub messages: Vec<AiMessageEntry>,
    #[serde(default)]
    pub command_blocks: Vec<AiCommandBlockEntry>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub created_device_id: Option<String>,
    #[serde(default)]
    pub updated_device_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiMessageEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub context: String,
    pub sequence: i64,
    #[serde(default)]
    pub tool_calls: Vec<Value>,
    #[serde(default)]
    pub tool_result: Option<Value>,
    #[serde(default)]
    pub attachments: Vec<Value>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiCommandBlockEntry {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub uuid: Option<String>,
    pub tool_call_id: String,
    pub command: String,
    pub explanation: String,
    pub status: String,
    pub sequence: i64,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub exit_code: Option<i64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub finished_at: Option<i64>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum DatabaseRequest {
    SaveGroup {
        group: Box<HostGroup>,
    },
    GetGroup {
        id: i64,
    },
    ListGroups,
    DeleteGroup {
        id: i64,
    },
    SaveKey {
        key: KeyEntry,
    },
    GetKey {
        id: i64,
    },
    ListKeys,
    DeleteKey {
        id: i64,
    },
    SaveIdentity {
        identity: IdentityEntry,
    },
    GetIdentity {
        id: i64,
    },
    ListIdentities,
    DeleteIdentity {
        id: i64,
    },
    SaveTag {
        tag: TagEntry,
    },
    GetTag {
        id: i64,
    },
    ListTags,
    DeleteTag {
        id: i64,
    },
    SaveHost {
        host: Box<HostEntry>,
    },
    GetHost {
        id: i64,
    },
    ListHosts {
        group_id: Option<i64>,
    },
    DeleteHost {
        id: i64,
    },
    SavePortForward {
        port_forward: PortForwardEntry,
    },
    GetPortForward {
        id: i64,
    },
    ListPortForwards {
        connection_id: Option<i64>,
    },
    DeletePortForward {
        id: i64,
    },
    SaveProxy {
        proxy: ProxyEntry,
    },
    GetProxy {
        id: i64,
    },
    ListProxies,
    DeleteProxy {
        id: i64,
    },
    SaveAiProvider {
        provider: AiProviderEntry,
    },
    GetActiveAiProvider,
    ListAiProviders,
    DeleteAiProvider {
        id: i64,
    },
    SaveSftpFavoritePath {
        favorite: SftpFavoritePathEntry,
    },
    ListSftpFavoritePaths {
        scope: String,
        #[serde(default)]
        host_id: Option<i64>,
    },
    DeleteSftpFavoritePathByTarget {
        scope: String,
        #[serde(default)]
        host_id: Option<i64>,
        path: String,
    },
    SaveSftpTaskHistory {
        task: SftpTaskHistoryEntry,
        cutoff: i64,
    },
    ListSftpTaskHistory {
        cutoff: i64,
    },
    DeleteSftpTaskHistory {
        id: i64,
    },
    ClearSftpTaskHistory,
    SaveSnippetPackage {
        package: SnippetPackageEntry,
    },
    ListSnippetPackages,
    DeleteSnippetPackage {
        id: i64,
    },
    SaveSnippet {
        snippet: SnippetEntry,
    },
    GetSnippet {
        id: i64,
    },
    ListSnippets {
        package_id: Option<i64>,
    },
    DeleteSnippet {
        id: i64,
    },
    SaveTerminalLog {
        log: TerminalLogEntry,
        events: Vec<TerminalLogEvent>,
    },
    ListTerminalLogs {
        limit: Option<i64>,
        #[serde(default)]
        offset: Option<i64>,
    },
    ListTerminalLogEvents {
        log_id: String,
    },
    DeleteTerminalLog {
        log_id: String,
    },
    ClearTerminalLogs,
    ListTerminalCaptureFiles,
    ListIncompleteTerminalCaptures,
    ClearMissingTerminalCapture {
        log_id: String,
    },
    FinalizeRecoveredTerminalCapture {
        log_id: String,
        capture_bytes: i64,
        capture_sha256: String,
        ended_at: i64,
    },
    SaveAiConversation {
        conversation: AiConversationEntry,
    },
    GetAiConversation {
        uuid: String,
    },
    ListAiConversations {
        scope: Option<String>,
        host_uuid: Option<String>,
        limit: Option<i64>,
    },
    DeleteAiConversation {
        uuid: String,
    },
    EncryptionStatus,
    InitEncryption,
    SetMasterKey {
        master_key: String,
    },
    ChangeMasterKey {
        current_master_key: String,
        new_master_key: String,
    },
    RemoveMasterKey {
        current_master_key: String,
    },
    VerifyMasterKey {
        master_key: String,
    },
    GithubSaveToken {
        token: String,
    },
    GithubLoadToken,
    GithubReadToken,
    GithubDeleteToken,
    GithubGistSaveToken {
        token: String,
    },
    GithubGistLoadToken,
    GithubGistDeleteToken,
    GithubGistSaveConfig {
        config: crate::github_gist_sync::GithubGistConfig,
    },
    GithubGistLoadConfig,
    GithubGistDeleteConfig,
    GithubGistSync {
        #[serde(default)]
        master_key: Option<String>,
        #[serde(default)]
        strategy: crate::sync::SyncStrategy,
        #[serde(default)]
        backup_count: usize,
    },
    GithubGistChangeMasterKey {
        current_master_key: String,
        new_master_key: String,
    },
    GithubGistListHistory {
        #[serde(default = "default_sync_history_limit")]
        limit: usize,
    },
    GithubGistRestoreVersion {
        version: String,
        #[serde(default)]
        backup_count: usize,
    },
    SyncKeyStatus,
    ForgetSyncKey,
    GithubSaveConfig {
        config: crate::github_sync::GithubRepoConfig,
    },
    GithubLoadConfig,
    GithubDeleteConfig,
    GithubSync {
        #[serde(default)]
        master_key: Option<String>,
        #[serde(default)]
        strategy: crate::sync::SyncStrategy,
        #[serde(default)]
        backup_count: usize,
    },
    GithubChangeMasterKey {
        current_master_key: String,
        new_master_key: String,
    },
    GithubListHistory {
        #[serde(default = "default_sync_history_limit")]
        limit: usize,
    },
    GithubRestoreRevision {
        commit_sha: String,
        #[serde(default)]
        backup_count: usize,
    },
    S3SaveCredentials {
        access_key_id: String,
        secret_access_key: String,
    },
    S3LoadCredentials,
    S3ReadCredentials,
    S3DeleteCredentials,
    S3SaveConfig {
        config: crate::s3_sync::S3Config,
    },
    S3LoadConfig,
    S3DeleteConfig,
    S3Sync {
        #[serde(default)]
        master_key: Option<String>,
        #[serde(default)]
        strategy: crate::sync::SyncStrategy,
        #[serde(default)]
        backup_count: usize,
    },
    S3ListHistory {
        #[serde(default = "default_sync_history_limit")]
        limit: usize,
    },
    S3RestoreVersion {
        version_id: String,
        #[serde(default)]
        backup_count: usize,
    },
    S3ChangeMasterKey {
        current_master_key: String,
        new_master_key: String,
    },
    CloudListProviders,
    CloudLoadCredentials {
        provider_id: String,
    },
    SyncPreferences,
    RefreshRemoteSyncStatus,
    SaveSyncPreferences {
        preferences: crate::cloud_sync::SyncPreferences,
    },
    CloudSaveProvider {
        provider: crate::cloud_sync::CloudProviderConfig,
        #[serde(default)]
        credentials: Option<crate::cloud_sync::CloudProviderCredentials>,
    },
    CloudDeleteProvider {
        provider_id: String,
    },
    CloudSync {
        provider_id: String,
        #[serde(default)]
        master_key: Option<String>,
        #[serde(default)]
        strategy: crate::sync::SyncStrategy,
        #[serde(default)]
        backup_count: usize,
    },
    CloudListHistory {
        provider_id: String,
        #[serde(default = "default_sync_history_limit")]
        limit: usize,
    },
    CloudRestoreVersion {
        provider_id: String,
        version_id: String,
        #[serde(default)]
        backup_count: usize,
    },
    LocalSyncBackupList,
    LocalSyncBackupRestore {
        backup_id: String,
        #[serde(default)]
        backup_count: usize,
    },
    CloudChangeMasterKey {
        provider_id: String,
        current_master_key: String,
        new_master_key: String,
    },
    DeviceId,
    GetAppMeta {
        key: String,
    },
    SetAppMeta {
        key: String,
        value: String,
    },
    SchemaVersion,
}

fn crypto_sql_error(error: crate::crypto::CryptoError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

fn default_sync_history_limit() -> usize {
    20
}

fn configured_dek(connection: &Connection) -> rusqlite::Result<Option<crate::crypto::Dek>> {
    crate::crypto::local_dek_if_configured(connection).map_err(crypto_sql_error)
}

fn encrypt_optional_field(
    dek: Option<&crate::crypto::Dek>,
    value: Option<&str>,
) -> rusqlite::Result<Option<String>> {
    value
        .map(|value| match dek {
            Some(dek) => crate::crypto::encrypt_field(dek, value).map_err(crypto_sql_error),
            None => Ok(value.to_string()),
        })
        .transpose()
}

fn decrypt_optional_field(
    dek: Option<&crate::crypto::Dek>,
    value: Option<String>,
) -> rusqlite::Result<Option<String>> {
    value
        .map(|value| match dek {
            Some(dek) => crate::crypto::decrypt_field(dek, &value).map_err(crypto_sql_error),
            None => Ok(value),
        })
        .transpose()
}

impl NautermDatabase {
    pub fn open_default() -> rusqlite::Result<Self> {
        Self::open(default_database_path())
    }

    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| rusqlite::Error::InvalidPath(parent.to_path_buf()))?;
        }

        let mut connection = Connection::open(path)?;
        Self::key_database(&connection)?;
        Self::configure(&connection)?;
        Self::ensure_schema(&mut connection)?;
        #[cfg(not(windows))]
        Self::enable_extended_memory_security(&connection)?;
        Ok(Self { connection })
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let mut connection = Connection::open_in_memory()?;
        Self::configure(&connection)?;
        Self::ensure_schema(&mut connection)?;
        Ok(Self { connection })
    }

    fn key_database(connection: &Connection) -> rusqlite::Result<()> {
        let key = crate::crypto::load_or_create_database_key().map_err(crypto_sql_error)?;
        let mut hex = zeroize::Zeroizing::new(String::with_capacity(key.len() * 2));
        for byte in key.iter() {
            write!(&mut *hex, "{byte:02x}").map_err(|_| rusqlite::Error::InvalidQuery)?;
        }
        let mut statement =
            zeroize::Zeroizing::new(format!("PRAGMA key = \"x'{}'\";", hex.as_str()));
        connection.execute_batch(statement.as_str())?;
        statement.zeroize();
        connection.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;
        let cipher_version: String =
            connection.query_row("PRAGMA cipher_version", [], |row| row.get(0))?;
        if cipher_version.trim().is_empty() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn enable_extended_memory_security(connection: &Connection) -> rusqlite::Result<()> {
        // Windows keeps the default OFF because failed VirtualLock calls can
        // overflow the stack while SQLCipher releases protected allocations.
        connection.execute_batch("PRAGMA cipher_memory_security = ON;")
    }

    fn backup_connection_to(connection: &Connection, path: &Path) -> rusqlite::Result<()> {
        let staging_path = path.with_extension("sqlite.part");
        let result = (|| {
            let mut destination = Connection::open(&staging_path)?;
            Self::key_database(&destination)?;
            {
                let backup = rusqlite::backup::Backup::new(connection, &mut destination)?;
                backup.run_to_completion(32, Duration::from_millis(10), None)?;
            }
            destination.close().map_err(|(_, error)| error)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(&staging_path, fs::Permissions::from_mode(0o600))
                    .map_err(|_| rusqlite::Error::InvalidPath(staging_path.clone()))?;
            }
            fs::rename(&staging_path, path)
                .map_err(|_| rusqlite::Error::InvalidPath(path.to_path_buf()))
        })();
        if result.is_err() {
            let _ = fs::remove_file(staging_path);
        }
        result
    }

    pub fn schema_version(&self) -> rusqlite::Result<i32> {
        self.connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
    }

    /// Reads an arbitrary key from the `app_metadata` table, returning `None`
    /// when the key is absent. Used by the Dart layer as a lightweight
    /// key/value store (e.g. first-launch flags) backed by the encrypted DB.
    pub fn get_app_meta(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT value FROM app_metadata WHERE key = ?",
                params![key],
                |row| row.get(0),
            )
            .optional()
    }

    /// Inserts or updates an `app_metadata` key. Mirrors the upsert used by the
    /// internal encryption/sync metadata helpers.
    pub fn set_app_meta(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            r#"INSERT INTO app_metadata (key, value) VALUES (?, ?)
               ON CONFLICT(key) DO UPDATE SET
                 value = excluded.value,
                 updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)"#,
            params![key, value],
        )?;
        Ok(())
    }

    pub fn save_sftp_favorite_path(
        &mut self,
        favorite: &SftpFavoritePathEntry,
    ) -> rusqlite::Result<i64> {
        let scope = normalize_sftp_favorite_scope(&favorite.scope)?;
        let host_id = favorite.host_id.filter(|id| *id > 0);
        let host_uuid = relation_uuid(
            &self.connection,
            "hosts",
            host_id,
            favorite.host_uuid.as_deref(),
        )?;
        let host_uuid = host_uuid.ok_or_else(|| {
            rusqlite::Error::InvalidParameterName("sftp favorites require a saved host".to_string())
        })?;
        let path = favorite.path.trim();
        if path.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "sftp favorite path is required".to_string(),
            ));
        }

        if let Some(id) = favorite.id {
            self.connection.execute(
                r#"
                UPDATE sftp_favorites
                SET scope = ?, host_uuid = ?, path = ?,
                    updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                    deleted_at = NULL,
                    version = COALESCE(version, 0) + 1
                WHERE id = ?
                "#,
                params![scope, host_uuid, path, id],
            )?;
            return Ok(id);
        }

        let uuid = uuid_or_new(&self.connection, favorite.uuid.as_deref())?;
        self.connection.execute(
            r#"
            INSERT OR IGNORE INTO sftp_favorites (
              uuid, scope, host_uuid, path, created_at, updated_at
            )
            VALUES (
              ?, ?, ?, ?, CAST(unixepoch('subsec') * 1000 AS INTEGER),
              CAST(unixepoch('subsec') * 1000 AS INTEGER)
            )
            "#,
            params![uuid, scope, host_uuid, path],
        )?;
        self.connection.execute(
            r#"
            UPDATE sftp_favorites
            SET updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                deleted_at = NULL,
                version = COALESCE(version, 0) + 1
            WHERE scope = ?
              AND host_uuid = ?
              AND path = ?
            "#,
            params![scope, host_uuid, path],
        )?;
        self.connection.query_row(
            r#"
            SELECT id FROM sftp_favorites
            WHERE scope = ?
              AND host_uuid = ?
              AND path = ?
              AND deleted_at IS NULL
            ORDER BY id DESC
            "#,
            params![scope, host_uuid, path],
            |row| row.get(0),
        )
    }

    pub fn list_sftp_favorite_paths(
        &self,
        scope: String,
        host_id: Option<i64>,
    ) -> rusqlite::Result<Vec<SftpFavoritePathEntry>> {
        let scope = normalize_sftp_favorite_scope(&scope)?;
        let host_id = host_id.filter(|id| *id > 0).ok_or_else(|| {
            rusqlite::Error::InvalidParameterName("sftp favorites require a saved host".to_string())
        })?;
        let mut statement = self.connection.prepare(
            r#"
            SELECT
              sftp_favorites.*,
              (SELECT id FROM hosts WHERE hosts.uuid = sftp_favorites.host_uuid AND hosts.deleted_at IS NULL) AS host_id
            FROM sftp_favorites
            WHERE scope = ?
              AND host_uuid = (SELECT uuid FROM hosts WHERE id = ?)
              AND deleted_at IS NULL
            ORDER BY updated_at DESC, id DESC
            "#,
        )?;
        let favorites = statement
            .query_map(params![scope, host_id], sftp_favorite_path_from_row)?
            .collect();
        favorites
    }

    pub fn delete_sftp_favorite_path_by_target(
        &mut self,
        scope: String,
        host_id: Option<i64>,
        path: String,
    ) -> rusqlite::Result<usize> {
        let scope = normalize_sftp_favorite_scope(&scope)?;
        let host_id = host_id.filter(|id| *id > 0).ok_or_else(|| {
            rusqlite::Error::InvalidParameterName("sftp favorites require a saved host".to_string())
        })?;
        self.connection.execute(
            r#"
            UPDATE sftp_favorites
            SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                version = COALESCE(version, 0) + 1
            WHERE scope = ?
              AND host_uuid = (SELECT uuid FROM hosts WHERE id = ?)
              AND path = ?
              AND deleted_at IS NULL
            "#,
            params![scope, host_id, path.trim()],
        )
    }

    pub fn save_sftp_task_history(
        &mut self,
        task: &SftpTaskHistoryEntry,
        cutoff: i64,
    ) -> rusqlite::Result<i64> {
        validate_sftp_task_history(task)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM sftp_tasks WHERE finished_at < ?",
            params![cutoff],
        )?;
        let uuid = uuid_or_new(&transaction, task.uuid.as_deref())?;
        transaction.execute(
            r#"
            INSERT INTO sftp_tasks (
              uuid, host_uuid, transfer_type, host, username, port, display_name,
              source_path, target_path, item_kind, status, bytes, total_bytes,
              error_text, created_at, finished_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                uuid,
                task.host_uuid,
                normalize_sftp_task_type(&task.task_type),
                task.host,
                task.username,
                task.port,
                task.display_name,
                task.source_path,
                task.target_path,
                normalize_sftp_item_kind(&task.item_kind),
                task.status,
                task.bytes,
                task.total_bytes,
                task.error,
                task.created_at,
                task.finished_at,
            ],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.commit()?;
        Ok(id)
    }

    pub fn list_sftp_task_history(
        &mut self,
        cutoff: i64,
    ) -> rusqlite::Result<Vec<SftpTaskHistoryEntry>> {
        self.connection.execute(
            "DELETE FROM sftp_tasks WHERE finished_at < ?",
            params![cutoff],
        )?;
        let mut statement = self.connection.prepare(
            r#"
            SELECT id, uuid, host_uuid, transfer_type, host, username, port, status,
                   display_name, source_path, target_path, created_at,
                   finished_at, bytes, total_bytes, item_kind, error_text
            FROM sftp_tasks
            ORDER BY finished_at DESC, id DESC
            "#,
        )?;
        let tasks = statement
            .query_map([], sftp_task_history_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tasks)
    }

    pub fn delete_sftp_task_history(&mut self, id: i64) -> rusqlite::Result<usize> {
        self.connection
            .execute("DELETE FROM sftp_tasks WHERE id = ?", params![id])
    }

    pub fn clear_sftp_task_history(&mut self) -> rusqlite::Result<usize> {
        self.connection.execute("DELETE FROM sftp_tasks", [])
    }

    pub fn save_snippet_package(&mut self, package: &SnippetPackageEntry) -> rusqlite::Result<i64> {
        if let Some(id) = package.id {
            self.connection.execute(
                r#"
                UPDATE snippet_packages
                SET name = ?, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                    deleted_at = NULL,
                    version = COALESCE(version, 0) + 1
                WHERE id = ?
                "#,
                params![package.name, id],
            )?;
            return Ok(id);
        }

        let uuid = uuid_or_new(&self.connection, package.uuid.as_deref())?;
        self.connection.execute(
            r#"
            INSERT INTO snippet_packages (uuid, name, created_at, updated_at)
            VALUES (
              ?, ?, CAST(unixepoch('subsec') * 1000 AS INTEGER),
              CAST(unixepoch('subsec') * 1000 AS INTEGER)
            )
            "#,
            params![uuid, package.name],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn list_snippet_packages(&self) -> rusqlite::Result<Vec<SnippetPackageEntry>> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM snippet_packages WHERE deleted_at IS NULL ORDER BY id ASC")?;
        let packages = statement.query_map([], snippet_package_from_row)?.collect();
        packages
    }

    pub fn delete_snippet_package(&mut self, id: i64) -> rusqlite::Result<usize> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE snippets SET package_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE package_uuid = (SELECT uuid FROM snippet_packages WHERE id = ?)",
            params![id],
        )?;
        let deleted = transaction.execute(
            "UPDATE snippet_packages SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE id = ? AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn save_snippet(&mut self, snippet: &SnippetEntry) -> rusqlite::Result<i64> {
        let scope = normalize_snippet_scope(&snippet.scope);
        let target_group_ids = if scope == "targeted" {
            dedupe_ids(&snippet.target_group_ids)
        } else {
            Vec::new()
        };
        let target_host_ids = if scope == "targeted" {
            dedupe_ids(&snippet.target_host_ids)
        } else {
            Vec::new()
        };
        let package_uuid = relation_uuid(
            &self.connection,
            "snippet_packages",
            snippet.package_id,
            snippet.package_uuid.as_deref(),
        )?;
        let transaction = self.connection.transaction()?;
        let id = if let Some(id) = snippet.id {
            transaction.execute(
                r#"
                UPDATE snippets
                SET package_uuid = ?, scope = ?,
                    description = ?, script = ?,
                    updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
                    deleted_at = NULL,
                    version = COALESCE(version, 0) + 1
                WHERE id = ?
                "#,
                params![package_uuid, scope, snippet.description, snippet.script, id],
            )?;
            id
        } else {
            let uuid = uuid_or_new(&transaction, snippet.uuid.as_deref())?;
            transaction.execute(
                r#"
                INSERT INTO snippets (
                  uuid, package_uuid, scope, description, script,
                  created_at, updated_at
                )
                VALUES (
                  ?, ?, ?, ?, ?, CAST(unixepoch('subsec') * 1000 AS INTEGER),
                  CAST(unixepoch('subsec') * 1000 AS INTEGER)
                )
                "#,
                params![
                    uuid,
                    package_uuid,
                    scope,
                    snippet.description,
                    snippet.script
                ],
            )?;
            transaction.last_insert_rowid()
        };

        transaction.execute(
            "DELETE FROM snippet_target_groups WHERE snippet_uuid = (SELECT uuid FROM snippets WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "DELETE FROM snippet_target_hosts WHERE snippet_uuid = (SELECT uuid FROM snippets WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE groups SET startup_snippet_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE startup_snippet_uuid = (SELECT uuid FROM snippets WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "UPDATE hosts SET startup_snippet_uuid = NULL, updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE startup_snippet_uuid = (SELECT uuid FROM snippets WHERE id = ?)",
            params![id],
        )?;
        for group_id in target_group_ids {
            let group_uuid = lookup_uuid(&transaction, "groups", Some(group_id))?;
            if let Some(group_uuid) = group_uuid {
                transaction.execute(
                    "INSERT OR IGNORE INTO snippet_target_groups (snippet_uuid, group_uuid) VALUES ((SELECT uuid FROM snippets WHERE id = ?), ?)",
                    params![id, group_uuid],
                )?;
            }
        }
        for host_id in target_host_ids {
            let host_uuid = lookup_uuid(&transaction, "hosts", Some(host_id))?;
            if let Some(host_uuid) = host_uuid {
                transaction.execute(
                    "INSERT OR IGNORE INTO snippet_target_hosts (snippet_uuid, host_uuid) VALUES ((SELECT uuid FROM snippets WHERE id = ?), ?)",
                    params![id, host_uuid],
                )?;
            }
        }
        transaction.commit()?;
        Ok(id)
    }

    pub fn get_snippet(&self, id: i64) -> rusqlite::Result<Option<SnippetEntry>> {
        let mut snippet = self
            .connection
            .query_row(
                r#"
                SELECT
                  snippets.*,
                  (SELECT id FROM snippet_packages WHERE snippet_packages.uuid = snippets.package_uuid) AS package_id
                FROM snippets
                WHERE id = ? AND deleted_at IS NULL
                "#,
                params![id],
                snippet_from_row,
            )
            .optional()?;
        if let Some(snippet) = snippet.as_mut() {
            self.load_snippet_targets(snippet)?;
        }
        Ok(snippet)
    }

    pub fn list_snippets(&self, package_id: Option<i64>) -> rusqlite::Result<Vec<SnippetEntry>> {
        let mut snippets = if let Some(package_id) = package_id {
            let mut statement = self.connection.prepare(
                r#"
                SELECT
                  snippets.*,
                  (SELECT id FROM snippet_packages WHERE snippet_packages.uuid = snippets.package_uuid) AS package_id
                FROM snippets
                WHERE package_uuid = (SELECT uuid FROM snippet_packages WHERE id = ?)
                  AND deleted_at IS NULL
                ORDER BY description COLLATE NOCASE, id ASC
                "#,
            )?;
            let snippets = statement
                .query_map(params![package_id], snippet_from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            snippets
        } else {
            let mut statement = self.connection.prepare(
                r#"
                SELECT
                  snippets.*,
                  (SELECT id FROM snippet_packages WHERE snippet_packages.uuid = snippets.package_uuid) AS package_id
                FROM snippets
                WHERE deleted_at IS NULL
                ORDER BY description COLLATE NOCASE, id ASC
                "#,
            )?;
            let snippets = statement
                .query_map([], snippet_from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            snippets
        };

        for snippet in &mut snippets {
            self.load_snippet_targets(snippet)?;
        }
        Ok(snippets)
    }

    pub fn delete_snippet(&mut self, id: i64) -> rusqlite::Result<usize> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM snippet_target_groups WHERE snippet_uuid = (SELECT uuid FROM snippets WHERE id = ?)",
            params![id],
        )?;
        transaction.execute(
            "DELETE FROM snippet_target_hosts WHERE snippet_uuid = (SELECT uuid FROM snippets WHERE id = ?)",
            params![id],
        )?;
        let deleted = transaction.execute(
            "UPDATE snippets SET deleted_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER), version = COALESCE(version, 0) + 1 WHERE id = ? AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    fn load_snippet_targets(&self, snippet: &mut SnippetEntry) -> rusqlite::Result<()> {
        let Some(id) = snippet.id else {
            snippet.target_group_ids.clear();
            snippet.target_host_ids.clear();
            return Ok(());
        };

        let mut group_statement = self.connection.prepare(
            r#"
            SELECT groups.id
            FROM snippet_target_groups
            JOIN groups ON groups.uuid = snippet_target_groups.group_uuid
            WHERE snippet_target_groups.snippet_uuid = (SELECT uuid FROM snippets WHERE id = ?)
              AND groups.deleted_at IS NULL
            ORDER BY groups.id ASC
            "#,
        )?;
        snippet.target_group_ids = group_statement
            .query_map(params![id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut host_statement = self.connection.prepare(
            r#"
            SELECT hosts.id
            FROM snippet_target_hosts
            JOIN hosts ON hosts.uuid = snippet_target_hosts.host_uuid
            WHERE snippet_target_hosts.snippet_uuid = (SELECT uuid FROM snippets WHERE id = ?)
              AND hosts.deleted_at IS NULL
            ORDER BY hosts.id ASC
            "#,
        )?;
        snippet.target_host_ids = host_statement
            .query_map(params![id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(())
    }

    fn configure(connection: &Connection) -> rusqlite::Result<()> {
        connection.execute_batch(
            r#"
            PRAGMA foreign_keys = OFF;
            PRAGMA journal_mode = WAL;
            PRAGMA busy_timeout = 5000;
            "#,
        )
    }

    fn ensure_schema(connection: &mut Connection) -> rusqlite::Result<()> {
        let version: i32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version == 0 {
            connection.execute_batch("PRAGMA foreign_keys = OFF;")?;
            let transaction = connection.transaction()?;
            create_schema(&transaction)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            transaction.commit()?;
        } else {
            migrations::migrate_schema(connection, version)?;
        }

        let foreign_key_error = connection
            .query_row("PRAGMA foreign_key_check", [], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .optional()?;
        if let Some((table, row_id, parent)) = foreign_key_error {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "foreign key check failed for {table} row {row_id:?} referencing {parent}"
            )));
        }
        connection.execute_batch("PRAGMA foreign_keys = ON;")
    }

    fn handle_json_request(&mut self, request: &str) -> Result<Value, Box<dyn Error>> {
        let request: DatabaseRequest = serde_json::from_str(request)?;
        let data = match request {
            DatabaseRequest::SaveGroup { group } => json!(self.save_group(&group)?),
            DatabaseRequest::GetGroup { id } => json!(self.get_group(id)?),
            DatabaseRequest::ListGroups => json!(self.list_groups()?),
            DatabaseRequest::DeleteGroup { id } => json!(self.delete_group(id)?),
            DatabaseRequest::SaveKey { key } => json!(self.save_key(&key)?),
            DatabaseRequest::GetKey { id } => json!(self.get_key(id)?),
            DatabaseRequest::ListKeys => json!(self.list_keys()?),
            DatabaseRequest::DeleteKey { id } => json!(self.delete_key(id)?),
            DatabaseRequest::SaveIdentity { identity } => {
                json!(self.save_identity(&identity)?)
            }
            DatabaseRequest::GetIdentity { id } => json!(self.get_identity(id)?),
            DatabaseRequest::ListIdentities => json!(self.list_identities()?),
            DatabaseRequest::DeleteIdentity { id } => json!(self.delete_identity(id)?),
            DatabaseRequest::SaveTag { tag } => json!(self.save_tag(&tag)?),
            DatabaseRequest::GetTag { id } => json!(self.get_tag(id)?),
            DatabaseRequest::ListTags => json!(self.list_tags()?),
            DatabaseRequest::DeleteTag { id } => json!(self.delete_tag(id)?),
            DatabaseRequest::SaveHost { host } => json!(self.save_host(&host)?),
            DatabaseRequest::GetHost { id } => json!(self.get_host(id)?),
            DatabaseRequest::ListHosts { group_id } => json!(self.list_hosts(group_id)?),
            DatabaseRequest::DeleteHost { id } => json!(self.delete_host(id)?),
            DatabaseRequest::SavePortForward { port_forward } => {
                json!(self.save_port_forward(&port_forward)?)
            }
            DatabaseRequest::GetPortForward { id } => json!(self.get_port_forward(id)?),
            DatabaseRequest::ListPortForwards { connection_id } => {
                json!(self.list_port_forwards(connection_id)?)
            }
            DatabaseRequest::DeletePortForward { id } => json!(self.delete_port_forward(id)?),
            DatabaseRequest::SaveProxy { proxy } => json!(self.save_proxy(&proxy)?),
            DatabaseRequest::GetProxy { id } => json!(self.get_proxy(id)?),
            DatabaseRequest::ListProxies => json!(self.list_proxies()?),
            DatabaseRequest::DeleteProxy { id } => json!(self.delete_proxy(id)?),
            DatabaseRequest::SaveAiProvider { provider } => {
                json!(self.save_ai_provider(&provider)?)
            }
            DatabaseRequest::GetActiveAiProvider => json!(self.get_active_ai_provider()?),
            DatabaseRequest::ListAiProviders => json!(self.list_ai_providers()?),
            DatabaseRequest::DeleteAiProvider { id } => {
                json!(self.delete_ai_provider(id)?)
            }
            DatabaseRequest::SaveSftpFavoritePath { favorite } => {
                json!(self.save_sftp_favorite_path(&favorite)?)
            }
            DatabaseRequest::ListSftpFavoritePaths { scope, host_id } => {
                json!(self.list_sftp_favorite_paths(scope, host_id)?)
            }
            DatabaseRequest::DeleteSftpFavoritePathByTarget {
                scope,
                host_id,
                path,
            } => json!(self.delete_sftp_favorite_path_by_target(scope, host_id, path)?),
            DatabaseRequest::SaveSftpTaskHistory { task, cutoff } => {
                json!(self.save_sftp_task_history(&task, cutoff)?)
            }
            DatabaseRequest::ListSftpTaskHistory { cutoff } => {
                json!(self.list_sftp_task_history(cutoff)?)
            }
            DatabaseRequest::DeleteSftpTaskHistory { id } => {
                json!(self.delete_sftp_task_history(id)?)
            }
            DatabaseRequest::ClearSftpTaskHistory => {
                json!(self.clear_sftp_task_history()?)
            }
            DatabaseRequest::SaveSnippetPackage { package } => {
                json!(self.save_snippet_package(&package)?)
            }
            DatabaseRequest::ListSnippetPackages => json!(self.list_snippet_packages()?),
            DatabaseRequest::DeleteSnippetPackage { id } => {
                json!(self.delete_snippet_package(id)?)
            }
            DatabaseRequest::SaveSnippet { snippet } => json!(self.save_snippet(&snippet)?),
            DatabaseRequest::GetSnippet { id } => json!(self.get_snippet(id)?),
            DatabaseRequest::ListSnippets { package_id } => json!(self.list_snippets(package_id)?),
            DatabaseRequest::DeleteSnippet { id } => json!(self.delete_snippet(id)?),
            DatabaseRequest::SaveTerminalLog { log, events } => {
                json!(self.save_terminal_log(&log, &events)?)
            }
            DatabaseRequest::ListTerminalLogs { limit, offset } => {
                json!(self.list_terminal_logs(limit, offset)?)
            }
            DatabaseRequest::ListTerminalLogEvents { log_id } => {
                json!(self.list_terminal_log_events(&log_id)?)
            }
            DatabaseRequest::DeleteTerminalLog { log_id } => {
                json!(self.delete_terminal_log(&log_id)?)
            }
            DatabaseRequest::ClearTerminalLogs => json!(self.clear_terminal_logs()?),
            DatabaseRequest::ListTerminalCaptureFiles => {
                json!(self.list_terminal_capture_files()?)
            }
            DatabaseRequest::ListIncompleteTerminalCaptures => {
                json!(self.list_incomplete_terminal_captures()?)
            }
            DatabaseRequest::ClearMissingTerminalCapture { log_id } => {
                json!(self.clear_missing_terminal_capture(&log_id)?)
            }
            DatabaseRequest::FinalizeRecoveredTerminalCapture {
                log_id,
                capture_bytes,
                capture_sha256,
                ended_at,
            } => json!(self.finalize_recovered_terminal_capture(
                &log_id,
                capture_bytes,
                &capture_sha256,
                ended_at,
            )?),
            DatabaseRequest::SaveAiConversation { conversation } => {
                json!(self.save_ai_conversation(&conversation)?)
            }
            DatabaseRequest::GetAiConversation { uuid } => {
                json!(self.get_ai_conversation(&uuid)?)
            }
            DatabaseRequest::ListAiConversations {
                scope,
                host_uuid,
                limit,
            } => {
                json!(self.list_ai_conversations(scope.as_deref(), host_uuid.as_deref(), limit,)?)
            }
            DatabaseRequest::DeleteAiConversation { uuid } => {
                json!(self.delete_ai_conversation(&uuid)?)
            }
            DatabaseRequest::EncryptionStatus => {
                let cfg = crate::crypto::load_encryption_config(&self.connection)?;
                match cfg {
                    None => json!({"initialised": false, "has_master_key": false}),
                    Some(cfg) => json!({
                        "initialised": true,
                        "has_master_key": cfg.has_master_key,
                    }),
                }
            }
            DatabaseRequest::InitEncryption => {
                let dek = crate::crypto::initialise_encryption(&self.connection)?;
                let encrypted =
                    crate::crypto::encrypt_unprotected_sensitive_fields(&self.connection, &dek)?;
                let cfg = crate::crypto::load_encryption_config(&self.connection)?;
                json!({
                    "initialised": true,
                    "has_master_key": cfg.map(|c| c.has_master_key).unwrap_or(false),
                    "encrypted_fields": encrypted,
                })
            }
            DatabaseRequest::SetMasterKey { mut master_key } => {
                let result = crate::crypto::set_master_key(&self.connection, &master_key);
                master_key.zeroize();
                result?;
                json!({"ok": true})
            }
            DatabaseRequest::ChangeMasterKey {
                mut current_master_key,
                mut new_master_key,
            } => {
                let result = crate::crypto::change_master_key(
                    &self.connection,
                    &current_master_key,
                    &new_master_key,
                );
                current_master_key.zeroize();
                new_master_key.zeroize();
                result?;
                json!({"ok": true})
            }
            DatabaseRequest::RemoveMasterKey {
                mut current_master_key,
            } => {
                let result =
                    crate::crypto::remove_master_key(&self.connection, &current_master_key);
                current_master_key.zeroize();
                result?;
                json!({"ok": true})
            }
            DatabaseRequest::VerifyMasterKey { mut master_key } => {
                let result = crate::crypto::verify_master_key(&self.connection, &master_key);
                master_key.zeroize();
                json!({"ok": result?})
            }
            DatabaseRequest::GithubSaveToken { mut token } => {
                let result = self.save_github_pat(&token);
                token.zeroize();
                result?;
                json!({"ok": true})
            }
            DatabaseRequest::GithubLoadToken => json!({"has_token": self.has_github_pat()?}),
            DatabaseRequest::GithubReadToken => {
                let token = self.load_github_pat()?;
                json!({"token": token.as_deref()})
            }
            DatabaseRequest::GithubDeleteToken => {
                self.delete_github_pat()?;
                json!({"ok": true})
            }
            DatabaseRequest::GithubGistSaveToken { mut token } => {
                let result = self.save_github_gist_token(&token);
                token.zeroize();
                result?;
                json!({"ok": true})
            }
            DatabaseRequest::GithubGistLoadToken => {
                json!({"has_token": self.has_github_gist_token()?})
            }
            DatabaseRequest::GithubGistDeleteToken => {
                self.delete_github_gist_token()?;
                json!({"ok": true})
            }
            DatabaseRequest::GithubGistSaveConfig { config } => {
                self.save_github_gist_config(&config)?;
                json!({"ok": true})
            }
            DatabaseRequest::GithubGistLoadConfig => {
                json!(self.github_gist_config()?)
            }
            DatabaseRequest::GithubGistDeleteConfig => {
                self.delete_sync_provider_config("github_gist")?;
                json!({"ok": true})
            }
            DatabaseRequest::GithubGistSync {
                mut master_key,
                strategy,
                backup_count,
            } => {
                self.create_local_sync_backup(backup_count)?;
                let outcome = self.github_gist_sync(master_key.as_deref(), strategy);
                if let Some(master_key) = master_key.as_mut() {
                    master_key.zeroize();
                }
                json!(outcome?)
            }
            DatabaseRequest::GithubGistChangeMasterKey {
                mut current_master_key,
                mut new_master_key,
            } => {
                let outcome =
                    self.github_gist_change_master_key(&current_master_key, &new_master_key);
                current_master_key.zeroize();
                new_master_key.zeroize();
                json!(outcome?)
            }
            DatabaseRequest::GithubGistListHistory { limit } => {
                json!(self.github_gist_client()?.list_history(limit)?)
            }
            DatabaseRequest::GithubGistRestoreVersion {
                version,
                backup_count,
            } => {
                self.create_local_sync_backup(backup_count)?;
                self.github_gist_restore_version(&version)?
            }
            DatabaseRequest::SyncKeyStatus => {
                let has_sync_key: bool = self.connection.query_row(
                    "SELECT
                       EXISTS(SELECT 1 FROM app_metadata WHERE key = 'sync_dek')
                       AND EXISTS(SELECT 1 FROM app_metadata WHERE key = 'sync_envelope_header')",
                    [],
                    |row| row.get(0),
                )?;
                json!({"has_local_sync_key": has_sync_key})
            }
            DatabaseRequest::ForgetSyncKey => {
                self.connection.execute(
                    "DELETE FROM app_metadata
                     WHERE key IN ('sync_dek', 'sync_vault_id', 'sync_envelope_header')
                        OR key LIKE 'sync_revision:%'",
                    [],
                )?;
                self.clear_sync_snapshot()?;
                json!({"ok": true})
            }
            DatabaseRequest::GithubSaveConfig { config } => {
                crate::github_sync::validate_repository_url(&config.repository_url)?;
                let text = serde_json::to_string(&config)?;
                self.save_sync_provider_config(
                    "github_repository",
                    "github_repository",
                    "GitHub Repository",
                    &text,
                )?;
                json!({"ok": true})
            }
            DatabaseRequest::GithubLoadConfig => {
                let value = self.sync_provider_config("github_repository")?;
                match value {
                    None => Value::Null,
                    Some(text) => serde_json::from_str::<Value>(&text)?,
                }
            }
            DatabaseRequest::GithubDeleteConfig => {
                self.delete_sync_provider_config("github_repository")?;
                json!({"ok": true})
            }
            DatabaseRequest::GithubSync {
                mut master_key,
                strategy,
                backup_count,
            } => {
                self.create_local_sync_backup(backup_count)?;
                let outcome = self.github_sync(master_key.as_deref(), strategy);
                if let Some(master_key) = master_key.as_mut() {
                    master_key.zeroize();
                }
                json!(outcome?)
            }
            DatabaseRequest::GithubChangeMasterKey {
                mut current_master_key,
                mut new_master_key,
            } => {
                let outcome = self.github_change_master_key(&current_master_key, &new_master_key);
                current_master_key.zeroize();
                new_master_key.zeroize();
                json!(outcome?)
            }
            DatabaseRequest::GithubListHistory { limit } => {
                json!(self.github_client()?.list_history(limit)?)
            }
            DatabaseRequest::GithubRestoreRevision {
                commit_sha,
                backup_count,
            } => {
                self.create_local_sync_backup(backup_count)?;
                self.github_restore_revision(&commit_sha)?
            }
            DatabaseRequest::S3SaveCredentials {
                mut access_key_id,
                mut secret_access_key,
            } => {
                let result = self.save_s3_credentials(&crate::s3_sync::S3Credentials {
                    access_key_id: std::mem::take(&mut access_key_id),
                    secret_access_key: std::mem::take(&mut secret_access_key),
                });
                access_key_id.zeroize();
                secret_access_key.zeroize();
                result?;
                json!({"ok": true})
            }
            DatabaseRequest::S3LoadCredentials => {
                json!({"has_credentials": self.has_s3_credentials()?})
            }
            DatabaseRequest::S3ReadCredentials => json!(self.load_s3_credentials()?),
            DatabaseRequest::S3DeleteCredentials => {
                self.delete_s3_credentials()?;
                json!({"ok": true})
            }
            DatabaseRequest::S3SaveConfig { config } => {
                self.save_s3_config(&config)?;
                json!({"ok": true})
            }
            DatabaseRequest::S3LoadConfig => json!(self.s3_config()?),
            DatabaseRequest::S3DeleteConfig => {
                self.delete_sync_provider_config("s3")?;
                json!({"ok": true})
            }
            DatabaseRequest::S3Sync {
                mut master_key,
                strategy,
                backup_count,
            } => {
                self.create_local_sync_backup(backup_count)?;
                let outcome = self.s3_sync(master_key.as_deref(), strategy);
                if let Some(master_key) = master_key.as_mut() {
                    master_key.zeroize();
                }
                json!(outcome?)
            }
            DatabaseRequest::S3ListHistory { limit } => {
                json!(self.s3_client()?.list_versions(limit)?)
            }
            DatabaseRequest::S3RestoreVersion {
                version_id,
                backup_count,
            } => {
                self.create_local_sync_backup(backup_count)?;
                self.s3_restore_version(&version_id)?
            }
            DatabaseRequest::S3ChangeMasterKey {
                mut current_master_key,
                mut new_master_key,
            } => {
                let outcome = self.s3_change_master_key(&current_master_key, &new_master_key);
                current_master_key.zeroize();
                new_master_key.zeroize();
                json!(outcome?)
            }
            DatabaseRequest::CloudListProviders => json!(self.cloud_provider_summaries()?),
            DatabaseRequest::CloudLoadCredentials { provider_id } => {
                json!(self.load_cloud_credentials(&provider_id)?)
            }
            DatabaseRequest::SyncPreferences => self.sync_preferences_status()?,
            DatabaseRequest::RefreshRemoteSyncStatus => self.refresh_remote_sync_status()?,
            DatabaseRequest::SaveSyncPreferences { preferences } => {
                self.save_sync_preferences(&preferences)?;
                self.sync_preferences_status()?
            }
            DatabaseRequest::CloudSaveProvider {
                provider,
                credentials,
            } => {
                let provider = self.save_cloud_provider(provider)?;
                if let Some(credentials) = credentials {
                    self.save_cloud_credentials(&provider.id, &credentials)?;
                }
                json!(crate::cloud_sync::CloudProviderSummary {
                    has_credentials: self.has_cloud_credentials(&provider.id)?,
                    provider,
                })
            }
            DatabaseRequest::CloudDeleteProvider { provider_id } => {
                self.delete_cloud_credentials(&provider_id)?;
                json!({"deleted": self.delete_cloud_provider(&provider_id)?})
            }
            DatabaseRequest::CloudSync {
                provider_id,
                mut master_key,
                strategy,
                backup_count,
            } => {
                self.create_local_sync_backup(backup_count)?;
                let outcome =
                    self.cloud_provider_sync(&provider_id, master_key.as_deref(), strategy);
                if let Some(master_key) = master_key.as_mut() {
                    master_key.zeroize();
                }
                json!(outcome?)
            }
            DatabaseRequest::CloudListHistory { provider_id, limit } => {
                json!(self.cloud_provider_list_history(&provider_id, limit)?)
            }
            DatabaseRequest::CloudRestoreVersion {
                provider_id,
                version_id,
                backup_count,
            } => {
                self.create_local_sync_backup(backup_count)?;
                self.cloud_provider_restore_version(&provider_id, &version_id)?
            }
            DatabaseRequest::LocalSyncBackupList => self.local_sync_backups()?,
            DatabaseRequest::LocalSyncBackupRestore {
                backup_id,
                backup_count,
            } => self.restore_local_sync_backup(&backup_id, backup_count)?,
            DatabaseRequest::CloudChangeMasterKey {
                provider_id,
                mut current_master_key,
                mut new_master_key,
            } => {
                let outcome = self.cloud_provider_change_master_key(
                    &provider_id,
                    &current_master_key,
                    &new_master_key,
                );
                current_master_key.zeroize();
                new_master_key.zeroize();
                json!(outcome?)
            }
            DatabaseRequest::DeviceId => json!(self.device_id()?),
            DatabaseRequest::GetAppMeta { key } => json!(self.get_app_meta(&key)?),
            DatabaseRequest::SetAppMeta { key, value } => {
                self.set_app_meta(&key, &value)?;
                json!(())
            }
            DatabaseRequest::SchemaVersion => json!(self.schema_version()?),
        };
        Ok(data)
    }
}

pub fn default_database_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Nauterm")
                .join("nauterm.sqlite");
        }
    }

    if cfg!(target_os = "windows") {
        if let Some(app_data) = env::var_os("APPDATA") {
            return PathBuf::from(app_data)
                .join("Nauterm")
                .join("nauterm.sqlite");
        }
        if let Some(user_profile) = env::var_os("USERPROFILE") {
            return PathBuf::from(user_profile)
                .join("AppData")
                .join("Roaming")
                .join("Nauterm")
                .join("nauterm.sqlite");
        }
    }

    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(data_home)
            .join("nauterm")
            .join("nauterm.sqlite");
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("nauterm")
            .join("nauterm.sqlite");
    }

    PathBuf::from("nauterm.sqlite")
}

fn host_environment_variables_json(variables: &[HostEnvironmentVariable]) -> String {
    let variables = variables
        .iter()
        .filter(|entry| !entry.variable.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    serde_json::to_string(&variables).unwrap_or_else(|_| "[]".to_string())
}

fn host_environment_variables_from_json(value: Option<String>) -> Vec<HostEnvironmentVariable> {
    let Some(value) = value else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<HostEnvironmentVariable>>(&value)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !entry.variable.trim().is_empty())
        .collect()
}

fn normalize_optional_host_encoding(encoding: Option<&str>) -> Option<String> {
    encoding
        .map(str::trim)
        .filter(|encoding| !encoding.is_empty())
        .map(str::to_string)
}

fn normalize_optional_mosh_server_command(command: Option<&str>) -> Option<String> {
    command
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(str::to_string)
}

fn host_select_sql(where_clause: &str) -> String {
    format!(
        r#"
        SELECT
          hosts.*,
          COALESCE((
            SELECT json_group_array(tag_uuid)
            FROM (
              SELECT host_tags.tag_uuid
              FROM host_tags
              JOIN tags ON tags.uuid = host_tags.tag_uuid
              WHERE host_tags.host_uuid = hosts.uuid
                AND tags.deleted_at IS NULL
              ORDER BY host_tags.tag_uuid
            )
          ), '[]') AS tag_uuids,
          (SELECT id FROM groups WHERE groups.uuid = hosts.group_uuid AND groups.deleted_at IS NULL) AS group_id,
          (SELECT id FROM identities WHERE identities.uuid = hosts.identity_uuid AND identities.deleted_at IS NULL) AS identity_id,
          (SELECT id FROM proxies WHERE proxies.uuid = hosts.proxy_uuid AND proxies.deleted_at IS NULL) AS proxy_id,
          (SELECT id FROM snippets WHERE snippets.uuid = hosts.startup_snippet_uuid AND snippets.deleted_at IS NULL) AS startup_snippet_id,
          (SELECT id FROM identities WHERE identities.uuid = hosts.telnet_identity_uuid AND identities.deleted_at IS NULL) AS telnet_identity_id,
          (SELECT id FROM keys WHERE keys.uuid = hosts.key_uuid AND keys.deleted_at IS NULL) AS key_id
        FROM hosts
        {where_clause}
        "#
    )
}

fn host_summary_select_sql(where_clause: &str) -> String {
    format!(
        r#"
        SELECT
          hosts.id, hosts.uuid, hosts.name, hosts.group_uuid,
          hosts.identity_uuid, hosts.proxy_uuid,
          hosts.host, hosts.port, hosts.username,
          NULL AS password, hosts.theme_id, hosts.startup_snippet_uuid,
          hosts.ssh_enabled, hosts.mosh_enabled, hosts.mosh_server_command,
          hosts.telnet_enabled,
          hosts.telnet_identity_uuid, hosts.telnet_username,
          NULL AS telnet_password, hosts.telnet_port, hosts.telnet_theme_id,
          NULL AS environment_variables, hosts.encoding, hosts.telnet_encoding,
          hosts.type,
          hosts.key_uuid, hosts.shell_path, hosts.work_dir, hosts.os, hosts.distro,
          COALESCE((
            SELECT json_group_array(tag_uuid)
            FROM (
              SELECT host_tags.tag_uuid
              FROM host_tags
              JOIN tags ON tags.uuid = host_tags.tag_uuid
              WHERE host_tags.host_uuid = hosts.uuid
                AND tags.deleted_at IS NULL
              ORDER BY host_tags.tag_uuid
            )
          ), '[]') AS tag_uuids,
          hosts.created_at, hosts.updated_at, hosts.deleted_at,
          hosts.version, hosts.created_device_id, hosts.updated_device_id,
          (SELECT id FROM groups WHERE groups.uuid = hosts.group_uuid AND groups.deleted_at IS NULL) AS group_id,
          (SELECT id FROM identities WHERE identities.uuid = hosts.identity_uuid AND identities.deleted_at IS NULL) AS identity_id,
          (SELECT id FROM proxies WHERE proxies.uuid = hosts.proxy_uuid AND proxies.deleted_at IS NULL) AS proxy_id,
          (SELECT id FROM snippets WHERE snippets.uuid = hosts.startup_snippet_uuid AND snippets.deleted_at IS NULL) AS startup_snippet_id,
          (SELECT id FROM identities WHERE identities.uuid = hosts.telnet_identity_uuid AND identities.deleted_at IS NULL) AS telnet_identity_id,
          (SELECT id FROM keys WHERE keys.uuid = hosts.key_uuid AND keys.deleted_at IS NULL) AS key_id
        FROM hosts
        {where_clause}
        "#
    )
}

pub(crate) fn tag_uuids_json(tag_uuids: &[String]) -> String {
    let unique = tag_uuids
        .iter()
        .map(|uuid| uuid.trim())
        .filter(|uuid| is_uuid_value(uuid))
        .collect::<BTreeSet<_>>();
    serde_json::to_string(&unique).unwrap_or_else(|_| "[]".to_string())
}

fn replace_host_tags(
    connection: &Connection,
    host_id: i64,
    tag_uuids: &[String],
) -> rusqlite::Result<()> {
    let host_uuid: String = connection.query_row(
        "SELECT uuid FROM hosts WHERE id = ?",
        params![host_id],
        |row| row.get(0),
    )?;
    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "DELETE FROM host_tags WHERE host_uuid = ?",
        params![host_uuid],
    )?;
    for tag_uuid in tag_uuids
        .iter()
        .map(|uuid| uuid.trim())
        .filter(|uuid| is_uuid_value(uuid))
        .collect::<BTreeSet<_>>()
    {
        transaction.execute(
            r#"
            INSERT OR IGNORE INTO host_tags (host_uuid, tag_uuid)
            SELECT ?, ? WHERE EXISTS (
              SELECT 1 FROM tags
              WHERE uuid = ? AND deleted_at IS NULL
            )
            "#,
            params![host_uuid, tag_uuid, tag_uuid],
        )?;
    }
    transaction.commit()
}

fn tag_uuids_from_json(raw: String) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(&raw)
        .unwrap_or_default()
        .into_iter()
        .map(|uuid| uuid.trim().to_string())
        .filter(|uuid| is_uuid_value(uuid))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_sftp_task_type(value: &str) -> &str {
    match value {
        "transferDownload" => "download",
        "transferUpload" => "upload",
        value => value,
    }
}

fn normalize_sftp_item_kind(value: &str) -> &str {
    match value {
        "file" | "folder" => value,
        "directory" => "folder",
        _ => "unknown",
    }
}

fn validate_sftp_task_history(task: &SftpTaskHistoryEntry) -> rusqlite::Result<()> {
    if !matches!(
        normalize_sftp_task_type(&task.task_type),
        "download" | "upload" | "edit" | "move" | "copy" | "delete"
    ) {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "unsupported SFTP task type: {}",
            task.task_type
        )));
    }
    if !matches!(task.status.as_str(), "completed" | "failed" | "cancelled") {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "unsupported SFTP task status: {}",
            task.status
        )));
    }
    if !(1..=65535).contains(&task.port)
        || task.bytes < 0
        || task.total_bytes < 0
        || task.created_at < 0
        || task.finished_at < task.created_at
    {
        return Err(rusqlite::Error::InvalidParameterName(
            "invalid SFTP task history values".to_string(),
        ));
    }
    Ok(())
}

fn generate_uuid_v7(connection: &Connection) -> rusqlite::Result<String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    let random: Vec<u8> = connection.query_row("SELECT randomblob(10)", [], |row| row.get(0))?;
    let mut bytes = [0u8; 16];
    bytes[0] = ((millis >> 40) & 0xff) as u8;
    bytes[1] = ((millis >> 32) & 0xff) as u8;
    bytes[2] = ((millis >> 24) & 0xff) as u8;
    bytes[3] = ((millis >> 16) & 0xff) as u8;
    bytes[4] = ((millis >> 8) & 0xff) as u8;
    bytes[5] = (millis & 0xff) as u8;
    bytes[6] = 0x70 | (random.first().copied().unwrap_or_default() & 0x0f);
    bytes[7] = random.get(1).copied().unwrap_or_default();
    bytes[8] = 0x80 | (random.get(2).copied().unwrap_or_default() & 0x3f);
    bytes[9..16].copy_from_slice(&random[3..10]);
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    ))
}

fn normalize_uuid(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| is_uuid_value(value))
        .map(str::to_string)
}

fn uuid_or_new(connection: &Connection, value: Option<&str>) -> rusqlite::Result<String> {
    match normalize_uuid(value) {
        Some(value) => Ok(value),
        None => generate_uuid_v7(connection),
    }
}

fn is_uuid_value(value: &str) -> bool {
    value.len() == 36
        && value
            .as_bytes()
            .iter()
            .enumerate()
            .all(|(index, byte)| match index {
                8 | 13 | 18 | 23 => *byte == b'-',
                _ => byte.is_ascii_hexdigit(),
            })
}

fn lookup_uuid(
    connection: &Connection,
    table: &str,
    id: Option<i64>,
) -> rusqlite::Result<Option<String>> {
    let Some(id) = id.filter(|id| *id > 0) else {
        return Ok(None);
    };
    connection
        .query_row(
            &format!("SELECT uuid FROM {table} WHERE id = ?"),
            params![id],
            |row| row.get(0),
        )
        .optional()
}

fn relation_uuid(
    connection: &Connection,
    table: &str,
    id: Option<i64>,
    uuid: Option<&str>,
) -> rusqlite::Result<Option<String>> {
    if let Some(uuid) = normalize_uuid(uuid) {
        return Ok(Some(uuid));
    }
    lookup_uuid(connection, table, id)
}

fn delete_missing_ai_children(
    connection: &Connection,
    table: &str,
    conversation_uuid: &str,
    active_uuids: &BTreeSet<String>,
) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(&format!(
        "SELECT uuid FROM {table} WHERE conversation_uuid = ?"
    ))?;
    let existing = statement
        .query_map(params![conversation_uuid], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    for uuid in existing {
        if !active_uuids.contains(&uuid) {
            connection.execute(
                &format!("DELETE FROM {table} WHERE uuid = ?"),
                params![uuid],
            )?;
        }
    }
    Ok(())
}

fn json_values_from_row(row: &Row<'_>, column: &str) -> rusqlite::Result<Vec<Value>> {
    let raw: String = row.get(column)?;
    serde_json::from_str(&raw)
        .map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error)))
}

fn optional_json_value_from_row(row: &Row<'_>, column: &str) -> rusqlite::Result<Option<Value>> {
    let raw: Option<String> = row.get(column)?;
    raw.map(|value| {
        serde_json::from_str(&value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error))
        })
    })
    .transpose()
}

fn ai_conversation_from_row(row: &Row<'_>) -> rusqlite::Result<AiConversationEntry> {
    Ok(AiConversationEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        title: row.get("title")?,
        preview: None,
        scope: row.get("scope")?,
        host_uuid: row.get("host_uuid")?,
        provider_uuid: row.get("provider_uuid")?,
        model: row.get::<_, String>("model").unwrap_or_default(),
        messages: Vec::new(),
        command_blocks: Vec::new(),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        version: None,
        created_device_id: None,
        updated_device_id: None,
    })
}

fn ai_conversation_summary_from_row(row: &Row<'_>) -> rusqlite::Result<AiConversationEntry> {
    let mut conversation = ai_conversation_from_row(row)?;
    conversation.preview = row.get("preview")?;
    Ok(conversation)
}

fn ai_message_from_row(row: &Row<'_>) -> rusqlite::Result<AiMessageEntry> {
    Ok(AiMessageEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        role: row.get("role")?,
        content: row.get("content")?,
        context: row.get("context")?,
        sequence: row.get("sequence")?,
        tool_calls: json_values_from_row(row, "tool_calls")?,
        tool_result: optional_json_value_from_row(row, "tool_result")?,
        attachments: json_values_from_row(row, "attachments")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        version: None,
    })
}

fn ai_command_block_from_row(row: &Row<'_>) -> rusqlite::Result<AiCommandBlockEntry> {
    Ok(AiCommandBlockEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        tool_call_id: row.get("tool_call_id")?,
        command: row.get("command")?,
        explanation: row.get("explanation")?,
        status: row.get("status")?,
        sequence: row.get("sequence")?,
        output: row.get("output")?,
        exit_code: row.get("exit_code")?,
        error: row.get("error")?,
        started_at: row.get("started_at")?,
        finished_at: row.get("finished_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        version: None,
    })
}

fn group_from_row(row: &Row<'_>) -> rusqlite::Result<HostGroup> {
    Ok(HostGroup {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        name: row.get("name")?,
        parent_id: row.get("parent_id")?,
        parent_uuid: row.get("parent_uuid")?,
        identity_id: row.get("identity_id")?,
        identity_uuid: row.get("identity_uuid")?,
        proxy_id: row.get("proxy_id")?,
        proxy_uuid: row.get("proxy_uuid")?,
        port: row.get("port")?,
        username: row.get("username")?,
        password: row.get("password")?,
        theme_id: row.get("theme_id")?,
        startup_snippet_id: row.get("startup_snippet_id")?,
        startup_snippet_uuid: row.get("startup_snippet_uuid")?,
        ssh_enabled: row.get("ssh_enabled")?,
        mosh_enabled: row.get("mosh_enabled")?,
        mosh_server_command: normalize_optional_mosh_server_command(
            row.get::<_, Option<String>>("mosh_server_command")?
                .as_deref(),
        ),
        telnet_enabled: row.get("telnet_enabled")?,
        telnet_identity_id: row.get("telnet_identity_id")?,
        telnet_identity_uuid: row.get("telnet_identity_uuid")?,
        telnet_username: row.get("telnet_username")?,
        telnet_password: row.get("telnet_password")?,
        telnet_port: row.get("telnet_port")?,
        telnet_theme_id: row.get("telnet_theme_id")?,
        environment_variables: host_environment_variables_from_json(
            row.get("environment_variables")?,
        ),
        encoding: normalize_optional_host_encoding(
            row.get::<_, Option<String>>("encoding")?.as_deref(),
        ),
        telnet_encoding: normalize_optional_host_encoding(
            row.get::<_, Option<String>>("telnet_encoding")?.as_deref(),
        ),
        key_id: row.get("key_id")?,
        key_uuid: row.get("key_uuid")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
        version: row.get("version")?,
        created_device_id: row.get("created_device_id")?,
        updated_device_id: row.get("updated_device_id")?,
    })
}

fn key_from_row(row: &Row<'_>) -> rusqlite::Result<KeyEntry> {
    Ok(KeyEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        name: row.get("name")?,
        private_key: row.get("private_key")?,
        public_key: row.get("public_key")?,
        certificate: row.get("certificate")?,
        passphrase: row.get("passphrase")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
        version: row.get("version")?,
        created_device_id: row.get("created_device_id")?,
        updated_device_id: row.get("updated_device_id")?,
    })
}

fn identity_from_row(row: &Row<'_>) -> rusqlite::Result<IdentityEntry> {
    Ok(IdentityEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        name: row.get("name")?,
        username: row.get("username")?,
        password: row.get("password")?,
        key_id: row.get("key_id")?,
        key_uuid: row.get("key_uuid")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
        version: row.get("version")?,
        created_device_id: row.get("created_device_id")?,
        updated_device_id: row.get("updated_device_id")?,
    })
}

fn tag_from_row(row: &Row<'_>) -> rusqlite::Result<TagEntry> {
    Ok(TagEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        name: row.get("name")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
        version: row.get("version")?,
        created_device_id: row.get("created_device_id")?,
        updated_device_id: row.get("updated_device_id")?,
    })
}

fn host_from_row(row: &Row<'_>) -> rusqlite::Result<HostEntry> {
    let host_type: String = row.get("type")?;
    Ok(HostEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        name: row.get("name")?,
        group_id: row.get("group_id")?,
        group_uuid: row.get("group_uuid")?,
        identity_id: row.get("identity_id")?,
        identity_uuid: row.get("identity_uuid")?,
        proxy_id: row.get("proxy_id")?,
        proxy_uuid: row.get("proxy_uuid")?,
        host: row.get("host")?,
        port: row.get("port")?,
        username: row.get("username")?,
        password: row.get("password")?,
        theme_id: row.get("theme_id")?,
        startup_snippet_id: row.get("startup_snippet_id")?,
        startup_snippet_uuid: row.get("startup_snippet_uuid")?,
        ssh_enabled: row.get("ssh_enabled")?,
        mosh_enabled: row.get("mosh_enabled")?,
        mosh_server_command: normalize_optional_mosh_server_command(
            row.get::<_, Option<String>>("mosh_server_command")?
                .as_deref(),
        ),
        telnet_enabled: row.get("telnet_enabled")?,
        telnet_identity_id: row.get("telnet_identity_id")?,
        telnet_identity_uuid: row.get("telnet_identity_uuid")?,
        telnet_username: row.get("telnet_username")?,
        telnet_password: row.get("telnet_password")?,
        telnet_port: row.get("telnet_port")?,
        telnet_theme_id: row.get("telnet_theme_id")?,
        environment_variables: host_environment_variables_from_json(
            row.get("environment_variables")?,
        ),
        encoding: normalize_optional_host_encoding(
            row.get::<_, Option<String>>("encoding")?.as_deref(),
        ),
        telnet_encoding: normalize_optional_host_encoding(
            row.get::<_, Option<String>>("telnet_encoding")?.as_deref(),
        ),
        host_type: HostType::from_str(&host_type)?,
        key_id: row.get("key_id")?,
        key_uuid: row.get("key_uuid")?,
        shell_path: row.get("shell_path")?,
        work_dir: row.get("work_dir")?,
        os: row.get("os")?,
        distro: row.get("distro")?,
        tag_uuids: tag_uuids_from_json(row.get("tag_uuids")?),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
        version: row.get("version")?,
        created_device_id: row.get("created_device_id")?,
        updated_device_id: row.get("updated_device_id")?,
    })
}

fn port_forward_from_row(row: &Row<'_>) -> rusqlite::Result<PortForwardEntry> {
    Ok(PortForwardEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        name: row.get("name")?,
        r#type: row.get("type")?,
        bind_address: row.get("bind_address")?,
        bind_port: row.get("bind_port")?,
        destination_host: row.get("destination_host")?,
        destination_port: row.get("destination_port")?,
        connection_id: row.get("connection_id")?,
        host_uuid: row.get("host_uuid")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
        version: row.get("version")?,
        created_device_id: row.get("created_device_id")?,
        updated_device_id: row.get("updated_device_id")?,
    })
}

fn proxy_from_row(row: &Row<'_>) -> rusqlite::Result<ProxyEntry> {
    Ok(ProxyEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        name: row.get("name")?,
        r#type: row.get("type")?,
        host: row.get("host")?,
        port: row.get("port")?,
        identity_id: row.get("identity_id")?,
        identity_uuid: row.get("identity_uuid")?,
        username: row.get("username")?,
        password: row.get("password")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
        version: row.get("version")?,
        created_device_id: row.get("created_device_id")?,
        updated_device_id: row.get("updated_device_id")?,
    })
}

fn ai_provider_from_row(row: &Row<'_>) -> rusqlite::Result<AiProviderEntry> {
    let config_json: String = row.get("config")?;
    let config = serde_json::from_str(&config_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error))
    })?;
    Ok(AiProviderEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        name: row.get("name")?,
        protocol: row.get("protocol")?,
        base_url: row.get("base_url")?,
        model: row.get("model")?,
        api_key: row.get("api_key")?,
        config,
        active: row.get("active")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        version: None,
        created_device_id: None,
        updated_device_id: None,
    })
}

fn sftp_favorite_path_from_row(row: &Row<'_>) -> rusqlite::Result<SftpFavoritePathEntry> {
    Ok(SftpFavoritePathEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        scope: row.get("scope")?,
        host_id: row.get("host_id")?,
        host_uuid: row.get("host_uuid")?,
        path: row.get("path")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
        version: row.get("version")?,
        created_device_id: row.get("created_device_id")?,
        updated_device_id: row.get("updated_device_id")?,
    })
}

fn sftp_task_history_from_row(row: &Row<'_>) -> rusqlite::Result<SftpTaskHistoryEntry> {
    Ok(SftpTaskHistoryEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        host_uuid: row.get("host_uuid")?,
        task_type: row.get("transfer_type")?,
        host: row.get("host")?,
        username: row.get("username")?,
        port: row.get("port")?,
        status: row.get("status")?,
        display_name: row.get("display_name")?,
        source_path: row.get("source_path")?,
        target_path: row.get("target_path")?,
        created_at: row.get("created_at")?,
        finished_at: row.get("finished_at")?,
        bytes: row.get("bytes")?,
        total_bytes: row.get("total_bytes")?,
        item_kind: row.get("item_kind")?,
        error: row.get("error_text")?,
    })
}

fn snippet_package_from_row(row: &Row<'_>) -> rusqlite::Result<SnippetPackageEntry> {
    Ok(SnippetPackageEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        name: row.get("name")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
        version: row.get("version")?,
        created_device_id: row.get("created_device_id")?,
        updated_device_id: row.get("updated_device_id")?,
    })
}

fn snippet_from_row(row: &Row<'_>) -> rusqlite::Result<SnippetEntry> {
    Ok(SnippetEntry {
        id: row.get("id")?,
        uuid: row.get("uuid")?,
        package_id: row.get("package_id")?,
        package_uuid: row.get("package_uuid")?,
        scope: row.get("scope")?,
        description: row.get("description")?,
        script: row.get("script")?,
        target_group_ids: Vec::new(),
        target_host_ids: Vec::new(),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
        version: row.get("version")?,
        created_device_id: row.get("created_device_id")?,
        updated_device_id: row.get("updated_device_id")?,
    })
}

fn normalize_sftp_favorite_scope(value: &str) -> rusqlite::Result<&'static str> {
    match value {
        "remote" => Ok("remote"),
        _ => Err(rusqlite::Error::InvalidParameterName(format!(
            "unknown sftp favorite scope: {value}"
        ))),
    }
}

fn normalize_snippet_scope(value: &str) -> &'static str {
    match value {
        "targeted" => "targeted",
        _ => "global",
    }
}

fn dedupe_ids(ids: &[i64]) -> Vec<i64> {
    ids.iter()
        .copied()
        .filter(|id| *id > 0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn terminal_log_from_row(row: &Row<'_>) -> rusqlite::Result<TerminalLogEntry> {
    Ok(TerminalLogEntry {
        id: row.get("id")?,
        local_id: row.get("local_id")?,
        title: row.get("title")?,
        theme_id: row.get("theme_id")?,
        host_id: row.get("host_id")?,
        host_uuid: row.get("host_uuid")?,
        host: row.get("host")?,
        port: row.get("port")?,
        username: row.get("username")?,
        shell_path: row.get("shell_path")?,
        work_dir: row.get("work_dir")?,
        cwd: row.get("cwd")?,
        capture_file: row.get("capture_file")?,
        capture_bytes: row.get("capture_bytes")?,
        capture_sha256: row.get("capture_sha256")?,
        columns: row.get("columns")?,
        rows: row.get("rows")?,
        started_at: row.get("started_at")?,
        ended_at: row.get("ended_at")?,
    })
}

fn terminal_log_event_from_row(row: &Row<'_>) -> rusqlite::Result<TerminalLogEvent> {
    Ok(TerminalLogEvent {
        id: row.get("id")?,
        log_id: row.get("log_id")?,
        log_uuid: row.get("log_uuid")?,
        timestamp: row.get("timestamp")?,
        event_type: row.get("type")?,
        message: row.get("message")?,
        connection_kind: row.get("connection_kind")?,
        data: row.get("data")?,
    })
}

fn guard<T>(fallback: T, work: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or(fallback)
}

fn response_ok(data: Value) -> *mut c_char {
    response_json(json!({"ok": true, "data": data}))
}

fn response_error(error: impl ToString) -> *mut c_char {
    response_json(json!({"ok": false, "error": error.to_string()}))
}

fn response_json(value: Value) -> *mut c_char {
    let text = serde_json::to_string(&value)
        .unwrap_or_else(|_| r#"{"ok":false,"error":"failed to serialize response"}"#.to_string());
    CString::new(text)
        .unwrap_or_else(|_| CString::new(r#"{"ok":false,"error":"invalid response"}"#).unwrap())
        .into_raw()
}

unsafe fn cstr_to_str<'a>(value: *const c_char) -> Result<&'a str, Box<dyn Error>> {
    if value.is_null() {
        return Err("null string pointer".into());
    }
    Ok(unsafe { CStr::from_ptr(value) }.to_str()?)
}

/// # Safety
///
/// The returned handle must be released with `nauterm_database_destroy`.
#[no_mangle]
pub extern "C" fn nauterm_database_open_default() -> *mut NautermDatabase {
    guard(ptr::null_mut(), || match NautermDatabase::open_default() {
        Ok(database) => Box::into_raw(Box::new(database)),
        Err(_) => ptr::null_mut(),
    })
}

/// # Safety
///
/// `path` must point to a valid UTF-8 C string. The returned handle must be
/// released with `nauterm_database_destroy`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_database_open_path(path: *const c_char) -> *mut NautermDatabase {
    guard(ptr::null_mut(), || {
        let Ok(path) = (unsafe { cstr_to_str(path) }) else {
            return ptr::null_mut();
        };
        match NautermDatabase::open(path) {
            Ok(database) => Box::into_raw(Box::new(database)),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// # Safety
///
/// `handle` must either be null or a pointer returned by
/// `nauterm_database_open_default` or `nauterm_database_open_path`. After this
/// call returns, the handle must not be used again.
#[no_mangle]
pub unsafe extern "C" fn nauterm_database_destroy(handle: *mut NautermDatabase) {
    guard((), || {
        if !handle.is_null() {
            drop(unsafe { Box::from_raw(handle) });
        }
    });
}

/// # Safety
///
/// `handle` must be a live database pointer. `request` must point to a valid
/// UTF-8 JSON C string. The returned string must be released with
/// `nauterm_string_free`.
#[no_mangle]
pub unsafe extern "C" fn nauterm_database_call(
    handle: *mut NautermDatabase,
    request: *const c_char,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if handle.is_null() {
            return response_error("null database handle");
        }
        let request = match unsafe { cstr_to_str(request) } {
            Ok(value) => value,
            Err(error) => return response_error(error),
        };

        let database = unsafe { &mut *handle };
        match database.handle_json_request(request) {
            Ok(data) => response_ok(data),
            Err(error) => response_error(error),
        }
    })
}

#[no_mangle]
pub extern "C" fn nauterm_database_default_path() -> *mut c_char {
    guard(ptr::null_mut(), || {
        CString::new(default_database_path().to_string_lossy().as_bytes())
            .unwrap()
            .into_raw()
    })
}

/// # Safety
///
/// `value` must either be null or a string returned by this library.
#[no_mangle]
pub unsafe extern "C" fn nauterm_string_free(value: *mut c_char) {
    guard((), || {
        if !value.is_null() {
            drop(unsafe { CString::from_raw(value) });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_exists(connection: &Connection, table: &str) -> rusqlite::Result<bool> {
        connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?
             )",
            params![table],
            |row| row.get(0),
        )
    }

    fn table_column_type(
        connection: &Connection,
        table: &str,
        column: &str,
    ) -> rusqlite::Result<Option<String>> {
        let mut statement = connection.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
        let columns = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>("name")?, row.get::<_, String>("type")?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(columns
            .into_iter()
            .find_map(|(name, kind)| (name == column).then_some(kind)))
    }

    fn table_column_exists(
        connection: &Connection,
        table: &str,
        column: &str,
    ) -> rusqlite::Result<bool> {
        Ok(table_column_type(connection, table, column)?.is_some())
    }

    fn table_sql(connection: &Connection, table: &str) -> rusqlite::Result<String> {
        connection.query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?",
            params![table],
            |row| row.get(0),
        )
    }

    #[test]
    fn opens_encrypted_file_database() {
        let path = std::env::temp_dir().join(format!(
            "nauterm-database-open-{}-{}.sqlite",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        {
            let database = NautermDatabase::open(&path).unwrap();
            assert_eq!(database.schema_version().unwrap(), SCHEMA_VERSION);
            let expected_memory_security = if cfg!(windows) { "0" } else { "1" };
            assert_eq!(
                database
                    .connection
                    .query_row("PRAGMA cipher_memory_security", [], |row| row
                        .get::<_, String>(0))
                    .unwrap(),
                expected_memory_security
            );
        }
        let reopened = NautermDatabase::open(&path).unwrap();
        assert_eq!(reopened.schema_version().unwrap(), SCHEMA_VERSION);
        let expected_memory_security = if cfg!(windows) { "0" } else { "1" };
        assert_eq!(
            reopened
                .connection
                .query_row("PRAGMA cipher_memory_security", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            expected_memory_security
        );
        drop(reopened);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn initializes_schema() {
        let database = NautermDatabase::open_in_memory().unwrap();
        assert_eq!(database.schema_version().unwrap(), SCHEMA_VERSION);
        assert!(table_exists(&database.connection, "sftp_tasks").unwrap());
        assert_eq!(
            table_column_type(&database.connection, "hosts", "created_at")
                .unwrap()
                .as_deref(),
            Some("INTEGER")
        );
        assert_eq!(
            database
                .connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        for table in [
            "ai_providers",
            "ai_conversations",
            "ai_messages",
            "ai_command_blocks",
        ] {
            assert!(!table_column_exists(&database.connection, table, "version").unwrap());
            assert!(
                !table_column_exists(&database.connection, table, "updated_device_id").unwrap()
            );
        }
        assert!(
            !table_column_exists(&database.connection, "ai_conversations", "session_id").unwrap()
        );
    }

    #[test]
    fn enum_values_are_enforced_in_code_instead_of_sqlite() {
        let database = NautermDatabase::open_in_memory().unwrap();
        for (table, column) in [
            ("hosts", "type IN"),
            ("port_forwards", "type IN"),
            ("proxies", "type IN"),
            ("sftp_favorites", "scope ="),
            ("snippets", "scope IN"),
            ("sftp_tasks", "transfer_type IN"),
            ("sftp_tasks", "item_kind IN"),
            ("sftp_tasks", "status IN"),
            ("ai_conversations", "scope IN"),
            ("ai_messages", "role IN"),
            ("ai_command_blocks", "status IN"),
        ] {
            assert!(
                !table_sql(&database.connection, table)
                    .unwrap()
                    .contains(column),
                "{table}.{column} should not be constrained by SQLite"
            );
        }

        database
            .connection
            .execute(
                "INSERT INTO hosts (uuid, name, type) VALUES (?, ?, ?)",
                params![
                    "01979f62-8548-7000-8000-000000000201",
                    "Future host",
                    "future"
                ],
            )
            .unwrap();
        assert_eq!(
            database
                .connection
                .query_row("SELECT type FROM hosts", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "future"
        );

        assert!(normalize_port_forward_type("future").is_err());
        assert!(normalize_proxy_type("future").is_err());
        assert!(normalize_ai_conversation_scope("future").is_err());
        assert!(normalize_ai_message_role("future").is_err());
        assert!(normalize_ai_command_status("future").is_err());
    }

    #[test]
    fn migrates_v2_enum_constraints_without_losing_data_or_triggers() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        database
            .connection
            .execute(
                "INSERT INTO hosts (uuid, name, type) VALUES (?, ?, ?)",
                params![
                    "01979f62-8548-7000-8000-000000000202",
                    "Existing host",
                    "remote"
                ],
            )
            .unwrap();
        database
            .connection
            .execute_batch(
                "PRAGMA foreign_keys = OFF;
                 ALTER TABLE keys DROP COLUMN passphrase;
                 PRAGMA user_version = 2;",
            )
            .unwrap();

        migrations::migrate_schema(&mut database.connection, 2).unwrap();

        assert_eq!(database.schema_version().unwrap(), SCHEMA_VERSION);
        assert!(table_sql(&database.connection, "keys")
            .unwrap()
            .contains("passphrase TEXT"));
        assert_eq!(
            database
                .connection
                .query_row("PRAGMA legacy_alter_table", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
        let stale_schema_references = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE sql LIKE '%_v2%'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(stale_schema_references, 0);
        assert_eq!(
            database
                .connection
                .query_row("SELECT name FROM hosts", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "Existing host"
        );
        database
            .connection
            .execute(
                "INSERT INTO hosts (uuid, name, type) VALUES (?, ?, ?)",
                params!["01979f62-8548-7000-8000-000000000203", "New host", "future"],
            )
            .unwrap();
        assert!(database
            .connection
            .query_row(
                "SELECT created_device_id FROM hosts WHERE name = 'New host'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .unwrap()
            .is_some());
        assert!(!table_sql(&database.connection, "hosts")
            .unwrap()
            .contains("type IN"));
    }

    #[test]
    fn forgetting_sync_key_clears_only_local_unlock_state() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        database
            .connection
            .execute_batch(
                r#"
                INSERT INTO app_metadata (key, value) VALUES
                  ('sync_dek', 'sealed-dek'),
                  ('sync_vault_id', 'vault-id'),
                  ('sync_envelope_header', 'header'),
                  ('sync_revision:test:vault-id', '9');
                "#,
            )
            .unwrap();
        database.save_github_pat("provider-credential").unwrap();
        database
            .save_sync_preferences(&crate::cloud_sync::SyncPreferences {
                active_provider_id: Some("github_repository".to_string()),
                sync_snapshot: Some(crate::cloud_sync::SyncSnapshotStatus {
                    revision: 9,
                    snapshot_id: "snapshot-9".to_string(),
                }),
            })
            .unwrap();

        database
            .handle_json_request(r#"{"op":"forget_sync_key"}"#)
            .unwrap();

        assert!(database.has_github_pat().unwrap());
        let remaining_unlock_rows: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM app_metadata
                 WHERE key IN ('sync_dek', 'sync_vault_id', 'sync_envelope_header')
                    OR key LIKE 'sync_revision:%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining_unlock_rows, 0);
        assert!(database.sync_preferences().unwrap().sync_snapshot.is_none());
        assert_eq!(
            database
                .sync_preferences()
                .unwrap()
                .active_provider_id
                .as_deref(),
            Some("github_repository")
        );
    }

    #[test]
    fn s3_config_round_trips_through_the_database_request_api() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        database
            .handle_json_request(
                r#"{"op":"s3_save_config","config":{"endpoint":"https://storage.example.com","region":"auto","bucket":"vault","prefix":"folder","filename":"nauterm-sync.enc"}}"#,
            )
            .unwrap();
        let loaded = database
            .handle_json_request(r#"{"op":"s3_load_config"}"#)
            .unwrap();
        assert_eq!(loaded["endpoint"], "https://storage.example.com");
        assert_eq!(loaded["region"], "auto");
        assert_eq!(loaded["bucket"], "vault");
        assert_eq!(loaded["prefix"], "folder");
        assert_eq!(loaded["filename"], "nauterm-sync.enc");
        database
            .handle_json_request(
                r#"{"op":"s3_save_credentials","access_key_id":"access-secret","secret_access_key":"secret-secret"}"#,
            )
            .unwrap();
        let credentials = database
            .handle_json_request(r#"{"op":"s3_read_credentials"}"#)
            .unwrap();
        assert_eq!(credentials["access_key_id"], "access-secret");
        assert_eq!(credentials["secret_access_key"], "secret-secret");

        let invalid = database.handle_json_request(
            r#"{"op":"s3_save_config","config":{"endpoint":"ftp://storage.example.com","region":"auto","bucket":"vault","prefix":"","filename":"nauterm-sync.enc"}}"#,
        );
        assert!(invalid.is_err());
    }

    #[test]
    fn cloud_provider_catalog_records_round_trip_without_exposing_credentials() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let provider = crate::cloud_sync::CloudProviderConfig {
            id: "provider-1".to_string(),
            scheme: "s3".to_string(),
            vendor: "minio".to_string(),
            name: "Lab MinIO".to_string(),
            config: std::collections::BTreeMap::from([
                (
                    "endpoint".to_string(),
                    "https://minio.example.com".to_string(),
                ),
                ("region".to_string(), "auto".to_string()),
                ("bucket".to_string(), "nauterm".to_string()),
                ("prefix".to_string(), "backups".to_string()),
                ("filename".to_string(), "nauterm-sync.enc".to_string()),
            ]),
        };
        database.save_cloud_provider(provider.clone()).unwrap();
        assert_eq!(database.cloud_providers().unwrap(), vec![provider]);
        let stored: (String, String, Option<i64>, Option<String>, Option<i64>) = database
            .connection
            .query_row(
                "SELECT provider, config_json, last_seen_revision, last_seen_snapshot_id, last_sync_at FROM sync_providers WHERE uuid = 'provider-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();
        assert_eq!(stored.0, "minio");
        assert!(stored.1.contains("\"scheme\":\"s3\""));
        assert_eq!(stored.2, None);
        assert_eq!(stored.3, None);
        assert_eq!(stored.4, None);
        database
            .remember_remote_sync_status(
                "provider-1",
                Some(&crate::sync::RemoteSyncStatus {
                    revision: 6,
                    snapshot_id: "snapshot-6".to_string(),
                }),
            )
            .unwrap();
        let observed: (Option<i64>, Option<String>, Option<i64>) = database
            .connection
            .query_row(
                "SELECT last_seen_revision, last_seen_snapshot_id, last_sync_at FROM sync_providers WHERE uuid = 'provider-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(observed, (Some(6), Some("snapshot-6".to_string()), None));
        database
            .save_sync_preferences(&crate::cloud_sync::SyncPreferences {
                active_provider_id: Some("cloud:provider-1".to_string()),
                sync_snapshot: None,
            })
            .unwrap();
        database
            .connection
            .execute(
                "UPDATE sync_providers SET last_seen_revision = 7, last_seen_snapshot_id = 'snapshot-7', last_sync_at = 1234 WHERE uuid = 'provider-1'",
                [],
            )
            .unwrap();
        let status = database.sync_preferences_status().unwrap();
        assert_eq!(status["remote_revision"], 7);
        assert_eq!(status["remote_snapshot_id"], "snapshot-7");
        assert_eq!(status["remote_synced_at"], 1234);
        let saved = database
            .handle_json_request(
                r#"{"op":"cloud_save_provider","provider":{"id":"provider-1","scheme":"s3","vendor":"minio","name":"Lab MinIO","config":{"endpoint":"https://minio.example.com","region":"auto","bucket":"nauterm","prefix":"backups","filename":"nauterm-sync.enc"}},"credentials":{"values":{"access_key_id":"access-secret","secret_access_key":"secret-secret"}}}"#,
            )
            .unwrap();
        assert_eq!(saved["has_credentials"], true);
        assert!(!saved.to_string().contains("secret-secret"));
        let revealed = database
            .handle_json_request(r#"{"op":"cloud_load_credentials","provider_id":"provider-1"}"#)
            .unwrap();
        assert_eq!(revealed["values"]["access_key_id"], "access-secret");
        assert_eq!(revealed["values"]["secret_access_key"], "secret-secret");
        let loaded_credentials = database
            .load_cloud_credentials("provider-1")
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded_credentials.values["secret_access_key"],
            "secret-secret"
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT active FROM sync_providers WHERE uuid = 'provider-1'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM app_metadata
                     WHERE key LIKE '%credentials%' OR key = 'github_pat'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert!(database
            .connection
            .query_row(
                "SELECT json_extract(value, '$.active_provider_id')
                 FROM app_metadata WHERE key = 'sync_preferences_v1'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .unwrap()
            .is_none());
        let deleted = database
            .handle_json_request(r#"{"op":"cloud_delete_provider","provider_id":"provider-1"}"#)
            .unwrap();
        assert_eq!(deleted["deleted"], true);
        assert!(database.cloud_providers().unwrap().is_empty());
        assert!(database
            .load_cloud_credentials("provider-1")
            .unwrap()
            .is_none());
    }

    #[test]
    fn github_pat_is_stored_inside_the_sqlcipher_database() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        database
            .handle_json_request(r#"{"op":"github_save_token","token":"github-secret"}"#)
            .unwrap();
        assert_eq!(
            database
                .handle_json_request(r#"{"op":"github_load_token"}"#)
                .unwrap()["has_token"],
            true
        );
        assert_eq!(
            database
                .handle_json_request(r#"{"op":"github_read_token"}"#)
                .unwrap()["token"],
            "github-secret"
        );
        assert_eq!(
            database
                .load_github_pat()
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some("github-secret")
        );
        database
            .handle_json_request(r#"{"op":"github_delete_token"}"#)
            .unwrap();
        assert!(!database.has_github_pat().unwrap());
    }

    #[test]
    fn enforces_relations_and_basic_constraints() {
        let database = NautermDatabase::open_in_memory().unwrap();
        let connection = &database.connection;
        connection
            .execute_batch(
                r#"
                INSERT INTO groups (uuid, name) VALUES ('group-1', 'Group');
                INSERT INTO hosts (uuid, name, group_uuid, type)
                  VALUES ('host-1', 'Host', 'group-1', 'remote');
                INSERT INTO port_forwards (
                  uuid, name, type, bind_address, bind_port, destination_host,
                  destination_port, host_uuid
                ) VALUES (
                  'forward-1', 'Forward', 'local', '127.0.0.1', 2200,
                  '127.0.0.1', 22, 'host-1'
                );
                INSERT INTO snippet_packages (uuid, name) VALUES ('package-1', 'Package');
                INSERT INTO snippets (uuid, package_uuid, description, script)
                  VALUES ('snippet-1', 'package-1', 'Snippet', 'echo ok');
                INSERT INTO snippet_target_hosts (snippet_uuid, host_uuid)
                  VALUES ('snippet-1', 'host-1');
                DELETE FROM hosts WHERE uuid = 'host-1';
                "#,
            )
            .unwrap();
        assert!(connection
            .query_row(
                "SELECT host_uuid IS NULL FROM port_forwards WHERE uuid = 'forward-1'",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM snippet_target_hosts", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
        assert!(connection
            .execute(
                "INSERT INTO hosts (uuid, name, type, environment_variables) VALUES ('host-2', 'Invalid', 'remote', 'not-json')",
                [],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO proxies (uuid, name, type, host, port) VALUES ('proxy-1', 'Invalid', 'ftp', 'localhost', 21)",
                [],
            )
            .is_ok());
    }

    #[test]
    fn links_sqlcipher() {
        let database = NautermDatabase::open_in_memory().unwrap();
        let version: String = database
            .connection
            .query_row("PRAGMA cipher_version", [], |row| row.get(0))
            .unwrap();
        assert!(!version.trim().is_empty());
    }

    #[test]
    fn persists_device_id_and_tracks_local_writes() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let device_id = database.device_id().unwrap();
        assert!(is_uuid_value(&device_id));
        assert_eq!(device_id.as_bytes()[14], b'7');

        NautermDatabase::ensure_schema(&mut database.connection).unwrap();
        assert_eq!(database.device_id().unwrap(), device_id);

        let group_id = database
            .save_group(&HostGroup {
                id: None,
                uuid: None,
                name: "production".to_string(),
                parent_id: None,
                parent_uuid: None,
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
                ..HostGroup::default()
            })
            .unwrap();
        let mut group = database.get_group(group_id).unwrap().unwrap();
        assert_eq!(group.created_device_id.as_deref(), Some(device_id.as_str()));
        assert_eq!(group.updated_device_id.as_deref(), Some(device_id.as_str()));

        let imported_device_id = "018f3a7a-1111-7c8d-9b6a-8b8f9c0d1e2f";
        database
            .connection
            .execute(
                "UPDATE groups SET updated_device_id = ? WHERE id = ?",
                params![imported_device_id, group_id],
            )
            .unwrap();
        assert_eq!(
            database
                .get_group(group_id)
                .unwrap()
                .unwrap()
                .updated_device_id
                .as_deref(),
            Some(imported_device_id)
        );

        group.name = "production updated".to_string();
        group.updated_device_id = Some(imported_device_id.to_string());
        database.save_group(&group).unwrap();
        let updated = database.get_group(group_id).unwrap().unwrap();
        assert_eq!(
            updated.created_device_id.as_deref(),
            Some(device_id.as_str())
        );
        assert_eq!(
            updated.updated_device_id.as_deref(),
            Some(device_id.as_str())
        );
    }

    #[test]
    fn local_file_sync_merges_updates_and_tombstones_between_devices() {
        let path = std::env::temp_dir().join(format!(
            "nauterm-sync-{}-{}.pnz",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let master_key = "Correct horse battery 1!";
        let mut first = NautermDatabase::open_in_memory().unwrap();
        let group_id = first
            .save_group(&HostGroup {
                id: None,
                uuid: None,
                name: "production secret".to_string(),
                parent_id: None,
                parent_uuid: None,
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
                ..HostGroup::default()
            })
            .unwrap();
        let created = first
            .sync_local_file(path.to_str().unwrap(), master_key)
            .unwrap();
        assert!(created.created);
        assert!(!fs::read_to_string(&path)
            .unwrap()
            .contains("production secret"));
        first
            .sync_local_file_with_saved_key(path.to_str().unwrap(), None)
            .unwrap();

        let mut second = NautermDatabase::open_in_memory().unwrap();
        assert!(second
            .sync_local_file(path.to_str().unwrap(), "Incorrect master key 2!")
            .is_err());
        let imported = second
            .sync_local_file(path.to_str().unwrap(), master_key)
            .unwrap();
        assert_eq!(imported.imported_records, 1);
        second
            .sync_local_file_with_saved_key(path.to_str().unwrap(), None)
            .unwrap();
        let mut second_group = second.list_groups().unwrap().remove(0);
        assert_eq!(second_group.name, "production secret");

        second_group.name = "production updated".to_string();
        second.save_group(&second_group).unwrap();
        second
            .sync_local_file(path.to_str().unwrap(), master_key)
            .unwrap();
        first
            .sync_local_file(path.to_str().unwrap(), master_key)
            .unwrap();
        assert_eq!(
            first.get_group(group_id).unwrap().unwrap().name,
            "production updated"
        );

        first.delete_group(group_id).unwrap();
        first
            .sync_local_file(path.to_str().unwrap(), master_key)
            .unwrap();
        second
            .sync_local_file(path.to_str().unwrap(), master_key)
            .unwrap();
        assert!(second.list_groups().unwrap().is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sync_master_key_rotation_rewraps_the_existing_dek() {
        let path = std::env::temp_dir().join(format!(
            "nauterm-sync-key-rotation-{}-{}.enc",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let old_key = "Correct horse battery 1!";
        let new_key = "New correct horse battery 2!";
        let mut source = NautermDatabase::open_in_memory().unwrap();
        source
            .sync_local_file(path.to_str().unwrap(), old_key)
            .unwrap();

        let before_failed_rotation = fs::read(&path).unwrap();
        assert!(source
            .rotate_local_file_master_key(
                path.to_str().unwrap(),
                "Incorrect current master key 3!",
                new_key,
            )
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), before_failed_rotation);

        source
            .rotate_local_file_master_key(path.to_str().unwrap(), old_key, new_key)
            .unwrap();

        let mut old_key_device = NautermDatabase::open_in_memory().unwrap();
        assert!(old_key_device
            .sync_local_file(path.to_str().unwrap(), old_key)
            .is_err());
        let mut new_key_device = NautermDatabase::open_in_memory().unwrap();
        new_key_device
            .sync_local_file(path.to_str().unwrap(), new_key)
            .unwrap();
        let _ = fs::remove_file(path);
    }

    #[test]
    fn provider_staging_can_create_a_new_remote_after_the_local_sync_key_exists() {
        let first_path = std::env::temp_dir().join(format!(
            "nauterm-linked-sync-{}-{}.enc",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let provider_path = std::env::temp_dir().join(format!(
            "nauterm-provider-staging-{}-{}.enc",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let master_key = "Correct horse battery 1!";
        let mut database = NautermDatabase::open_in_memory().unwrap();

        database
            .sync_local_file(first_path.to_str().unwrap(), master_key)
            .unwrap();
        fs::remove_file(&first_path).unwrap();

        let local_file_error = database
            .sync_local_file_with_saved_key(first_path.to_str().unwrap(), None)
            .unwrap_err();
        assert!(local_file_error
            .to_string()
            .contains("linked sync file is missing"));

        let synced = database
            .sync_provider_staging_file(
                provider_path.to_str().unwrap(),
                None,
                "test_provider",
                crate::sync::SyncStrategy::SmartMerge,
            )
            .unwrap();
        assert!(synced.created);
        assert!(provider_path.exists());

        fs::remove_file(&provider_path).unwrap();
        database
            .connection
            .execute(
                "DELETE FROM app_metadata WHERE key = 'sync_envelope_header'",
                [],
            )
            .unwrap();
        let upgraded = database
            .sync_provider_staging_file(
                provider_path.to_str().unwrap(),
                Some(master_key),
                "test_provider",
                crate::sync::SyncStrategy::SmartMerge,
            )
            .unwrap();
        assert!(upgraded.created);
        assert!(provider_path.exists());

        let _ = fs::remove_file(provider_path);
    }

    #[test]
    fn provider_rollback_checkpoints_are_scoped_per_transport() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let first_path = std::env::temp_dir().join(format!(
            "nauterm-provider-a-{}-{suffix}.enc",
            std::process::id()
        ));
        let second_path = std::env::temp_dir().join(format!(
            "nauterm-provider-b-{}-{suffix}.enc",
            std::process::id()
        ));
        let master_key = "Provider checkpoint key 1!";
        let mut database = NautermDatabase::open_in_memory().unwrap();

        let first = database
            .sync_provider_staging_file(
                first_path.to_str().unwrap(),
                Some(master_key),
                "provider_a",
                crate::sync::SyncStrategy::SmartMerge,
            )
            .unwrap();
        let second = database
            .sync_provider_staging_file(
                second_path.to_str().unwrap(),
                None,
                "provider_b",
                crate::sync::SyncStrategy::SmartMerge,
            )
            .unwrap();
        assert!(second.revision > first.revision);

        let first_again = database
            .sync_provider_staging_file(
                first_path.to_str().unwrap(),
                None,
                "provider_a",
                crate::sync::SyncStrategy::SmartMerge,
            )
            .unwrap();
        assert!(first_again.revision > second.revision);
        assert_ne!(first.snapshot_id, second.snapshot_id);
        assert_ne!(second.snapshot_id, first_again.snapshot_id);

        let _ = fs::remove_file(first_path);
        let _ = fs::remove_file(second_path);
    }

    #[test]
    fn provider_sync_strategies_choose_local_or_remote_records() {
        let path = std::env::temp_dir().join(format!(
            "nauterm-provider-strategy-{}-{}.enc",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let master_key = "Correct horse battery 1!";
        let mut cloud = NautermDatabase::open_in_memory().unwrap();
        cloud
            .save_group(&HostGroup {
                id: None,
                uuid: None,
                name: "cloud v1".to_string(),
                parent_id: None,
                parent_uuid: None,
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
                ..HostGroup::default()
            })
            .unwrap();
        cloud
            .sync_provider_staging_file(
                path.to_str().unwrap(),
                Some(master_key),
                "strategy",
                crate::sync::SyncStrategy::SmartMerge,
            )
            .unwrap();

        let mut local = NautermDatabase::open_in_memory().unwrap();
        local
            .sync_provider_staging_file(
                path.to_str().unwrap(),
                Some(master_key),
                "strategy",
                crate::sync::SyncStrategy::SmartMerge,
            )
            .unwrap();
        cloud
            .sync_provider_staging_file(
                path.to_str().unwrap(),
                None,
                "strategy",
                crate::sync::SyncStrategy::SmartMerge,
            )
            .unwrap();

        let mut local_group = local.list_groups().unwrap().remove(0);
        local_group.name = "local winner".to_string();
        local.save_group(&local_group).unwrap();
        let mut cloud_group = cloud.list_groups().unwrap().remove(0);
        cloud_group.name = "cloud ignored".to_string();
        cloud.save_group(&cloud_group).unwrap();
        cloud
            .sync_provider_staging_file(
                path.to_str().unwrap(),
                None,
                "strategy",
                crate::sync::SyncStrategy::SmartMerge,
            )
            .unwrap();
        local
            .sync_provider_staging_file(
                path.to_str().unwrap(),
                None,
                "strategy",
                crate::sync::SyncStrategy::LocalWins,
            )
            .unwrap();
        assert_eq!(local.list_groups().unwrap()[0].name, "local winner");

        cloud
            .sync_provider_staging_file(
                path.to_str().unwrap(),
                None,
                "strategy",
                crate::sync::SyncStrategy::SmartMerge,
            )
            .unwrap();
        let mut cloud_group = cloud.list_groups().unwrap().remove(0);
        cloud_group.name = "cloud winner".to_string();
        cloud.save_group(&cloud_group).unwrap();
        cloud
            .sync_provider_staging_file(
                path.to_str().unwrap(),
                None,
                "strategy",
                crate::sync::SyncStrategy::LocalWins,
            )
            .unwrap();
        local
            .sync_provider_staging_file(
                path.to_str().unwrap(),
                None,
                "strategy",
                crate::sync::SyncStrategy::RemoteWins,
            )
            .unwrap();
        assert_eq!(local.list_groups().unwrap()[0].name, "cloud winner");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn local_file_sync_includes_only_the_selected_data_scope() {
        let path = std::env::temp_dir().join(format!(
            "nauterm-sync-scope-{}-{}.pnz",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let master_key = "Scope verification key 1!";
        let mut source = NautermDatabase::open_in_memory().unwrap();
        source
            .connection
            .execute_batch(
                r#"
                INSERT INTO groups (uuid, name)
                VALUES ('01979f62-8548-7000-8000-000000000100', 'Secret group');
                INSERT INTO keys (uuid, name, private_key, public_key)
                VALUES (
                  '01979f62-8548-7000-8000-000000000101',
                  'Secret key',
                  'PRIVATE KEY MATERIAL',
                  'PUBLIC KEY MATERIAL'
                );
                INSERT INTO identities (uuid, name, username, password, key_uuid)
                VALUES (
                  '01979f62-8548-7000-8000-000000000102',
                  'Secret identity',
                  'admin',
                  'identity-password',
                  '01979f62-8548-7000-8000-000000000101'
                );
                INSERT INTO proxies (
                  uuid, name, type, host, port, identity_uuid, password
                ) VALUES (
                  '01979f62-8548-7000-8000-000000000103',
                  'Secret proxy',
                  'socks5',
                  'proxy.internal',
                  1080,
                  '01979f62-8548-7000-8000-000000000102',
                  'proxy-password'
                );
                INSERT INTO hosts (
                  uuid, name, type, host, port, username, password, group_uuid,
                  identity_uuid, proxy_uuid, environment_variables
                ) VALUES (
                  '01979f62-8548-7000-8000-000000000104',
                  'Remote host',
                  'remote',
                  '10.20.30.40',
                  22,
                  'admin',
                  'host-password',
                  '01979f62-8548-7000-8000-000000000100',
                  '01979f62-8548-7000-8000-000000000102',
                  '01979f62-8548-7000-8000-000000000103',
                  '[{"variable":"TOKEN","value":"environment-secret"}]'
                );
                INSERT INTO hosts (uuid, name, type, shell_path, work_dir)
                VALUES (
                  '01979f62-8548-7000-8000-000000000105',
                  'Local host',
                  'local',
                  '/bin/zsh',
                  '/Users/local'
                );
                INSERT INTO tags (uuid, name)
                VALUES ('01979f62-8548-7000-8000-000000000112', 'Production');
                INSERT INTO host_tags (host_uuid, tag_uuid)
                VALUES (
                  '01979f62-8548-7000-8000-000000000104',
                  '01979f62-8548-7000-8000-000000000112'
                );
                INSERT INTO port_forwards (
                  uuid, name, type, bind_address, bind_port,
                  destination_host, destination_port, host_uuid
                ) VALUES (
                  '01979f62-8548-7000-8000-000000000106',
                  'Database tunnel',
                  'local',
                  '127.0.0.1',
                  5432,
                  '127.0.0.1',
                  5432,
                  '01979f62-8548-7000-8000-000000000104'
                );
                INSERT INTO snippet_packages (uuid, name)
                VALUES ('01979f62-8548-7000-8000-000000000107', 'Operations');
                INSERT INTO snippets (
                  uuid, package_uuid, scope, description, script
                ) VALUES (
                  '01979f62-8548-7000-8000-000000000108',
                  '01979f62-8548-7000-8000-000000000107',
                  'targeted',
                  'Restart service',
                  'systemctl restart secret-service'
                );
                INSERT INTO snippet_target_groups (snippet_uuid, group_uuid)
                VALUES (
                  '01979f62-8548-7000-8000-000000000108',
                  '01979f62-8548-7000-8000-000000000100'
                );
                INSERT INTO snippet_target_hosts (snippet_uuid, host_uuid)
                VALUES (
                  '01979f62-8548-7000-8000-000000000108',
                  '01979f62-8548-7000-8000-000000000104'
                );
                INSERT INTO sftp_favorites (uuid, scope, host_uuid, path)
                VALUES (
                  '01979f62-8548-7000-8000-000000000109',
                  'remote',
                  '01979f62-8548-7000-8000-000000000104',
                  '/srv/secret'
                );
                INSERT INTO ai_providers (
                  uuid, name, protocol, base_url, api_key
                ) VALUES (
                  '01979f62-8548-7000-8000-000000000111',
                  'Local AI',
                  'openai',
                  'https://ai.example/v1',
                  'ai-provider-secret'
                );
                "#,
            )
            .unwrap();

        source
            .sync_local_file(path.to_str().unwrap(), master_key)
            .unwrap();
        let encrypted = fs::read_to_string(&path).unwrap();
        for plaintext in [
            "10.20.30.40",
            "PRIVATE KEY MATERIAL",
            "identity-password",
            "proxy-password",
            "host-password",
            "environment-secret",
            "systemctl restart secret-service",
        ] {
            assert!(!encrypted.contains(plaintext));
        }

        let mut target = NautermDatabase::open_in_memory().unwrap();
        target
            .sync_local_file(path.to_str().unwrap(), master_key)
            .unwrap();
        assert_eq!(target.list_groups().unwrap().len(), 1);
        assert_eq!(target.list_keys().unwrap().len(), 1);
        assert_eq!(target.list_identities().unwrap().len(), 1);
        assert_eq!(target.list_proxies().unwrap().len(), 1);
        let synced_hosts = target.list_hosts(None).unwrap();
        assert_eq!(synced_hosts.len(), 1);
        assert_eq!(synced_hosts[0].name, "Remote host");
        assert_eq!(
            synced_hosts[0].tag_uuids,
            vec!["01979f62-8548-7000-8000-000000000112"]
        );
        let remote_host_id = synced_hosts[0].id;
        assert_eq!(target.list_port_forwards(None).unwrap().len(), 1);
        assert_eq!(target.list_snippet_packages().unwrap().len(), 1);
        let snippet = target.list_snippets(None).unwrap().remove(0);
        assert_eq!(snippet.target_group_ids.len(), 1);
        assert_eq!(snippet.target_host_ids.len(), 1);
        assert_eq!(
            target
                .list_sftp_favorite_paths("remote".to_string(), remote_host_id)
                .unwrap()
                .len(),
            1
        );
        assert!(target
            .list_sftp_favorite_paths("local".to_string(), None)
            .is_err());
        assert!(target.list_ai_providers().unwrap().is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_unsupported_schema_versions() {
        let mut connection = Connection::open_in_memory().unwrap();
        NautermDatabase::configure(&connection).unwrap();
        connection.pragma_update(None, "user_version", 40).unwrap();

        let error = NautermDatabase::ensure_schema(&mut connection).unwrap_err();
        assert!(error.to_string().contains("unsupported"));
    }

    #[test]
    fn migrates_ai_provider_schema_to_v2() {
        let mut connection = Connection::open_in_memory().unwrap();
        NautermDatabase::configure(&connection).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE ai_providers (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  uuid TEXT NOT NULL UNIQUE,
                  name TEXT NOT NULL,
                  protocol TEXT NOT NULL CHECK (protocol IN ('openai', 'anthropic')),
                  base_url TEXT NOT NULL,
                  model TEXT NOT NULL DEFAULT '',
                  api_key TEXT NOT NULL DEFAULT '',
                  config TEXT NOT NULL DEFAULT '{"max_tokens":4096}',
                  active INTEGER NOT NULL DEFAULT 0,
                  created_at INTEGER NOT NULL DEFAULT 0,
                  updated_at INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX idx_ai_providers_active ON ai_providers(active);
                CREATE INDEX idx_ai_providers_updated_at ON ai_providers(updated_at DESC);
                INSERT INTO ai_providers
                  (uuid, name, protocol, base_url)
                VALUES ('provider-1', 'OpenAI', 'openai', 'https://api.openai.com/v1');
                PRAGMA user_version = 1;
                "#,
            )
            .unwrap();

        migrations::migrate_v1_to_v2(&mut connection).unwrap();
        connection
            .execute(
                "INSERT INTO ai_providers (uuid, name, protocol, base_url) VALUES (?, ?, ?, ?)",
                params!["provider-2", "Ollama", "ollama", "http://localhost:11434"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO ai_providers (uuid, name, protocol, base_url) VALUES (?, ?, ?, ?)",
                params![
                    "provider-3",
                    "Future Provider",
                    "future_protocol",
                    "https://example.com"
                ],
            )
            .unwrap();

        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i32>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM ai_providers", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT config FROM ai_providers WHERE uuid = 'provider-3'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            "{}"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT config FROM ai_providers WHERE uuid = 'provider-1'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            r#"{"max_tokens":4096}"#
        );
    }

    #[test]
    fn manages_host_records() {
        let mut database = NautermDatabase::open_in_memory().unwrap();

        let group_id = database
            .save_group(&HostGroup {
                id: None,
                uuid: None,
                name: "production".to_string(),
                parent_id: None,
                parent_uuid: None,
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
                ..HostGroup::default()
            })
            .unwrap();
        let key_id = database
            .save_key(&KeyEntry {
                id: None,
                uuid: None,
                name: "main key".to_string(),
                private_key: Some("private".to_string()),
                public_key: Some("public".to_string()),
                certificate: Some(
                    "ecdsa-sha2-nistp384-cert-v01@openssh.com certificate-body".to_string(),
                ),
                passphrase: Some("saved passphrase".to_string()),
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();
        let identity_id = database
            .save_identity(&IdentityEntry {
                id: None,
                uuid: None,
                name: "admin".to_string(),
                username: Some("admin".to_string()),
                password: Some("secret".to_string()),
                key_id: Some(key_id),
                key_uuid: None,
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();
        let host_id = database
            .save_host(&HostEntry {
                tag_uuids: Vec::new(),
                id: None,
                uuid: None,
                name: "cassandra".to_string(),
                group_id: Some(group_id),
                group_uuid: None,
                identity_id: Some(identity_id),
                identity_uuid: None,
                proxy_id: None,
                proxy_uuid: None,
                host: Some("10.0.0.12".to_string()),
                port: Some(22),
                username: Some("admin".to_string()),
                password: None,
                theme_id: None,
                startup_snippet_id: None,
                startup_snippet_uuid: None,
                ssh_enabled: Some(true),
                mosh_enabled: Some(true),
                mosh_server_command: Some(
                    "custom-mosh-server new -s -l LANG=en_US.UTF-8".to_string(),
                ),
                telnet_enabled: Some(true),
                telnet_identity_id: Some(identity_id),
                telnet_identity_uuid: None,
                telnet_username: Some("telnet-user".to_string()),
                telnet_password: Some("telnet-secret".to_string()),
                telnet_port: Some(23),
                telnet_theme_id: Some("ayu-dark".to_string()),
                environment_variables: vec![HostEnvironmentVariable {
                    variable: "LANG".to_string(),
                    value: "en_US.UTF-8".to_string(),
                }],
                encoding: Some("UTF-8".to_string()),
                telnet_encoding: Some("UTF-8".to_string()),
                host_type: HostType::Remote,
                key_id: Some(key_id),
                key_uuid: None,
                shell_path: None,
                work_dir: None,
                os: None,
                distro: None,
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();
        let port_forward_id = database
            .save_port_forward(&PortForwardEntry {
                id: None,
                uuid: None,
                name: "postgres".to_string(),
                r#type: "local".to_string(),
                bind_address: "127.0.0.1".to_string(),
                bind_port: 5432,
                destination_host: "127.0.0.1".to_string(),
                destination_port: 5432,
                connection_id: host_id,
                host_uuid: None,
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();

        assert_eq!(database.list_groups().unwrap().len(), 1);
        let key_summary = database.list_keys().unwrap().remove(0);
        assert_eq!(
            key_summary.certificate.as_deref(),
            Some("ecdsa-sha2-nistp384-cert-v01@openssh.com")
        );
        assert_eq!(key_summary.private_key, None);
        assert_eq!(key_summary.passphrase, None);
        assert_eq!(
            database
                .get_key(key_id)
                .unwrap()
                .unwrap()
                .certificate
                .as_deref(),
            Some("ecdsa-sha2-nistp384-cert-v01@openssh.com certificate-body")
        );
        assert_eq!(
            database
                .get_key(key_id)
                .unwrap()
                .unwrap()
                .passphrase
                .as_deref(),
            Some("saved passphrase")
        );
        let identities = database.list_identities().unwrap();
        assert_eq!(identities.len(), 1);
        assert_eq!(identities[0].password, None);
        assert_eq!(
            database
                .get_identity(identity_id)
                .unwrap()
                .unwrap()
                .password,
            Some("secret".to_string())
        );
        assert_eq!(
            database.get_host(host_id).unwrap().unwrap().host,
            Some("10.0.0.12".to_string())
        );
        let host = database.get_host(host_id).unwrap().unwrap();
        assert_eq!(host.key_id, Some(key_id));
        assert_eq!(
            host.key_uuid,
            database.get_key(key_id).unwrap().unwrap().uuid
        );
        assert_eq!(host.mosh_enabled, Some(true));
        assert_eq!(
            host.mosh_server_command,
            Some("custom-mosh-server new -s -l LANG=en_US.UTF-8".to_string())
        );
        assert_eq!(host.telnet_enabled, Some(true));
        assert_eq!(host.telnet_port, Some(23));
        assert_eq!(host.telnet_theme_id, Some("ayu-dark".to_string()));
        assert_eq!(host.environment_variables.len(), 1);
        assert_eq!(host.environment_variables[0].variable, "LANG");
        assert_eq!(host.encoding, Some("UTF-8".to_string()));
        let summary = database.list_hosts(None).unwrap().remove(0);
        assert_eq!(summary.password, None);
        assert_eq!(summary.telnet_password, None);
        assert_eq!(summary.startup_snippet_id, None);
        assert!(summary.environment_variables.is_empty());

        let tag_id = database
            .save_tag(&TagEntry {
                id: None,
                uuid: None,
                name: "production".to_string(),
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();
        let tag = database.get_tag(tag_id).unwrap().unwrap();
        let mut tagged_host = database.get_host(host_id).unwrap().unwrap();
        tagged_host.tag_uuids = vec![
            tag.uuid.clone().unwrap(),
            tag.uuid.clone().unwrap(),
            "not-a-uuid".to_string(),
        ];
        database.save_host(&tagged_host).unwrap();
        assert_eq!(database.list_tags().unwrap(), vec![tag.clone()]);
        assert!(!table_column_exists(&database.connection, "hosts", "tag_uuids").unwrap());
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM host_tags", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            1
        );
        assert_eq!(
            database.list_hosts(None).unwrap()[0].tag_uuids,
            vec![tag.uuid.clone().unwrap()]
        );
        database.delete_tag(tag_id).unwrap();
        assert!(database
            .get_host(host_id)
            .unwrap()
            .unwrap()
            .tag_uuids
            .is_empty());
        assert_eq!(
            database
                .list_port_forwards(Some(host_id))
                .unwrap()
                .first()
                .unwrap()
                .id,
            Some(port_forward_id)
        );

        database.delete_host(host_id).unwrap();
        assert!(database
            .list_port_forwards(Some(host_id))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn manages_snippet_records() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let group_id = database
            .save_group(&HostGroup {
                id: None,
                uuid: None,
                name: "production".to_string(),
                parent_id: None,
                parent_uuid: None,
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
                ..HostGroup::default()
            })
            .unwrap();
        let host_id = database
            .save_host(&HostEntry {
                tag_uuids: Vec::new(),
                id: None,
                uuid: None,
                name: "cassandra".to_string(),
                group_id: Some(group_id),
                group_uuid: None,
                identity_id: None,
                identity_uuid: None,
                proxy_id: None,
                proxy_uuid: None,
                host: Some("10.0.0.12".to_string()),
                port: Some(22),
                username: Some("admin".to_string()),
                password: None,
                theme_id: None,
                startup_snippet_id: None,
                startup_snippet_uuid: None,
                ssh_enabled: Some(true),
                mosh_enabled: Some(false),
                mosh_server_command: Some(default_mosh_server_command()),
                telnet_enabled: Some(false),
                telnet_identity_id: None,
                telnet_identity_uuid: None,
                telnet_username: None,
                telnet_password: None,
                telnet_port: None,
                telnet_theme_id: None,
                environment_variables: Vec::new(),
                encoding: Some("UTF-8".to_string()),
                telnet_encoding: Some("UTF-8".to_string()),
                host_type: HostType::Remote,
                key_id: None,
                key_uuid: None,
                shell_path: None,
                work_dir: None,
                os: None,
                distro: None,
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();
        let package_id = database
            .save_snippet_package(&SnippetPackageEntry {
                id: None,
                uuid: None,
                name: "Ops".to_string(),
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();

        let snippet_id = database
            .save_snippet(&SnippetEntry {
                id: None,
                uuid: None,
                package_id: Some(package_id),
                package_uuid: None,
                scope: "targeted".to_string(),
                description: "Restart service".to_string(),
                script: "systemctl restart app".to_string(),
                target_group_ids: vec![group_id, group_id],
                target_host_ids: vec![host_id, host_id],
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();

        let snippet = database.get_snippet(snippet_id).unwrap().unwrap();
        assert_eq!(snippet.scope, "targeted");
        assert_eq!(snippet.target_group_ids, vec![group_id]);
        assert_eq!(snippet.target_host_ids, vec![host_id]);
        assert_eq!(
            database
                .list_snippets(Some(package_id))
                .unwrap()
                .first()
                .unwrap()
                .id,
            Some(snippet_id)
        );

        database
            .save_snippet(&SnippetEntry {
                id: Some(snippet_id),
                uuid: None,
                package_id: None,
                package_uuid: None,
                scope: "global".to_string(),
                description: "Restart all".to_string(),
                script: "systemctl restart app".to_string(),
                target_group_ids: vec![group_id],
                target_host_ids: vec![host_id],
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();
        let updated = database.get_snippet(snippet_id).unwrap().unwrap();
        assert_eq!(updated.package_id, None);
        assert_eq!(updated.description, "Restart all");
        assert!(updated.target_group_ids.is_empty());
        assert!(updated.target_host_ids.is_empty());

        database
            .save_snippet(&SnippetEntry {
                id: Some(snippet_id),
                uuid: None,
                package_id: Some(package_id),
                package_uuid: None,
                scope: "targeted".to_string(),
                description: "Restart production".to_string(),
                script: "systemctl restart app".to_string(),
                target_group_ids: vec![group_id],
                target_host_ids: vec![host_id],
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();

        database.delete_host(host_id).unwrap();
        assert!(database
            .get_snippet(snippet_id)
            .unwrap()
            .unwrap()
            .target_host_ids
            .is_empty());
        database.delete_group(group_id).unwrap();
        assert!(database
            .get_snippet(snippet_id)
            .unwrap()
            .unwrap()
            .target_group_ids
            .is_empty());

        assert_eq!(database.delete_snippet_package(package_id).unwrap(), 1);
        assert_eq!(
            database
                .get_snippet(snippet_id)
                .unwrap()
                .unwrap()
                .package_id,
            None
        );
        assert_eq!(database.delete_snippet(snippet_id).unwrap(), 1);
        assert!(database.get_snippet(snippet_id).unwrap().is_none());
    }

    #[test]
    fn manages_sftp_favorite_path_records() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        database
            .connection
            .execute(
                "INSERT INTO hosts (uuid, name) VALUES (?, ?)",
                params!["01979f62-8548-7000-8000-000000000201", "Saved host"],
            )
            .unwrap();
        let host_id = database.connection.last_insert_rowid();
        let id = database
            .save_sftp_favorite_path(&SftpFavoritePathEntry {
                id: None,
                uuid: None,
                scope: "remote".to_string(),
                host_id: Some(host_id),
                host_uuid: None,
                path: "/tmp".to_string(),
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();
        let same_id = database
            .save_sftp_favorite_path(&SftpFavoritePathEntry {
                id: None,
                uuid: None,
                scope: "remote".to_string(),
                host_id: Some(host_id),
                host_uuid: None,
                path: "/tmp".to_string(),
                created_at: None,
                updated_at: None,
                deleted_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();

        assert_eq!(same_id, id);
        let favorites = database
            .list_sftp_favorite_paths("remote".to_string(), Some(host_id))
            .unwrap();
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].path, "/tmp");
        assert_eq!(
            database
                .delete_sftp_favorite_path_by_target(
                    "remote".to_string(),
                    Some(host_id),
                    "/tmp".to_string(),
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn retains_thirty_day_sftp_task_history() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let mut ids = Vec::new();
        for finished_at in [1_768_392_000_000, 1_768_305_600_000, 1_761_998_400_000] {
            ids.push(
                database
                    .save_sftp_task_history(
                        &SftpTaskHistoryEntry {
                            id: None,
                            uuid: None,
                            host_uuid: None,
                            task_type: "download".to_string(),
                            host: "example.com".to_string(),
                            username: "admin".to_string(),
                            port: 22,
                            status: "completed".to_string(),
                            display_name: "example.txt".to_string(),
                            source_path: "/remote/example.txt".to_string(),
                            target_path: "/tmp/example.txt".to_string(),
                            created_at: finished_at,
                            finished_at,
                            bytes: 12,
                            total_bytes: 12,
                            item_kind: "file".to_string(),
                            error: None,
                        },
                        1_735_689_600_000,
                    )
                    .unwrap(),
            );
        }

        let history = database.list_sftp_task_history(1_765_843_200_000).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].id, Some(ids[0]));
        assert_eq!(database.delete_sftp_task_history(ids[1]).unwrap(), 1);
        assert_eq!(database.clear_sftp_task_history().unwrap(), 1);
        assert!(database
            .list_sftp_task_history(1_765_843_200_000)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn manages_terminal_log_records() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let log = TerminalLogEntry {
            id: "session-1".to_string(),
            local_id: None,
            title: "cassandra".to_string(),
            theme_id: Some("atom-one-dark".to_string()),
            host_id: Some(7),
            host_uuid: None,
            host: Some("10.0.0.12".to_string()),
            port: Some(22),
            username: Some("admin".to_string()),
            shell_path: None,
            work_dir: Some("/srv/app".to_string()),
            cwd: Some("/srv/app".to_string()),
            capture_file: "session-1.bin".to_string(),
            capture_bytes: 5,
            capture_sha256: Some(
                "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".to_string(),
            ),
            columns: Some(100),
            rows: Some(30),
            started_at: 1_747_738_800_000,
            ended_at: None,
        };
        let id = database
            .save_terminal_log(
                &log,
                &[TerminalLogEvent {
                    id: None,
                    log_id: None,
                    log_uuid: None,
                    timestamp: 1_747_738_801_000,
                    event_type: "connection".to_string(),
                    message: "Connected.".to_string(),
                    connection_kind: Some("connected".to_string()),
                    data: None,
                }],
            )
            .unwrap();

        assert_ne!(id, "session-1");
        assert!(is_uuid_value(&id));
        assert_eq!(id.as_bytes()[14], b'7');
        let logs = database.list_terminal_logs(Some(20), None).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].id, id);
        assert_eq!(logs[0].host_id, None);
        assert_eq!(logs[0].capture_bytes, 5);
        assert!(database
            .list_terminal_logs(Some(20), Some(1))
            .unwrap()
            .is_empty());
        let events = database.list_terminal_log_events(&id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "connection");

        let updated = TerminalLogEntry {
            id: id.clone(),
            capture_bytes: 11,
            ended_at: Some(1_747_739_100_000),
            ..log
        };
        database.save_terminal_log(&updated, &[]).unwrap();
        let logs = database.list_terminal_logs(Some(20), None).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].capture_bytes, 11);
        assert!(database.list_terminal_log_events(&id).unwrap().is_empty());

        assert_eq!(
            database.delete_terminal_log(&id).unwrap(),
            Some("session-1.bin".to_string())
        );
        assert!(database
            .list_terminal_logs(Some(20), None)
            .unwrap()
            .is_empty());

        let incomplete = TerminalLogEntry {
            id: "019b0000-0000-7000-8000-000000000001".to_string(),
            capture_file: "incomplete.ntrcap".to_string(),
            capture_bytes: 42,
            capture_sha256: None,
            ended_at: None,
            ..updated
        };
        let incomplete_id = database.save_terminal_log(&incomplete, &[]).unwrap();
        assert_eq!(
            database.list_incomplete_terminal_captures().unwrap().len(),
            1
        );
        database
            .finalize_recovered_terminal_capture(
                &incomplete_id,
                64,
                "recovered-sha256",
                1_747_739_160_000,
            )
            .unwrap();
        assert!(database
            .list_incomplete_terminal_captures()
            .unwrap()
            .is_empty());
        let recovered = database.list_terminal_logs(Some(20), None).unwrap();
        assert_eq!(recovered[0].capture_bytes, 64);
        assert_eq!(
            recovered[0].capture_sha256.as_deref(),
            Some("recovered-sha256")
        );

        let missing = TerminalLogEntry {
            id: "019b0000-0000-7000-8000-000000000002".to_string(),
            capture_file: "missing.ntrcap".to_string(),
            capture_bytes: 0,
            capture_sha256: None,
            ended_at: None,
            ..recovered[0].clone()
        };
        let missing_id = database.save_terminal_log(&missing, &[]).unwrap();
        database
            .clear_missing_terminal_capture(&missing_id)
            .unwrap();
        let cleared = database.list_terminal_logs(Some(20), None).unwrap();
        let cleared = cleared.iter().find(|entry| entry.id == missing_id).unwrap();
        assert_eq!(cleared.capture_file, "");
        assert_eq!(cleared.capture_bytes, 0);
    }

    #[test]
    fn saves_restores_and_physically_deletes_ai_conversations() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let conversation = AiConversationEntry {
            id: None,
            uuid: None,
            title: "Investigate deployment".to_string(),
            preview: None,
            scope: "terminal".to_string(),
            host_uuid: None,
            provider_uuid: None,
            model: String::new(),
            messages: vec![AiMessageEntry {
                id: None,
                uuid: None,
                role: "user".to_string(),
                content: "Why did it fail?".to_string(),
                context: "exit code 1".to_string(),
                sequence: 0,
                tool_calls: vec![],
                tool_result: None,
                attachments: vec![json!({"name": "deploy.log"})],
                created_at: None,
                updated_at: None,
                version: None,
            }],
            command_blocks: vec![AiCommandBlockEntry {
                id: None,
                uuid: None,
                tool_call_id: "tool-1".to_string(),
                command: "tail deploy.log".to_string(),
                explanation: "Inspect the deployment log.".to_string(),
                status: "succeeded".to_string(),
                sequence: 1,
                output: Some("failed".to_string()),
                exit_code: Some(0),
                error: None,
                started_at: Some(1_750_503_600_000),
                finished_at: Some(1_750_503_601_000),
                created_at: None,
                updated_at: None,
                version: None,
            }],
            created_at: None,
            updated_at: None,
            version: None,
            created_device_id: None,
            updated_device_id: None,
        };

        let saved = database.save_ai_conversation(&conversation).unwrap();
        let uuid = saved.uuid.clone().unwrap();
        assert!(saved.messages[0].uuid.is_some());
        assert!(saved.command_blocks[0].uuid.is_some());
        assert_eq!(saved.messages[0].attachments[0]["name"], "deploy.log");
        let summaries = database
            .list_ai_conversations(Some("terminal"), None, Some(10))
            .unwrap();
        assert_eq!(summaries[0].preview.as_deref(), Some("Why did it fail?"));
        assert!(summaries[0].messages.is_empty());
        assert!(summaries[0].command_blocks.is_empty());

        let updated = AiConversationEntry {
            messages: vec![],
            command_blocks: saved.command_blocks.clone(),
            ..saved
        };
        let updated = database.save_ai_conversation(&updated).unwrap();
        assert!(updated.messages.is_empty());
        assert_eq!(updated.command_blocks.len(), 1);
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM ai_messages", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            database
                .list_ai_conversations(Some("terminal"), None, Some(10))
                .unwrap()
                .len(),
            1
        );
        assert!(database
            .list_ai_conversations(Some("workspace"), None, Some(10))
            .unwrap()
            .is_empty());

        assert_eq!(database.delete_ai_conversation(&uuid).unwrap(), 1);
        assert!(database.get_ai_conversation(&uuid).unwrap().is_none());
        assert!(database
            .list_ai_conversations(None, None, Some(10))
            .unwrap()
            .is_empty());
        for table in ["ai_conversations", "ai_messages", "ai_command_blocks"] {
            assert_eq!(
                database
                    .connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .unwrap(),
                0
            );
        }
    }

    #[test]
    fn groups_ai_history_by_optional_host_without_session_binding() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let base = AiConversationEntry {
            id: None,
            uuid: None,
            title: "Conversation".to_string(),
            preview: None,
            scope: "terminal".to_string(),
            host_uuid: Some("01979f62-8548-7000-8000-000000000100".to_string()),
            provider_uuid: None,
            model: String::new(),
            messages: vec![],
            command_blocks: vec![],
            created_at: None,
            updated_at: None,
            version: None,
            created_device_id: None,
            updated_device_id: None,
        };
        database.save_ai_conversation(&base).unwrap();
        database.save_ai_conversation(&base).unwrap();
        database
            .save_ai_conversation(&AiConversationEntry {
                host_uuid: None,
                ..base.clone()
            })
            .unwrap();
        database
            .save_ai_conversation(&AiConversationEntry {
                host_uuid: None,
                ..base.clone()
            })
            .unwrap();

        assert_eq!(
            database
                .list_ai_conversations(Some("terminal"), base.host_uuid.as_deref(), Some(10))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            database
                .list_ai_conversations(Some("terminal"), None, Some(10))
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn saves_and_switches_active_ai_provider() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let first = database
            .save_ai_provider(&AiProviderEntry {
                id: None,
                uuid: None,
                name: "OpenAI".to_string(),
                protocol: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-test".to_string(),
                api_key: "first-key".to_string(),
                config: default_ai_provider_config(),
                active: true,
                created_at: None,
                updated_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();
        assert!(first.uuid.is_some());
        assert_eq!(
            database.get_active_ai_provider().unwrap().unwrap().protocol,
            "openai"
        );

        database
            .save_ai_provider(&AiProviderEntry {
                id: None,
                uuid: None,
                name: "Anthropic".to_string(),
                protocol: "anthropic".to_string(),
                base_url: "https://api.anthropic.com/v1".to_string(),
                model: "claude-test".to_string(),
                api_key: "second-key".to_string(),
                config: Map::from_iter([
                    ("max_tokens".to_string(), json!(8192)),
                    ("temperature".to_string(), json!(0.25)),
                ]),
                active: true,
                created_at: None,
                updated_at: None,
                version: None,
                created_device_id: None,
                updated_device_id: None,
            })
            .unwrap();
        let providers = database.list_ai_providers().unwrap();
        assert_eq!(providers.len(), 2);
        assert_eq!(
            providers.iter().filter(|provider| provider.active).count(),
            1
        );
        assert_eq!(
            database.get_active_ai_provider().unwrap().unwrap().protocol,
            "anthropic"
        );
        assert_eq!(
            database
                .get_active_ai_provider()
                .unwrap()
                .unwrap()
                .config
                .get("max_tokens"),
            Some(&json!(8192))
        );
        assert_eq!(
            database
                .get_active_ai_provider()
                .unwrap()
                .unwrap()
                .config
                .get("temperature"),
            Some(&json!(0.25))
        );
        let active_id = database
            .get_active_ai_provider()
            .unwrap()
            .unwrap()
            .id
            .unwrap();
        assert_eq!(database.delete_ai_provider(active_id).unwrap(), 1);
        assert_eq!(database.list_ai_providers().unwrap().len(), 1);
        assert!(database.get_active_ai_provider().unwrap().is_none());
    }

    #[test]
    fn handles_json_requests() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let group_id = database
            .handle_json_request(r#"{"op":"save_group","group":{"name":"test","parent_id":null}}"#)
            .unwrap();
        assert_eq!(group_id.as_i64(), Some(1));

        let identity_id = database
            .handle_json_request(
                r#"{"op":"save_identity","identity":{"name":"admin","username":"admin","password":"secret","key_id":null}}"#,
            )
            .unwrap();
        assert_eq!(identity_id.as_i64(), Some(1));

        let identities = database
            .handle_json_request(r#"{"op":"list_identities"}"#)
            .unwrap();
        assert!(identities.as_array().unwrap()[0]["password"].is_null());
        let identity = database
            .handle_json_request(r#"{"op":"get_identity","id":1}"#)
            .unwrap();
        assert_eq!(identity["password"].as_str(), Some("secret"));

        let host_id = database
            .handle_json_request(
                r#"{"op":"save_host","host":{"name":"cassandra","group_id":1,"identity_id":1,"type":"remote","host":"10.0.0.12","port":22,"username":"admin","ssh_enabled":true}}"#,
            )
            .unwrap();
        assert_eq!(host_id.as_i64(), Some(1));

        let hosts = database
            .handle_json_request(r#"{"op":"list_hosts","group_id":1}"#)
            .unwrap();
        assert_eq!(hosts.as_array().unwrap().len(), 1);

        let groups = database
            .handle_json_request(r#"{"op":"list_groups"}"#)
            .unwrap();
        assert_eq!(groups.as_array().unwrap().len(), 1);

        let packages = database
            .handle_json_request(r#"{"op":"list_snippet_packages"}"#)
            .unwrap();
        assert!(packages.as_array().unwrap().is_empty());

        let snippet_id = database
            .handle_json_request(
                r#"{"op":"save_snippet","snippet":{"scope":"targeted","description":"Restart","script":"systemctl restart app","target_group_ids":[1],"target_host_ids":[1]}}"#,
            )
            .unwrap();
        assert_eq!(snippet_id.as_i64(), Some(1));

        let snippets = database
            .handle_json_request(r#"{"op":"list_snippets","package_id":null}"#)
            .unwrap();
        assert!(snippets.as_array().unwrap()[0]["package_id"].is_null());
        assert_eq!(snippets.as_array().unwrap()[0]["target_host_ids"][0], 1);
    }
}
