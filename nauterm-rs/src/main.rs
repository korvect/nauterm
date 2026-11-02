#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
mod about;
mod ai;
mod app_menu;
mod assets;
mod avatar;
mod cancel;
mod components;
mod composer_completion;
mod connection;
mod connection_state;
mod directory_follow;
mod editor_config;
mod export_request;
mod external_application;
mod fido2;
mod file_commit;
mod file_conflict;
mod file_dialog;
mod file_download;
mod file_drop;
mod file_model;
mod file_operation;
mod fonts;
mod general_preferences;
mod host_import;
mod host_password;
mod host_picker;
mod host_platform;
mod host_session;
mod host_transfer;
mod key_export;
mod keychain;
mod keygen;
mod known_hosts;
mod layout;
mod lifecycle;
mod localization;
mod management;
mod model;
mod mosh;
mod native;
mod palette;
mod pane_transfer;
mod pane_tree;
mod path_completion;
mod path_reader;
mod port_forward;
mod proxies;
mod quick_connect;
mod recordings;
mod recovery;
mod remote_edit;
mod resource_reference;
mod scroll_trace;
mod serial;
mod settings;
mod sftp;
mod sftp_client;
mod sftp_preferences;
mod sftp_tasks;
mod shell_history;
mod shells;
mod shortcuts;
mod snippet_store;
mod snippets;
#[cfg(target_os = "macos")]
mod sparkle;
mod ssh_address;
mod store;
mod sync;
mod system_info;
mod tags;
mod telnet;
mod terminal;
mod terminal_appearance;
mod terminal_bell;
mod terminal_graphics;
mod terminal_padding;
mod terminal_prediction;
mod terminal_preferences;
mod terminal_search;
mod theme_catalog;
mod theme_preview;
mod theme_resource_parser;
mod transfer_history;
mod transfer_queue;
mod typography;
mod ui;
mod vault;
mod window_chrome;
mod working_directory;
mod workspace;

use gpui_kit::{AppContext, KeyBinding, TitlebarOptions, WindowOptions, point, px, size};

fn main() -> anyhow::Result<()> {
    let store = std::sync::Arc::new(store::Store::discover()?);
    let loaded = theme_catalog::load(&store.paths);
    for warning in &loaded.warnings {
        eprintln!("Terminal theme: {warning}");
    }
    theme_catalog::publish(loaded);
    let (recovery, recovery_error) = match recovery::Journal::begin(&store.state_directory) {
        Ok(recovery) => (Some(recovery), None),
        Err(error)
            if matches!(
                error.downcast_ref::<std::fs::TryLockError>(),
                Some(std::fs::TryLockError::WouldBlock)
            ) =>
        {
            return Err(error);
        }
        Err(error) => (
            None,
            Some(format!(
                "Workspace recovery is unavailable; the original file was preserved: {error}"
            )),
        ),
    };
    let quit_journal = recovery.as_ref().map(|recovery| recovery.journal.clone());
    let reopen_journal = quit_journal.clone();
    let application = gpui_kit::application().with_assets(assets::Assets);
    application.on_reopen(app_menu::reopen);
    application.run(move |cx| {
        gpui_kit::init(cx);
        cx.set_app_identity("com.korvect.nauterm.rs", "Nauterm RS");
        if let Err(error) = fonts::install(cx) {
            eprintln!("Could not load bundled terminal symbols: {error}");
        }
        components::overlay::init(cx);
        cx.on_app_quit(move |_| {
            let journal = quit_journal.clone();
            async move {
                lifecycle::prepare_shutdown().await;
                if let Some(journal) = journal
                    && let Err(error) = journal.finish()
                {
                    eprintln!("Could not finalize workspace recovery: {error}");
                }
            }
        })
        .detach();
        cx.bind_keys([
            KeyBinding::new("escape", settings::Dismiss, Some("Settings")),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-w"
                } else {
                    "ctrl-w"
                },
                settings::Dismiss,
                Some("Settings"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-n"
                } else {
                    "ctrl-n"
                },
                ui::NewItem,
                Some("Nauterm"),
            ),
            KeyBinding::new("escape", ui::CloseEditor, Some("Nauterm")),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-a"
                } else {
                    "ctrl-a"
                },
                ui::SelectCards,
                Some("Nauterm"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-backspace"
                } else {
                    "delete"
                },
                ui::DeleteCards,
                Some("Nauterm"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-q"
                } else {
                    "ctrl-q"
                },
                ui::Quit,
                None,
            ),
        ]);
        app_menu::init(cx);
        app_menu::set_window_factory(cx, {
            let store = store.clone();
            let recovery_error = recovery_error.clone();
            move |cx| {
                let recovery = reopen_journal
                    .as_ref()
                    .map(|journal| journal.resume_window())
                    .transpose()?;
                open_main_window(store.clone(), recovery, recovery_error.clone(), cx)
            }
        });
        cx.on_window_closed(|cx, _| {
            if !cfg!(target_os = "macos") && cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let result = open_main_window(store, recovery, recovery_error, cx);
        if let Err(error) = result {
            eprintln!("Cannot open Nauterm RS: {error:#}");
            cx.quit();
        }
        cx.activate(true);
        #[cfg(target_os = "macos")]
        cx.foreground_executor()
            .spawn(async {
                if sparkle::configuration().is_ok()
                    && let Err(error) = sparkle::init()
                {
                    eprintln!("Cannot initialize Sparkle: {error:#}");
                }
            })
            .detach();
    });
    Ok(())
}

fn open_main_window(
    store: std::sync::Arc<store::Store>,
    recovery: Option<recovery::BeginRun>,
    recovery_error: Option<String>,
    cx: &mut gpui_kit::App,
) -> anyhow::Result<()> {
    let saved_window = recovery
        .as_ref()
        .and_then(|recovery| recovery.previous.as_ref())
        .and_then(|previous| previous.snapshot.window);
    let default_window_size = store.initial_window_size().unwrap_or_else(|error| {
        eprintln!("Cannot read default window size: {error:#}");
        Default::default()
    });
    let bounds = ui::initial_window_bounds(saved_window, default_window_size, cx);
    gpui_kit::open_window(
        WindowOptions {
            app_id: Some("com.korvect.nauterm.rs".into()),
            window_bounds: Some(bounds),
            window_min_size: Some(size(px(750.), px(440.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Nauterm RS".into()),
                appears_transparent: true,
                // GPUI expects the button's inset, not its center.
                // Native buttons are 14pt high: 15pt centers them in
                // our 44pt chrome. GPUI reapplies this after AppKit's
                // titlebar/fullscreen layout (as Electron's proxy does).
                traffic_light_position: Some(point(px(8.), px(15.))),
            }),
            app_owns_titlebar_drag: true,
            ..Default::default()
        },
        cx,
        move |window, cx| {
            window_chrome::align_traffic_lights(window);
            let view = cx.new(|cx| ui::Nauterm::new(store, recovery, recovery_error, window, cx));
            app_menu::register_main_window(window, &view, cx);
            view
        },
    )
    .map(|_| ())
}
