//! Proxy management over the isolated native encrypted proxy repository.
use crate::{model::Proxy as LegacyProxy, vault::Vault};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use zeroize::{Zeroize, Zeroizing};

mod deletion;
mod view;
pub(crate) use view::RelatedEvent;
pub use view::{Changed, MigrationReady, Proxies, ReferencesDeleted};
pub type Deletions = std::collections::BTreeMap<i64, String>;
#[derive(Clone)]
pub struct Catalog {
    pub entries: Vec<Summary>,
    pub deletions: Deletions,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Http,
    #[default]
    Socks5,
}
impl Kind {
    pub const ALL: [Self; 2] = [Self::Http, Self::Socks5];
    pub fn label(self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Socks5 => "SOCKS5",
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Record {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: Kind,
    pub host: String,
    pub port: i64,
    pub identity_id: Option<i64>,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
}
impl Drop for Record {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.name.trim().is_empty(), "Name is required.");
        ensure!(
            !self.host.trim().is_empty()
                && !self.host.chars().any(char::is_whitespace)
                && !self.host.contains('\0'),
            "Enter a proxy address without whitespace."
        );
        ensure!(
            (1..=65535).contains(&self.port),
            "Port must be between 1 and 65535."
        );
        ensure!(
            self.id.is_none_or(|id| id > 0) && self.identity_id.is_none_or(|id| id > 0),
            "Invalid proxy or identity reference."
        );
        Ok(())
    }
    fn summary(&self) -> Result<Summary> {
        Ok(Summary {
            id: self
                .id
                .filter(|id| *id > 0)
                .context("Stored proxy has no ID")?,
            name: self.name.clone(),
            kind: self.kind,
            host: self.host.clone(),
            port: u16::try_from(self.port)
                .ok()
                .filter(|port| *port > 0)
                .context("Stored proxy has an invalid port")?,
            identity_id: self.identity_id,
            username: self.username.clone().unwrap_or_default(),
            created_at: self.created_at,
        })
    }
}
#[derive(Clone)]
pub struct Summary {
    pub id: i64,
    pub name: String,
    pub kind: Kind,
    pub host: String,
    pub port: u16,
    pub identity_id: Option<i64>,
    pub username: String,
    pub created_at: Option<i64>,
}
impl Summary {
    pub fn matches(&self, query: &str, identity: &str) -> bool {
        let text = format!(
            "{} {} {} {} {} {}",
            self.name,
            self.kind.label(),
            self.host,
            self.port,
            self.username,
            identity
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|token| text.contains(&token.to_lowercase()))
    }
}

