use std::fmt::{self, Write as _};

use opendal::{ErrorKind, Operator};
use serde::Serialize;
use sha2::{Digest, Sha256};

const MAX_OBJECT_BYTES: u64 = 64 * 1024 * 1024;
const CONTENT_TOKEN_PREFIX: &str = "nauterm-sha256:";

pub fn join_object_path(prefix: &str, filename: &str) -> Result<String, ObjectSyncError> {
    let filename = filename.trim();
    if filename.is_empty()
        || filename == "."
        || filename == ".."
        || filename.contains('/')
        || filename.contains('\\')
        || filename.chars().any(char::is_control)
    {
        return Err(ObjectSyncError::new(
            "Sync filename must be a filename, not a path.",
        ));
    }

    let prefix = prefix.trim().trim_matches('/');
    if prefix.contains('\\')
        || prefix.chars().any(char::is_control)
        || (!prefix.is_empty()
            && prefix
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | "..")))
    {
        return Err(ObjectSyncError::new("Sync prefix is invalid."));
    }

    Ok(if prefix.is_empty() {
        filename.to_string()
    } else {
        format!("{prefix}/{filename}")
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteSyncObject {
    pub bytes: Vec<u8>,
    pub etag: String,
    pub version_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ObjectWriteResult {
    pub etag: String,
    pub version_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ObjectVersionSummary {
    pub version_id: String,
    pub etag: String,
    pub content_length: u64,
    pub last_modified: Option<String>,
    pub is_current: bool,
}

#[derive(Debug)]
pub struct ObjectSyncError {
    message: String,
    kind: Option<ErrorKind>,
}

impl ObjectSyncError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: None,
        }
    }

    pub fn from_opendal(error: opendal::Error) -> Self {
        let kind = error.kind();
        let mut message = error.to_string();
        let mut source = std::error::Error::source(&error);
        while let Some(cause) = source {
            let cause_text = cause.to_string();
            if !cause_text.is_empty() && !message.contains(&cause_text) {
                message.push_str("\nCaused by: ");
                message.push_str(&cause_text);
            }
            source = cause.source();
        }
        Self {
            message: message.chars().take(4096).collect(),
            kind: Some(kind),
        }
    }
}

impl fmt::Display for ObjectSyncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ObjectSyncError {}

/// Shared OpenDAL object transport used by encrypted sync providers. Provider
/// modules only build an Operator; all object safety semantics stay here.
pub struct OpenDalSyncTransport {
    runtime: tokio::runtime::Runtime,
    operator: Operator,
    object_path: String,
}

impl OpenDalSyncTransport {
    pub fn new(operator: Operator, object_path: String) -> Result<Self, ObjectSyncError> {
        let capability = operator.info().capability();
        if !capability.read || !capability.stat || !capability.write {
            return Err(ObjectSyncError::new(
                "This storage backend does not support the object operations required for sync.",
            ));
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                ObjectSyncError::new(format!(
                    "Unable to initialize the object storage runtime: {error}"
                ))
            })?;
        Ok(Self {
            runtime,
            operator,
            object_path,
        })
    }

    pub fn fetch_current(&self) -> Result<Option<RemoteSyncObject>, ObjectSyncError> {
        self.runtime.block_on(async {
            let metadata = match self.operator.stat(&self.object_path).await {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(ObjectSyncError::from_opendal(error)),
            };
            if metadata.content_length() > MAX_OBJECT_BYTES {
                return Err(object_too_large());
            }
            let provider_etag = metadata.etag().filter(|value| !value.is_empty());
            let version_id = metadata.version().map(str::to_string);
            let request = self.operator.read_with(&self.object_path);
            let bytes = match provider_etag {
                Some(etag) if self.operator.info().capability().read_with_if_match => {
                    request.if_match(etag).await
                }
                _ => request.await,
            }
            .map_err(ObjectSyncError::from_opendal)?
            .to_vec();
            if bytes.len() as u64 > MAX_OBJECT_BYTES {
                return Err(object_too_large());
            }
            let etag = provider_etag
                .map(str::to_string)
                .unwrap_or_else(|| content_token(&bytes));
            Ok(Some(RemoteSyncObject {
                bytes,
                etag,
                version_id,
            }))
        })
    }

    pub fn write(
        &self,
        bytes: &[u8],
        expected_etag: Option<&str>,
    ) -> Result<ObjectWriteResult, ObjectSyncError> {
        if bytes.len() as u64 > MAX_OBJECT_BYTES {
            return Err(object_too_large());
        }
        self.runtime.block_on(async {
            let capability = self.operator.info().capability();
            let request = self.operator.write_with(&self.object_path, bytes.to_vec());
            let metadata = match (
                expected_etag,
                capability.write_with_if_match,
                capability.write_with_if_not_exists,
            ) {
                (Some(etag), true, _) if !is_content_token(etag) => request.if_match(etag).await,
                (None, _, true) => request.if_not_exists(true).await,
                (expected, _, _) => {
                    let current = self.operator.stat(&self.object_path).await;
                    match (expected, current) {
                        (Some(expected), Ok(metadata)) if metadata.etag() == Some(expected) => {
                            request.await
                        }
                        (Some(expected), Ok(metadata)) if is_content_token(expected) => {
                            if metadata.content_length() > MAX_OBJECT_BYTES {
                                return Err(object_too_large());
                            }
                            let current_bytes = self
                                .operator
                                .read(&self.object_path)
                                .await
                                .map_err(ObjectSyncError::from_opendal)?
                                .to_vec();
                            if current_bytes.len() as u64 > MAX_OBJECT_BYTES {
                                return Err(object_too_large());
                            }
                            if content_token(&current_bytes) == expected {
                                request.await
                            } else {
                                return Err(remote_changed());
                            }
                        }
                        (None, Err(error)) if error.kind() == ErrorKind::NotFound => request.await,
                        (_, Err(error)) if error.kind() != ErrorKind::NotFound => Err(error),
                        _ => return Err(remote_changed()),
                    }
                }
            }
            .map_err(ObjectSyncError::from_opendal)?;
            let etag = metadata
                .etag()
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| content_token(bytes));
            Ok(ObjectWriteResult {
                etag,
                version_id: metadata.version().map(str::to_string),
            })
        })
    }

    pub fn restore_version(&self, version_id: &str) -> Result<ObjectWriteResult, ObjectSyncError> {
        if version_id.trim().is_empty() {
            return Err(ObjectSyncError::new("Choose a storage version to restore."));
        }
        let capability = self.operator.info().capability();
        if !capability.read_with_version {
            return Err(ObjectSyncError::new(
                "This storage backend does not support restoring object versions.",
            ));
        }
        let bytes = self.runtime.block_on(async {
            self.operator
                .read_with(&self.object_path)
                .version(version_id)
                .await
                .map_err(ObjectSyncError::from_opendal)
                .map(|value| value.to_vec())
        })?;
        if bytes.len() as u64 > MAX_OBJECT_BYTES {
            return Err(object_too_large());
        }
        let current = self.fetch_current()?;
        self.write(&bytes, current.as_ref().map(|value| value.etag.as_str()))
    }

    pub fn list_versions(
        &self,
        limit: usize,
    ) -> Result<Vec<ObjectVersionSummary>, ObjectSyncError> {
        let capability = self.operator.info().capability();
        if !capability.list || !capability.list_with_versions {
            return Err(ObjectSyncError::new(
                "This storage backend does not support object version history.",
            ));
        }
        let limit = limit.clamp(1, 100);
        self.runtime.block_on(async {
            let mut versions = self
                .operator
                .list_with(&self.object_path)
                .versions(true)
                .deleted(false)
                .await
                .map_err(ObjectSyncError::from_opendal)?
                .into_iter()
                .filter(|entry| entry.path() == self.object_path)
                .filter_map(|entry| {
                    let metadata = entry.metadata();
                    let version_id = metadata.version()?.trim();
                    if version_id.is_empty() || version_id == "null" || metadata.is_deleted() {
                        return None;
                    }
                    Some(ObjectVersionSummary {
                        version_id: version_id.to_string(),
                        etag: metadata.etag().unwrap_or_default().to_string(),
                        content_length: metadata.content_length(),
                        last_modified: metadata
                            .last_modified()
                            .map(|timestamp| timestamp.to_string()),
                        is_current: metadata.is_current().unwrap_or(false),
                    })
                })
                .collect::<Vec<_>>();
            versions.sort_by(|left, right| {
                right
                    .is_current
                    .cmp(&left.is_current)
                    .then_with(|| right.last_modified.cmp(&left.last_modified))
            });
            versions.truncate(limit);
            Ok(versions)
        })
    }

    pub fn is_conflict(error: &ObjectSyncError) -> bool {
        matches!(
            error.kind,
            Some(ErrorKind::ConditionNotMatch | ErrorKind::AlreadyExists)
        )
    }
}

