//! Native application menus; product overlays remain separate controls.
use gpui_kit::*;
mod windows;
#[cfg(target_os = "macos")]
pub use windows::close_main_window;
pub use windows::set_window_factory;

gpui_kit::actions!(
    native_menu,
    [
        About,
        CheckForUpdates,
        Settings,
        NewConnection,
        NewLocalTerminal,
        CloseTab,
        PreviousTab,
        NextTab,
        CloseWindow,
        Find,
        Hide,
        HideOthers,
        ShowAll,
        Minimize,
        Zoom,
        FullScreen,
        BringAllToFront,
        Documentation,
        ReportIssue
    ]
);

#[derive(Clone, PartialEq, Action)]
#[action(namespace = native_menu, no_json)]
pub struct SelectTab {
    pub index: u8,
}

struct MainWindow {
    handle: AnyWindowHandle,
    #[cfg(target_os = "macos")]
    view: WeakEntity<crate::ui::Nauterm>,
}
impl Global for MainWindow {}

pub fn register_main_window(window: &Window, view: &Entity<crate::ui::Nauterm>, cx: &mut App) {
    cx.set_global(MainWindow {
        handle: window.window_handle(),
        #[cfg(target_os = "macos")]
        view: view.downgrade(),
    });
    #[cfg(not(target_os = "macos"))]
    let _ = view;
    crate::window_chrome::register_window(window);
}

#[cfg(target_os = "macos")]
fn menus(language: crate::localization::Language) -> Vec<Menu> {
    let text = |source| crate::localization::text_for(language, source);
    use gpui_kit::base::input::{Copy, Cut, Paste, Redo, SelectAll, Undo};
    vec![
        Menu::new("Nauterm RS").items([
            MenuItem::action(text("rs.appMenu.about"), About),
            MenuItem::action(text("rs.appMenu.checkForUpdates"), CheckForUpdates),
            MenuItem::separator(),
            MenuItem::action(text("rs.appMenu.settings"), Settings),
            MenuItem::separator(),
            // The pinned gpui-pre-macos Services helper passes an NSMenuItem
            // where AppKit needs NSMenu. window_chrome attaches the native role.
            MenuItem::submenu(Menu::new(text("rs.appMenu.services"))),
            MenuItem::separator(),
            MenuItem::action(text("rs.appMenu.hide"), Hide),
            MenuItem::action(text("rs.appMenu.hideOthers"), HideOthers),
            MenuItem::action(text("rs.appMenu.showAll"), ShowAll),
            MenuItem::separator(),
            MenuItem::action(text("rs.appMenu.quit"), crate::ui::Quit),
        ]),
        Menu::new(text("rs.appMenu.file")).items([
            MenuItem::action(text("rs.appMenu.newConnection"), NewConnection),
            MenuItem::action(text("rs.appMenu.newLocalTerminal"), NewLocalTerminal),
            MenuItem::separator(),
            MenuItem::action(text("rs.appMenu.closeTab"), CloseTab),
            MenuItem::action(text("rs.appMenu.closeWindow"), CloseWindow),
        ]),
        Menu::new(text("rs.appMenu.edit")).items([
            MenuItem::os_action(text("rs.appMenu.undo"), Undo, OsAction::Undo),
            MenuItem::os_action(text("rs.appMenu.redo"), Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action(text("rs.appMenu.cut"), Cut, OsAction::Cut),
            MenuItem::os_action(text("rs.appMenu.copy"), Copy, OsAction::Copy),
            MenuItem::os_action(text("rs.appMenu.paste"), Paste, OsAction::Paste),
            MenuItem::os_action(text("rs.appMenu.selectAll"), SelectAll, OsAction::SelectAll),
            MenuItem::separator(),
            MenuItem::action(text("rs.appMenu.find"), Find),
        ]),
        Menu::new(text("rs.appMenu.view")).items([
            MenuItem::action(text("rs.appMenu.previousTab"), PreviousTab),
            MenuItem::action(text("rs.appMenu.nextTab"), NextTab),
            MenuItem::separator(),
            MenuItem::action(text("rs.appMenu.toggleFullScreen"), FullScreen),
        ]),
        // GPUI recognizes this exact name and calls NSApplication.setWindowsMenu.
        Menu::new("Window").items([
            MenuItem::action(text("rs.appMenu.minimize"), Minimize),
            MenuItem::action(text("rs.appMenu.zoom"), Zoom),
            MenuItem::separator(),
            MenuItem::action(text("rs.appMenu.bringAllToFront"), BringAllToFront),
        ]),
        Menu::new("Help").items([
            MenuItem::action(text("rs.appMenu.documentation"), Documentation),
            MenuItem::action(text("rs.appMenu.reportIssue"), ReportIssue),
        ]),
    ]
}

#[cfg(target_os = "macos")]
fn rebuild_menus(cx: &mut App) {
    cx.set_menus(menus(crate::localization::language(cx)));
    crate::window_chrome::register_app_menus(cx);
    // A rebuilt Window menu needs the existing native windows reattached.
    cx.defer(|cx| {
        for handle in cx.windows() {
            let _ = handle.update(cx, |_, window, _| {
                crate::window_chrome::refresh_window_registration(window)
            });
        }
    });
}

pub fn refresh_language(cx: &mut App) {
    #[cfg(target_os = "macos")]
    cx.defer(rebuild_menus);
    #[cfg(not(target_os = "macos"))]
    let _ = cx;
}

#[cfg(target_os = "macos")]
fn active_window(cx: &mut App, action: fn(&mut Window, &mut App)) {
    // A native menu may dispatch while its focused window is already borrowed.
    cx.defer(move |cx| {
        if let Some(handle) = cx.active_window() {
            let _ = handle.update(cx, |_, window, cx| action(window, cx));
        }
    });
}

#[cfg(target_os = "macos")]
fn main_window(
    cx: &mut App,
    action: fn(&mut crate::ui::Nauterm, &mut Window, &mut Context<crate::ui::Nauterm>),
) {
    if !windows::has_main_window(cx) && !windows::open(cx) {
        return;
    }
    if let Some(main) = cx.try_global::<MainWindow>() {
        let handle = main.handle;
        let view = main.view.clone();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |this, cx| action(this, window, cx));
            });
        });
    }
}

