//! Encrypted reference database access on a dedicated serial worker.
//! UI state contains only summaries; secret records are fetched only for editing
//! or connecting, zeroized on drop, and never copied into workspace.json.
use crate::native::{Core, Database, SshOptions};
use anyhow::{Context, Result, ensure};
use async_channel::Sender;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock, Weak},
    thread::JoinHandle,
};
use zeroize::{Zeroize, Zeroizing};
pub(crate) mod authentication;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct KeyRecord {
    pub id: Option<i64>,
    pub name: String,
    pub private_key: Option<String>,
    pub public_key: Option<String>,
    pub certificate: Option<String>,
    pub passphrase: Option<String>,
}
impl KeyRecord {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(!self.name.trim().is_empty(), "Name is required.");
        let present = |value: &Option<String>| {
            value
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
        };
        ensure!(
            present(&self.private_key)
                || self.id.is_some() && (present(&self.public_key) || present(&self.certificate)),
            if self.id.is_some() {
                "Key material is required."
            } else {
                "Private key is required."
            }
        );
        Ok(())
    }
}
pub(crate) fn derive_public_key(key: &mut KeyRecord) {
    if key
        .public_key
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        let text = key.private_key.as_deref().unwrap_or_default();
        let decoded = russh::keys::ssh_key::PrivateKey::from_openssh(text)
            .ok()
            .or_else(|| russh::keys::decode_secret_key(text, key.passphrase.as_deref()).ok());
        if let Some(private) = decoded {
            key.public_key = private.public_key().to_openssh().ok();
        }
    }
}
impl Drop for KeyRecord {
    fn drop(&mut self) {
        self.private_key.zeroize();
        self.passphrase.zeroize();
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct IdentityRecord {
    pub id: Option<i64>,
    pub name: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub key_id: Option<i64>,
}
impl Drop for IdentityRecord {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeySummary {
    pub id: i64,
    pub name: String,
    pub algorithm: String,
    pub certificate: bool,
    pub public_key: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentitySummary {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub key_id: Option<i64>,
}
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Inventory {
    pub keys: Vec<KeySummary>,
    pub identities: Vec<IdentitySummary>,
}

#[derive(Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request<'a> {
    ListKeys,
    ListIdentities,
    GetKey { id: i64 },
    GetIdentity { id: i64 },
    SaveKey { key: &'a KeyRecord },
    SaveIdentity { identity: &'a IdentityRecord },
    DeleteKey { id: i64 },
    DeleteIdentity { id: i64 },
}

type Work = Box<dyn FnOnce(&mut std::result::Result<Database, String>) + Send>;

#[derive(Clone)]
pub struct Vault {
    worker: Arc<Worker>,
}
struct Worker {
    queue: Sender<Work>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
impl Worker {
    fn shutdown(&self) {
        // Closing rejects new work but drains every already accepted operation.
        // Join before process-level SQLCipher/crypto teardown can begin.
        self.queue.close();
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
fn workers() -> &'static Mutex<Vec<Weak<Worker>>> {
    static WORKERS: OnceLock<Mutex<Vec<Weak<Worker>>>> = OnceLock::new();
    WORKERS.get_or_init(Mutex::default)
}
impl Vault {
    pub fn prepare_shutdown() {
        let workers: Vec<_> = workers()
            .lock()
            .unwrap()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for worker in &workers {
            worker.queue.close();
        }
        for worker in workers {
            worker.shutdown();
        }
    }
    pub fn open(core: Arc<Core>, path: PathBuf) -> Result<Self> {
        core.require_database()?;
        let (queue, receiver) = async_channel::bounded::<Work>(64);
        let thread = std::thread::Builder::new()
            .name("nauterm-rs-vault".into())
            .spawn(move || {
                let mut database = (|| {
                    ensure!(
                        !std::fs::symlink_metadata(&path)
                            .is_ok_and(|meta| meta.file_type().is_symlink()),
                        "Refusing a symlinked Nauterm database"
                    );
                    let database = core.open_database(&path)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
                    }
                    Ok::<_, anyhow::Error>(database)
                })()
                .map_err(|error| error.to_string());
                while let Ok(work) = receiver.recv_blocking() {
                    work(&mut database);
                }
                // Database (and its native SQLite connection) are destroyed here,
                // after every queued operation finishes, never on the UI thread.
            })?;
        let worker = Arc::new(Worker {
            queue,
            thread: Mutex::new(Some(thread)),
        });
        let mut workers = workers().lock().unwrap();
        workers.retain(|worker| worker.strong_count() > 0);
        workers.push(Arc::downgrade(&worker));
        Ok(Self { worker })
    }

    pub(crate) async fn dispatch<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Database) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (sender, receiver) = async_channel::bounded(1);
        self.worker
            .queue
            .send(Box::new(move |database| {
                let result = match database {
                    Ok(database) => work(database),
                    Err(error) => Err(anyhow::anyhow!(error.clone())),
                };
                let _ = sender.try_send(result);
            }))
            .await
            .map_err(|_| anyhow::anyhow!("The keychain worker has stopped"))?;
        receiver
            .recv()
            .await
            .context("The keychain operation was interrupted")?
    }
    pub(crate) fn dispatch_blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Database) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (sender, receiver) = async_channel::bounded(1);
        self.worker
            .queue
            .send_blocking(Box::new(move |database| {
                let result = match database {
                    Ok(database) => work(database),
                    Err(error) => Err(anyhow::anyhow!(error.clone())),
                };
                let _ = sender.try_send(result);
            }))
            .map_err(|_| anyhow::anyhow!("The keychain worker has stopped"))?;
        receiver
            .recv_blocking()
            .context("The database operation was interrupted")?
    }

    pub async fn inventory(&self) -> Result<Inventory> {
        self.dispatch(|db| {
            let keys: Vec<KeyRecord> = db.call(&Request::ListKeys)?;
            let identities: Vec<IdentityRecord> = db.call(&Request::ListIdentities)?;
            Ok(Inventory {
                keys: keys
                    .iter()
                    .map(|key| {
                        Ok(KeySummary {
                            id: key.id.context("Stored key has no id")?,
                            name: key.name.clone(),
                            certificate: key.certificate.is_some(),
                            public_key: key.public_key.clone(),
                            algorithm: key_algorithm(
                                key.public_key.as_deref(),
                                key.certificate.as_deref(),
                            ),
                        })
                    })
                    .collect::<Result<_>>()?,
                identities: identities
                    .iter()
                    .map(|identity| {
                        Ok(IdentitySummary {
                            id: identity.id.context("Stored identity has no id")?,
                            name: identity.name.clone(),
                            username: identity.username.clone().unwrap_or_default(),
                            key_id: identity.key_id,
                        })
                    })
                    .collect::<Result<_>>()?,
            })
        })
        .await
    }

    pub async fn key(&self, id: i64) -> Result<KeyRecord> {
        self.dispatch(move |db| db.call(&Request::GetKey { id }))
            .await
    }

    pub async fn identity(&self, id: i64) -> Result<IdentityRecord> {
        self.dispatch(move |db| db.call(&Request::GetIdentity { id }))
            .await
    }

    /// A hardware-generated credential cannot be regenerated safely on a save
    /// retry. Keep one public intent UUID and resolve an uncertain prior commit.
    pub(crate) async fn save_generated_key(&self, token: String, key: KeyRecord) -> Result<i64> {
        ensure!(
            token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Invalid generated-key receipt."
        );
        ensure!(
            key.id.is_none() && !key.name.trim().is_empty(),
            "A new generated key is required."
        );
        ensure!(
            key.private_key
                .as_deref()
                .is_some_and(|value| !value.is_empty())
                && key
                    .public_key
                    .as_deref()
                    .is_some_and(|value| !value.is_empty()),
            "Generated key material is incomplete."
        );
        self.dispatch(move |db| {
            let native_uuid = format!(
                "{}-{}-{}-{}-{}",
                &token[..8],
                &token[8..12],
                &token[12..16],
                &token[16..20],
                &token[20..]
            );
            #[derive(Deserialize)]
            struct Receipt {
                id: i64,
                uuid: Option<String>,
                name: String,
                public_key: Option<String>,
            }
            let existing = |db: &mut Database| -> Result<Option<i64>> {
                let entries: Vec<Receipt> = db.call(&Request::ListKeys)?;
                let Some(entry) = entries.into_iter().find(|entry| {
                    entry
                        .uuid
                        .as_deref()
                        .is_some_and(|uuid| uuid.replace('-', "").eq_ignore_ascii_case(&token))
                }) else {
                    return Ok(None);
                };
                ensure!(
                    entry.name == key.name && entry.public_key == key.public_key,
                    "Generated-key receipt no longer matches its key."
                );
                Ok(Some(entry.id))
            };
            if let Some(id) = existing(db)? {
                return Ok(id);
            }
            #[derive(Serialize)]
            struct NativeKey<'a> {
                uuid: &'a str,
                #[serde(flatten)]
                key: &'a KeyRecord,
            }
            #[derive(Serialize)]
            struct Save<'a> {
                op: &'static str,
                key: NativeKey<'a>,
            }
            let result = db.call(&Save {
                op: "save_key",
                key: NativeKey {
                    uuid: &native_uuid,
                    key: &key,
                },
            });
            match result {
                Ok(id) => Ok(id),
                Err(error) => match existing(db) {
                    Ok(Some(id)) => Ok(id),
                    _ => Err(error),
                },
            }
        })
        .await
    }
    pub async fn save_key(&self, mut key: KeyRecord) -> Result<i64> {
        key.validate()?;
        self.dispatch(move |db| {
            derive_public_key(&mut key);
            db.call(&Request::SaveKey { key: &key })
        })
        .await
    }

