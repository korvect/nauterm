use super::*;
use tokio::time::Instant;

const VERIFICATION_URI: &str = "https://github.com/login/device";
#[derive(Default, Deserialize)]
struct Device {
    #[serde(default)]
    device_code: String,
    #[serde(default)]
    user_code: String,
    #[serde(default)]
    verification_uri: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
    #[serde(default)]
    error: String,
}
impl Drop for Device {
    fn drop(&mut self) {
        self.device_code.zeroize();
    }
}
impl Device {
    fn validate(&self, status: u16) -> Result<()> {
        ensure!(
            self.error.is_empty(),
            "{}",
            authorization_error(&self.error)
        );
        ensure!(
            (200..300).contains(&status)
                && valid_secret(&self.device_code)
                && self.device_code.len() <= 4096
                && (1..=3600).contains(&self.expires_in)
                && (3..=32).contains(&self.user_code.len())
                && self
                    .user_code
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && self.verification_uri.trim_end_matches('/') == VERIFICATION_URI
                && self
                    .interval
                    .is_none_or(|interval| (1..=3600).contains(&interval)),
            "GitHub returned an invalid device authorization response."
        );
        Ok(())
    }
}
fn next_interval(current: u64, supplied: Option<u64>) -> Result<u64> {
    let next = current.saturating_add(5).max(supplied.unwrap_or_default());
    ensure!(next <= 3600, "GitHub returned an invalid polling interval.");
    Ok(next)
}
pub(super) async fn authorize(
    config: config::Config,
    client: reqwest::Client,
    events: &async_channel::Sender<Event>,
) -> Result<Authorized> {
    let response = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("scope", "gist read:user"),
        ])
        .send()
        .await
        .map_err(network_error)?;
    let (status, device): (_, Device) = payload(response).await?;
    device.validate(status)?;
    let mut interval = device.interval.unwrap_or(5).max(5);
    let expires = Instant::now() + Duration::from_secs(device.expires_in);
    progress(
        events,
        Event::Device {
            code: device.user_code.clone(),
            url: VERIFICATION_URI.into(),
            expires_in: device.expires_in,
        },
    )
    .await?;
    loop {
        let remaining = expires.saturating_duration_since(Instant::now());
        ensure!(
            !remaining.is_zero(),
            "The GitHub device code expired. Try signing in again."
        );
        tokio::time::sleep(Duration::from_secs(interval).min(remaining)).await;
        ensure!(
            Instant::now() < expires,
            "The GitHub device code expired. Try signing in again."
        );
        let response = client
            .post(config.token_uri())
            .header("Accept", "application/json")
            .form(&[
                ("client_id", config.client_id.as_str()),
                ("device_code", device.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(network_error)?;
        let (status, mut tokens): (_, Tokens) = payload(response).await?;
        match tokens.error.as_str() {
            "authorization_pending" => continue,
            "slow_down" => {
                interval = next_interval(interval, tokens.interval)?;
                continue;
            }
            _ => {}
        }
        tokens.validate_access(status)?;
        ensure!(
            tokens
                .scope
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .any(|scope| scope == "gist"),
            "GitHub did not grant the required gist permission."
        );
        progress(events, Event::Exchanging).await?;
        let mut authorization =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", tokens.access_token))
                .map_err(|_| anyhow::anyhow!("GitHub returned an unusable authorization token."))?;
        authorization.set_sensitive(true);
        let response = client
            .get("https://api.github.com/user")
            .header("Authorization", authorization)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(network_error)?;
        #[derive(Deserialize)]
        struct User {
            #[serde(default)]
            login: String,
        }
        let (status, user): (_, User) = payload(response).await?;
        ensure!(
            (200..300).contains(&status)
                && !user.login.is_empty()
                && user.login.len() <= 80
                && user
                    .login
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "GitHub did not return the authorized account."
        );
        let mut credentials = SyncProviderCredentials::default();
        credentials.values.insert(
            "token".into(),
            Zeroizing::new(std::mem::take(&mut tokens.access_token)),
        );
        return Ok(Authorized {
            credentials,
            account: Some(user.login),
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_codes_keep_poll_limits_and_only_use_the_github_verification_page() {
        let mut device = Device {
            device_code: "synthetic-device".into(),
            user_code: "ABCD-EFGH".into(),
            verification_uri: VERIFICATION_URI.into(),
            expires_in: 900,
            interval: Some(5),
            error: String::new(),
        };
        assert!(device.validate(200).is_ok());
        device.verification_uri = "https://attacker.invalid/login/device".into();
        assert!(device.validate(200).is_err());
        assert_eq!(next_interval(5, Some(6)).unwrap(), 10);
        assert_eq!(next_interval(10, Some(30)).unwrap(), 30);
        assert!(next_interval(u64::MAX, None).is_err());
    }
}
