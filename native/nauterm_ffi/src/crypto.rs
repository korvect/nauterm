// Local encryption and credential-storage primitives for Nauterm.
//
// Current storage design:
//   Database Key → 32 random bytes, stored in the OS credential store
//     macOS   → Keychain (Security framework)
//     Windows → Credential Manager
//     Linux   → Secret Service (libsecret)
//   SQLCipher    → encrypts the complete local SQLite database
//   Provider credentials → stored only inside that SQLCipher database
//   Master Key   → never persisted; the sync module uses it only to wrap/unwrap the Sync DEK
//   Sync DEK     → stored in the SQLCipher-protected database after the first successful unlock
//
// The remaining field-sealing helpers are isolated from the new database/sync key path and are
// retained temporarily while their old FFI surface is removed.

use std::fmt;
use std::sync::{Mutex, OnceLock};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

#[cfg(all(feature = "test-credential-store", not(debug_assertions)))]
compile_error!("test-credential-store must never be enabled in release builds");

#[cfg(not(feature = "test-credential-store"))]
pub const KEYRING_SERVICE: &str = "com.korvect.nauterm";
pub const KEYRING_DEVICE_KEY_ACCOUNT: &str = "device-key";
pub const KEYRING_DATABASE_KEY_ACCOUNT: &str = "local-database-key";

pub const ENCRYPTED_FIELD_PREFIX: &str = "enc:1:";

pub const AES_KEY_LENGTH: usize = 32;
pub const AES_NONCE_LENGTH: usize = 12;
pub const AES_TAG_LENGTH: usize = 16;
pub const ARGON2_SALT_LENGTH: usize = 16;
pub const ARGON2_OUTPUT_LENGTH: usize = 32;

// OWASP baseline is 64 MiB / 3 iter / 1 lane. We follow the review recommendation and use
// 128 MiB memory for a desktop app which comfortably exceeds the minimum guidance.
pub const ARGON2_MEMORY_KIB: u32 = 131_072;
pub const ARGON2_ITERATIONS: u32 = 3;
pub const ARGON2_PARALLELISM: u32 = 1;

pub const MASTER_KEY_MIN_LENGTH: usize = 12;
pub const MASTER_KEY_MIN_CHARACTER_CLASSES: usize = 3;

fn aes_nonce(bytes: &[u8]) -> Nonce<aes_gcm::aead::consts::U12> {
    Nonce::try_from(bytes).expect("AES-GCM nonce must be 12 bytes")
}

fn fill_random(bytes: &mut [u8]) {
    getrandom::fill(bytes).expect("operating system random source is unavailable");
}

static DATABASE_KEY_CACHE: OnceLock<Mutex<Option<Zeroizing<[u8; AES_KEY_LENGTH]>>>> =
    OnceLock::new();
static DEVICE_KEY_CACHE: OnceLock<Mutex<Option<Zeroizing<[u8; AES_KEY_LENGTH]>>>> = OnceLock::new();

// app_metadata keys
pub const META_ENCRYPTION_CONFIG: &str = "encryption_config";
pub const META_DEK_DEVICE_SEALED: &str = "dek_device_sealed";
pub const META_DEK_MASTER_SEALED: &str = "dek_master_sealed";
pub const META_FIELD_ENCRYPTION_VERSION: &str = "field_encryption_version";

#[derive(Debug)]
pub struct CryptoError {
    message: String,
}

impl CryptoError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CryptoError {}

impl From<rusqlite::Error> for CryptoError {
    fn from(err: rusqlite::Error) -> Self {
        Self::new(format!("database error: {err}"))
    }
}

impl From<serde_json::Error> for CryptoError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(format!("json error: {err}"))
    }
}

impl From<base64::DecodeError> for CryptoError {
    fn from(err: base64::DecodeError) -> Self {
        Self::new(format!("base64 error: {err}"))
    }
}

