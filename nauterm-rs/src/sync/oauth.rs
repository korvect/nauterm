//! Explicit user-initiated authorization. No provider credentials are persisted
//! until the owning editor saves; dropping a flow cancels requests/listeners.
use crate::cancel::Cancel;
use anyhow::{Result, ensure};
use futures_util::future::{Either, select};
use nauterm_core::SyncProviderCredentials;
use serde::{Deserialize, de::DeserializeOwned};
use std::time::Duration;
use zeroize::{Zeroize, Zeroizing};
mod callback;
mod cloud;
mod config;
mod github;
pub use config::Vendor;

pub struct Authorized {
    pub credentials: SyncProviderCredentials,
    pub account: Option<String>,
}
pub enum Event {
    Device {
        code: String,
        url: String,
        expires_in: u64,
    },
    Browser(String),
    Exchanging,
    Authorized(Authorized),
    Failed(String),
}
pub struct Flow {
    pub events: async_channel::Receiver<Event>,
    pub cancel: Cancel,
}
impl Drop for Flow {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
pub fn start(vendor: Vendor, language: crate::localization::Language) -> Result<Flow> {
    let config = config::Config::load(vendor)?;
    let (sender, events) = async_channel::bounded(8);
    let cancel = Cancel::new();
    let stop = cancel.clone();
    std::thread::Builder::new()
        .name("nauterm-oauth".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => {
                    let _ = sender.try_send(Event::Failed("Could not start authorization.".into()));
                    return;
                }
            };
            let outcome = runtime.block_on(async {
                let request = async {
                    let client = reqwest::Client::builder()
                        .user_agent("nauterm-sync/1")
                        .redirect(reqwest::redirect::Policy::none())
                        .connect_timeout(Duration::from_secs(10))
                        .timeout(Duration::from_secs(30))
                        .build()
                        .map_err(|_| anyhow::anyhow!("Could not start authorization."))?;
                    if vendor == Vendor::GithubGist {
                        github::authorize(config, client, &sender).await
                    } else {
                        cloud::authorize(config, client, &sender, language).await
                    }
                };
                match select(Box::pin(request), Box::pin(stop.wait())).await {
                    Either::Left((result, _)) => Some(result),
                    Either::Right(_) => None,
                }
            });
            if let Some(outcome) = outcome {
                let event = match outcome {
                    Ok(authorized) => Event::Authorized(authorized),
                    Err(error) => Event::Failed(error.to_string()),
                };
                let _ = sender.try_send(event);
            }
            runtime.shutdown_timeout(Duration::from_millis(250));
        })
        .map_err(|_| anyhow::anyhow!("Could not start authorization."))?;
    Ok(Flow { events, cancel })
}
async fn progress(sender: &async_channel::Sender<Event>, event: Event) -> Result<()> {
    sender
        .send(event)
        .await
        .map_err(|_| anyhow::anyhow!("Authorization was cancelled."))
}
fn network_error(error: reqwest::Error) -> anyhow::Error {
    if error.is_timeout() {
        anyhow::anyhow!("Authorization request timed out. Try again.")
    } else {
        anyhow::anyhow!("Authorization request failed. Check your connection and try again.")
    }
}
async fn payload<T: DeserializeOwned>(mut response: reqwest::Response) -> Result<(u16, T)> {
    const MAX_RESPONSE: usize = 128 * 1024;
    let status = response.status().as_u16();
    ensure!(
        response
            .content_length()
            .is_none_or(|length| length <= MAX_RESPONSE as u64),
        "Authorization response is too large."
    );
    let mut bytes = Zeroizing::new(Vec::new());
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        ensure!(
            bytes.len().saturating_add(chunk.len()) <= MAX_RESPONSE,
            "Authorization response is too large."
        );
        bytes.extend_from_slice(&chunk);
    }
    let decoded = serde_json::from_slice(&bytes).map_err(|_| {
        anyhow::anyhow!("The provider returned an invalid authorization response (HTTP {status}).")
    })?;
    Ok((status, decoded))
}
#[derive(Default, Deserialize)]
struct Tokens {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    token_type: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    interval: Option<u64>,
}
impl Drop for Tokens {
    fn drop(&mut self) {
        self.access_token.zeroize();
        self.refresh_token.zeroize();
    }
}
impl Tokens {
    fn validate_access(&self, status: u16) -> Result<()> {
        ensure!(
            self.error.is_empty(),
            "{}",
            authorization_error(&self.error)
        );
        ensure!(
            (200..300).contains(&status),
            "Authorization failed (HTTP {status})."
        );
        ensure!(
            valid_secret(&self.access_token)
                && (self.token_type.is_empty() || self.token_type.eq_ignore_ascii_case("bearer")),
            "The provider did not return a usable authorization token."
        );
        Ok(())
    }
}
fn valid_secret(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 16_384 && !value.chars().any(char::is_control)
}
fn authorization_error(code: &str) -> &'static str {
    match code {
        "access_denied" => "Authorization was denied. Try signing in again.",
        "expired_token" | "token_expired" => {
            "The authorization code expired. Try signing in again."
        }
        "device_flow_disabled" => "Device authorization is not enabled for this application.",
        "invalid_client" | "incorrect_client_credentials" => {
            "Sign-in is not configured correctly for this application."
        }
        "invalid_scope" | "redirect_uri_mismatch" | "unauthorized_client" => {
            "The application is not configured for this sign-in request."
        }
        "invalid_grant" => "Authorization could not be completed. Start a new sign-in.",
        _ => "The provider could not complete authorization. Try signing in again.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn token_validation_requires_real_success_and_never_replays_provider_error_text() {
        let mut tokens = Tokens {
            access_token: "synthetic-token".into(),
            token_type: "bearer".into(),
            refresh_token: String::new(),
            scope: String::new(),
            error: String::new(),
            interval: None,
        };
        assert!(tokens.validate_access(200).is_ok());
        assert!(tokens.validate_access(401).is_err());
        tokens.error = "synthetic-secret-from-provider-error".into();
        assert!(
            !tokens
                .validate_access(400)
                .unwrap_err()
                .to_string()
                .contains("synthetic-secret")
        );
        tokens.error.clear();
        tokens.access_token = "bad\nheader".into();
        assert!(tokens.validate_access(200).is_err());
    }
}
