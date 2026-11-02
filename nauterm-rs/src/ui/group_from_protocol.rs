//! Copy one protocol into a nested Group draft without reparenting its Host.
use super::host_credentials::Target;
use super::*;
use anyhow::{Context as _, Result, ensure};

fn template(source: &Host, target: Target) -> Host {
    match target {
        Target::Ssh => {
            let identity = matches!(source.credential, Some(CredentialRef::Identity(_)));
            let mut setup = source.ssh.clone();
            setup.profile_source = None;
            if identity {
                setup.password_id = None;
            }
            Host {
                port: source.port,
                username: if identity {
                    String::new()
                } else {
                    source.username.clone()
                },
                credential: source.credential,
                proxy_id: source.proxy_id,
                theme: source.theme,
                ssh: setup,
                connections: ConnectionProfiles {
                    ssh: true,
                    mosh: source.connections.mosh,
                    mosh_command: if source.connections.mosh {
                        source.connections.mosh_command.clone()
                    } else {
                        crate::mosh::DEFAULT_COMMAND.into()
                    },
                    telnet: None,
                },
                ..Default::default()
            }
        }
        Target::Telnet => {
            let mut telnet = source.connections.telnet.clone().unwrap_or_default();
            telnet.source.clear();
            if telnet.identity_id.is_some() {
                telnet.username.clear();
                telnet.password_id = None;
            }
            Host {
                port: 22,
                connections: ConnectionProfiles {
                    ssh: false,
                    mosh: false,
                    telnet: Some(telnet),
                    ..Default::default()
                },
                ..Default::default()
            }
        }
    }
}

struct Seed {
    host: Host,
    password: Option<zeroize::Zeroizing<String>>,
    environment: Option<Vec<crate::host_session::Variable>>,
    environment_changed: bool,
    focus: FocusHandle,
}

impl Nauterm {
    pub(super) fn protocol_actions(
        &self,
        editor: &Editor,
        target: Target,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let draft = editor.host.as_ref().unwrap();
        let auth = editor.auth(target).unwrap();
        let create = editor.kind == ItemKind::Host && draft.kind == HostKind::Remote;
        let blocked = auth.blocked()
            || (target == Target::Ssh && (draft.inheriting || draft.inheritance_error.is_some()));
        let token = editor.fields[0].1.entity_id();
        let owner = cx.weak_entity();
        Button::new(SharedString::from(format!("host-{}-actions", target.id())))
            .toolbar()
            .ghost()
            .icon(IconName::Ellipsis)
            .tooltip(if target == Target::Ssh {
                "SSH actions"
            } else {
                "Telnet actions"
            })
            .disabled(draft.busy)
            .dropdown_menu(move |menu, _, _| {
                let create_owner = owner.clone();
                let remove_owner = owner.clone();
                menu.width(190.)
                    .when(create, |menu| {
                        menu.item(
                            MenuItem::localized("rs.workspace.action.createGroup")
                                .icon(crate::assets::GROUP_ICON)
                                .disabled(blocked)
                                .on_click(move |_, window, cx| {
                                    let _ = create_owner.update(cx, |this, cx| {
                                        if this.editor.as_ref().is_some_and(|editor| {
                                            editor.fields[0].1.entity_id() == token
                                        }) {
                                            this.create_group_from_protocol(target, window, cx);
                                        }
                                    });
                                }),
                        )
                        .separator()
                    })
                    .item(
                        MenuItem::localized("Remove protocol")
                            .icon(IconName::Trash)
                            .destructive(true)
                            .on_click(move |_, _, cx| {
                                let _ = remove_owner.update(cx, |this, cx| {
                                    if this.editor.as_ref().is_some_and(|editor| {
                                        editor.fields[0].1.entity_id() == token
                                    }) {
                                        match target {
                                            Target::Ssh => this.set_editor_ssh_enabled(false, cx),
                                            Target::Telnet => {
                                                this.set_editor_telnet_enabled(false, cx)
                                            }
                                        }
                                    }
                                });
                            }),
                    )
            })
            .into_any_element()
    }

