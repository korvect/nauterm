//! Shared decision type, independent of UI and network execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Cancel,
    KeepRemote,
    SaveCopy,
    OverwriteRemote,
}
