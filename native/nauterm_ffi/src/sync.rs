use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

const ENVELOPE_FORMAT_VERSION: u32 = 2;
const PAYLOAD_FORMAT_VERSION: u32 = 3;
const KEY_VERSION: u32 = 1;
const ARGON2_MEMORY_KIB: u32 = 65_536;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;
const ARGON2_OUTPUT_LENGTH: usize = 32;
const ARGON2_SALT_LENGTH: usize = 16;
const AES_NONCE_LENGTH: usize = 12;
const VAULT_ID_LENGTH: usize = 16;
const SNAPSHOT_ID_LENGTH: usize = 16;
const META_SYNC_DEK: &str = "sync_dek";
const META_SYNC_VAULT_ID: &str = "sync_vault_id";
const META_SYNC_ENVELOPE_HEADER: &str = "sync_envelope_header";

fn aes_nonce(bytes: &[u8]) -> Nonce<aes_gcm::aead::consts::U12> {
    Nonce::try_from(bytes).expect("AES-GCM nonce must be 12 bytes")
}

fn fill_random(bytes: &mut [u8]) {
    getrandom::fill(bytes).expect("operating system random source is unavailable");
}

#[derive(Debug)]
pub(crate) struct SyncError {
    message: String,
}

impl SyncError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SyncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SyncError {}

impl From<rusqlite::Error> for SyncError {
    fn from(error: rusqlite::Error) -> Self {
        Self::new(format!("Database sync failed: {error}"))
    }
}

impl From<std::io::Error> for SyncError {
    fn from(error: std::io::Error) -> Self {
        Self::new(format!("Sync file operation failed: {error}"))
    }
}

impl From<serde_json::Error> for SyncError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(format!("Invalid sync file: {error}"))
    }
}