    fn protocol_group_seed(&self, target: Target, cx: &App) -> Result<Seed> {
        let editor = self
            .editor
            .as_ref()
            .context("The Host editor was closed.")?;
        let draft = editor
            .host
            .as_ref()
            .context("The Host editor was closed.")?;
        ensure!(
            editor.kind == ItemKind::Host && draft.kind == HostKind::Remote && !draft.busy,
            "Select an editable remote Host first."
        );
        let auth = editor.auth(target).unwrap();
        auth.validate(cx)?;
        let mut source = Host::default();
        let (environment, environment_changed, focus) = match target {
            Target::Ssh => {
                ensure!(
                    draft.ssh_enabled && !draft.inheriting && draft.inheritance_error.is_none(),
                    "Finish loading the SSH settings before creating a Group."
                );
                source.port = editor.fields[2].1.read(cx).value().parse().unwrap_or(22);
                source.username = editor.fields[3].1.read(cx).value().trim().to_owned();
                source.credential = editor.credential;
                source.proxy_id = editor.proxy_id;
                source.theme = editor.theme.as_ref().and_then(|theme| theme.selected);
                source.ssh = draft.setup.clone();
                source.connections.mosh = draft.mosh.enabled;
                source.connections.mosh_command = draft.mosh.value(cx)?.unwrap_or_default().command;
                (
                    draft.environment.clone(),
                    draft.environment_changed,
                    editor.fields[2].1.read(cx).focus_handle(cx),
                )
            }
            Target::Telnet => {
                source.connections.telnet = Some(
                    draft
                        .telnet
                        .value(cx)
                        .context("Telnet is no longer enabled.")?,
                );
                (None, false, draft.telnet.port.read(cx).focus_handle(cx))
            }
        };
        let manual = match target {
            Target::Ssh => !matches!(source.credential, Some(CredentialRef::Identity(_))),
            Target::Telnet => source
                .connections
                .telnet
                .as_ref()
                .is_some_and(|profile| profile.identity_id.is_none()),
        };
        Ok(Seed {
            host: template(&source, target),
            password: manual.then(|| auth.password_snapshot(cx)).flatten(),
            environment_changed: environment_changed || environment.is_some(),
            environment,
            focus,
        })
    }

