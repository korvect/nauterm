//! Public projections of shared resource rows, with optimistic edit baselines.
//! Native profiles are short lived and erase their secrets on drop. Updates
//! patch supported fields on current rows instead of replacing a stale catalog.
use crate::{
    host_session::{Encoding, Profile, Setup},
    model::{AppState, CredentialRef, Forward, Group, Host, HostKind, Id},
    port_forward::{self, Kind},
    resource_reference::{self, Profile as SecretProfile},
    tags::Tag,
    terminal_appearance::ThemeId,
};
use anyhow::{Context, Result, ensure};
use nauterm_core::{
    HostEntry, HostGroup, HostType, NautermDatabase, NewCatalogRecord, PortForwardEntry, TagEntry,
};
use std::collections::BTreeMap;
use zeroize::Zeroize;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    id: i64,
    uuid: String,
    version: Option<i64>,
}
impl Stamp {
    fn new(id: Option<i64>, uuid: Option<&str>, version: Option<i64>) -> Result<Self> {
        let id = id
            .filter(|id| *id > 0)
            .context("Missing shared resource ID")?;
        let uuid = uuid
            .filter(|value| resource_reference::valid_uuid(value))
            .context("Missing shared resource UUID")?
            .to_owned();
        Ok(Self { id, uuid, version })
    }
    fn reference(&self, kind: &str) -> String {
        format!("native:{kind}:{}:{}", self.id, self.uuid)
    }
}

#[derive(Clone, Default)]
pub(super) struct Catalog {
    pub hosts: Vec<Host>,
    pub groups: Vec<Group>,
    pub tags: Vec<Tag>,
    pub forwards: Vec<Forward>,
    next_id: Id,
    hosts_at: BTreeMap<Id, Stamp>,
    groups_at: BTreeMap<Id, Stamp>,
    tags_at: BTreeMap<String, Stamp>,
    forwards_at: BTreeMap<Id, Stamp>,
}

