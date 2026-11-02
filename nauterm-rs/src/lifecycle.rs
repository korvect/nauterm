//! Normal UI exits await this latch before GPUI's quit/restart path.
//! The short final quit hook is a fallback for platform-initiated exits.
use std::sync::OnceLock;
struct Shutdown {
    done: async_channel::Receiver<()>,
}
pub async fn prepare_shutdown() {
    static SHUTDOWN: OnceLock<Shutdown> = OnceLock::new();
    let shutdown = SHUTDOWN.get_or_init(|| {
        let (sender, done) = async_channel::bounded::<()>(1);
        let complete = sender.clone();
        let spawned = std::thread::Builder::new()
            .name("nauterm-rs-shutdown".into())
            .spawn(move || {
                drain();
                complete.close();
            });
        if spawned.is_err() {
            drain();
            sender.close();
        }
        Shutdown { done }
    });
    let _ = shutdown.done.recv().await;
}
fn drain() {
    stage(
        "port forwarding",
        crate::port_forward::Service::prepare_shutdown,
    );
    stage(
        "transfer cancellation",
        crate::transfer_queue::begin_shutdown,
    );
    stage("update downloads", crate::about::download::prepare_shutdown);
    stage("external edits", crate::remote_edit::prepare_shutdown);
    stage("transfers", crate::transfer_queue::finish_shutdown);
    stage("SFTP tasks", crate::sftp_tasks::prepare_shutdown);
    stage("recordings", crate::recordings::Service::prepare_shutdown);
    stage("AI history", || {
        smol::block_on(crate::ai::history::prepare_shutdown())
    });
    stage("database workers", crate::vault::Vault::prepare_shutdown);
    stage("native sessions", crate::native::Core::prepare_shutdown);
}
/// The last macOS main window has released its UI-owned sessions. Drain its
/// accepted work, but keep process-wide services and PTY callbacks reusable.
/// The window factory serializes this with reopen and Quit.
#[cfg(target_os = "macos")]
pub fn prepare_window_close() {
    stage(
        "window port forwarding",
        crate::port_forward::Service::prepare_shutdown,
    );
    stage(
        "window transfers",
        crate::transfer_queue::begin_window_close,
    );
    stage("window external edits", crate::remote_edit::close_window);
    stage(
        "window transfer completion",
        crate::transfer_queue::finish_shutdown,
    );
    stage("window SFTP requests", crate::sftp_tasks::close_window);
    stage(
        "window recordings",
        crate::recordings::Service::prepare_shutdown,
    );
    stage("window AI history", || {
        smol::block_on(crate::ai::history::close_window())
    });
}
fn stage(name: &str, work: impl FnOnce() + std::panic::UnwindSafe) {
    if std::panic::catch_unwind(work).is_err() {
        eprintln!("Could not finish shutting down {name}.");
    }
}
