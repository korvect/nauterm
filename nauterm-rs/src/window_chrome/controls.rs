//! Product titlebar controls, with platform hit testing for Windows Snap Layouts.
use gpui_kit::{assets::IconName, component::Icon, prelude::*, *};

pub fn has_controls(window: &Window) -> bool {
    if cfg!(target_os = "linux") {
        // A compositor may grant server decorations despite our request. In
        // that case it already supplies the buttons; never draw a second set.
        matches!(window.window_decorations(), Decorations::Client { .. })
    } else {
        cfg!(target_os = "windows")
    }
}

pub fn window_controls(
    foreground: Rgba,
    window: &Window,
    cx: &App,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let supported = window.window_controls();
    let maximized = window.is_maximized();
    div()
        .id("window-controls")
        .flex()
        .h_full()
        .flex_shrink_0()
        .when(has_controls(window), |controls| {
            controls
                .pr(px(6.))
                .when(supported.minimize, |controls| {
                    controls.child(control(
                        "window-minimize",
                        IconName::WindowMinimize,
                        WindowControlArea::Min,
                        foreground,
                        crate::localization::text(cx, "rs.window.minimize"),
                        |_, window, _| window.minimize_window(),
                    ))
                })
                .when(supported.maximize, |controls| {
                    controls.child(control(
                        "window-maximize",
                        if maximized {
                            IconName::WindowRestore
                        } else {
                            IconName::WindowMaximize
                        },
                        WindowControlArea::Max,
                        foreground,
                        crate::localization::text(
                            cx,
                            if maximized {
                                "rs.window.restore"
                            } else {
                                "rs.window.maximize"
                            },
                        ),
                        |_, window, _| window.zoom_window(),
                    ))
                })
                .child(control(
                    "window-close",
                    IconName::WindowClose,
                    WindowControlArea::Close,
                    foreground,
                    crate::localization::text(cx, "common.action.close"),
                    on_close,
                ))
        })
}

fn control(
    id: &'static str,
    icon: IconName,
    area: WindowControlArea,
    foreground: Rgba,
    label: SharedString,
    action: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let hover = if area == WindowControlArea::Close {
        rgb(0xe05545)
    } else {
        foreground.alpha(0.10)
    };
    let glyph = match icon {
        IconName::WindowMinimize => Icon::new(IconName::Minus).size(px(15.)),
        IconName::WindowMaximize => Icon::new(IconName::Square).size(px(13.)),
        IconName::WindowRestore => Icon::default()
            .path("nauterm/window-restore.svg")
            .size(px(15.)),
        _ => Icon::new(IconName::X).size(px(15.)),
    };
    div()
        .id(id)
        .group(id)
        .role(Role::Button)
        .aria_label(label)
        .w(px(34.))
        .h_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .cursor(CursorStyle::PointingHand)
        .text_color(foreground)
        .when(cfg!(target_os = "windows"), |button| {
            button.window_control_area(area)
        })
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            // Consume the press before the surrounding titlebar starts a move.
            // Windows also forwards non-client presses through GPUI, so handle
            // both platforms here instead of performing the native action twice.
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            action(event, window, cx);
        })
        .child(
            div()
                .size(px(26.))
                .rounded(px(7.))
                .flex()
                .items_center()
                .justify_center()
                .group_hover(id, move |style| style.bg(hover))
                .child(glyph),
        )
}
