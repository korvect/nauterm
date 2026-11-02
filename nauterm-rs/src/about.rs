//! Application metadata is embedded; entering About performs no network IO.
pub mod download;
pub mod licenses;
pub mod releases;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD: &str = env!("NAUTERM_RS_BUILD_NUMBER");
pub const PACKAGE: &str = "com.korvect.nauterm.rs";
pub const SOURCE_LICENSE: &str = include_str!("../../LICENSE");
pub fn platform() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    };
    format!("{os} · {}", std::env::consts::ARCH)
}
