//! Native application menus; product overlays remain separate controls.
use gpui_kit::*;
#[cfg(target_os = "macos")]
mod native_edit;
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
        FindNext,
        FindPrevious,
        FindSelection,
        JumpToSelection,
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
    view: WeakEntity<crate::ui::Nauterm>,
}
impl Global for MainWindow {}

pub fn register_main_window(window: &Window, view: &Entity<crate::ui::Nauterm>, cx: &mut App) {
    cx.set_global(MainWindow {
        handle: window.window_handle(),
        view: view.downgrade(),
    });
    crate::window_chrome::register_window(window);
    if cx.try_global::<ConnectionLinks>().is_some() {
        cx.defer(dispatch_connection_links);
    }
}

#[derive(Default)]
struct ConnectionLinks(Vec<crate::quick_connect::Target>);
impl Global for ConnectionLinks {}

pub fn open_connection_links(urls: Vec<String>, cx: &mut App) {
    let links = urls
        .iter()
        .filter_map(|url| crate::quick_connect::connection_url(url))
        .collect::<Vec<_>>();
    if links.is_empty() {
        return;
    }
    if cx.try_global::<ConnectionLinks>().is_none() {
        cx.set_global(ConnectionLinks::default());
    }
    cx.global_mut::<ConnectionLinks>().0.extend(links);
    if windows::has_main_window(cx) {
        dispatch_connection_links(cx);
    } else {
        windows::open(cx);
    }
}

fn dispatch_connection_links(cx: &mut App) {
    if !windows::has_main_window(cx) {
        return;
    }
    let (handle, view) = {
        let main = cx.global::<MainWindow>();
        (main.handle, main.view.clone())
    };
    let links = std::mem::take(&mut cx.global_mut::<ConnectionLinks>().0);
    if !links.is_empty() {
        let _ = handle.update(cx, |_, window, cx| {
            window.activate_window();
            let _ = view.update(cx, |this, cx| this.open_connection_links(links, window, cx));
        });
    }
}

#[cfg(target_os = "macos")]
pub(crate) const SERVICES_MENU_KEY: &str = "NMo-om-nkz.title";

#[cfg(target_os = "macos")]
pub(crate) fn menu_title(
    key: &str,
    fallback: &str,
) -> objc2::rc::Retained<objc2_foundation::NSString> {
    use objc2_foundation::{NSBundle, NSString};
    NSBundle::mainBundle().localizedStringForKey_value_table(
        &NSString::from_str(key),
        Some(&NSString::from_str(fallback)),
        Some(&NSString::from_str("MainMenu")),
    )
}

