use std::collections::BTreeMap;
use std::time::Duration;

use opendal::layers::{RetryLayer, TimeoutLayer};
use opendal::services::{Azblob, Dropbox, Gcs, Gdrive, Onedrive, Webdav};
use opendal::Operator;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::object_sync::{
    join_object_path, ObjectSyncError, ObjectVersionSummary, OpenDalSyncTransport,
};
use crate::s3_sync::{S3Config, S3Credentials};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudProviderConfig {
    pub id: String,
    pub scheme: String,
    pub vendor: String,
    pub name: String,
    #[serde(default)]
    pub config: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudProviderSummary {
    #[serde(flatten)]
    pub provider: CloudProviderConfig,
    pub has_credentials: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncPreferences {
    #[serde(default)]
    pub active_provider_id: Option<String>,
    #[serde(default)]
    pub sync_snapshot: Option<SyncSnapshotStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncSnapshotStatus {
    pub revision: u64,
    pub snapshot_id: String,
}

#[derive(Default, Serialize, Deserialize)]
pub struct CloudProviderCredentials {
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

impl std::fmt::Debug for CloudProviderCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CloudProviderCredentials")
            .field("field_count", &self.values.len())
            .finish_non_exhaustive()
    }
}

impl Drop for CloudProviderCredentials {
    fn drop(&mut self) {
        for value in self.values.values_mut() {
            value.zeroize();
        }
    }
}

pub trait CloudTransportStrategy {
    fn build(
        &self,
        provider: &CloudProviderConfig,
        credentials: CloudProviderCredentials,
    ) -> Result<OpenDalSyncTransport, ObjectSyncError>;
}

struct S3TransportStrategy;

struct OpenDalServiceStrategy;

impl CloudTransportStrategy for S3TransportStrategy {
    fn build(
        &self,
        provider: &CloudProviderConfig,
        mut credentials: CloudProviderCredentials,
    ) -> Result<OpenDalSyncTransport, ObjectSyncError> {
        let config = S3Config {
            endpoint: required_config(provider, "endpoint")?.to_string(),
            region: config_or(provider, "region", "auto").to_string(),
            bucket: required_config(provider, "bucket")?.to_string(),
            prefix: config_or(provider, "prefix", "").to_string(),
            filename: required_config(provider, "filename")?.to_string(),
        };
        let access_key_id = take_required_credential(&mut credentials, "access_key_id")?;
        let secret_access_key = take_required_credential(&mut credentials, "secret_access_key")?;
        crate::s3_sync::build_transport(
            config,
            S3Credentials {
                access_key_id,
                secret_access_key,
            },
        )
    }
}

impl CloudTransportStrategy for OpenDalServiceStrategy {
    fn build(
        &self,
        provider: &CloudProviderConfig,
        mut credentials: CloudProviderCredentials,
    ) -> Result<OpenDalSyncTransport, ObjectSyncError> {
        let final_path = object_path(provider)?;
        macro_rules! finish_operator {
            ($builder:expr $(,)?) => {{
                Operator::new($builder)
                    .map_err(ObjectSyncError::from_opendal)?
                    .layer(
                        TimeoutLayer::default()
                            .with_timeout(Duration::from_secs(30))
                            .with_io_timeout(Duration::from_secs(30)),
                    )
                    .layer(RetryLayer::new().with_max_times(3).with_jitter())
            }};
        }
        let operator = match provider.scheme.as_str() {
            "webdav" => finish_operator!(Webdav::default()
                .endpoint(required_config(provider, "endpoint")?)
                .root(config_or(provider, "root", "/"))
                .username(&take_required_credential(&mut credentials, "username")?)
                .password(&take_required_credential(&mut credentials, "password")?),),
            "azblob" => finish_operator!(Azblob::default()
                .endpoint(required_config(provider, "endpoint")?)
                .container(required_config(provider, "container")?)
                .account_name(&take_required_credential(&mut credentials, "account_name")?)
                .account_key(&take_required_credential(&mut credentials, "account_key")?),),
            "gcs" => {
                let mut access_token = take_required_credential(&mut credentials, "access_token")?;
                let operator = finish_operator!(Gcs::default()
                    .bucket(required_config(provider, "bucket")?)
                    .token(access_token.clone()));
                access_token.zeroize();
                operator
            }
            "gdrive" => {
                let mut access_token = take_required_credential(&mut credentials, "access_token")?;
                let operator = finish_operator!(Gdrive::default()
                    .root(config_or(provider, "root", "/"))
                    .access_token(&access_token));
                access_token.zeroize();
                operator
            }
            "onedrive" => {
                let mut access_token = take_required_credential(&mut credentials, "access_token")?;
                let operator = finish_operator!(Onedrive::default()
                    .root(config_or(provider, "root", "/"))
                    .access_token(&access_token));
                access_token.zeroize();
                operator
            }
            "dropbox" => {
                let mut access_token = take_required_credential(&mut credentials, "access_token")?;
                let operator = finish_operator!(Dropbox::default()
                    .root(config_or(provider, "root", "/"))
                    .access_token(&access_token));
                access_token.zeroize();
                operator
            }
            _ => {
                return Err(ObjectSyncError::new(format!(
                    "Unsupported cloud storage scheme: {}.",
                    provider.scheme
                )))
            }
        };
        OpenDalSyncTransport::new(operator, final_path)
    }
}

pub fn validate_provider(provider: &CloudProviderConfig) -> Result<(), ObjectSyncError> {
    if provider.id.trim().is_empty()
        || provider.id.len() > 128
        || !provider
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ObjectSyncError::new("Cloud provider ID is invalid."));
    }
    if matches!(
        provider.id.as_str(),
        "github_repository" | "github_gist" | "s3"
    ) {
        return Err(ObjectSyncError::new("Cloud provider ID is reserved."));
    }
    if provider.name.trim().is_empty() || provider.name.chars().count() > 80 {
        return Err(ObjectSyncError::new(
            "Cloud provider name must contain 1 to 80 characters.",
        ));
    }
    if provider.vendor.trim().is_empty() || provider.vendor.len() > 64 {
        return Err(ObjectSyncError::new("Cloud provider vendor is invalid."));
    }
    if provider.config.len() > 32
        || provider.config.iter().any(|(key, value)| {
            key.is_empty()
                || key.len() > 64
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                || value.len() > 4096
        })
    {
        return Err(ObjectSyncError::new(
            "Cloud provider configuration is too large or contains an invalid field.",
        ));
    }
    match provider.scheme.as_str() {
        "s3" => crate::s3_sync::validate_config(&S3Config {
            endpoint: required_config(provider, "endpoint")?.to_string(),
            region: config_or(provider, "region", "auto").to_string(),
            bucket: required_config(provider, "bucket")?.to_string(),
            prefix: config_or(provider, "prefix", "").to_string(),
            filename: required_config(provider, "filename")?.to_string(),
        }),
        "webdav" => validate_required_configs(provider, &["endpoint", "filename"])
            .and_then(|_| object_path(provider).map(|_| ())),
        "azblob" => validate_required_configs(provider, &["endpoint", "container", "filename"])
            .and_then(|_| object_path(provider).map(|_| ())),
        "gcs" => validate_required_configs(provider, &["bucket", "filename"])
            .and_then(|_| object_path(provider).map(|_| ())),
        "gdrive" | "onedrive" | "dropbox" => validate_required_configs(provider, &["filename"])
            .and_then(|_| object_path(provider).map(|_| ())),
        _ => Err(ObjectSyncError::new(format!(
            "Unsupported cloud storage scheme: {}.",
            provider.scheme
        ))),
    }
}

