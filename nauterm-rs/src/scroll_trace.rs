//! Opt-in input diagnostics. No terminal contents or persistent data are logged.
use gpui_kit::ScrollEvent;
use std::{fmt, sync::OnceLock, time::Instant};

pub(crate) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("NAUTERM_SCROLL_TRACE").as_deref() == Ok("1"))
}

pub(crate) fn record(scope: &str, event: &ScrollEvent, result: fmt::Arguments<'_>) {
    if !enabled() {
        return;
    }
    static START: OnceLock<Instant> = OnceLock::new();
    let elapsed = START.get_or_init(Instant::now).elapsed().as_micros();
    eprintln!(
        "[scroll +{elapsed}us] {scope} source={:?} phase={:?} delta={:?} {result}",
        event.source, event.touch_phase, event.delta,
    );
}
