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
pub(crate) fn menu_title(source: &str) -> objc2::rc::Retained<objc2_foundation::NSString> {
    use objc2_foundation::{NSBundle, NSString};
    NSBundle::mainBundle().localizedStringForKey_value_table(
        &NSString::from_str(source),
        None,
        Some(&NSString::from_str("MainMenu")),
    )
}

#[cfg(target_os = "macos")]
fn menus(text: impl Fn(&'static str) -> SharedString) -> Vec<Menu> {
    use gpui_kit::base::input::{Copy, Cut, Paste, Redo, SelectAll, Undo};
    vec![
        Menu::new("Nauterm RS").items([
            MenuItem::action(text("About Nauterm RS"), About),
            MenuItem::action(text("Check for Updates…"), CheckForUpdates),
            MenuItem::separator(),
            MenuItem::action(text("Settings…"), Settings),
            MenuItem::separator(),
            // The pinned gpui-pre-macos Services helper passes an NSMenuItem
            // where AppKit needs NSMenu. window_chrome attaches the native role.
            MenuItem::submenu(Menu::new(text("Services"))),
            MenuItem::separator(),
            MenuItem::action(text("Hide Nauterm RS"), Hide),
            MenuItem::action(text("Hide Others"), HideOthers),
            MenuItem::action(text("Show All"), ShowAll),
            MenuItem::separator(),
            MenuItem::action(text("Quit Nauterm RS"), crate::ui::Quit),
        ]),
        Menu::new(text("File")).items([
            MenuItem::action(text("New Connection…"), NewConnection),
            MenuItem::action(text("New Local Terminal"), NewLocalTerminal),
            MenuItem::separator(),
            MenuItem::action(text("Close Tab"), CloseTab),
            MenuItem::action(text("Close Window"), CloseWindow),
        ]),
        Menu::new(text("Edit")).items([
            MenuItem::os_action(text("Undo"), Undo, OsAction::Undo),
            MenuItem::os_action(text("Redo"), Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action(text("Cut"), Cut, OsAction::Cut),
            MenuItem::os_action(text("Copy"), Copy, OsAction::Copy),
            MenuItem::os_action(text("Paste"), Paste, OsAction::Paste),
            MenuItem::os_action(text("Select All"), SelectAll, OsAction::SelectAll),
            MenuItem::separator(),
            MenuItem::action(text("Find…"), Find),
        ]),
        Menu::new(text("View")).items([
            MenuItem::action(text("Previous Tab"), PreviousTab),
            MenuItem::action(text("Next Tab"), NextTab),
            MenuItem::separator(),
            MenuItem::action(text("Toggle Full Screen"), FullScreen),
        ]),
        // GPUI recognizes this exact name and calls NSApplication.setWindowsMenu.
        Menu::new("Window").items([
            MenuItem::action(text("Minimize"), Minimize),
            MenuItem::action(text("Zoom"), Zoom),
            MenuItem::separator(),
            MenuItem::action(text("Bring All to Front"), BringAllToFront),
        ]),
        Menu::new("Help").items([
            MenuItem::action(text("Nauterm Documentation"), Documentation),
            MenuItem::action(text("Report an Issue…"), ReportIssue),
        ]),
    ]
}

#[cfg(target_os = "macos")]
fn rebuild_menus(cx: &mut App) {
    cx.set_menus(menus(|source| menu_title(source).to_string().into()));
    crate::window_chrome::register_app_menus();
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
        let english = menus(SharedString::from);
        let translated = menus(|source| match source {
            "File" => "文件".into(),
            "Copy" => "复制".into(),
            source => source.into(),
        });
        assert_eq!(english.len(), translated.len());
        for name in ["Window", "Help"] {
            assert!(english.iter().any(|menu| menu.name == name));
            assert!(translated.iter().any(|menu| menu.name == name));
        }
        assert!(translated.iter().any(|menu| menu.name == "文件"));
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
        assert_eq!(actions(&english), actions(&translated));
        assert!(translated.iter().flat_map(|menu| &menu.items).any(|item| matches!(item, MenuItem::Action { name, action, .. } if name == "复制" && action.as_any().is::<Copy>())));
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