pub fn validate_credentials(credentials: &CloudProviderCredentials) -> Result<(), ObjectSyncError> {
    if credentials.values.is_empty()
        || credentials.values.len() > 16
        || credentials.values.iter().any(|(key, value)| {
            key.is_empty()
                || key.len() > 64
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                || value.trim().is_empty()
                || value.len() > 16_384
        })
    {
        return Err(ObjectSyncError::new(
            "Cloud provider credentials are invalid.",
        ));
    }
    Ok(())
}

pub fn build_transport(
    provider: &CloudProviderConfig,
    credentials: CloudProviderCredentials,
) -> Result<OpenDalSyncTransport, ObjectSyncError> {
    validate_provider(provider)?;
    match provider.scheme.as_str() {
        "s3" => S3TransportStrategy.build(provider, credentials),
        "webdav" | "azblob" | "gcs" | "gdrive" | "onedrive" | "dropbox" => {
            OpenDalServiceStrategy.build(provider, credentials)
        }
        _ => Err(ObjectSyncError::new(format!(
            "Unsupported cloud storage scheme: {}.",
            provider.scheme
        ))),
    }
}

pub struct OAuthRefreshResult {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

impl Drop for OAuthRefreshResult {
    fn drop(&mut self) {
        self.access_token.zeroize();
        if let Some(refresh_token) = self.refresh_token.as_mut() {
            refresh_token.zeroize();
        }
    }
}

pub fn refresh_oauth_access_token(
    provider: &CloudProviderConfig,
    credentials: &CloudProviderCredentials,
) -> Result<OAuthRefreshResult, ObjectSyncError> {
    let (label, token_endpoint, scope) = match provider.scheme.as_str() {
        "gcs" => (
            "Google Cloud Storage",
            "https://oauth2.googleapis.com/token",
            None,
        ),
        "gdrive" => ("Google Drive", "https://oauth2.googleapis.com/token", None),
        "onedrive" => (
            "OneDrive",
            "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            Some("offline_access Files.ReadWrite"),
        ),
        "dropbox" => ("Dropbox", "https://api.dropboxapi.com/oauth2/token", None),
        _ => {
            return Err(ObjectSyncError::new(format!(
                "OAuth refresh is unsupported for {}.",
                provider.scheme
            )))
        }
    };
    let form = oauth_refresh_form(provider.scheme.as_str(), credentials, scope)?;
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| ObjectSyncError::new(format!("Unable to prepare {label} OAuth: {error}")))?
        .post(token_endpoint)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&form)
        .send()
        .map_err(|error| {
            ObjectSyncError::new(format!("Unable to refresh {label} access: {error}"))
        })?;
    let status = response.status();
    let body = Zeroizing::new(response.text().map_err(|error| {
        ObjectSyncError::new(format!("Unable to read {label} OAuth response: {error}"))
    })?);
    parse_oauth_refresh_response(label, status.as_u16(), body.as_str())
}

