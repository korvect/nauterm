use std::time::Duration;

use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::github_sync::{GithubRevision, GithubSyncError};

const GITHUB_API: &str = "https://api.github.com";
const USER_AGENT_VALUE: &str = "nauterm-sync/1";
const API_VERSION: &str = "2022-11-28";
const DEFAULT_FILENAME: &str = "nauterm-sync.enc";
const GIST_DESCRIPTION: &str = "Nauterm encrypted sync data";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GithubGistConfig {
    #[serde(default)]
    pub gist_id: String,
    #[serde(default = "default_filename")]
    pub filename: String,
}

impl Default for GithubGistConfig {
    fn default() -> Self {
        Self {
            gist_id: String::new(),
            filename: default_filename(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GithubGistWriteResult {
    pub gist_id: String,
    pub version: String,
}

fn default_filename() -> String {
    DEFAULT_FILENAME.to_string()
}

pub struct GithubGistClient {
    client: Client,
    token: Zeroizing<String>,
    cfg: GithubGistConfig,
}

impl GithubGistClient {
    pub fn new(token: String, mut cfg: GithubGistConfig) -> Result<Self, GithubSyncError> {
        if token.trim().is_empty() {
            return Err(GithubSyncError::new("Missing GitHub Gist token."));
        }
        cfg.gist_id = cfg.gist_id.trim().to_string();
        cfg.filename = cfg.filename.trim().to_string();
        if cfg.filename.is_empty() {
            cfg.filename = default_filename();
        }
        if cfg.filename.contains('/') || cfg.filename.contains('\\') {
            return Err(GithubSyncError::new(
                "GitHub Gist filename must not contain path separators.",
            ));
        }
        let client = Client::builder()
            .user_agent(USER_AGENT_VALUE)
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            token: Zeroizing::new(token),
            cfg,
        })
    }

    pub fn config(&self) -> &GithubGistConfig {
        &self.cfg
    }

    fn headers(&self) -> Result<HeaderMap, GithubSyncError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.token.as_str()))
                .map_err(|_| GithubSyncError::new("Invalid token contents."))?,
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static(API_VERSION),
        );
        Ok(headers)
    }

    pub fn fetch_current(&self) -> Result<Option<Vec<u8>>, GithubSyncError> {
        if self.cfg.gist_id.is_empty() {
            return Ok(None);
        }
        let body: Value = expect_ok(
            self.client
                .get(format!("{GITHUB_API}/gists/{}", self.cfg.gist_id))
                .headers(self.headers()?)
                .send()?,
        )?
        .json()?;
        let Some(file) = body["files"].get(&self.cfg.filename) else {
            return Ok(None);
        };
        if file["truncated"].as_bool().unwrap_or(false) {
            let raw_url = file["raw_url"]
                .as_str()
                .ok_or_else(|| GithubSyncError::new("Truncated Gist file has no raw URL."))?;
            let bytes =
                expect_ok(self.client.get(raw_url).headers(self.headers()?).send()?)?.bytes()?;
            return Ok(Some(bytes.to_vec()));
        }
        let content = file["content"]
            .as_str()
            .ok_or_else(|| GithubSyncError::new("GitHub Gist file has no content."))?;
        Ok(Some(content.as_bytes().to_vec()))
    }

    pub fn write(&self, bytes: &[u8]) -> Result<GithubGistWriteResult, GithubSyncError> {
        let content = std::str::from_utf8(bytes)
            .map_err(|_| GithubSyncError::new("Encrypted sync data is not valid UTF-8."))?;
        let files = json!({
            self.cfg.filename.clone(): {
                "content": content,
            },
        });
        let response = if self.cfg.gist_id.is_empty() {
            self.client
                .post(format!("{GITHUB_API}/gists"))
                .headers(self.headers()?)
                .json(&json!({
                    "description": GIST_DESCRIPTION,
                    "public": false,
                    "files": files,
                }))
                .send()?
        } else {
            self.client
                .patch(format!("{GITHUB_API}/gists/{}", self.cfg.gist_id))
                .headers(self.headers()?)
                .json(&json!({
                    "description": GIST_DESCRIPTION,
                    "files": files,
                }))
                .send()?
        };
        parse_write_response(expect_ok(response)?.json()?)
    }

    pub fn list_history(&self, limit: usize) -> Result<Vec<GithubRevision>, GithubSyncError> {
        if self.cfg.gist_id.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<Value> = expect_ok(
            self.client
                .get(format!("{GITHUB_API}/gists/{}/commits", self.cfg.gist_id))
                .headers(self.headers()?)
                .query(&[("per_page", limit.clamp(1, 100).to_string())])
                .send()?,
        )?
        .json()?;
        rows.into_iter()
            .map(|row| {
                let version = row["version"]
                    .as_str()
                    .ok_or_else(|| GithubSyncError::new("Gist revision has no version."))?;
                Ok(GithubRevision {
                    sha: version.to_string(),
                    message: "Encrypted Gist sync".to_string(),
                    committed_at: row["committed_at"].as_str().unwrap_or_default().to_string(),
                    author: row["user"]["login"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                })
            })
            .collect()
    }

    pub fn restore_version(&self, version: &str) -> Result<GithubGistWriteResult, GithubSyncError> {
        if self.cfg.gist_id.is_empty() {
            return Err(GithubSyncError::new(
                "GitHub Gist has not been created yet.",
            ));
        }
        if version.len() != 40 || !version.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(GithubSyncError::new("GitHub Gist revision is invalid."));
        }
        let body: Value = expect_ok(
            self.client
                .get(format!(
                    "{GITHUB_API}/gists/{}/{}",
                    self.cfg.gist_id, version
                ))
                .headers(self.headers()?)
                .send()?,
        )?
        .json()?;
        let file = body["files"]
            .get(&self.cfg.filename)
            .ok_or_else(|| GithubSyncError::new("Gist revision does not contain the sync file."))?;
        let bytes = if file["truncated"].as_bool().unwrap_or(false) {
            let raw_url = file["raw_url"]
                .as_str()
                .ok_or_else(|| GithubSyncError::new("Truncated Gist revision has no raw URL."))?;
            expect_ok(self.client.get(raw_url).headers(self.headers()?).send()?)?
                .bytes()?
                .to_vec()
        } else {
            file["content"]
                .as_str()
                .ok_or_else(|| GithubSyncError::new("Gist revision has no content."))?
                .as_bytes()
                .to_vec()
        };
        self.write(&bytes)
    }
}

