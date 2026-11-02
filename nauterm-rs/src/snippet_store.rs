//! Shared Nauterm snippet/package rows. No RS-private encrypted catalog.
use crate::{
    model::{Id, Snippet, SnippetScope},
    native::Database,
    vault::Vault,
};
use anyhow::{Context, Result, ensure};
use nauterm_core::{NautermDatabase, NewCatalogRecord, SnippetEntry, SnippetPackageEntry};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub id: Id,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    uuid: Option<String>,
    version: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Catalog {
    next_id: Id,
    pub packages: Vec<Package>,
    pub snippets: Vec<Snippet>,
    #[serde(skip)]
    package_stamps: BTreeMap<Id, Stamp>,
    #[serde(skip)]
    snippet_stamps: BTreeMap<Id, Stamp>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            next_id: 1,
            packages: vec![],
            snippets: vec![],
            package_stamps: BTreeMap::new(),
            snippet_stamps: BTreeMap::new(),
        }
    }
}

impl Catalog {
    pub fn verify_import(&self, legacy: &[Snippet]) -> Result<()> {
        ensure!(
            legacy.is_empty(),
            "Prototype RS snippets are not automatically merged into shared Nauterm data."
        );
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.snippets.len() <= 10000 && self.packages.len() <= 2048,
            "Snippet library is too large"
        );
        let mut maximum = 0;
        for ids in [
            self.packages.iter().map(|item| item.id).collect::<Vec<_>>(),
            self.snippets.iter().map(|item| item.id).collect(),
        ] {
            let mut seen = BTreeSet::new();
            for id in ids {
                ensure!(
                    id > 0 && id <= i64::MAX as u64 && seen.insert(id),
                    "Invalid or duplicate catalog ID"
                );
                maximum = maximum.max(id);
            }
        }
        ensure!(
            self.next_id > maximum && self.next_id <= i64::MAX as u64,
            "Invalid next catalog ID"
        );
        for package in &self.packages {
            ensure!(!package.name.trim().is_empty(), "Package name is required.");
        }
        for snippet in &self.snippets {
            snippet.validate()?;
            ensure!(
                snippet
                    .package_id
                    .is_none_or(|id| self.packages.iter().any(|p| p.id == id)),
                "Snippet package is unavailable."
            );
        }
        Ok(())
    }
    fn allocate(&mut self) -> Result<Id> {
        ensure!(self.next_id < i64::MAX as u64, "No catalog IDs remain");
        let id = self.next_id;
        self.next_id += 1;
        Ok(id)
    }
    pub fn save(&mut self, mut snippet: Snippet, existing: Option<Id>) -> Result<Id> {
        let mut candidate = self.clone();
        let id = match existing {
            Some(id) => {
                ensure!(
                    candidate.snippets.iter().any(|s| s.id == id),
                    "Snippet is no longer available."
                );
                id
            }
            None => candidate.allocate()?,
        };
        snippet.id = id;
        snippet.validate()?;
        candidate.snippets.retain(|s| s.id != id);
        candidate.snippets.push(snippet);
        candidate.validate()?;
        *self = candidate;
        Ok(id)
    }
    pub fn duplicate_many(&mut self, ids: &BTreeSet<Id>) -> Result<Vec<Id>> {
        ensure!(!ids.is_empty(), "Select at least one snippet.");
        let sources = ids
            .iter()
            .map(|id| {
                self.snippets
                    .iter()
                    .find(|s| s.id == *id)
                    .cloned()
                    .context("Snippet is no longer available.")
            })
            .collect::<Result<Vec<_>>>()?;
        let mut candidate = self.clone();
        let mut created = Vec::with_capacity(sources.len());
        for mut snippet in sources {
            snippet.id = candidate.allocate()?;
            snippet.name.push_str(" copy");
            created.push(snippet.id);
            candidate.snippets.push(snippet);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(created)
    }
    pub fn save_package(&mut self, id: Option<Id>, name: String) -> Result<Id> {
        let mut candidate = self.clone();
        let id = match id {
            Some(id) => {
                ensure!(
                    candidate.packages.iter().any(|p| p.id == id),
                    "Package is no longer available."
                );
                id
            }
            None => candidate.allocate()?,
        };
        candidate.packages.retain(|p| p.id != id);
        candidate.packages.push(Package {
            id,
            name: name.trim().into(),
        });
        candidate.validate()?;
        *self = candidate;
        Ok(id)
    }
    pub fn delete(&mut self, ids: &BTreeSet<Id>, packages: bool) -> Result<()> {
        let mut candidate = self.clone();
        if packages {
            candidate.packages.retain(|p| !ids.contains(&p.id));
            for s in &mut candidate.snippets {
                if s.package_id.is_some_and(|id| ids.contains(&id)) {
                    s.package_id = None;
                }
            }
        } else {
            candidate.snippets.retain(|s| !ids.contains(&s.id));
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

fn id(value: Option<i64>) -> Result<Id> {
    let value = value.context("Missing shared resource ID")?;
    ensure!(value > 0, "Invalid shared resource ID");
    Ok(value as Id)
}
fn from_rows(
    packages: Vec<SnippetPackageEntry>,
    snippets: Vec<SnippetEntry>,
    next_id: i64,
) -> Result<Catalog> {
    let mut catalog = Catalog {
        next_id: id(Some(next_id))?,
        ..Default::default()
    };
    for row in packages {
        let id = id(row.id)?;
        catalog.package_stamps.insert(
            id,
            Stamp {
                uuid: row.uuid,
                version: row.version,
            },
        );
        catalog.packages.push(Package { id, name: row.name });
    }
    for row in snippets {
        let id = id(row.id)?;
        catalog.snippet_stamps.insert(
            id,
            Stamp {
                uuid: row.uuid,
                version: row.version,
            },
        );
        catalog.snippets.push(Snippet {
            id,
            name: row.description,
            command: row.script,
            description: String::new(),
            scope: match row.scope.as_str() {
                "global" => SnippetScope::Global,
                "targeted" => SnippetScope::Targeted,
                _ => anyhow::bail!("Unknown snippet scope; existing data was preserved"),
            },
            package_id: row
                .package_id
                .map(|value| self::id(Some(value)))
                .transpose()?,
            target_group_ids: row
                .target_group_ids
                .into_iter()
                .map(|value| self::id(Some(value)))
                .collect::<Result<_>>()?,
            target_host_ids: row
                .target_host_ids
                .into_iter()
                .map(|value| self::id(Some(value)))
                .collect::<Result<_>>()?,
        });
    }
    catalog.validate()?;
    Ok(catalog)
}
fn load(database: &NautermDatabase) -> Result<Catalog> {
    from_rows(
        database.list_snippet_packages()?,
        database.list_snippets(None)?,
        database.next_catalog_id()?,
    )
}
fn check_stamp(expected: Option<&Stamp>, current: Option<&Stamp>) -> Result<()> {
    ensure!(
        expected.is_some() && expected == current,
        "This snippet or package changed in another window. Close the editor and reload before saving."
    );
    Ok(())
}
fn changed<T: PartialEq>(before: Option<&T>, after: Option<&T>) -> bool {
    before != after
}
fn apply_snippet(row: &mut SnippetEntry, value: &Snippet) -> Result<()> {
    row.description = value.name.clone();
    row.script = value.command.clone();
    row.scope = if value.targeted() {
        "targeted"
    } else {
        "global"
    }
    .into();
    row.package_id = value.package_id.map(i64::try_from).transpose()?;
    row.package_uuid = None;
    row.target_group_ids = value
        .target_group_ids
        .iter()
        .map(|id| i64::try_from(*id))
        .collect::<std::result::Result<_, _>>()?;
    row.target_host_ids = value
        .target_host_ids
        .iter()
        .map(|id| i64::try_from(*id))
        .collect::<std::result::Result<_, _>>()?;
    Ok(())
}
fn persist(database: &mut NautermDatabase, before: &Catalog, after: &Catalog) -> Result<Catalog> {
    after.validate()?;
    let current = load(database)?;
    // Check every edited/deleted row before publishing any part of the edit.
    for old in &before.packages {
        let new = after.packages.iter().find(|item| item.id == old.id);
        if changed(Some(old), new) {
            check_stamp(
                before.package_stamps.get(&old.id),
                current.package_stamps.get(&old.id),
            )?;
        }
    }
    for old in &before.snippets {
        let new = after.snippets.iter().find(|item| item.id == old.id);
        if changed(Some(old), new) {
            check_stamp(
                before.snippet_stamps.get(&old.id),
                current.snippet_stamps.get(&old.id),
            )?;
        }
    }
    for value in &after.packages {
        let old = before.packages.iter().find(|item| item.id == value.id);
        if !changed(old, Some(value)) {
            continue;
        }
        if old.is_some() {
            let mut row = database
                .list_snippet_packages()?
                .into_iter()
                .find(|row| row.id == Some(value.id as i64))
                .context("Snippet package was deleted")?;
            row.name = value.name.clone();
            database.save_snippet_package(&row)?;
        } else {
            let row: SnippetPackageEntry =
                serde_json::from_value(serde_json::json!({"name":value.name}))?;
            database.insert_catalog_at(value.id as i64, NewCatalogRecord::SnippetPackage(&row))?;
        }
    }
    for value in &after.snippets {
        let old = before.snippets.iter().find(|item| item.id == value.id);
        if !changed(old, Some(value)) {
            continue;
        }
        if old.is_some() {
            let mut row = database
                .get_snippet(value.id as i64)?
                .context("Snippet was deleted")?;
            apply_snippet(&mut row, value)?;
            database.save_snippet(&row)?;
        } else {
            let mut row: SnippetEntry =
                serde_json::from_value(serde_json::json!({"description":"", "script":""}))?;
            apply_snippet(&mut row, value)?;
            database.insert_catalog_at(value.id as i64, NewCatalogRecord::Snippet(&row))?;
        }
    }
    for old in &before.snippets {
        if !after.snippets.iter().any(|item| item.id == old.id) {
            database.delete_snippet(old.id as i64)?;
        }
    }
    for old in &before.packages {
        if !after.packages.iter().any(|item| item.id == old.id) {
            database.delete_snippet_package(old.id as i64)?;
        }
    }
    load(database)
}
impl Vault {
    pub async fn snippet_catalog(&self, legacy: Vec<Snippet>) -> Result<Catalog> {
        self.dispatch(move |db| {
            let catalog = load(&db.inner)?;
            catalog.verify_import(&legacy)?;
            Ok(catalog)
        })
        .await
    }
    pub async fn update_snippets(
        &self,
        before: Catalog,
        work: impl FnOnce(&mut Catalog) -> Result<Option<Id>> + Send + 'static,
    ) -> Result<(Catalog, Option<Id>)> {
        self.dispatch(move |db: &mut Database| {
            let mut after = before.clone();
            let id = work(&mut after)?;
            let catalog = db
                .inner
                .atomic(|database| persist(database, &before, &after))?;
            Ok((catalog, id))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_and_snippet_ids_overlap_and_empty_targeted_scope_stays_targeted() -> Result<()> {
        let package: SnippetPackageEntry =
            serde_json::from_value(serde_json::json!({"id":1,"name":"Package"}))?;
        let snippet: SnippetEntry = serde_json::from_value(
            serde_json::json!({"id":1,"package_id":1,"description":"Restricted", "script":"echo example", "scope":"targeted"}),
        )?;
        let catalog = from_rows(vec![package], vec![snippet], 2)?;
        assert_eq!(catalog.packages[0].id, catalog.snippets[0].id);
        assert!(catalog.snippets[0].targeted());
        assert!(!catalog.snippets[0].matches_host(None, &crate::model::AppState::default()));
        Ok(())
    }
    #[test]
    fn stale_or_replaced_rows_cannot_pass_the_edit_check() {
        let before = Stamp {
            uuid: Some("first".into()),
            version: Some(3),
        };
        let updated = Stamp {
            uuid: Some("first".into()),
            version: Some(4),
        };
        let replaced = Stamp {
            uuid: Some("second".into()),
            version: Some(3),
        };
        assert!(check_stamp(Some(&before), Some(&before)).is_ok());
        assert!(check_stamp(Some(&before), Some(&updated)).is_err());
        assert!(check_stamp(Some(&before), Some(&replaced)).is_err());
        assert!(check_stamp(Some(&before), None).is_err());
    }
}
