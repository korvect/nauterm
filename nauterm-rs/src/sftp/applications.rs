//! Open With: frozen local/remote selections, async catalog and native chooser.
use super::*;
use crate::{
    components::menu::Menu,
    editor_config::Editor,
    external_application::{self, Application, FileTarget},
};

fn application_item(
    label: impl Into<SharedString>,
    editor: Option<&Editor>,
    sudo: bool,
) -> MenuItem {
    let item = MenuItem::new(label)
        .destructive(sudo)
        .icon(if editor.is_some() {
            IconName::AppWindow
        } else {
            IconName::ExternalLink
        });
    if let Some(image) = editor.and_then(external_application::cached_icon) {
        item.image(image)
    } else {
        item
    }
}

#[derive(Clone)]
struct Scope {
    token: u64,
    path: FilePath,
    sudo: bool,
}
#[derive(Clone)]
enum FilePath {
    Local(PathBuf),
    Remote(String),
}
impl Scope {
    fn matches(&self, token: u64, entries: &[Entry]) -> bool {
        self.token == token
            && entries.len() == 1
            && !entries[0].directory
            && !entries[0].parent
            && match &self.path {
                FilePath::Local(path) => entries[0].native_path.as_ref() == Some(path),
                FilePath::Remote(path) => {
                    entries[0].native_path.is_none() && entries[0].path == *path
                }
            }
    }
}
fn configured_for_selection(
    config: &crate::editor_config::Editors,
    scope: &Scope,
    token: u64,
    entries: &[Entry],
) -> Option<Editor> {
    scope
        .matches(token, entries)
        .then(|| config.for_name(&entries[0].name).cloned())
        .flatten()
}

