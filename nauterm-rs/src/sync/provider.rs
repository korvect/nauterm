//! Reference provider fields and save drafts, with write-only credentials.
use super::*;
use nauterm_core::{SyncProviderCredentials, SyncProviderRecord, SyncProviderUpdate};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Repository,
    Gist,
    LegacyS3,
    S3,
    Minio,
    Webdav,
    Azure,
    Gcs,
    Drive,
    Onedrive,
    Dropbox,
}
#[derive(Clone, Copy)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub default: &'static str,
    pub credential: bool,
    pub masked: bool,
    pub readonly: bool,
}
impl Field {
    const fn config(key: &'static str, label: &'static str, default: &'static str) -> Self {
        Self {
            key,
            label,
            default,
            credential: false,
            masked: false,
            readonly: false,
        }
    }
    const fn credential(key: &'static str, label: &'static str, masked: bool) -> Self {
        Self {
            key,
            label,
            default: "",
            credential: true,
            masked,
            readonly: false,
        }
    }
}
impl Kind {
    pub const BUILT_IN: [Self; 3] = [Self::Repository, Self::Gist, Self::LegacyS3];
    pub const CATALOG: [Self; 8] = [
        Self::Minio,
        Self::S3,
        Self::Webdav,
        Self::Azure,
        Self::Gcs,
        Self::Drive,
        Self::Onedrive,
        Self::Dropbox,
    ];
    pub fn oauth(self) -> Option<oauth::Vendor> {
        Some(match self {
            Self::Gist => oauth::Vendor::GithubGist,
            Self::Gcs => oauth::Vendor::GoogleCloudStorage,
            Self::Drive => oauth::Vendor::GoogleDrive,
            Self::Onedrive => oauth::Vendor::OneDrive,
            Self::Dropbox => oauth::Vendor::Dropbox,
            _ => return None,
        })
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Repository => "GitHub Repository",
            Self::Gist => "GitHub Gist",
            Self::LegacyS3 | Self::S3 => "S3 Compatible",
            Self::Minio => "MinIO",
            Self::Webdav => "WebDAV",
            Self::Azure => "Azure Blob Storage",
            Self::Gcs => "Google Cloud Storage",
            Self::Drive => "Google Drive",
            Self::Onedrive => "OneDrive",
            Self::Dropbox => "Dropbox",
        }
    }
    pub fn fixed_id(self) -> Option<&'static str> {
        match self {
            Self::Repository => Some("github_repository"),
            Self::Gist => Some("github_gist"),
            Self::LegacyS3 => Some("s3"),
            _ => None,
        }
    }
    fn vendor(self) -> &'static str {
        match self {
            Self::Repository => "github_repository",
            Self::Gist => "github_gist",
            Self::LegacyS3 => "s3_compatible",
            Self::S3 => "s3-compatible",
            Self::Minio => "minio",
            Self::Webdav => "webdav",
            Self::Azure => "azure-blob",
            Self::Gcs => "google-cloud-storage",
            Self::Drive => "google-drive",
            Self::Onedrive => "onedrive",
            Self::Dropbox => "dropbox",
        }
    }
    fn scheme(self) -> &'static str {
        match self {
            Self::S3 | Self::Minio | Self::LegacyS3 => "s3",
            Self::Webdav => "webdav",
            Self::Azure => "azblob",
            Self::Gcs => "gcs",
            Self::Drive => "gdrive",
            Self::Onedrive => "onedrive",
            Self::Dropbox => "dropbox",
            Self::Repository | Self::Gist => "",
        }
    }
    pub fn of(record: &SyncProviderRecord) -> Result<Self> {
        if record.uuid == "github_repository" {
            return Ok(Self::Repository);
        }
        if record.uuid == "github_gist" {
            return Ok(Self::Gist);
        }
        if record.uuid == "s3" {
            return Ok(Self::LegacyS3);
        }
        Ok(match record.config["scheme"].as_str() {
            Some("s3") if record.provider == "minio" => Self::Minio,
            Some("s3") => Self::S3,
            Some("webdav") => Self::Webdav,
            Some("azblob") => Self::Azure,
            Some("gcs") => Self::Gcs,
            Some("gdrive") => Self::Drive,
            Some("onedrive") => Self::Onedrive,
            Some("dropbox") => Self::Dropbox,
            _ => anyhow::bail!(
                "This provider uses an unsupported storage scheme; its saved settings were preserved."
            ),
        })
    }
    pub fn fields(self) -> Vec<Field> {
        use Field as F;
        match self {
            Self::Repository => vec![
                F::config("repository_url", "Repository URL", ""),
                F::config("branch", "Branch", "main"),
                F::config("path", "File Path", "nauterm-sync.enc"),
                F::credential("token", "Access Token", true),
            ],
            Self::Gist => vec![
                F::config("gist_id", "Gist ID", ""),
                Field {
                    readonly: true,
                    ..F::config("filename", "Filename", "nauterm-sync.enc")
                },
            ],
            Self::LegacyS3 | Self::S3 | Self::Minio => vec![
                F::config("endpoint", "Endpoint", ""),
                F::config("region", "Region", "auto"),
                F::config("bucket", "Bucket", ""),
                F::credential("access_key_id", "Access Key ID", false),
                F::credential("secret_access_key", "Secret Access Key", true),
                F::config("prefix", "Folder", ""),
                F::config("filename", "Filename", "nauterm-sync.enc"),
            ],
            Self::Webdav => vec![
                F::config("endpoint", "Server URL", ""),
                F::credential("username", "Username", false),
                F::credential("password", "Password", true),
                F::config("root", "Remote Folder", "/"),
                F::config("filename", "Filename", "nauterm-sync.enc"),
            ],
            Self::Azure => vec![
                F::config("endpoint", "Endpoint", ""),
                F::config("container", "Container", ""),
                F::credential("account_name", "Account Name", false),
                F::credential("account_key", "Account Key", true),
                F::config("prefix", "Folder", ""),
                F::config("filename", "Filename", "nauterm-sync.enc"),
            ],
            Self::Gcs => vec![
                F::config("bucket", "Bucket", ""),
                F::config("prefix", "Folder", ""),
                F::config("filename", "Filename", "nauterm-sync.enc"),
            ],
            Self::Drive | Self::Onedrive | Self::Dropbox => vec![
                F::config("root", "Remote Folder", "/"),
                F::config("filename", "Filename", "nauterm-sync.enc"),
            ],
        }
    }
}
pub struct Draft {
    pub kind: Kind,
    pub expected: Option<SyncProviderRecord>,
    pub id: String,
    pub name: String,
    provider: String,
    config: Value,
}
impl Draft {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            expected: None,
            id: kind
                .fixed_id()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("provider-{}", uuid::Uuid::new_v4())),
            name: kind.label().into(),
            provider: kind.vendor().into(),
            config: if kind.fixed_id().is_some() {
                json!({})
            } else {
                json!({"scheme":kind.scheme(),"config":{}})
            },
        }
    }
    pub fn stored(record: SyncProviderRecord) -> Result<Self> {
        let kind = Kind::of(&record)?;
        Ok(Self {
            kind,
            id: record.uuid.clone(),
            name: record.name.clone(),
            provider: record.provider.clone(),
            config: record.config.clone(),
            expected: Some(record),
        })
    }
    pub fn has_credentials(&self) -> bool {
        self.expected
            .as_ref()
            .is_some_and(|record| record.has_credentials)
    }
    pub fn value(&self, field: Field) -> String {
        if field.credential {
            return String::new();
        }
        let config = if self.kind.fixed_id().is_some() {
            &self.config
        } else {
            &self.config["config"]
        };
        config[field.key].as_str().unwrap_or(field.default).into()
    }
    pub fn update(
        &self,
        name: &str,
        values: Vec<(Field, Zeroizing<String>)>,
    ) -> Result<SyncProviderUpdate> {
        let mut config = self.config.clone();
        let target = if self.kind.fixed_id().is_some() {
            &mut config
        } else {
            &mut config["config"]
        };
        let target = target
            .as_object_mut()
            .context("The saved provider configuration cannot be edited by this version.")?;
        let mut credentials = SyncProviderCredentials::default();
        let mut credential_fields = 0;
        for (field, value) in values {
            if field.credential {
                credential_fields += 1;
                if !value.is_empty() {
                    credentials.values.insert(field.key.into(), value);
                }
            } else {
                let value = value.trim();
                let normalized = if value.is_empty()
                    && matches!(
                        field.key,
                        "region" | "filename" | "branch" | "path" | "root"
                    ) {
                    field.default
                } else {
                    value
                };
                target.insert(field.key.into(), normalized.into());
            }
        }
        ensure!(
            credentials.values.is_empty() || credentials.values.len() == credential_fields,
            "Enter both credential fields, or leave them blank to keep the saved credentials."
        );
        if !self.has_credentials()
            && self.kind.fixed_id() != Some("github_repository")
            && self.kind.fixed_id() != Some("github_gist")
            && credential_fields > 0
        {
            ensure!(
                !credentials.values.is_empty(),
                "Enter the connection credentials."
            );
        }
        let update = SyncProviderUpdate {
            uuid: self.id.clone(),
            provider: self.provider.clone(),
            name: if self.kind.fixed_id().is_some() {
                self.name.clone()
            } else {
                name.trim().into()
            },
            config,
            credentials: (!credentials.values.is_empty()).then_some(credentials),
        };
        update
            .validate()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(update)
    }
    pub fn update_authorized(
        &self,
        name: &str,
        values: Vec<(Field, Zeroizing<String>)>,
        authorized: Option<SyncProviderCredentials>,
    ) -> Result<SyncProviderUpdate> {
        let mut update = self.update(name, values)?;
        if let Some(credentials) = authorized {
            ensure!(
                self.kind.oauth().is_some(),
                "This connection does not use browser authorization."
            );
            update.credentials = Some(credentials);
        }
        ensure!(
            self.kind.oauth().is_none() || self.has_credentials() || update.credentials.is_some(),
            "Sign in before saving this provider."
        );
        update
            .validate()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(update)
    }
}
impl Service {
    pub async fn provider(&self, id: String, fallback: Option<Kind>) -> Result<Draft> {
        self.vault
            .dispatch(move |db| match db.inner.get_sync_provider_record(&id)? {
                Some(record) => Draft::stored(record),
                None if fallback.is_some_and(|kind| kind.fixed_id() == Some(id.as_str())) => {
                    Ok(Draft::new(fallback.unwrap()))
                }
                None => anyhow::bail!("This provider was removed. Refresh the sync page."),
            })
            .await
    }
    pub async fn save_provider(
        &self,
        expected: Option<SyncProviderRecord>,
        update: SyncProviderUpdate,
    ) -> Result<Overview> {
        self.vault
            .dispatch(move |db| {
                db.inner
                    .save_sync_provider_checked(&update, expected.as_ref())
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                overview(&mut db.inner)
            })
            .await
    }
    pub async fn remove_provider(
        &self,
        expected: SyncProviderRecord,
        disconnect: bool,
    ) -> Result<Overview> {
        self.vault
            .dispatch(move |db| {
                db.inner
                    .remove_sync_provider_checked(&expected, disconnect)
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                overview(&mut db.inner)
            })
            .await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_providers_keep_the_shared_sync_object_name() {
        for kind in [
            Kind::Repository,
            Kind::Gist,
            Kind::LegacyS3,
            Kind::S3,
            Kind::Minio,
            Kind::Webdav,
            Kind::Azure,
            Kind::Gcs,
            Kind::Drive,
            Kind::Onedrive,
            Kind::Dropbox,
        ] {
            let draft = Draft::new(kind);
            let field = kind
                .fields()
                .into_iter()
                .find(|field| matches!(field.key, "path" | "filename"))
                .unwrap();
            assert_eq!(draft.value(field), "nauterm-sync.enc");
        }
    }
    #[test]
    fn oauth_connections_require_staged_credentials_and_keep_them_out_of_public_config()
    -> Result<()> {
        for kind in [
            Kind::Gist,
            Kind::Gcs,
            Kind::Drive,
            Kind::Onedrive,
            Kind::Dropbox,
        ] {
            let draft = Draft::new(kind);
            let values = || {
                kind.fields()
                    .into_iter()
                    .map(|field| {
                        let value = if field.key == "bucket" {
                            "synthetic-bucket"
                        } else {
                            field.default
                        };
                        (field, Zeroizing::new(value.into()))
                    })
                    .collect()
            };
            assert!(draft.update_authorized("Test", values(), None).is_err());
            let mut credentials = SyncProviderCredentials::default();
            if kind == Kind::Gist {
                credentials
                    .values
                    .insert("token".into(), Zeroizing::new("synthetic-token".into()));
            } else {
                credentials
                    .values
                    .insert("client_id".into(), Zeroizing::new("registered-id".into()));
                credentials.values.insert(
                    "refresh_token".into(),
                    Zeroizing::new("synthetic-refresh-token".into()),
                );
            }
            let update = draft.update_authorized("Test", values(), Some(credentials.clone()))?;
            assert!(update.credentials.is_some());
            assert!(!update.config.to_string().contains("synthetic-token"));
            assert!(
                !update
                    .config
                    .to_string()
                    .contains("synthetic-refresh-token")
            );
            assert!(
                !credentials.values.is_empty(),
                "the editor retains its staged credentials until save succeeds"
            );
        }
        Ok(())
    }
    #[test]
    fn provider_forms_match_transport_fields_and_keep_credentials_write_only() -> Result<()> {
        for kind in Kind::BUILT_IN.into_iter().chain(Kind::CATALOG) {
            let draft = Draft::new(kind);
            let values = kind
                .fields()
                .into_iter()
                .map(|field| {
                    assert!(draft.value(field).is_empty() || !field.credential);
                    let value = match field.key {
                        "repository_url" => "https://github.com/example/synthetic",
                        "endpoint" => "https://example.invalid",
                        "bucket" | "container" => "synthetic",
                        "prefix" => "backups",
                        _ if field.credential => "synthetic-credential",
                        _ => field.default,
                    };
                    (field, Zeroizing::new(value.into()))
                })
                .collect();
            let update = draft.update("Test", values)?;
            if kind == Kind::Azure {
                assert_eq!(update.config["config"]["prefix"], "backups");
                assert!(update.config["config"].get("root").is_none());
            }
            assert_eq!(
                update.credentials.is_some(),
                kind.fields().iter().any(|field| field.credential)
            );
        }
        Ok(())
    }
    #[test]
    fn provider_drafts_keep_unknown_config_and_do_not_trim_passwords() -> Result<()> {
        let mut draft = Draft::new(Kind::Webdav);
        draft.config =
            json!({"scheme":"webdav","config":{"future":"retained"},"future_outer":true});
        let values = draft
            .kind
            .fields()
            .into_iter()
            .map(|field| {
                let value = match field.key {
                    "endpoint" => "https://example.invalid/dav",
                    "username" => "user",
                    "password" => " exact secret ",
                    _ => field.default,
                };
                (field, Zeroizing::new(value.into()))
            })
            .collect();
        let update = draft.update("Test", values)?;
        assert_eq!(update.config["future_outer"], true);
        assert_eq!(update.config["config"]["future"], "retained");
        assert_eq!(
            update.credentials.unwrap().values["password"].as_str(),
            " exact secret "
        );
        Ok(())
    }
    #[test]
    fn partial_credential_pairs_do_not_turn_into_a_save() {
        let draft = Draft::new(Kind::Webdav);
        let values = draft
            .kind
            .fields()
            .into_iter()
            .map(|field| {
                let value = match field.key {
                    "endpoint" => "https://example.invalid/dav",
                    "username" => "user",
                    _ => field.default,
                };
                (field, Zeroizing::new(value.into()))
            })
            .collect();
        assert!(draft.update("Test", values).is_err());
    }
}