fn expect_ok(response: Response) -> Result<Response, GithubSyncError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let code = status.as_u16();
    let text = response.text().unwrap_or_default();
    if status == StatusCode::NOT_FOUND {
        return Err(GithubSyncError::with_status(
            "GitHub Gist was not found or is not accessible.",
            code,
        ));
    }
    Err(GithubSyncError::with_status(text, code))
}

fn parse_write_response(body: Value) -> Result<GithubGistWriteResult, GithubSyncError> {
    let gist_id = body["id"]
        .as_str()
        .ok_or_else(|| GithubSyncError::new("GitHub Gist response has no ID."))?;
    let version = body["history"]
        .as_array()
        .and_then(|history| history.first())
        .and_then(|entry| entry["version"].as_str())
        .or_else(|| body["updated_at"].as_str())
        .unwrap_or_default();
    Ok(GithubGistWriteResult {
        gist_id: gist_id.to_string(),
        version: version.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_to_sync_filename() {
        let cfg: GithubGistConfig = serde_json::from_str(r#"{"gist_id":""}"#).unwrap();
        assert_eq!(cfg.filename, DEFAULT_FILENAME);
    }

    #[test]
    fn rejects_path_as_gist_filename() {
        let error = GithubGistClient::new(
            "token".to_string(),
            GithubGistConfig {
                gist_id: String::new(),
                filename: "folder/nauterm-sync.enc".to_string(),
            },
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("path separators"));
    }

    #[test]
    fn parses_created_gist_response() {
        let result = parse_write_response(json!({
            "id": "abc123",
            "history": [{"version": "deadbeef"}],
        }))
        .unwrap();
        assert_eq!(
            result,
            GithubGistWriteResult {
                gist_id: "abc123".to_string(),
                version: "deadbeef".to_string(),
            }
        );
    }
}
