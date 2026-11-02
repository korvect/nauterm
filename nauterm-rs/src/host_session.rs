//! Saved SSH shell setup. Public Host metadata contains references only;
//! environment values and snippet bodies are resolved from shared Nauterm data.
use crate::{
    model::Id,
    native::SshOptions,
    resource_reference::{Protocol, Reference},
    vault::Vault,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use zeroize::{Zeroize, Zeroizing};

const KEY: &str = "nauterm_rs.host_environments.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    pub port: u16,
    pub username: String,
    pub credential: Option<crate::model::CredentialRef>,
    pub theme: Option<crate::terminal_appearance::ThemeId>,
    pub setup: Setup,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            port: 22,
            username: String::new(),
            credential: None,
            theme: None,
            setup: Setup::default(),
        }
    }
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.port > 0, "SSH port must be between 1 and 65535.");
        self.setup.validate()
    }
}
#[derive(Default)]
pub struct Inheritance {
    pub port_configured: bool,
    pub encoding_configured: bool,
    pub password_configured: bool,
}
/// Reference behavior is a draft-time fill/merge. Editing a Group never changes
/// the settings of Hosts that were already saved under it.
pub fn inherit_defaults(
    host: &mut crate::model::Host,
    profile: &Profile,
    proxy: Option<i64>,
    flags: &mut Inheritance,
) {
    if !host.is_remote() {
        return;
    }
    host.proxy_id = host.proxy_id.or(proxy);
    if !flags.port_configured && matches!(host.port, 0 | 22) {
        host.port = profile.port;
        flags.port_configured = true;
    }
    let manual_auth_empty = host.username.trim().is_empty()
        && host.ssh.password_id.is_none()
        && !flags.password_configured
        && !matches!(
            host.credential,
            Some(crate::model::CredentialRef::Identity(_))
        );
    if manual_auth_empty {
        if matches!(
            profile.credential,
            Some(crate::model::CredentialRef::Identity(_))
        ) {
            host.credential = profile.credential;
            host.ssh.profile_source = profile.setup.profile_source.clone();
        } else {
            host.username = profile.username.clone();
            host.ssh.password_id = profile.setup.password_id.clone();
        }
    }
    if host.credential.is_none()
        && matches!(
            profile.credential,
            Some(crate::model::CredentialRef::Key(_))
        )
    {
        host.credential = profile.credential;
    }
    host.theme = host.theme.or(profile.theme);
    host.ssh.startup_snippet = host.ssh.startup_snippet.or(profile.setup.startup_snippet);
    if !flags.encoding_configured {
        host.ssh.encoding = profile.setup.encoding;
        flags.encoding_configured = true;
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Encoding {
    #[default]
    #[serde(rename = "UTF-8")]
    Utf8,
    #[serde(rename = "GBK")]
    Gbk,
    #[serde(rename = "GB18030")]
    Gb18030,
    Big5,
    #[serde(rename = "ISO-8859-1")]
    Latin1,
}
impl Encoding {
    pub const ALL: [Self; 5] = [
        Self::Utf8,
        Self::Gbk,
        Self::Gb18030,
        Self::Big5,
        Self::Latin1,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Gbk => "GBK",
            Self::Gb18030 => "GB18030",
            Self::Big5 => "Big5",
            Self::Latin1 => "ISO-8859-1",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Setup {
    /// Canonical shared profile backing this setup. It contains no secret.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_source: Option<String>,
    pub startup_snippet: Option<Id>,
    pub environment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_id: Option<String>,
    pub encoding: Encoding,
}
impl Setup {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.profile_source
                .as_deref()
                .is_none_or(|value| Reference::parse(value)
                    .is_some_and(|reference| reference.protocol == Protocol::Ssh)),
            "Invalid shared profile reference."
        );
        ensure!(
            self.password_id
                .as_deref()
                .is_none_or(crate::host_password::valid_reference),
            "Invalid password reference."
        );
        ensure!(
            self.startup_snippet
                .is_none_or(|id| id > 0 && id <= i64::MAX as u64),
            "Invalid startup snippet reference."
        );
        ensure!(
            self.environment_id.as_deref().is_none_or(valid_id),
            "Invalid environment reference."
        );
        Ok(())
    }
}
fn valid_id(id: &str) -> bool {
    Reference::parse(id).is_some() || draft_id(id)
}
fn draft_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variable {
    pub variable: String,
    pub value: String,
}
impl Drop for Variable {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}
pub fn validate_environment(environment: &[Variable]) -> Result<()> {
    ensure!(
        environment.len() <= 256,
        "Use at most 256 environment variables."
    );
    let mut names = BTreeSet::new();
    let mut bytes = 0usize;
    for entry in environment {
        ensure!(
            !entry.variable.is_empty()
                && entry.variable.len() <= 256
                && entry.variable == entry.variable.trim()
                && !entry
                    .variable
                    .chars()
                    .any(|ch| ch.is_control() || ch == '='),
            "Environment variable names must be nonempty and contain no '=' or control characters."
        );
        ensure!(
            names.insert(&entry.variable),
            "Environment variable names must be unique."
        );
        ensure!(
            entry.value.len() <= 64 * 1024 && !entry.value.contains('\0'),
            "Environment values must contain no NUL and be at most 64 KiB."
        );
        bytes += entry.variable.len() + entry.value.len();
    }
    ensure!(bytes <= 1024 * 1024, "Environment variables exceed 1 MiB.");
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u32,
    entries: BTreeMap<String, Vec<Variable>>,
}
fn load(database: &nauterm_core::NautermDatabase) -> Result<Catalog> {
    let encoded = database.get_app_meta(KEY)?;
    let catalog: Catalog = match encoded {
        Some(encoded) => serde_json::from_str(&Zeroizing::new(encoded))
            .context("Invalid encrypted Host environment catalog; preserved.")?,
        None => Catalog {
            version: 1,
            entries: BTreeMap::new(),
        },
    };
    ensure!(
        catalog.version == 1,
        "Unsupported Host environment catalog version."
    );
    for (id, entries) in &catalog.entries {
        ensure!(draft_id(id), "Invalid draft environment reference.");
        validate_environment(entries)?;
    }
    Ok(catalog)
}
fn persist(database: &nauterm_core::NautermDatabase, catalog: &Catalog) -> Result<()> {
    let encoded = Zeroizing::new(serde_json::to_string(catalog)?);
    Ok(database.set_app_meta(KEY, &encoded)?)
}
pub(crate) fn resolve_environment(
    database: &nauterm_core::NautermDatabase,
    id: &str,
) -> Result<Vec<Variable>> {
    ensure!(valid_id(id), "Invalid environment reference.");
    let environment = if let Some(reference) = Reference::parse(id) {
        ensure!(
            reference.protocol == Protocol::Ssh,
            "Telnet profiles do not define an SSH environment."
        );
        reference.load(database)?.environment()
    } else {
        load(database)?
            .entries
            .remove(id)
            .context("The draft environment is unavailable.")?
    };
    validate_environment(&environment)?;
    Ok(environment)
}
impl Vault {
    pub async fn merge_host_environments(
        &self,
        local: Option<Vec<Variable>>,
        base: Option<String>,
        inherited: Vec<String>,
    ) -> Result<Vec<Variable>> {
        self.dispatch(move |db| {
            let mut entries = match local {
                Some(entries) => entries,
                None => base
                    .map(|id| resolve_environment(&db.inner, &id))
                    .transpose()?
                    .unwrap_or_default(),
            };
            let mut names: BTreeSet<_> =
                entries.iter().map(|entry| entry.variable.clone()).collect();
            for id in inherited {
                for entry in resolve_environment(&db.inner, &id)? {
                    if names.insert(entry.variable.clone()) {
                        entries.push(entry);
                    }
                }
            }
            validate_environment(&entries)?;
            Ok(entries)
        })
        .await
    }
    pub async fn host_environment(&self, id: String) -> Result<Vec<Variable>> {
        self.dispatch(move |db| resolve_environment(&db.inner, &id))
            .await
    }
    /// Draft values are encrypted snapshots until a resource save publishes
    /// them in the original Host/Group row. Shared references are never pruned.
    pub async fn save_host_environment(&self, environment: Vec<Variable>) -> Result<String> {
        validate_environment(&environment)?;
        self.dispatch(move |db| {
            db.inner.atomic(|database| {
                let mut catalog = load(database)?;
                let id = loop {
                    let id = format!("{:032x}", rand::random::<u128>());
                    if !catalog.entries.contains_key(&id) {
                        break id;
                    }
                };
                catalog.entries.insert(id.clone(), environment);
                persist(database, &catalog)?;
                Ok(id)
            })
        })
        .await
    }
    pub async fn remove_host_environments(&self, ids: Vec<String>) -> Result<()> {
        let ids: Vec<_> = ids.into_iter().filter(|id| draft_id(id)).collect();
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
    pub async fn apply_shell_setup(
        &self,
        setup: Setup,
        mut options: SshOptions,
    ) -> Result<SshOptions> {
        setup.validate()?;
        options.encoding = setup.encoding;
        if let Some(id) = setup.environment_id {
            options.environment = self.host_environment(id).await?;
        }
        if let Some(id) = setup.startup_snippet {
            let catalog = self.snippet_catalog(vec![]).await?;
            let snippet = catalog.snippets.iter().find(|snippet| snippet.id == id)
                .context("The startup snippet is unavailable. Edit the Host to choose another snippet or None.")?;
            options.startup = Zeroizing::new(snippet.command.clone());
        }
        Ok(options)
    }
}
pub fn startup_input(script: &str) -> Result<Zeroizing<Vec<u8>>> {
    ensure!(
        !script.contains('\0') && script.len() <= 1024 * 1024,
        "Invalid startup snippet."
    );
    let normalized = Zeroizing::new(script.replace("\r\n", "\n").replace('\n', "\r"));
    let mut bytes = Zeroizing::new(normalized.trim_end().as_bytes().to_vec());
    if !bytes.is_empty() {
        bytes.push(b'\r');
    }
    Ok(bytes)
}
