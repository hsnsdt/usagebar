//! OAuth usage endpoint client, response cache and 429 backoff.
//!
//! The endpoint is undocumented. Every field is optional, unknown fields are
//! ignored, and nothing here panics on an unexpected body.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct UsageWindow {
    /// Percent 0..100 (the API already multiplies by 100).
    pub utilization: Option<f64>,
    /// RFC3339, UTC. Kept as string so a weird format never fails the parse.
    pub resets_at: Option<String>,
}

impl UsageWindow {
    pub fn resets_at_utc(&self) -> Option<DateTime<Utc>> {
        self.resets_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc))
    }
    pub fn is_usable(&self) -> bool {
        self.utilization.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct UsageResponse {
    pub five_hour: Option<UsageWindow>,
    pub seven_day: Option<UsageWindow>,
}

impl UsageResponse {
    /// True when at least one window carries a utilization number.
    pub fn has_data(&self) -> bool {
        self.five_hour.as_ref().is_some_and(UsageWindow::is_usable)
            || self.seven_day.as_ref().is_some_and(UsageWindow::is_usable)
    }

    pub fn from_value(v: &Value) -> Result<Self, FetchError> {
        if !v.is_object() {
            return Err(FetchError::BadSchema("response is not a JSON object".into()));
        }
        let parsed: UsageResponse = serde_json::from_value(v.clone())
            .map_err(|e| FetchError::BadSchema(e.to_string()))?;
        if !parsed.has_data() {
            return Err(FetchError::BadSchema(
                "no five_hour/seven_day utilization in response".into(),
            ));
        }
        Ok(parsed)
    }
}

#[derive(Debug, Error, Clone)]
pub enum FetchError {
    #[error("rate limited (429)")]
    RateLimited { retry_after_sec: Option<u64> },
    #[error("unauthorized ({0})")]
    Unauthorized(u16),
    #[error("http {status}")]
    Http { status: u16, body: String },
    #[error("network: {0}")]
    Network(String),
    #[error("unexpected schema: {0}")]
    BadSchema(String),
}

impl FetchError {
    pub fn is_offline(&self) -> bool {
        matches!(self, FetchError::Network(_))
    }
}

pub fn build_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(config::USER_AGENT)
        .timeout(Duration::from_secs(config::HTTP_TIMEOUT_SEC))
        .build()
}

/// Perform one request. Returns the raw JSON body so callers can log/print
/// exactly what came back.
pub async fn fetch_raw(client: &reqwest::Client, access_token: &str) -> Result<Value, FetchError> {
    let resp = client
        .get(config::USAGE_ENDPOINT)
        .bearer_auth(access_token)
        .header("anthropic-beta", config::ANTHROPIC_BETA)
        .header("User-Agent", config::USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| FetchError::Network(scrub(&e.to_string())))?;

    let status = resp.status();
    if status.as_u16() == 429 {
        let retry_after_sec = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok());
        return Err(FetchError::RateLimited { retry_after_sec });
    }
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(FetchError::Unauthorized(status.as_u16()));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| FetchError::Network(scrub(&e.to_string())))?;
    if !status.is_success() {
        return Err(FetchError::Http {
            status: status.as_u16(),
            body: body.chars().take(300).collect(),
        });
    }
    serde_json::from_str::<Value>(&body).map_err(|e| FetchError::BadSchema(e.to_string()))
}

pub async fn fetch(client: &reqwest::Client, access_token: &str) -> Result<UsageResponse, FetchError> {
    let raw = fetch_raw(client, access_token).await?;
    UsageResponse::from_value(&raw)
}

/// Make sure an error string can never leak a bearer token.
fn scrub(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for word in s.split_whitespace() {
        if word.starts_with("sk-ant-") {
            out.push_str("sk-ant-****");
        } else {
            out.push_str(word);
        }
        out.push(' ');
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------- cache

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageCache {
    pub fetched_at: DateTime<Utc>,
    pub response: UsageResponse,
}

pub fn cache_path() -> PathBuf {
    config::app_data_dir().join(config::CACHE_FILE)
}

pub fn load_cache() -> Option<UsageCache> {
    let bytes = std::fs::read(cache_path()).ok()?;
    serde_json::from_slice::<UsageCache>(&bytes).ok()
}

pub fn save_cache(cache: &UsageCache) -> anyhow::Result<()> {
    config::write_json_atomic(&cache_path(), cache)
}

// -------------------------------------------------------------- backoff

/// 5 -> 10 -> 20 -> 30 min, then stays at 30. `reset()` on any success.
#[derive(Debug, Default, Clone, Copy)]
pub struct Backoff {
    level: usize,
}

impl Backoff {
    /// Advance one step and return how long to wait.
    pub fn next_delay(&mut self) -> Duration {
        let idx = self.level.min(config::BACKOFF_STEPS_SEC.len() - 1);
        self.level = (self.level + 1).min(config::BACKOFF_STEPS_SEC.len());
        Duration::from_secs(config::BACKOFF_STEPS_SEC[idx])
    }
    pub fn reset(&mut self) {
        self.level = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_expected_body() {
        let v: Value = serde_json::json!({
            "five_hour": {"utilization": 33.0, "resets_at": "2026-09-22T18:00:00Z"},
            "seven_day": {"utilization": 13.0, "resets_at": "2026-09-28T14:00:00Z"},
            "some_new_field": {"x": 1}
        });
        let r = UsageResponse::from_value(&v).unwrap();
        assert_eq!(r.five_hour.as_ref().unwrap().utilization, Some(33.0));
        assert!(r.seven_day.as_ref().unwrap().resets_at_utc().is_some());
    }

    #[test]
    fn tolerates_nulls_and_missing() {
        let v: Value = serde_json::json!({"five_hour": {"utilization": 5}, "seven_day": null});
        let r = UsageResponse::from_value(&v).unwrap();
        assert!(r.seven_day.is_none());
        assert_eq!(r.five_hour.unwrap().utilization, Some(5.0));
    }

    #[test]
    fn rejects_empty_or_garbage() {
        assert!(matches!(
            UsageResponse::from_value(&serde_json::json!({})),
            Err(FetchError::BadSchema(_))
        ));
        assert!(matches!(
            UsageResponse::from_value(&serde_json::json!([1, 2])),
            Err(FetchError::BadSchema(_))
        ));
        assert!(matches!(
            UsageResponse::from_value(&serde_json::json!({"five_hour": {"utilization": "lots"}})),
            Err(FetchError::BadSchema(_))
        ));
    }

    #[test]
    fn backoff_schedule() {
        let mut b = Backoff::default();
        assert_eq!(b.next_delay().as_secs(), 300);
        assert_eq!(b.next_delay().as_secs(), 600);
        assert_eq!(b.next_delay().as_secs(), 1200);
        assert_eq!(b.next_delay().as_secs(), 1800);
        assert_eq!(b.next_delay().as_secs(), 1800);
        b.reset();
        assert_eq!(b.next_delay().as_secs(), 300);
    }

    #[test]
    fn scrub_hides_tokens() {
        assert_eq!(scrub("error with sk-ant-oat01-secret here"), "error with sk-ant-**** here");
    }
}