pub struct GoogleDriveHistoryClient {
    client: reqwest::blocking::Client,
    access_token: Zeroizing<String>,
    root: String,
    filename: String,
}

impl GoogleDriveHistoryClient {
    pub fn new(
        provider: &CloudProviderConfig,
        access_token: String,
    ) -> Result<Self, ObjectSyncError> {
        if provider.scheme != "gdrive" || access_token.trim().is_empty() {
            return Err(ObjectSyncError::new(
                "Google Drive history credentials are invalid.",
            ));
        }
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|error| {
                    ObjectSyncError::new(format!("Unable to prepare Google Drive history: {error}"))
                })?,
            access_token: Zeroizing::new(access_token),
            root: config_or(provider, "root", "/").to_string(),
            filename: required_config(provider, "filename")?.to_string(),
        })
    }

    pub fn list_versions(
        &self,
        limit: usize,
    ) -> Result<Vec<ObjectVersionSummary>, ObjectSyncError> {
        let file_id = self.resolve_file_id()?;
        let response = self
            .client
            .get(format!(
                "https://www.googleapis.com/drive/v3/files/{file_id}/revisions"
            ))
            .bearer_auth(self.access_token.as_str())
            .query(&[
                ("pageSize", limit.clamp(1, 100).to_string()),
                (
                    "fields",
                    "revisions(id,modifiedTime,size,md5Checksum),nextPageToken".to_string(),
                ),
            ])
            .send()
            .map_err(google_drive_request_error)?;
        let body: serde_json::Value =
            google_drive_expect_ok(response)?.json().map_err(|error| {
                ObjectSyncError::new(format!(
                    "Google Drive returned invalid revision history: {error}"
                ))
            })?;
        let mut versions = body["revisions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|revision| {
                let version_id = revision["id"].as_str()?.to_string();
                Some(ObjectVersionSummary {
                    version_id,
                    etag: revision["md5Checksum"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    content_length: revision["size"]
                        .as_str()
                        .and_then(|value| value.parse().ok())
                        .or_else(|| revision["size"].as_u64())
                        .unwrap_or(0),
                    last_modified: revision["modifiedTime"].as_str().map(str::to_string),
                    is_current: false,
                })
            })
            .collect::<Vec<_>>();
        versions.sort_by(|left, right| right.last_modified.cmp(&left.last_modified));
        if let Some(current) = versions.first_mut() {
            current.is_current = true;
        }
        versions.truncate(limit.clamp(1, 100));
        Ok(versions)
    }

    pub fn read_version(&self, version_id: &str) -> Result<Vec<u8>, ObjectSyncError> {
        if version_id.trim().is_empty()
            || version_id.len() > 256
            || !version_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(ObjectSyncError::new(
                "Google Drive revision identifier is invalid.",
            ));
        }
        let file_id = self.resolve_file_id()?;
        let response = self
            .client
            .get(format!(
                "https://www.googleapis.com/drive/v3/files/{file_id}/revisions/{version_id}"
            ))
            .bearer_auth(self.access_token.as_str())
            .query(&[("alt", "media")])
            .send()
            .map_err(google_drive_request_error)?;
        let bytes = google_drive_expect_ok(response)?
            .bytes()
            .map_err(|error| {
                ObjectSyncError::new(format!("Unable to read Google Drive revision: {error}"))
            })?
            .to_vec();
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(ObjectSyncError::new(
                "Google Drive revision exceeds the 64 MiB sync object limit.",
            ));
        }
        Ok(bytes)
    }

    fn resolve_file_id(&self) -> Result<String, ObjectSyncError> {
        let mut parent_id = "root".to_string();
        for segment in self
            .root
            .trim()
            .trim_matches('/')
            .split('/')
            .filter(|segment| !segment.is_empty())
        {
            parent_id = self
                .find_child(&parent_id, segment, true)?
                .ok_or_else(|| ObjectSyncError::new("Google Drive root folder was not found."))?;
        }
        self.find_child(&parent_id, &self.filename, false)?
            .ok_or_else(|| ObjectSyncError::new("Google Drive sync file was not found."))
    }

    fn find_child(
        &self,
        parent_id: &str,
        name: &str,
        folder: bool,
    ) -> Result<Option<String>, ObjectSyncError> {
        let mut query = format!(
            "name = '{}' and '{}' in parents and trashed = false",
            google_drive_query_literal(name),
            google_drive_query_literal(parent_id),
        );
        if folder {
            query.push_str(" and mimeType = 'application/vnd.google-apps.folder'");
        }
        let response = self
            .client
            .get("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(self.access_token.as_str())
            .query(&[
                ("q", query),
                ("spaces", "drive".to_string()),
                ("pageSize", "2".to_string()),
                ("fields", "files(id,name,mimeType)".to_string()),
            ])
            .send()
            .map_err(google_drive_request_error)?;
        let body: serde_json::Value =
            google_drive_expect_ok(response)?.json().map_err(|error| {
                ObjectSyncError::new(format!("Google Drive returned invalid file data: {error}"))
            })?;
        let files = body["files"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        if files.len() > 1 {
            return Err(ObjectSyncError::new(format!(
                "Google Drive contains multiple items named {name} in the configured folder."
            )));
        }
        Ok(files
            .first()
            .and_then(|file| file["id"].as_str())
            .map(str::to_string))
    }
}

fn google_drive_query_literal(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn google_drive_request_error(error: reqwest::Error) -> ObjectSyncError {
    ObjectSyncError::new(format!("Google Drive history request failed: {error}"))
}

fn google_drive_expect_ok(
    response: reqwest::blocking::Response,
) -> Result<reqwest::blocking::Response, ObjectSyncError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().unwrap_or_default();
    let detail = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| value["error"]["message"].as_str().map(str::to_string))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "request failed".to_string());
    Err(ObjectSyncError::new(format!(
        "Google Drive history error ({status}): {detail}"
    )))
}

