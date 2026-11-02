// GitHub sync via the GitHub REST API.
//
// Strategy: create a normal commit whose parent is the current branch head. Keeping the parent
// chain gives the sync UI a provider-native revision history while every stored revision remains
// independently encrypted.
//
// Flow:
//   1. Resolve the current branch and encrypted blob.
//   2. Merge the decrypted payload with the local database.
//   3. PUT the new encrypted blob through the Contents API with the current blob SHA.
//   4. GitHub creates a normal commit, preserving encrypted revision history.
//
// Conflict handling: on 409/422 responses to the PUT, we re-fetch the current remote blob,
// merge it against the local state again, encrypt, and retry. Bounded by MAX_ATTEMPTS.

use std::fmt;
use std::thread;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;
use zeroize::Zeroizing;

const GITHUB_API: &str = "https://api.github.com";
const USER_AGENT_VALUE: &str = "nauterm-sync/1";
const API_VERSION: &str = "2022-11-28";
const MAX_ATTEMPTS: u32 = 3;

#[derive(Debug)]
pub struct GithubSyncError {
    message: String,
    status: Option<u16>,
}

impl GithubSyncError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: None,
        }
    }
    pub fn with_status(message: impl Into<String>, status: u16) -> Self {
        Self {
            message: message.into(),
            status: Some(status),
        }
    }
    #[cfg(test)]
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn status(&self) -> Option<u16> {
        self.status
    }
}

impl fmt::Display for GithubSyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.status {
            Some(s) => write!(f, "GitHub sync error ({s}): {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for GithubSyncError {}

impl From<reqwest::Error> for GithubSyncError {
    fn from(err: reqwest::Error) -> Self {
        Self::new(format!("network error: {err}"))
    }
}

impl From<serde_json::Error> for GithubSyncError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(format!("json error: {err}"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GithubRepoConfig {
    pub repository_url: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default = "default_path")]
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GithubRevision {
    pub sha: String,
    pub message: String,
    pub committed_at: String,
    pub author: String,
}

fn default_branch() -> String {
    "main".into()
}
fn default_path() -> String {
    "nauterm-sync.enc".into()
}

#[derive(Debug)]
pub struct GithubClient {
    client: Client,
    token: Zeroizing<String>,
    cfg: GithubRepoConfig,
    remote: GithubRemote,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GithubRemote {
    owner: String,
    repo: String,
    api_base: String,
}

fn parse_github_repository_url(value: &str) -> Result<GithubRemote, GithubSyncError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(GithubSyncError::new("GitHub repository URL is required."));
    }

    let (host, path, api_scheme, api_port) = if value.contains("://") {
        let url = Url::parse(value)
            .map_err(|_| GithubSyncError::new("GitHub repository URL is invalid."))?;
        if !matches!(url.scheme(), "http" | "https" | "ssh" | "git") {
            return Err(GithubSyncError::new(
                "GitHub repository URL uses an unsupported scheme.",
            ));
        }
        let host = url
            .host_str()
            .ok_or_else(|| GithubSyncError::new("GitHub repository URL has no host."))?
            .to_ascii_lowercase();
        let port = url.port();
        let api_scheme = if url.scheme() == "http" {
            "http"
        } else {
            "https"
        };
        // The repository transport and GitHub Enterprise API may use
        // different schemes, but an explicit port still belongs to the same
        // server. Preserve it for HTTP(S), Git, and SSH URLs.
        let api_port = port;
        (host, url.path().to_string(), api_scheme, api_port)
    } else {
        let without_user = value
            .rsplit_once('@')
            .map(|(_, remote)| remote)
            .unwrap_or(value);
        let slash = without_user.find('/').ok_or_else(|| {
            GithubSyncError::new("Repository URL must include an owner and repository path.")
        })?;
        let authority = &without_user[..slash];
        let trailing_path = &without_user[slash + 1..];
        if let Some((host, suffix)) = authority.rsplit_once(':') {
            if let Ok(port) = suffix.parse::<u16>() {
                (
                    host.to_ascii_lowercase(),
                    trailing_path.to_string(),
                    "https",
                    Some(port),
                )
            } else {
                (
                    host.to_ascii_lowercase(),
                    format!("{suffix}/{trailing_path}"),
                    "https",
                    None,
                )
            }
        } else {
            (
                authority.to_ascii_lowercase(),
                trailing_path.to_string(),
                "https",
                None,
            )
        }
    };

    if host.trim().is_empty() {
        return Err(GithubSyncError::new("GitHub repository URL has no host."));
    }
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.len() != 2 {
        return Err(GithubSyncError::new(
            "Repository URL must end with owner/repository.",
        ));
    }
    let owner = segments[0].trim();
    let repo = segments[1]
        .trim()
        .strip_suffix(".git")
        .unwrap_or(segments[1].trim());
    if owner.is_empty() || repo.is_empty() {
        return Err(GithubSyncError::new(
            "Repository URL must end with owner/repository.",
        ));
    }

    let api_base = if host == "github.com" {
        GITHUB_API.to_string()
    } else {
        let port = api_port.map(|port| format!(":{port}")).unwrap_or_default();
        format!("{api_scheme}://{host}{port}/api/v3")
    };
    Ok(GithubRemote {
        owner: owner.to_string(),
        repo: repo.to_string(),
        api_base,
    })
}

pub fn validate_repository_url(value: &str) -> Result<(), GithubSyncError> {
    parse_github_repository_url(value).map(|_| ())
}

enum RemoteVault {
    EmptyRepository,
    Missing,
    Present { bytes: Vec<u8>, blob_sha: String },
}

impl GithubClient {
    pub fn new(token: String, cfg: GithubRepoConfig) -> Result<Self, GithubSyncError> {
        if token.trim().is_empty() {
            return Err(GithubSyncError::new("Missing GitHub token."));
        }
        let remote = parse_github_repository_url(&cfg.repository_url)?;
        let client = Client::builder()
            .user_agent(USER_AGENT_VALUE)
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            token: Zeroizing::new(token),
            cfg,
            remote,
        })
    }

