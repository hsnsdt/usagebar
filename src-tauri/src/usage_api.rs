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
    /// Newer list format (`session`, `weekly_all`, `weekly_scoped`). Kept raw
    /// and read with `scoped_limits()` so one odd entry never fails the parse.
    pub limits: Option<Value>,
    /// Extra-usage spend in minor currency units.
    pub spend: Option<Value>,
}

/// A weekly limit that applies to one model or surface only (e.g. Fable).
#[derive(Debug, Clone, PartialEq)]
pub struct ScopedLimit {
    pub label: String,
    pub utilization: f64,
    pub resets_at: Option<String>,
}

/// Extra usage (pay-as-you-go credits past the plan limits).
#[derive(Debug, Clone, PartialEq)]
pub struct Spend {
    pub used: f64,
    pub limit: Option<f64>,
    pub currency: String,
    pub percent: Option<f64>,
}

impl UsageResponse {
    /// True when at least one window carries a utilization number.
    pub fn has_data(&self) -> bool {
        self.five_hour.as_ref().is_some_and(UsageWindow::is_usable)
            || self.seven_day.as_ref().is_some_and(UsageWindow::is_usable)
    }

    /// `weekly_scoped` entries of `limits[]`, in API order. Entries without a
    /// usable percent or name are skipped.
    pub fn scoped_limits(&self) -> Vec<ScopedLimit> {
        let Some(list) = self.limits.as_ref().and_then(Value::as_array) else { return Vec::new() };
        list.iter()
            .filter(|l| l.get("kind").and_then(Value::as_str) == Some("weekly_scoped"))
            .filter_map(|l| {
                let utilization = l.get("percent").and_then(Value::as_f64)?;
                let scope = l.get("scope")?;
                let label = ["model", "surface"]
                    .iter()
                    .filter_map(|k| scope.get(*k))
                    .find_map(|v| match v {
                        Value::String(s) => Some(s.clone()),
                        Value::Object(o) => o.get("display_name").and_then(Value::as_str).map(str::to_string),
                        _ => None,
                    })
                    .filter(|s| !s.trim().is_empty())?;
                let resets_at = l.get("resets_at").and_then(Value::as_str).map(str::to_string);
                Some(ScopedLimit { label, utilization, resets_at })
            })
            .collect()
    }

    /// Extra usage, only when the account has it switched on.
    pub fn spend(&self) -> Option<Spend> {
        let s = self.spend.as_ref()?;
        if s.get("enabled").and_then(Value::as_bool) != Some(true) {
            return None;
        }
        let money = |v: Option<&Value>| -> Option<(f64, String)> {
            let v = v?;
            let minor = v.get("amount_minor").and_then(Value::as_f64)?;
            let exp = v.get("exponent").and_then(Value::as_i64).unwrap_or(2).clamp(0, 6) as i32;
            let cur = v.get("currency").and_then(Value::as_str).unwrap_or("USD").to_string();
            Some((minor / 10f64.powi(exp), cur))
        };
        let (used, currency) = money(s.get("used"))?;
        Some(Spend {
            used,
            limit: money(s.get("limit")).map(|(v, _)| v),
            currency,
            percent: s.get("percent").and_then(Value::as_f64),
        })
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
        assert!(r.scoped_limits().is_empty());
        assert!(r.spend().is_none());
    }

    #[test]
    fn reads_scoped_limits_and_spend() {
        let v: Value = serde_json::json!({
            "five_hour": {"utilization": 1.0, "resets_at": "2026-09-23T18:30:00Z"},
            "limits": [
                {"kind": "session", "percent": 1, "scope": null},
                {"kind": "weekly_all", "percent": 4, "scope": null},
                {"kind": "weekly_scoped", "percent": 7, "resets_at": "2026-09-29T16:00:00Z",
                 "scope": {"model": {"display_name": "Fable", "id": null}, "surface": null}},
                {"kind": "weekly_scoped", "percent": 12, "scope": {"model": null, "surface": "Design"}},
                {"kind": "weekly_scoped", "percent": "x", "scope": {"model": {"display_name": "Bad"}}},
                {"kind": "weekly_scoped", "percent": 3, "scope": null}
            ],
            "spend": {"enabled": true, "percent": 61.5,
                      "used": {"amount_minor": 1538, "currency": "EUR", "exponent": 2},
                      "limit": {"amount_minor": 2500, "currency": "EUR", "exponent": 2}}
        });
        let r = UsageResponse::from_value(&v).unwrap();
        let s = r.scoped_limits();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].label, "Fable");
        assert_eq!(s[0].utilization, 7.0);
        assert!(s[0].resets_at.is_some());
        assert_eq!(s[1].label, "Design");
        let sp = r.spend().unwrap();
        assert!((sp.used - 15.38).abs() < 1e-9);
        assert_eq!(sp.limit, Some(25.0));
        assert_eq!(sp.currency, "EUR");
    }

    #[test]
    fn disabled_spend_is_hidden() {
        let v: Value = serde_json::json!({
            "five_hour": {"utilization": 1.0},
            "limits": null,
            "spend": {"enabled": false, "used": {"amount_minor": 0, "currency": "USD", "exponent": 2}}
        });
        let r = UsageResponse::from_value(&v).unwrap();
        assert!(r.spend().is_none());
        assert!(r.scoped_limits().is_empty());
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