/// A resolved, transient connection snapshot. Never Serialize/Debug this type;
/// only `wire` can encode it into a short-lived, zeroizing native argument.
#[derive(Clone)]
pub struct Transport {
    kind: Kind,
    host: String,
    port: u16,
    username: Option<String>,
    password: Zeroizing<String>,
}
impl Transport {
    fn resolve(record: &Record, identity: Option<&crate::vault::IdentityRecord>) -> Result<Self> {
        record.validate()?;
        ensure!(
            record
                .identity_id
                .is_none_or(|id| identity.and_then(|identity| identity.id) == Some(id)),
            "The proxy's saved identity is unavailable."
        );
        let nonempty = |value: &Option<String>| {
            value
                .as_ref()
                .filter(|value| !value.trim().is_empty())
                .cloned()
        };
        let username = nonempty(&record.username)
            .or_else(|| identity.and_then(|identity| nonempty(&identity.username)));
        let secret =
            |value: &Option<String>| value.as_ref().filter(|value| !value.is_empty()).cloned();
        let password = secret(&record.password)
            .or_else(|| identity.and_then(|identity| secret(&identity.password)))
            .unwrap_or_default();
        Ok(Self {
            kind: record.kind,
            host: record.host.trim().to_owned(),
            port: record.port as u16,
            username,
            password: Zeroizing::new(password),
        })
    }
    pub(crate) fn native(&self) -> nauterm_core::ssh::SshProxyConfig {
        nauterm_core::ssh::SshProxyConfig {
            proxy_type: match self.kind {
                Kind::Http => "http",
                Kind::Socks5 => "socks5",
            }
            .to_owned(),
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            password: (!self.password.is_empty()).then(|| self.password.to_string()),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request<'a> {
    ListProxies,
    GetProxy { id: i64 },
    SaveProxy { proxy: &'a Record },
    DeleteProxy { id: i64 },
    GetAppMeta { key: &'a str },
    SetAppMeta { key: &'a str, value: &'a str },
    GetIdentity { id: i64 },
}
const IMPORT_RECEIPTS: &str = "rs.proxies.legacy-import.v1";

fn legacy_uuid(item: &LegacyProxy) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(b"Nauterm RS proxy import v1\0");
    digest.update(item.id.to_be_bytes());
    for text in [&item.name, &item.address] {
        digest.update((text.len() as u64).to_be_bytes());
        digest.update(text.as_bytes());
    }
    digest.update(item.port.to_be_bytes());
    let digest = digest.finalize();
    let mut bytes: [u8; 16] = digest[..16].try_into().unwrap();
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes.map(|byte| format!("{byte:02x}")).concat();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}
fn fetch(db: &mut crate::native::Database, id: i64) -> Result<Record> {
    db.call::<Option<Record>>(&Request::GetProxy { id })?
        .context("The proxy no longer exists.")
}
fn inventory(db: &mut crate::native::Database) -> Result<Vec<Summary>> {
    db.call::<Vec<Record>>(&Request::ListProxies)?
        .iter()
        .map(Record::summary)
        .collect()
}
impl Vault {
    pub async fn apply_connection(
        &self,
        credential: Option<crate::model::CredentialRef>,
        proxy: Option<i64>,
        setup: crate::host_session::Setup,
        options: crate::native::SshOptions,
    ) -> Result<crate::native::SshOptions> {
        self.apply_remote_connection(credential, proxy, setup, options, false)
            .await
    }
    pub async fn apply_remote_connection(
        &self,
        credential: Option<crate::model::CredentialRef>,
        proxy: Option<i64>,
        setup: crate::host_session::Setup,
        mut options: crate::native::SshOptions,
        mosh: bool,
    ) -> Result<crate::native::SshOptions> {
        let saved_profile = setup.profile_source.is_some();
        let source = if let Some(reference) = setup
            .profile_source
            .as_deref()
            .and_then(crate::resource_reference::Reference::parse)
        {
            Some(
                self.dispatch(move |db| {
                    let source = reference.load(&db.inner)?;
                    ensure!(
                        if mosh {
                            source.mosh_enabled()
                        } else {
                            source.ssh_enabled()
                        },
                        "The requested protocol is disabled for this saved profile."
                    );
                    Ok(source.auth_fallback())
                })
                .await?,
            )
        } else {
            None
        };
        let fallback = matches!(credential, Some(crate::model::CredentialRef::Identity(_)))
            .then_some(source)
            .flatten();
        if options.username.trim().is_empty()
            && let Some((Some(username), _)) = &fallback
        {
            options.username = username.clone();
        }
        if !matches!(credential, Some(crate::model::CredentialRef::Identity(_)))
            && let Some(id) = setup.password_id
        {
            options.password = self.host_password(id).await?;
        }
        match credential {
            Some(crate::model::CredentialRef::Identity(id)) => {
                options = self
                    .apply_identity_with_fallback(id, options, fallback.and_then(|(_, key)| key))
                    .await?;
            }
            Some(reference) => options = self.apply_reference(reference, options).await?,
            None => {}
        }
        if saved_profile && options.username.trim().is_empty() {
            options.username = "user".into();
        }
        if let Some(id) = proxy {
            options.proxy = Some(
                self.dispatch(move |db| {
                    let record = fetch(db, id)?;
                    let identity = record
                        .identity_id
                        .map(|id| {
                            db.call::<Option<crate::vault::IdentityRecord>>(
                                &Request::GetIdentity { id },
                            )?
                            .context("The proxy's saved identity no longer exists.")
                        })
                        .transpose()?;
                    Transport::resolve(&record, identity.as_ref())
                })
                .await?,
            );
        } else {
            options.proxy = None;
        }
        Ok(options)
    }
    pub async fn proxies(&self, legacy: Vec<LegacyProxy>) -> Result<Catalog> {
        self.dispatch(move |db| {
            if !legacy.is_empty() {
                let stored: Option<String> = db.call(&Request::GetAppMeta {
                    key: IMPORT_RECEIPTS,
                })?;
                let mut receipts: BTreeSet<String> = stored
                    .map(|text| serde_json::from_str(&text))
                    .transpose()
                    .context("Invalid proxy import receipts; existing data was preserved")?
                    .unwrap_or_default();
                let stored: Vec<Record> = db.call(&Request::ListProxies)?;
                let mut uuids: BTreeSet<_> = stored
                    .iter()
                    .filter_map(|record| record.uuid.clone())
                    .collect();
                for item in legacy {
                    let uuid = legacy_uuid(&item);
                    if receipts.contains(&uuid) {
                        continue;
                    }
                    if !uuids.contains(&uuid) {
                        let record = Record {
                            uuid: Some(uuid.clone()),
                            name: if item.name.trim().is_empty() {
                                item.address.clone()
                            } else {
                                item.name
                            },
                            host: item.address,
                            port: i64::from(item.port),
                            id: None,
                            kind: Kind::Socks5,
                            identity_id: None,
                            username: None,
                            password: None,
                            created_at: None,
                        };
                        record.validate()?;
                        let _: i64 = db.call(&Request::SaveProxy { proxy: &record })?;
                        uuids.insert(uuid.clone());
                    }
                    receipts.insert(uuid);
                }
                let encoded = Zeroizing::new(serde_json::to_string(&receipts)?);
                let _: () = db.call(&Request::SetAppMeta {
                    key: IMPORT_RECEIPTS,
                    value: &encoded,
                })?;
            }
            Ok(Catalog {
                entries: inventory(db)?,
                deletions: deletion::confirmed(db)?,
            })
        })
        .await
    }
    pub async fn proxy(&self, id: i64) -> Result<Record> {
        self.dispatch(move |db| fetch(db, id)).await
    }
    pub async fn save_proxy(&self, mut record: Record) -> Result<i64> {
        record.validate()?;
        self.dispatch(move |db| {
            if let Some(id) = record.id {
                let _ = fetch(db, id)?;
            }
            if let Some(id) = record.identity_id {
                ensure!(
                    db.call::<Option<crate::vault::IdentityRecord>>(&Request::GetIdentity { id })?
                        .is_some(),
                    "The selected identity no longer exists."
                );
                record.username = None;
                record.password.zeroize();
                record.password = None;
            }
            db.call(&Request::SaveProxy { proxy: &record })
        })
        .await
    }
    pub async fn delete_proxies(&self, ids: Vec<i64>) -> Result<()> {
        self.dispatch(move |db| {
            let ids = deletion::prepare(db, ids)?;
            for id in ids {
                ensure!(
                    db.call::<usize>(&Request::DeleteProxy { id })? == 1,
                    "The proxy no longer exists; refresh the list."
                );
            }
            Ok(())
        })
        .await
    }
    pub async fn acknowledge_proxy_deletions(&self, deleted: Deletions) -> Result<()> {
        self.dispatch(move |db| deletion::acknowledge(db, &deleted))
            .await
    }
    pub async fn duplicate_proxies(&self, ids: Vec<i64>) -> Result<Vec<i64>> {
        self.dispatch(move |db| {
            let sources = ids.into_iter().collect::<BTreeSet<_>>().into_iter().map(|id| fetch(db, id)).collect::<Result<Vec<_>>>()?;
            let mut created = vec![];
            for mut record in sources {
                record.id = None; record.uuid = None; record.created_at = None; record.name.push_str(" copy");
                match db.call::<i64>(&Request::SaveProxy { proxy: &record }) {
                    Ok(id) => created.push(id),
                    Err(error) => {
                        let failed = created.into_iter().filter(|id| db.call::<usize>(&Request::DeleteProxy { id: *id }).is_err()).count();
                        ensure!(failed == 0, "{error}. Some new proxy copies could not be rolled back; refresh the list before retrying.");
                        return Err(error);
                    }
                }
            }
            Ok(created)
        }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proxy_transport_keeps_authentication_and_strict_clones_without_entering_metadata()
    -> Result<()> {
        let record: Record = serde_json::from_value(serde_json::json!({
            "id": 7, "name": "Fixture", "type": "socks5", "host": "proxy.invalid", "port": 1080,
            "identity_id": 17, "username": null, "password": null
        }))?;
        let identity: crate::vault::IdentityRecord = serde_json::from_value(serde_json::json!({
            "id": 17, "name": "Synthetic", "username": "proxy-user", "password": "synthetic-proxy-secret", "key_id": null
        }))?;
        assert!(Transport::resolve(&record, None).is_err());
        let transport = Transport::resolve(&record, Some(&identity))?;
        let mut options = crate::native::SshOptions::new(
            "destination.invalid".into(),
            2222,
            "ssh-user".into(),
            std::path::PathBuf::from("/synthetic/rs/known_hosts"),
        );
        options.trust = crate::native::HostTrust::AcceptOnce;
        options.proxy = Some(transport);
        let strict = options.for_new_connection();
        assert!(strict.trust == crate::native::HostTrust::Strict);
        assert!(strict.password.is_empty());
        assert_eq!(strict.username, "ssh-user");
        let args = crate::native::SshArguments::new(&strict)?;
        let proxy = args.native_proxy().context("Missing proxy")?;
        assert_eq!(proxy.proxy_type, "socks5");
        assert_eq!(proxy.host, "proxy.invalid");
        assert_eq!(proxy.port, 1080);
        assert_eq!(proxy.username.as_deref(), Some("proxy-user"));
        assert_eq!(proxy.password.as_deref(), Some("synthetic-proxy-secret"));
        let mut manual = record.clone();
        manual.identity_id = None;
        manual.username = Some("manual-proxy-user".into());
        manual.password = Some("   ".into());
        let proxy = Transport::resolve(&manual, None)?.native();
        assert_eq!(
            proxy.password.as_deref(),
            Some("   "),
            "Proxy passwords must not be trimmed"
        );
        options.proxy = None;
        assert!(
            crate::native::SshArguments::new(&options)?
                .native_proxy()
                .is_none()
        );
        Ok(())
    }
    #[test]
    fn proxy_model_uses_native_schema_and_public_summaries_exclude_secrets() -> Result<()> {
        let record: Record = serde_json::from_value(
            serde_json::json!({"id": 7, "uuid": "fixture", "name": "Fixture", "type": "http", "host": "::1", "port": 8080, "identity_id": null, "username": "user", "password": "synthetic-secret", "created_at": 123}),
        )?;
        record.validate()?;
        let summary = record.summary()?;
        assert_eq!(summary.kind, Kind::Http);
        assert!(summary.matches("FIXTURE http 8080", ""));
        assert!(!summary.matches("synthetic-secret", ""));
        let json = serde_json::to_value(Request::SaveProxy { proxy: &record })?;
        assert_eq!(json["op"], "save_proxy");
        assert_eq!(json["proxy"]["type"], "http");
        let mut legacy = LegacyProxy {
            id: 7,
            name: "Legacy".into(),
            address: "test.invalid".into(),
            port: 1080,
        };
        let original = legacy_uuid(&legacy);
        assert_eq!(original, legacy_uuid(&legacy));
        legacy.address = "changed.invalid".into();
        assert_ne!(original, legacy_uuid(&legacy));
        legacy.id = u64::MAX;
        assert_eq!(legacy_uuid(&legacy).len(), 36);
        let mut invalid = record;
        invalid.port = 0;
        assert!(invalid.validate().is_err());
        Ok(())
    }
}