    fn headers(&self) -> Result<HeaderMap, GithubSyncError> {
        let mut h = HeaderMap::new();
        h.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.token.as_str()))
                .map_err(|_| GithubSyncError::new("Invalid token contents."))?,
        );
        h.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        h.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        h.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static(API_VERSION),
        );
        Ok(h)
    }

    pub fn fetch_current(&self) -> Result<Option<Vec<u8>>, GithubSyncError> {
        Ok(match self.fetch_current_blob()? {
            RemoteVault::Present { bytes, .. } => Some(bytes),
            RemoteVault::EmptyRepository | RemoteVault::Missing => None,
        })
    }

    fn url(&self, tail: &str) -> String {
        format!(
            "{}/repos/{}/{}{}",
            self.remote.api_base, self.remote.owner, self.remote.repo, tail
        )
    }

    pub fn list_history(&self, limit: usize) -> Result<Vec<GithubRevision>, GithubSyncError> {
        let limit = limit.clamp(1, 100).to_string();
        let response = expect_ok(
            self.client
                .get(self.url("/commits"))
                .headers(self.headers()?)
                .query(&[
                    ("path", self.cfg.path.as_str()),
                    ("sha", self.cfg.branch.as_str()),
                    ("per_page", limit.as_str()),
                ])
                .send()?,
        )?;
        let rows: Vec<Value> = response.json()?;
        rows.into_iter()
            .map(|row| {
                let commit = &row["commit"];
                Ok(GithubRevision {
                    sha: row["sha"]
                        .as_str()
                        .ok_or_else(|| GithubSyncError::new("History entry is missing its SHA."))?
                        .to_string(),
                    message: commit["message"].as_str().unwrap_or_default().to_string(),
                    committed_at: commit["committer"]["date"]
                        .as_str()
                        .or_else(|| commit["author"]["date"].as_str())
                        .unwrap_or_default()
                        .to_string(),
                    author: commit["author"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                })
            })
            .collect()
    }

    pub fn restore_revision(&self, commit_sha: &str) -> Result<String, GithubSyncError> {
        if commit_sha.len() != 40 || !commit_sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(GithubSyncError::new("GitHub revision SHA is invalid."));
        }
        let body: Value = expect_ok(
            self.client
                .get(self.url(&format!("/contents/{}", self.cfg.path)))
                .headers(self.headers()?)
                .query(&[("ref", commit_sha)])
                .send()?,
        )?
        .json()?;
        let encoding = body["encoding"].as_str().unwrap_or("base64");
        let content = body["content"]
            .as_str()
            .ok_or_else(|| GithubSyncError::new("GitHub revision has no file content."))?;
        let bytes = match encoding {
            "base64" => STANDARD
                .decode(content.replace('\n', ""))
                .map_err(|_| GithubSyncError::new("Revision content is not valid base64."))?,
            "utf-8" => content.as_bytes().to_vec(),
            other => {
                return Err(GithubSyncError::new(format!(
                    "Unsupported revision encoding: {other}"
                )))
            }
        };
        let message = format!("Restore Nauterm sync revision {}", &commit_sha[..7]);
        match self.fetch_current_blob()? {
            RemoteVault::Present { blob_sha, .. } => {
                self.push_version(&bytes, Some(&blob_sha), &message)
            }
            RemoteVault::Missing => self.push_version(&bytes, None, &message),
            RemoteVault::EmptyRepository => self.bootstrap_empty_repository(&bytes, &message),
        }
    }

    /// Fetch the current blob content if the branch already contains the file.
    fn fetch_current_blob(&self) -> Result<RemoteVault, GithubSyncError> {
        // Resolve the branch ref → commit SHA.
        let ref_url = self.url(&format!("/git/ref/heads/{}", self.cfg.branch));
        let response = self.client.get(&ref_url).headers(self.headers()?).send()?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(RemoteVault::Missing);
        }
        if response.status() == StatusCode::CONFLICT {
            let code = response.status().as_u16();
            let text = response.text().unwrap_or_default();
            if is_empty_repository_response(code, &text) {
                return Ok(RemoteVault::EmptyRepository);
            }
            return Err(GithubSyncError::with_status(text, code));
        }
        let response = expect_ok(response)?;
        let ref_body: Value = response.json()?;
        let commit_sha = ref_body["object"]["sha"]
            .as_str()
            .ok_or_else(|| GithubSyncError::new("Malformed ref response from GitHub."))?;

        // Read the commit → tree SHA.
        let commit_url = self.url(&format!("/git/commits/{commit_sha}"));
        let commit_body: Value = expect_ok(
            self.client
                .get(&commit_url)
                .headers(self.headers()?)
                .send()?,
        )?
        .json()?;
        let tree_sha = commit_body["tree"]["sha"]
            .as_str()
            .ok_or_else(|| GithubSyncError::new("Malformed commit response from GitHub."))?;

        // Look up the blob by path inside the tree.
        let tree_url = self.url(&format!("/git/trees/{tree_sha}?recursive=1"));
        let tree_body: Value =
            expect_ok(self.client.get(&tree_url).headers(self.headers()?).send()?)?.json()?;
        let Some(entries) = tree_body["tree"].as_array() else {
            return Ok(RemoteVault::Missing);
        };
        let Some(entry) = entries
            .iter()
            .find(|e| e["path"].as_str() == Some(self.cfg.path.as_str()))
        else {
            return Ok(RemoteVault::Missing);
        };
        let blob_sha = entry["sha"]
            .as_str()
            .ok_or_else(|| GithubSyncError::new("Malformed tree response from GitHub."))?;
        let blob_url = self.url(&format!("/git/blobs/{blob_sha}"));
        let blob_body: Value =
            expect_ok(self.client.get(&blob_url).headers(self.headers()?).send()?)?.json()?;
        let encoding = blob_body["encoding"].as_str().unwrap_or("base64");
        let content = blob_body["content"]
            .as_str()
            .ok_or_else(|| GithubSyncError::new("Malformed blob response from GitHub."))?;
        let bytes = match encoding {
            "base64" => STANDARD
                .decode(content.replace('\n', ""))
                .map_err(|_| GithubSyncError::new("Blob is not valid base64."))?,
            "utf-8" => content.as_bytes().to_vec(),
            other => {
                return Err(GithubSyncError::new(format!(
                    "Unsupported blob encoding: {other}"
                )));
            }
        };
        Ok(RemoteVault::Present {
            bytes,
            blob_sha: blob_sha.to_string(),
        })
    }

    /// Create the first commit in a repository that has no branches.
    ///
    /// GitHub does not allow creating a Git ref in an empty repository, even when a commit
    /// object already exists. The Contents API is the supported bootstrap path; its first
    /// commit becomes the first entry in the encrypted revision history.
    fn bootstrap_empty_repository(
        &self,
        bytes: &[u8],
        message: &str,
    ) -> Result<String, GithubSyncError> {
        let body: Value = expect_ok(
            self.client
                .put(self.url(&format!("/contents/{}", self.cfg.path)))
                .headers(self.headers()?)
                .json(&json!({
                    "message": message,
                    "content": STANDARD.encode(bytes),
                    "branch": self.cfg.branch,
                }))
                .send()?,
        )?
        .json()?;
        body["commit"]["sha"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| GithubSyncError::new("Initial commit response missing SHA."))
    }

    /// Write a new version through the Contents API. GitHub links this commit to the current
    /// branch head, which preserves provider-native encrypted revision history.
    fn push_version(
        &self,
        bytes: &[u8],
        current_blob_sha: Option<&str>,
        message: &str,
    ) -> Result<String, GithubSyncError> {
        let mut body = serde_json::Map::from_iter([
            ("message".to_string(), Value::String(message.to_string())),
            ("content".to_string(), Value::String(STANDARD.encode(bytes))),
            ("branch".to_string(), Value::String(self.cfg.branch.clone())),
        ]);
        if let Some(sha) = current_blob_sha {
            body.insert("sha".to_string(), Value::String(sha.to_string()));
        }
        let response: Value = expect_ok(
            self.client
                .put(self.url(&format!("/contents/{}", self.cfg.path)))
                .headers(self.headers()?)
                .json(&body)
                .send()?,
        )?
        .json()?;
        response["commit"]["sha"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| GithubSyncError::new("Commit response missing SHA."))
    }
}