fn id(value: i64) -> Result<Id> {
    ensure!(value > 0, "Invalid shared resource ID");
    Ok(value as Id)
}
fn port(value: Option<i64>) -> Result<u16> {
    u16::try_from(value.unwrap_or(22))
        .ok()
        .filter(|value| *value > 0)
        .context("Invalid saved port")
}
trait NativeProfile {
    fn view(&self, reference: String) -> Result<Profile>;
    fn telnet_view(&self, reference: String) -> Result<Option<crate::telnet::Profile>>;
    fn mosh_view(&self) -> Result<Option<crate::mosh::Config>>;
    fn patch_mosh(
        &mut self,
        before: Option<&crate::mosh::Config>,
        after: Option<&crate::mosh::Config>,
    ) -> Result<()>;
    fn telnet_edit(&mut self) -> TelnetEdit<'_>;
    fn edit(&mut self) -> ProfileEdit<'_>;
}
struct TelnetEdit<'a> {
    enabled: &'a mut Option<bool>,
    port: &'a mut Option<i64>,
    username: &'a mut Option<String>,
    password: &'a mut Option<String>,
    identity_id: &'a mut Option<i64>,
    identity_uuid: &'a mut Option<String>,
    theme: &'a mut Option<String>,
    encoding: &'a mut Option<String>,
}
fn patch_telnet(
    database: &NautermDatabase,
    row: TelnetEdit<'_>,
    before: Option<&crate::telnet::Profile>,
    after: Option<&crate::telnet::Profile>,
) -> Result<()> {
    if before == after {
        return Ok(());
    }
    *row.enabled = Some(after.is_some());
    let Some(after) = after else {
        *row.port = None;
        *row.username = None;
        row.password.zeroize();
        *row.password = None;
        *row.identity_id = None;
        *row.identity_uuid = None;
        *row.theme = None;
        *row.encoding = None;
        return Ok(());
    };
    after.validate()?;
    if differs(before.map(|value| &value.port), &after.port) {
        *row.port = Some(i64::from(after.port));
    }
    if differs(before.map(|value| &value.identity_id), &after.identity_id) {
        *row.identity_id = after.identity_id;
        *row.identity_uuid = None;
    }
    if after.identity_id.is_none() && differs(before.map(|value| &value.username), &after.username)
    {
        *row.username = (!after.username.is_empty()).then(|| after.username.clone());
    }
    if differs(before.map(|value| &value.theme), &after.theme) {
        *row.theme = after.theme.map(|theme| theme.id().into());
    }
    if differs(before.map(|value| &value.encoding), &after.encoding) {
        *row.encoding = Some(after.encoding.label().into());
    }
    if differs(before.map(|value| &value.password_id), &after.password_id) {
        let password = after
            .password_id
            .as_deref()
            .map(|id| crate::host_password::resolve(database, id))
            .transpose()?;
        row.password.zeroize();
        *row.password = password
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
    }
    Ok(())
}
struct ProfileEdit<'a> {
    port: &'a mut Option<i64>,
    username: &'a mut Option<String>,
    password: &'a mut Option<String>,
    identity_id: &'a mut Option<i64>,
    identity_uuid: &'a mut Option<String>,
    key_id: &'a mut Option<i64>,
    key_uuid: &'a mut Option<String>,
    theme: &'a mut Option<String>,
    startup_id: &'a mut Option<i64>,
    startup_uuid: &'a mut Option<String>,
    environment: &'a mut Vec<nauterm_core::HostEnvironmentVariable>,
    encoding: &'a mut Option<String>,
}
macro_rules! native_profile {
    ($row:ty) => {
        impl NativeProfile for $row {
            fn mosh_view(&self) -> Result<Option<crate::mosh::Config>> {
                self.mosh_enabled
                    .unwrap_or(false)
                    .then(|| crate::mosh::Config::new(self.mosh_server_command.as_deref()))
                    .transpose()
            }
            fn patch_mosh(
                &mut self,
                before: Option<&crate::mosh::Config>,
                after: Option<&crate::mosh::Config>,
            ) -> Result<()> {
                if before != after {
                    if let Some(config) = after {
                        config.validate()?;
                    }
                    self.mosh_enabled = Some(after.is_some());
                    self.mosh_server_command = after.map(|config| config.command.clone());
                }
                Ok(())
            }
            fn telnet_view(&self, reference: String) -> Result<Option<crate::telnet::Profile>> {
                if !self.telnet_enabled.unwrap_or(false) {
                    return Ok(None);
                }
                let mut profile = crate::telnet::Profile::project(
                    format!("{reference}:telnet"),
                    self.telnet_port,
                    self.telnet_username.as_deref(),
                    self.telnet_identity_id,
                    self.telnet_theme_id.as_deref(),
                    self.telnet_encoding.as_deref(),
                )?;
                profile.password_id = self
                    .telnet_password
                    .as_ref()
                    .filter(|value| !value.is_empty())
                    .map(|_| profile.source.clone());
                Ok(Some(profile))
            }
            fn view(&self, reference: String) -> Result<Profile> {
                let credential = self
                    .identity_id
                    .map(CredentialRef::Identity)
                    .or_else(|| self.key_id.map(CredentialRef::Key));
                Ok(Profile {
                    port: port(self.port)?,
                    username: if matches!(credential, Some(CredentialRef::Identity(_))) {
                        String::new()
                    } else {
                        self.username.clone().unwrap_or_default()
                    },
                    credential,
                    theme: self.theme_id.as_deref().and_then(ThemeId::from_id),
                    setup: Setup {
                        profile_source: Some(reference.clone()),
                        password_id: self
                            .password
                            .as_ref()
                            .filter(|value| !value.is_empty())
                            .map(|_| reference.clone()),
                        environment_id: (!self.environment_variables.is_empty())
                            .then_some(reference),
                        startup_snippet: self.startup_snippet_id.map(id).transpose()?,
                        encoding: self
                            .encoding
                            .as_deref()
                            .and_then(|value| {
                                Encoding::ALL
                                    .into_iter()
                                    .find(|encoding| encoding.label().eq_ignore_ascii_case(value))
                            })
                            .unwrap_or_default(),
                    },
                })
            }
            fn telnet_edit(&mut self) -> TelnetEdit<'_> {
                TelnetEdit {
                    enabled: &mut self.telnet_enabled,
                    port: &mut self.telnet_port,
                    username: &mut self.telnet_username,
                    password: &mut self.telnet_password,
                    identity_id: &mut self.telnet_identity_id,
                    identity_uuid: &mut self.telnet_identity_uuid,
                    theme: &mut self.telnet_theme_id,
                    encoding: &mut self.telnet_encoding,
                }
            }
            fn edit(&mut self) -> ProfileEdit<'_> {
                ProfileEdit {
                    port: &mut self.port,
                    username: &mut self.username,
                    password: &mut self.password,
                    identity_id: &mut self.identity_id,
                    identity_uuid: &mut self.identity_uuid,
                    key_id: &mut self.key_id,
                    key_uuid: &mut self.key_uuid,
                    theme: &mut self.theme_id,
                    startup_id: &mut self.startup_snippet_id,
                    startup_uuid: &mut self.startup_snippet_uuid,
                    environment: &mut self.environment_variables,
                    encoding: &mut self.encoding,
                }
            }
        }
    };
}
native_profile!(HostEntry);
native_profile!(HostGroup);

