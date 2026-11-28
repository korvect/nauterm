//! Protocol and embedded, target-matched console hook for Nauterm-owned CMD
//! sessions. No user profile, registry AutoRun or other console is modified.
mod prompt;
mod scanner;
mod session;

pub use prompt::{PromptCapture, ReadBoundary};
pub use scanner::{MarkerScanner, ScanChunk};
pub use session::{ProtocolError, Session, Token};

#[cfg(windows)]
pub fn stage(directory: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    std::fs::write(
        directory.join("nauterm-cmd-hook.dll"),
        include_bytes!(concat!(env!("OUT_DIR"), "/nauterm-cmd-hook.dll")),
    )?;
    let launcher = directory.join("nauterm-cmd.exe");
    std::fs::write(
        &launcher,
        include_bytes!(concat!(env!("OUT_DIR"), "/nauterm-cmd.exe")),
    )?;
    Ok(launcher)
}
