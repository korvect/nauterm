use super::*;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use url::Url;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    GithubGist,
    GoogleCloudStorage,
    GoogleDrive,
    OneDrive,
    Dropbox,
}
pub(super) struct Config {
    pub vendor: Vendor,
    pub client_id: String,
    pub client_secret: Zeroizing<String>,
}
impl Config {
    pub fn load(vendor: Vendor) -> Result<Self> {
        let setting = |name: &str, compiled: Option<&str>| {
            compiled
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
                .or_else(|| std::env::var(name).ok())
                .unwrap_or_default()
                .trim()
                .to_owned()
        };
        let (name, compiled) = match vendor {
            Vendor::GithubGist => (
                "NAUTERM_GITHUB_CLIENT_ID",
                option_env!("NAUTERM_GITHUB_CLIENT_ID"),
            ),
            Vendor::GoogleCloudStorage | Vendor::GoogleDrive => (
                "NAUTERM_GOOGLE_CLIENT_ID",
                option_env!("NAUTERM_GOOGLE_CLIENT_ID"),
            ),
            Vendor::OneDrive => (
                "NAUTERM_ONEDRIVE_CLIENT_ID",
                option_env!("NAUTERM_ONEDRIVE_CLIENT_ID"),
            ),
            Vendor::Dropbox => (
                "NAUTERM_DROPBOX_CLIENT_ID",
                option_env!("NAUTERM_DROPBOX_CLIENT_ID"),
            ),
        };
        let client_id = setting(name, compiled);
        ensure!(
            !client_id.is_empty(),
            "Sign-in is not configured for this application."
        );
        ensure!(
            client_id.len() <= 4096 && !client_id.chars().any(char::is_control),
            "Sign-in is not configured correctly for this application."
        );
        let client_secret = if matches!(vendor, Vendor::GoogleCloudStorage | Vendor::GoogleDrive) {
            setting(
                "NAUTERM_GOOGLE_CLIENT_SECRET",
                option_env!("NAUTERM_GOOGLE_CLIENT_SECRET"),
            )
        } else {
            String::new()
        };
        ensure!(
            client_secret.len() <= 4096 && !client_secret.chars().any(char::is_control),
            "Sign-in is not configured correctly for this application."
        );
        Ok(Self {
            vendor,
            client_id,
            client_secret: Zeroizing::new(client_secret),
        })
    }
    pub fn host(&self) -> &'static str {
        if self.vendor == Vendor::OneDrive {
            "localhost"
        } else {
            "127.0.0.1"
        }
    }
    pub fn path(&self) -> &'static str {
        if self.vendor == Vendor::OneDrive {
            "/"
        } else {
            "/oauth/callback"
        }
    }
    pub fn port(&self) -> u16 {
        if self.vendor == Vendor::Dropbox {
            53682
        } else {
            0
        }
    }
    pub fn token_uri(&self) -> &'static str {
        match self.vendor {
            Vendor::GithubGist => "https://github.com/login/oauth/access_token",
            Vendor::GoogleCloudStorage | Vendor::GoogleDrive => {
                "https://oauth2.googleapis.com/token"
            }
            Vendor::OneDrive => "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            Vendor::Dropbox => "https://api.dropboxapi.com/oauth2/token",
        }
    }
    pub fn required_scope(&self) -> &'static str {
        match self.vendor {
            Vendor::GoogleCloudStorage => "https://www.googleapis.com/auth/devstorage.read_write",
            Vendor::GoogleDrive => "https://www.googleapis.com/auth/drive.file",
            Vendor::OneDrive => "Files.ReadWrite",
            Vendor::GithubGist => "gist",
            Vendor::Dropbox => "",
        }
    }
    pub fn authorization_uri(&self, redirect: &str, state: &str, challenge: &str) -> Url {
        let endpoint = match self.vendor {
            Vendor::GoogleCloudStorage | Vendor::GoogleDrive => {
                "https://accounts.google.com/o/oauth2/v2/auth"
            }
            Vendor::OneDrive => "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            Vendor::Dropbox => "https://www.dropbox.com/oauth2/authorize",
            Vendor::GithubGist => unreachable!("GitHub uses device authorization"),
        };
        let mut url = Url::parse(endpoint).expect("fixed authorization endpoint");
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("client_id", &self.client_id)
                .append_pair("redirect_uri", redirect)
                .append_pair("response_type", "code")
                .append_pair("state", state)
                .append_pair("code_challenge", challenge)
                .append_pair("code_challenge_method", "S256");
            if !self.required_scope().is_empty() {
                let scopes = if self.vendor == Vendor::OneDrive {
                    "offline_access Files.ReadWrite"
                } else {
                    self.required_scope()
                };
                query.append_pair("scope", scopes);
            }
            if matches!(
                self.vendor,
                Vendor::GoogleCloudStorage | Vendor::GoogleDrive
            ) {
                query
                    .append_pair("access_type", "offline")
                    .append_pair("prompt", "consent");
            }
            if self.vendor == Vendor::Dropbox {
                query.append_pair("token_access_type", "offline");
            }
        }
        url
    }
    pub fn credentials(&self, tokens: &mut Tokens, status: u16) -> Result<SyncProviderCredentials> {
        tokens.validate_access(status)?;
        ensure!(
            valid_secret(&tokens.refresh_token),
            "The provider did not return a refresh token. Start a new sign-in."
        );
        if !tokens.scope.is_empty() && !self.required_scope().is_empty() {
            ensure!(
                tokens.scope.split([',', ' ']).any(|scope| {
                    if self.vendor == Vendor::OneDrive {
                        scope.eq_ignore_ascii_case(self.required_scope())
                    } else {
                        scope == self.required_scope()
                    }
                }),
                "The required storage permission was not granted."
            );
        }
        let mut credentials = SyncProviderCredentials::default();
        credentials
            .values
            .insert("client_id".into(), Zeroizing::new(self.client_id.clone()));
        credentials.values.insert(
            "refresh_token".into(),
            Zeroizing::new(std::mem::take(&mut tokens.refresh_token)),
        );
        if !self.client_secret.is_empty() {
            credentials
                .values
                .insert("client_secret".into(), self.client_secret.clone());
        }
        Ok(credentials)
    }
}
pub(super) fn verifier() -> Zeroizing<String> {
    let bytes = Zeroizing::new(rand::random::<[u8; 64]>());
    Zeroizing::new(URL_SAFE_NO_PAD.encode(bytes.as_slice()))
}
pub(super) fn state() -> Zeroizing<String> {
    let bytes = Zeroizing::new(rand::random::<[u8; 32]>());
    Zeroizing::new(URL_SAFE_NO_PAD.encode(bytes.as_slice()))
}
pub(super) fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_and_authorization_requests_keep_secrets_out_of_browser_urls() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        for vendor in [
            Vendor::GoogleCloudStorage,
            Vendor::GoogleDrive,
            Vendor::OneDrive,
            Vendor::Dropbox,
        ] {
            let config = Config {
                vendor,
                client_id: "synthetic-id".into(),
                client_secret: Zeroizing::new("synthetic-secret".into()),
            };
            let url = config.authorization_uri(
                "http://127.0.0.1:1234/oauth/callback",
                "synthetic-state",
                "synthetic-challenge",
            );
            assert_eq!(url.scheme(), "https");
            assert!(!url.as_str().contains("synthetic-secret"));
            let query: std::collections::BTreeMap<_, _> = url.query_pairs().collect();
            assert_eq!(query["state"], "synthetic-state");
            assert_eq!(query["code_challenge_method"], "S256");
            assert_eq!(
                query["redirect_uri"],
                "http://127.0.0.1:1234/oauth/callback"
            );
            if vendor == Vendor::Dropbox {
                assert_eq!(query["token_access_type"], "offline");
            }
        }
    }
    #[test]
    fn cloud_credentials_require_offline_access_and_retain_the_registered_client() {
        let config = Config {
            vendor: Vendor::GoogleDrive,
            client_id: "registered-id".into(),
            client_secret: Zeroizing::new("synthetic-secret".into()),
        };
        let mut tokens = Tokens {
            access_token: "access".into(),
            refresh_token: String::new(),
            token_type: String::new(),
            scope: String::new(),
            error: String::new(),
            interval: None,
        };
        assert!(config.credentials(&mut tokens, 200).is_err());
        tokens.refresh_token = "refresh".into();
        tokens.scope = "https://www.googleapis.com/auth/drive.file".into();
        let credentials = config.credentials(&mut tokens, 200).unwrap();
        assert_eq!(credentials.values["client_id"].as_str(), "registered-id");
        assert_eq!(credentials.values["refresh_token"].as_str(), "refresh");
        assert!(!credentials.values.contains_key("access_token"));
    }
}
