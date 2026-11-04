//! Manual, data-free reproduction for native Settings window decoration.
//! Run with `cargo run -p nauterm-rs --example native_titlebar`.
use gpui_kit::*;

struct NativeTitlebar;

impl Render for NativeTitlebar {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_8()
            .bg(rgb(0xf4f6f8))
            .text_color(rgb(0x20242a))
            .child("Native Settings titlebar: drag, resize, maximize, restore and close.")
    }
}

fn main() {
    application().run(|cx| {
        init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        open_window(
            WindowOptions {
                app_id: Some("com.korvect.nauterm.rs.titlebar-test".into()),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(700.), px(400.)),
                    cx,
                ))),
                window_decorations: Some(WindowDecorations::Server),
                titlebar: Some(TitlebarOptions {
                    title: Some("Nauterm RS — Native Settings titlebar test".into()),
                    appears_transparent: false,
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|_| NativeTitlebar),
        )
        .expect("native titlebar test window");
        cx.activate(true);
    });
}