pub struct OneDriveHistoryClient {
    client: reqwest::blocking::Client,
    access_token: Zeroizing<String>,
    object_path: String,
}

impl OneDriveHistoryClient {
    pub fn new(
        provider: &CloudProviderConfig,
        access_token: String,
    ) -> Result<Self, ObjectSyncError> {
        if provider.scheme != "onedrive" || access_token.trim().is_empty() {
            return Err(ObjectSyncError::new(
                "OneDrive history credentials are invalid.",
            ));
        }
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|error| {
                    ObjectSyncError::new(format!("Unable to prepare OneDrive history: {error}"))
                })?,
            access_token: Zeroizing::new(access_token),
            object_path: join_object_path(
                config_or(provider, "root", "/"),
                required_config(provider, "filename")?,
            )?,
        })
    }

    pub fn list_versions(
        &self,
        limit: usize,
    ) -> Result<Vec<ObjectVersionSummary>, ObjectSyncError> {
        let item_id = self.resolve_item_id()?;
        let response = self
            .client
            .get(format!(
                "https://graph.microsoft.com/v1.0/me/drive/items/{item_id}/versions"
            ))
            .bearer_auth(self.access_token.as_str())
            .query(&[
                ("$top", limit.clamp(1, 100).to_string()),
                ("$select", "id,lastModifiedDateTime,size".to_string()),
            ])
            .send()
            .map_err(onedrive_request_error)?;
        let body: serde_json::Value = onedrive_expect_ok(response)?.json().map_err(|error| {
            ObjectSyncError::new(format!(
                "OneDrive returned invalid version history: {error}"
            ))
        })?;
        Ok(parse_onedrive_versions(&body, limit))
    }

    pub fn restore_version(&self, version_id: &str) -> Result<(), ObjectSyncError> {
        validate_onedrive_version_id(version_id)?;
        let item_id = self.resolve_item_id()?;
        let response = self
            .client
            .post(format!(
                "https://graph.microsoft.com/v1.0/me/drive/items/{item_id}/versions/{version_id}/restoreVersion"
            ))
            .bearer_auth(self.access_token.as_str())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body("")
            .send()
            .map_err(onedrive_request_error)?;
        onedrive_expect_ok(response)?;
        Ok(())
    }

    fn resolve_item_id(&self) -> Result<String, ObjectSyncError> {
        let encoded_path = self
            .object_path
            .split('/')
            .map(|segment| utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string())
            .collect::<Vec<_>>()
            .join("/");
        let response = self
            .client
            .get(format!(
                "https://graph.microsoft.com/v1.0/me/drive/root:/{encoded_path}"
            ))
            .bearer_auth(self.access_token.as_str())
            .query(&[("$select", "id")])
            .send()
            .map_err(onedrive_request_error)?;
        let body: serde_json::Value = onedrive_expect_ok(response)?.json().map_err(|error| {
            ObjectSyncError::new(format!("OneDrive returned invalid file data: {error}"))
        })?;
        body["id"]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| ObjectSyncError::new("OneDrive sync file has no item ID."))
    }
}

