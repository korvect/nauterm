//! Multi-protocol Host connections keep their existing tab and load credentials
//! only after the user confirms the protocol and per-attempt port.
use super::*;
use crate::connection::{Protocol, ProtocolSelected};

impl Nauterm {
    pub(super) fn open_protocol_choice(
        &mut self,
        host: Host,
        workspace: Id,
        overview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.data.workspaces.iter().any(|item| item.id == workspace) {
            return;
        }
        let core = match self.core.clone().map(Ok).unwrap_or_else(Core::discover) {
            Ok(core) => core,
            Err(error) => {
                self.notice = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let id = self.next_tab;
        self.next_tab += 1;
        let label = host.name.clone();
        let palette = self.data.terminal_for_host(Some(host.id)).palette();
        self.terminals.push(TerminalTab {
            id,
            workspace,
            show_sftp: false,
            tools: Default::default(),
            tree: PaneTree::new_with_composer(
                id,
                SessionView {
                    id,
                    title: label.clone(),
                    content: TabContent::Preparing(None),
                    ssh: None,
                    restore_target: None,
                    restore_directory: None,
                    files: None,
                    tool_files: None,
                    _file_events: vec![],
                    replay: None,
                    _metadata: vec![],
                    preparation_generation: 0,
                },
                self.data.terminal.composer_enabled,
            ),
        });
        self.core = Some(core.clone());
        let connection = cx.new(|cx| {
            Connection::new_host_choice(
                core,
                host,
                crate::connection::TerminalPresentation {
                    title: label,
                    dark: self.data.dark,
                    palette,
                    recording: None,
                },
                window,
                cx,
            )
        });
        self.watch_connection(id, connection.clone(), window, cx);
        let chosen = cx.subscribe_in(&connection, window, move |this, connection, choice: &ProtocolSelected, window, cx| {
            if !this.session(id).is_some_and(|session| matches!(&session.content, TabContent::Connection(current) if current == connection)) { return; }
            this.start_protocol_choice(id, connection, choice, window, cx);
        });
        self.session_mut(id).unwrap()._metadata.push(chosen);
        self.show_session(id, overview, window, cx);
        self.notice = None;
        cx.notify();
    }
    fn start_protocol_choice(
        &mut self,
        id: u64,
        connection: &Entity<Connection>,
        choice: &ProtocolSelected,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = (|| -> anyhow::Result<Host> {
            let host = self
                .data
                .hosts
                .iter()
                .find(|host| host.id == choice.host_id && host.uuid == choice.uuid)
                .cloned()
                .ok_or_else(|| {
                    anyhow::anyhow!("The saved Host was deleted or replaced. Reopen it from Hosts.")
                })?;
            anyhow::ensure!(
                host.is_remote(),
                "This Host is no longer a remote connection."
            );
            anyhow::ensure!(
                match choice.protocol {
                    Protocol::Ssh => host.supports_ssh(),
                    Protocol::Mosh => host.connections.mosh,
                    Protocol::Telnet => host.connections.telnet.is_some(),
                },
                "This protocol is no longer enabled. Reopen the Host to refresh its settings."
            );
            Ok(host)
        })();
        let host = match result {
            Ok(host) => host,
            Err(error) => {
                connection.update(cx, |view, cx| {
                    view.protocol_choice_error(error.to_string(), cx)
                });
                return;
            }
        };
        let Some(core) = self.core.clone() else {
            connection.update(cx, |view, cx| {
                view.protocol_choice_error(
                    "The terminal core is unavailable. Reopen the Host.".into(),
                    cx,
                )
            });
            return;
        };
        let workspace = self
            .terminals
            .iter()
            .find(|tab| tab.tree.session(id).is_some())
            .unwrap()
            .workspace;
        let session = self.session_mut(id).unwrap();
        session.title = host.name.clone();
        session.content = TabContent::Preparing(None);
        session._metadata.clear();
        match choice.protocol {
            Protocol::Ssh => {
                session.restore_target = Some(RestoreTarget::Host {
                    id: host.id,
                    port: (choice.port != host.port).then_some(choice.port),
                });
                let options = SshOptions::new(
                    host.address,
                    choice.port,
                    host.username,
                    self.store.paths.known_hosts(),
                );
                self.load_host_options(
                    (id, workspace),
                    core,
                    options,
                    (host.credential, host.proxy_id, host.ssh),
                    window,
                    cx,
                );
            }
            Protocol::Mosh => {
                session.restore_target = Some(RestoreTarget::MoshHost {
                    id: host.id,
                    port: (choice.port != host.port).then_some(choice.port),
                    command: choice
                        .command
                        .clone()
                        .filter(|command| *command != host.connections.mosh_command),
                });
                let options = SshOptions::new(
                    host.address,
                    choice.port,
                    host.username,
                    self.store.paths.known_hosts(),
                );
                self.load_host_options(
                    (id, workspace),
                    core,
                    options,
                    (host.credential, host.proxy_id, host.ssh),
                    window,
                    cx,
                );
            }
            Protocol::Telnet => {
                let profile = host.connections.telnet.unwrap();
                let port = (choice.port != profile.port).then_some(choice.port);
                session.restore_target = Some(RestoreTarget::TelnetHost { id: host.id, port });
                self.load_saved_telnet_connection(
                    id,
                    core,
                    (profile.source, Some(choice.port)),
                    window,
                    cx,
                );
            }
        }
        cx.notify();
    }
}

#[derive(Debug, PartialEq, Eq)]
enum RetryPlan {
    Ssh { port: u16, mosh: bool },
    Telnet { source: String, port: Option<u16> },
}
impl RetryPlan {
    fn new(host: &Host, expected_uuid: &str, target: &RestoreTarget) -> anyhow::Result<Self> {
        anyhow::ensure!(
            host.uuid.as_deref() == Some(expected_uuid),
            "The saved Host was deleted or replaced. Reopen it from Hosts."
        );
        anyhow::ensure!(
            host.is_remote(),
            "This Host is no longer a remote connection."
        );
        match target {
            RestoreTarget::Host { id, port } if *id == host.id => {
                anyhow::ensure!(
                    host.supports_ssh(),
                    "SSH is no longer enabled for this Host."
                );
                let port = port.unwrap_or(host.port);
                anyhow::ensure!(port > 0, "SSH port must be between 1 and 65535.");
                Ok(Self::Ssh { port, mosh: false })
            }
            RestoreTarget::MoshHost { id, port, command } if *id == host.id => {
                anyhow::ensure!(
                    host.connections.mosh,
                    "Mosh is no longer enabled for this Host."
                );
                crate::mosh::Config::new(
                    command.as_deref().or(Some(&host.connections.mosh_command)),
                )?;
                let port = port.unwrap_or(host.port);
                anyhow::ensure!(port > 0, "SSH port must be between 1 and 65535.");
                Ok(Self::Ssh { port, mosh: true })
            }
            RestoreTarget::TelnetHost { id, port } if *id == host.id => {
                let profile =
                    host.connections.telnet.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Telnet is no longer enabled for this Host.")
                    })?;
                anyhow::ensure!(
                    port.unwrap_or(profile.port) > 0,
                    "Telnet port must be between 1 and 65535."
                );
                Ok(Self::Telnet {
                    source: profile.source.clone(),
                    port: *port,
                })
            }
            _ => anyhow::bail!("This connection no longer belongs to the saved Host."),
        }
    }
}
impl Nauterm {
    pub(super) fn connection_host_action(
        &mut self,
        session_id: u64,
        source: (Id, &str),
        action: crate::connection::HostAction,
        connection: &Entity<Connection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = (|| -> anyhow::Result<_> {
            anyhow::ensure!(self.resources_ready(), "Host storage is not ready.");
            self.store.refresh_resources(&mut self.data)?;
            let host = self
                .data
                .hosts
                .iter()
                .find(|host| host.id == source.0 && host.uuid.as_deref() == Some(source.1))
                .cloned()
                .ok_or_else(|| {
                    anyhow::anyhow!("The saved Host was deleted or replaced. Reopen it from Hosts.")
                })?;
            Ok(host)
        })();
        let host = match result {
            Ok(host) => host,
            Err(error) => {
                self.notice = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        if matches!(action, crate::connection::HostAction::Edit) {
            self.open_editor(ItemKind::Host, Some(host.id), window, cx);
            return;
        }
        let Some(target) = self
            .session(session_id)
            .and_then(|session| session.restore_target.as_ref())
        else {
            return;
        };
        let plan = match RetryPlan::new(&host, source.1, target) {
            Ok(plan) => plan,
            Err(error) => {
                self.notice = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let Some(core) = self.core.clone() else {
            return;
        };
        let Some(vault) = self.ensure_keychain(cx).read(cx).vault() else {
            self.notice = Some("Saved connection settings cannot be loaded. Resolve the Keychain storage error and retry.".into());
            cx.notify();
            return;
        };
        connection.update(cx, |view, cx| view.begin_saved_reload(cx));
        let known_hosts = self.store.paths.known_hosts();
        let source_view = connection.downgrade();
        cx.spawn_in(window, async move |this, cx| {
            let result = async {
                match plan {
                    RetryPlan::Ssh { port, mosh } => {
                        let options = SshOptions::new(host.address.clone(), port, host.username.clone(), known_hosts);
                        let options = vault.apply_remote_connection(host.credential, host.proxy_id, host.ssh.clone(), options, mosh).await?;
                        let options = vault.apply_shell_setup(host.ssh.clone(), options).await?;
                        Ok::<_, anyhow::Error>(Reloaded::Ssh(Box::new(options)))
                    }
                    RetryPlan::Telnet { source, port } => {
                        let mut resolved = vault.saved_telnet_host(source).await?;
                        if let Some(port) = port { resolved.options.port = port; }
                        Ok(Reloaded::Telnet(resolved.options))
                    }
                }
            }.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.exiting.is_some() { return; }
                let Some(connection) = source_view.upgrade() else { return; };
                if !this.session(session_id).is_some_and(|session| matches!(&session.content, TabContent::Connection(current) if current == &connection)) { return; }
                let result = result.and_then(|resolved| {
                    anyhow::ensure!(this.data.hosts.iter().any(|current| current == &host), "The Host changed while reloading. Start over to use its latest settings.");
                    Ok(resolved)
                });
                match result {
                    Ok(resolved) => {
                        this.session_mut(session_id).unwrap().title = host.name;
                        match resolved {
                            Reloaded::Ssh(options) => this.install_connection(session_id, core, *options, window, cx),
                            Reloaded::Telnet(options) => this.install_telnet_connection(session_id, core, options, window, cx),
                        }
                    }
                    Err(error) => connection.update(cx, |view, cx| view.saved_reload_failed(error.to_string(), cx)),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}
enum Reloaded {
    Ssh(Box<SshOptions>),
    Telnet(crate::telnet::Options),
}

#[cfg(test)]
mod retry_tests {
    use super::RetryPlan;
    use crate::{model::Host, recovery::Target as RestoreTarget};
    const UUID: &str = "12345678-1234-1234-1234-123456789abc";
    fn host() -> Host {
        Host {
            id: 7,
            uuid: Some(UUID.into()),
            name: "fixture".into(),
            address: "example.invalid".into(),
            port: 2244,
            kind: crate::model::HostKind::Remote,
            ..Default::default()
        }
    }
    #[test]
    fn retry_reads_latest_host_port_and_preserves_explicit_attempt_overrides() {
        assert_eq!(
            RetryPlan::new(&host(), UUID, &RestoreTarget::Host { id: 7, port: None }).unwrap(),
            RetryPlan::Ssh {
                port: 2244,
                mosh: false
            }
        );
        assert_eq!(
            RetryPlan::new(
                &host(),
                UUID,
                &RestoreTarget::Host {
                    id: 7,
                    port: Some(2022)
                }
            )
            .unwrap(),
            RetryPlan::Ssh {
                port: 2022,
                mosh: false
            }
        );
    }
    #[test]
    fn retry_rejects_replaced_hosts_wrong_sessions_and_disabled_protocols() {
        let mut host = host();
        let target = RestoreTarget::Host { id: 7, port: None };
        assert!(RetryPlan::new(&host, "22345678-1234-1234-1234-123456789abc", &target).is_err());
        assert!(RetryPlan::new(&host, UUID, &RestoreTarget::Host { id: 8, port: None }).is_err());
        host.connections.ssh = false;
        assert!(RetryPlan::new(&host, UUID, &target).is_err());
        host.connections.ssh = true;
        host.kind = crate::model::HostKind::Local;
        assert!(RetryPlan::new(&host, UUID, &target).is_err());
    }
    #[test]
    fn mosh_retry_validates_the_current_command_or_explicit_override() {
        let mut host = host();
        host.connections.mosh = true;
        let target = RestoreTarget::MoshHost {
            id: 7,
            port: None,
            command: None,
        };
        assert_eq!(
            RetryPlan::new(&host, UUID, &target).unwrap(),
            RetryPlan::Ssh {
                port: 2244,
                mosh: true
            }
        );
        host.connections.mosh_command = "invalid\ncommand".into();
        assert!(RetryPlan::new(&host, UUID, &target).is_err());
        let overridden = RestoreTarget::MoshHost {
            id: 7,
            port: Some(2022),
            command: Some("/opt/bin/mosh-server new".into()),
        };
        assert_eq!(
            RetryPlan::new(&host, UUID, &overridden).unwrap(),
            RetryPlan::Ssh {
                port: 2022,
                mosh: true
            }
        );
    }
    #[test]
    fn telnet_retry_resolves_its_own_protocol_reference_and_optional_port() {
        let mut host = host();
        let source = format!("native:host:7:{UUID}:telnet");
        host.connections.telnet = Some(crate::telnet::Profile {
            source: source.clone(),
            port: 2323,
            ..Default::default()
        });
        assert_eq!(
            RetryPlan::new(
                &host,
                UUID,
                &RestoreTarget::TelnetHost { id: 7, port: None }
            )
            .unwrap(),
            RetryPlan::Telnet {
                source: source.clone(),
                port: None
            }
        );
        assert_eq!(
            RetryPlan::new(
                &host,
                UUID,
                &RestoreTarget::TelnetHost {
                    id: 7,
                    port: Some(2023)
                }
            )
            .unwrap(),
            RetryPlan::Telnet {
                source,
                port: Some(2023)
            }
        );
        host.connections.telnet = None;
        assert!(
            RetryPlan::new(
                &host,
                UUID,
                &RestoreTarget::TelnetHost { id: 7, port: None }
            )
            .is_err()
        );
    }
}
