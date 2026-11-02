//! Public tag catalog using shared Nauterm UUIDs. Hosts keep their compatible name arrays; stable
//! catalog IDs own filter/editor selections and survive rename, including async saves.
use crate::model::{AppState, Host};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};
pub mod picker;
mod tag;
pub use tag::Tag;
#[cfg(test)]
mod tests;
fn format_uuid(hex: &str) -> String {
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}
pub fn validate(tags: &[Tag]) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for tag in tags {
        ensure!(
            (crate::resource_reference::valid_uuid(&tag.id)
                || tag.id.len() == 32 && tag.id.bytes().all(|b| b.is_ascii_hexdigit()))
                && ids.insert(&tag.id),
            "Invalid or duplicate tag ID."
        );
        ensure!(
            !tag.name.trim().is_empty()
                && tag.name == tag.name.trim()
                && !tag.name.contains('\0')
                && names.insert(tag.name.to_lowercase()),
            "Invalid or duplicate tag name."
        );
    }
    Ok(())
}
pub fn inventory(data: &AppState) -> Vec<Tag> {
    use sha2::{Digest, Sha256};
    let mut names: BTreeMap<_, _> = data
        .tags
        .iter()
        .map(|tag| (tag.name.to_lowercase(), tag.clone()))
        .collect();
    let mut ids: BTreeSet<_> = data.tags.iter().map(|tag| tag.id.clone()).collect();
    for name in data
        .hosts
        .iter()
        .flat_map(|host| &host.tags)
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
    {
        let key = name.to_lowercase();
        if names.contains_key(&key) {
            continue;
        }
        let mut nonce = 0u64;
        let id = loop {
            let mut hash = Sha256::new();
            hash.update(b"Nauterm RS legacy tag\0");
            hash.update(key.as_bytes());
            hash.update(nonce.to_be_bytes());
            let id: String = hash.finalize()[..16]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            let id = format_uuid(&id);
            if ids.insert(id.clone()) {
                break id;
            }
            nonce += 1;
        };
        names.insert(
            key,
            Tag {
                id,
                name: name.into(),
            },
        );
    }
    names.into_values().collect()
}
pub fn materialize(data: &mut AppState) {
    data.tags = inventory(data);
    let names: BTreeMap<_, _> = data
        .tags
        .iter()
        .map(|tag| (tag.name.to_lowercase(), tag.name.clone()))
        .collect();
    for host in &mut data.hosts {
        let mut seen = BTreeSet::new();
        host.tags = host
            .tags
            .iter()
            .filter_map(|name| names.get(&name.trim().to_lowercase()))
            .filter(|name| seen.insert(name.to_lowercase()))
            .cloned()
            .collect();
    }
}
pub fn ids(tags: &[Tag], names: &[String]) -> BTreeSet<String> {
    let names: BTreeSet<_> = names.iter().map(|name| name.to_lowercase()).collect();
    tags.iter()
        .filter(|tag| names.contains(&tag.name.to_lowercase()))
        .map(|tag| tag.id.clone())
        .collect()
}
pub fn names(tags: &[Tag], ids: &BTreeSet<String>) -> Vec<String> {
    tags.iter()
        .filter(|tag| ids.contains(&tag.id))
        .map(|tag| tag.name.clone())
        .collect()
}
pub fn create(data: &AppState, name: &str) -> Result<(AppState, Tag)> {
    let name = name.trim();
    ensure!(
        !name.is_empty() && !name.contains('\0'),
        "Name is required."
    );
    let mut data = data.clone();
    materialize(&mut data);
    if let Some(tag) = data
        .tags
        .iter()
        .find(|tag| tag.name.to_lowercase() == name.to_lowercase())
        .cloned()
    {
        return Ok((data, tag));
    }
    let id = loop {
        let id = format_uuid(&format!("{:032x}", rand::random::<u128>()));
        if !data.tags.iter().any(|tag| tag.id == id) {
            break id;
        }
    };
    let tag = Tag {
        id,
        name: name.into(),
    };
    data.tags.push(tag.clone());
    materialize(&mut data);
    data.validate()?;
    Ok((data, tag))
}
pub fn rename(data: &AppState, id: &str, name: &str) -> Result<AppState> {
    let name = name.trim();
    ensure!(
        !name.is_empty() && !name.contains('\0'),
        "Name is required."
    );
    let mut data = data.clone();
    materialize(&mut data);
    let old = data
        .tags
        .iter()
        .find(|tag| tag.id == id)
        .ok_or_else(|| anyhow::anyhow!("The tag no longer exists."))?
        .name
        .to_lowercase();
    ensure!(
        !data
            .tags
            .iter()
            .any(|tag| tag.id != id && tag.name.to_lowercase() == name.to_lowercase()),
        "A tag with this name already exists."
    );
    data.tags.iter_mut().find(|tag| tag.id == id).unwrap().name = name.into();
    for host in &mut data.hosts {
        for value in &mut host.tags {
            if value.to_lowercase() == old {
                *value = name.into();
            }
        }
    }
    materialize(&mut data);
    data.validate()?;
    Ok(data)
}
pub fn delete(data: &AppState, id: &str) -> Result<AppState> {
    let mut data = data.clone();
    materialize(&mut data);
    let old = data
        .tags
        .iter()
        .find(|tag| tag.id == id)
        .ok_or_else(|| anyhow::anyhow!("The tag no longer exists."))?
        .name
        .to_lowercase();
    data.tags.retain(|tag| tag.id != id);
    for host in &mut data.hosts {
        host.tags.retain(|name| name.to_lowercase() != old);
    }
    data.validate()?;
    Ok(data)
}
pub struct Filter {
    names: BTreeSet<String>,
}
impl Filter {
    pub fn is_active(&self) -> bool {
        !self.names.is_empty()
    }
    pub fn new(tags: &[Tag], selected: &BTreeSet<String>) -> Self {
        Self {
            names: tags
                .iter()
                .filter(|tag| selected.contains(&tag.id))
                .map(|tag| tag.name.to_lowercase())
                .collect(),
        }
    }
    pub fn matches(&self, host: &Host) -> bool {
        self.names.is_empty()
            || host
                .tags
                .iter()
                .any(|name| self.names.contains(&name.to_lowercase()))
    }
}