    pub async fn save_identity(&self, identity: IdentityRecord) -> Result<i64> {
        ensure!(!identity.name.trim().is_empty(), "Name is required.");
        ensure!(
            identity
                .username
                .as_deref()
                .is_some_and(|name| !name.trim().is_empty()),
            "Username is required."
        );
        self.dispatch(move |db| {
            db.call(&Request::SaveIdentity {
                identity: &identity,
            })
        })
        .await
    }

    pub async fn delete(&self, keys: bool, ids: Vec<i64>) -> Result<()> {
        self.dispatch(move |db| {
            for id in ids {
                let changed: usize = if keys {
                    db.call(&Request::DeleteKey { id })?
                } else {
                    db.call(&Request::DeleteIdentity { id })?
                };
                ensure!(changed == 1, "The selected keychain item no longer exists.");
            }
            Ok(())
        })
        .await
    }

    pub async fn duplicate(&self, keys: bool, ids: Vec<i64>) -> Result<Vec<i64>> {
        self.dispatch(move |db| {
            // Fetch every source first. Missing sources never create a partial
            // copy, and secret records remain inside the serial vault worker.
            let mut key_records = vec![];
            let mut identities = vec![];
            for id in ids.into_iter().collect::<std::collections::BTreeSet<_>>() {
                if keys {
                    let mut item: KeyRecord = db.call(&Request::GetKey { id })?;
                    item.id = None;
                    item.name.push_str(" copy");
                    key_records.push(item);
                } else {
                    let mut item: IdentityRecord = db.call(&Request::GetIdentity { id })?;
                    item.id = None;
                    item.name.push_str(" copy");
                    identities.push(item);
                }
            }
            let mut created = vec![];
            let result = (|| {
                for key in &key_records {
                    created.push(db.call::<i64>(&Request::SaveKey { key })?);
                }
                for identity in &identities {
                    created.push(db.call::<i64>(&Request::SaveIdentity { identity })?);
                }
                anyhow::Ok(())
            })();
            if let Err(error) = result {
                let mut cleanup_failed = false;
                for id in created {
                    let result = if keys { db.call::<usize>(&Request::DeleteKey { id }) } else { db.call::<usize>(&Request::DeleteIdentity { id }) };
                    cleanup_failed |= result.is_err();
                }
                if cleanup_failed { anyhow::bail!("{error}. Some copies could not be rolled back; refresh the keychain before retrying."); }
                return Err(error);
            }
            Ok(created)
        }).await
    }