impl From<keyring::Error> for CryptoError {
    fn from(err: keyring::Error) -> Self {
        Self::new(format!("keychain error: {err}"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KdfParams {
    pub algorithm: String,
    pub salt: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptionConfig {
    pub version: u32,
    pub has_master_key: bool,
    pub kdf: KdfParams,
    pub cipher: String,
}

impl EncryptionConfig {
    pub fn new_default() -> Self {
        let mut salt = [0u8; ARGON2_SALT_LENGTH];
        fill_random(&mut salt);
        Self {
            version: 1,
            has_master_key: false,
            kdf: KdfParams {
                algorithm: "argon2id".into(),
                salt: STANDARD.encode(salt),
                memory_kib: ARGON2_MEMORY_KIB,
                iterations: ARGON2_ITERATIONS,
                parallelism: ARGON2_PARALLELISM,
            },
            cipher: "aes-256-gcm".into(),
        }
    }

    pub fn salt_bytes(&self) -> Result<Vec<u8>, CryptoError> {
        STANDARD
            .decode(&self.kdf.salt)
            .map_err(|_| CryptoError::new("Invalid KDF salt in encryption_config."))
    }
}

// ---------------------------------------------------------------------------
// Low level primitives
// ---------------------------------------------------------------------------

fn normalize_passphrase(passphrase: &str) -> Zeroizing<Vec<u8>> {
    // NFC-normalise, then UTF-8 encode. Zeroize afterwards.
    let normalized: String = passphrase.nfc().collect();
    let bytes = normalized.into_bytes();
    Zeroizing::new(bytes)
}

pub fn validate_master_key(master_key: &str) -> Result<(), CryptoError> {
    if master_key.chars().count() < MASTER_KEY_MIN_LENGTH {
        return Err(CryptoError::new(format!(
            "Master key must be at least {MASTER_KEY_MIN_LENGTH} characters."
        )));
    }

    if !master_key.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Err(CryptoError::new(
            "Master key may contain only printable ASCII characters (space through ~).",
        ));
    }

    let has_uppercase = master_key.bytes().any(|byte| byte.is_ascii_uppercase());
    let has_lowercase = master_key.bytes().any(|byte| byte.is_ascii_lowercase());
    let has_digit = master_key.bytes().any(|byte| byte.is_ascii_digit());
    let has_symbol = master_key.bytes().any(|byte| !byte.is_ascii_alphanumeric());
    let character_classes = [has_uppercase, has_lowercase, has_digit, has_symbol]
        .into_iter()
        .filter(|present| *present)
        .count();
    if character_classes < MASTER_KEY_MIN_CHARACTER_CLASSES {
        return Err(CryptoError::new(format!(
            "Master key must include at least {MASTER_KEY_MIN_CHARACTER_CLASSES} of these character types: uppercase letters, lowercase letters, numbers, and symbols."
        )));
    }

    Ok(())
}

pub fn derive_wrapping_key(
    master_key: &str,
    salt: &[u8],
    params: &KdfParams,
) -> Result<Zeroizing<[u8; AES_KEY_LENGTH]>, CryptoError> {
    if params.algorithm != "argon2id" {
        return Err(CryptoError::new("Unsupported KDF algorithm."));
    }
    if salt.len() != ARGON2_SALT_LENGTH
        || !(8..=262_144).contains(&params.memory_kib)
        || !(1..=10).contains(&params.iterations)
        || !(1..=8).contains(&params.parallelism)
    {
        return Err(CryptoError::new("Unsafe or invalid Argon2id parameters."));
    }
    let argon_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(ARGON2_OUTPUT_LENGTH),
    )
    .map_err(|_| CryptoError::new("Invalid Argon2id parameters."))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let passphrase = normalize_passphrase(master_key);
    let mut out = Zeroizing::new([0u8; AES_KEY_LENGTH]);
    argon
        .hash_password_into(passphrase.as_slice(), salt, out.as_mut_slice())
        .map_err(|_| CryptoError::new("Master key derivation failed."))?;
    Ok(out)
}

pub fn aes_seal(key: &[u8; AES_KEY_LENGTH], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| CryptoError::new("Unable to initialize AES-256-GCM."))?;
    let mut nonce = [0u8; AES_NONCE_LENGTH];
    fill_random(&mut nonce);
    let ct = cipher
        .encrypt(&aes_nonce(&nonce), plaintext)
        .map_err(|_| CryptoError::new("AES-GCM encryption failed."))?;
    let mut out = Vec::with_capacity(AES_NONCE_LENGTH + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn aes_open(key: &[u8; AES_KEY_LENGTH], sealed: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if sealed.len() < AES_NONCE_LENGTH + AES_TAG_LENGTH {
        return Err(CryptoError::new("Sealed payload is too short."));
    }
    let (nonce, ct) = sealed.split_at(AES_NONCE_LENGTH);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| CryptoError::new("Unable to initialize AES-256-GCM."))?;
    cipher
        .decrypt(&aes_nonce(nonce), ct)
        .map_err(|_| CryptoError::new("AES-GCM decryption failed."))
}

// ---------------------------------------------------------------------------
// Device Key (system keychain)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "test-credential-store"))]
fn keyring_entry(account: &str) -> Result<keyring::Entry, CryptoError> {
    keyring::Entry::new(KEYRING_SERVICE, account).map_err(CryptoError::from)
}

#[cfg(not(feature = "test-credential-store"))]
fn load_or_create_keyring_key(
    account: &str,
    description: &str,
) -> Result<Zeroizing<[u8; AES_KEY_LENGTH]>, CryptoError> {
    let entry = keyring_entry(account)?;
    match entry.get_password() {
        Ok(encoded) => {
            let decoded = Zeroizing::new(
                STANDARD
                    .decode(encoded.trim())
                    .map_err(|_| CryptoError::new(format!("Stored {description} is corrupt.")))?,
            );
            if decoded.len() != AES_KEY_LENGTH {
                return Err(CryptoError::new(format!(
                    "Stored {description} has invalid length."
                )));
            }
            let mut key = Zeroizing::new([0u8; AES_KEY_LENGTH]);
            key.copy_from_slice(&decoded);
            Ok(key)
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = Zeroizing::new([0u8; AES_KEY_LENGTH]);
            fill_random(key.as_mut_slice());
            let encoded = Zeroizing::new(STANDARD.encode(key.as_slice()));
            entry
                .set_password(encoded.as_str())
                .map_err(CryptoError::from)?;
            Ok(key)
        }
        Err(err) => Err(CryptoError::from(err)),
    }
}

#[cfg(feature = "test-credential-store")]
fn load_or_create_keyring_key(
    account: &str,
    _description: &str,
) -> Result<Zeroizing<[u8; AES_KEY_LENGTH]>, CryptoError> {
    let byte = match account {
        KEYRING_DEVICE_KEY_ACCOUNT => 0x44,
        KEYRING_DATABASE_KEY_ACCOUNT => 0x4e,
        _ => return Err(CryptoError::new("Unknown test credential account.")),
    };
    Ok(Zeroizing::new([byte; AES_KEY_LENGTH]))
}

pub fn load_or_create_device_key() -> Result<Zeroizing<[u8; AES_KEY_LENGTH]>, CryptoError> {
    load_or_create_cached_key(&DEVICE_KEY_CACHE, KEYRING_DEVICE_KEY_ACCOUNT, "device key")
}

/// Returns the per-device SQLCipher key. This key never leaves the operating system's
/// credential store and is independent from the cross-device Sync DEK.
pub fn load_or_create_database_key() -> Result<Zeroizing<[u8; AES_KEY_LENGTH]>, CryptoError> {
    load_or_create_cached_key(
        &DATABASE_KEY_CACHE,
        KEYRING_DATABASE_KEY_ACCOUNT,
        "database key",
    )
}

fn load_or_create_cached_key(
    cache: &'static OnceLock<Mutex<Option<Zeroizing<[u8; AES_KEY_LENGTH]>>>>,
    account: &str,
    description: &str,
) -> Result<Zeroizing<[u8; AES_KEY_LENGTH]>, CryptoError> {
    let cache = cache.get_or_init(|| Mutex::new(None));
    let mut cached = cache
        .lock()
        .map_err(|_| CryptoError::new(format!("{description} cache is unavailable.")))?;
    if let Some(key) = cached.as_ref() {
        let mut copy = Zeroizing::new([0u8; AES_KEY_LENGTH]);
        copy.copy_from_slice(key.as_slice());
        return Ok(copy);
    }
    let key = load_or_create_keyring_key(account, description)?;
    let mut copy = Zeroizing::new([0u8; AES_KEY_LENGTH]);
    copy.copy_from_slice(key.as_slice());
    *cached = Some(copy);
    Ok(key)
}

// ---------------------------------------------------------------------------
// app_metadata helpers
// ---------------------------------------------------------------------------

fn metadata_get(connection: &Connection, key: &str) -> Result<Option<String>, CryptoError> {
    let value = connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = ?",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(value)
}

fn metadata_set(connection: &Connection, key: &str, value: &str) -> Result<(), CryptoError> {
    connection.execute(
        r#"INSERT INTO app_metadata (key, value) VALUES (?, ?)
           ON CONFLICT(key) DO UPDATE SET
             value = excluded.value,
             updated_at = CAST(unixepoch('subsec') * 1000 AS INTEGER)"#,
        params![key, value],
    )?;
    Ok(())
}

fn metadata_delete(connection: &Connection, key: &str) -> Result<(), CryptoError> {
    connection.execute("DELETE FROM app_metadata WHERE key = ?", params![key])?;
    Ok(())
}

pub fn load_encryption_config(
    connection: &Connection,
) -> Result<Option<EncryptionConfig>, CryptoError> {
    let Some(text) = metadata_get(connection, META_ENCRYPTION_CONFIG)? else {
        return Ok(None);
    };
    let cfg: EncryptionConfig = serde_json::from_str(&text)?;
    Ok(Some(cfg))
}

pub fn save_encryption_config(
    connection: &Connection,
    cfg: &EncryptionConfig,
) -> Result<(), CryptoError> {
    let text = serde_json::to_string(cfg)?;
    metadata_set(connection, META_ENCRYPTION_CONFIG, &text)
}

// ---------------------------------------------------------------------------
// DEK
// ---------------------------------------------------------------------------

pub type Dek = Zeroizing<[u8; AES_KEY_LENGTH]>;

fn new_dek() -> Dek {
    let mut dek = Zeroizing::new([0u8; AES_KEY_LENGTH]);
    fill_random(dek.as_mut_slice());
    dek
}

fn seal_dek_with_key(
    dek: &Dek,
    wrapping_key: &[u8; AES_KEY_LENGTH],
) -> Result<String, CryptoError> {
    let sealed = aes_seal(wrapping_key, dek.as_slice())?;
    Ok(STANDARD.encode(sealed))
}

fn open_dek_with_key(
    encoded: &str,
    wrapping_key: &[u8; AES_KEY_LENGTH],
) -> Result<Dek, CryptoError> {
    let raw = STANDARD
        .decode(encoded)
        .map_err(|_| CryptoError::new("Sealed DEK payload is invalid base64."))?;
    let plaintext = aes_open(wrapping_key, &raw)?;
    if plaintext.len() != AES_KEY_LENGTH {
        return Err(CryptoError::new("Unsealed DEK has invalid length."));
    }
    let mut dek = Zeroizing::new([0u8; AES_KEY_LENGTH]);
    dek.copy_from_slice(&plaintext);
    Ok(dek)
}

/// Initialise encryption on first run: generate DEK, seal it with the Device Key, store both
/// in the database and keychain. Returns the plaintext DEK for immediate in-memory use.
pub fn initialise_encryption(connection: &Connection) -> Result<Dek, CryptoError> {
    if load_encryption_config(connection)?.is_some() {
        // The device binding is intentionally retained when a Master Key is configured, so
        // normal local use remains transparent. A password is only required on a new device or
        // after the user explicitly removes this binding.
        return unlock_with_device_key(connection);
    }
    let cfg = EncryptionConfig::new_default();
    let dek = new_dek();
    let device_key = load_or_create_device_key()?;
    let sealed_device = seal_dek_with_key(&dek, &device_key)?;
    let transaction = connection.unchecked_transaction()?;
    save_encryption_config(&transaction, &cfg)?;
    metadata_set(&transaction, META_DEK_DEVICE_SEALED, &sealed_device)?;
    metadata_delete(&transaction, META_DEK_MASTER_SEALED)?;
    transaction.commit()?;
    Ok(dek)
}

/// Unlock the local DEK only when field encryption has already been enabled.
///
/// Database tests and import tools may intentionally operate on an uninitialised database; in
/// that case fields remain unprotected until `initialise_encryption` runs.
pub fn local_dek_if_configured(connection: &Connection) -> Result<Option<Dek>, CryptoError> {
    if load_encryption_config(connection)?.is_none() {
        return Ok(None);
    }
    unlock_with_device_key(connection).map(Some)
}

/// Encrypt unprotected values in place without changing their logical CRDT timestamps.
///
/// This operation is idempotent because `encrypt_field` preserves values carrying `enc:1:`.
pub fn encrypt_unprotected_sensitive_fields(
    connection: &Connection,
    dek: &Dek,
) -> Result<usize, CryptoError> {
    if metadata_get(connection, META_FIELD_ENCRYPTION_VERSION)?.as_deref() == Some("1") {
        return Ok(0);
    }

    const FIELDS: &[(&str, &str)] = &[
        ("keys", "private_key"),
        ("keys", "certificate"),
        ("keys", "passphrase"),
        ("identities", "password"),
        ("hosts", "password"),
        ("hosts", "telnet_password"),
        ("proxies", "password"),
    ];
    let transaction = connection.unchecked_transaction()?;
    let mut changed = 0;
    for (table, column) in FIELDS {
        let sql = format!(
            "SELECT id, {column} FROM {table} WHERE {column} IS NOT NULL AND {column} != ''"
        );
        let rows = {
            let mut statement = transaction.prepare(&sql)?;
            let values = statement
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            values
        };
        let update = format!("UPDATE {table} SET {column} = ? WHERE id = ?");
        for (id, value) in rows {
            if is_encrypted_field(&value) {
                continue;
            }
            let encrypted = encrypt_field(dek, &value)?;
            transaction.execute(&update, params![encrypted, id])?;
            changed += 1;
        }
    }
    metadata_set(&transaction, META_FIELD_ENCRYPTION_VERSION, "1")?;
    transaction.commit()?;
    Ok(changed)
}

pub fn unlock_with_device_key(connection: &Connection) -> Result<Dek, CryptoError> {
    let sealed = metadata_get(connection, META_DEK_DEVICE_SEALED)?
        .ok_or_else(|| CryptoError::new("This vault is only unlockable with a Master Key."))?;
    let device_key = load_or_create_device_key()?;
    open_dek_with_key(&sealed, &device_key)
}

pub fn unlock_with_master_key(
    connection: &Connection,
    master_key: &str,
) -> Result<Dek, CryptoError> {
    let cfg = load_encryption_config(connection)?
        .ok_or_else(|| CryptoError::new("Encryption has not been initialised yet."))?;
    if !cfg.has_master_key {
        return Err(CryptoError::new("This vault has no Master Key set."));
    }
    let sealed = metadata_get(connection, META_DEK_MASTER_SEALED)?
        .ok_or_else(|| CryptoError::new("Sealed DEK for Master Key is missing."))?;
    let salt = cfg.salt_bytes()?;
    let wrapping = derive_wrapping_key(master_key, &salt, &cfg.kdf)?;
    open_dek_with_key(&sealed, &wrapping)
}

pub fn set_master_key(connection: &Connection, master_key: &str) -> Result<(), CryptoError> {
    validate_master_key(master_key)?;
    let mut cfg = load_encryption_config(connection)?
        .ok_or_else(|| CryptoError::new("Encryption has not been initialised yet."))?;
    if cfg.has_master_key {
        return Err(CryptoError::new(
            "A Master Key already exists. Use change_master_key instead.",
        ));
    }
    let dek = unlock_with_device_key(connection)?;
    // Rotate salt to make sure it is fresh.
    let mut salt = [0u8; ARGON2_SALT_LENGTH];
    fill_random(&mut salt);
    cfg.kdf.salt = STANDARD.encode(salt);
    let wrapping = derive_wrapping_key(master_key, &salt, &cfg.kdf)?;
    let sealed = seal_dek_with_key(&dek, &wrapping)?;
    let transaction = connection.unchecked_transaction()?;
    metadata_set(&transaction, META_DEK_MASTER_SEALED, &sealed)?;
    // Decision (#10 in review): keep `dek_device_sealed` so the local machine can still open
    // the vault transparently. UI can offer an explicit "require Master Key on this device"
    // toggle that would call `remove_device_binding` below.
    cfg.has_master_key = true;
    save_encryption_config(&transaction, &cfg)?;
    transaction.commit()?;
    Ok(())
}

pub fn change_master_key(
    connection: &Connection,
    current_master_key: &str,
    new_master_key: &str,
) -> Result<(), CryptoError> {
    validate_master_key(new_master_key)?;
    let dek = unlock_with_master_key(connection, current_master_key)?;
    let mut cfg = load_encryption_config(connection)?
        .ok_or_else(|| CryptoError::new("Encryption has not been initialised yet."))?;
    let mut salt = [0u8; ARGON2_SALT_LENGTH];
    fill_random(&mut salt);
    cfg.kdf.salt = STANDARD.encode(salt);
    let wrapping = derive_wrapping_key(new_master_key, &salt, &cfg.kdf)?;
    let sealed = seal_dek_with_key(&dek, &wrapping)?;
    let transaction = connection.unchecked_transaction()?;
    metadata_set(&transaction, META_DEK_MASTER_SEALED, &sealed)?;
    save_encryption_config(&transaction, &cfg)?;
    transaction.commit()?;
    Ok(())
}

pub fn remove_master_key(
    connection: &Connection,
    current_master_key: &str,
) -> Result<(), CryptoError> {
    // Requires that the device binding still exists (otherwise the user would be locked out).
    let _dek = unlock_with_master_key(connection, current_master_key)?;
    if metadata_get(connection, META_DEK_DEVICE_SEALED)?.is_none() {
        return Err(CryptoError::new(
            "Cannot remove Master Key: no device binding is present.",
        ));
    }
    let mut cfg = load_encryption_config(connection)?
        .ok_or_else(|| CryptoError::new("Encryption has not been initialised yet."))?;
    cfg.has_master_key = false;
    let transaction = connection.unchecked_transaction()?;
    save_encryption_config(&transaction, &cfg)?;
    metadata_delete(&transaction, META_DEK_MASTER_SEALED)?;
    transaction.commit()?;
    Ok(())
}

pub fn verify_master_key(connection: &Connection, master_key: &str) -> Result<bool, CryptoError> {
    match unlock_with_master_key(connection, master_key) {
        Ok(_) => Ok(true),
        Err(err) if err.message().contains("decryption failed") => Ok(false),
        Err(err) if err.message().contains("derivation failed") => Ok(false),
        Err(err) => Err(err),
    }
}

// ---------------------------------------------------------------------------
// Field level encryption
// ---------------------------------------------------------------------------

pub fn is_encrypted_field(value: &str) -> bool {
    value.starts_with(ENCRYPTED_FIELD_PREFIX)
}

pub fn encrypt_field(dek: &Dek, plaintext: &str) -> Result<String, CryptoError> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }
    if is_encrypted_field(plaintext) {
        return Ok(plaintext.to_string());
    }
    let sealed = aes_seal(dek, plaintext.as_bytes())?;
    Ok(format!(
        "{ENCRYPTED_FIELD_PREFIX}{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sealed)
    ))
}

pub fn decrypt_field(dek: &Dek, value: &str) -> Result<String, CryptoError> {
    if !is_encrypted_field(value) {
        return Ok(value.to_string());
    }
    let encoded = &value[ENCRYPTED_FIELD_PREFIX.len()..];
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| CryptoError::new("Encrypted field is not valid base64url."))?;
    let plaintext = aes_open(dek, &raw)?;
    String::from_utf8(plaintext)
        .map_err(|_| CryptoError::new("Encrypted field decoded to invalid UTF-8."))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_key_requires_printable_ascii_and_three_character_classes() {
        assert!(validate_master_key("Abcdefghij12").is_ok());
        assert!(validate_master_key("abcdefghij1 ").is_ok());
        assert!(validate_master_key("ABCDEFGHI1 !").is_ok());
        assert!(validate_master_key("Abcdefghij1 ").is_ok());

        assert!(validate_master_key("Abcdefghi1!").is_err());
        assert!(validate_master_key("abcdefghij12").is_err());
        assert!(validate_master_key("Abcdefghij1\n").is_err());
        assert!(validate_master_key("Abcdefghij1é").is_err());
    }

