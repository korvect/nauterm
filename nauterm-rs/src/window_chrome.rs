//! Use live AppKit button metrics, as Electron's WindowButtonsProxy does.
//! GPUI remains responsible for relayout, tracking areas and fullscreen; no
//! timers, retained NSButton pointers or duplicated native window ownership.
#[cfg(target_os = "macos")]
pub fn align_traffic_lights(window: &gpui_kit::Window) {
    use objc2_app_kit::NSWindowButton;
    let Some(native) = native_window(window) else {
        return;
    };
    let Some(button) = native.standardWindowButton(NSWindowButton::CloseButton) else {
        return;
    };
    let height = button.frame().size.height as f32;
    if height.is_finite() && (1. ..44.).contains(&height) {
        window.set_traffic_light_position(gpui_kit::point(
            gpui_kit::px(8.),
            gpui_kit::px((44. - height) / 2.),
        ));
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn native_window(
    window: &gpui_kit::Window,
) -> Option<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    // SAFETY: GPUI supplies a live NSView for this window. This function runs
    // synchronously on GPUI/AppKit's main thread and never retains the pointer.
    let view = unsafe { &*(handle.ns_view.as_ptr() as *const NSView) };
    view.window()
}

pub fn register_window(window: &gpui_kit::Window) {
    #[cfg(target_os = "macos")]
    if let Some(native) = native_window(window)
        && let Some(main) = objc2::MainThreadMarker::new()
    {
        native.setExcludedFromWindowsMenu(false);
        let app = objc2_app_kit::NSApplication::sharedApplication(main);
        app.addWindowsItem_title_filename(&native, &native.title(), false);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub fn refresh_window_registration(window: &gpui_kit::Window) {
    #[cfg(target_os = "macos")]
    if native_window(window).is_some_and(|native| native.isExcludedFromWindowsMenu()) {
        return;
    }
    register_window(window);
}

#[cfg(target_os = "macos")]
pub fn register_app_menus(cx: &gpui_kit::App) {
    let Some(main) = objc2::MainThreadMarker::new() else {
        return;
    };
    let app = objc2_app_kit::NSApplication::sharedApplication(main);
    let Some(menu) = app.mainMenu() else {
        return;
    };
    for (source, key) in [("Window", "rs.appMenu.window"), ("Help", "rs.appMenu.help")] {
        // GPUI registers the Window role by this canonical internal name.
        // Translate its native display title only after that registration.
        if let Some(item) = menu.itemWithTitle(&objc2_foundation::NSString::from_str(source))
            && let Some(submenu) = item.submenu()
        {
            if source == "Help" {
                app.setHelpMenu(Some(&submenu));
            }
            let title = objc2_foundation::NSString::from_str(&crate::localization::text(cx, key));
            item.setTitle(&title);
            submenu.setTitle(&title);
        }
    }
    // AppKit owns this submenu. Avoid the pinned gpui-pre-macos 0.3.10 helper,
    // which passes an NSMenuItem rather than NSMenu to setServicesMenu:.
    if let Some(item) = menu
        .itemWithTitle(&objc2_foundation::NSString::from_str("Nauterm RS"))
        .and_then(|item| item.submenu())
        .and_then(|menu| {
            menu.itemWithTitle(&objc2_foundation::NSString::from_str(
                &crate::localization::text(cx, "rs.appMenu.services"),
            ))
        })
    {
        let services = objc2_app_kit::NSMenu::new(main);
        services.setTitle(&item.title());
        item.setSubmenu(Some(&services));
        app.setServicesMenu(Some(&services));
    }
}

#[cfg(target_os = "macos")]
pub fn bring_all_to_front() {
    if let Some(main) = objc2::MainThreadMarker::new() {
        objc2_app_kit::NSApplication::sharedApplication(main).arrangeInFront(None);
    }
}
#[cfg(not(target_os = "macos"))]
pub fn align_traffic_lights(_: &gpui_kit::Window) {}