fn host_profile(host: &Host) -> Profile {
    Profile {
        port: host.port,
        username: host.username.clone(),
        credential: host.credential,
        theme: host.theme,
        setup: host.ssh.clone(),
    }
}
fn differs<T: PartialEq>(before: Option<&T>, after: &T) -> bool {
    before != Some(after)
}
fn patch_profile(
    database: &NautermDatabase,
    row: ProfileEdit<'_>,
    before: Option<&Profile>,
    after: &Profile,
) -> Result<()> {
    after.validate()?;
    if differs(before.map(|value| &value.port), &after.port) {
        *row.port = Some(i64::from(after.port));
    }
    if differs(before.map(|value| &value.credential), &after.credential) {
        match after.credential {
            Some(CredentialRef::Identity(id)) => {
                *row.identity_id = Some(id);
                *row.identity_uuid = None;
                // Keep a saved Host's hidden fallback. A new inherited profile
                // obtains its fallback from the shared source before publication.
                if before.is_none()
                    && let Some(source) = after
                        .setup
                        .profile_source
                        .as_deref()
                        .and_then(resource_reference::Reference::parse)
                {
                    let (username, key) = source.load(database)?.auth_fallback();
                    *row.username = username;
                    *row.key_id = key;
                    *row.key_uuid = None;
                }
            }
            value => {
                *row.identity_id = None;
                *row.identity_uuid = None;
                *row.key_id = match value {
                    Some(CredentialRef::Key(id)) => Some(id),
                    _ => None,
                };
                *row.key_uuid = None;
            }
        }
    }
    if !matches!(after.credential, Some(CredentialRef::Identity(_)))
        && differs(before.map(|value| &value.username), &after.username)
    {
        *row.username = (!after.username.is_empty()).then(|| after.username.clone());
    }
    if differs(before.map(|value| &value.theme), &after.theme) {
        *row.theme = after.theme.map(|theme| theme.id().to_owned());
    }
    if differs(
        before.map(|value| &value.setup.startup_snippet),
        &after.setup.startup_snippet,
    ) {
        *row.startup_id = after.setup.startup_snippet.map(i64::try_from).transpose()?;
        *row.startup_uuid = None;
    }
    if differs(
        before.map(|value| &value.setup.encoding),
        &after.setup.encoding,
    ) {
        *row.encoding = Some(after.setup.encoding.label().into());
    }
    if differs(
        before.map(|value| &value.setup.password_id),
        &after.setup.password_id,
    ) {
        let value = after
            .setup
            .password_id
            .as_deref()
            .map(|reference| crate::host_password::resolve(database, reference))
            .transpose()?;
        row.password.zeroize();
        *row.password = value
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
    }
    if differs(
        before.map(|value| &value.setup.environment_id),
        &after.setup.environment_id,
    ) {
        let values = after
            .setup
            .environment_id
            .as_deref()
            .map(|reference| crate::host_session::resolve_environment(database, reference))
            .transpose()?
            .unwrap_or_default();
        for variable in row.environment.iter_mut() {
            variable.value.zeroize();
        }
        *row.environment = values
            .into_iter()
            .map(|mut value| nauterm_core::HostEnvironmentVariable {
                variable: std::mem::take(&mut value.variable),
                value: std::mem::take(&mut value.value),
            })
            .collect();
    }
    Ok(())
}

impl Catalog {
    pub fn load(database: &NautermDatabase) -> Result<Self> {
        let tags = database.list_tags()?;
        let forwards = database.list_port_forwards(None)?;
        let next = database.next_catalog_id()?;
        let hosts = database
            .list_hosts(None)?
            .into_iter()
            .map(|row| SecretProfile::Host(Box::new(row)))
            .collect();
        let groups = database
            .list_groups()?
            .into_iter()
            .map(|row| SecretProfile::Group(Box::new(row)))
            .collect();
        Self::from_rows(hosts, groups, tags, forwards, next)
    }

