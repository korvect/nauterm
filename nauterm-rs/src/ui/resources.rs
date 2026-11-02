//! Shared saved-resource actions, committed before publishing UI state.
use super::*;
use anyhow::Context as _;

impl Nauterm {
    pub(super) fn ensure_recordings(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<crate::recordings::Service> {
        if let Some(service) = &self.recordings {
            return Some(service.clone());
        }
        let result = (|| {
            let core = self.core.clone().map(Ok).unwrap_or_else(Core::discover)?;
            let vault = self
                .ensure_keychain(cx)
                .read(cx)
                .vault()
                .context("The encrypted database is unavailable.")?;
            crate::recordings::Service::new(
                core,
                vault,
                self.store.paths.captures(),
                self.shell_history.clone(),
            )
            .and_then(|service| {
                service.configure(self.data.recordings)?;
                Ok(service)
            })
        })();
        match result {
            Ok(service) => {
                self.recordings = Some(service.clone());
                Some(service)
            }
            Err(error) => {
                self.notice = Some(format!("Session history unavailable: {error}"));
                None
            }
        }
    }
    pub(super) fn record_session(
        &mut self,
        session: &mut crate::native::Session,
        title: &str,
        appearance: &crate::terminal_appearance::Appearance,
        host_id: Option<Id>,
        cx: &mut Context<Self>,
    ) {
        if let Some(start) = self.prepare_connection_recording(title, appearance, host_id, cx) {
            session.record(&start);
        }
    }
    pub(super) fn prepare_connection_recording(
        &mut self,
        title: &str,
        appearance: &crate::terminal_appearance::Appearance,
        host_id: Option<Id>,
        cx: &mut Context<Self>,
    ) -> Option<crate::recordings::Start> {
        if !self.data.recordings.enabled {
            return None;
        }
        let service = self.ensure_recordings(cx)?;
        let mut start =
            crate::recordings::Start::new(service, self.data.recordings, title, appearance.theme);
        if let Some(host) = host_id.and_then(|id| self.data.hosts.iter().find(|host| host.id == id))
        {
            start.host_id = i64::try_from(host.id).ok();
            start.host_uuid = host.uuid.clone();
        }
        Some(start)
    }
    pub(super) fn proxy_options(&self, cx: &App) -> Vec<SelectOption<i64>> {
        self.proxies
            .as_ref()
            .map(|view| view.read(cx).summaries())
            .unwrap_or_default()
            .into_iter()
            .map(|proxy| {
                SelectOption::new(proxy.id, format!("{} — {}", proxy.name, proxy.kind.label()))
            })
            .collect()
    }
    pub(super) fn ensure_proxies(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Entity<crate::proxies::Proxies> {
        if let Some(view) = &self.proxies {
            return view.clone();
        }
        let keychain = self.ensure_keychain(cx);
        let vault = keychain.read(cx).vault();
        let view = cx.new(|cx| {
            crate::components::message::watch_window_many(
                self.window_id,
                cx,
                crate::proxies::Proxies::feedback,
            );
            crate::proxies::Proxies::new(
                self.store.database_path(),
                vault,
                self.data.proxies.clone(),
                self.searches[&Page::Proxies].clone(),
                self.data.dark,
                keychain.downgrade(),
                cx,
            )
        });
        self.subscriptions.push(cx.observe(&view, |this, view, cx| {
            if this.page == Page::Proxies && this.child_drawer_open != view.read(cx).drawer_open() {
                cx.notify();
            }
        }));
        self.subscriptions.push(cx.subscribe(
            &view,
            |this, view, _: &crate::proxies::MigrationReady, cx| {
                let result = (|| {
                    anyhow::ensure!(this.resources_ready(), "Workspace metadata is unavailable.");
                    let mut candidate = this.data.clone();
                    candidate.proxies.clear();
                    this.store.save(&mut candidate)?;
                    this.data = candidate;
                    anyhow::Ok(())
                })();
                view.update(cx, |view, cx| view.migration_result(result, cx));
                cx.notify();
            },
        ));
        self.subscriptions
            .push(cx.subscribe(&view, |_, _, _: &crate::proxies::Changed, cx| cx.notify()));
        self.subscriptions.push(cx.subscribe(
            &view,
            |this, view, event: &crate::proxies::ReferencesDeleted, cx| {
                let result = (|| {
                    anyhow::ensure!(this.resources_ready(), "Workspace metadata is unavailable.");
                    let deleted = event.0.keys().copied().collect();
                    if this
                        .editor
                        .iter()
                        .chain(
                            this.saved_drawers
                                .values()
                                .filter_map(|saved| saved.editor.as_ref()),
                        )
                        .any(|editor| {
                            matches!(
                                editor.kind,
                                ItemKind::Host | ItemKind::Group | ItemKind::Forward
                            )
                        })
                    {
                        // An open draft keeps its previous edit baseline;
                        // native row versions will reject a stale save.
                        this.data.clear_proxy_references(&deleted);
                    } else {
                        this.store.refresh_resources(&mut this.data)?;
                    }
                    this.deleted_proxies.extend(&deleted);
                    if let Some(editor) = &mut this.editor {
                        editor.clear_proxy_references(&deleted);
                    }
                    for editor in this
                        .saved_drawers
                        .values_mut()
                        .filter_map(|saved| saved.editor.as_mut())
                    {
                        editor.clear_proxy_references(&deleted);
                    }
                    if this
                        .notice
                        .as_ref()
                        .is_some_and(|notice| notice.starts_with("Proxy reference cleanup failed:"))
                    {
                        this.notice = None;
                    }
                    anyhow::Ok(())
                })();
                if let Err(error) = &result {
                    this.notice = Some(format!(
                        "Proxy reference cleanup failed: {error}. Open Proxies and retry."
                    ));
                }
                view.update(cx, |view, cx| {
                    view.references_result(event.0.clone(), result, cx)
                });
                cx.notify();
            },
        ));
        self.proxies = Some(view.clone());
        view
    }
    pub(super) fn copy_ssh_command(&self, id: Id, cx: &mut Context<Self>) {
        let Some(host) = self.data.hosts.iter().find(|host| host.id == id) else {
            return;
        };
        if !host.supports_ssh() {
            return;
        }
        let username = match host.credential {
            Some(CredentialRef::Identity(id)) => self
                .keychain
                .as_ref()
                .and_then(|view| {
                    view.read(cx)
                        .inventory()
                        .identities
                        .into_iter()
                        .find(|identity| identity.id == id)
                })
                .map(|identity| identity.username)
                .unwrap_or_else(|| host.username.clone()),
            _ => host.username.clone(),
        };
        let command = crate::ssh_address::SshAddress {
            host: host.address.clone(),
            port: host.port,
            username,
        }
        .command();
        cx.write_to_clipboard(ClipboardItem::new_string(command));
    }
    pub(super) fn enter_group(
        &mut self,
        group: Option<Id>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.group = group.filter(|id| self.data.groups.iter().any(|group| group.id == *id));
        self.retire_editor();
        self.selection.clear();
        self.selection_kind = None;
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn duplicate_resources(&mut self, kind: ItemKind, cx: &mut Context<Self>) {
        let result = (|| {
            anyhow::ensure!(self.resources_ready(), "Workspace storage is unavailable.");
            let mut candidate = self.data.clone();
            let created = candidate.duplicate(kind, &self.selection)?;
            self.store.save(&mut candidate)?;
            self.data = candidate;
            self.retire_editor();
            self.selection = created.into_iter().collect();
            self.selection_kind = Some(kind);
            anyhow::Ok(())
        })();
        if let Err(error) = result {
            self.notice = Some(format!("Could not duplicate: {error}"));
        }
        cx.notify();
    }
    pub(super) fn delete_resources(
        &mut self,
        kind: ItemKind,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(self.resources_ready(), "Workspace storage is unavailable.");
        if kind == ItemKind::Host {
            anyhow::ensure!(
                !self.data.forwards.iter().any(|rule| rule
                    .host_id
                    .is_some_and(|id| self.selection.contains(&id))
                    && (self.forward_active(rule.id) || self.forward_mutations.contains(&rule.id))),
                "Stop forwarding rules for the selected Hosts before deleting them."
            );
        }
        let mut candidate = self.data.clone();
        let passwords = if kind == ItemKind::Host {
            self.data
                .hosts
                .iter()
                .filter(|host| self.selection.contains(&host.id))
                .filter_map(|host| host.ssh.password_id.clone())
                .collect()
        } else if kind == ItemKind::Group {
            self.data
                .groups
                .iter()
                .filter(|group| self.selection.contains(&group.id))
                .filter_map(|group| {
                    group
                        .ssh
                        .as_ref()
                        .and_then(|profile| profile.setup.password_id.clone())
                })
                .collect()
        } else {
            vec![]
        };
        let environments = if kind == ItemKind::Host {
            self.data
                .hosts
                .iter()
                .filter(|host| self.selection.contains(&host.id))
                .filter_map(|host| host.ssh.environment_id.clone())
                .collect()
        } else if kind == ItemKind::Group {
            self.data
                .groups
                .iter()
                .filter(|group| self.selection.contains(&group.id))
                .filter_map(|group| {
                    group
                        .ssh
                        .as_ref()
                        .and_then(|profile| profile.setup.environment_id.clone())
                })
                .collect()
        } else {
            vec![]
        };
        candidate.delete(kind, &self.selection);
        self.store.save(&mut candidate)?;
        self.data = candidate;
        self.cleanup_host_environments(environments, cx);
        self.cleanup_host_passwords(passwords, cx);
        if self
            .group
            .is_some_and(|id| !self.data.groups.iter().any(|group| group.id == id))
        {
            self.group = None;
        }
        Ok(())
    }
}
