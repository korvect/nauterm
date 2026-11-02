pub mod application;
pub mod capture_crypto;
mod cloud_sync;
mod crypto;
mod database;
#[cfg(feature = "ffi")]
mod ffi;
pub mod fido2;
#[cfg(feature = "terminal-ghostty")]
mod ghostty_terminal;
mod github_gist_sync;
mod github_sync;
mod mosh;
mod object_sync;
mod output_queue;
pub mod pty;
mod s3_sync;
pub mod serial;
pub mod session;
pub mod shell_integration;
mod sync;
mod telnet;
pub mod terminal;
mod theme;

pub mod port_forward;
pub mod ssh;

pub use database::*;
#[cfg(feature = "ffi")]
pub use ffi::*;
pub use sync::SyncStrategy;

pub mod runtime;
pub mod subscription;

#[cfg(all(test, feature = "terminal-ghostty"))]
#[path = "../colors/tests.rs"]
mod palette_tests;

pub use mosh::MoshServerCommand;

/// True only for isolated core unit-test builds; persistent clients must refuse it.
pub const TEST_CREDENTIAL_STORE: bool = cfg!(feature = "test-credential-store");

pub mod history_file;
pub mod paths;
pub mod recording_lease;
