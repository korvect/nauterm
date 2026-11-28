//! Startup events shared with the Flutter client. The payload is limited to
//! installation and application metadata.
use crate::{APPLICATION_ID, APPLICATION_NAME, about, store::Store, vault::Vault};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, SecondsFormat, Utc};
use gpui_kit::App;
use nauterm_core::NautermDatabase;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use url::Url;
use uuid::Uuid;

const DEFAULT_HOST: &str = "https://us.i.posthog.com";
const STARTUP_METADATA: &str = "analytics_startup_v1";
static STARTED: AtomicBool = AtomicBool::new(false);

pub(crate) fn start(store: &Store, cx: &App) {
    let api_key = option_env!("NAUTERM_POSTHOG_API_KEY")
        .unwrap_or_default()
        .trim();
    if api_key.is_empty() {
        return;
    }
    let Some(vault) = store.loaded_vault() else {
        return;
    };
    // A new main window can load resources again within the same process.
    if STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let vault = vault.clone();
    cx.background_executor()
        .spawn(async move {
            if let Err(error) = report_startup(vault, api_key).await {
                eprintln!("Startup analytics: {error}");
            }
        })
        .detach();
}

#[derive(Deserialize, Serialize)]
struct FirstLaunch {
    timestamp_millis: i64,
    event_id: String,
    sent: bool,
}

impl FirstLaunch {
    fn load(database: &NautermDatabase) -> Result<Self> {
        if let Some(metadata) = database.get_app_meta(STARTUP_METADATA)? {
            return Ok(serde_json::from_str(&metadata)?);
        }
        let launch = Self {
            timestamp_millis: Utc::now().timestamp_millis(),
            event_id: Uuid::new_v4().to_string(),
            sent: false,
        };
        launch.save(database)?;
        Ok(launch)
    }

    fn save(&self, database: &NautermDatabase) -> Result<()> {
        database.set_app_meta(STARTUP_METADATA, &serde_json::to_string(self)?)?;
        Ok(())
    }

    fn events(&self, installation_id: &str, api_key: &str) -> Result<Vec<Value>> {
        let launched = DateTime::from_timestamp_millis(self.timestamp_millis)
            .context("Invalid first launch timestamp")?
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let capture = |event: &str, timestamp: &str| {
            json!({
                "api_key": api_key,
                "event": event,
                "distinct_id": installation_id,
                "timestamp": timestamp,
                "properties": {
                    "$os": std::env::consts::OS,
                    "$app_name": APPLICATION_NAME,
                    "$app_namespace": APPLICATION_ID,
                    "$app_version": about::VERSION,
                    "$app_build": about::BUILD,
                    "$geoip_disable": true,
                },
            })
        };
        let mut events = Vec::with_capacity(3);
        if !self.sent {
            let mut first_open = capture("first_open", &launched);
            // Keep the UUID and timestamp stable if acceptance or saving fails.
            first_open["uuid"] = json!(self.event_id);
            first_open["properties"]["first_launch_at"] = json!(self.timestamp_millis);
            events.push(first_open);
        }
        let mut screen = capture("$screen", &now);
        screen["properties"]["$screen_name"] = json!(APPLICATION_NAME);
        events.push(screen);
        let mut started = capture("app_started", &now);
        started["properties"]["$set_once"] = json!({ "first_launch_at": launched });
        events.push(started);
        Ok(events)
    }
}

async fn report_startup(vault: Vault, api_key: &'static str) -> Result<()> {
    let host = option_env!("NAUTERM_POSTHOG_HOST")
        .unwrap_or_default()
        .trim();
    let host = if host.is_empty() { DEFAULT_HOST } else { host };
    let endpoint = Url::parse(&format!("{}/i/v0/e/", host.trim_end_matches('/')))
        .context("Invalid PostHog host")?;
    ensure!(
        matches!(endpoint.scheme(), "http" | "https")
            && endpoint.host().is_some()
            && endpoint.username().is_empty()
            && endpoint.password().is_none()
            && endpoint.query().is_none()
            && endpoint.fragment().is_none(),
        "Invalid PostHog host"
    );
    let (installation_id, mut launch) = vault
        .dispatch(|database| {
            database
                .inner
                .atomic(|database| Ok((database.device_id()?, FirstLaunch::load(database)?)))
        })
        .await?;
    let events = launch.events(&installation_id, api_key)?;
    let first_open_accepted = smol::unblock(move || post_events(&endpoint, events)).await?;
    if first_open_accepted {
        launch.sent = true;
        vault
            .dispatch(move |database| launch.save(&database.inner))
            .await?;
    }
    Ok(())
}

fn post_events(endpoint: &Url, events: Vec<Value>) -> Result<bool> {
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .context("Could not create the PostHog HTTP client")?;
    let mut first_open_accepted = false;
    for event in events {
        match client.post(endpoint.clone()).json(&event).send() {
            Ok(response) if response.status().is_success() => {
                first_open_accepted |= event["event"] == "first_open";
            }
            Ok(response) => eprintln!("PostHog returned HTTP {}", response.status().as_u16()),
            Err(_) => eprintln!("Could not reach PostHog"),
        }
    }
    Ok(first_open_accepted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_open_keeps_its_identity_until_accepted() -> Result<()> {
        let database = NautermDatabase::open_in_memory()?;
        let installation_id = database.device_id()?;
        let original = FirstLaunch::load(&database)?.events(&installation_id, "test-token")?;
        let mut pending = FirstLaunch::load(&database)?;
        let retry = pending.events(&installation_id, "test-token")?;
        assert_eq!(retry[0], original[0]);
        assert_eq!(retry[0]["event"], "first_open");

        pending.sent = true;
        pending.save(&database)?;
        let next = FirstLaunch::load(&database)?.events(&installation_id, "test-token")?;
        assert_eq!(next.len(), 2);
        assert_eq!(next[0]["event"], "$screen");
        assert_eq!(next[1]["event"], "app_started");
        for event in next {
            assert_eq!(event["distinct_id"], installation_id);
            assert_eq!(event["properties"]["$app_namespace"], APPLICATION_ID);
            assert_eq!(event["properties"]["$geoip_disable"], true);
        }
        Ok(())
    }
}