pub fn init(cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        cx.on_action::<Settings>(|_, cx| {
            main_window(cx, |this, window, cx| this.open_settings(window, cx));
        });
        cx.on_action::<About>(|_, cx| {
            main_window(cx, |this, window, cx| {
                this.open_settings_at(Some(crate::settings::Page::About), window, cx)
            });
        });
        cx.on_action::<CheckForUpdates>(|_, cx| {
            main_window(cx, |this, window, cx| this.check_native_updates(window, cx));
        });
        cx.on_action::<NewConnection>(|_, cx| {
            main_window(cx, |this, window, cx| {
                this.open_quick_connect(false, window, cx)
            });
        });
        cx.on_action::<NewLocalTerminal>(|_, cx| {
            main_window(cx, |this, window, cx| this.open_local(window, cx));
        });
        // Cmd-Q must also work with no windows, including during a window drain.
        cx.on_action::<crate::ui::Quit>(|_, cx| {
            if windows::defer_quit(cx) {
                return;
            }
            if let Some(main) = cx
                .try_global::<MainWindow>()
                .filter(|_| windows::has_main_window(cx))
            {
                let handle = main.handle;
                let view = main.view.clone();
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.request_exit(crate::settings::ExitAction::Quit, window, cx);
                        });
                    });
                });
            } else {
                cx.quit();
            }
        });
        cx.on_action::<Hide>(|_, cx| cx.hide());
        cx.on_action::<HideOthers>(|_, cx| cx.hide_other_apps());
        cx.on_action::<ShowAll>(|_, cx| cx.unhide_other_apps());
        cx.on_action::<Minimize>(|_, cx| active_window(cx, |window, _| window.minimize_window()));
        cx.on_action::<Zoom>(|_, cx| active_window(cx, |window, _| window.zoom_window()));
        cx.on_action::<FullScreen>(|_, cx| {
            active_window(cx, |window, _| window.toggle_fullscreen())
        });
        cx.on_action::<BringAllToFront>(|_, cx| {
            cx.defer(|_| crate::window_chrome::bring_all_to_front())
        });
        cx.on_action::<Documentation>(|_, cx| {
            cx.open_url("https://github.com/korvect/nauterm#readme")
        });
        cx.on_action::<ReportIssue>(|_, cx| {
            cx.open_url("https://github.com/korvect/nauterm/issues")
        });
        cx.bind_keys([
            KeyBinding::new("cmd-h", Hide, None),
            KeyBinding::new("cmd-alt-h", HideOthers, None),
            KeyBinding::new("cmd-m", Minimize, None),
            KeyBinding::new("ctrl-cmd-f", FullScreen, None),
            KeyBinding::new("cmd-shift-w", CloseWindow, None),
        ]);
        rebuild_menus(cx);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = cx;
}

pub fn reopen(cx: &mut App) {
    let main = cx.try_global::<MainWindow>().map(|main| main.handle);
    if let Some(handle) = main.filter(|_| windows::has_main_window(cx)) {
        let result = handle.update(cx, |_, window, _| {
            crate::window_chrome::register_window(window);
            window.activate_window();
        });
        if result.is_ok() {
            return;
        }
    }
    windows::open(cx);
}

#[cfg(target_os = "macos")]
fn owns_binding(action: &dyn Action) -> bool {
    let action = action.as_any();
    action.is::<NewConnection>()
        || action.is::<NewLocalTerminal>()
        || action.is::<Settings>()
        || action.is::<CloseTab>()
        || action.is::<PreviousTab>()
        || action.is::<NextTab>()
        || action.is::<SelectTab>()
        || action.is::<Find>()
}

