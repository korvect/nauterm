//! Saved passwords resolve from shared Host/Group rows. Unsaved editor drafts
//! use encrypted temporary references, never password text in workspace files.
use crate::{resource_reference::Reference, vault::Vault};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use zeroize::{Zeroize, Zeroizing};

const KEY: &str = "nauterm_rs.host_passwords.v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Password {
    value: String,
}
impl Drop for Password {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u32,
    entries: BTreeMap<String, Password>,
}
pub fn valid_reference(id: &str) -> bool {
    Reference::parse(id).is_some() || is_draft(id)
}
fn is_draft(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn resolve(
    database: &nauterm_core::NautermDatabase,
    id: &str,
) -> Result<Zeroizing<String>> {
    ensure!(valid_reference(id), "Invalid password reference.");
    if let Some(reference) = Reference::parse(id) {
        return reference
            .load(database)?
            .password(reference.protocol)
            .context("The shared profile has no saved password.");
    }
    let mut password = load(database)?
        .entries
        .remove(id)
        .context("The draft password is unavailable. Enter it again in the Host editor.")?;
    Ok(Zeroizing::new(std::mem::take(&mut password.value)))
}

pub fn validate(value: &str) -> Result<()> {
    ensure!(
        value.len() <= 64 * 1024 && !value.contains('\0'),
        "Password must contain no NUL and be at most 64 KiB."
    );
    Ok(())
}
fn load(database: &nauterm_core::NautermDatabase) -> Result<Catalog> {
    let encoded = database.get_app_meta(KEY)?;
    let catalog: Catalog = match encoded {
        Some(encoded) => serde_json::from_str(&Zeroizing::new(encoded))
            .context("Invalid encrypted Host password catalog; preserved.")?,
        None => Catalog {
            version: 1,
            entries: BTreeMap::new(),
        },
    };
    ensure!(
        catalog.version == 1,
        "Unsupported Host password catalog version."
    );
    for (id, password) in &catalog.entries {
        ensure!(is_draft(id), "Invalid draft password reference.");
        validate(&password.value)?;
    }
    Ok(catalog)
}
fn persist(database: &nauterm_core::NautermDatabase, catalog: &Catalog) -> Result<()> {
    let encoded = Zeroizing::new(serde_json::to_string(catalog)?);
    Ok(database.set_app_meta(KEY, &encoded)?)
}

impl Vault {
    pub async fn host_password(&self, id: String) -> Result<Zeroizing<String>> {
        ensure!(valid_reference(&id), "Invalid password reference.");
        self.dispatch(move |db| resolve(&db.inner, &id)).await
    }

    pub async fn save_host_password(&self, value: Zeroizing<String>) -> Result<String> {
        self.save_host_passwords(vec![value])
            .await?
            .pop()
            .context("No password reference was created.")
    }
    pub async fn save_host_passwords(&self, values: Vec<Zeroizing<String>>) -> Result<Vec<String>> {
        for value in &values {
            validate(value)?;
            ensure!(
                !value.is_empty(),
                "An empty password does not need a saved reference."
            );
        }
        if values.is_empty() {
            return Ok(vec![]);
        }
        self.dispatch(move |db| {
            db.inner.atomic(|database| {
                let mut catalog = load(database)?;
                let mut ids = Vec::with_capacity(values.len());
                for value in values {
                    let id = loop {
                        let id = format!("{:032x}", rand::random::<u128>());
                        if !catalog.entries.contains_key(&id) {
                            break id;
                        }
                    };
                    catalog.entries.insert(
                        id.clone(),
                        Password {
                            value: value.to_string(),
                        },
                    );
                    ids.push(id);
                }
                persist(database, &catalog)?;
                Ok(ids)
            })
        })
        .await
    }
    pub async fn host_passwords(
        &self,
        ids: Vec<String>,
    ) -> Result<BTreeMap<String, Zeroizing<String>>> {
        if ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        ensure!(
            ids.iter().all(|id| valid_reference(id)),
            "Invalid password reference."
        );
        let ids: BTreeSet<_> = ids.into_iter().collect();
        self.dispatch(move |db| {
            ids.into_iter()
                .map(|id| resolve(&db.inner, &id).map(|value| (id, value)))
                .collect()
        })
        .await
    }

    pub async fn remove_host_passwords(&self, ids: Vec<String>) -> Result<()> {
        let ids: Vec<_> = ids.into_iter().filter(|id| is_draft(id)).collect();
        if ids.is_empty() {
            return Ok(());
        }
        self.dispatch(move |db| {
            db.inner.atomic(|database| {
                let mut catalog = load(database)?;
                for id in ids {
                    catalog.entries.remove(&id);
                }
                persist(database, &catalog)
            })
        })
        .await
    }
}
