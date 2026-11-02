//! Transient selected key imports. Publication runs in Store's resource
//! transaction so failed Host/config publication cannot leave imported keys.
use super::*;
use crate::{
    model::{AppState, CredentialRef, Id},
    vault::{KeyRecord, derive_public_key},
};
use nauterm_core::{KeyEntry, NautermDatabase};
use zeroize::Zeroize;

#[derive(Clone)]
pub struct KeyCandidate {
    pub name: String,
    pub private: Option<Zeroizing<String>>,
    pub public: Option<Zeroizing<String>>,
    pub certificate: Option<Zeroizing<String>>,
    pub paths: Vec<String>,
}
impl KeyCandidate {
    pub fn detail(&self) -> &'static str {
        if self.certificate.is_some() {
            "SSH certificate"
        } else if self.private.is_some() && self.public.is_some() {
            "Public and private key"
        } else if self.private.is_some() {
            "Private key"
        } else {
            "Public key"
        }
    }
    pub fn matches(&self, query: &str) -> bool {
        let text = format!("{} {}", self.name, self.detail()).to_lowercase();
        query
            .split_whitespace()
            .all(|word| text.contains(&word.to_lowercase()))
    }
}
fn material(value: &str) -> (bool, bool, bool) {
    let header = value.trim_start().lines().next().unwrap_or_default().trim();
    let private = header.starts_with("-----BEGIN ") && header.ends_with("PRIVATE KEY-----");
    let public = [
        "ssh-rsa ",
        "ssh-ed25519 ",
        "ecdsa-sha2-",
        "sk-ssh-",
        "sk-ecdsa-",
    ]
    .iter()
    .any(|prefix| header.starts_with(prefix));
    let certificate = header
        .split_whitespace()
        .next()
        .is_some_and(|kind| kind.ends_with("-cert-v01@openssh.com"));
    (private, public, certificate)
}
pub(crate) fn recognized(value: &str) -> bool {
    let (private, public, certificate) = material(value);
    private || public || certificate
}
pub fn collect(files: Vec<(PathBuf, Zeroizing<String>)>) -> Vec<KeyCandidate> {
    let mut keys = BTreeMap::<PathBuf, KeyCandidate>::new();
    for (path, text) in files {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let (private, public, certificate) = material(&text);
        if !private && !public && !certificate {
            continue;
        }
        let base = if certificate {
            path.with_file_name(name.strip_suffix("-cert.pub").unwrap_or(name))
        } else if public && name.to_ascii_lowercase().ends_with(".pub") {
            path.with_file_name(&name[..name.len() - 4])
        } else {
            path.clone()
        };
        let key = keys.entry(base).or_insert_with(|| KeyCandidate {
            name: name.into(),
            private: None,
            public: None,
            certificate: None,
            paths: vec![],
        });
        key.paths
            .push(normalize_path(&path.to_string_lossy(), None, None));
        if private {
            key.name = name.into();
            key.private = Some(text);
        } else if certificate {
            key.certificate = Some(text);
        } else {
            key.public = Some(text);
        }
    }
    let mut keys: Vec<_> = keys.into_values().collect();
    keys.sort_by(|a, b| a.name.cmp(&b.name));
    keys
}
#[derive(Default)]
pub struct KeyBatch {
    pub keys: Vec<KeyCandidate>,
    pub bindings: BTreeMap<Id, String>,
}
#[derive(Clone, Default)]
pub struct KeyReport {
    pub created: usize,
    pub reused: usize,
    pub unmatched_hosts: usize,
}
struct SecretKeys(Vec<KeyEntry>);
impl Drop for SecretKeys {
    fn drop(&mut self) {
        for key in &mut self.0 {
            key.private_key.zeroize();
            key.passphrase.zeroize();
            key.certificate.zeroize();
        }
    }
}
fn public_identity(value: &str) -> Option<(&str, &str)> {
    let mut fields = value.split_whitespace();
    Some((fields.next()?, fields.next()?))
}
fn matches(existing: &KeyEntry, candidate: &KeyRecord) -> bool {
    if existing.certificate.as_deref().map(str::trim)
        != candidate.certificate.as_deref().map(str::trim)
    {
        return false;
    }
    if candidate.private_key.is_some()
        && existing
            .private_key
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return false;
    }
    if candidate.certificate.is_some()
        && candidate.private_key.is_none()
        && candidate.public_key.is_none()
    {
        return true;
    }
    if let (Some(left), Some(right)) = (&existing.private_key, &candidate.private_key)
        && left.trim() == right.trim()
    {
        return true;
    }
    match (&existing.public_key, &candidate.public_key) {
        (Some(left), Some(right)) => {
            public_identity(left).is_some() && public_identity(left) == public_identity(right)
        }
        _ => false,
    }
}
impl KeyBatch {
    pub(crate) fn prepare(
        self,
        database: &mut NautermDatabase,
        state: &mut AppState,
    ) -> Result<KeyReport> {
        if self.keys.is_empty() {
            return Ok(KeyReport {
                unmatched_hosts: self.bindings.len(),
                ..Default::default()
            });
        }
        let mut existing = SecretKeys(database.list_keys()?);
        let mut report = KeyReport::default();
        let mut paths = BTreeMap::new();
        let mut names: BTreeSet<_> = existing
            .0
            .iter()
            .map(|key| key.name.to_lowercase())
            .collect();
        for candidate in self.keys {
            let mut record = KeyRecord {
                id: None,
                name: candidate.name.clone(),
                private_key: candidate.private.as_ref().map(|text| text.to_string()),
                public_key: candidate.public.as_ref().map(|text| text.trim().to_owned()),
                certificate: candidate
                    .certificate
                    .as_ref()
                    .map(|text| text.trim().to_owned()),
                passphrase: None,
            };
            ensure!(
                record.private_key.is_some()
                    || record.public_key.is_some()
                    || record.certificate.is_some(),
                "The selected key is empty."
            );
            derive_public_key(&mut record);
            let id = if let Some(key) = existing.0.iter().find(|key| matches(key, &record)) {
                report.reused += 1;
                key.id.context("An existing key has no ID")?
            } else {
                let base = if record.name.trim().is_empty() {
                    "Imported key"
                } else {
                    record.name.trim()
                };
                let mut name = base.to_owned();
                let mut suffix = 2;
                while !names.insert(name.to_lowercase()) {
                    name = format!("{base} ({suffix})");
                    suffix += 1;
                }
                let mut key: KeyEntry = serde_json::from_value(serde_json::json!({"name":name}))?;
                key.private_key = record.private_key.take();
                key.public_key = record.public_key.take();
                key.certificate = record.certificate.take();
                let mut guarded = SecretKeys(vec![key]);
                let id = database.save_key(&guarded.0[0])?;
                guarded.0[0].id = Some(id);
                existing.0.push(guarded.0[0].clone());
                report.created += 1;
                id
            };
            for path in candidate.paths {
                paths.insert(path, id);
            }
        }
        for (host, path) in self.bindings {
            if let Some(id) = paths.get(&path) {
                let host = state
                    .hosts
                    .iter_mut()
                    .find(|item| item.id == host)
                    .context("The imported Host is unavailable")?;
                host.credential = Some(CredentialRef::Key(*id));
            } else {
                report.unmatched_hosts += 1;
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn key_pairing_retains_certificates_paths_and_never_searches_secret_text() {
        let keys=collect(vec![
            ("/synthetic/.ssh/id.pub".into(),Zeroizing::new("ssh-ed25519 AAAA comment".into())),
            ("/synthetic/.ssh/id".into(),Zeroizing::new("-----BEGIN OPENSSH PRIVATE KEY-----\nSYNTHETIC_PRIVATE\n-----END OPENSSH PRIVATE KEY-----".into())),
            ("/synthetic/.ssh/id-cert.pub".into(),Zeroizing::new("ssh-ed25519-cert-v01@openssh.com CCCC certificate".into())),
            ("/synthetic/.ssh/other.pub".into(),Zeroizing::new("ssh-rsa BBBB public".into())),
        ]);
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].name, "id");
        assert!(
            keys[0].private.is_some() && keys[0].public.is_some() && keys[0].certificate.is_some()
        );
        assert_eq!(keys[0].paths.len(), 3);
        assert_eq!(keys[0].detail(), "SSH certificate");
        assert!(!keys[0].matches("SYNTHETIC_PRIVATE"));
        assert_eq!(keys[1].detail(), "Public key");
    }
    #[test]
    fn deduplication_ignores_public_comments_but_preserves_private_key_availability() -> Result<()>
    {
        let mut existing: KeyEntry = serde_json::from_value(
            serde_json::json!({"name":"Existing","public_key":"ssh-ed25519 AAAA old"}),
        )?;
        let mut candidate = KeyRecord {
            id: None,
            name: "New".into(),
            private_key: Some("SYNTHETIC_PRIVATE".into()),
            public_key: Some("ssh-ed25519 AAAA new".into()),
            certificate: None,
            passphrase: None,
        };
        assert!(!matches(&existing, &candidate));
        existing.private_key = Some("SYNTHETIC_EXISTING".into());
        assert!(matches(&existing, &candidate));
        candidate.certificate = Some("ssh-ed25519-cert-v01@openssh.com CERT new".into());
        assert!(!matches(&existing, &candidate));
        Ok(())
    }
}