impl From<crate::crypto::CryptoError> for SyncError {
    fn from(error: crate::crypto::CryptoError) -> Self {
        Self::new(format!("Vault encryption failed: {error}"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct LocalSyncResult {
    pub path: String,
    pub created: bool,
    pub revision: u64,
    pub snapshot_id: String,
    pub imported_records: usize,
    pub total_records: usize,
    pub synced_at: u64,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SyncStrategy {
    #[default]
    SmartMerge,
    LocalWins,
    RemoteWins,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncEnvelope {
    header: SyncEnvelopeHeader,
    payload: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncEnvelopeHeader {
    format_version: u32,
    schema_version: i32,
    vault_id: String,
    key_version: u32,
    revision: u64,
    snapshot_id: String,
    encrypted_at: u64,
    kdf: SyncKdfHeader,
    key_wrap: SyncKeyWrapHeader,
    cipher: SyncCipherHeader,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct RemoteSyncStatus {
    pub revision: u64,
    pub snapshot_id: String,
}

pub(crate) fn inspect_envelope(
    bytes: &[u8],
    current_schema_version: i32,
) -> Result<RemoteSyncStatus, SyncError> {
    let envelope: SyncEnvelope = serde_json::from_slice(bytes)?;
    validate_envelope_header(&envelope.header, current_schema_version)?;
    Ok(RemoteSyncStatus {
        revision: envelope.header.revision,
        snapshot_id: envelope.header.snapshot_id,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncKdfHeader {
    algorithm: String,
    salt: String,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    output_length: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncCipherHeader {
    algorithm: String,
    nonce: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncKeyWrapHeader {
    algorithm: String,
    nonce: String,
    wrapped_sync_dek: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncPayload {
    format_version: u32,
    exported_at: u64,
    source_device_id: String,
    tables: BTreeMap<String, Vec<SyncRecord>>,
}

type SyncRecord = Map<String, Value>;

struct SyncTable {
    name: &'static str,
    columns: &'static [&'static str],
    filter: &'static str,
    snippet_targets: bool,
}

const SYNC_TABLES: &[SyncTable] = &[
    SyncTable {
        name: "groups",
        columns: &[
            "uuid",
            "name",
            "parent_uuid",
            "identity_uuid",
            "proxy_uuid",
            "port",
            "username",
            "password",
            "theme_id",
            "startup_snippet_uuid",
            "ssh_enabled",
            "mosh_enabled",
            "mosh_server_command",
            "telnet_enabled",
            "telnet_identity_uuid",
            "telnet_username",
            "telnet_password",
            "telnet_port",
            "telnet_theme_id",
            "environment_variables",
            "encoding",
            "telnet_encoding",
            "key_uuid",
            "created_at",
            "updated_at",
            "deleted_at",
            "version",
            "created_device_id",
            "updated_device_id",
        ],
        filter: "",
        snippet_targets: false,
    },
    SyncTable {
        name: "keys",
        columns: &[
            "uuid",
            "name",
            "private_key",
            "public_key",
            "certificate",
            "passphrase",
            "created_at",
            "updated_at",
            "deleted_at",
            "version",
            "created_device_id",
            "updated_device_id",
        ],
        filter: "",
        snippet_targets: false,
    },
    SyncTable {
        name: "identities",
        columns: &[
            "uuid",
            "name",
            "username",
            "password",
            "key_uuid",
            "created_at",
            "updated_at",
            "deleted_at",
            "version",
            "created_device_id",
            "updated_device_id",
        ],
        filter: "",
        snippet_targets: false,
    },
    SyncTable {
        name: "proxies",
        columns: &[
            "uuid",
            "name",
            "type",
            "host",
            "port",
            "identity_uuid",
            "username",
            "password",
            "created_at",
            "updated_at",
            "deleted_at",
            "version",
            "created_device_id",
            "updated_device_id",
        ],
        filter: "",
        snippet_targets: false,
    },
    SyncTable {
        name: "hosts",
        columns: &[
            "uuid",
            "name",
            "group_uuid",
            "identity_uuid",
            "proxy_uuid",
            "host",
            "port",
            "username",
            "password",
            "theme_id",
            "startup_snippet_uuid",
            "ssh_enabled",
            "mosh_enabled",
            "mosh_server_command",
            "telnet_enabled",
            "telnet_identity_uuid",
            "telnet_username",
            "telnet_password",
            "telnet_port",
            "telnet_theme_id",
            "environment_variables",
            "encoding",
            "telnet_encoding",
            "type",
            "key_uuid",
            "shell_path",
            "work_dir",
            "created_at",
            "updated_at",
            "deleted_at",
            "version",
            "created_device_id",
            "updated_device_id",
        ],
        filter: "WHERE type = 'remote'",
        snippet_targets: false,
    },
    SyncTable {
        name: "tags",
        columns: &[
            "uuid",
            "name",
            "created_at",
            "updated_at",
            "deleted_at",
            "version",
            "created_device_id",
            "updated_device_id",
        ],
        filter: "",
        snippet_targets: false,
    },
    SyncTable {
        name: "port_forwards",
        columns: &[
            "uuid",
            "name",
            "type",
            "bind_address",
            "bind_port",
            "destination_host",
            "destination_port",
            "host_uuid",
            "created_at",
            "updated_at",
            "deleted_at",
            "version",
            "created_device_id",
            "updated_device_id",
        ],
        filter: "WHERE host_uuid IN (SELECT uuid FROM hosts WHERE type = 'remote')",
        snippet_targets: false,
    },
    SyncTable {
        name: "snippet_packages",
        columns: &[
            "uuid",
            "name",
            "created_at",
            "updated_at",
            "deleted_at",
            "version",
            "created_device_id",
            "updated_device_id",
        ],
        filter: "",
        snippet_targets: false,
    },
    SyncTable {
        name: "snippets",
        columns: &[
            "uuid",
            "package_uuid",
            "scope",
            "description",
            "script",
            "created_at",
            "updated_at",
            "deleted_at",
            "version",
            "created_device_id",
            "updated_device_id",
        ],
        filter: "",
        snippet_targets: true,
    },
    SyncTable {
        name: "sftp_favorites",
        columns: &[
            "uuid",
            "scope",
            "host_uuid",
            "path",
            "created_at",
            "updated_at",
            "deleted_at",
            "version",
            "created_device_id",
            "updated_device_id",
        ],
        filter: "WHERE scope = 'remote'",
        snippet_targets: false,
    },
];

#[cfg(test)]
pub(crate) fn sync_local_file(
    connection: &mut Connection,
    path: &str,
    master_key: Option<&str>,
) -> Result<LocalSyncResult, SyncError> {
    sync_file(
        connection,
        path,
        master_key,
        None,
        SyncFileKind::LinkedLocal,
        SyncStrategy::SmartMerge,
    )
}

#[cfg(test)]
pub(crate) fn rotate_local_file_master_key(
    connection: &mut Connection,
    path: &str,
    current_master_key: &str,
    new_master_key: &str,
) -> Result<LocalSyncResult, SyncError> {
    sync_file(
        connection,
        path,
        Some(current_master_key),
        Some(new_master_key),
        SyncFileKind::LinkedLocal,
        SyncStrategy::SmartMerge,
    )
}

pub(crate) fn sync_provider_staging_file(
    connection: &mut Connection,
    path: &str,
    master_key: Option<&str>,
    revision_scope: &str,
    strategy: SyncStrategy,
) -> Result<LocalSyncResult, SyncError> {
    sync_file(
        connection,
        path,
        master_key,
        None,
        SyncFileKind::ProviderStaging(revision_scope.to_string()),
        strategy,
    )
}

pub(crate) fn rotate_provider_staging_file_master_key(
    connection: &mut Connection,
    path: &str,
    current_master_key: &str,
    new_master_key: &str,
    revision_scope: &str,
) -> Result<LocalSyncResult, SyncError> {
    sync_file(
        connection,
        path,
        Some(current_master_key),
        Some(new_master_key),
        SyncFileKind::ProviderStaging(revision_scope.to_string()),
        SyncStrategy::SmartMerge,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SyncFileKind {
    LinkedLocal,
    ProviderStaging(String),
}

impl SyncFileKind {
    fn revision_scope(&self) -> Option<&str> {
        match self {
            Self::LinkedLocal => None,
            Self::ProviderStaging(scope) => Some(scope.as_str()),
        }
    }
}

fn sync_file(
    connection: &mut Connection,
    path: &str,
    master_key: Option<&str>,
    new_master_key: Option<&str>,
    file_kind: SyncFileKind,
    strategy: SyncStrategy,
) -> Result<LocalSyncResult, SyncError> {
    let path = PathBuf::from(path);
    if path.as_os_str().is_empty() {
        return Err(SyncError::new("Choose a local sync file first."));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        return Err(SyncError::new(
            "The folder containing the sync file does not exist.",
        ));
    }

    let schema_version: i32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let original_bytes = if path.exists() {
        Some(fs::read(&path)?)
    } else {
        None
    };
    let created = original_bytes.is_none();
    let mut imported_records = 0;
    let local_key = load_local_sync_dek(connection)?;
    let local_header = load_local_sync_envelope_header(connection)?;
    let (mut previous_header, sync_key) = if let Some(bytes) = original_bytes.as_deref() {
        let envelope: SyncEnvelope = serde_json::from_slice(bytes)?;
        if let Some(local_vault_id) = load_local_sync_vault_id(connection)? {
            if local_vault_id != envelope.header.vault_id {
                return Err(SyncError::new(
                    "This device is already linked to a different sync vault.",
                ));
            }
        }
        let (remote_header, remote_payload, key) =
            decrypt_envelope(envelope, master_key, local_key.as_deref(), schema_version)?;
        reject_rollback(connection, &remote_header, file_kind.revision_scope())?;
        imported_records = match strategy {
            SyncStrategy::SmartMerge => merge_payload(connection, &remote_payload)?,
            SyncStrategy::LocalWins => 0,
            SyncStrategy::RemoteWins => replace_payload(connection, &remote_payload)?,
        };
        let header = match local_header {
            Some(mut local_header) => {
                if local_header.vault_id != remote_header.vault_id {
                    return Err(SyncError::new(
                        "This device is already linked to a different sync vault.",
                    ));
                }
                local_header.revision = local_header.revision.max(remote_header.revision);
                local_header
            }
            None => remote_header,
        };
        (Some(header), key)
    } else if let Some(key) = local_key {
        if file_kind == SyncFileKind::LinkedLocal {
            return Err(SyncError::new(
                "The linked sync file is missing. Restore it or unlink this sync vault first.",
            ));
        }
        let header = match local_header {
            Some(header) => header,
            None => rebuild_envelope_header(connection, master_key, schema_version, &key)?,
        };
        (Some(header), key)
    } else {
        let master_key = master_key.ok_or_else(|| {
            SyncError::new("Enter a Master Key to create the encrypted sync vault.")
        })?;
        crate::crypto::validate_master_key(master_key)?;
        let mut key = Zeroizing::new([0u8; 32]);
        fill_random(key.as_mut_slice());
        (None, key)
    };

    if let Some(new_master_key) = new_master_key {
        let current_master_key = master_key
            .ok_or_else(|| SyncError::new("Enter the current Master Key before changing it."))?;
        crate::crypto::validate_master_key(current_master_key)?;
        crate::crypto::validate_master_key(new_master_key)?;
        let header = previous_header.take().ok_or_else(|| {
            SyncError::new("Create the encrypted sync vault before changing its Master Key.")
        })?;
        let current_matches = unwrap_sync_dek(current_master_key, &header)
            .map(|key| key.as_slice() == sync_key.as_slice())
            .unwrap_or(false);
        if current_matches {
            previous_header = Some(rewrap_envelope_header(header, new_master_key, &sync_key)?);
        } else {
            // A retry can encounter providers already updated by a partially
            // successful earlier attempt. Treat the new wrapping as complete.
            let new_matches = unwrap_sync_dek(new_master_key, &header)
                .map(|key| key.as_slice() == sync_key.as_slice())
                .unwrap_or(false);
            if !new_matches {
                return Err(SyncError::new("The current Master Key is incorrect."));
            }
            previous_header = Some(header);
        }
    }

    let payload = export_payload(connection)?;
    let total_records = payload.tables.values().map(Vec::len).sum();
    let envelope = encrypt_payload(
        payload,
        master_key,
        schema_version,
        previous_header,
        &sync_key,
    )?;
    let bytes = serde_json::to_vec_pretty(&envelope)?;

    if let Some(original) = original_bytes.as_deref() {
        let current = fs::read(&path)?;
        if Sha256::digest(&current) != Sha256::digest(original) {
            return Err(SyncError::new(
                "The sync file changed while syncing. Run Sync Now again.",
            ));
        }
    }
    write_atomically(&path, &bytes)?;
    save_local_sync_state(connection, &envelope.header, &sync_key)?;
    remember_revision(connection, &envelope.header, None)?;
    if let Some(scope) = file_kind.revision_scope() {
        remember_revision(connection, &envelope.header, Some(scope))?;
    }

    Ok(LocalSyncResult {
        path: path.to_string_lossy().into_owned(),
        created,
        revision: envelope.header.revision,
        snapshot_id: envelope.header.snapshot_id.clone(),
        imported_records,
        total_records,
        synced_at: envelope.header.encrypted_at,
    })
}

fn rewrap_envelope_header(
    mut header: SyncEnvelopeHeader,
    new_master_key: &str,
    sync_dek: &[u8; 32],
) -> Result<SyncEnvelopeHeader, SyncError> {
    let mut salt = [0u8; ARGON2_SALT_LENGTH];
    fill_random(&mut salt);
    header.kdf = SyncKdfHeader {
        algorithm: "argon2id".to_string(),
        salt: STANDARD.encode(salt),
        memory_kib: ARGON2_MEMORY_KIB,
        iterations: ARGON2_ITERATIONS,
        parallelism: ARGON2_PARALLELISM,
        output_length: ARGON2_OUTPUT_LENGTH,
    };
    header.key_wrap = wrap_sync_dek(
        new_master_key,
        &header.vault_id,
        header.key_version,
        &header.kdf,
        sync_dek,
    )?;
    Ok(header)
}

fn load_local_sync_dek(connection: &Connection) -> Result<Option<Zeroizing<[u8; 32]>>, SyncError> {
    let encoded = connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = ?",
            params![META_SYNC_DEK],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(encoded) = encoded else {
        return Ok(None);
    };
    let bytes = Zeroizing::new(
        STANDARD
            .decode(encoded)
            .map_err(|_| SyncError::new("The saved Sync DEK is corrupt."))?,
    );
    if bytes.len() != 32 {
        return Err(SyncError::new("The saved Sync DEK has an invalid length."));
    }
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&bytes);
    Ok(Some(key))
}

fn load_local_sync_vault_id(connection: &Connection) -> Result<Option<String>, SyncError> {
    connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = ?",
            params![META_SYNC_VAULT_ID],
            |row| row.get(0),
        )
        .optional()
        .map_err(SyncError::from)
}

fn load_local_sync_envelope_header(
    connection: &Connection,
) -> Result<Option<SyncEnvelopeHeader>, SyncError> {
    let encoded = connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = ?",
            params![META_SYNC_ENVELOPE_HEADER],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    encoded
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                SyncError::new(format!("The saved sync header is corrupt: {error}"))
            })
        })
        .transpose()
}

fn rebuild_envelope_header(
    connection: &Connection,
    master_key: Option<&str>,
    schema_version: i32,
    sync_dek: &[u8; 32],
) -> Result<SyncEnvelopeHeader, SyncError> {
    let master_key = master_key.ok_or_else(|| {
        SyncError::new("Enter the Master Key once to finish upgrading this sync vault.")
    })?;
    crate::crypto::validate_master_key(master_key)?;
    let vault_id = load_local_sync_vault_id(connection)?
        .ok_or_else(|| SyncError::new("The saved sync vault identity is missing."))?;
    let mut salt = [0u8; ARGON2_SALT_LENGTH];
    let mut nonce = [0u8; AES_NONCE_LENGTH];
    fill_random(&mut salt);
    fill_random(&mut nonce);
    let kdf = SyncKdfHeader {
        algorithm: "argon2id".to_string(),
        salt: STANDARD.encode(salt),
        memory_kib: ARGON2_MEMORY_KIB,
        iterations: ARGON2_ITERATIONS,
        parallelism: ARGON2_PARALLELISM,
        output_length: ARGON2_OUTPUT_LENGTH,
    };
    let key_wrap = wrap_sync_dek(master_key, &vault_id, KEY_VERSION, &kdf, sync_dek)?;
    let revision = load_last_seen_revision(connection, &vault_id, None)?.unwrap_or(0);
    Ok(SyncEnvelopeHeader {
        format_version: ENVELOPE_FORMAT_VERSION,
        schema_version,
        vault_id,
        key_version: KEY_VERSION,
        revision,
        snapshot_id: String::new(),
        encrypted_at: unix_millis(),
        kdf,
        key_wrap,
        cipher: SyncCipherHeader {
            algorithm: "aes-256-gcm".to_string(),
            nonce: STANDARD.encode(nonce),
        },
    })
}

fn save_local_sync_state(
    connection: &Connection,
    header: &SyncEnvelopeHeader,
    sync_dek: &[u8; 32],
) -> Result<(), SyncError> {
    let encoded = Zeroizing::new(STANDARD.encode(sync_dek));
    let encoded_header = serde_json::to_string(header)?;
    let transaction = connection.unchecked_transaction()?;
    for (key, value) in [
        (META_SYNC_DEK, encoded.as_str()),
        (META_SYNC_VAULT_ID, header.vault_id.as_str()),
        (META_SYNC_ENVELOPE_HEADER, encoded_header.as_str()),
    ] {
        transaction.execute(
            r#"
            INSERT INTO app_metadata (key, value)
            VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
            "#,
            params![key, value],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

fn revision_metadata_key(vault_id: &str, scope: Option<&str>) -> String {
    match scope {
        Some(scope) => format!("sync_revision:{scope}:{vault_id}"),
        None => format!("sync_revision:{vault_id}"),
    }
}

fn reject_rollback(
    connection: &Connection,
    header: &SyncEnvelopeHeader,
    scope: Option<&str>,
) -> Result<(), SyncError> {
    let last_seen = load_last_seen_revision(connection, &header.vault_id, scope)?;
    if let Some(last_seen) = last_seen {
        if header.revision < last_seen {
            return Err(SyncError::new(format!(
                "The sync vault revision moved backwards (remote {}, last seen {last_seen}). Refusing a possible rollback.",
                header.revision
            )));
        }
    }
    Ok(())
}

fn load_last_seen_revision(
    connection: &Connection,
    vault_id: &str,
    scope: Option<&str>,
) -> Result<Option<u64>, SyncError> {
    let key = revision_metadata_key(vault_id, scope);
    Ok(connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = ?",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .and_then(|value| value.parse::<u64>().ok()))
}

fn remember_revision(
    connection: &Connection,
    header: &SyncEnvelopeHeader,
    scope: Option<&str>,
) -> Result<(), SyncError> {
    let key = revision_metadata_key(&header.vault_id, scope);
    connection.execute(
        r#"
        INSERT INTO app_metadata (key, value)
        VALUES (?, ?)
        ON CONFLICT(key) DO UPDATE SET
          value = excluded.value,
          updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)
        "#,
        params![key, header.revision.to_string()],
    )?;
    Ok(())
}

fn export_payload(connection: &Connection) -> Result<SyncPayload, SyncError> {
    let dek = crate::crypto::local_dek_if_configured(connection)?;
    let source_device_id: String = connection.query_row(
        "SELECT value FROM app_metadata WHERE key = 'device_id'",
        [],
        |row| row.get(0),
    )?;
    let mut tables = BTreeMap::new();
    for table in SYNC_TABLES {
        tables.insert(
            table.name.to_string(),
            export_table(connection, table, dek.as_ref())?,
        );
    }
    Ok(SyncPayload {
        format_version: PAYLOAD_FORMAT_VERSION,
        exported_at: unix_millis(),
        source_device_id,
        tables,
    })
}

fn export_table(
    connection: &Connection,
    table: &SyncTable,
    dek: Option<&crate::crypto::Dek>,
) -> Result<Vec<SyncRecord>, SyncError> {
    let sql = format!(
        "SELECT {} FROM {} {} ORDER BY uuid ASC",
        table.columns.join(", "),
        table.name,
        table.filter
    );
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query([])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        let mut record = Map::new();
        for (index, column) in table.columns.iter().enumerate() {
            let mut value = json_from_sql_value(row.get_ref(index)?)?;
            if is_sensitive_field(table.name, column) {
                if let (Some(dek), Some(encrypted)) = (dek, value.as_str()) {
                    value = Value::String(crate::crypto::decrypt_field(dek, encrypted)?);
                }
            }
            record.insert((*column).to_string(), value);
        }
        if table.snippet_targets {
            let uuid = record_string(&record, "uuid")?.to_string();
            record.insert(
                "target_group_uuids".to_string(),
                Value::Array(
                    list_relation_uuids(
                        connection,
                        "snippet_target_groups",
                        "group_uuid",
                        &uuid,
                        "",
                    )?
                    .into_iter()
                    .map(Value::String)
                    .collect(),
                ),
            );
            record.insert(
                "target_host_uuids".to_string(),
                Value::Array(
                    list_relation_uuids(
                        connection,
                        "snippet_target_hosts",
                        "host_uuid",
                        &uuid,
                        "AND host_uuid IN (SELECT uuid FROM hosts WHERE type = 'remote')",
                    )?
                    .into_iter()
                    .map(Value::String)
                    .collect(),
                ),
            );
        }
        if table.name == "hosts" {
            let uuid = record_string(&record, "uuid")?.to_string();
            record.insert(
                "tag_uuids".to_string(),
                Value::String(crate::database::tag_uuids_json(&list_host_tag_uuids(
                    connection, &uuid,
                )?)),
            );
        }
        records.push(record);
    }
    Ok(records)
}

fn list_host_tag_uuids(connection: &Connection, host_uuid: &str) -> Result<Vec<String>, SyncError> {
    let mut statement = connection.prepare(
        "SELECT tag_uuid FROM host_tags
         WHERE host_uuid = ? ORDER BY tag_uuid ASC",
    )?;
    let values = statement
        .query_map(params![host_uuid], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(values)
}

fn list_relation_uuids(
    connection: &Connection,
    table: &str,
    relation_column: &str,
    snippet_uuid: &str,
    filter: &str,
) -> Result<Vec<String>, SyncError> {
    let sql = format!(
        "SELECT {relation_column} FROM {table} WHERE snippet_uuid = ? {filter} ORDER BY {relation_column} ASC"
    );
    let mut statement = connection.prepare(&sql)?;
    let values = statement
        .query_map(params![snippet_uuid], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(values)
}

fn merge_payload(connection: &mut Connection, payload: &SyncPayload) -> Result<usize, SyncError> {
    validate_payload_tables(payload)?;
    let dek = crate::crypto::local_dek_if_configured(connection)?;
    let transaction = connection.transaction()?;
    let imported = merge_payload_in_transaction(&transaction, payload, dek.as_ref())?;
    transaction.commit()?;
    Ok(imported)
}

fn merge_payload_in_transaction(
    transaction: &Transaction<'_>,
    payload: &SyncPayload,
    dek: Option<&crate::crypto::Dek>,
) -> Result<usize, SyncError> {
    let mut imported = 0;
    // Host tag relations are restored while importing each host, so tags must
    // exist even when the destination database starts empty.
    for table in SYNC_TABLES
        .iter()
        .filter(|table| table.name == "tags")
        .chain(SYNC_TABLES.iter().filter(|table| table.name != "tags"))
    {
        let records = payload
            .tables
            .get(table.name)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for record in records {
            if merge_record(transaction, table, record, dek)? {
                imported += 1;
            }
        }
    }
    Ok(imported)
}

fn replace_payload(connection: &mut Connection, payload: &SyncPayload) -> Result<usize, SyncError> {
    validate_payload_tables(payload)?;
    let dek = crate::crypto::local_dek_if_configured(connection)?;
    let transaction = connection.transaction()?;
    transaction.execute("DELETE FROM snippet_target_groups", [])?;
    transaction.execute("DELETE FROM snippet_target_hosts", [])?;
    transaction.execute("DELETE FROM host_tags", [])?;
    for table in SYNC_TABLES.iter().rev() {
        transaction.execute(&format!("DELETE FROM {} {}", table.name, table.filter), [])?;
    }
    let imported = merge_payload_in_transaction(&transaction, payload, dek.as_ref())?;
    transaction.commit()?;
    Ok(imported)
}

fn validate_payload_tables(payload: &SyncPayload) -> Result<(), SyncError> {
    if payload.format_version != PAYLOAD_FORMAT_VERSION {
        return Err(SyncError::new(format!(
            "Unsupported sync payload version {}.",
            payload.format_version
        )));
    }
    let allowed_tables = SYNC_TABLES
        .iter()
        .map(|table| table.name)
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = payload
        .tables
        .keys()
        .find(|name| !allowed_tables.contains(name.as_str()))
    {
        return Err(SyncError::new(format!(
            "Unsupported table in sync file: {unknown}."
        )));
    }
    Ok(())
}

fn merge_record(
    transaction: &Transaction<'_>,
    table: &SyncTable,
    record: &SyncRecord,
    dek: Option<&crate::crypto::Dek>,
) -> Result<bool, SyncError> {
    validate_record(table, record)?;
    let uuid = record_string(record, "uuid")?;
    let local_stamp = transaction
        .query_row(
            &format!(
                "SELECT updated_at, version, updated_device_id FROM {} WHERE uuid = ?",
                table.name
            ),
            params![uuid],
            |row| {
                Ok(SyncStamp {
                    updated_at: row.get::<_, i64>(0)?,
                    version: row.get::<_, i64>(1)?,
                    device_id: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                })
            },
        )
        .optional()?;
    let remote_stamp = SyncStamp {
        updated_at: record_i64(record, "updated_at")?,
        version: record_i64(record, "version")?,
        device_id: record
            .get("updated_device_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    };
    if local_stamp
        .as_ref()
        .is_some_and(|local| !remote_stamp.is_newer_than(local))
    {
        return Ok(false);
    }

    let values = table
        .columns
        .iter()
        .map(|column| {
            let Some(value) = record.get(*column) else {
                return Err(SyncError::new(format!(
                    "Missing {}.{} in sync record.",
                    table.name, column
                )));
            };
            if let Some(target_table) = weak_reference_target(table.name, column) {
                let Some(uuid) = value.as_str().filter(|value| is_uuid_value(value)) else {
                    return Ok(SqlValue::Null);
                };
                let exists = transaction.query_row(
                    &format!("SELECT EXISTS(SELECT 1 FROM {target_table} WHERE uuid = ?)"),
                    params![uuid],
                    |row| row.get::<_, bool>(0),
                )?;
                return Ok(if exists {
                    SqlValue::Text(uuid.to_string())
                } else {
                    SqlValue::Null
                });
            }
            if is_sensitive_field(table.name, column) {
                if let (Some(dek), Some(plaintext)) = (dek, value.as_str()) {
                    return Ok(SqlValue::Text(crate::crypto::encrypt_field(
                        dek, plaintext,
                    )?));
                }
            }
            sql_value_from_json(value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let assignments = table
        .columns
        .iter()
        .filter(|column| **column != "uuid")
        .map(|column| format!("{column} = excluded.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = vec!["?"; table.columns.len()].join(", ");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT(uuid) DO UPDATE SET {}",
        table.name,
        table.columns.join(", "),
        placeholders,
        assignments
    );
    transaction.execute(&sql, params_from_iter(values))?;

    if table.snippet_targets {
        replace_snippet_targets(transaction, uuid, record)?;
    }
    if table.name == "hosts" {
        replace_synced_host_tags(transaction, uuid, record.get("tag_uuids"))?;
    }
    Ok(true)
}

fn weak_reference_target(table: &str, column: &str) -> Option<&'static str> {
    match (table, column) {
        ("identities", "key_uuid") => Some("keys"),
        ("proxies", "identity_uuid") => Some("identities"),
        ("hosts", "group_uuid") => Some("groups"),
        ("hosts", "key_uuid") => Some("keys"),
        ("hosts", "identity_uuid" | "telnet_identity_uuid") => Some("identities"),
        ("hosts", "proxy_uuid") => Some("proxies"),
        ("port_forwards", "host_uuid") => Some("hosts"),
        ("snippets", "package_uuid") => Some("snippet_packages"),
        ("sftp_favorites", "host_uuid") => Some("hosts"),
        _ => None,
    }
}

fn is_sensitive_field(table: &str, column: &str) -> bool {
    matches!(
        (table, column),
        ("keys", "private_key")
            | ("keys", "certificate")
            | ("keys", "passphrase")
            | ("identities", "password")
            | ("groups", "password")
            | ("groups", "telnet_password")
            | ("hosts", "password")
            | ("hosts", "telnet_password")
            | ("proxies", "password")
    )
}

fn validate_record(table: &SyncTable, record: &SyncRecord) -> Result<(), SyncError> {
    for column in table.columns {
        if !record.contains_key(*column) {
            return Err(SyncError::new(format!(
                "Sync record in {} is missing {column}.",
                table.name
            )));
        }
    }
    let uuid = record_string(record, "uuid")?;
    if !is_uuid_value(uuid) {
        return Err(SyncError::new(format!(
            "Sync record in {} contains an invalid UUID.",
            table.name
        )));
    }
    for key in record.keys() {
        let synthetic = (table.snippet_targets
            && matches!(key.as_str(), "target_group_uuids" | "target_host_uuids"))
            || (table.name == "hosts" && key == "tag_uuids");
        if !synthetic && !table.columns.contains(&key.as_str()) {
            return Err(SyncError::new(format!(
                "Sync record in {} contains an unsupported field: {key}.",
                table.name
            )));
        }
    }
    Ok(())
}

fn replace_synced_host_tags(
    transaction: &Transaction<'_>,
    host_uuid: &str,
    value: Option<&Value>,
) -> Result<(), SyncError> {
    transaction.execute(
        "DELETE FROM host_tags WHERE host_uuid = ?",
        params![host_uuid],
    )?;
    let values = value
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
        .unwrap_or_default();
    for tag_uuid in values {
        if !is_uuid_value(&tag_uuid) {
            continue;
        }
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
    Ok(())
}

fn replace_snippet_targets(
    transaction: &Transaction<'_>,
    snippet_uuid: &str,
    record: &SyncRecord,
) -> Result<(), SyncError> {
    transaction.execute(
        "DELETE FROM snippet_target_groups WHERE snippet_uuid = ?",
        params![snippet_uuid],
    )?;
    transaction.execute(
        "DELETE FROM snippet_target_hosts WHERE snippet_uuid = ?",
        params![snippet_uuid],
    )?;
    insert_snippet_targets(
        transaction,
        "snippet_target_groups",
        "group_uuid",
        snippet_uuid,
        record.get("target_group_uuids"),
    )?;
    insert_snippet_targets(
        transaction,
        "snippet_target_hosts",
        "host_uuid",
        snippet_uuid,
        record.get("target_host_uuids"),
    )
}

fn insert_snippet_targets(
    transaction: &Transaction<'_>,
    table: &str,
    relation_column: &str,
    snippet_uuid: &str,
    value: Option<&Value>,
) -> Result<(), SyncError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| SyncError::new(format!("Missing {relation_column} values.")))?;
    let target_table = match relation_column {
        "group_uuid" => "groups",
        "host_uuid" => "hosts",
        _ => return Err(SyncError::new("Unsupported snippet target relation.")),
    };
    let sql = format!(
        "INSERT OR IGNORE INTO {table} (snippet_uuid, {relation_column}) \
         SELECT ?, ? WHERE EXISTS (SELECT 1 FROM {target_table} WHERE uuid = ?)"
    );
    for value in values {
        let uuid = value
            .as_str()
            .filter(|value| is_uuid_value(value))
            .ok_or_else(|| SyncError::new(format!("Invalid {relation_column} value.")))?;
        transaction.execute(&sql, params![snippet_uuid, uuid, uuid])?;
    }
    Ok(())
}

#[derive(Debug)]
struct SyncStamp {
    updated_at: i64,
    version: i64,
    device_id: String,
}

impl SyncStamp {
    fn is_newer_than(&self, other: &Self) -> bool {
        (self.updated_at, self.version, self.device_id.as_str())
            > (other.updated_at, other.version, other.device_id.as_str())
    }
}

fn encrypt_payload(
    payload: SyncPayload,
    master_key: Option<&str>,
    schema_version: i32,
    previous_header: Option<SyncEnvelopeHeader>,
    sync_key: &[u8; 32],
) -> Result<SyncEnvelope, SyncError> {
    let now = unix_millis();
    let (vault_id, key_version, revision, kdf, key_wrap) = match previous_header {
        Some(header) => (
            header.vault_id,
            header.key_version,
            header.revision.saturating_add(1),
            header.kdf,
            header.key_wrap,
        ),
        None => {
            let master_key = master_key.ok_or_else(|| {
                SyncError::new("Enter a Master Key to create the encrypted sync vault.")
            })?;
            crate::crypto::validate_master_key(master_key)?;
            let mut vault_id = [0u8; VAULT_ID_LENGTH];
            let mut salt = [0u8; ARGON2_SALT_LENGTH];
            fill_random(&mut vault_id);
            fill_random(&mut salt);
            let vault_id = URL_SAFE_NO_PAD.encode(vault_id);
            let kdf = SyncKdfHeader {
                algorithm: "argon2id".to_string(),
                salt: STANDARD.encode(salt),
                memory_kib: ARGON2_MEMORY_KIB,
                iterations: ARGON2_ITERATIONS,
                parallelism: ARGON2_PARALLELISM,
                output_length: ARGON2_OUTPUT_LENGTH,
            };
            let key_wrap = wrap_sync_dek(master_key, &vault_id, KEY_VERSION, &kdf, sync_key)?;
            (vault_id, KEY_VERSION, 1, kdf, key_wrap)
        }
    };
    validate_kdf_header(&kdf)?;
    let mut nonce = [0u8; AES_NONCE_LENGTH];
    let mut snapshot_id = [0u8; SNAPSHOT_ID_LENGTH];
    fill_random(&mut nonce);
    fill_random(&mut snapshot_id);
    let header = SyncEnvelopeHeader {
        format_version: ENVELOPE_FORMAT_VERSION,
        schema_version,
        vault_id,
        key_version,
        revision,
        snapshot_id: URL_SAFE_NO_PAD.encode(snapshot_id),
        encrypted_at: now,
        kdf,
        key_wrap,
        cipher: SyncCipherHeader {
            algorithm: "aes-256-gcm".to_string(),
            nonce: STANDARD.encode(nonce),
        },
    };
    let aad = serde_json::to_vec(&header)?;
    let plaintext = Zeroizing::new(serde_json::to_vec(&payload)?);
    let cipher = Aes256Gcm::new_from_slice(sync_key)
        .map_err(|_| SyncError::new("Unable to initialize sync encryption."))?;
    let ciphertext = cipher
        .encrypt(
            &aes_nonce(&nonce),
            Payload {
                msg: plaintext.as_slice(),
                aad: &aad,
            },
        )
        .map_err(|_| SyncError::new("Unable to encrypt the sync file."))?;
    Ok(SyncEnvelope {
        header,
        payload: STANDARD.encode(ciphertext),
    })
}

fn decrypt_envelope(
    envelope: SyncEnvelope,
    master_key: Option<&str>,
    local_sync_key: Option<&[u8; 32]>,
    current_schema_version: i32,
) -> Result<(SyncEnvelopeHeader, SyncPayload, Zeroizing<[u8; 32]>), SyncError> {
    validate_envelope_header(&envelope.header, current_schema_version)?;
    let nonce = STANDARD
        .decode(&envelope.header.cipher.nonce)
        .map_err(|_| SyncError::new("The sync file contains an invalid nonce."))?;
    if nonce.len() != AES_NONCE_LENGTH {
        return Err(SyncError::new(
            "The sync file contains an invalid nonce length.",
        ));
    }
    let ciphertext = STANDARD
        .decode(&envelope.payload)
        .map_err(|_| SyncError::new("The encrypted sync payload is invalid."))?;
    let aad = serde_json::to_vec(&envelope.header)?;
    let key = match local_sync_key {
        Some(key) => Zeroizing::new(*key),
        None => {
            let master_key = master_key.ok_or_else(|| {
                SyncError::new("Enter the Master Key once to unlock this sync vault.")
            })?;
            crate::crypto::validate_master_key(master_key)?;
            unwrap_sync_dek(master_key, &envelope.header)?
        }
    };
    let cipher = Aes256Gcm::new_from_slice(key.as_slice())
        .map_err(|_| SyncError::new("Unable to initialize sync decryption."))?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                &aes_nonce(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| {
                SyncError::new(
                    "Unable to decrypt the sync file. Check the Master Key or file integrity.",
                )
            })?,
    );
    let payload = serde_json::from_slice::<SyncPayload>(&plaintext)?;
    Ok((envelope.header, payload, key))
}

fn validate_envelope_header(
    header: &SyncEnvelopeHeader,
    current_schema_version: i32,
) -> Result<(), SyncError> {
    if header.format_version != ENVELOPE_FORMAT_VERSION {
        return Err(SyncError::new(format!(
            "Unsupported sync file version {}.",
            header.format_version
        )));
    }
    if header.schema_version != current_schema_version {
        return Err(SyncError::new(format!(
            "Unsupported sync database schema version {}.",
            header.schema_version
        )));
    }
    if header.key_version != KEY_VERSION {
        return Err(SyncError::new(format!(
            "Unsupported sync key version {}.",
            header.key_version
        )));
    }
    if header.cipher.algorithm != "aes-256-gcm" {
        return Err(SyncError::new("Unsupported sync cipher."));
    }
    if header.key_wrap.algorithm != "aes-256-gcm" {
        return Err(SyncError::new("Unsupported Sync DEK wrapping cipher."));
    }
    let vault_id = URL_SAFE_NO_PAD
        .decode(&header.vault_id)
        .map_err(|_| SyncError::new("The sync file contains an invalid vault ID."))?;
    if vault_id.len() != VAULT_ID_LENGTH {
        return Err(SyncError::new(
            "The sync file contains an invalid vault ID.",
        ));
    }
    if !header.snapshot_id.is_empty() {
        let snapshot_id = URL_SAFE_NO_PAD
            .decode(&header.snapshot_id)
            .map_err(|_| SyncError::new("The sync file contains an invalid snapshot ID."))?;
        if snapshot_id.len() != SNAPSHOT_ID_LENGTH {
            return Err(SyncError::new(
                "The sync file contains an invalid snapshot ID.",
            ));
        }
    }
    validate_kdf_header(&header.kdf)
}

fn validate_kdf_header(kdf: &SyncKdfHeader) -> Result<(), SyncError> {
    if kdf.algorithm != "argon2id"
        || !(8_192..=262_144).contains(&kdf.memory_kib)
        || !(1..=10).contains(&kdf.iterations)
        || !(1..=8).contains(&kdf.parallelism)
        || kdf.output_length != ARGON2_OUTPUT_LENGTH
    {
        return Err(SyncError::new(
            "The sync file contains unsupported key derivation parameters.",
        ));
    }
    let salt = STANDARD
        .decode(&kdf.salt)
        .map_err(|_| SyncError::new("The sync file contains an invalid KDF salt."))?;
    if !(16..=32).contains(&salt.len()) {
        return Err(SyncError::new(
            "The sync file contains an invalid KDF salt length.",
        ));
    }
    Ok(())
}

fn derive_wrapping_key(master_key: &str, kdf: &SyncKdfHeader) -> Result<[u8; 32], SyncError> {
    let normalized = Zeroizing::new(master_key.nfc().collect::<String>());
    let salt = STANDARD
        .decode(&kdf.salt)
        .map_err(|_| SyncError::new("The sync file contains an invalid KDF salt."))?;
    let params = Params::new(
        kdf.memory_kib,
        kdf.iterations,
        kdf.parallelism,
        Some(kdf.output_length),
    )
    .map_err(|_| SyncError::new("Invalid Argon2id parameters."))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut root_key = Zeroizing::new([0u8; ARGON2_OUTPUT_LENGTH]);
    argon2
        .hash_password_into(normalized.as_bytes(), &salt, root_key.as_mut())
        .map_err(|_| SyncError::new("Unable to derive the sync encryption key."))?;

    let mut key = [0u8; 32];
    key.copy_from_slice(root_key.as_slice());
    Ok(key)
}

fn key_wrap_aad(vault_id: &str, key_version: u32) -> Vec<u8> {
    format!("nauterm-sync/v2/sync-dek/{vault_id}/{key_version}").into_bytes()
}

fn wrap_sync_dek(
    master_key: &str,
    vault_id: &str,
    key_version: u32,
    kdf: &SyncKdfHeader,
    sync_dek: &[u8; 32],
) -> Result<SyncKeyWrapHeader, SyncError> {
    let wrapping_key = Zeroizing::new(derive_wrapping_key(master_key, kdf)?);
    let cipher = Aes256Gcm::new_from_slice(wrapping_key.as_slice())
        .map_err(|_| SyncError::new("Unable to initialize Sync DEK wrapping."))?;
    let mut nonce = [0u8; AES_NONCE_LENGTH];
    fill_random(&mut nonce);
    let aad = key_wrap_aad(vault_id, key_version);
    let wrapped = cipher
        .encrypt(
            &aes_nonce(&nonce),
            Payload {
                msg: sync_dek,
                aad: &aad,
            },
        )
        .map_err(|_| SyncError::new("Unable to wrap the Sync DEK."))?;
    Ok(SyncKeyWrapHeader {
        algorithm: "aes-256-gcm".to_string(),
        nonce: STANDARD.encode(nonce),
        wrapped_sync_dek: STANDARD.encode(wrapped),
    })
}

fn unwrap_sync_dek(
    master_key: &str,
    header: &SyncEnvelopeHeader,
) -> Result<Zeroizing<[u8; 32]>, SyncError> {
    let wrapping_key = Zeroizing::new(derive_wrapping_key(master_key, &header.kdf)?);
    let nonce = STANDARD
        .decode(&header.key_wrap.nonce)
        .map_err(|_| SyncError::new("The sync file contains an invalid key-wrap nonce."))?;
    if nonce.len() != AES_NONCE_LENGTH {
        return Err(SyncError::new(
            "The sync file contains an invalid key-wrap nonce length.",
        ));
    }
    let wrapped = STANDARD
        .decode(&header.key_wrap.wrapped_sync_dek)
        .map_err(|_| SyncError::new("The wrapped Sync DEK is invalid."))?;
    let cipher = Aes256Gcm::new_from_slice(wrapping_key.as_slice())
        .map_err(|_| SyncError::new("Unable to initialize Sync DEK unwrapping."))?;
    let aad = key_wrap_aad(&header.vault_id, header.key_version);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                &aes_nonce(&nonce),
                Payload {
                    msg: &wrapped,
                    aad: &aad,
                },
            )
            .map_err(|_| {
                SyncError::new(
                    "Unable to unlock the Sync DEK. Check the Master Key or file integrity.",
                )
            })?,
    );
    if plaintext.len() != 32 {
        return Err(SyncError::new(
            "The unwrapped Sync DEK has an invalid length.",
        ));
    }
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&plaintext);
    Ok(key)
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), SyncError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("nauterm-sync");
    let mut random_suffix = [0u8; 6];
    fill_random(&mut random_suffix);
    let temporary_path = path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        URL_SAFE_NO_PAD.encode(random_suffix),
    ));

    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary_path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);

    match fs::rename(&temporary_path, path) {
        Ok(()) => Ok(()),
        Err(_error) if cfg!(windows) && path.exists() => {
            fs::remove_file(path)?;
            fs::rename(&temporary_path, path)?;
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary_path);
            Err(error.into())
        }
    }
}

fn json_from_sql_value(value: ValueRef<'_>) -> Result<Value, SyncError> {
    match value {
        ValueRef::Null => Ok(Value::Null),
        ValueRef::Integer(value) => Ok(Value::Number(Number::from(value))),
        ValueRef::Real(value) => Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| SyncError::new("Database contains a non-finite number.")),
        ValueRef::Text(value) => String::from_utf8(value.to_vec())
            .map(Value::String)
            .map_err(|_| SyncError::new("Database contains invalid UTF-8 text.")),
        ValueRef::Blob(value) => Ok(Value::String(STANDARD.encode(value))),
    }
}

fn sql_value_from_json(value: &Value) -> Result<SqlValue, SyncError> {
    match value {
        Value::Null => Ok(SqlValue::Null),
        Value::Bool(value) => Ok(SqlValue::Integer(i64::from(*value))),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(SqlValue::Integer(value))
            } else if let Some(value) = value.as_f64() {
                Ok(SqlValue::Real(value))
            } else {
                Err(SyncError::new("Sync payload contains an invalid number."))
            }
        }
        Value::String(value) => Ok(SqlValue::Text(value.clone())),
        Value::Array(_) | Value::Object(_) => Err(SyncError::new(
            "Sync payload contains a structured value in a database column.",
        )),
    }
}

fn record_string<'a>(record: &'a SyncRecord, field: &str) -> Result<&'a str, SyncError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| SyncError::new(format!("Sync record contains an invalid {field}.")))
}

fn record_i64(record: &SyncRecord, field: &str) -> Result<i64, SyncError> {
    record
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| SyncError::new(format!("Sync record contains an invalid {field}.")))
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

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encryption_round_trip_and_wrong_key_rejection() {
        let sync_key = [7u8; 32];
        let mut tables = BTreeMap::new();
        tables.insert("groups".to_string(), Vec::new());
        let payload = SyncPayload {
            format_version: PAYLOAD_FORMAT_VERSION,
            exported_at: 42,
            source_device_id: "01979f62-8548-7000-8000-000000000100".to_string(),
            tables,
        };
        let envelope =
            encrypt_payload(payload, Some("Correct master key 1!"), 31, None, &sync_key).unwrap();
        assert!(!envelope.header.snapshot_id.is_empty());
        let (_, decrypted, _) =
            decrypt_envelope(envelope.clone(), Some("Correct master key 1!"), None, 31).unwrap();
        assert_eq!(decrypted.exported_at, 42);
        assert!(decrypt_envelope(envelope, Some("Wrong master key 2!"), None, 31).is_err());
    }

    #[test]
    fn inspects_remote_status_without_decrypting_payload() {
        let sync_key = [7u8; 32];
        let envelope = encrypt_payload(
            SyncPayload {
                format_version: PAYLOAD_FORMAT_VERSION,
                exported_at: 42,
                source_device_id: "01979f62-8548-7000-8000-000000000100".to_string(),
                tables: BTreeMap::new(),
            },
            Some("Correct master key 1!"),
            31,
            None,
            &sync_key,
        )
        .unwrap();
        let expected_snapshot_id = envelope.header.snapshot_id.clone();
        let bytes = serde_json::to_vec(&envelope).unwrap();

        let status = inspect_envelope(&bytes, 31).unwrap();

        assert_eq!(status.revision, 1);
        assert_eq!(status.snapshot_id, expected_snapshot_id);
    }

    #[test]
    fn rejects_sync_files_from_a_different_database_schema() {
        let sync_key = [7u8; 32];
        let mut envelope = encrypt_payload(
            SyncPayload {
                format_version: PAYLOAD_FORMAT_VERSION,
                exported_at: 42,
                source_device_id: "01979f62-8548-7000-8000-000000000100".to_string(),
                tables: BTreeMap::new(),
            },
            Some("Correct master key 1!"),
            1,
            None,
            &sync_key,
        )
        .unwrap();

        envelope.header.schema_version = 40;
        let bytes = serde_json::to_vec(&envelope).unwrap();
        assert!(inspect_envelope(&bytes, 1).is_err());
    }

    #[test]
    fn rejects_a_revision_older_than_the_last_seen_vault() {
        let sync_key = [8u8; 32];
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE app_metadata (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL,
                    created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
                    updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
                );
                "#,
            )
            .unwrap();
        let payload = SyncPayload {
            format_version: PAYLOAD_FORMAT_VERSION,
            exported_at: 42,
            source_device_id: "01979f62-8548-7000-8000-000000000100".to_string(),
            tables: BTreeMap::new(),
        };
        let envelope =
            encrypt_payload(payload, Some("Correct master key 1!"), 31, None, &sync_key).unwrap();
        let mut header = envelope.header;
        header.revision = 7;
        remember_revision(&connection, &header, None).unwrap();
        assert!(reject_rollback(&connection, &header, None).is_ok());

        header.revision = 6;
        assert!(reject_rollback(&connection, &header, None)
            .unwrap_err()
            .to_string()
            .contains("moved backwards"));
    }

    #[test]
    fn encrypted_payload_does_not_expose_plaintext() {
        let sync_key = [9u8; 32];
        let mut tables = BTreeMap::new();
        let mut host = Map::new();
        host.insert(
            "secret".to_string(),
            Value::String("10.20.30.40 private value".to_string()),
        );
        tables.insert("hosts".to_string(), vec![host]);
        let envelope = encrypt_payload(
            SyncPayload {
                format_version: PAYLOAD_FORMAT_VERSION,
                exported_at: 42,
                source_device_id: "01979f62-8548-7000-8000-000000000100".to_string(),
                tables,
            },
            Some("Correct master key 1!"),
            31,
            None,
            &sync_key,
        )
        .unwrap();
        let serialized = serde_json::to_string(&envelope).unwrap();
        assert!(!serialized.contains("10.20.30.40"));
        assert!(!serialized.contains("private value"));
    }

    #[test]
    fn authenticated_header_rejects_tampering() {
        let sync_key = [10u8; 32];
        let envelope = encrypt_payload(
            SyncPayload {
                format_version: PAYLOAD_FORMAT_VERSION,
                exported_at: 42,
                source_device_id: "01979f62-8548-7000-8000-000000000100".to_string(),
                tables: BTreeMap::new(),
            },
            Some("Correct master key 1!"),
            31,
            None,
            &sync_key,
        )
        .unwrap();
        let mut tampered = envelope;
        tampered.header.snapshot_id = URL_SAFE_NO_PAD.encode([42u8; SNAPSHOT_ID_LENGTH]);

        assert!(decrypt_envelope(tampered, Some("Correct master key 1!"), None, 31).is_err());
    }
}