    fn from_rows(
        hosts: Vec<SecretProfile>,
        groups: Vec<SecretProfile>,
        tags: Vec<TagEntry>,
        forwards: Vec<PortForwardEntry>,
        next: i64,
    ) -> Result<Self> {
        let mut catalog = Self {
            next_id: id(next)?,
            ..Default::default()
        };
        for row in tags {
            let stamp = Stamp::new(row.id, row.uuid.as_deref(), row.version)?;
            catalog.tags.push(Tag {
                id: stamp.uuid.clone(),
                name: row.name,
            });
            catalog.tags_at.insert(stamp.uuid.clone(), stamp);
        }
        for row in groups {
            let SecretProfile::Group(group) = &row else {
                unreachable!()
            };
            let stamp = Stamp::new(group.id, group.uuid.as_deref(), group.version)?;
            catalog.groups.push(Group {
                id: id(stamp.id)?,
                uuid: Some(stamp.uuid.clone()),
                name: group.name.clone(),
                parent: group.parent_id.map(id).transpose()?,
                proxy_id: group.proxy_id,
                ssh: group
                    .ssh_enabled
                    .unwrap_or(false)
                    .then(|| group.view(stamp.reference("group")))
                    .transpose()?,
                mosh: group.mosh_view()?,
                telnet: group.telnet_view(stamp.reference("group"))?,
            });
            catalog.groups_at.insert(id(stamp.id)?, stamp);
        }
        for row in hosts {
            let SecretProfile::Host(host) = &row else {
                unreachable!()
            };
            let stamp = Stamp::new(host.id, host.uuid.as_deref(), host.version)?;
            let profile = host.view(stamp.reference("host"))?;
            catalog.hosts.push(Host {
                id: id(stamp.id)?,
                uuid: Some(stamp.uuid.clone()),
                platform: crate::avatar::Platform {
                    os: host.os.clone(),
                    distro: host.distro.clone(),
                },
                name: host.name.clone(),
                kind: match host.host_type {
                    HostType::Remote => HostKind::Remote,
                    HostType::Local => HostKind::Local,
                },
                shell_path: host.shell_path.clone(),
                work_dir: host.work_dir.clone(),
                ssh: profile.setup,
                address: host.host.clone().unwrap_or_default(),
                port: profile.port,
                username: profile.username,
                credential: profile.credential,
                group_id: host.group_id.map(id).transpose()?,
                proxy_id: host.proxy_id,
                theme: profile.theme,
                tags: host
                    .tag_uuids
                    .iter()
                    .filter_map(|uuid| {
                        catalog
                            .tags
                            .iter()
                            .find(|tag| tag.id == *uuid)
                            .map(|tag| tag.name.clone())
                    })
                    .collect(),
                connections: crate::model::ConnectionProfiles {
                    ssh: host.ssh_enabled.unwrap_or(false),
                    mosh: host.mosh_enabled.unwrap_or(false),
                    mosh_command: host.mosh_view()?.unwrap_or_default().command,
                    telnet: host.telnet_view(stamp.reference("host"))?,
                },
            });
            catalog.hosts_at.insert(id(stamp.id)?, stamp);
        }
        for row in forwards {
            let stamp = Stamp::new(row.id, row.uuid.as_deref(), row.version)?;
            let kind = match row.r#type.as_str() {
                "local" => Kind::Local,
                "remote" => Kind::Remote,
                "dynamic" => Kind::Dynamic,
                _ => anyhow::bail!("Unsupported forwarding type; original rule was preserved"),
            };
            let host = catalog
                .hosts
                .iter()
                .find(|host| host.id == row.connection_id as Id);
            catalog.forwards.push(Forward {
                id: id(stamp.id)?,
                name: row.name,
                kind,
                host_id: host.map(|host| host.id),
                host: host
                    .map(|host| host.address.clone())
                    .unwrap_or_else(|| "Unavailable Host".into()),
                local_port: port(Some(row.bind_port))?,
                bind_address: row.bind_address,
                destination: if kind == Kind::Dynamic {
                    String::new()
                } else {
                    port_forward::endpoint(&row.destination_host, port(Some(row.destination_port))?)
                },
            });
            catalog.forwards_at.insert(id(stamp.id)?, stamp);
        }
        Ok(catalog)
    }

    pub fn apply(&self, state: &mut AppState) {
        state.hosts = self.hosts.clone();
        state.groups = self.groups.clone();
        state.tags = self.tags.clone();
        state.forwards = self.forwards.clone();
        state.next_id = state.next_id.max(self.next_id);
    }
    pub fn apply_host_platform(
        &mut self,
        target: &crate::host_platform::Target,
        change: &nauterm_core::HostPlatformChange,
    ) -> bool {
        let Some(host) = self.hosts.iter_mut().find(|host| target.matches(host)) else {
            return false;
        };
        let Some(stamp) = self
            .hosts_at
            .get_mut(&target.id)
            .filter(|stamp| stamp.uuid == target.uuid)
        else {
            return false;
        };
        if stamp
            .version
            .zip(change.version)
            .is_some_and(|(current, published)| current > published)
        {
            return false;
        }
        // Advance only our own unchanged baseline. External edits must still
        // conflict when an already open editor tries to publish its old draft.
        if stamp.version == change.previous_version {
            stamp.version = change.version;
        }
        host.platform = crate::avatar::Platform {
            os: Some(change.os.clone()),
            distro: change.distro.clone(),
        };
        true
    }

    /// A settings save must not silently rebase an open editor onto a newer
    /// external record. Retain its old view/stamp until explicit refresh, while
    /// publishing our edits and newly discovered rows immediately.
    pub fn after_save(&self, requested: &AppState, mut published: Self) -> Self {
        retain_stale(
            &self.hosts,
            &requested.hosts,
            &mut published.hosts,
            &self.hosts_at,
            &mut published.hosts_at,
            |value| value.id,
            same_host_configuration,
        );
        retain_stale(
            &self.groups,
            &requested.groups,
            &mut published.groups,
            &self.groups_at,
            &mut published.groups_at,
            |value| value.id,
            PartialEq::eq,
        );
        retain_stale(
            &self.tags,
            &requested.tags,
            &mut published.tags,
            &self.tags_at,
            &mut published.tags_at,
            |value| value.id.clone(),
            PartialEq::eq,
        );
        retain_stale(
            &self.forwards,
            &requested.forwards,
            &mut published.forwards,
            &self.forwards_at,
            &mut published.forwards_at,
            |value| value.id,
            PartialEq::eq,
        );
        published
    }

    /// Caller holds the native atomic edit, including this method's reload.
    pub fn persist(&self, database: &mut NautermDatabase, state: &AppState) -> Result<Self> {
        state.validate()?;
        let current = Self::load(database)?;
        if self.hosts.len() == state.hosts.len()
            && self
                .hosts
                .iter()
                .zip(&state.hosts)
                .all(|(before, after)| same_host_configuration(before, after))
            && self.groups == state.groups
            && self.tags == state.tags
            && self.forwards == state.forwards
        {
            return Ok(current);
        }
        check_changes(
            &self.hosts,
            &state.hosts,
            &self.hosts_at,
            &current.hosts_at,
            |value| value.id,
            same_host_configuration,
        )?;
        check_changes(
            &self.groups,
            &state.groups,
            &self.groups_at,
            &current.groups_at,
            |value| value.id,
            PartialEq::eq,
        )?;
        crate::model::validate_group_parent_changes(&self.groups, &state.groups, &current.groups)?;
        check_changes(
            &self.tags,
            &state.tags,
            &self.tags_at,
            &current.tags_at,
            |value| value.id.clone(),
            PartialEq::eq,
        )?;
        check_changes(
            &self.forwards,
            &state.forwards,
            &self.forwards_at,
            &current.forwards_at,
            |value| value.id,
            PartialEq::eq,
        )?;
        for value in &state.tags {
            let old = self.tags.iter().find(|old| old.id == value.id);
            if old == Some(value) {
                continue;
            }
            ensure!(
                resource_reference::valid_uuid(&value.id),
                "Shared tags require UUIDs; import the prototype catalog explicitly"
            );
            let mut row = if let Some(stamp) = self.tags_at.get(&value.id) {
                database.get_tag(stamp.id)?.context("The Tag was deleted")?
            } else {
                ensure!(
                    !current.tags_at.contains_key(&value.id),
                    "The Tag already exists; reload before saving"
                );
                serde_json::from_value(serde_json::json!({"uuid":value.id,"name":""}))?
            };
            row.name = value.name.clone();
            database.save_tag(&row)?;
        }
        // New parents must exist before their children resolve numeric IDs.
        let mut groups: Vec<_> = state.groups.iter().collect();
        groups.sort_by_key(|value| state.group_path(value.id).len());
        for value in groups {
            let old = self.groups.iter().find(|old| old.id == value.id);
            if old == Some(value) {
                continue;
            }
            let template = if old.is_none() {
                value.uuid.as_deref().and_then(|uuid| {
                    self.groups
                        .iter()
                        .find(|old| old.uuid.as_deref() == Some(uuid))
                })
            } else {
                old
            };
            let source = template.map(|old| old.id);
            let mut row = match source {
                Some(source) => {
                    check_stamp(self.groups_at.get(&source), current.groups_at.get(&source))?;
                    SecretProfile::Group(Box::new(
                        database
                            .get_group(source as i64)?
                            .context("The Group was deleted")?,
                    ))
                }
                None => {
                    ensure!(
                        value.uuid.is_none(),
                        "The source Group is unavailable; reload before saving"
                    );
                    SecretProfile::Group(Box::default())
                }
            };
            let SecretProfile::Group(group) = &mut row else {
                unreachable!()
            };
            if old.is_none() {
                clear_group_identity(group);
            }
            group.name = value.name.clone();
            if differs(template.map(|value| &value.parent), &value.parent) {
                group.parent_id = value.parent.map(i64::try_from).transpose()?;
                group.parent_uuid = None;
            }
            if differs(template.map(|value| &value.proxy_id), &value.proxy_id) {
                group.proxy_id = value.proxy_id;
                group.proxy_uuid = None;
            }
            if let Some(profile) = &value.ssh {
                patch_profile(
                    database,
                    group.edit(),
                    template.and_then(|value| value.ssh.as_ref()),
                    profile,
                )?;
            }
            if template.is_none()
                || template.is_some_and(|old| old.ssh.is_some() != value.ssh.is_some())
            {
                group.ssh_enabled = Some(value.ssh.is_some());
                if value.ssh.is_none() {
                    group.mosh_enabled = Some(false);
                }
            }
            group.patch_mosh(
                template.and_then(|value| value.mosh.as_ref()),
                value.mosh.as_ref(),
            )?;
            patch_telnet(
                database,
                group.telnet_edit(),
                template.and_then(|value| value.telnet.as_ref()),
                value.telnet.as_ref(),
            )?;
            if old.is_some() {
                database.save_group(group)?;
            } else {
                database.insert_catalog_at(value.id as i64, NewCatalogRecord::Group(group))?;
            }
        }
        for value in &state.hosts {
            let old = self.hosts.iter().find(|old| old.id == value.id);
            if old.is_some_and(|old| same_host_configuration(old, value)) {
                continue;
            }
            let template = if old.is_none() {
                value.uuid.as_deref().and_then(|uuid| {
                    self.hosts
                        .iter()
                        .find(|old| old.uuid.as_deref() == Some(uuid))
                })
            } else {
                old
            };
            let source = template.map(|old| old.id);
            let mut row = match source {
                Some(source) => {
                    check_stamp(self.hosts_at.get(&source), current.hosts_at.get(&source))?;
                    SecretProfile::Host(Box::new(
                        database
                            .get_host(source as i64)?
                            .context("The Host was deleted")?,
                    ))
                }
                None => {
                    ensure!(
                        value.uuid.is_none(),
                        "The source Host is unavailable; reload before saving"
                    );
                    SecretProfile::Host(Box::new(serde_json::from_value(
                        serde_json::json!({"name":"","identity_id":null,"type":"remote"}),
                    )?))
                }
            };
            let SecretProfile::Host(host) = &mut row else {
                unreachable!()
            };
            if old.is_none() {
                clear_host_identity(host);
            }
            host.name = value.name.clone();
            host.host_type = match value.kind {
                HostKind::Remote => HostType::Remote,
                HostKind::Local => HostType::Local,
            };
            if differs(template.map(|value| &value.address), &value.address) {
                host.host = (!value.address.is_empty()).then(|| value.address.clone());
            }
            if differs(template.map(|value| &value.shell_path), &value.shell_path) {
                host.shell_path = value.shell_path.clone();
            }
            if differs(template.map(|value| &value.work_dir), &value.work_dir) {
                host.work_dir = value.work_dir.clone();
            }
            if differs(template.map(|value| &value.group_id), &value.group_id) {
                host.group_id = value.group_id.map(i64::try_from).transpose()?;
                host.group_uuid = None;
            }
            if differs(template.map(|value| &value.proxy_id), &value.proxy_id) {
                host.proxy_id = value.proxy_id;
                host.proxy_uuid = None;
            }
            if template.is_none()
                || template.is_some_and(|old| {
                    old.kind != value.kind || old.connections.ssh != value.connections.ssh
                })
            {
                host.ssh_enabled = Some(value.supports_ssh());
                if !value.supports_ssh() {
                    host.mosh_enabled = Some(false);
                }
            }
            if value.supports_ssh() {
                let old_profile = template.map(host_profile);
                patch_profile(
                    database,
                    host.edit(),
                    old_profile.as_ref(),
                    &host_profile(value),
                )?;
            } else if !value.is_remote()
                && differs(template.map(|value| &value.theme), &value.theme)
            {
                host.theme_id = value.theme.map(|theme| theme.id().into());
            }
            let before_mosh = template
                .filter(|value| value.connections.mosh)
                .map(|value| crate::mosh::Config::new(Some(&value.connections.mosh_command)))
                .transpose()?;
            let after_mosh = value
                .connections
                .mosh
                .then(|| crate::mosh::Config::new(Some(&value.connections.mosh_command)))
                .transpose()?;
            host.patch_mosh(before_mosh.as_ref(), after_mosh.as_ref())?;
            patch_telnet(
                database,
                host.telnet_edit(),
                template.and_then(|value| value.connections.telnet.as_ref()),
                value.connections.telnet.as_ref(),
            )?;
            if differs(template.map(|value| &value.tags), &value.tags) {
                host.tag_uuids = value
                    .tags
                    .iter()
                    .map(|name| {
                        state
                            .tags
                            .iter()
                            .find(|tag| tag.name.eq_ignore_ascii_case(name))
                            .map(|tag| tag.id.clone())
                            .context("The Tag is unavailable")
                    })
                    .collect::<Result<_>>()?;
            }
            if old.is_some() {
                database.save_host(host)?;
            } else {
                database.insert_catalog_at(value.id as i64, NewCatalogRecord::Host(host))?;
            }
        }
        for value in &state.forwards {
            let old = self.forwards.iter().find(|old| old.id == value.id);
            if old == Some(value) {
                continue;
            }
            let spec = value.spec()?;
            let connection = value
                .host_id
                .context("Select an available SSH Host before saving the forwarding rule")?;
            ensure!(
                state
                    .hosts
                    .iter()
                    .any(|host| host.id == connection && host.is_remote()),
                "The forwarding Host is unavailable"
            );
            let mut row = match old {
                Some(_) => database
                    .get_port_forward(value.id as i64)?
                    .context("The forwarding rule was deleted")?,
                None => serde_json::from_value(
                    serde_json::json!({"name":"","type":"local","bind_address":"","bind_port":1,"destination_host":"","destination_port":0,"connection_id":connection}),
                )?,
            };
            row.name = value.name.clone();
            row.r#type = value.kind.as_str().into();
            row.bind_address = value.bind_address.clone();
            row.bind_port = i64::from(value.local_port);
            row.connection_id = i64::try_from(connection)?;
            row.host_uuid = None;
            (row.destination_host, row.destination_port) = spec
                .destination
                .map(|(host, port)| (host, i64::from(port)))
                .unwrap_or_default();
            if old.is_some() {
                database.save_port_forward(&row)?;
            } else {
                database.insert_catalog_at(value.id as i64, NewCatalogRecord::PortForward(&row))?;
            }
        }
        for old in &self.forwards {
            if !state.forwards.iter().any(|value| value.id == old.id) {
                database.delete_port_forward(old.id as i64)?;
            }
        }
        for old in &self.hosts {
            if !state.hosts.iter().any(|value| value.id == old.id) {
                database.delete_host(old.id as i64)?;
            }
        }
        for old in &self.groups {
            if !state.groups.iter().any(|value| value.id == old.id) {
                database.delete_group(old.id as i64)?;
            }
        }
        for old in &self.tags {
            if !state.tags.iter().any(|value| value.id == old.id) {
                database.delete_tag(self.tags_at[&old.id].id)?;
            }
        }
        Self::load(database)
    }
}