#[cfg(target_os = "macos")]
fn menus(text: impl Fn(&'static str, &'static str) -> SharedString) -> Vec<Menu> {
    use gpui_kit::base::input::{Copy, Cut, Delete, Paste, Redo, SelectAll, Undo};
    let (edit_key, edit_title) = native_edit::EDIT;
    let (find_key, find_title) = native_edit::FIND;
    let (paste_style_key, paste_style_title) = native_edit::PASTE_STYLE;
    vec![
        Menu::new("Nauterm RS").items([
            MenuItem::action(text("5kV-Vb-QxS.title", "About Nauterm RS"), About),
            MenuItem::action(
                text("Nau-Upd-Che.title", "Check for Updates…"),
                CheckForUpdates,
            ),
            MenuItem::separator(),
            MenuItem::action(text("BOF-NM-1cW.title", "Settings…"), Settings),
            MenuItem::separator(),
            // The pinned gpui-pre-macos Services helper passes an NSMenuItem
            // where AppKit needs NSMenu. window_chrome attaches the native role.
            MenuItem::submenu(Menu::new(text(SERVICES_MENU_KEY, "Services"))),
            MenuItem::separator(),
            MenuItem::action(text("Olw-nP-bQN.title", "Hide Nauterm RS"), Hide),
            MenuItem::action(text("Vdr-fp-XzO.title", "Hide Others"), HideOthers),
            MenuItem::action(text("Kd2-mp-pUS.title", "Show All"), ShowAll),
            MenuItem::separator(),
            MenuItem::action(text("4sb-4s-VLi.title", "Quit Nauterm RS"), crate::ui::Quit),
        ]),
        Menu::new(text("3Lr-6F-vQ2.title", "File")).items([
            MenuItem::action(
                text("Nau-New-Connection.title", "New Connection…"),
                NewConnection,
            ),
            MenuItem::action(
                text("Nau-NL-Terminal.title", "New Local Terminal"),
                NewLocalTerminal,
            ),
            MenuItem::separator(),
            MenuItem::action(text("Nau-Close-Tab.title", "Close Tab"), CloseTab),
            MenuItem::action(text("Close-Window.title", "Close Window"), CloseWindow),
        ]),
        Menu::new(text(edit_key, edit_title)).items([
            MenuItem::os_action(text("dRJ-4n-Yzg.title", "Undo"), Undo, OsAction::Undo),
            MenuItem::os_action(text("6dh-zS-Vam.title", "Redo"), Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action(text("uRl-iY-unG.title", "Cut"), Cut, OsAction::Cut),
            MenuItem::os_action(text("x3v-GG-iWU.title", "Copy"), Copy, OsAction::Copy),
            MenuItem::os_action(text("gVA-U4-sdL.title", "Paste"), Paste, OsAction::Paste),
            MenuItem::os_action(
                text(paste_style_key, paste_style_title),
                Paste,
                OsAction::Paste,
            ),
            MenuItem::action(text("pa3-QI-u2k.title", "Delete"), Delete),
            MenuItem::os_action(
                text("Ruw-6m-B2m.title", "Select All"),
                SelectAll,
                OsAction::SelectAll,
            ),
            MenuItem::separator(),
            MenuItem::submenu(Menu::new(text(find_key, find_title)).items([
                MenuItem::action(text("Xz5-n4-O0W.title", "Find…"), Find),
                MenuItem::action(text("q09-fT-Sye.title", "Find Next"), FindNext),
                MenuItem::action(text("OwM-mh-QMV.title", "Find Previous"), FindPrevious),
                MenuItem::action(
                    text("buJ-ug-pKt.title", "Use Selection for Find"),
                    FindSelection,
                ),
                MenuItem::action(
                    text("S0p-oC-mLd.title", "Jump to Selection"),
                    JumpToSelection,
                ),
            ])),
        ]),
        Menu::new(text("H8h-7b-M4v.title", "View")).items([
            MenuItem::action(text("Nau-Previous-Tab.title", "Previous Tab"), PreviousTab),
            MenuItem::action(text("Nau-Next-Tab.title", "Next Tab"), NextTab),
            MenuItem::separator(),
            MenuItem::action(
                text("Toggle-Full-Screen.title", "Toggle Full Screen"),
                FullScreen,
            ),
        ]),
        // GPUI recognizes this exact name and calls NSApplication.setWindowsMenu.
        Menu::new("Window").items([
            MenuItem::action(text("OY7-WF-poV.title", "Minimize"), Minimize),
            MenuItem::action(text("R4o-n2-Eq4.title", "Zoom"), Zoom),
            MenuItem::separator(),
            MenuItem::action(
                text("LE2-aR-0XJ.title", "Bring All to Front"),
                BringAllToFront,
            ),
        ]),
        Menu::new("Help").items([
            MenuItem::action(
                text("Nau-Documentation.title", "Nauterm Documentation"),
                Documentation,
            ),
            MenuItem::action(
                text("Nau-Report-Issue.title", "Report an Issue…"),
                ReportIssue,
            ),
        ]),
    ]
}

#[cfg(target_os = "macos")]
fn rebuild_menus(cx: &mut App) {
    cx.set_menus(menus(|key, fallback| {
        menu_title(key, fallback).to_string().into()
    }));
    crate::window_chrome::register_app_menus();
    native_edit::install();
    // A rebuilt Window menu needs the existing native windows reattached.
    cx.defer(|cx| {
        for handle in cx.windows() {
            let _ = handle.update(cx, |_, window, _| {
                crate::window_chrome::refresh_window_registration(window)
            });
        }
    });
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
            KeyBinding::new("cmd-g", FindNext, Some("TerminalView")),
            KeyBinding::new("cmd-shift-g", FindPrevious, Some("TerminalView")),
            KeyBinding::new("cmd-e", FindSelection, Some("TerminalView")),
            KeyBinding::new("cmd-j", JumpToSelection, Some("TerminalView")),
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
    use gpui_kit::{KeyBinding, Keymap, Keystroke, SharedString, base::input::Copy};

    #[test]
    fn native_menu_language_keeps_system_roles_and_the_same_command_actions() {
        use gpui_kit::MenuItem;
        let english = menus(|_, fallback| SharedString::from(fallback));
        let translated = menus(|key, fallback| match key {
            "3Lr-6F-vQ2.title" => "文件".into(),
            "x3v-GG-iWU.title" => "拷贝".into(),
            _ => fallback.into(),
        });
        assert_eq!(english.len(), translated.len());
        for name in ["Window", "Help"] {
            assert!(english.iter().any(|menu| menu.name == name));
            assert!(translated.iter().any(|menu| menu.name == name));
        }
        assert!(translated.iter().any(|menu| menu.name == "文件"));
        fn actions(items: &[MenuItem], output: &mut Vec<std::any::TypeId>) {
            for item in items {
                match item {
                    MenuItem::Action { action, .. } => output.push(action.as_any().type_id()),
                    MenuItem::Submenu(menu) => actions(&menu.items, output),
                    _ => {}
                }
            }
        }
        let menu_actions = |menus: &[gpui_kit::Menu]| {
            let mut ids = Vec::new();
            for menu in menus {
                actions(&menu.items, &mut ids);
            }
            ids
        };
        assert_eq!(menu_actions(&english), menu_actions(&translated));

        let find = english
            .iter()
            .find(|menu| menu.name == "Edit")
            .and_then(|menu| {
                menu.items.iter().find_map(|item| match item {
                    MenuItem::Submenu(menu) if menu.name == "Find" => Some(menu),
                    _ => None,
                })
            })
            .expect("Find submenu in Edit");
        let mut find_actions = Vec::new();
        actions(&find.items, &mut find_actions);
        assert_eq!(
            find_actions,
            [
                std::any::TypeId::of::<super::Find>(),
                std::any::TypeId::of::<super::FindNext>(),
                std::any::TypeId::of::<super::FindPrevious>(),
                std::any::TypeId::of::<super::FindSelection>(),
                std::any::TypeId::of::<super::JumpToSelection>(),
            ]
        );
        assert!(translated.iter().flat_map(|menu| &menu.items).any(|item| matches!(item, MenuItem::Action { name, action, .. } if name == "拷贝" && action.as_any().is::<Copy>())));
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
