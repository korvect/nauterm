//! Platform-independent protocol for the Nauterm-owned CMD console hook.
//!
//! This library is not yet a working hook and is not used to advertise CMD
//! integration in the client. The Windows host, interception layer and ConPTY
//! runtime validation must be completed before enabling that capability.
mod prompt;
mod scanner;
mod session;

pub use prompt::{PromptCapture, ReadBoundary};
pub use scanner::{MarkerScanner, ScanChunk};
pub use session::{ProtocolError, Session, Token};
