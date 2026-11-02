//! The reference attachment payload, without local paths or an additional store.
use super::context;
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs::File, io::Read, path::Path};

pub const MAX_COUNT: usize = 5;
pub const MAX_IMAGE: usize = 5 * 1024 * 1024;
pub const MAX_TEXT: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Image,
    Text,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
    pub kind: Kind,
    pub text: Option<String>,
    pub bytes: Option<String>,
    #[serde(default)]
    pub redacted: bool,
}
#[derive(Clone)]
pub struct Summary {
    pub name: String,
    pub image: bool,
    pub redacted: bool,
}
impl Summary {
    /// Rendering must not copy base64 payloads on every streamed text delta.
    pub fn from_value(value: &Value) -> Self {
        Self {
            name: value["name"].as_str().unwrap_or("Attachment").into(),
            image: value["kind"].as_str() == Some("image"),
            redacted: value["redacted"].as_bool().unwrap_or(false),
        }
    }
}
impl Attachment {
    pub fn read(path: &Path) -> Result<Self> {
        let name = path
            .file_name()
            .context("Choose a file.")?
            .to_string_lossy();
        ensure!(
            std::fs::metadata(path)?.is_file(),
            "Choose a regular image or text file."
        );
        let mut file = File::open(path).with_context(|| format!("Could not open {name}."))?;
        let metadata = file.metadata()?;
        ensure!(metadata.is_file(), "Choose a regular image or text file.");
        ensure!(
            metadata.len() <= MAX_IMAGE as u64,
            "{name} exceeds the 5 MB image limit."
        );
        let mut header = [0_u8; 16];
        let length = (metadata.len() as usize).min(header.len());
        file.read_exact(&mut header[..length])?;
        let image = image_mime(&header[..length]);
        let text_type = text_mime(&name);
        let limit = if image.is_some() { MAX_IMAGE } else { MAX_TEXT };
        ensure!(
            metadata.len() <= limit as u64,
            "{name} exceeds the 512 KB text limit."
        );
        ensure!(
            image.is_some() || text_type.is_some(),
            "{name} is not a supported image or text file."
        );
        let mut data = header[..length].to_vec();
        file.take((limit - length + 1) as u64)
            .read_to_end(&mut data)?;
        ensure!(
            data.len() <= limit,
            "{name} grew beyond its attachment size limit."
        );
        let name: String = name
            .chars()
            .filter(|ch| !ch.is_control())
            .take(256)
            .collect();
        let (kind, mime_type, text, bytes, redacted) = if let Some(mime) = image {
            (Kind::Image, mime, None, Some(STANDARD.encode(&data)), false)
        } else {
            let (text, redacted) = context::sanitize_file(&String::from_utf8_lossy(&data));
            (
                Kind::Text,
                text_type.unwrap_or("text/plain"),
                Some(text),
                None,
                redacted,
            )
        };
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            mime_type: mime_type.into(),
            size: data.len() as u64,
            kind,
            text,
            bytes,
            redacted,
        })
    }
    pub fn value(&self) -> Value {
        serde_json::to_value(self).expect("serializable attachment")
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.is_empty() && self.name.len() <= 4096,
            "An attachment has an invalid name."
        );
        match self.kind {
            Kind::Text => {
                ensure!(
                    self.size <= MAX_TEXT as u64,
                    "A text attachment exceeds 512 KB."
                );
                let text = self
                    .text
                    .as_deref()
                    .context("A text attachment is missing its content.")?;
                ensure!(text.len() <= MAX_TEXT, "A text attachment exceeds 512 KB.");
            }
            Kind::Image => {
                ensure!(
                    self.size <= MAX_IMAGE as u64,
                    "An image attachment exceeds 5 MB."
                );
                let bytes = self
                    .bytes
                    .as_deref()
                    .context("An image attachment is missing its content.")?;
                ensure!(
                    bytes.len() <= MAX_IMAGE.div_ceil(3) * 4,
                    "An image attachment exceeds 5 MB."
                );
                let decoded = STANDARD
                    .decode(bytes)
                    .context("An image attachment has invalid base64 data.")?;
                ensure!(
                    decoded.len() <= MAX_IMAGE
                        && image_mime(&decoded) == Some(self.mime_type.as_str()),
                    "An image attachment has invalid or unsupported image data."
                );
            }
        }
        Ok(())
    }
    pub fn text_content(&self) -> String {
        let (text, _) = context::sanitize_file(self.text.as_deref().unwrap_or_default());
        json!({"type":"text_attachment","name":self.name,"mime_type":self.mime_type,"content":text})
            .to_string()
    }
}
pub fn parse(values: &[Value]) -> Result<Vec<Attachment>> {
    ensure!(
        values.len() <= MAX_COUNT,
        "Attach at most five files per message."
    );
    values
        .iter()
        .map(|value| {
            let attachment: Attachment = serde_json::from_value(value.clone())
                .context("This history contains an unsupported attachment.")?;
            attachment.validate()?;
            Ok(attachment)
        })
        .collect()
}
fn image_mime(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}
fn text_mime(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    let extension = lower.rsplit_once('.')?.1;
    match extension {
        "json" => Some("application/json"),
        "xml" => Some("application/xml"),
        "yaml" | "yml" => Some("application/x-yaml"),
        "bash" | "c" | "cc" | "conf" | "cpp" | "css" | "csv" | "dart" | "env" | "fish" | "go"
        | "h" | "hpp" | "html" | "ini" | "java" | "js" | "kt" | "log" | "md" | "py" | "rb"
        | "rs" | "sh" | "sql" | "toml" | "ts" | "tsx" | "txt" | "zsh" => Some("text/plain"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chosen_text_is_sanitized_bounded_and_uses_the_shared_payload() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join(".env");
        let source = format!(
            "password=synthetic-secret\n\x1b[31m{}\nend",
            "文".repeat(50_010)
        );
        std::fs::write(&path, &source)?;
        let attachment = Attachment::read(&path)?;
        assert_eq!(attachment.kind, Kind::Text);
        assert!(attachment.redacted);
        let text = attachment.text.as_deref().unwrap();
        assert!(!text.contains("synthetic-secret"));
        assert!(!text.contains('\x1b'));
        assert!(text.starts_with("password=[REDACTED]"));
        assert!(text.ends_with("[Remaining file content omitted]"));
        let value = attachment.value();
        assert_eq!(value["mime_type"], "text/plain");
        assert_eq!(value["name"], ".env");
        assert!(value["bytes"].is_null());
        assert!(
            !value
                .to_string()
                .contains(directory.path().to_str().unwrap())
        );
        assert_eq!(parse(&[value])?, vec![attachment]);
        Ok(())
    }
    #[test]
    fn image_detection_uses_the_bytes_and_rejects_bad_history_payloads() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("screenshot.data");
        std::fs::write(&path, b"\x89PNG\r\n\x1a\nsynthetic")?;
        let mut attachment = Attachment::read(&path)?;
        assert_eq!(attachment.kind, Kind::Image);
        assert_eq!(attachment.mime_type, "image/png");
        attachment.validate()?;
        attachment.mime_type = "image/jpeg".into();
        assert!(attachment.validate().is_err());
        attachment.bytes = Some("invalid base64".into());
        assert!(attachment.validate().is_err());
        assert!(Attachment::read(directory.path()).is_err());
        Ok(())
    }
    #[test]
    fn attachment_limits_and_unknown_kinds_are_enforced_without_a_database() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let text = directory.path().join("large.txt");
        std::fs::write(&text, vec![b'x'; MAX_TEXT + 1])?;
        assert!(Attachment::read(&text).is_err());
        let unsupported = directory.path().join("data.bin");
        std::fs::write(&unsupported, "text without a supported filename")?;
        assert!(Attachment::read(&unsupported).is_err());
        assert!(parse(&vec![Value::Null; MAX_COUNT + 1]).is_err());
        assert!(parse(&[json!({"id":"test","name":"file.pdf","mime_type":"application/pdf","kind":"document","size":1})]).is_err());
        Ok(())
    }
}