#[cfg(target_os = "macos")]
fn shortcut_bindings(config: &crate::shortcuts::Config, cx: &App) -> Vec<KeyBinding> {
    use crate::shortcuts::{Binding, Command};
    fn binding<A: Action>(
        config: &crate::shortcuts::Config,
        command: Command,
        action: A,
        context: &'static str,
        cx: &App,
    ) -> Option<KeyBinding> {
        Binding::parse(&config.raw(command)).and_then(|binding| {
            let stroke = binding.stroke(true, command.scope() == crate::shortcuts::Scope::Terminal);
            KeyBinding::load(
                &stroke.unparse(OperatingSystem::Mac),
                Box::new(action),
                Some(KeyBindingContextPredicate::parse(context).unwrap().into()),
                false,
                None,
                cx,
            )
            .inspect_err(|error| eprintln!("Cannot bind native menu shortcut: {error}"))
            .ok()
        })
    }
    [
        binding(config, Command::QuickConnect, NewConnection, "Nauterm", cx),
        binding(
            config,
            Command::NewLocalTerminal,
            NewLocalTerminal,
            "Nauterm",
            cx,
        ),
        binding(config, Command::OpenSettings, Settings, "Nauterm", cx),
        binding(config, Command::CloseTab, CloseTab, "Nauterm", cx),
        binding(config, Command::PreviousTab, PreviousTab, "Nauterm", cx),
        binding(config, Command::NextTab, NextTab, "Nauterm", cx),
        binding(config, Command::Search, Find, "TerminalView", cx),
    ]
    .into_iter()
    .flatten()
    .chain((0..9).filter_map(|index| {
        binding(
            config,
            Command::Tab(index),
            SelectTab { index },
            "Nauterm",
            cx,
        )
    }))
    .collect()
}

pub fn update_shortcuts(config: &crate::shortcuts::Config, cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        // Preserve every other component's binding and its original precedence.
        // Replacing only our entries also removes disabled/remapped old chords.
        let existing: Vec<_> = cx.key_bindings().borrow().bindings().cloned().collect();
        let replacements = shortcut_bindings(config, cx);
        cx.clear_key_bindings();
        cx.bind_keys(replace_bindings(existing, replacements));
        rebuild_menus(cx);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (config, cx);
}

#[cfg(target_os = "macos")]
fn replace_bindings(existing: Vec<KeyBinding>, replacements: Vec<KeyBinding>) -> Vec<KeyBinding> {
    existing
        .into_iter()
        .filter(|binding| !owns_binding(binding.action()))
        .chain(replacements)
        .collect()
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::{NewConnection, Settings, menus, replace_bindings};
    use gpui_kit::{KeyBinding, Keymap, Keystroke, base::input::Copy};

    #[test]
    fn native_menu_language_keeps_system_roles_and_the_same_command_actions() {
        use crate::localization::Language;
        use gpui_kit::MenuItem;
        let english = menus(Language::English);
        let chinese = menus(Language::Chinese);
        assert_eq!(english.len(), chinese.len());
        for name in ["Window", "Help"] {
            assert!(english.iter().any(|menu| menu.name == name));
            assert!(chinese.iter().any(|menu| menu.name == name));
        }
        assert!(chinese.iter().any(|menu| menu.name == "文件"));
        let actions = |menus: &[gpui_kit::Menu]| {
            menus
                .iter()
                .flat_map(|menu| &menu.items)
                .filter_map(|item| match item {
                    MenuItem::Action { action, .. } => Some(action.as_any().type_id()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(actions(&english), actions(&chinese));
        assert!(chinese.iter().flat_map(|menu| &menu.items).any(|item| matches!(item, MenuItem::Action { name, action, .. } if name == "复制" && action.as_any().is::<Copy>())));
    }

    #[test]
    fn rebinding_native_menus_removes_stale_chords_and_preserves_editor_bindings() {
        let initial = vec![
            KeyBinding::new("cmd-c", Copy, Some("Input")),
            KeyBinding::new("cmd-t", NewConnection, Some("Nauterm")),
            KeyBinding::new("cmd-,", Settings, Some("Nauterm")),
        ];
        let replacements = vec![KeyBinding::new("cmd-alt-r", NewConnection, Some("Nauterm"))];
        let bindings = replace_bindings(initial, replacements.clone());
        assert!(
            bindings
                .iter()
                .any(|binding| binding.action().as_any().is::<Copy>())
        );
        assert!(
            !bindings
                .iter()
                .any(|binding| binding.action().as_any().is::<Settings>())
        );
        let map = Keymap::new(bindings);
        let matches = |key: &'static str| {
            map.all_bindings_for_input(&[Keystroke::parse(key).unwrap()])
                .iter()
                .any(|binding| binding.action().as_any().is::<NewConnection>())
        };
        assert!(!matches("cmd-t"));
        assert!(matches("cmd-alt-r"));
        let second = replace_bindings(map.bindings().cloned().collect(), replacements);
        assert_eq!(
            second
                .iter()
                .filter(|binding| binding.action().as_any().is::<NewConnection>())
                .count(),
            1
        );
    }
}
