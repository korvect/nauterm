use super::{Draft, Protocol};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read, path::Path, time::Duration};
use zeroize::Zeroizing;

const LIMIT: u64 = 2 * 1024 * 1024;
#[derive(Clone, Deserialize)]
pub struct Preset {
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    #[serde(default)]
    pub default_models: Vec<String>,
}
impl Preset {
    pub fn draft(&self) -> Draft {
        Draft {
            name: self.name.clone(),
            protocol: self.protocol,
            base_url: self.base_url.clone(),
            model: self.default_models.first().cloned().unwrap_or_default(),
            api_key: Zeroizing::new(String::new()),
            max_tokens: None,
            temperature: None,
        }
    }
}
pub fn blank() -> Draft {
    Preset {
        name: "New Provider".into(),
        protocol: Protocol::OpenAi,
        base_url: Protocol::OpenAi.base_url().into(),
        default_models: vec![],
    }
    .draft()
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Preset>> {
    ensure!(bytes.len() as u64 <= LIMIT, "AI presets are too large.");
    #[derive(Deserialize)]
    struct Document {
        presets: BTreeMap<String, Preset>,
    }
    let document: Document =
        serde_json::from_slice(bytes).context("Invalid AI presets document.")?;
    ensure!(
        !document.presets.is_empty() && document.presets.len() <= 1024,
        "The presets catalog is empty or too large."
    );
    let mut presets: Vec<_> = document.presets.into_values().collect();
    for preset in &presets {
        preset.draft().validate()?;
    }
    presets.sort_by_key(|preset| preset.name.to_lowercase());
    Ok(presets)
}
pub fn bundled() -> Result<Vec<Preset>> {
    parse(include_bytes!("../../../assets/config/ai-presets.json"))
}
pub fn load(path: &Path) -> Result<Vec<Preset>> {
    let cached = (|| {
        ensure!(
            !std::fs::symlink_metadata(path)?.file_type().is_symlink(),
            "Invalid presets cache."
        );
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)?;
        parse(&bytes)
    })();
    cached.or_else(|_: anyhow::Error| bundled())
}
pub fn refresh(url: &str, path: &Path) -> Result<Vec<Preset>> {
    let parsed = url::Url::parse(url).context("Invalid presets URL.")?;
    ensure!(
        matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some(),
        "Invalid presets URL."
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?;
    let response = client
        .get(parsed)
        .send()
        .map_err(|_| anyhow::anyhow!("Could not download AI presets."))?;
    ensure!(
        response.status().is_success(),
        "Could not download AI presets (HTTP {}).",
        response.status().as_u16()
    );
    let mut bytes = Vec::new();
    response
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .context("Could not read AI presets.")?;
    let presets = parse(&bytes)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        ensure!(
            !std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()),
            "Refusing a symlinked presets cache."
        );
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        use std::io::Write;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        file.persist(path).context("Could not save AI presets.")?;
    }
    Ok(presets)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_presets_validate_for_all_reference_protocols_without_network_or_files() -> Result<()>
    {
        let presets = bundled()?;
        for protocol in Protocol::ALL {
            assert!(presets.iter().any(|preset| preset.protocol == protocol));
        }
        assert!(
            presets
                .iter()
                .all(|preset| preset.draft().api_key.is_empty())
        );
        blank().validate()?;
        Ok(())
    }
    #[test]
    fn malformed_empty_and_oversized_preset_responses_do_not_replace_a_catalog() {
        for data in [
            b"<html>failure</html>".as_slice(),
            b"{}",
            b"{\"presets\":{}}",
        ] {
            assert!(parse(data).is_err());
        }
        assert!(parse(&vec![b' '; LIMIT as usize + 1]).is_err());
    }
}