fn expect_ok(
    response: reqwest::blocking::Response,
) -> Result<reqwest::blocking::Response, GithubSyncError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let code = status.as_u16();
    let text = response.text().unwrap_or_default();
    Err(GithubSyncError::with_status(text, code))
}

fn is_empty_repository_response(status: u16, body: &str) -> bool {
    if status != StatusCode::CONFLICT.as_u16() {
        return false;
    }
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value["message"].as_str().map(str::to_owned))
        .is_some_and(|message| message.eq_ignore_ascii_case("Git Repository is empty."))
}

/// Perform a full push with `re-fetch and re-merge` on conflict.
///
/// The caller provides a closure that, given the current remote bytes (or `None` on first push),
/// produces the bytes to upload. This isolates encryption / merge concerns from the transport.
pub fn push_with_conflict_retry<F>(
    client: &GithubClient,
    mut build_payload: F,
    message: &str,
) -> Result<String, GithubSyncError>
where
    F: FnMut(Option<Vec<u8>>) -> Result<Vec<u8>, GithubSyncError>,
{
    let mut backoff = Duration::from_millis(500);
    let mut last_error: Option<GithubSyncError> = None;
    for attempt in 0..MAX_ATTEMPTS {
        let remote = client.fetch_current_blob()?;
        let (remote_bytes, current_blob_sha, empty_repository) = match remote {
            RemoteVault::EmptyRepository => (None, None, true),
            RemoteVault::Missing => (None, None, false),
            RemoteVault::Present { bytes, blob_sha } => (Some(bytes), Some(blob_sha), false),
        };
        let payload = build_payload(remote_bytes)?;
        let push = if empty_repository {
            client.bootstrap_empty_repository(&payload, message)
        } else {
            client.push_version(&payload, current_blob_sha.as_deref(), message)
        };
        match push {
            Ok(sha) => return Ok(sha),
            Err(err) => {
                let retryable = matches!(
                    err.status(),
                    Some(409) | Some(422) | Some(500) | Some(502) | Some(503) | Some(504)
                );
                if !retryable || attempt + 1 == MAX_ATTEMPTS {
                    return Err(err);
                }
                last_error = Some(err);
                thread::sleep(backoff);
                backoff *= 2;
            }
        }
    }
    Err(last_error.unwrap_or_else(|| GithubSyncError::new("push retries exhausted")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cfg_defaults() {
        let cfg: GithubRepoConfig =
            serde_json::from_str(r#"{"repository_url":"git@github.com:a/b.git"}"#).unwrap();
        assert_eq!(cfg.branch, "main");
        assert_eq!(cfg.path, "nauterm-sync.enc");
    }

    #[test]
    fn rejects_empty_repo() {
        let err = GithubClient::new(
            "x".into(),
            GithubRepoConfig {
                repository_url: "".into(),
                branch: "main".into(),
                path: "v.enc".into(),
            },
        )
        .unwrap_err();
        assert!(err.message().contains("URL"));
    }

    #[test]
    fn rejects_empty_token() {
        let err = GithubClient::new(
            "   ".into(),
            GithubRepoConfig {
                repository_url: "git@github.com:a/b.git".into(),
                branch: "main".into(),
                path: "v.enc".into(),
            },
        )
        .unwrap_err();
        assert!(err.message().contains("token"));
    }

    #[test]
    fn parses_github_repository_url_forms() {
        for (value, owner, repo, api_base) in [
            (
                "git@github.com:example/nauterm-sync.git",
                "example",
                "nauterm-sync",
                "https://api.github.com",
            ),
            (
                "https://github.com/example/nauterm-sync.git",
                "example",
                "nauterm-sync",
                "https://api.github.com",
            ),
            (
                "ssh://git@github.com:2222/example/nauterm-sync.git",
                "example",
                "nauterm-sync",
                "https://api.github.com",
            ),
            (
                "git@git.example.com:2222/example/nauterm-sync.git",
                "example",
                "nauterm-sync",
                "https://git.example.com:2222/api/v3",
            ),
            (
                "https://git.example.com:8443/example/nauterm-sync.git",
                "example",
                "nauterm-sync",
                "https://git.example.com:8443/api/v3",
            ),
            (
                "git://git.example.com:443/example/nauterm-sync",
                "example",
                "nauterm-sync",
                "https://git.example.com:443/api/v3",
            ),
            (
                "git.example.com:443/example/nauterm-sync.git",
                "example",
                "nauterm-sync",
                "https://git.example.com:443/api/v3",
            ),
            (
                "192.168.0.1:443/example/nauterm-sync",
                "example",
                "nauterm-sync",
                "https://192.168.0.1:443/api/v3",
            ),
            (
                "http://192.168.0.1:8080/example/nauterm-sync.git",
                "example",
                "nauterm-sync",
                "http://192.168.0.1:8080/api/v3",
            ),
        ] {
            let remote = parse_github_repository_url(value).unwrap();
            assert_eq!(remote.owner, owner);
            assert_eq!(remote.repo, repo);
            assert_eq!(remote.api_base, api_base);
        }
    }

    #[test]
    fn identifies_empty_repository_conflict() {
        assert!(is_empty_repository_response(
            409,
            r#"{"message":"Git Repository is empty.","status":"409"}"#,
        ));
        assert!(!is_empty_repository_response(
            409,
            r#"{"message":"Reference update failed.","status":"409"}"#,
        ));
        assert!(!is_empty_repository_response(
            422,
            r#"{"message":"Git Repository is empty.","status":"422"}"#,
        ));
    }
}