pub(super) fn menu(menu: Menu, owner: WeakEntity<FilePane>, cx: &App) -> Menu {
    menu_inner(menu, owner, false, cx)
}
pub(super) fn sudo_menu(menu: Menu, owner: WeakEntity<FilePane>, cx: &App) -> Menu {
    menu_inner(menu, owner, true, cx)
}
fn menu_inner(menu: Menu, owner: WeakEntity<FilePane>, sudo: bool, cx: &App) -> Menu {
    let menu = application_menu(menu, owner.clone(), sudo, cx);
    let selected = owner.upgrade().and_then(|pane| {
        let pane = pane.read(cx);
        let entries = pane.selected_entries();
        (pane.ssh_editor_available(cx)
            && entries.len() == 1
            && !entries[0].directory
            && !entries[0].parent)
            .then(|| (pane.endpoint_token, entries[0].path.clone()))
    });
    if let Some((token, path)) = selected {
        menu.item(
            MenuItem::localized("Open with SSH Editor")
                .destructive(sudo)
                .on_click(move |_, window, cx| {
                    let _ = owner.update(cx, |pane, cx| {
                        if sudo {
                            pane.open_ssh_editor_mode(token, &path, true, window, cx)
                        } else {
                            pane.open_ssh_editor(token, &path, window, cx)
                        }
                    });
                }),
        )
    } else {
        menu
    }
}
fn application_menu(menu: Menu, owner: WeakEntity<FilePane>, sudo: bool, cx: &App) -> Menu {
    let scope = owner.upgrade().and_then(|pane| {
        let pane = pane.read(cx);
        let entries = pane.selected_entries();
        (entries.len() == 1 && !entries[0].directory && !entries[0].parent)
            .then(|| {
                entries[0]
                    .native_path
                    .clone()
                    .map(FilePath::Local)
                    .unwrap_or_else(|| FilePath::Remote(entries[0].path.clone()))
            })
            .map(|path| Scope {
                token: pane.endpoint_token,
                path,
                sudo,
            })
    });
    let Some(scope) = scope else {
        return menu.item(
            MenuItem::localized("Open With")
                .destructive(sudo)
                .disabled(true),
        );
    };
    // Configured executables work without a native chooser on Linux/Windows.
    if !external_application::CAN_CHOOSE {
        let implicit = owner.upgrade().is_some_and(|pane| {
            pane.read(cx)
                .selected_entries()
                .first()
                .is_some_and(|entry| entry.can_open)
        });
        let configured = owner
            .upgrade()
            .is_some_and(|pane| pane.read(cx).configured_editor(&scope).is_some());
        if !implicit && !configured {
            return menu.item(
                MenuItem::localized("Open With")
                    .destructive(sudo)
                    .disabled(true),
            );
        }
        return menu.item(
            MenuItem::localized("Open With")
                .destructive(sudo)
                .submenu(items(&owner, &scope, &[], false, false, cx)),
        );
    }
    let revision = std::rc::Rc::new(std::cell::Cell::new(0u64));
    menu.item(
        MenuItem::localized("Open With")
            .destructive(sudo)
            .submenu_with(move |_, window, cx| {
                let generation = revision.get().wrapping_add(1);
                revision.set(generation);
                let revision = revision.clone();
                let initial = items(&owner, &scope, &[], true, false, cx);
                let configured_icon = owner
                    .upgrade()
                    .and_then(|pane| pane.read(cx).configured_editor(&scope));
                let path = scope.path.clone();
                let owner = owner.clone();
                let scope = scope.clone();
                cx.spawn_in(window, async move |menu, cx| {
                    let result = cx
                        .background_spawn(async move {
                            match path {
                                FilePath::Local(path) => external_application::list(&path),
                                FilePath::Remote(path) => external_application::list_for_name(
                                    path.rsplit('/').next().unwrap_or_default(),
                                ),
                            }
                        })
                        .await;
                    let failed = result.is_err();
                    let choices = result.unwrap_or_default();
                    let _ = menu.update_in(cx, |menu, _, cx| {
                        if revision.get() != generation
                            || !owner
                                .upgrade()
                                .is_some_and(|pane| pane.read(cx).local_file_current(&scope))
                        {
                            return;
                        }
                        let values = items(&owner, &scope, &choices, false, failed, cx);
                        menu.replace_items_if_open(values, cx);
                    });
                    let choices = cx
                        .background_spawn(async move {
                            if let Some(editor) = &configured_icon {
                                external_application::preload_icon(editor);
                            }
                            external_application::preload_application_icons(&choices);
                            choices
                        })
                        .await;
                    let _ = menu.update_in(cx, |menu, _, cx| {
                        if revision.get() != generation
                            || !owner
                                .upgrade()
                                .is_some_and(|pane| pane.read(cx).local_file_current(&scope))
                        {
                            return;
                        }
                        menu.replace_items_if_open(
                            items(&owner, &scope, &choices, false, failed, cx),
                            cx,
                        );
                    });
                })
                .detach();
                initial
            }),
    )
}
fn items(
    owner: &WeakEntity<FilePane>,
    scope: &Scope,
    applications: &[Application],
    loading: bool,
    failed: bool,
    cx: &App,
) -> Menu {
    let implicit = owner.upgrade().is_some_and(|pane| {
        let pane = pane.read(cx);
        pane.local_file_current(scope) && pane.selected_entries()[0].can_open
    });
    let configured = owner
        .upgrade()
        .and_then(|pane| pane.read(cx).configured_editor(scope));
    let mut menu = Menu::default();
    if implicit {
        let label = applications
            .iter()
            .find(|app| app.is_default)
            .map(|app| format!("{} (default)", app.name))
            .unwrap_or_else(|| "System Default Application".into());
        let owner = owner.clone();
        let scope = scope.clone();
        menu = menu
            .item(
                application_item(
                    label,
                    applications
                        .iter()
                        .find(|app| app.is_default)
                        .map(|app| Editor::from(app.clone()))
                        .as_ref(),
                    scope.sudo,
                )
                .on_click(move |_, window, cx| {
                    let _ = owner.update(cx, |pane, cx| {
                        pane.open_local_file(scope.clone(), None, false, window, cx)
                    });
                }),
            )
            .separator();
    }
    if let Some(editor) = configured.clone() {
        let owner = owner.clone();
        let scope = scope.clone();
        menu = menu
            .item(
                application_item(editor.label().to_owned(), Some(&editor), scope.sudo).on_click(
                    move |_, window, cx| {
                        let _ = owner.update(cx, |pane, cx| {
                            if pane.configured_editor(&scope).as_ref() == Some(&editor) {
                                pane.open_local_file(
                                    scope.clone(),
                                    Some(editor.clone()),
                                    false,
                                    window,
                                    cx,
                                );
                            }
                        });
                    },
                ),
            )
            .separator();
    }
    if loading || failed {
        menu = menu.item(
            MenuItem::localized(if loading {
                "Loading applications…"
            } else {
                "Unable to load applications"
            })
            .destructive(scope.sudo)
            .disabled(true),
        );
    }
    for app in applications.iter().filter(|app| {
        !app.is_default
            && !configured.as_ref().is_some_and(|editor| {
                editor.id() == Editor::from((*app).clone()).id()
                    || editor.label().eq_ignore_ascii_case(&app.name)
            })
    }) {
        let owner = owner.clone();
        let scope = scope.clone();
        let app = app.clone();
        menu = menu.item(
            application_item(
                app.name.clone(),
                Some(&Editor::from(app.clone())),
                scope.sudo,
            )
            .on_click(move |_, window, cx| {
                let _ = owner.update(cx, |pane, cx| {
                    pane.open_local_file(
                        scope.clone(),
                        Some(Editor::from(app.clone())),
                        false,
                        window,
                        cx,
                    )
                });
            }),
        );
    }
    if external_application::CAN_CHOOSE {
        let owner = owner.clone();
        let scope = scope.clone();
        menu = menu.separator().item(
            MenuItem::localized("Other...")
                .destructive(scope.sudo)
                .icon(IconName::Ellipsis)
                .on_click(move |_, window, cx| {
                    let _ = owner.update(cx, |pane, cx| {
                        pane.open_local_file(scope.clone(), None, true, window, cx)
                    });
                }),
        );
    }
    menu
}
impl FilePane {
    fn configured_editor(&self, scope: &Scope) -> Option<Editor> {
        self.local_file_current(scope)
            .then(|| {
                configured_for_selection(
                    &self.data.sftp_editors,
                    scope,
                    self.endpoint_token,
                    &self.selected_entries(),
                )
            })
            .flatten()
    }
    fn local_file_current(&self, scope: &Scope) -> bool {
        let entries = self.selected_entries();
        !crate::transfer_queue::is_shutting_down()
            && self.stage == Stage::Files
            && self.remote.is_some() == matches!(scope.path, FilePath::Remote(_))
            && self.active
            && !self.loading
            && scope.matches(self.endpoint_token, &entries)
    }
    pub(super) fn open_local_default(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_local_file(
            Scope {
                token: self.endpoint_token,
                path: FilePath::Local(path),
                sudo: false,
            },
            None,
            false,
            window,
            cx,
        );
    }
    fn open_local_file(
        &mut self,
        scope: Scope,
        selected: Option<Editor>,
        choose: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.local_file_current(&scope) || self.action_work.is_some() || self.dialog.is_some() {
            return;
        }
        if let FilePath::Remote(path) = &scope.path {
            if scope.sudo {
                self.action_sudo_open(scope.token, path.clone(), selected, choose, window, cx);
            } else {
                self.open_remote_file(scope.token, path.clone(), selected, choose, window, cx);
            }
            return;
        }
        self.notice = None;
        self.action_work = Some(cx.spawn_in(window, async move |this, cx| {
            let result = async {
                let FilePath::Local(path) = scope.path.clone() else {
                    unreachable!()
                };
                let explicit = choose || selected.is_some();
                let target = cx
                    .background_spawn(async move { FileTarget::prepare(path, explicit) })
                    .await?;
                #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
                let selected = if choose {
                    let choice = this.update_in(cx, |pane, window, cx| {
                        // Async file validation must not attach a native sheet
                        // to a Settings/other window opened in the meantime.
                        if pane.local_file_current(&scope)
                            && window.is_window_active()
                            && !crate::components::overlay::is_modal_open(window, cx)
                        {
                            Some(external_application::choose(window))
                        } else {
                            None
                        }
                    })?;
                    let Some(choice) = choice else {
                        return Ok(());
                    };
                    let Some(app) = choice?.receive().await? else {
                        return Ok(());
                    };
                    Some(Editor::from(app))
                } else {
                    selected
                };
                let target = cx
                    .background_spawn(async move {
                        target.revalidate()?;
                        Ok::<_, anyhow::Error>(target)
                    })
                    .await?;
                let started = this.update_in(cx, |pane, window, cx| {
                    if !pane.local_file_current(&scope)
                        || crate::components::overlay::is_modal_open(window, cx)
                    {
                        return None;
                    }
                    Some(external_application::open_editor(
                        &target.path,
                        selected.as_ref(),
                    ))
                })?;
                if let Some(started) = started {
                    started?.recv().await??;
                }
                Ok::<_, anyhow::Error>(())
            }
            .await;
            let _ = this.update_in(cx, |pane, _, cx| {
                if pane.endpoint_token != scope.token {
                    return;
                }
                pane.action_work = None;
                if let Err(error) = result
                    && pane.local_file_current(&scope)
                {
                    pane.notice = Some(Message::error(format!("Unable to open file: {error}")));
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{Entry, FilePath, PathBuf, Scope};
    #[test]
    fn configured_editor_actions_recheck_extension_endpoint_selection_and_configuration() {
        use crate::editor_config::{Editor, Editors};
        let editor = Editor::Command {
            label: "Fixture".into(),
            executable: "synthetic-editor".into(),
            arguments: vec![],
        };
        let mut config = Editors {
            external_editor: Some(editor.clone()),
            ..Default::default()
        };
        let scope = Scope {
            token: 8,
            path: FilePath::Remote("/home/fixture/unsafe.sh".into()),
            sudo: false,
        };
        let mut entry = Entry {
            native_path: None,
            path: "/home/fixture/unsafe.sh".into(),
            name: "unsafe.sh".into(),
            directory: false,
            parent: false,
            can_open: false,
            size: 0,
            modified: None,
            mode: None,
        };
        assert_eq!(
            super::configured_for_selection(&config, &scope, 8, std::slice::from_ref(&entry)),
            Some(editor.clone())
        );
        assert!(
            !entry.can_open,
            "Explicit configuration does not enable an unsafe implicit association."
        );
        assert!(
            super::configured_for_selection(&config, &scope, 9, std::slice::from_ref(&entry))
                .is_none()
        );
        assert!(
            super::configured_for_selection(&config, &scope, 8, &[entry.clone(), entry.clone()])
                .is_none()
        );
        config.text_extensions.clear();
        assert!(
            super::configured_for_selection(&config, &scope, 8, std::slice::from_ref(&entry))
                .is_none()
        );
        config.text_extensions.push("sh".into());
        config.external_editor = None;
        assert_ne!(
            super::configured_for_selection(&config, &scope, 8, std::slice::from_ref(&entry)),
            Some(editor)
        );
        entry.path = "/home/fixture/other.sh".into();
        assert!(super::configured_for_selection(&config, &scope, 8, &[entry]).is_none());
    }
    #[test]
    fn open_with_results_are_bound_to_one_exact_native_path_and_endpoint() {
        let scope = Scope {
            token: 8,
            path: FilePath::Local(PathBuf::from("/synthetic/中文.txt")),
            sudo: false,
        };
        let mut entry = Entry {
            native_path: Some(PathBuf::from("/synthetic/中文.txt")),
            can_open: true,
            path: "display text is not an authority".into(),
            name: "display".into(),
            directory: false,
            parent: false,
            size: 0,
            modified: None,
            mode: None,
        };
        assert!(scope.matches(8, &[entry.clone()]));
        assert!(!scope.matches(9, &[entry.clone()]));
        assert!(!scope.matches(8, &[]));
        assert!(!scope.matches(8, &[entry.clone(), entry.clone()]));
        entry.native_path = Some(PathBuf::from("/synthetic/other.txt"));
        assert!(!scope.matches(8, &[entry.clone()]));
        entry.native_path = None;
        assert!(!scope.matches(8, &[entry.clone()]));
        entry.native_path = Some(PathBuf::from("/synthetic/中文.txt"));
        entry.directory = true;
        assert!(!scope.matches(8, &[entry.clone()]));
        entry.directory = false;
        entry.parent = true;
        assert!(!scope.matches(8, &[entry]));
    }
    #[test]
    fn remote_open_with_cannot_be_redirected_to_a_local_or_different_selected_file() {
        let scope = Scope {
            token: 9,
            path: FilePath::Remote("/home/fixture/file.txt".into()),
            sudo: false,
        };
        let mut entry = Entry {
            native_path: None,
            can_open: true,
            path: "/home/fixture/file.txt".into(),
            name: "file.txt".into(),
            directory: false,
            parent: false,
            size: 0,
            modified: None,
            mode: None,
        };
        assert!(scope.matches(9, &[entry.clone()]));
        assert!(!scope.matches(10, &[entry.clone()]));
        entry.native_path = Some(PathBuf::from("/home/fixture/file.txt"));
        assert!(!scope.matches(9, &[entry.clone()]));
        entry.native_path = None;
        entry.path = "/home/fixture/other.txt".into();
        assert!(!scope.matches(9, &[entry]));
    }
}
