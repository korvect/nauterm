//! Platform window chrome: native macOS traffic lights and product-owned
//! Linux/Windows controls. GPUI owns native window state and hit testing.
mod controls;
pub use controls::{has_controls, window_controls};

/// Deferred dialogs paint outside the normal content clip. Exclude the native
/// surface's transparent shadow margin from their backdrop and layout bounds.
pub fn content_bounds(window: &gpui_kit::Window) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    use gpui_kit::*;
    let mut insets = Edges::all(px(0.));
    if cfg!(target_os = "linux")
        && let Decorations::Client { tiling } = window.window_decorations()
    {
        insets = gpui_kit::component::window_paddings(window);
        // The pinned WindowBorder adds a 1px frame on each non-tiled edge.
        if !tiling.top {
            insets.top += px(1.);
        }
        if !tiling.right {
            insets.right += px(1.);
        }
        if !tiling.bottom {
            insets.bottom += px(1.);
        }
        if !tiling.left {
            insets.left += px(1.);
        }
    }
    inset_content_bounds(window.viewport_size(), insets)
}

fn inset_content_bounds(
    viewport: gpui_kit::Size<gpui_kit::Pixels>,
    insets: gpui_kit::Edges<gpui_kit::Pixels>,
) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    use gpui_kit::*;
    Bounds::new(
        point(insets.left, insets.top),
        size(
            (viewport.width - insets.left - insets.right).max(px(0.)),
            (viewport.height - insets.top - insets.bottom).max(px(0.)),
        ),
    )
}

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

#[cfg(target_os = "macos")]
pub fn refresh_window_registration(window: &gpui_kit::Window) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{Bounds, Edges, point, px, size};
    #[test]
    fn modal_content_excludes_shadow_on_each_untiled_edge() {
        let viewport = size(px(950.), px(620.));
        assert_eq!(
            inset_content_bounds(viewport, Edges::all(px(0.))),
            Bounds::new(point(px(0.), px(0.)), viewport)
        );
        let floating = inset_content_bounds(viewport, Edges::all(px(21.)));
        assert_eq!(floating.origin, point(px(21.), px(21.)));
        assert_eq!(floating.size, size(px(908.), px(578.)));
        let tiled = inset_content_bounds(
            viewport,
            Edges {
                top: px(0.),
                left: px(0.),
                right: px(21.),
                bottom: px(21.),
            },
        );
        assert_eq!(tiled.origin, point(px(0.), px(0.)));
        assert_eq!(tiled.size, size(px(929.), px(599.)));
        assert_eq!(
            inset_content_bounds(size(px(10.), px(10.)), Edges::all(px(21.))).size,
            size(px(0.), px(0.))
        );
    }
}