    fn create_group_from_protocol(
        &mut self,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.resource_editor_busy() || !self.resources_ready() {
            return;
        }
        let mut seed = match self.protocol_group_seed(target, cx) {
            Ok(seed) => seed,
            Err(error) => {
                if let Some(editor) = &mut self.editor {
                    editor.error = Some(error.to_string());
                }
                cx.notify();
                return;
            }
        };
        let environment = (target == Target::Ssh && seed.environment.is_none())
            .then(|| seed.host.ssh.environment_id.clone())
            .flatten();
        let password = seed
            .password
            .is_none()
            .then(|| match target {
                Target::Ssh => seed.host.ssh.password_id.clone(),
                Target::Telnet => seed
                    .host
                    .connections
                    .telnet
                    .as_ref()
                    .and_then(|profile| profile.password_id.clone()),
            })
            .flatten();
        if environment.is_none() && password.is_none() {
            self.install_protocol_group(target, seed, window, cx);
            return;
        }
        let Some(vault) = self.ensure_keychain(cx).read(cx).vault() else {
            self.editor.as_mut().unwrap().error =
                Some("Keychain storage is not ready. Retry after it loads.".into());
            cx.notify();
            return;
        };
        let editor = self.editor.as_mut().unwrap();
        let token = editor.fields[0].1.entity_id();
        editor.host.as_mut().unwrap().busy = true;
        editor.error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result: Result<()> = async {
                if let Some(id) = environment {
                    seed.environment = Some(vault.host_environment(id).await?);
                    seed.environment_changed = true;
                }
                if let Some(id) = password {
                    seed.password = Some(vault.host_password(id).await?);
                }
                Ok(())
            }
            .await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(editor) = this
                    .editor
                    .as_mut()
                    .filter(|editor| editor.fields[0].1.entity_id() == token)
                else {
                    return;
                };
                editor.host.as_mut().unwrap().busy = false;
                match result {
                    Ok(()) => this.install_protocol_group(target, seed, window, cx),
                    Err(error) => {
                        editor.error = Some(error.to_string());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn install_protocol_group(
        &mut self,
        target: Target,
        seed: Seed,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_related_resource(related::Target::ProtocolGroup, "", seed.focus, window, cx);
        let Some(editor) = self
            .editor
            .as_ref()
            .filter(|editor| editor.kind == ItemKind::Group && editor.parent.is_some())
        else {
            return;
        };
        let username = editor.fields[3].1.clone();
        let port = editor.fields[2].1.clone();
        let inventory = self
            .keychain
            .as_ref()
            .map(|view| view.read(cx).inventory())
            .unwrap_or_default();
        port.update(cx, |input, cx| {
            input.set_value(seed.host.port.to_string(), window, cx)
        });
        username.update(cx, |input, cx| {
            input.set_value(seed.host.username.clone(), window, cx)
        });
        let mut auth = host_credentials::Authentication::new(
            seed.host.credential,
            &username,
            &inventory,
            window,
            cx,
        );
        let mut draft = host_config::Draft::new(
            Some(&seed.host),
            &self.data.snippets,
            &inventory,
            window,
            cx,
        );
        draft.watch_port(&port, cx);
        draft.environment = seed.environment.or(draft.environment);
        draft.environment_changed = seed.environment_changed;
        if let Some(password) = seed.password {
            match target {
                Target::Ssh => {
                    draft.setup.password_id = None;
                    auth.set_password_draft(password, window, cx);
                }
                Target::Telnet => {
                    draft.telnet.profile.password_id = None;
                    draft.telnet.auth.set_password_draft(password, window, cx);
                }
            }
        }
        let theme = host_themes::Draft::new(seed.host.theme, window, cx);
        let editor = self.editor.as_mut().unwrap();
        editor.credential = seed.host.credential;
        editor.proxy_id = seed.host.proxy_id;
        editor.theme = Some(theme);
        editor.host = Some(draft);
        let selects = editor.selects.as_mut().unwrap();
        selects.auth = auth;
        selects.proxies.set_selected(seed.host.proxy_id, window, cx);
        self.load_editor_password(window, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{Target, template};
    use crate::{
        host_session::Encoding,
        model::{ConnectionProfiles, CredentialRef, Host},
        terminal_appearance::ThemeId,
    };

    const SSH: &str = "native:host:7:00000000-0000-4000-8000-000000000007";
    const TELNET: &str = "native:host:7:00000000-0000-4000-8000-000000000007:telnet";

    fn mixed_host() -> Host {
        Host {
            id: 7,
            uuid: Some("00000000-0000-4000-8000-000000000007".into()),
            name: "Fixture".into(),
            address: "fixture.invalid".into(),
            port: 2022,
            username: "ssh-user".into(),
            group_id: Some(4),
            tags: vec!["fixture".into()],
            credential: Some(CredentialRef::Key(9)),
            proxy_id: Some(11),
            theme: Some(ThemeId::NysaDark),
            shell_path: Some("/fixture/shell".into()),
            work_dir: Some("/fixture/work".into()),
            platform: crate::avatar::Platform {
                os: Some("Linux".into()),
                distro: Some("Ubuntu".into()),
            },
            ssh: crate::host_session::Setup {
                profile_source: Some(SSH.into()),
                password_id: Some(SSH.into()),
                environment_id: Some(SSH.into()),
                startup_snippet: Some(13),
                encoding: Encoding::Gbk,
            },
            connections: ConnectionProfiles {
                ssh: true,
                mosh: true,
                mosh_command: "/fixture/mosh-server new -s".into(),
                telnet: Some(crate::telnet::Profile {
                    port: 2323,
                    username: "telnet-user".into(),
                    encoding: Encoding::Big5,
                    theme: Some(ThemeId::NysaLight),
                    source: TELNET.into(),
                    password_id: Some(TELNET.into()),
                    ..Default::default()
                }),
            },
            ..Default::default()
        }
    }

    #[test]
    fn ssh_template_preserves_ssh_settings_without_host_or_telnet_metadata() {
        let source = mixed_host();
        let mut setup = source.ssh.clone();
        setup.profile_source = None;
        assert_eq!(
            template(&source, Target::Ssh),
            Host {
                port: 2022,
                username: "ssh-user".into(),
                credential: Some(CredentialRef::Key(9)),
                proxy_id: Some(11),
                theme: Some(ThemeId::NysaDark),
                ssh: setup,
                connections: ConnectionProfiles {
                    ssh: true,
                    mosh: true,
                    mosh_command: "/fixture/mosh-server new -s".into(),
                    telnet: None,
                },
                ..Default::default()
            }
        );
        assert_eq!(source, mixed_host());
        let mut disabled = source;
        disabled.connections.mosh = false;
        assert_eq!(
            template(&disabled, Target::Ssh).connections.mosh_command,
            crate::mosh::DEFAULT_COMMAND
        );
    }

    #[test]
    fn telnet_template_excludes_ssh_settings_and_starts_with_a_new_profile() {
        let source = mixed_host();
        assert_eq!(
            template(&source, Target::Telnet),
            Host {
                port: 22,
                connections: ConnectionProfiles {
                    ssh: false,
                    mosh: false,
                    telnet: Some(crate::telnet::Profile {
                        port: 2323,
                        username: "telnet-user".into(),
                        encoding: Encoding::Big5,
                        theme: Some(ThemeId::NysaLight),
                        password_id: Some(TELNET.into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }
        );
        assert_eq!(source, mixed_host());
    }

    #[test]
    fn identity_templates_drop_manual_login_and_password_references() {
        let mut source = mixed_host();
        source.credential = Some(CredentialRef::Identity(17));
        source.connections.telnet.as_mut().unwrap().identity_id = Some(19);
        let ssh = template(&source, Target::Ssh);
        assert_eq!(ssh.credential, Some(CredentialRef::Identity(17)));
        assert!(ssh.username.is_empty());
        assert!(ssh.ssh.password_id.is_none());
        assert_eq!(ssh.ssh.environment_id.as_deref(), Some(SSH));
        let telnet = template(&source, Target::Telnet)
            .connections
            .telnet
            .unwrap();
        assert_eq!(telnet.identity_id, Some(19));
        assert!(telnet.username.is_empty());
        assert!(telnet.password_id.is_none());
        assert!(!source.username.is_empty());
        assert_eq!(source.ssh.password_id.as_deref(), Some(SSH));
        assert_eq!(
            source
                .connections
                .telnet
                .as_ref()
                .unwrap()
                .password_id
                .as_deref(),
            Some(TELNET)
        );
    }
}
