//! Optional Claude service status from status.claude.com (Statuspage JSON).
//! Off by default: it is the only request that goes anywhere other than
//! api.anthropic.com. No token or identifying data is sent.

use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use crate::config;
use crate::state::{AppState, ServiceSnap};

/// "none" | "minor" | "major" | "critical" | "maintenance"; anything else is
/// passed through and shown as unknown by the UI.
pub fn parse(v: &Value) -> Option<ServiceSnap> {
    let status = v.get("status")?;
    let indicator = status.get("indicator")?.as_str()?.to_string();
    let description = status.get("description").and_then(Value::as_str).unwrap_or_default().to_string();
    Some(ServiceSnap { indicator, description })
}

async fn fetch() -> Result<ServiceSnap, String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("UsageTray/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(config::HTTP_TIMEOUT_SEC))
        .build()
        .map_err(|e| e.to_string())?;
    let v: Value = client
        .get(config::STATUS_ENDPOINT)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    parse(&v).ok_or_else(|| "unexpected status schema".into())
}

/// Fetch and store the status when the setting is on; clear it when off.
/// A failed fetch keeps the previous value.
pub async fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    if !state.settings().show_status {
        if let Ok(mut snap) = state.snapshot.lock() {
            snap.service = None;
        }
        return;
    }
    match fetch().await {
        Ok(s) => {
            tracing::info!("service status: {}", s.indicator);
            if let Ok(mut snap) = state.snapshot.lock() {
                snap.service = Some(s);
            }
        }
        Err(e) => tracing::warn!("service status fetch failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_statuspage_body() {
        let v = serde_json::json!({
            "page": {"id": "x", "name": "Claude"},
            "status": {"indicator": "minor", "description": "Partially Degraded Service"}
        });
        let s = parse(&v).unwrap();
        assert_eq!(s.indicator, "minor");
        assert_eq!(s.description, "Partially Degraded Service");
        assert!(parse(&serde_json::json!({"status": null})).is_none());
    }
}
