//! Process-wide transport runtime shared by the Rust and C adapters.
use std::sync::{Mutex, OnceLock};

use crate::session::SessionManager;

pub fn sessions() -> &'static Mutex<SessionManager> {
    static MANAGER: OnceLock<Mutex<SessionManager>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(SessionManager::default()))
}

pub fn initialize() {
    crate::pty::enable_wakeup_callbacks();
}

pub fn prepare_shutdown() {
    crate::pty::disable_wakeup_callbacks();
    #[cfg(feature = "ffi")]
    crate::ffi::capture::shutdown_captures();
    if let Ok(mut manager) = sessions().lock() {
        manager.prepare_runtime_shutdown();
    }
    crate::ssh::cancel_all_sftp_tasks();
    if let Ok(mut manager) = crate::port_forward::port_forward_manager().lock() {
        manager.stop_all();
    }
}
