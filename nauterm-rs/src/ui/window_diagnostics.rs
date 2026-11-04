//! Opt-in window sizing benchmark. Requires an isolated temporary data directory.
use super::*;

struct ReferenceWindow;
impl Render for ReferenceWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0xf4f6f8))
            .child(crate::localization::text(cx, "Settings"))
    }
}

impl Nauterm {
    pub(crate) fn start_resize_diagnostics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // An explicit empty temporary directory is required: this diagnostic
        // opens real views and exits the process after collecting its samples.
        let Some(directory) = std::env::var_os("NAUTERM_RS_DATA_DIR") else {
            eprintln!("resize diagnostics require NAUTERM_RS_DATA_DIR");
            return;
        };
        let directory = std::path::PathBuf::from(directory);
        if !directory.starts_with(std::env::temp_dir()) {
            eprintln!(
                "resize diagnostics require a data directory under the system temporary directory"
            );
            return;
        }
        self.open_settings(window, cx);
        if std::env::var("NAUTERM_RESIZE_BENCH_MODE").as_deref() == Ok("manual") {
            // Keep both comparison windows available for manual interaction.
            // Minimizing only this isolated instance's main window avoids
            // covering the Settings view being compared.
            window.minimize_window();
            return;
        }
        let Some(settings) = self
            .settings_window
            .as_ref()
            .map(|settings| settings.handle)
        else {
            return;
        };
        gpui_kit::profiler::set_trace_enabled(true);
        let mut collector = cx.foreground_journal().collector();
        cx.spawn(async move |_, cx| {
            use std::time::Duration;
            cx.background_executor().timer(Duration::from_secs(2)).await;
            for (label, handle) in [("settings", Some(settings)), ("reference", None)] {
                let handle = if let Some(handle) = handle {
                    handle
                } else {
                    match cx.update(|cx| {
                        gpui_kit::open_window(
                            WindowOptions {
                                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                                    None, size(px(950.), px(620.)), cx,
                                ))),
                                titlebar: Some(TitlebarOptions {
                                    title: Some("Nauterm RS resize reference".into()),
                                    appears_transparent: false,
                                    ..Default::default()
                                }),
                                window_decorations: Some(WindowDecorations::Server),
                                ..Default::default()
                            },
                            cx,
                            |_, cx| cx.new(|_| ReferenceWindow),
                        )
                    }) {
                        Ok((handle, _)) => handle,
                        _ => break,
                    }
                };
                cx.background_executor().timer(Duration::from_secs(1)).await;
                collector.collect_unseen();
                eprintln!("[resize-bench] begin={label}");
                for index in 0..60 {
                    let delta = if index < 30 { index } else { 59 - index } as f32;
                    let _ = handle.update(cx, |_, window, _| {
                        window.resize(size(px(950. + delta * 10.), px(620. + delta * 4.)));
                    });
                    cx.background_executor().timer(Duration::from_millis(32)).await;
                }
                cx.background_executor().timer(Duration::from_millis(250)).await;
                use gpui_kit::profiler::journal::{ForegroundEvent, ForegroundJournalEntry};
                for entry in collector.collect_unseen().entries {
                    match entry {
                        ForegroundJournalEntry::Event(ForegroundEvent::Draw(timing)) => {
                            eprintln!("[resize-profile] target={label} phase=draw layout_us={} prepaint_us={} paint_us={}", timing.phases.request_layout.as_micros(), timing.phases.prepaint.as_micros(), timing.phases.paint.as_micros());
                        }
                        ForegroundJournalEntry::Event(ForegroundEvent::View(timing)) => {
                            eprintln!("[resize-profile] target={label} phase={} view={} self_us={}", timing.phase.as_str(), timing.name, timing.self_duration.as_micros());
                        }
                        _ => {}
                    }
                }
                eprintln!("[resize-bench] end={label}");
            }
            cx.update(|cx| cx.quit());
        }).detach();
    }
}