// OS/distro is a read-only projection. A probe may publish it while an editor
// still holds an older Host, without turning that draft into a resource edit.
fn same_host_configuration(before: &Host, after: &Host) -> bool {
    if before.platform == after.platform {
        return before == after;
    }
    let mut before = before.clone();
    before.platform = after.platform.clone();
    before == *after
}

fn retain_stale<T: Clone, K: Ord + Clone>(
    before: &[T],
    requested: &[T],
    published: &mut Vec<T>,
    old_stamps: &BTreeMap<K, Stamp>,
    new_stamps: &mut BTreeMap<K, Stamp>,
    key: impl Fn(&T) -> K,
    unchanged: impl Fn(&T, &T) -> bool,
) {
    for old in before {
        let id = key(old);
        if requested
            .iter()
            .find(|value| key(value) == id)
            .is_some_and(|value| unchanged(old, value))
            && old_stamps.get(&id) != new_stamps.get(&id)
        {
            published.retain(|value| key(value) != id);
            published.push(old.clone());
            if let Some(stamp) = old_stamps.get(&id) {
                new_stamps.insert(id, stamp.clone());
            }
        }
    }
}
fn check_stamp(before: Option<&Stamp>, current: Option<&Stamp>) -> Result<()> {
    ensure!(
        before.is_some() && before == current,
        "This resource changed in another window. Close the editor and reload before saving."
    );
    Ok(())
}
fn check_changes<T, K: Ord>(
    before: &[T],
    after: &[T],
    stamps: &BTreeMap<K, Stamp>,
    current: &BTreeMap<K, Stamp>,
    key: impl Fn(&T) -> K,
    unchanged: impl Fn(&T, &T) -> bool,
) -> Result<()> {
    for old in before {
        let id = key(old);
        if !after
            .iter()
            .find(|value| key(value) == id)
            .is_some_and(|value| unchanged(old, value))
        {
            check_stamp(stamps.get(&id), current.get(&id))?;
        }
    }
    Ok(())
}
fn clear_host_identity(row: &mut HostEntry) {
    row.id = None;
    row.uuid = None;
    row.created_at = None;
    row.updated_at = None;
    row.deleted_at = None;
    row.version = None;
    row.created_device_id = None;
    row.updated_device_id = None;
}
fn clear_group_identity(row: &mut HostGroup) {
    row.id = None;
    row.uuid = None;
    row.created_at = None;
    row.updated_at = None;
    row.deleted_at = None;
    row.version = None;
    row.created_device_id = None;
    row.updated_device_id = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platform_catalog() -> (Catalog, crate::host_platform::Target) {
        let host = Host {
            id: 1,
            uuid: Some("12345678-1234-4000-8000-123456789abc".into()),
            name: "Saved Host".into(),
            address: "example.invalid".into(),
            ..Default::default()
        };
        let target = crate::host_platform::Target::for_host(&host).unwrap();
        let stamp = Stamp {
            id: 1,
            uuid: target.uuid.clone(),
            version: Some(2),
        };
        (
            Catalog {
                hosts: vec![host],
                hosts_at: BTreeMap::from([(1, stamp)]),
                ..Default::default()
            },
            target,
        )
    }

    #[test]
    fn platform_publication_does_not_turn_an_open_draft_into_a_conflicting_edit() -> Result<()> {
        let (mut catalog, target) = platform_catalog();
        let mut draft = AppState::default();
        catalog.apply(&mut draft);
        assert!(catalog.apply_host_platform(
            &target,
            &nauterm_core::HostPlatformChange {
                previous_version: Some(2),
                version: Some(3),
                os: "Linux".into(),
                distro: Some("Ubuntu".into()),
            }
        ));
        assert_eq!(catalog.hosts_at[&1].version, Some(3));
        assert!(same_host_configuration(&catalog.hosts[0], &draft.hosts[0]));
        draft.hosts[0].name = "My pending edit".into();
        assert!(!same_host_configuration(&catalog.hosts[0], &draft.hosts[0]));
        check_changes(
            &catalog.hosts,
            &draft.hosts,
            &catalog.hosts_at,
            &catalog.hosts_at,
            |host| host.id,
            same_host_configuration,
        )?;
        assert_eq!(catalog.hosts[0].platform.distro.as_deref(), Some("Ubuntu"));
        Ok(())
    }

    #[test]
    fn platform_results_keep_external_edit_conflicts_and_reject_obsolete_targets() {
        let (mut catalog, target) = platform_catalog();
        let mut draft = AppState::default();
        catalog.apply(&mut draft);
        let mut change = nauterm_core::HostPlatformChange {
            previous_version: Some(4),
            version: Some(5),
            os: "Linux".into(),
            distro: None,
        };
        assert!(catalog.apply_host_platform(&target, &change));
        // An external configuration edit happened between loading and probing.
        assert_eq!(catalog.hosts_at[&1].version, Some(2));
        let mut current = catalog.clone();
        current.hosts[0].name = "External edit".into();
        current.hosts_at.get_mut(&1).unwrap().version = Some(5);
        let retained = catalog.after_save(&draft, current.clone());
        assert_eq!(retained.hosts_at[&1].version, Some(2));
        assert_eq!(retained.hosts[0].name, "Saved Host");
        assert_eq!(retained.hosts[0].platform.os.as_deref(), Some("Linux"));
        draft.hosts[0].name = "Stale pending edit".into();
        assert!(
            check_changes(
                &retained.hosts,
                &draft.hosts,
                &retained.hosts_at,
                &current.hosts_at,
                |host| host.id,
                same_host_configuration,
            )
            .is_err()
        );
        let mut other_endpoint = target.clone();
        other_endpoint.port = 2022;
        assert!(!catalog.apply_host_platform(&other_endpoint, &change));
        other_endpoint = target.clone();
        other_endpoint.uuid = "12345678-1234-4000-8000-123456789abd".into();
        assert!(!catalog.apply_host_platform(&other_endpoint, &change));
        change.version = Some(3);
        change.os = "Stale result".into();
        assert!(!current.apply_host_platform(&target, &change));
        assert_eq!(current.hosts[0].platform.os.as_deref(), Some("Linux"));
    }

    #[test]
    fn projections_preserve_table_ids_and_keep_secret_values_out_of_public_state() -> Result<()> {
        let uuid = "12345678-1234-1234-1234-123456789abc";
        let host: HostEntry = serde_json::from_value(
            serde_json::json!({"id":1,"uuid":uuid,"name":"SSH","type":"remote","identity_id":null,"host":"example.test","ssh_enabled":true,"mosh_enabled":true,"mosh_server_command":"/opt/mosh-server new -s","password":"SYNTHETIC_PASSWORD","environment_variables":[{"variable":"TOKEN","value":"SYNTHETIC_ENVIRONMENT"}],"telnet_enabled":true,"telnet_port":2323,"telnet_username":"telnet-user","telnet_encoding":"GBK","telnet_password":"SYNTHETIC_TELNET"}),
        )?;
        let group = HostGroup {
            id: Some(1),
            uuid: Some(uuid.into()),
            name: "Group".into(),
            mosh_enabled: Some(true),
            mosh_server_command: Some("/group/mosh-server new -s".into()),
            telnet_enabled: Some(true),
            telnet_port: Some(2324),
            telnet_identity_id: Some(7),
            telnet_password: Some("SYNTHETIC_GROUP_TELNET".into()),
            ..Default::default()
        };
        let catalog = Catalog::from_rows(
            vec![SecretProfile::Host(Box::new(host))],
            vec![SecretProfile::Group(Box::new(group))],
            vec![],
            vec![],
            2,
        )?;
        assert_eq!(catalog.hosts[0].id, catalog.groups[0].id);
        assert!(catalog.hosts[0].supports_ssh());
        assert!(catalog.hosts[0].connections.mosh);
        assert_eq!(
            catalog.hosts[0].connections.mosh_command,
            "/opt/mosh-server new -s"
        );
        assert_eq!(
            catalog.groups[0].editor_host().connections.mosh_command,
            "/group/mosh-server new -s"
        );
        let telnet = catalog.hosts[0].connections.telnet.as_ref().unwrap();
        assert_eq!(telnet.port, 2323);
        assert_eq!(telnet.username, "telnet-user");
        assert_eq!(telnet.encoding, crate::host_session::Encoding::Gbk);
        assert_eq!(telnet.source, format!("native:host:1:{uuid}:telnet"));
        assert!(
            catalog.hosts[0]
                .ssh
                .password_id
                .as_deref()
                .is_some_and(|value| value.starts_with("native:host:1:"))
        );
        assert_eq!(catalog.groups[0].telnet.as_ref().unwrap().port, 2324);
        let group_host = catalog.groups[0].editor_host();
        assert!(!group_host.supports_ssh());
        assert_eq!(
            group_host.connections.telnet.as_ref().unwrap().identity_id,
            Some(7)
        );
        let mut state = AppState::default();
        catalog.apply(&mut state);
        state.validate()?;
        let json = serde_json::to_string(&state)?;
        assert!(!json.contains("SYNTHETIC_"));
        let expected = Stamp {
            id: 1,
            uuid: uuid.into(),
            version: Some(2),
        };
        let mut newer = expected.clone();
        newer.version = Some(3);
        assert!(check_stamp(Some(&expected), Some(&newer)).is_err());
        assert!(check_stamp(Some(&expected), None).is_err());
        let mut concurrent = catalog.clone();
        concurrent.hosts[0].name = "External edit".into();
        concurrent.hosts_at.get_mut(&1).unwrap().version = Some(3);
        let retained = catalog.after_save(&state, concurrent);
        assert_eq!(retained.hosts[0].name, "SSH");
        assert_eq!(retained.hosts_at[&1], catalog.hosts_at[&1]);
        Ok(())
    }
}
