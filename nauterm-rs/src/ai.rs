//! Shared AI provider records. UI lists contain summaries, never API keys.
use crate::vault::Vault;
use anyhow::{Context, Result, ensure};
use nauterm_core::{AiProviderEntry, NautermDatabase};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

static PROVIDER_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn provider_revision() -> u64 {
    PROVIDER_REVISION.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn invalidate_providers() {
    PROVIDER_REVISION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub mod attachment;
pub mod chat;
pub mod client;
pub mod context;
pub mod history;
pub mod presets;
pub mod terminal_runner;
pub mod tools;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub include_selection: bool,
    pub include_recent_output: bool,
    pub presets_url: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            include_selection: true,
            include_recent_output: true,
            presets_url:
                "https://raw.githubusercontent.com/korvect/nauterm-ai-presets/main/presets.json"
                    .into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "google", alias = "googleai", alias = "gemini")]
    Google,
    #[serde(rename = "ollama")]
    Ollama,
}
impl Protocol {
    pub const ALL: [Self; 4] = [Self::OpenAi, Self::Anthropic, Self::Google, Self::Ollama];
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "openai" => Ok(Self::OpenAi),
            "anthropic" => Ok(Self::Anthropic),
            "google" | "googleai" | "gemini" => Ok(Self::Google),
            "ollama" => Ok(Self::Ollama),
            _ => anyhow::bail!("This provider uses an unsupported API protocol."),
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Google => "google",
            Self::Ollama => "ollama",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Google => "Google Gemini",
            Self::Ollama => "Ollama",
        }
    }
    pub fn base_url(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com",
            Self::Google => "https://generativelanguage.googleapis.com",
            Self::Ollama => "http://localhost:11434",
        }
    }
}