fn parse_onedrive_versions(body: &serde_json::Value, limit: usize) -> Vec<ObjectVersionSummary> {
    let mut versions = body["value"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|version| {
            let version_id = version["id"].as_str()?.to_string();
            Some(ObjectVersionSummary {
                version_id,
                etag: String::new(),
                content_length: version["size"].as_u64().unwrap_or(0),
                last_modified: version["lastModifiedDateTime"].as_str().map(str::to_string),
                is_current: false,
            })
        })
        .collect::<Vec<_>>();
    versions.sort_by(|left, right| right.last_modified.cmp(&left.last_modified));
    if let Some(current) = versions.first_mut() {
        current.is_current = true;
    }
    versions.truncate(limit.clamp(1, 100));
    versions
}

fn validate_onedrive_version_id(version_id: &str) -> Result<(), ObjectSyncError> {
    if version_id.trim().is_empty()
        || version_id.len() > 256
        || !version_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(ObjectSyncError::new(
            "OneDrive version identifier is invalid.",
        ));
    }
    Ok(())
}

fn onedrive_request_error(error: reqwest::Error) -> ObjectSyncError {
    ObjectSyncError::new(format!("OneDrive history request failed: {error}"))
}

fn onedrive_expect_ok(
    response: reqwest::blocking::Response,
) -> Result<reqwest::blocking::Response, ObjectSyncError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().unwrap_or_default();
    let detail = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| value["error"]["message"].as_str().map(str::to_string))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "request failed".to_string());
    Err(ObjectSyncError::new(format!(
        "OneDrive history error ({status}): {detail}"
    )))
}

pub struct DropboxHistoryClient {
    client: reqwest::blocking::Client,
    access_token: Zeroizing<String>,
    object_path: String,
}

impl DropboxHistoryClient {
    pub fn new(
        provider: &CloudProviderConfig,
        access_token: String,
    ) -> Result<Self, ObjectSyncError> {
        if provider.scheme != "dropbox" || access_token.trim().is_empty() {
            return Err(ObjectSyncError::new(
                "Dropbox history credentials are invalid.",
            ));
        }
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|error| {
                    ObjectSyncError::new(format!("Unable to prepare Dropbox history: {error}"))
                })?,
            access_token: Zeroizing::new(access_token),
            object_path: format!(
                "/{}",
                join_object_path(
                    config_or(provider, "root", "/"),
                    required_config(provider, "filename")?,
                )?
            ),
        })
    }

    pub fn list_versions(
        &self,
        limit: usize,
    ) -> Result<Vec<ObjectVersionSummary>, ObjectSyncError> {
        let response = self
            .client
            .post("https://api.dropboxapi.com/2/files/list_revisions")
            .bearer_auth(self.access_token.as_str())
            .json(&serde_json::json!({
                "path": &self.object_path,
                "mode": {".tag": "path"},
                "limit": limit.clamp(1, 100),
                "include_restorable_info": true,
            }))
            .send()
            .map_err(dropbox_request_error)?;
        let body: serde_json::Value = dropbox_expect_ok(response)?.json().map_err(|error| {
            ObjectSyncError::new(format!(
                "Dropbox returned invalid revision history: {error}"
            ))
        })?;
        Ok(parse_dropbox_versions(&body, limit))
    }

    pub fn restore_version(&self, version_id: &str) -> Result<(), ObjectSyncError> {
        validate_dropbox_version_id(version_id)?;
        let response = self
            .client
            .post("https://api.dropboxapi.com/2/files/restore")
            .bearer_auth(self.access_token.as_str())
            .json(&serde_json::json!({
                "path": &self.object_path,
                "rev": version_id,
            }))
            .send()
            .map_err(dropbox_request_error)?;
        dropbox_expect_ok(response)?;
        Ok(())
    }
}

