use super::*;
use crate::components::menu::{Menu, MenuState};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CardAction {
    Open,
    CurrentWorkspace,
    NewWorkspace,
    Ssh,
    Mosh,
    Sftp,
    Telnet,
    Edit,
    Duplicate,
    NewHost,
    CopySsh,
    Delete,
}

impl Nauterm {
    pub(super) fn resource_card_menu(
        &mut self,
        menu: Menu,
        kind: ItemKind,
        id: Id,
        entity: &Entity<MenuState>,
        cx: &mut Context<Self>,
    ) -> Menu {
        self.menu_subscription = Some(cx.subscribe(entity, |this, _, _: &DismissEvent, cx| {
            if !this.delete_pending
                && let Some((kind, id)) = this.editor.as_ref().and_then(Editor::edited)
            {
                this.selection = BTreeSet::from([id]);
                this.selection_kind = Some(kind);
                cx.notify();
            }
        }));
        let ids = if self.selection_kind == Some(kind) && self.selection.contains(&id) {
            self.selection.clone()
        } else {
            BTreeSet::from([id])
        };
        let single = ids.len() == 1;
        let owner = cx.weak_entity();
        let shortcuts = crate::shortcuts::current(cx);
        let item = |action, label: String, icon| {
            let owner = owner.clone();
            let ids = ids.clone();
            let entry = MenuItem::localized(label)
                .icon(icon)
                .on_click(move |_, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.card_action(kind, &ids, action, window, cx);
                    });
                });
            match action {
                CardAction::Edit => shortcuts.menu(entry, crate::shortcuts::Command::Edit),
                CardAction::Duplicate => {
                    shortcuts.menu(entry, crate::shortcuts::Command::Duplicate)
                }
                CardAction::Open => entry.shortcut("↵"),
                _ => entry,
            }
        };
        let hosts: Vec<_> = self
            .data
            .hosts
            .iter()
            .filter(|host| ids.contains(&host.id))
            .collect();
        let mut menu = menu.card_menu();
        if kind == ItemKind::Host {
            menu = menu.item(item(
                CardAction::Open,
                "Open".into(),
                IconName::ExternalLink,
            ));
            if self.data.general.workspace_enabled {
                menu = menu
                    .item(item(
                        CardAction::CurrentWorkspace,
                        format!("Open in {} Workspace", self.data.workspace().name),
                        IconName::LayoutGrid,
                    ))
                    .item(item(
                        CardAction::NewWorkspace,
                        "Open in New Workspace".into(),
                        IconName::Plus,
                    ));
            }
            if !hosts.is_empty() && hosts.iter().all(|host| host.is_remote()) {
                let mut protocols = Menu::default().card_menu().width(190.);
                let mut count = 0;
                for (enabled, action, label, icon) in [
                    (
                        hosts.iter().all(|host| host.supports_ssh()),
                        CardAction::Ssh,
                        "With SSH",
                        IconName::Terminal,
                    ),
                    (
                        hosts.iter().all(|host| host.connections.mosh),
                        CardAction::Mosh,
                        "With Mosh",
                        IconName::RadioTower,
                    ),
                    (
                        hosts.iter().all(|host| host.supports_ssh()),
                        CardAction::Sftp,
                        "With SFTP",
                        IconName::Folder,
                    ),
                    (
                        hosts.iter().all(|host| host.connections.telnet.is_some()),
                        CardAction::Telnet,
                        "With Telnet",
                        IconName::Network,
                    ),
                ] {
                    if enabled {
                        protocols = protocols.item(item(action, label.into(), icon));
                        count += 1;
                    }
                }
                if count > 0 {
                    menu = menu.item(
                        MenuItem::localized("Host")
                            .icon(IconName::Server)
                            .submenu(protocols),
                    );
                }
            }
        } else if single {
            match kind {
                ItemKind::Group => {
                    menu = menu.item(item(
                        CardAction::Open,
                        "Open Group".into(),
                        crate::assets::GROUP_ICON,
                    ))
                }
                ItemKind::SnippetPackage => {
                    menu = menu.item(item(CardAction::Open, "Open".into(), IconName::FolderOpen))
                }
                ItemKind::Snippet => {
                    menu = menu.item(item(CardAction::Open, "Run".into(), IconName::Play))
                }
                _ => {}
            }
        }
        if single {
            menu = menu.item(item(
                CardAction::Edit,
                match kind {
                    ItemKind::Host => "Edit Host",
                    ItemKind::Group => "Edit Group",
                    ItemKind::Proxy => "Edit Proxy",
                    ItemKind::SnippetPackage => "Rename",
                    _ => "Edit",
                }
                .into(),
                IconName::Pencil,
            ));
        }
        menu = menu.separator();
        if kind != ItemKind::SnippetPackage {
            menu = menu.item(item(
                CardAction::Duplicate,
                if kind == ItemKind::Group {
                    "Duplicate Group"
                } else {
                    "Duplicate"
                }
                .into(),
                IconName::Copy,
            ));
        }
        if kind == ItemKind::Group && single {
            menu = menu.item(item(
                CardAction::NewHost,
                "New Host in Group".into(),
                IconName::CirclePlus,
            ));
        }
        if kind == ItemKind::Host && single {
            menu = menu.item(
                item(
                    CardAction::CopySsh,
                    "Copy SSH Command".into(),
                    IconName::KeyRound,
                )
                .disabled(!hosts.iter().all(|host| host.supports_ssh())),
            );
        }
        menu.separator().item(
            item(
                CardAction::Delete,
                match kind {
                    ItemKind::Host => "Delete Host",
                    ItemKind::Group => "Delete Group",
                    ItemKind::Proxy => "Delete Proxy",
                    _ => "Delete",
                }
                .into(),
                IconName::Trash,
            )
            .destructive(true),
        )
    }

    fn card_action(
        &mut self,
        kind: ItemKind,
        ids: &BTreeSet<Id>,
        action: CardAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exiting.is_some() || ids.is_empty() {
            return;
        }
        let id = *ids.first().unwrap();
        self.selection = ids.clone();
        self.selection_kind = Some(kind);
        match action {
            CardAction::Edit => self.open_editor(kind, Some(id), window, cx),
            CardAction::Duplicate if kind == ItemKind::Snippet => {
                self.duplicate_snippets(ids.clone(), window, cx)
            }
            CardAction::Duplicate => self.duplicate_resources(kind, cx),
            CardAction::CopySsh => self.copy_ssh_command(id, cx),
            CardAction::Delete => self.delete_pending = true,
            CardAction::NewHost => {
                if kind == ItemKind::Group && self.data.groups.iter().any(|group| group.id == id) {
                    self.open_editor_in(ItemKind::Host, None, Some(id), None, window, cx);
                }
            }
            CardAction::Open if kind == ItemKind::Group => self.enter_group(Some(id), window, cx),
            CardAction::Open if kind == ItemKind::SnippetPackage => {
                self.open_snippet_package(Some(id), window, cx)
            }
            CardAction::Open if kind == ItemKind::Snippet => {
                self.run_snippet_locally(id, window, cx)
            }
            _ if kind == ItemKind::Host => {
                let hosts: Vec<_> = self
                    .data
                    .hosts
                    .iter()
                    .filter(|host| ids.contains(&host.id))
                    .cloned()
                    .collect();
                let Some(first) = hosts.first() else {
                    return;
                };
                let overview = matches!(
                    action,
                    CardAction::CurrentWorkspace | CardAction::NewWorkspace
                );
                let workspace = if action == CardAction::NewWorkspace {
                    let mut candidate = self.data.clone();
                    match candidate
                        .add_workspace(&first.name)
                        .and_then(|id| self.commit_workspace_metadata(candidate).map(|_| id))
                    {
                        Ok(id) => id,
                        Err(error) => {
                            self.notice = Some(error.to_string());
                            cx.notify();
                            return;
                        }
                    }
                } else {
                    self.data.selected_workspace
                };
                for host in hosts {
                    match action {
                        CardAction::Ssh => {
                            self.open_remote_in(host, workspace, false, RemoteMode::Ssh, window, cx)
                        }
                        CardAction::Mosh => self.open_remote_in(
                            host,
                            workspace,
                            false,
                            RemoteMode::Mosh,
                            window,
                            cx,
                        ),
                        CardAction::Sftp => self.open_sftp_in(host, workspace, window, cx),
                        CardAction::Telnet => {
                            self.open_saved_telnet_in(host, workspace, false, window, cx)
                        }
                        _ => self.open_ssh_in(host, workspace, overview, window, cx),
                    }
                }
            }
            _ => {}
        }
        cx.notify();
    }
}
