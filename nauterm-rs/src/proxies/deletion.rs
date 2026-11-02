//! Cross-store receipt: intent is durable before native deletion; only an
//! explicitly requested, now absent proxy can clear public RS references.
use super::{Deletions, Request, fetch};
use crate::native::Database;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const KEY: &str = "nauterm_rs.proxy_deletions.v1";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u32,
    entries: Deletions,
}
fn valid_uuid(uuid: &str) -> bool {
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn load(db: &mut Database) -> Result<Journal> {
    let stored: Option<String> = db.call(&Request::GetAppMeta { key: KEY })?;
    let journal = match stored {
        Some(stored) => serde_json::from_str::<Journal>(&stored)
            .context("Invalid proxy deletion journal; existing data is preserved.")?,
        None => Journal {
            version: 1,
            entries: Deletions::new(),
        },
    };
    ensure!(
        journal.version == 1
            && journal
                .entries
                .iter()
                .all(|(id, uuid)| *id > 0 && valid_uuid(uuid)),
        "Unsupported or invalid proxy deletion journal; existing data is preserved."
    );
    Ok(journal)
}
fn save(db: &mut Database, journal: &Journal) -> Result<()> {
    db.call(&Request::SetAppMeta {
        key: KEY,
        value: &serde_json::to_string(journal)?,
    })
}
pub(super) fn prepare(db: &mut Database, ids: Vec<i64>) -> Result<BTreeSet<i64>> {
    let ids: BTreeSet<_> = ids.into_iter().collect();
    if ids.is_empty() {
        return Ok(ids);
    }
    ensure!(ids.iter().all(|id| *id > 0), "Invalid proxy reference.");
    let mut journal = load(db)?;
    // Resolve the whole selection before recording intent or deleting anything.
    for id in &ids {
        let record = fetch(db, *id)?;
        let uuid = record
            .uuid
            .as_ref()
            .filter(|uuid| valid_uuid(uuid))
            .context("The proxy has no stable UUID; refresh before deleting.")?;
        journal.entries.insert(*id, uuid.clone());
    }
    save(db, &journal)?;
    Ok(ids)
}
pub(super) fn confirmed(db: &mut Database) -> Result<Deletions> {
    let mut journal = load(db)?;
    let mut deleted = Deletions::new();
    for (id, uuid) in &journal.entries {
        if db
            .call::<Option<super::Record>>(&Request::GetProxy { id: *id })?
            .is_none()
        {
            deleted.insert(*id, uuid.clone());
        }
    }
    // A surviving proxy means that deletion did not commit (including a crash
    // after intent). Never resume a destructive action during startup/refresh.
    if journal.entries != deleted {
        journal.entries = deleted.clone();
        save(db, &journal)?;
    }
    Ok(deleted)
}
pub(super) fn acknowledge(db: &mut Database, deleted: &Deletions) -> Result<()> {
    if deleted.is_empty() {
        return Ok(());
    }
    let mut journal = load(db)?;
    journal
        .entries
        .retain(|id, uuid| deleted.get(id) != Some(uuid));
    save(db, &journal)
}