    fn temp_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                r#"CREATE TABLE app_metadata (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL,
                    created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
                    updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
                );"#,
            )
            .unwrap();
        connection
    }

    fn deterministic_dek() -> Dek {
        let mut dek = Zeroizing::new([0u8; AES_KEY_LENGTH]);
        for (idx, byte) in dek.as_mut_slice().iter_mut().enumerate() {
            *byte = (idx as u8).wrapping_mul(7).wrapping_add(3);
        }
        dek
    }

    #[test]
    fn aes_roundtrip() {
        let key = [42u8; AES_KEY_LENGTH];
        let sealed = aes_seal(&key, b"hello, world").unwrap();
        let opened = aes_open(&key, &sealed).unwrap();
        assert_eq!(opened, b"hello, world");
    }

    #[cfg(feature = "test-credential-store")]
    #[test]
    fn test_credential_store_returns_stable_distinct_keys() {
        let database_key = load_or_create_database_key().unwrap();
        let repeated_database_key = load_or_create_database_key().unwrap();
        let device_key = load_or_create_device_key().unwrap();

        assert_eq!(database_key.as_slice(), repeated_database_key.as_slice());
        assert_ne!(database_key.as_slice(), device_key.as_slice());
        assert_eq!(database_key.as_slice(), &[0x4e; AES_KEY_LENGTH]);
        assert_eq!(device_key.as_slice(), &[0x44; AES_KEY_LENGTH]);
    }

    #[test]
    fn field_encryption_roundtrip() {
        let dek = deterministic_dek();
        let ct = encrypt_field(&dek, "hunter2").unwrap();
        assert!(ct.starts_with(ENCRYPTED_FIELD_PREFIX));
        assert_eq!(decrypt_field(&dek, &ct).unwrap(), "hunter2");
        // Plain values pass through decrypt unchanged.
        assert_eq!(decrypt_field(&dek, "plain").unwrap(), "plain");
        // Re-encrypting an already-encrypted value keeps it stable.
        assert_eq!(encrypt_field(&dek, &ct).unwrap(), ct);
    }

    #[test]
    fn empty_field_stays_empty() {
        let dek = deterministic_dek();
        assert_eq!(encrypt_field(&dek, "").unwrap(), "");
    }

    #[test]
    fn wrong_key_fails() {
        let dek = deterministic_dek();
        let ct = encrypt_field(&dek, "top secret").unwrap();
        let mut other = Zeroizing::new([0u8; AES_KEY_LENGTH]);
        other[0] = 1;
        assert!(decrypt_field(&other, &ct).is_err());
    }

    #[test]
    fn argon2id_key_is_deterministic() {
        let params = KdfParams {
            algorithm: "argon2id".into(),
            salt: STANDARD.encode([1u8; ARGON2_SALT_LENGTH]),
            memory_kib: 8, // tiny for tests
            iterations: 1,
            parallelism: 1,
        };
        let salt = params.salt_bytes_for_test();
        let a = derive_wrapping_key("correct horse battery staple", &salt, &params).unwrap();
        let b = derive_wrapping_key("correct horse battery staple", &salt, &params).unwrap();
        assert_eq!(a.as_slice(), b.as_slice());
        let c = derive_wrapping_key("different", &salt, &params).unwrap();
        assert_ne!(a.as_slice(), c.as_slice());
    }

    #[test]
    fn master_key_lifecycle_in_memory() {
        // We only exercise the metadata-only flow without touching the OS keychain, so we
        // stub the device key path by directly writing a sealed DEK.
        let connection = temp_connection();
        let dek = deterministic_dek();
        let device_key = [7u8; AES_KEY_LENGTH];
        let sealed = seal_dek_with_key(&dek, &device_key).unwrap();
        let mut cfg = EncryptionConfig::new_default();
        // Use tiny KDF params for tests.
        cfg.kdf.memory_kib = 8;
        cfg.kdf.iterations = 1;
        cfg.kdf.parallelism = 1;
        save_encryption_config(&connection, &cfg).unwrap();
        metadata_set(&connection, META_DEK_DEVICE_SEALED, &sealed).unwrap();

        // Set a master key using the injected device key.
        let salt = cfg.salt_bytes().unwrap();
        let wrapping =
            derive_wrapping_key("correct horse battery staple", &salt, &cfg.kdf).unwrap();
        let master_sealed = seal_dek_with_key(&dek, &wrapping).unwrap();
        metadata_set(&connection, META_DEK_MASTER_SEALED, &master_sealed).unwrap();
        cfg.has_master_key = true;
        save_encryption_config(&connection, &cfg).unwrap();

        let unlocked = unlock_with_master_key(&connection, "correct horse battery staple").unwrap();
        assert_eq!(unlocked.as_slice(), dek.as_slice());
    }

    #[test]
    fn unprotected_sensitive_fields_are_encrypted_once() {
        let connection = temp_connection();
        connection
            .execute_batch(
                r#"
                CREATE TABLE keys (
                    id INTEGER PRIMARY KEY,
                    private_key TEXT,
                    certificate TEXT,
                    passphrase TEXT
                );
                CREATE TABLE identities (id INTEGER PRIMARY KEY, password TEXT);
                CREATE TABLE hosts (
                    id INTEGER PRIMARY KEY,
                    password TEXT,
                    telnet_password TEXT
                );
                CREATE TABLE proxies (id INTEGER PRIMARY KEY, password TEXT);
                INSERT INTO keys (private_key, certificate, passphrase)
                VALUES ('private material', 'certificate material', 'saved passphrase');
                INSERT INTO identities (password) VALUES ('identity secret');
                INSERT INTO hosts (password, telnet_password)
                VALUES ('ssh secret', 'telnet secret');
                INSERT INTO proxies (password) VALUES ('proxy secret');
                "#,
            )
            .unwrap();
        let dek = deterministic_dek();

        assert_eq!(
            encrypt_unprotected_sensitive_fields(&connection, &dek).unwrap(),
            7
        );
        assert_eq!(
            encrypt_unprotected_sensitive_fields(&connection, &dek).unwrap(),
            0
        );

        for (table, column, expected) in [
            ("keys", "private_key", "private material"),
            ("keys", "certificate", "certificate material"),
            ("keys", "passphrase", "saved passphrase"),
            ("identities", "password", "identity secret"),
            ("hosts", "password", "ssh secret"),
            ("hosts", "telnet_password", "telnet secret"),
            ("proxies", "password", "proxy secret"),
        ] {
            let value: String = connection
                .query_row(&format!("SELECT {column} FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert!(is_encrypted_field(&value));
            assert_eq!(decrypt_field(&dek, &value).unwrap(), expected);
        }
    }

    // Small helper for the tests above.
    impl KdfParams {
        fn salt_bytes_for_test(&self) -> Vec<u8> {
            STANDARD.decode(&self.salt).unwrap()
        }
    }
}
