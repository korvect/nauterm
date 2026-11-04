//! Serializable handler identity, independent of COM and the host platform.
//! Persist the association used for discovery so a configured editor can still
//! open another text extension without guessing or evaluating a command line.
use anyhow::{Result, ensure};

const ASSOCIATION: &str = "win32-assoc:";
const EXECUTABLE: &str = "win32-executable:";

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Association {
    pub extension: String,
    pub handler: String,
}
impl Association {
    pub fn new(extension: String, handler: String) -> Result<Self> {
        ensure!(
            extension.starts_with('.')
                && extension.len() <= 512
                && extension.len() > 1
                && !extension.chars().any(char::is_control)
                && !extension.contains(['/', '\\', ':']),
            "Invalid file association extension."
        );
        ensure!(
            !handler.trim().is_empty()
                && handler.len() <= 16 * 1024
                && !handler.chars().any(char::is_control),
            "Invalid application handler."
        );
        Ok(Self { extension, handler })
    }
    pub fn identifier(&self) -> String {
        format!(
            "{ASSOCIATION}{}",
            serde_json::to_string(&(&self.extension, &self.handler)).expect("strings serialize")
        )
    }
    pub fn parse(identifier: &str) -> Result<Self> {
        let raw = identifier.strip_prefix(ASSOCIATION).ok_or_else(|| {
            anyhow::anyhow!(
                "This saved application belongs to another platform. Choose a Windows application."
            )
        })?;
        ensure!(
            raw.len() <= 64 * 1024,
            "Invalid application handler identity."
        );
        let (extension, handler) = serde_json::from_str(raw)?;
        Self::new(extension, handler)
    }

    /// Packaged Windows applications can expose a Shell handler name such as
    /// `Photos` instead of an executable path. Retain that opaque locator only
    /// when it is bound to the serialized association, never as a command.
    pub fn matches_path(&self, path: &std::path::Path) -> bool {
        path.to_str()
            .is_some_and(|path| path.eq_ignore_ascii_case(&self.handler))
    }
}
pub(super) fn executable_identifier(path: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let mut identifier = EXECUTABLE.to_owned();
    for byte in Sha256::digest(path.as_bytes()) {
        write!(&mut identifier, "{byte:02x}").expect("writing to a String cannot fail");
    }
    identifier
}
pub(super) fn is_executable(identifier: &str, path: &str) -> bool {
    identifier == executable_identifier(path)
}

/// Recover straight RGBA from native icons rendered onto black/white mattes.
/// This handles legacy mask-only icons as well as premultiplied alpha icons.
pub(super) fn rgba_from_mattes(black: &[u8], white: &[u8]) -> Option<Vec<u8>> {
    if black.len() != white.len() || !black.len().is_multiple_of(4) {
        return None;
    }
    let mut rgba = Vec::with_capacity(black.len());
    for (black, white) in black
        .as_chunks::<4>()
        .0
        .iter()
        .zip(white.as_chunks::<4>().0.iter())
    {
        let transparency = (0..3)
            .map(|channel| white[channel].saturating_sub(black[channel]))
            .max()?;
        let alpha = 255 - transparency;
        for channel in [2, 1, 0] {
            rgba.push(if alpha == 0 {
                0
            } else {
                ((u16::from(black[channel]) * 255 / u16::from(alpha)).min(255)) as u8
            });
        }
        rgba.push(alpha);
    }
    Some(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handler_identity_roundtrips_unicode_and_special_characters_without_command_parsing() {
        let handler = r#"C:\Program Files\Editor 用户 & Co\editor.exe"#;
        let value = Association::new(".txt".into(), handler.into()).unwrap();
        assert_eq!(Association::parse(&value.identifier()).unwrap(), value);
        assert!(Association::parse("com.example.macos-editor").is_err());
        assert!(Association::parse("win32-assoc:[\".txt\",\"x\"] trailing").is_err());
        assert!(Association::new(".txt:stream".into(), handler.into()).is_err());
        assert!(Association::new(".txt".into(), "bad\0handler".into()).is_err());
        assert!(Association::new(".txt".into(), "".into()).is_err());
        let packaged = Association::new(".png".into(), "Photos".into()).unwrap();
        assert!(packaged.matches_path(std::path::Path::new("Photos")));
        assert!(!packaged.matches_path(std::path::Path::new("Other App")));
        let identifier = executable_identifier(handler);
        assert!(is_executable(&identifier, handler));
        assert!(!is_executable(&identifier, r"C:\another\editor.exe"));
    }
    #[test]
    fn native_icon_mattes_preserve_transparency_and_channel_order() {
        let black = [0, 0, 0, 0, 3, 2, 1, 0, 16, 32, 64, 0];
        let white = [255, 255, 255, 0, 3, 2, 1, 0, 143, 159, 191, 0];
        assert_eq!(
            rgba_from_mattes(&black, &white).unwrap(),
            [0, 0, 0, 0, 1, 2, 3, 255, 127, 63, 31, 128]
        );
        assert!(rgba_from_mattes(&black, &white[..4]).is_none());
    }
}