fn parse_dropbox_versions(body: &serde_json::Value, limit: usize) -> Vec<ObjectVersionSummary> {
    let mut versions = body["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|revision| {
            let version_id = revision["rev"].as_str()?.to_string();
            Some((
                ObjectVersionSummary {
                    version_id,
                    etag: revision["content_hash"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    content_length: revision["size"].as_u64().unwrap_or(0),
                    last_modified: revision["server_modified"].as_str().map(str::to_string),
                    is_current: false,
                },
                revision["is_restorable"].as_bool().unwrap_or(true),
            ))
        })
        .collect::<Vec<_>>();
    versions.sort_by(|left, right| right.0.last_modified.cmp(&left.0.last_modified));
    versions
        .into_iter()
        .enumerate()
        .filter_map(|(index, (mut version, restorable))| {
            if index != 0 && !restorable {
                return None;
            }
            version.is_current = index == 0;
            Some(version)
        })
        .take(limit.clamp(1, 100))
        .collect()
}

fn validate_dropbox_version_id(version_id: &str) -> Result<(), ObjectSyncError> {
    if version_id.trim().is_empty()
        || version_id.len() > 256
        || !version_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ObjectSyncError::new(
            "Dropbox revision identifier is invalid.",
        ));
    }
    Ok(())
}

fn dropbox_request_error(error: reqwest::Error) -> ObjectSyncError {
    ObjectSyncError::new(format!("Dropbox history request failed: {error}"))
}

fn dropbox_expect_ok(
    response: reqwest::blocking::Response,
) -> Result<reqwest::blocking::Response, ObjectSyncError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().unwrap_or_default();
    let detail = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| value["error_summary"].as_str().map(str::to_string))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "request failed".to_string());
    Err(ObjectSyncError::new(format!(
        "Dropbox history error ({status}): {detail}"
    )))
}

fn parse_oauth_refresh_response(
    label: &str,
    status: u16,
    body: &str,
) -> Result<OAuthRefreshResult, ObjectSyncError> {
    let payload: serde_json::Value = serde_json::from_str(body).map_err(|_| {
        ObjectSyncError::new(format!("{label} returned an invalid OAuth response."))
    })?;
    if !(200..300).contains(&status) {
        let detail = payload
            .get("error_description")
            .or_else(|| payload.get("error_summary"))
            .or_else(|| payload.get("error"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("access token refresh failed");
        return Err(ObjectSyncError::new(format!("{label} OAuth: {detail}.")));
    }
    let access_token = payload
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ObjectSyncError::new(format!("{label} did not return an access token.")))?
        .to_string();
    let refresh_token = payload
        .get("refresh_token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(OAuthRefreshResult {
        access_token,
        refresh_token,
    })
}

fn credential<'a>(
    credentials: &'a CloudProviderCredentials,
    key: &str,
) -> Result<&'a str, ObjectSyncError> {
    credentials
        .values
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ObjectSyncError::new(format!("Cloud provider credential {key} is required."))
        })
}

fn optional_credential<'a>(
    credentials: &'a CloudProviderCredentials,
    key: &str,
) -> Option<&'a str> {
    credentials
        .values
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn oauth_refresh_form<'a>(
    provider_scheme: &str,
    credentials: &'a CloudProviderCredentials,
    scope: Option<&'static str>,
) -> Result<Vec<(&'static str, &'a str)>, ObjectSyncError> {
    let mut form = vec![
        ("grant_type", "refresh_token"),
        ("client_id", credential(credentials, "client_id")?),
        ("refresh_token", credential(credentials, "refresh_token")?),
    ];
    if matches!(provider_scheme, "gcs" | "gdrive") {
        if let Some(client_secret) = optional_credential(credentials, "client_secret") {
            form.push(("client_secret", client_secret));
        }
    }
    if let Some(scope) = scope {
        form.push(("scope", scope));
    }
    Ok(form)
}

fn validate_required_configs(
    provider: &CloudProviderConfig,
    keys: &[&str],
) -> Result<(), ObjectSyncError> {
    for key in keys {
        required_config(provider, key)?;
    }
    Ok(())
}

fn required_config<'a>(
    provider: &'a CloudProviderConfig,
    key: &str,
) -> Result<&'a str, ObjectSyncError> {
    provider
        .config
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ObjectSyncError::new(format!("Cloud provider field {key} is required.")))
}

