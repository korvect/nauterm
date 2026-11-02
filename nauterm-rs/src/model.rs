use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub type Id = u64;
pub const GROUP_DEPTH_ERROR: &str = "Groups can only be nested two levels deep.";

fn group_can_be_nested(groups: &[Group], group: Option<Id>) -> bool {
    group.is_none_or(|id| {
        groups.iter().any(|group| group.id == id)
            && !groups.iter().any(|group| group.parent == Some(id))
    })
}

fn group_parent_allowed(groups: &[Group], group: Option<Id>, parent: Id) -> bool {
    group != Some(parent)
        && groups
            .iter()
            .any(|group| group.id == parent && group.parent.is_none())
        && group_can_be_nested(groups, group)
}

/// Check only new/reparented Groups against the current shared hierarchy.
/// Existing deeper catalogs remain readable and unrelated edits remain possible.
pub(crate) fn validate_group_parent_changes(
    before: &[Group],
    requested: &[Group],
    current: &[Group],
) -> anyhow::Result<()> {
    let changes: Vec<_> = requested
        .iter()
        .filter(|group| {
            before
                .iter()
                .find(|old| old.id == group.id)
                .is_none_or(|old| old.parent != group.parent)
        })
        .collect();
    if changes.is_empty() {
        return Ok(());
    }
    let mut merged: Vec<_> = current
        .iter()
        .filter(|group| {
            !before.iter().any(|old| old.id == group.id)
                || requested.iter().any(|next| next.id == group.id)
        })
        .cloned()
        .collect();
    for &next in &changes {
        if let Some(group) = merged.iter_mut().find(|group| group.id == next.id) {
            group.parent = next.parent;
        } else {
            merged.push(next.clone());
        }
    }
    for group in changes {
        if let Some(parent) = group.parent {
            anyhow::ensure!(
                group_parent_allowed(&merged, Some(group.id), parent),
                GROUP_DEPTH_ERROR
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Page {
    #[default]
    Hosts,
    Keychain,
    Proxies,
    PortForwards,
    Snippets,
    KnownHosts,
    Logs,
    Sftp,
    Sessions,
}

impl Page {
    pub const SIDEBAR: [Self; 7] = [
        Self::Hosts,
        Self::Keychain,
        Self::Proxies,
        Self::PortForwards,
        Self::Snippets,
        Self::KnownHosts,
        Self::Logs,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hosts => "Hosts",
            Self::Keychain => "Keychain",
            Self::Proxies => "Proxies",
            Self::PortForwards => "Port Forwarding",
            Self::Snippets => "Snippets",
            Self::KnownHosts => "Known Hosts",
            Self::Logs => "Logs",
            Self::Sftp => "SFTP",
            Self::Sessions => "Sessions",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    Host,
    Group,
    Snippet,
    SnippetPackage,
    Forward,
    Proxy,
}

impl ItemKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Host => "Host",
            Self::Group => "Group",
            Self::Snippet => "Snippet",
            Self::SnippetPackage => "Snippet Package",
            Self::Forward => "Port Forward",
            Self::Proxy => "Proxy",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    #[default]
    Remote,
    Local,
}
impl HostKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Remote => "Remote",
            Self::Local => "Local",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    pub id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Read-only projection of the shared native Host's detected platform.
    #[serde(skip)]
    pub platform: crate::avatar::Platform,
    pub name: String,
    #[serde(default, rename = "type")]
    pub kind: HostKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_dir: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::host_session::Setup::is_default"
    )]
    pub ssh: crate::host_session::Setup,
    pub address: String,
    pub port: u16,
    pub username: String,
    pub group_id: Option<Id>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub credential: Option<CredentialRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_id: Option<i64>,
    /// None follows the application terminal theme; persist only a resource ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<crate::terminal_appearance::ThemeId>,
    /// Native protocol availability, refreshed with the shared catalog. This
    /// projection does not become a second owner of protocol configuration.
    #[serde(skip)]
    pub connections: ConnectionProfiles,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionProfiles {
    pub ssh: bool,
    pub mosh: bool,
    pub mosh_command: String,
    pub telnet: Option<crate::telnet::Profile>,
}
impl Default for ConnectionProfiles {
    fn default() -> Self {
        Self {
            ssh: true,
            mosh: false,
            mosh_command: crate::mosh::DEFAULT_COMMAND.into(),
            telnet: None,
        }
    }
}
impl Host {
    pub fn is_remote(&self) -> bool {
        self.kind == HostKind::Remote
    }
    pub fn supports_ssh(&self) -> bool {
        self.is_remote() && self.connections.ssh
    }
    pub fn protocol_summary(&self) -> String {
        if !self.is_remote() {
            return "local".into();
        }
        [
            self.connections.ssh.then_some("ssh"),
            self.connections.mosh.then_some("mosh"),
            self.connections.telnet.is_some().then_some("telnet"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ")
    }
    pub fn validate_endpoint(&self) -> anyhow::Result<()> {
        use anyhow::ensure;
        if self.is_remote() {
            if self.connections.ssh {
                self.ssh.validate()?;
            }
            if self.connections.mosh {
                crate::mosh::Config::new(Some(&self.connections.mosh_command))?;
            }
            if let Some(profile) = &self.connections.telnet {
                profile.validate()?;
            }
            ensure!(
                (!self.connections.ssh || self.port != 0) && !self.address.trim().is_empty(),
                "Invalid remote host {}",
                self.name
            );
        } else {
            ensure!(
                self.shell_path
                    .as_deref()
                    .is_none_or(|shell| std::path::Path::new(shell).is_absolute()
                        && !shell.chars().any(char::is_control)),
                "Shell must be an absolute executable path."
            );
            crate::shells::validate_directory(self.work_dir.as_deref())?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRef {
    Identity(i64),
    Key(i64),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub name: String,
    pub parent: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<crate::host_session::Profile>,
    #[serde(skip)]
    pub telnet: Option<crate::telnet::Profile>,
    #[serde(skip)]
    pub mosh: Option<crate::mosh::Config>,
}
impl Group {
    pub fn profile(&self) -> Option<crate::host_session::Profile> {
        self.ssh.clone()
    }
    pub fn editor_host(&self) -> Host {
        let profile = self.profile().unwrap_or_default();
        Host {
            id: self.id,
            uuid: self.uuid.clone(),
            name: self.name.clone(),
            group_id: self.parent,
            port: profile.port,
            username: profile.username,
            credential: profile.credential,
            theme: profile.theme,
            proxy_id: self.proxy_id,
            ssh: profile.setup,
            connections: ConnectionProfiles {
                ssh: self.ssh.is_some(),
                mosh: self.mosh.is_some(),
                mosh_command: self
                    .mosh
                    .as_ref()
                    .map(|config| config.command.clone())
                    .unwrap_or_else(|| crate::mosh::DEFAULT_COMMAND.into()),
                telnet: self.telnet.clone(),
            },
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnippetScope {
    #[default]
    Global,
    Targeted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snippet {
    pub id: Id,
    #[serde(default)]
    pub scope: SnippetScope,
    pub name: String,
    pub command: String,
    pub description: String,
    #[serde(default)]
    pub target_group_ids: BTreeSet<Id>,
    #[serde(default)]
    pub target_host_ids: BTreeSet<Id>,
    #[serde(default)]
    pub package_id: Option<Id>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forward {
    pub id: Id,
    pub name: String,
    pub host: String,
    pub local_port: u16,
    pub destination: String,
    #[serde(default)]
    pub kind: crate::port_forward::Kind,
    #[serde(default)]
    pub host_id: Option<Id>,
    #[serde(default = "forward_bind_address")]
    pub bind_address: String,
}
fn forward_bind_address() -> String {
    "127.0.0.1".into()
}
impl Forward {
    pub fn matches_query(
        &self,
        query: &str,
        host: &str,
        status: Option<&crate::port_forward::Status>,
    ) -> bool {
        let state = status.map_or(crate::port_forward::State::Stopped, |status| status.state);
        let text = format!(
            "{} {} {} {} {} {} {} {} {}",
            self.name,
            self.kind.label(),
            self.bind_address,
            self.local_port,
            self.destination,
            host,
            state.label(),
            if state.active() {
                "enabled active"
            } else {
                "disabled"
            },
            status
                .and_then(|status| status.error.as_deref())
                .unwrap_or_default(),
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|word| text.contains(&word.to_lowercase()))
    }
    pub fn spec(&self) -> anyhow::Result<crate::port_forward::Spec> {
        use crate::port_forward::{Kind, Spec, parse_destination};
        let spec = Spec {
            id: self.id,
            kind: self.kind,
            bind_address: self.bind_address.clone(),
            bind_port: self.local_port,
            destination: if self.kind == Kind::Dynamic {
                None
            } else {
                Some(parse_destination(&self.destination)?)
            },
        };
        spec.validate()?;
        Ok(spec)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Early RS address-only metadata, retained until encrypted proxy migration is
/// published. Live proxy management uses the native repository, not this type.
pub struct Proxy {
    pub id: Id,
    pub name: String,
    pub address: String,
    pub port: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Id,
    pub name: String,
    pub color: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreBehavior {
    #[default]
    Ask,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppTheme {
    Light,
    #[default]
    System,
    Dark,
}
impl AppTheme {
    pub fn dark(self, appearance: gpui_kit::WindowAppearance) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => matches!(
                appearance,
                gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark
            ),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub schema: u32,
    pub next_id: Id,
    pub selected_workspace: Id,
    pub workspaces: Vec<Workspace>,
    pub hosts: Vec<Host>,
    pub tags: Vec<crate::tags::Tag>,
    pub groups: Vec<Group>,
    #[serde(skip)]
    pub snippets: Vec<Snippet>,
    /// Only early RS metadata, never original Nauterm data. Preserved until
    /// encrypted import and metadata publication both succeed.
    #[serde(rename = "snippets", skip_serializing_if = "Vec::is_empty")]
    pub legacy_snippets: Vec<Snippet>,
    pub forwards: Vec<Forward>,
    pub proxies: Vec<Proxy>,
    pub dark: bool,
    /// None migrates early RS metadata's explicit `dark` preference.
    #[serde(default)]
    pub theme: Option<AppTheme>,
    pub host_icons: crate::avatar::Mode,
    pub shortcuts: crate::shortcuts::Config,
    pub general: crate::general_preferences::Preferences,
    pub restore_behavior: RestoreBehavior,
    pub terminal: crate::terminal_appearance::Appearance,
    pub terminal_preferences: crate::terminal_preferences::Preferences,
    pub sftp_editors: crate::editor_config::Editors,
    pub sftp: crate::sftp_preferences::Preferences,
    pub recordings: crate::recordings::Preferences,
    pub ai: crate::ai::Preferences,
    pub sync: crate::sync::Preferences,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            schema: 1,
            next_id: 2,
            selected_workspace: 1,
            workspaces: vec![Workspace {
                id: 1,
                name: "Default".into(),
                color: 0x075e92,
            }],
            hosts: vec![],
            tags: vec![],
            groups: vec![],
            snippets: vec![],
            legacy_snippets: vec![],
            forwards: vec![],
            proxies: vec![],
            dark: false,
            theme: Some(AppTheme::System),
            host_icons: Default::default(),
            shortcuts: Default::default(),
            general: Default::default(),
            restore_behavior: RestoreBehavior::Ask,
            terminal: Default::default(),
            terminal_preferences: Default::default(),
            sftp_editors: Default::default(),
            sftp: Default::default(),
            recordings: Default::default(),
            ai: Default::default(),
            sync: Default::default(),
        }
    }
}

impl AppState {
    /// Only confirmed deletions, never a possibly stale/incomplete inventory.
    pub fn clear_proxy_references(&mut self, deleted: &BTreeSet<i64>) -> usize {
        let mut changed = 0;
        for reference in self
            .hosts
            .iter_mut()
            .map(|host| &mut host.proxy_id)
            .chain(self.groups.iter_mut().map(|group| &mut group.proxy_id))
        {
            if reference.is_some_and(|id| deleted.contains(&id)) {
                *reference = None;
                changed += 1;
            }
        }
        changed
    }
    pub fn password_in_use(&self, id: &str) -> bool {
        self.hosts
            .iter()
            .any(|host| host.ssh.password_id.as_deref() == Some(id))
            || self.groups.iter().any(|group| {
                group
                    .ssh
                    .as_ref()
                    .and_then(|profile| profile.setup.password_id.as_deref())
                    == Some(id)
            })
    }
    pub fn environment_in_use(&self, id: &str) -> bool {
        self.hosts
            .iter()
            .any(|host| host.ssh.environment_id.as_deref() == Some(id))
            || self.groups.iter().any(|group| {
                group
                    .ssh
                    .as_ref()
                    .and_then(|profile| profile.setup.environment_id.as_deref())
                    == Some(id)
            })
    }
    pub fn terminal_for_host(&self, id: Option<Id>) -> crate::terminal_appearance::Appearance {
        let mut appearance = self.terminal.clone();
        if let Some(theme) = id
            .and_then(|id| self.hosts.iter().find(|host| host.id == id))
            .and_then(|host| host.theme)
        {
            appearance.select_theme(theme);
        }
        appearance
    }
    pub fn app_theme(&self) -> AppTheme {
        self.theme.unwrap_or(if self.dark {
            AppTheme::Dark
        } else {
            AppTheme::Light
        })
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        crate::tags::validate(&self.tags)?;
        use anyhow::ensure;
        self.terminal.validate()?;
        self.general.validate()?;
        self.terminal_preferences.validate()?;
        self.sftp_editors.validate()?;
        self.sftp.validate()?;
        self.recordings.validate()?;
        self.sync.validate()?;
        for snippet in &self.snippets {
            snippet.validate()?;
        }
        for snippet in &self.legacy_snippets {
            snippet.validate()?;
        }
        ensure!(
            self.schema == 1,
            "Unsupported workspace metadata version {}",
            self.schema
        );
        ensure!(
            !self.workspaces.is_empty(),
            "At least one workspace is required"
        );
        ensure!(
            self.workspaces
                .iter()
                .all(|workspace| !workspace.name.trim().is_empty()),
            "Workspace name is required."
        );
        let collections = [
            self.workspaces
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            self.hosts.iter().map(|item| item.id).collect(),
            self.groups.iter().map(|item| item.id).collect(),
            self.legacy_snippets.iter().map(|item| item.id).collect(),
            self.proxies.iter().map(|item| item.id).collect(),
            self.forwards.iter().map(|item| item.id).collect(),
        ];
        let mut maximum = 0;
        for collection in collections {
            let mut ids = BTreeSet::new();
            for id in collection {
                ensure!(
                    id != 0 && ids.insert(id),
                    "Invalid or duplicate metadata id {id}"
                );
                maximum = maximum.max(id);
            }
        }
        ensure!(
            self.next_id > maximum && self.next_id < u64::MAX,
            "Invalid next metadata id"
        );
        ensure!(
            self.workspaces
                .iter()
                .any(|w| w.id == self.selected_workspace),
            "Active workspace does not exist"
        );
        for host in &self.hosts {
            ensure!(
                host.proxy_id.is_none_or(|id| id > 0),
                "Invalid host proxy reference"
            );
            ensure!(
                host.credential.is_none_or(|reference| match reference {
                    CredentialRef::Identity(id) | CredentialRef::Key(id) => id > 0,
                }),
                "Invalid saved credential reference"
            );
            host.validate_endpoint()?;
            ensure!(
                host.group_id
                    .is_none_or(|id| self.groups.iter().any(|g| g.id == id)),
                "Host references a missing group"
            );
        }
        for group in &self.groups {
            if let Some(config) = &group.mosh {
                config.validate()?;
            }
            if let Some(profile) = &group.telnet {
                profile.validate()?;
            }
            if let Some(profile) = &group.ssh {
                profile.validate()?;
            }
            ensure!(
                group.proxy_id.is_none_or(|id| id > 0),
                "Invalid group proxy reference"
            );
            let mut visited = BTreeSet::from([group.id]);
            let mut parent = group.parent;
            while let Some(id) = parent {
                ensure!(visited.insert(id), "Group hierarchy contains a cycle");
                parent = self
                    .groups
                    .iter()
                    .find(|g| g.id == id)
                    .ok_or_else(|| anyhow::anyhow!("Group references a missing parent"))?
                    .parent;
            }
        }
        ensure!(
            self.proxies
                .iter()
                .all(|p| p.port != 0 && !p.address.trim().is_empty()),
            "Invalid proxy"
        );
        ensure!(
            self.forwards
                .iter()
                .all(|f| f.local_port != 0 && !f.host.trim().is_empty()),
            "Invalid port forward"
        );
        for forward in &self.forwards {
            // Legacy address-only rules remain editable. New ID-bound rules
            // are validated fully; a deleted host is not silently substituted.
            if let Some(host) = forward.host_id {
                ensure!(
                    host > 0 && !forward.name.trim().is_empty(),
                    "Invalid forwarding host or name"
                );
                forward.spec()?;
            }
        }
        Ok(())
    }

    pub fn allocate_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn workspace(&self) -> &Workspace {
        self.workspaces
            .iter()
            .find(|w| w.id == self.selected_workspace)
            .unwrap_or(&self.workspaces[0])
    }

    pub fn add_workspace(&mut self, name: &str) -> anyhow::Result<Id> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "Workspace name is required.");
        let id = self.allocate_id();
        self.workspaces.push(Workspace {
            id,
            name: name.into(),
            color: 0x075e92,
        });
        self.selected_workspace = id;
        Ok(id)
    }
    pub fn rename_workspace(&mut self, id: Id, name: &str) -> anyhow::Result<()> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "Workspace name is required.");
        let workspace = self
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
            .ok_or_else(|| anyhow::anyhow!("Workspace is no longer available."))?;
        workspace.name = name.into();
        Ok(())
    }
    pub fn close_workspace(&mut self, id: Id) -> anyhow::Result<()> {
        let index = self
            .workspaces
            .iter()
            .position(|workspace| workspace.id == id)
            .ok_or_else(|| anyhow::anyhow!("Workspace is no longer available."))?;
        self.workspaces.remove(index);
        if self.workspaces.is_empty() {
            self.add_workspace("Default")?;
        }
        if self.selected_workspace == id {
            self.selected_workspace = self.workspaces[index.min(self.workspaces.len() - 1)].id;
        }
        Ok(())
    }

    pub fn group_path(&self, id: Id) -> Vec<&Group> {
        let mut result = vec![];
        let mut next = Some(id);
        let mut seen = BTreeSet::new();
        while let Some(id) = next.filter(|id| seen.insert(*id)) {
            let Some(group) = self.groups.iter().find(|group| group.id == id) else {
                break;
            };
            result.push(group);
            next = group.parent;
        }
        result.reverse();
        result
    }
    pub fn group_parent_allowed(&self, group: Option<Id>, parent: Id) -> bool {
        group_parent_allowed(&self.groups, group, parent)
    }
    pub fn group_can_be_nested(&self, group: Option<Id>) -> bool {
        group_can_be_nested(&self.groups, group)
    }
    /// Duplicate public configuration only; credential references are reused,
    /// and running sessions/forwarding state are never cloned or started.
    pub fn duplicate(&mut self, kind: ItemKind, ids: &BTreeSet<Id>) -> anyhow::Result<Vec<Id>> {
        let mut candidate = self.clone();
        let mut created = vec![];
        for source in ids {
            let id = candidate.allocate_id();
            match kind {
                ItemKind::Host => {
                    let mut item = self
                        .hosts
                        .iter()
                        .find(|item| item.id == *source)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("The host is no longer available."))?;
                    item.id = id;
                    item.name.push_str(" copy");
                    candidate.hosts.push(item);
                }
                ItemKind::Group => {
                    let mut item = self
                        .groups
                        .iter()
                        .find(|item| item.id == *source)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("The group is no longer available."))?;
                    item.id = id;
                    item.name.push_str(" copy");
                    candidate.groups.push(item);
                }
                ItemKind::Proxy => {
                    let mut item = self
                        .proxies
                        .iter()
                        .find(|item| item.id == *source)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("The proxy is no longer available."))?;
                    item.id = id;
                    item.name.push_str(" copy");
                    candidate.proxies.push(item);
                }
                ItemKind::Forward => {
                    let mut item = self
                        .forwards
                        .iter()
                        .find(|item| item.id == *source)
                        .cloned()
                        .ok_or_else(|| {
                            anyhow::anyhow!("The forwarding rule is no longer available.")
                        })?;
                    item.id = id;
                    item.name.push_str(" copy");
                    candidate.forwards.push(item);
                }
                ItemKind::Snippet | ItemKind::SnippetPackage => {
                    anyhow::bail!("Snippets use their encrypted catalog.")
                }
            }
            created.push(id);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(created)
    }

    pub fn delete(&mut self, kind: ItemKind, ids: &BTreeSet<Id>) {
        match kind {
            ItemKind::Host => {
                self.hosts.retain(|host| !ids.contains(&host.id));
                self.forwards
                    .retain(|rule| rule.host_id.is_none_or(|id| !ids.contains(&id)));
            }
            ItemKind::Group => {
                // Ungroup children rather than silently deleting connections.
                for host in &mut self.hosts {
                    if host.group_id.is_some_and(|id| ids.contains(&id)) {
                        host.group_id = None;
                    }
                }
                for group in &mut self.groups {
                    if group.parent.is_some_and(|id| ids.contains(&id)) {
                        group.parent = None;
                    }
                }
                self.groups.retain(|g| !ids.contains(&g.id));
            }
            ItemKind::Snippet => self.snippets.retain(|s| !ids.contains(&s.id)),
            ItemKind::SnippetPackage => unreachable!("Packages belong to the encrypted RS catalog"),
            ItemKind::Forward => self.forwards.retain(|f| !ids.contains(&f.id)),
            ItemKind::Proxy => self.proxies.retain(|p| !ids.contains(&p.id)),
        }
    }

    pub fn host_matches(&self, host: &Host, query: &str) -> bool {
        query.split_whitespace().all(|token| {
            let token = token.to_lowercase();
            let (field, value) = token.split_once(':').unwrap_or(("", &token));
            let group = host
                .group_id
                .and_then(|id| self.groups.iter().find(|g| g.id == id))
                .map(|g| g.name.as_str())
                .unwrap_or("");
            let contains = |text: &str| text.to_lowercase().contains(value);
            if field.is_empty()
                && let Some((user, address)) = token.split_once('@')
            {
                return host.username.to_lowercase().contains(user)
                    && (host.address.to_lowercase().contains(address)
                        || host.name.to_lowercase().contains(address));
            }
            match field {
                "type" => contains(host.kind.label()),
                "host" => contains(&host.address) || contains(&host.name),
                "username" | "user" => contains(&host.username),
                "protocol" => contains(&host.protocol_summary()),
                "group" => contains(group),
                "tag" => host.tags.iter().any(|tag| contains(tag)),
                _ => format!(
                    "{} {} {} {} {} {} {} {} {} {}",
                    host.name,
                    host.address,
                    host.username,
                    group,
                    host.tags.join(" "),
                    host.kind.label(),
                    host.shell_path.as_deref().unwrap_or_default(),
                    host.work_dir.as_deref().unwrap_or_default(),
                    host.protocol_summary(),
                    host.connections
                        .telnet
                        .as_ref()
                        .map(|profile| profile.username.as_str())
                        .unwrap_or_default()
                )
                .to_lowercase()
                .contains(&token),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn forward_search_combines_endpoint_host_runtime_state_and_errors() {
        use crate::port_forward::{Kind, State, Status};
        let rule = super::Forward {
            id: 7,
            name: "Production DB".into(),
            kind: Kind::Local,
            bind_address: "127.0.0.1".into(),
            local_port: 5433,
            destination: "database.internal:5432".into(),
            host: "Old host name".into(),
            host_id: Some(2),
        };
        assert!(rule.matches_query("  LOCAL\t5432  Bastion\nDISABLED ", "Bastion", None));
        assert!(!rule.matches_query("active", "Bastion", None));
        let running = Status::new(7, State::Running);
        assert!(rule.matches_query("active DB bastion", "Bastion", Some(&running)));
        assert!(!rule.matches_query("disabled", "Bastion", Some(&running)));
        let failed = Status::failed(7, "Connection timed out");
        assert!(rule.matches_query("ERROR timed DB", "Bastion", Some(&failed)));
        assert!(!rule.matches_query("Old host name", "Bastion", Some(&failed)));
    }

    #[test]
    fn local_host_config_roundtrips_duplicates_and_legacy_hosts_remain_remote() -> anyhow::Result<()>
    {
        let old: Host = serde_json::from_str(
            r#"{"id":2,"name":"Legacy","address":"fixture.invalid","port":22,"username":"fixture","group_id":null}"#,
        )?;
        assert!(old.is_remote());
        assert_eq!(serde_json::to_value(&old)?["type"], "remote");
        old.validate_endpoint()?;
        let directory = std::env::temp_dir()
            .join("nauterm-local-config-fixture-does-not-need-to-exist")
            .to_string_lossy()
            .into_owned();
        let shell = std::env::temp_dir()
            .join("synthetic-shell-not-executed")
            .to_string_lossy()
            .into_owned();
        let local = Host {
            id: 2,
            name: "Local fixture".into(),
            kind: HostKind::Local,
            shell_path: Some(shell.clone()),
            work_dir: Some(directory.clone()),
            ..Default::default()
        };
        local.validate_endpoint()?;
        let saved = serde_json::to_value(&local)?;
        assert_eq!(saved["type"], "local");
        assert_eq!(saved["shell_path"], shell);
        assert_eq!(saved["work_dir"], directory);
        assert_eq!(serde_json::from_value::<Host>(saved)?, local);
        let mut state = AppState {
            next_id: 3,
            hosts: vec![local.clone()],
            ..Default::default()
        };
        state.validate()?;
        assert!(state.host_matches(&local, "type:local fixture"));
        assert!(!state.host_matches(&old, "type:local"));
        let created = state.duplicate(ItemKind::Host, &BTreeSet::from([local.id]))?;
        let copy = state
            .hosts
            .iter()
            .find(|host| host.id == created[0])
            .unwrap();
        assert_eq!(copy.kind, HostKind::Local);
        assert_eq!(copy.shell_path, local.shell_path);
        assert_eq!(copy.work_dir, local.work_dir);
        assert!(copy.credential.is_none() && copy.proxy_id.is_none());
        state.validate()?;
        let invalid = Host {
            kind: HostKind::Local,
            work_dir: Some("relative/path".into()),
            ..local
        };
        assert!(invalid.validate_endpoint().is_err());
        assert!(Host { port: 0, ..old }.validate_endpoint().is_err());
        Ok(())
    }
    use super::*;

    #[test]
    fn management_duplicates_preserve_configuration_and_group_parents_exclude_descendants() {
        let groups = vec![
            Group {
                mosh: None,
                telnet: None,
                uuid: None,
                ssh: None,
                id: 2,
                name: "Parent".into(),
                parent: None,
                proxy_id: Some(7),
            },
            Group {
                mosh: None,
                telnet: None,
                uuid: None,
                ssh: None,
                id: 3,
                name: "Child".into(),
                parent: Some(2),
                proxy_id: None,
            },
            Group {
                mosh: None,
                telnet: None,
                uuid: None,
                ssh: None,
                id: 4,
                name: "Other".into(),
                parent: None,
                proxy_id: None,
            },
        ];
        let mut state = AppState {
            groups,
            ..Default::default()
        };
        state.hosts.push(
            serde_json::from_value(serde_json::json!({
                "id": 5, "name": "Synthetic", "address": "test.invalid", "port": 2222,
                "username": "test", "group_id": 3, "tags": ["keep"], "notes": "keep",
                "credential": {"identity": 7}, "theme": "nysa-dark"
            }))
            .unwrap(),
        );
        state.next_id = 6;
        assert_eq!(
            state
                .group_path(3)
                .iter()
                .map(|group| group.id)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        assert!(!state.group_parent_allowed(Some(2), 2));
        assert!(!state.group_parent_allowed(Some(2), 3));
        assert!(state.group_parent_allowed(Some(3), 2));
        assert!(state.group_parent_allowed(Some(3), 4));
        let created = state
            .duplicate(ItemKind::Host, &BTreeSet::from([5]))
            .unwrap();
        assert_eq!(created, [6]);
        let mut expected = state.hosts[0].clone();
        expected.id = 6;
        expected.name.push_str(" copy");
        assert_eq!(state.hosts[1], expected);
        let before = serde_json::to_value(&state).unwrap();
        assert!(
            state
                .duplicate(ItemKind::Host, &BTreeSet::from([5, 999]))
                .is_err()
        );
        assert_eq!(serde_json::to_value(&state).unwrap(), before);
        let groups = state
            .duplicate(ItemKind::Group, &BTreeSet::from([3]))
            .unwrap();
        assert_eq!(state.groups.last().unwrap().parent, Some(2));
        assert_eq!(state.hosts.len(), 2); // Copying a group doesn't clone children.
        assert_eq!(groups, [7]);
    }

    fn hierarchy_group(id: Id, parent: Option<Id>) -> Group {
        Group {
            id,
            uuid: None,
            name: format!("Group {id}"),
            parent,
            proxy_id: None,
            ssh: None,
            telnet: None,
            mosh: None,
        }
    }

    #[test]
    fn group_parent_choices_stop_at_two_levels_and_include_existing_descendants() {
        let state = AppState {
            groups: vec![
                hierarchy_group(2, None),
                hierarchy_group(3, Some(2)),
                hierarchy_group(4, None),
            ],
            ..Default::default()
        };
        assert!(state.group_parent_allowed(None, 2));
        assert!(state.group_parent_allowed(None, 4));
        assert!(!state.group_parent_allowed(None, 3));
        assert!(!state.group_parent_allowed(None, 99));
        assert!(!state.group_parent_allowed(Some(2), 2));
        assert!(!state.group_parent_allowed(Some(2), 3));
        assert!(!state.group_parent_allowed(Some(2), 4));
        assert!(state.group_parent_allowed(Some(3), 2));
        assert!(state.group_parent_allowed(Some(3), 4));
        assert!(!state.group_parent_allowed(Some(3), 3));
        assert!(!state.group_parent_allowed(Some(99), 4));
    }

    #[test]
    fn group_publication_validates_the_final_tree_without_rejecting_unrelated_legacy_data() {
        let before = vec![
            hierarchy_group(2, None),
            hierarchy_group(3, Some(2)),
            hierarchy_group(4, None),
        ];
        let mut requested = before.clone();
        requested[0].parent = Some(4);
        assert_eq!(
            validate_group_parent_changes(&before, &requested, &before)
                .unwrap_err()
                .to_string(),
            GROUP_DEPTH_ERROR
        );
        requested[1].parent = Some(4);
        assert!(validate_group_parent_changes(&before, &requested, &before).is_ok());
        let mut requested = before.clone();
        requested.push(hierarchy_group(5, Some(3)));
        assert!(validate_group_parent_changes(&before, &requested, &before).is_err());
        requested[3].parent = Some(4);
        assert!(validate_group_parent_changes(&before, &requested, &before).is_ok());
        requested.push(hierarchy_group(6, Some(5)));
        assert!(validate_group_parent_changes(&before, &requested, &before).is_err());

        let mut legacy = before;
        legacy.push(hierarchy_group(5, Some(3)));
        let mut renamed = legacy.clone();
        renamed[0].name = "Renamed".into();
        assert!(validate_group_parent_changes(&legacy, &renamed, &legacy).is_ok());
        renamed[3].parent = Some(4);
        assert!(validate_group_parent_changes(&legacy, &renamed, &legacy).is_ok());
    }

    #[test]
    fn group_publication_rechecks_external_parent_moves_and_new_children() {
        let before = vec![hierarchy_group(2, None), hierarchy_group(4, None)];
        let mut requested = before.clone();
        requested[0].parent = Some(4);
        assert!(validate_group_parent_changes(&before, &requested, &before).is_ok());
        let mut current = before.clone();
        current.push(hierarchy_group(3, Some(2)));
        assert!(validate_group_parent_changes(&before, &requested, &current).is_err());
        current.pop();
        current[1].parent = Some(6);
        current.push(hierarchy_group(6, None));
        assert!(validate_group_parent_changes(&before, &requested, &current).is_err());
        assert_eq!(current[0].parent, None);
    }

    #[test]
    fn legacy_hosts_default_to_no_credential_and_new_metadata_contains_only_a_reference() {
        let mut host: Host = serde_json::from_value(serde_json::json!({"id":2,"name":"Fixture","address":"127.0.0.1","port":22,"username":"fixture","group_id":null})).unwrap();
        assert_eq!(host.credential, None);
        assert_eq!(host.proxy_id, None);
        host.credential = Some(CredentialRef::Identity(7));
        host.proxy_id = Some(11);
        let json = serde_json::to_value(host).unwrap();
        assert_eq!(json["credential"]["identity"], 7);
        assert_eq!(json["proxy_id"], 11);
        for field in ["password", "private_key", "passphrase"] {
            assert!(json.get(field).is_none());
        }
    }

    #[test]
    fn host_theme_override_round_trips_without_replacing_global_appearance() {
        let mut state = AppState::default();
        let mut host: Host = serde_json::from_value(serde_json::json!({
            "id": 2, "name": "Synthetic", "address": "test.invalid", "port": 22,
            "username": "test", "group_id": null
        }))
        .unwrap();
        assert_eq!(host.theme, None);
        state.terminal.family = "Synthetic Mono".into();
        state.terminal.change_color(2, 0x123456).unwrap();
        let global = state.terminal.clone();
        state.hosts.push(host.clone());
        assert_eq!(state.terminal_for_host(Some(2)), global);
        host.theme = Some(crate::terminal_appearance::ThemeId::NysaDark);
        let json = serde_json::to_value(&host).unwrap();
        assert_eq!(json["theme"], "nysa-dark");
        state.hosts[0] = serde_json::from_value(json).unwrap();
        let selected = state.terminal_for_host(Some(2));
        assert_eq!(
            selected.theme,
            crate::terminal_appearance::ThemeId::NysaDark
        );
        assert!(selected.custom_palette.is_none());
        assert_eq!(selected.family, "Synthetic Mono");
        assert_eq!(state.terminal, global);
        state.hosts[0].theme = None;
        assert!(
            serde_json::to_value(&state.hosts[0])
                .unwrap()
                .get("theme")
                .is_none()
        );
        assert_eq!(state.terminal_for_host(Some(2)), global);
        assert_eq!(state.terminal_for_host(None), global);
        assert_eq!(state.terminal_for_host(Some(999)), global);
    }

    #[test]
    fn scoped_search_combines_fields_and_is_case_insensitive() {
        let state = AppState::default();
        let host = Host {
            connections: Default::default(),
            uuid: None,
            platform: Default::default(),
            kind: Default::default(),
            shell_path: None,
            work_dir: None,
            ssh: Default::default(),
            id: 2,
            name: "Production".into(),
            address: "10.0.0.4".into(),
            port: 22,
            username: "admin".into(),
            group_id: None,
            tags: vec!["Linux".into()],
            credential: None,
            proxy_id: None,
            theme: None,
        };
        assert!(state.host_matches(&host, "host:10.0 user:ADMIN tag:linux"));
        assert!(state.host_matches(&host, "admin@10.0.0"));
        assert!(!state.host_matches(&host, "root@10.0.0"));
        assert!(!state.host_matches(&host, "user:root"));
    }

    #[test]
    fn deleting_a_group_preserves_its_hosts() {
        let mut state = AppState::default();
        state.groups.push(Group {
            mosh: None,
            telnet: None,
            uuid: None,
            ssh: None,
            id: 2,
            name: "Servers".into(),
            parent: None,
            proxy_id: None,
        });
        state.hosts.push(Host {
            connections: Default::default(),
            uuid: None,
            platform: Default::default(),
            kind: Default::default(),
            shell_path: None,
            work_dir: None,
            ssh: Default::default(),
            id: 3,
            name: "server".into(),
            address: "localhost".into(),
            port: 22,
            username: "admin".into(),
            group_id: Some(2),
            tags: vec![],
            credential: None,
            proxy_id: None,
            theme: None,
        });
        state.delete(ItemKind::Group, &BTreeSet::from([2]));
        assert_eq!(state.hosts.len(), 1);
        assert_eq!(state.hosts[0].group_id, None);
    }

    #[test]
    fn table_local_ids_may_overlap_without_merging_the_resources() -> anyhow::Result<()> {
        let mut state = AppState::default();
        state.groups.push(Group {
            mosh: None,
            telnet: None,
            uuid: None,
            id: 1,
            name: "Group".into(),
            parent: None,
            proxy_id: None,
            ssh: None,
        });
        state.hosts.push(Host {
            id: 1,
            name: "Host".into(),
            address: "example.invalid".into(),
            port: 22,
            group_id: Some(1),
            ..Default::default()
        });
        state.validate()?;
        state.delete(ItemKind::Host, &BTreeSet::from([1]));
        assert!(state.hosts.is_empty());
        assert_eq!(state.groups.len(), 1);
        assert_eq!(state.workspace().id, 1);
        Ok(())
    }

    #[test]
    fn metadata_rejects_cycles_duplicate_ids_and_reused_next_id() {
        let mut state = AppState::default();
        assert!(state.validate().is_ok());
        state.groups.push(Group {
            mosh: None,
            telnet: None,
            uuid: None,
            ssh: None,
            id: 2,
            name: "A".into(),
            parent: Some(3),
            proxy_id: None,
        });
        state.groups.push(Group {
            mosh: None,
            telnet: None,
            uuid: None,
            ssh: None,
            id: 3,
            name: "B".into(),
            parent: Some(2),
            proxy_id: None,
        });
        state.next_id = 4;
        assert!(state.validate().unwrap_err().to_string().contains("cycle"));
        state.groups.clear();
        state.workspaces.push(state.workspaces[0].clone());
        assert!(
            state
                .validate()
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
        state.workspaces.pop();
        state.next_id = 1;
        assert!(state.validate().is_err());
    }
}
