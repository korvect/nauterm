use std::time::Duration;

use opendal::layers::{RetryLayer, TimeoutLayer};
use opendal::services::S3;
use opendal::Operator;
use serde::{Deserialize, Serialize};
use url::Url;
use zeroize::Zeroize;

use crate::object_sync::{join_object_path, ObjectSyncError, OpenDalSyncTransport};

const DEFAULT_FILENAME: &str = "nauterm-sync.enc";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct S3Config {
    pub endpoint: String,
    #[serde(default = "default_region")]
    pub region: String,
    pub bucket: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default = "default_filename")]
    pub filename: String,
}

#[derive(Serialize, Deserialize)]
pub struct S3Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl Drop for S3Credentials {
    fn drop(&mut self) {
        self.access_key_id.zeroize();
        self.secret_access_key.zeroize();
    }
}

fn default_region() -> String {
    "auto".to_string()
}

fn default_filename() -> String {
    DEFAULT_FILENAME.to_string()
}

pub fn validate_config(config: &S3Config) -> Result<(), ObjectSyncError> {
    normalized_config(config).map(|_| ())
}

pub fn build_transport(
    config: S3Config,
    mut credentials: S3Credentials,
) -> Result<OpenDalSyncTransport, ObjectSyncError> {
    let config = normalized_config(&config)?;
    let access_key_id = credentials.access_key_id.trim();
    let secret_access_key = credentials.secret_access_key.trim();
    if access_key_id.is_empty() {
        return Err(ObjectSyncError::new("S3 Access Key ID is required."));
    }
    if secret_access_key.is_empty() {
        return Err(ObjectSyncError::new("S3 Secret Access Key is required."));
    }
    if access_key_id.chars().any(char::is_control)
        || secret_access_key.chars().any(char::is_control)
    {
        return Err(ObjectSyncError::new(
            "S3 credentials must not contain control characters.",
        ));
    }
    let builder = S3::default()
        .endpoint(&config.endpoint)
        .region(&config.region)
        .bucket(&config.bucket)
        .access_key_id(access_key_id)
        .secret_access_key(secret_access_key)
        .disable_config_load();
    let operator = Operator::new(builder)
        .map_err(ObjectSyncError::from_opendal)?
        .layer(
            TimeoutLayer::default()
                .with_timeout(Duration::from_secs(30))
                .with_io_timeout(Duration::from_secs(30)),
        )
        .layer(RetryLayer::new().with_max_times(3).with_jitter());
    credentials.access_key_id.zeroize();
    credentials.secret_access_key.zeroize();
    OpenDalSyncTransport::new(
        operator,
        join_object_path(&config.prefix, &config.filename)?,
    )
}

fn normalized_config(config: &S3Config) -> Result<S3Config, ObjectSyncError> {
    let endpoint_text = config.endpoint.trim().trim_end_matches('/');
    let endpoint = Url::parse(endpoint_text)
        .map_err(|_| ObjectSyncError::new("S3 endpoint URL is invalid."))?;
    if !matches!(endpoint.scheme(), "http" | "https") || endpoint.host_str().is_none() {
        return Err(ObjectSyncError::new(
            "S3 endpoint must be an HTTP or HTTPS URL with a host.",
        ));
    }
    if !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err(ObjectSyncError::new(
            "S3 endpoint must not contain credentials, a query, or a fragment.",
        ));
    }
    let bucket = config.bucket.trim();
    if bucket.is_empty() || bucket.contains('/') || bucket.contains('\\') {
        return Err(ObjectSyncError::new("S3 bucket name is invalid."));
    }
    let prefix = config.prefix.trim().trim_matches('/');
    let filename = config.filename.trim();
    join_object_path(prefix, filename)?;
    let region = config.region.trim();
    Ok(S3Config {
        endpoint: endpoint_text.to_string(),
        region: if region.is_empty() { "auto" } else { region }.to_string(),
        bucket: bucket.to_string(),
        prefix: prefix.to_string(),
        filename: filename.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> S3Config {
        S3Config {
            endpoint: "https://storage.example.com".to_string(),
            region: "auto".to_string(),
            bucket: "vault".to_string(),
            prefix: "folder".to_string(),
            filename: "nauterm sync.enc".to_string(),
        }
    }

    #[test]
    fn builds_an_opendal_s3_operator_with_safe_capabilities() {
        build_transport(
            config(),
            S3Credentials {
                access_key_id: "access".to_string(),
                secret_access_key: "secret".to_string(),
            },
        )
        .unwrap();
    }

    #[test]
    fn rejects_ambiguous_endpoint_and_bucket_values() {
        let mut invalid = config();
        invalid.endpoint = "https://user@example.com/storage?x=1".to_string();
        assert!(validate_config(&invalid).is_err());
        invalid = config();
        invalid.bucket = "parent/child".to_string();
        assert!(validate_config(&invalid).is_err());
    }
}