fn object_too_large() -> ObjectSyncError {
    ObjectSyncError::new("The encrypted sync object exceeds the 64 MiB safety limit.")
}

fn remote_changed() -> ObjectSyncError {
    ObjectSyncError {
        message: "The remote sync object changed before it could be written.".to_string(),
        kind: Some(ErrorKind::ConditionNotMatch),
    }
}

fn content_token(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut token = String::with_capacity(CONTENT_TOKEN_PREFIX.len() + digest.len() * 2);
    token.push_str(CONTENT_TOKEN_PREFIX);
    for byte in digest {
        write!(&mut token, "{byte:02x}").expect("writing to a String cannot fail");
    }
    token
}

fn is_content_token(value: &str) -> bool {
    value.len() == CONTENT_TOKEN_PREFIX.len() + 64
        && value.starts_with(CONTENT_TOKEN_PREFIX)
        && value[CONTENT_TOKEN_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn joins_optional_prefix_and_filename() {
        assert_eq!(
            join_object_path(" /backups/nauterm/ ", "vault.enc").unwrap(),
            "backups/nauterm/vault.enc"
        );
        assert_eq!(
            join_object_path("", "nauterm-sync.enc").unwrap(),
            "nauterm-sync.enc"
        );
    }

    #[test]
    fn rejects_paths_in_filename_and_ambiguous_prefixes() {
        assert!(join_object_path("backups", "nested/vault.enc").is_err());
        assert!(join_object_path("backups//daily", "vault.enc").is_err());
        assert!(join_object_path("backups/../daily", "vault.enc").is_err());
    }

    #[test]
    fn content_tokens_are_stable_and_distinguish_payloads() {
        let first = content_token(b"encrypted payload");
        assert!(is_content_token(&first));
        assert_eq!(first, content_token(b"encrypted payload"));
        assert_ne!(first, content_token(b"different payload"));
        assert!(!is_content_token("storage-etag"));
    }

    #[test]
    fn local_conflicts_participate_in_sync_retries() {
        assert!(OpenDalSyncTransport::is_conflict(&remote_changed()));
    }

    #[test]
    fn transport_falls_back_to_content_tokens_without_etags() {
        let operator = Operator::new(opendal::services::Memory::default()).unwrap();
        let transport = OpenDalSyncTransport::new(operator, "vault.enc".to_string()).unwrap();

        let first = transport.write(b"first encrypted payload", None).unwrap();
        assert!(is_content_token(&first.etag));
        let fetched = transport.fetch_current().unwrap().unwrap();
        assert_eq!(fetched.bytes, b"first encrypted payload");
        assert_eq!(fetched.etag, first.etag);

        let second = transport
            .write(b"second encrypted payload", Some(&fetched.etag))
            .unwrap();
        assert!(is_content_token(&second.etag));
        assert_ne!(second.etag, fetched.etag);

        let conflict = transport
            .write(b"stale overwrite", Some(&fetched.etag))
            .unwrap_err();
        assert!(OpenDalSyncTransport::is_conflict(&conflict));
    }
}