fn config_or<'a>(provider: &'a CloudProviderConfig, key: &str, fallback: &'a str) -> &'a str {
    provider
        .config
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
}

fn object_path(provider: &CloudProviderConfig) -> Result<String, ObjectSyncError> {
    join_object_path(
        config_or(provider, "prefix", ""),
        required_config(provider, "filename")?,
    )
}

fn take_required_credential(
    credentials: &mut CloudProviderCredentials,
    key: &str,
) -> Result<String, ObjectSyncError> {
    credentials
        .values
        .remove(key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ObjectSyncError::new(format!("Cloud provider credential {key} is required."))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> CloudProviderConfig {
        CloudProviderConfig {
            id: "019f-provider".to_string(),
            scheme: "s3".to_string(),
            vendor: "cloudflare-r2".to_string(),
            name: "R2 Production".to_string(),
            config: BTreeMap::from([
                (
                    "endpoint".to_string(),
                    "https://account.r2.cloudflarestorage.com".to_string(),
                ),
                ("region".to_string(), "auto".to_string()),
                ("bucket".to_string(), "nauterm".to_string()),
                ("prefix".to_string(), "backups".to_string()),
                ("filename".to_string(), "vault.enc".to_string()),
            ]),
        }
    }

    #[test]
    fn scheme_strategy_builds_an_s3_vendor() {
        build_transport(
            &provider(),
            CloudProviderCredentials {
                values: BTreeMap::from([
                    ("access_key_id".to_string(), "access".to_string()),
                    ("secret_access_key".to_string(), "secret".to_string()),
                ]),
            },
        )
        .unwrap();
    }

    #[test]
    fn provider_validation_rejects_unknown_schemes_and_unsafe_ids() {
        let mut value = provider();
        value.scheme = "unknown".to_string();
        assert!(validate_provider(&value).is_err());
        value.scheme = "s3".to_string();
        value.id = "../provider".to_string();
        assert!(validate_provider(&value).is_err());
    }

    #[test]
    fn oauth_drive_transport_uses_short_lived_access_token() {
        let provider = CloudProviderConfig {
            id: "019f-drive".to_string(),
            scheme: "gdrive".to_string(),
            vendor: "google-drive".to_string(),
            name: "Google Drive".to_string(),
            config: BTreeMap::from([
                ("root".to_string(), "/Nauterm".to_string()),
                ("filename".to_string(), "nauterm-sync.enc".to_string()),
            ]),
        };
        build_transport(
            &provider,
            CloudProviderCredentials {
                values: BTreeMap::from([(
                    "access_token".to_string(),
                    "short-lived-token".to_string(),
                )]),
            },
        )
        .unwrap();
    }

    #[test]
    fn gcs_transport_uses_short_lived_access_token() {
        let provider = CloudProviderConfig {
            id: "019f-gcs".to_string(),
            scheme: "gcs".to_string(),
            vendor: "google-cloud-storage".to_string(),
            name: "Google Cloud Storage".to_string(),
            config: BTreeMap::from([
                ("bucket".to_string(), "nauterm-sync".to_string()),
                ("filename".to_string(), "nauterm-sync.enc".to_string()),
            ]),
        };
        build_transport(
            &provider,
            CloudProviderCredentials {
                values: BTreeMap::from([(
                    "access_token".to_string(),
                    "short-lived-token".to_string(),
                )]),
            },
        )
        .unwrap();
    }

    #[test]
    fn google_drive_query_literals_escape_quotes_and_backslashes() {
        assert_eq!(google_drive_query_literal(r"team's\sync"), r"team\'s\\sync");
    }

    #[test]
    fn google_drive_history_rejects_revision_paths() {
        let provider = CloudProviderConfig {
            id: "019f-drive".to_string(),
            scheme: "gdrive".to_string(),
            vendor: "google-drive".to_string(),
            name: "Google Drive".to_string(),
            config: BTreeMap::from([("filename".to_string(), "nauterm-sync.enc".to_string())]),
        };
        let client = GoogleDriveHistoryClient::new(&provider, "token".to_string()).unwrap();
        assert!(client.read_version("../files/other").is_err());
    }

    #[test]
    fn onedrive_versions_are_sorted_and_mark_the_latest_current() {
        let body = serde_json::json!({
            "value": [
                {"id": "1.0", "lastModifiedDateTime": "2026-07-20T10:00:00Z", "size": 12},
                {"id": "3.0", "lastModifiedDateTime": "2026-07-22T10:00:00Z", "size": 34},
                {"id": "2.0", "lastModifiedDateTime": "2026-07-21T10:00:00Z", "size": 23}
            ]
        });

        let versions = parse_onedrive_versions(&body, 2);

        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version_id, "3.0");
        assert!(versions[0].is_current);
        assert_eq!(versions[1].version_id, "2.0");
        assert!(!versions[1].is_current);
        assert!(validate_onedrive_version_id("3.0").is_ok());
        assert!(validate_onedrive_version_id("../3.0").is_err());
    }

    #[test]
    fn onedrive_history_uses_the_configured_root() {
        let provider = CloudProviderConfig {
            id: "019f-onedrive".to_string(),
            scheme: "onedrive".to_string(),
            vendor: "onedrive".to_string(),
            name: "OneDrive".to_string(),
            config: BTreeMap::from([
                ("root".to_string(), "/Nauterm Backups".to_string()),
                ("filename".to_string(), "nauterm-sync.enc".to_string()),
            ]),
        };

        let client = OneDriveHistoryClient::new(&provider, "token".to_string()).unwrap();

        assert_eq!(client.object_path, "Nauterm Backups/nauterm-sync.enc");
    }

    #[test]
    fn dropbox_versions_are_sorted_and_mark_the_latest_current() {
        let body = serde_json::json!({
            "entries": [
                {
                    "rev": "a1",
                    "server_modified": "2026-07-20T10:00:00Z",
                    "size": 12,
                    "content_hash": "hash-a1",
                    "is_restorable": true
                },
                {
                    "rev": "c3",
                    "server_modified": "2026-07-22T10:00:00Z",
                    "size": 34,
                    "content_hash": "hash-c3",
                    "is_restorable": false
                },
                {
                    "rev": "b2",
                    "server_modified": "2026-07-21T10:00:00Z",
                    "size": 23,
                    "is_restorable": false
                }
            ]
        });

        let versions = parse_dropbox_versions(&body, 20);

        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version_id, "c3");
        assert_eq!(versions[0].etag, "hash-c3");
        assert!(versions[0].is_current);
        assert_eq!(versions[1].version_id, "a1");
        assert!(!versions[1].is_current);
        assert!(validate_dropbox_version_id("015abc_DEF").is_ok());
        assert!(validate_dropbox_version_id("../015abc").is_err());
    }

    #[test]
    fn dropbox_history_uses_the_app_folder_relative_root() {
        let provider = CloudProviderConfig {
            id: "019f-dropbox".to_string(),
            scheme: "dropbox".to_string(),
            vendor: "dropbox".to_string(),
            name: "Dropbox".to_string(),
            config: BTreeMap::from([
                ("root".to_string(), "/Backups".to_string()),
                ("filename".to_string(), "nauterm-sync.enc".to_string()),
            ]),
        };

        let client = DropboxHistoryClient::new(&provider, "token".to_string()).unwrap();

        assert_eq!(client.object_path, "/Backups/nauterm-sync.enc");
    }

    #[test]
    fn oauth_refresh_response_accepts_rotated_refresh_token() {
        let mut result = parse_oauth_refresh_response(
            "OneDrive",
            200,
            r#"{"access_token":"access","refresh_token":"rotated"}"#,
        )
        .unwrap();
        assert_eq!(result.access_token, "access");
        assert_eq!(result.refresh_token.as_deref(), Some("rotated"));
        result.access_token.zeroize();
    }

    #[test]
    fn google_oauth_refresh_does_not_require_a_client_secret() {
        let credentials = CloudProviderCredentials {
            values: BTreeMap::from([
                ("client_id".to_string(), "desktop-client".to_string()),
                ("refresh_token".to_string(), "refresh".to_string()),
            ]),
        };

        let form = oauth_refresh_form("gdrive", &credentials, None).unwrap();

        assert!(form.contains(&("client_id", "desktop-client")));
        assert!(form.contains(&("refresh_token", "refresh")));
        assert!(!form.iter().any(|(key, _)| *key == "client_secret"));
    }

    #[test]
    fn google_oauth_refresh_preserves_a_configured_client_secret() {
        let credentials = CloudProviderCredentials {
            values: BTreeMap::from([
                ("client_id".to_string(), "desktop-client".to_string()),
                ("client_secret".to_string(), "compatible-secret".to_string()),
                ("refresh_token".to_string(), "refresh".to_string()),
            ]),
        };

        let form = oauth_refresh_form("gcs", &credentials, None).unwrap();

        assert!(form.contains(&("client_secret", "compatible-secret")));
    }

    #[test]
    fn oauth_refresh_response_rejects_missing_access_token() {
        let error = parse_oauth_refresh_response("Dropbox", 200, r#"{"token_type":"bearer"}"#)
            .err()
            .expect("missing access token should fail");
        assert!(error.to_string().contains("did not return an access token"));
    }
}