    pub async fn apply_identity(&self, id: i64, options: SshOptions) -> Result<SshOptions> {
        self.apply_identity_with_fallback(id, options, None).await
    }
    pub(crate) async fn apply_identity_with_fallback(
        &self,
        id: i64,
        mut options: SshOptions,
        fallback_key: Option<i64>,
    ) -> Result<SshOptions> {
        let identity = self.identity(id).await?;
        if let Some(username) = identity
            .username
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            options.username = username.trim().to_owned();
        }
        options.password = Zeroizing::new(identity.password.clone().unwrap_or_default());
        if let Some(id) = identity.key_id.or(fallback_key) {
            self.apply_key(id, &mut options).await?;
        }
        Ok(options)
    }

    pub async fn apply_key(&self, id: i64, options: &mut SshOptions) -> Result<()> {
        let key = self.key(id).await?;
        options.private_key = Zeroizing::new(key.private_key.clone().unwrap_or_default());
        options.certificate = Zeroizing::new(key.certificate.clone().unwrap_or_default());
        options.passphrase = Zeroizing::new(key.passphrase.clone().unwrap_or_default());
        Ok(())
    }
    pub async fn apply_reference(
        &self,
        reference: crate::model::CredentialRef,
        options: SshOptions,
    ) -> Result<SshOptions> {
        match reference {
            crate::model::CredentialRef::Identity(id) => self.apply_identity(id, options).await,
            crate::model::CredentialRef::Key(id) => {
                let mut options = options;
                self.apply_key(id, &mut options).await?;
                Ok(options)
            }
        }
    }
}