#[derive(Clone)]
pub struct Record(AiProviderEntry);
impl Drop for Record {
    fn drop(&mut self) {
        self.0.api_key.zeroize();
    }
}
impl Record {
    pub fn id(&self) -> Option<i64> {
        self.0.id
    }
    pub fn draft(&self) -> Result<Draft> {
        Ok(Draft {
            name: self.0.name.clone(),
            protocol: Protocol::parse(&self.0.protocol)?,
            base_url: self.0.base_url.clone(),
            model: self.0.model.clone(),
            api_key: Zeroizing::new(self.0.api_key.clone()),
            max_tokens: self.0.config.get("max_tokens").and_then(Value::as_i64),
            temperature: self.0.config.get("temperature").and_then(Value::as_f64),
        })
    }
    fn stamp(&self) -> Result<Stamp> {
        let bytes = Zeroizing::new(serde_json::to_vec(&self.0)?);
        Ok(Stamp(Sha256::digest(bytes.as_slice()).into()))
    }
    fn summary(&self) -> Result<Summary> {
        Ok(Summary {
            id: self.0.id.context("The provider has no database ID.")?,
            uuid: self.0.uuid.clone(),
            name: self.0.name.clone(),
            protocol: self.0.protocol.clone(),
            base_url: self.0.base_url.clone(),
            model: self.0.model.clone(),
            active: self.0.active,
            stamp: self.stamp()?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp([u8; 32]);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub id: i64,
    pub uuid: Option<String>,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub model: String,
    pub active: bool,
    stamp: Stamp,
}

#[derive(Clone)]
pub struct Draft {
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub model: String,
    pub api_key: Zeroizing<String>,
    pub max_tokens: Option<i64>,
    pub temperature: Option<f64>,
}
impl Draft {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.trim().is_empty() && self.name.len() <= 1024,
            "A provider name is required."
        );
        let url = url::Url::parse(self.base_url.trim()).context("Enter a valid AI base URL.")?;
        ensure!(
            matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
            "Enter an HTTP or HTTPS AI base URL."
        );
        ensure!(
            self.base_url.len() <= 16_384
                && self.model.len() <= 65_536
                && self.api_key.len() <= 65_536,
            "Provider fields are too long."
        );
        ensure!(
            self.max_tokens.is_none_or(|value| value > 0),
            "Max Tokens must be a positive whole number or blank."
        );
        ensure!(
            self.temperature
                .is_none_or(|value| value.is_finite() && (0. ..=1.).contains(&value)),
            "Temperature must be between 0 and 1, or blank."
        );
        Ok(())
    }
    fn apply(&self, record: &mut Record) -> Result<()> {
        self.validate()?;
        record.0.name = self.name.trim().into();
        record.0.protocol = self.protocol.id().into();
        record.0.base_url = self.base_url.trim().into();
        record.0.model = self.model.trim().into();
        record.0.api_key.zeroize();
        record.0.api_key = self.api_key.to_string();
        for (key, value) in [
            ("max_tokens", self.max_tokens.map(Value::from)),
            ("temperature", self.temperature.map(Value::from)),
        ] {
            if let Some(value) = value {
                record.0.config.insert(key.into(), value);
            } else {
                record.0.config.remove(key);
            }
        }
        Ok(())
    }
}

pub struct Snapshot {
    pub rows: Vec<Summary>,
    pub selected: Option<Record>,
}

#[derive(Clone)]
pub struct Service {
    vault: Vault,
    pub presets_path: std::path::PathBuf,
}
impl Service {
    pub async fn provider_for_chat(&self, preferred: Option<String>) -> Result<Record> {
        self.vault
            .dispatch(move |db| {
                let providers = records(&db.inner)?;
                if let Some(uuid) = preferred {
                    providers
                        .into_iter()
                        .find(|record| record.0.uuid.as_ref() == Some(&uuid))
                        .context(
                            "The conversation's provider was removed. Choose another provider.",
                        )
                } else {
                    providers
                        .into_iter()
                        .find(|record| record.0.active)
                        .context("Configure an active AI provider in Settings first.")
                }
            })
            .await
    }
    pub async fn provider_summaries(&self) -> Result<Vec<Summary>> {
        self.vault.dispatch(|db| summaries(&db.inner)).await
    }
    pub fn new(vault: Vault, directory: std::path::PathBuf) -> Self {
        Self {
            vault,
            presets_path: directory.join("ai_presets_cache.json"),
        }
    }
    pub async fn load(&self, selected: Option<i64>) -> Result<Snapshot> {
        self.vault
            .dispatch(move |db| db.inner.atomic(|db| snapshot(db, selected)))
            .await
    }
    pub async fn select(&self, summary: Summary) -> Result<Record> {
        self.vault
            .dispatch(move |db| {
                db.inner
                    .atomic(|db| checked(db, summary.id, &summary.stamp))
            })
            .await
    }
    pub async fn save(&self, mut record: Record, draft: Draft) -> Result<Snapshot> {
        draft.validate()?;
        self.vault
            .dispatch(move |db| {
                db.inner.atomic(|db| {
                    let id = record.id().context("The provider was not saved.")?;
                    checked(db, id, &record.stamp()?)?;
                    draft.apply(&mut record)?;
                    let saved = Record(db.save_ai_provider(&record.0)?);
                    Ok(Snapshot {
                        rows: summaries(db)?,
                        selected: Some(saved),
                    })
                })
            })
            .await
            .inspect(|_| invalidate_providers())
    }
    pub async fn create(&self, draft: Draft) -> Result<Snapshot> {
        draft.validate()?;
        self.vault
            .dispatch(move |db| {
                db.inner.atomic(|db| {
                    let active = db.get_active_ai_provider()?.map(Record).is_none();
                    let mut record = Record(AiProviderEntry {
                        id: None,
                        uuid: None,
                        name: String::new(),
                        protocol: draft.protocol.id().into(),
                        base_url: String::new(),
                        model: String::new(),
                        api_key: String::new(),
                        config: Map::new(),
                        active,
                        created_at: None,
                        updated_at: None,
                        version: None,
                        created_device_id: None,
                        updated_device_id: None,
                    });
                    draft.apply(&mut record)?;
                    let saved = Record(db.save_ai_provider(&record.0)?);
                    Ok(Snapshot {
                        rows: summaries(db)?,
                        selected: Some(saved),
                    })
                })
            })
            .await
            .inspect(|_| invalidate_providers())
    }
    pub async fn activate(&self, summary: Summary) -> Result<Snapshot> {
        self.vault
            .dispatch(move |db| {
                db.inner.atomic(|db| {
                    let mut record = checked(db, summary.id, &summary.stamp)?;
                    record.0.active = true;
                    let saved = Record(db.save_ai_provider(&record.0)?);
                    Ok(Snapshot {
                        rows: summaries(db)?,
                        selected: Some(saved),
                    })
                })
            })
            .await
            .inspect(|_| invalidate_providers())
    }
    pub async fn delete(&self, summary: Summary, selected: Option<i64>) -> Result<Snapshot> {
        self.vault
            .dispatch(move |db| {
                db.inner.atomic(|db| {
                    let record = checked(db, summary.id, &summary.stamp)?;
                    ensure!(
                        db.delete_ai_provider(summary.id)? == 1,
                        "The provider was already removed."
                    );
                    if record.0.active {
                        let first = records(db)?.into_iter().next();
                        if let Some(mut next) = first {
                            next.0.active = true;
                            let _saved = Record(db.save_ai_provider(&next.0)?);
                        }
                    }
                    snapshot(db, selected)
                })
            })
            .await
            .inspect(|_| invalidate_providers())
    }
}

fn checked(db: &NautermDatabase, id: i64, expected: &Stamp) -> Result<Record> {
    let record = Record(
        db.get_ai_provider(id)?
            .context("The provider was removed by another client. Reload to continue.")?,
    );
    ensure!(
        &record.stamp()? == expected,
        "The provider changed in another client. Reload before saving."
    );
    Ok(record)
}
fn summaries(db: &NautermDatabase) -> Result<Vec<Summary>> {
    records(db)?
        .into_iter()
        .map(|record| record.summary())
        .collect()
}
fn records(db: &NautermDatabase) -> Result<Vec<Record>> {
    Ok(db.list_ai_providers()?.into_iter().map(Record).collect())
}
fn snapshot(db: &NautermDatabase, selected: Option<i64>) -> Result<Snapshot> {
    let rows = summaries(db)?;
    let id = selected
        .filter(|id| rows.iter().any(|row| row.id == *id))
        .or_else(|| rows.first().map(|row| row.id));
    let selected = id
        .map(|id| db.get_ai_provider(id))
        .transpose()?
        .flatten()
        .map(Record);
    Ok(Snapshot { rows, selected })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> Record {
        Record(AiProviderEntry {
            id: Some(1),
            uuid: Some("provider-uuid".into()),
            name: "Example".into(),
            protocol: "openai".into(),
            base_url: Protocol::OpenAi.base_url().into(),
            model: "example-model".into(),
            api_key: "secret-sentinel-key".into(),
            config: serde_json::from_value(
                serde_json::json!({"future":{"preserve":true},"max_tokens":4096}),
            )
            .unwrap(),
            active: true,
            created_at: Some(1),
            updated_at: Some(2),
            version: None,
            created_device_id: None,
            updated_device_id: None,
        })
    }
    #[test]
    fn editing_preserves_unknown_provider_config_and_clears_optional_generation_limits()
    -> Result<()> {
        let mut record = record();
        let mut draft = record.draft()?;
        draft.max_tokens = None;
        draft.temperature = Some(0.25);
        draft.api_key = Zeroizing::new("rotated-key".into());
        draft.apply(&mut record)?;
        assert_eq!(record.0.config["future"]["preserve"], true);
        assert!(!record.0.config.contains_key("max_tokens"));
        assert_eq!(record.0.config["temperature"], 0.25);
        assert_eq!(record.0.api_key, "rotated-key");
        assert_eq!(record.0.uuid.as_deref(), Some("provider-uuid"));
        Ok(())
    }
    #[test]
    fn summary_redacts_keys_but_conflict_stamp_covers_secret_and_activation_changes() -> Result<()>
    {
        let original = record();
        let summary = original.summary()?;
        assert!(!format!("{summary:?}").contains("secret-sentinel-key"));
        let mut changed = original.clone();
        changed.0.api_key = "different-secret".into();
        assert_ne!(original.stamp()?, changed.stamp()?);
        changed = original.clone();
        changed.0.active = false;
        assert_ne!(original.stamp()?, changed.stamp()?);
        Ok(())
    }
    #[test]
    fn protocol_aliases_and_generation_validation_match_reference_settings() -> Result<()> {
        assert_eq!(Protocol::parse("gemini")?, Protocol::Google);
        assert_eq!(Protocol::parse("googleai")?, Protocol::Google);
        assert!(Protocol::parse("future-protocol").is_err());
        let mut draft = record().draft()?;
        for temperature in [f64::NAN, -0.1, 1.1] {
            draft.temperature = Some(temperature);
            assert!(draft.validate().is_err());
        }
        draft.temperature = None;
        draft.max_tokens = Some(0);
        assert!(draft.validate().is_err());
        draft.max_tokens = None;
        draft.model.clear();
        draft.api_key.clear();
        draft.validate()?;
        draft.base_url = "file:///example".into();
        assert!(draft.validate().is_err());
        Ok(())
    }
}
