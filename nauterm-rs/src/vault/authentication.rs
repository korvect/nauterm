//! Connection authentication resolves secrets only when submitted. Optional
//! persistence patches the original Host/Identity inside one native transaction.
use super::{KeySummary, Vault, key_algorithm};
use crate::{
    model::{CredentialRef, Host},
    native::SshOptions,
    resource_reference::{Kind, Profile, Protocol, Reference},
};
use anyhow::{Context, Result, ensure};
use nauterm_core::{HostEntry, IdentityEntry, KeyEntry, NautermDatabase};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Binding {
    reference: Reference,
    address: String,
    port: u16,
    credential: Option<CredentialRef>,
}
impl Binding {
    pub fn matches(&self, options: &SshOptions) -> bool {
        self.address == options.host && self.port == options.port
    }
    pub fn from_host(host: &Host) -> Option<Self> {
        let uuid = host
            .uuid
            .clone()
            .filter(|uuid| crate::resource_reference::valid_uuid(uuid))?;
        host.supports_ssh().then(|| Self {
            reference: Reference {
                kind: Kind::Host,
                id: host.id as i64,
                uuid,
                protocol: Protocol::Ssh,
            },
            address: host.address.clone(),
            port: host.port,
            credential: host.credential,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Revision {
    id: i64,
    uuid: String,
    version: Option<i64>,
}
impl Revision {
    fn new(id: Option<i64>, uuid: Option<&str>, version: Option<i64>) -> Result<Self> {
        let uuid = uuid
            .filter(|uuid| crate::resource_reference::valid_uuid(uuid))
            .context("The saved resource has no valid identity. Reload it and try again.")?;
        Ok(Self {
            id: id
                .filter(|id| *id > 0)
                .context("The saved resource has no ID.")?,
            uuid: uuid.into(),
            version,
        })
    }
    fn check(&self, id: Option<i64>, uuid: Option<&str>, version: Option<i64>) -> Result<()> {
        ensure!(
            id == Some(self.id) && uuid == Some(self.uuid.as_str()) && version == self.version,
            "The saved resource changed. Reload the authentication options and try again."
        );
        Ok(())
    }
}
#[derive(Clone)]
pub(crate) struct Key {
    pub summary: KeySummary,
    revision: Revision,
}
#[derive(Clone)]
pub(crate) struct SaveTarget {
    binding: Binding,
    host: Revision,
    identity: Option<Revision>,
}
pub(crate) struct Catalog {
    pub keys: Option<Vec<Key>>,
    pub selected_key: Option<i64>,
    pub save_target: Option<SaveTarget>,
    pub save_error: Option<String>,
}
pub(crate) enum Credential {
    Password(Zeroizing<String>),
    Key {
        key: Key,
        passphrase: Option<Zeroizing<String>>,
    },
}
struct SecretKey(KeyEntry);
impl Drop for SecretKey {
    fn drop(&mut self) {
        self.0.private_key.zeroize();
        self.0.passphrase.zeroize();
    }
}
struct SecretIdentity(IdentityEntry);
impl Drop for SecretIdentity {
    fn drop(&mut self) {
        self.0.password.zeroize();
    }
}
fn key_choice(key: &KeyEntry) -> Result<Key> {
    let revision = Revision::new(key.id, key.uuid.as_deref(), key.version)?;
    Ok(Key {
        summary: KeySummary {
            id: revision.id,
            name: key.name.clone(),
            public_key: key.public_key.clone(),
            certificate: key
                .certificate
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            algorithm: key_algorithm(key.public_key.as_deref(), key.certificate.as_deref()),
        },
        revision,
    })
}
fn load_host(database: &NautermDatabase, binding: &Binding) -> Result<Profile> {
    let profile = binding.reference.load(database)?;
    let Profile::Host(host) = &profile else {
        anyhow::bail!("Authentication requires a saved Host.");
    };
    let identity = match binding.credential {
        Some(CredentialRef::Identity(id)) => Some(id),
        _ => None,
    };
    ensure!(
        host.host.as_deref() == Some(binding.address.as_str())
            && host.port.unwrap_or(22) == i64::from(binding.port)
            && host.identity_id == identity,
        "The saved Host connection changed. Reopen it before saving authentication."
    );
    Ok(profile)
}
fn save_target(database: &NautermDatabase, binding: Binding) -> Result<(SaveTarget, Option<i64>)> {
    let profile = load_host(database, &binding)?;
    let Profile::Host(host) = &profile else {
        unreachable!()
    };
    let host_revision = Revision::new(host.id, host.uuid.as_deref(), host.version)?;
    let (identity, selected_key) = if let Some(id) = host.identity_id {
        let row = SecretIdentity(
            database
                .get_identity(id)?
                .context("The saved Identity was deleted.")?,
        );
        let revision = Revision::new(row.0.id, row.0.uuid.as_deref(), row.0.version)?;
        ensure!(
            host.identity_uuid.as_deref() == Some(revision.uuid.as_str()),
            "The saved Identity was replaced."
        );
        (Some(revision), row.0.key_id.or(host.key_id))
    } else {
        (None, host.key_id)
    };
    Ok((
        SaveTarget {
            binding,
            host: host_revision,
            identity,
        },
        selected_key,
    ))
}
pub(crate) fn apply_password(options: &mut SshOptions, password: Zeroizing<String>) -> Result<()> {
    ensure!(!password.is_empty(), "Enter a password to continue.");
    options.password = password;
    options.private_key.zeroize();
    options.certificate.zeroize();
    options.passphrase.zeroize();
    options.fido2_pin.zeroize();
    Ok(())
}
fn patch_host(host: &mut HostEntry, options: &SshOptions, key: Option<&Revision>) {
    host.username = Some(options.username.clone());
    host.password.zeroize();
    host.password = (!options.password.is_empty()).then(|| options.password.to_string());
    host.key_id = key.map(|key| key.id);
    host.key_uuid = key.map(|key| key.uuid.clone());
}
fn patch_identity(identity: &mut IdentityEntry, options: &SshOptions, key: Option<&Revision>) {
    if identity
        .username
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        identity.username = Some(options.username.clone());
    }
    identity.password.zeroize();
    identity.password = (!options.password.is_empty()).then(|| options.password.to_string());
    identity.key_id = key.map(|key| key.id);
    identity.key_uuid = key.map(|key| key.uuid.clone());
}
fn prepare_connection(
    database: &mut NautermDatabase,
    mut options: SshOptions,
    credential: Credential,
    save: Option<SaveTarget>,
) -> Result<SshOptions> {
    ensure!(
        !options.username.trim().is_empty() && !options.username.chars().any(char::is_control),
        "Enter a valid username."
    );
    let key_revision = match credential {
        Credential::Password(password) => {
            apply_password(&mut options, password)?;
            None
        }
        Credential::Key { key, passphrase } => {
            let mut row = SecretKey(
                database
                    .get_key(key.revision.id)?
                    .context("The selected key was deleted.")?,
            );
            key.revision
                .check(row.0.id, row.0.uuid.as_deref(), row.0.version)?;
            options.private_key = Zeroizing::new(
                row.0
                    .private_key
                    .take()
                    .filter(|value| !value.trim().is_empty())
                    .context("The selected key has no private key.")?,
            );
            options.certificate = Zeroizing::new(row.0.certificate.take().unwrap_or_default());
            options.passphrase = passphrase
                .unwrap_or_else(|| Zeroizing::new(row.0.passphrase.take().unwrap_or_default()));
            options.password.zeroize();
            options.fido2_pin.zeroize();
            Some(key.revision)
        }
    };
    if let Some(target) = save {
        let mut profile = load_host(database, &target.binding)?;
        let Profile::Host(host) = &mut profile else {
            unreachable!()
        };
        target
            .host
            .check(host.id, host.uuid.as_deref(), host.version)?;
        if let Some(identity) = target.identity {
            let mut row = SecretIdentity(
                database
                    .get_identity(identity.id)?
                    .context("The saved Identity was deleted.")?,
            );
            identity.check(row.0.id, row.0.uuid.as_deref(), row.0.version)?;
            ensure!(
                host.identity_uuid.as_deref() == Some(identity.uuid.as_str()),
                "The saved Identity was replaced."
            );
            patch_identity(&mut row.0, &options, key_revision.as_ref());
            database.save_identity(&row.0)?;
        } else {
            patch_host(host, &options, key_revision.as_ref());
            database.save_host(host)?;
        }
    }
    Ok(options)
}

impl Vault {
    pub(crate) async fn authentication_catalog(
        &self,
        binding: Option<Binding>,
        keys: bool,
    ) -> Result<Catalog> {
        self.dispatch(move |db| {
            db.inner.atomic(|database| -> Result<Catalog> {
                let keys = if keys {
                    let rows = database.list_authentication_keys()?;
                    Some(rows.iter().map(key_choice).collect::<Result<Vec<_>>>()?)
                } else {
                    None
                };
                let (save_target, selected_key, save_error) = match binding {
                    Some(binding) => match save_target(database, binding) {
                        Ok((target, selected)) => (Some(target), selected, None),
                        Err(error) => (None, None, Some(error.to_string())),
                    },
                    None => (None, None, None),
                };
                Ok(Catalog {
                    keys,
                    selected_key,
                    save_target,
                    save_error,
                })
            })
        })
        .await
    }
    pub(crate) async fn authenticate_connection(
        &self,
        options: SshOptions,
        credential: Credential,
        save: Option<SaveTarget>,
    ) -> Result<SshOptions> {
        self.dispatch(move |db| {
            db.inner
                .atomic(|database| prepare_connection(database, options, credential, save))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const UUID: &str = "12345678-1234-1234-1234-123456789abc";
    const OTHER: &str = "22345678-1234-1234-1234-123456789abc";
    fn options() -> SshOptions {
        SshOptions::new(
            "example.invalid".into(),
            22,
            "admin".into(),
            "unused-known-hosts".into(),
        )
    }
    #[test]
    fn saving_a_certificate_preserves_the_exact_continue_authentication_request() -> Result<()> {
        let mut database = NautermDatabase::open_in_memory()?;
        let key: KeyEntry = serde_json::from_value(json!({
            "name":"Certificate fixture", "private_key":"synthetic private key",
            "public_key":"ssh-ed25519 synthetic-public", "passphrase":"synthetic passphrase",
            "certificate":"ssh-ed25519-cert-v01@openssh.com synthetic-certificate"
        }))?;
        let key_id = database.save_key(&key)?;
        let host: HostEntry = serde_json::from_value(json!({
            "name":"Synthetic host", "type":"remote", "identity_id":null,
            "host":"example.invalid", "port":22, "username":"admin"
        }))?;
        let host_id = database.save_host(&host)?;
        let host = database.get_host(host_id)?.unwrap();
        let binding = Binding {
            reference: Reference {
                kind: Kind::Host,
                id: host_id,
                uuid: host.uuid.clone().unwrap(),
                protocol: Protocol::Ssh,
            },
            address: "example.invalid".into(),
            port: 22,
            credential: None,
        };
        let (target, _) = save_target(&database, binding)?;
        let key = key_choice(&database.get_key(key_id)?.unwrap())?;
        let credential = || Credential::Key {
            key: key.clone(),
            passphrase: None,
        };
        let continued = database
            .atomic(|database| prepare_connection(database, options(), credential(), None))?;
        let saved = database.atomic(|database| {
            prepare_connection(database, options(), credential(), Some(target))
        })?;
        assert_eq!(continued.username, saved.username);
        assert_eq!(continued.private_key, saved.private_key);
        assert_eq!(continued.certificate, saved.certificate);
        assert_eq!(continued.passphrase, saved.passphrase);
        assert_eq!(continued.password, saved.password);
        assert_eq!(
            crate::native::SshArguments::new(&continued)?.auth(),
            crate::native::SshArguments::new(&saved)?.auth()
        );
        assert_eq!(database.get_host(host_id)?.unwrap().key_id, Some(key_id));
        assert_eq!(
            database.get_key(key_id)?.unwrap().certificate.as_deref(),
            Some(saved.certificate.as_str())
        );
        let original = database.get_host(host_id)?.unwrap();
        let rolled_back: Result<()> = database.atomic(|database| {
            let mut edit = original.clone();
            edit.username = Some("rolled-back-user".into());
            database.save_host(&edit)?;
            anyhow::bail!("Synthetic failure after saving Host tags")
        });
        assert!(rolled_back.is_err());
        assert_eq!(database.get_host(host_id)?.unwrap(), original);
        Ok(())
    }
    #[test]
    fn resource_revision_rejects_reused_ids_and_concurrent_edits() {
        let revision = Revision::new(Some(7), Some(UUID), Some(3)).unwrap();
        assert!(revision.check(Some(7), Some(UUID), Some(3)).is_ok());
        assert!(revision.check(Some(7), Some(OTHER), Some(3)).is_err());
        assert!(revision.check(Some(7), Some(UUID), Some(4)).is_err());
        assert!(revision.check(Some(8), Some(UUID), Some(3)).is_err());
    }
    #[test]
    fn password_retry_clears_key_certificate_passphrase_and_device_pin() {
        let mut options = options();
        options.private_key = Zeroizing::new("private fixture".into());
        options.certificate = Zeroizing::new("certificate fixture".into());
        options.passphrase = Zeroizing::new("passphrase fixture".into());
        options.fido2_pin = Zeroizing::new("pin fixture".into());
        assert!(apply_password(&mut options, Zeroizing::new(String::new())).is_err());
        apply_password(&mut options, Zeroizing::new("new fixture password".into())).unwrap();
        assert_eq!(options.password.as_str(), "new fixture password");
        assert!(
            options.private_key.is_empty()
                && options.certificate.is_empty()
                && options.passphrase.is_empty()
                && options.fido2_pin.is_empty()
        );
    }
    #[test]
    fn host_auth_patch_preserves_other_protocols_appearance_and_relations() {
        let mut host: HostEntry = serde_json::from_value(json!({
            "id":7,"uuid":UUID,"name":"fixture","type":"remote","identity_id":null,
            "host":"example.invalid","port":2222,"username":"old","password":"old fixture",
            "group_id":9,"proxy_id":5,"theme_id":"fixture-theme","mosh_enabled":true,
            "mosh_server_command":"mosh-server new","telnet_password":"telnet fixture",
            "telnet_port":2323,"work_dir":"/fixture","encoding":"UTF-8",
            "environment_variables":[{"variable":"LANG","value":"en_US.UTF-8"}],"version":3
        }))
        .unwrap();
        let key = Revision::new(Some(4), Some(OTHER), Some(2)).unwrap();
        let mut expected = host.clone();
        expected.username = Some("admin".into());
        expected.password = None;
        expected.key_id = Some(4);
        expected.key_uuid = Some(OTHER.into());
        patch_host(&mut host, &options(), Some(&key));
        assert_eq!(host, expected);
    }
    #[test]
    fn identity_auth_patch_preserves_existing_username_and_metadata() {
        let mut identity: IdentityEntry = serde_json::from_value(json!({
            "id":7,"uuid":UUID,"name":"shared fixture","username":"shared-user",
            "password":"old fixture","key_id":4,"key_uuid":OTHER,"version":3
        }))
        .unwrap();
        let mut options = options();
        options.password = Zeroizing::new("new fixture".into());
        let mut expected = identity.clone();
        expected.password = Some("new fixture".into());
        expected.key_id = None;
        expected.key_uuid = None;
        patch_identity(&mut identity, &options, None);
        assert_eq!(identity, expected);
        identity.username = None;
        patch_identity(&mut identity, &options, None);
        assert_eq!(identity.username.as_deref(), Some("admin"));
    }
    #[test]
    fn authentication_catalog_is_public_metadata_from_an_in_memory_fixture() {
        let mut database = NautermDatabase::open_in_memory().unwrap();
        let key: KeyEntry = serde_json::from_value(json!({
            "name":"Private fixture","private_key":"synthetic private fixture",
            "public_key":"ssh-ed25519 public-fixture","passphrase":"synthetic passphrase"
        }))
        .unwrap();
        let id = database.save_key(&key).unwrap();
        let public: KeyEntry = serde_json::from_value(json!({
            "name":"Public-only fixture","public_key":"ssh-ed25519 public-only"
        }))
        .unwrap();
        database.save_key(&public).unwrap();
        assert_eq!(database.list_keys().unwrap().len(), 2);
        let entries = database.list_authentication_keys().unwrap();
        assert_eq!(entries.len(), 1);
        let row = &entries[0];
        assert_eq!(row.id, Some(id));
        assert!(row.private_key.is_none() && row.passphrase.is_none());
        assert_eq!(key_choice(row).unwrap().summary.algorithm, "ED25519");
        database.delete_key(id).unwrap();
        assert!(database.list_authentication_keys().unwrap().is_empty());
    }
}