fn key_algorithm(public_key: Option<&str>, certificate: Option<&str>) -> String {
    let kind = certificate
        .or(public_key)
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default();
    let algorithm = if kind.contains("ed25519") {
        "ED25519"
    } else if kind.contains("ecdsa") {
        "ECDSA"
    } else if kind.contains("rsa") {
        "RSA"
    } else if kind.contains("dss") {
        "DSA"
    } else {
        "Private key"
    };
    let security_key = kind.starts_with("sk-");
    format!(
        "{}{}{}",
        if security_key { "FIDO2 " } else { "" },
        algorithm,
        if certificate.is_some() {
            " SSH Certificate"
        } else {
            ""
        }
    )
}

#[cfg(test)]
mod tests {
    use super::{KeyRecord, key_algorithm};

    #[test]
    fn last_vault_owner_drains_and_joins_its_worker() {
        use super::{Arc, Mutex, Vault, Work, Worker};
        use std::sync::atomic::{AtomicBool, Ordering};
        let (queue, receiver) = async_channel::bounded::<Work>(2);
        let finished = Arc::new(AtomicBool::new(false));
        let worker_finished = finished.clone();
        let thread = std::thread::spawn(move || {
            let mut database = Err("No database needed for lifetime test".into());
            while let Ok(work) = receiver.recv_blocking() {
                work(&mut database);
            }
            worker_finished.store(true, Ordering::SeqCst);
        });
        let vault = Vault {
            worker: Arc::new(Worker {
                queue: queue.clone(),
                thread: Mutex::new(Some(thread)),
            }),
        };
        let other_owner = vault.clone();
        drop(vault);
        assert!(!queue.is_closed());
        let (sent, received) = std::sync::mpsc::channel();
        queue
            .send_blocking(Box::new(move |_| sent.send(()).unwrap()))
            .unwrap();
        drop(other_owner);
        assert!(queue.is_closed());
        assert!(
            finished.load(Ordering::SeqCst),
            "Database worker must finish before the final owner returns"
        );
        assert!(
            received.try_recv().is_ok(),
            "Accepted work must not be discarded at shutdown"
        );
    }

    #[test]
    fn saved_public_keys_and_certificates_remain_editable_without_a_private_key() {
        let mut key = KeyRecord {
            id: Some(7),
            name: "Imported public key".into(),
            private_key: None,
            public_key: Some("ssh-ed25519 SYNTHETIC public".into()),
            certificate: None,
            passphrase: None,
        };
        assert!(key.validate().is_ok());
        key.public_key = None;
        key.certificate = Some("ssh-ed25519-cert-v01@openssh.com SYNTHETIC".into());
        assert!(key.validate().is_ok());
        key.certificate = None;
        assert!(key.validate().is_err());
        key.public_key = Some("ssh-ed25519 SYNTHETIC public".into());
        key.id = None;
        assert!(key.validate().is_err());
        key.private_key = Some("SYNTHETIC_PRIVATE".into());
        assert!(key.validate().is_ok());
        key.name.clear();
        assert!(key.validate().is_err());
    }

    #[test]
    fn summaries_distinguish_certificate_algorithm_and_security_key() {
        assert_eq!(
            key_algorithm(Some("ssh-ed25519 AAAA comment"), None),
            "ED25519"
        );
        assert_eq!(
            key_algorithm(None, Some("ecdsa-sha2-nistp256-cert-v01@openssh.com")),
            "ECDSA SSH Certificate"
        );
        assert_eq!(
            key_algorithm(Some("sk-ssh-ed25519@openssh.com AAAA"), None),
            "FIDO2 ED25519"
        );
    }
}
