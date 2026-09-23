//! Shared application state, the poll loop and the `usage-updated` event.
//!
//! One tokio task owns the schedule. Everything else (tray menu, frontend
//! commands, file watcher) only nudges it via `request_refresh` or updates the
//! local-stats half of the snapshot and re-emits.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::Notify;

use crate::credentials::{self, CredentialError};
use crate::settings::{self, Settings};
use crate::usage_api::{self, Backoff, FetchError, UsageCache, UsageResponse};
use crate::{config, tray};

pub const EVENT_USAGE_UPDATED: &str = "usage-updated";

/// Minimum spacing between two real HTTP requests, even for manual refresh.
const MANUAL_REFRESH_MIN_GAP: Duration = Duration::from_secs(60);

/// The poll loop sleeps in slices of this length to notice a wake from sleep.
const WAKE_CHECK_SLICE: Duration = Duration::from_secs(30);
/// A slice that took this much longer than planned means the PC was asleep.
const WAKE_GAP_TOLERANCE: Duration = Duration::from_secs(60);
/// Wait after wake before polling, so the network is back.
const WAKE_SETTLE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    NoCredentials,
    TokenExpired,
    RateLimited,
    Offline,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowSnap {
    pub utilization: f64,
    /// RFC3339 UTC, or null when the API did not send one.
    pub resets_at: Option<String>,
}

impl WindowSnap {
    pub fn resets_at_utc(&self) -> Option<DateTime<Utc>> {
        self.resets_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc))
    }
    fn from_api(w: &usage_api::UsageWindow) -> Option<WindowSnap> {
        Some(WindowSnap {
            utilization: w.utilization?,
            resets_at: w.resets_at_utc().map(|d| d.to_rfc3339()),
        })
    }
}

/// A weekly limit that only counts one model or surface (e.g. Fable).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScopedSnap {
    pub label: String,
    pub utilization: f64,
    pub resets_at: Option<String>,
}

/// Extra usage spend, in major currency units.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpendSnap {
    pub used: f64,
    pub limit: Option<f64>,
    pub currency: String,
    pub percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnap {
    pub used: u64,
    pub usable: u64,
    pub project: Option<String>,
    pub model: Option<String>,
    /// `usable` came from the model, not from the manual setting.
    pub auto: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodaySnap {
    pub messages: u64,
    pub tokens: u64,
}

/// One local day of transcript activity.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DayStat {
    /// YYYY-MM-DD, local.
    pub date: String,
    pub messages: u64,
    pub tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub status: Status,
    pub plan: Option<String>,
    pub five_hour: Option<WindowSnap>,
    pub seven_day: Option<WindowSnap>,
    /// Model/surface specific weekly limits, API order.
    pub scoped: Vec<ScopedSnap>,
    /// Only present when extra usage is switched on for the account.
    pub spend: Option<SpendSnap>,
    pub context: Option<ContextSnap>,
    pub today: TodaySnap,
    /// Last 7 local days, oldest first, today last. Empty before the first scan.
    pub week: Vec<DayStat>,
    /// When the API data was last *successfully* fetched (ISO). Null before first success.
    pub last_updated: Option<String>,
    pub stale: bool,
    pub message: Option<String>,
    /// Seconds until the next automatic attempt (informational).
    pub retry_in_sec: Option<u64>,
    pub poll_interval_sec: u64,
}

impl Snapshot {
    fn initial(settings: &Settings) -> Snapshot {
        Snapshot {
            status: Status::Ok,
            plan: None,
            five_hour: None,
            seven_day: None,
            scoped: Vec::new(),
            spend: None,
            context: None,
            today: TodaySnap::default(),
            week: Vec::new(),
            last_updated: None,
            stale: true,
            message: None,
            retry_in_sec: None,
            poll_interval_sec: settings.poll_interval(),
        }
    }

    fn apply_response(&mut self, resp: &UsageResponse, fetched_at: DateTime<Utc>) {
        self.five_hour = resp.five_hour.as_ref().and_then(WindowSnap::from_api);
        self.seven_day = resp.seven_day.as_ref().and_then(WindowSnap::from_api);
        self.scoped = resp
            .scoped_limits()
            .into_iter()
            .map(|l| ScopedSnap {
                resets_at: usage_api::UsageWindow { utilization: None, resets_at: l.resets_at }
                    .resets_at_utc()
                    .map(|d| d.to_rfc3339()),
                label: l.label,
                utilization: l.utilization,
            })
            .collect();
        self.spend = resp.spend().map(|s| SpendSnap {
            used: s.used,
            limit: s.limit,
            currency: s.currency,
            percent: s.percent,
        });
        self.last_updated = Some(fetched_at.to_rfc3339());
    }
}

/// Local transcript derived numbers (filled by `transcripts.rs`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LocalStats {
    pub context_used: Option<u64>,
    pub project: Option<String>,
    pub model: Option<String>,
    /// Transcript file the context figure came from (toast dedupe key).
    pub session_file: Option<String>,
    pub today_messages: u64,
    pub today_tokens: u64,
    pub days: Vec<DayStat>,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub snapshot: Mutex<Snapshot>,
    pub local: Mutex<LocalStats>,
    pub toasts: Mutex<crate::toasts::ToastState>,
    /// No settings.json existed at startup: show the welcome screen once.
    pub first_run: bool,
    refresh: Notify,
    last_request: Mutex<Option<Instant>>,
    backoff: Mutex<Backoff>,
}

impl AppState {
    pub fn new() -> Self {
        let first_run = !settings::settings_path().exists();
        let settings = settings::load();
        let mut snapshot = Snapshot::initial(&settings);
        // Show cached data immediately; the loop will refresh shortly.
        if let Some(cache) = usage_api::load_cache() {
            snapshot.apply_response(&cache.response, cache.fetched_at);
            snapshot.stale = true;
        }
        Self {
            settings: Mutex::new(settings),
            snapshot: Mutex::new(snapshot),
            local: Mutex::new(LocalStats::default()),
            toasts: Mutex::new(crate::toasts::load()),
            first_run,
            refresh: Notify::new(),
            last_request: Mutex::new(None),
            backoff: Mutex::new(Backoff::default()),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().map(|s| s.clone()).unwrap_or_else(|_| Snapshot::initial(&self.settings()))
    }

    /// Ask the poll loop to run now (menu "Yenile", popup button).
    pub fn request_refresh(&self) {
        self.refresh.notify_one();
    }

    pub fn set_start_with_windows(&self, enabled: bool) {
        if let Ok(mut s) = self.settings.lock() {
            s.start_with_windows = enabled;
            if let Err(e) = settings::save(&s) {
                tracing::warn!("settings save failed: {e}");
            }
        }
    }

    /// Replace settings (from the UI), persist, and return the sanitized copy.
    pub fn update_settings(&self, new: Settings) -> Settings {
        let new = new.sanitized();
        if let Ok(mut s) = self.settings.lock() {
            *s = new.clone();
        }
        if let Err(e) = settings::save(&new) {
            tracing::warn!("settings save failed: {e}");
        }
        if let Ok(mut snap) = self.snapshot.lock() {
            snap.poll_interval_sec = new.poll_interval();
        }
        new
    }

    /// Merge transcript stats into the snapshot (does not touch API fields).
    pub fn set_local(&self, local: LocalStats) -> bool {
        let changed = self.local.lock().map(|mut l| {
            if *l == local {
                false
            } else {
                *l = local.clone();
                true
            }
        }).unwrap_or(false);
        if changed {
            let settings = self.settings();
            if let Ok(mut snap) = self.snapshot.lock() {
                snap.today = TodaySnap { messages: local.today_messages, tokens: local.today_tokens };
                snap.week = local.days.clone();
                snap.context = local.context_used.map(|used| ContextSnap {
                    used,
                    usable: crate::context::resolve_usable(&settings, local.model.as_deref(), used),
                    project: local.project.clone(),
                    model: local.model.clone(),
                    auto: settings.auto_context_window,
                });
            }
        }
        changed
    }
}

/// Emit the current snapshot to the webview and refresh the tray.
pub fn publish<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let snap = state.snapshot();
    tray::apply(app, &snap, &state.settings());
    if let Err(e) = app.emit(EVENT_USAGE_UPDATED, &snap) {
        tracing::warn!("emit failed: {e}");
    }
    crate::toasts::check(app);
}

// -------------------------------------------------------------- poll loop

pub fn spawn_poll_loop<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let client = match usage_api::build_client() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("http client: {e}");
                return;
            }
        };
        // First publish: cached data (or empty) so the UI has something.
        publish(&app);

        loop {
            let wait = poll_once(&app, &client).await;
            wait_for_next(&app, wait).await;
        }
    });
}

/// Sleep until `wait` has passed on the wall clock or a manual refresh
/// arrives. Sleeps in short slices because a monotonic timer may not count
/// the time the PC spent suspended; after a wake the next poll is pulled
/// forward, but never closer than the minimum interval to the last one.
async fn wait_for_next<R: Runtime>(app: &AppHandle<R>, wait: Duration) {
    let state = app.state::<AppState>();
    let start = Utc::now();
    let mut deadline = start + chrono::Duration::from_std(wait).unwrap_or_default();
    let floor = start + chrono::Duration::seconds(config::MIN_POLL_INTERVAL_SEC as i64);
    loop {
        let before = Utc::now();
        if before >= deadline {
            return;
        }
        let slice = (deadline - before).to_std().unwrap_or_default().min(WAKE_CHECK_SLICE);
        tokio::select! {
            _ = tokio::time::sleep(slice) => {}
            _ = state.refresh.notified() => {
                tracing::info!("manual refresh requested");
                return;
            }
        }
        let after = Utc::now();
        let gap = (after - before).to_std().unwrap_or_default();
        if gap > slice + WAKE_GAP_TOLERANCE {
            // Give Wi-Fi a moment to come back before polling.
            let early = (after + chrono::Duration::from_std(WAKE_SETTLE).unwrap_or_default()).max(floor);
            if early < deadline {
                tracing::info!("resumed after ~{}s asleep; polling early", gap.as_secs());
                deadline = early;
            }
        }
    }
}

/// One iteration. Returns how long to wait before the next one.
async fn poll_once<R: Runtime>(app: &AppHandle<R>, client: &reqwest::Client) -> Duration {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let interval = Duration::from_secs(settings.poll_interval().max(config::MIN_POLL_INTERVAL_SEC));
    let jitter = Duration::from_secs(fastrand::u64(0..=config::JITTER_MAX_SEC));

    // Manual refresh spam guard: never two requests closer than the gap.
    if let Ok(last) = state.last_request.lock() {
        if let Some(t) = *last {
            if t.elapsed() < MANUAL_REFRESH_MIN_GAP {
                let remaining = MANUAL_REFRESH_MIN_GAP - t.elapsed();
                tracing::info!("refresh too soon, re-publishing cached snapshot");
                publish(app);
                return remaining.max(Duration::from_secs(1));
            }
        }
    }

    let cred = match credentials::load() {
        Ok(c) => c,
        Err(err) => {
            let wait = handle_credential_error(&state, err);
            publish(app);
            return wait.unwrap_or(interval + jitter);
        }
    };

    if let Ok(mut last) = state.last_request.lock() {
        *last = Some(Instant::now());
    }
    let result = usage_api::fetch(client, &cred.access_token).await;
    let now = Utc::now();
    let st = settings.strings();

    let mut wait = interval + jitter;
    if let Ok(mut snap) = state.snapshot.lock() {
        snap.plan = cred.subscription_type.clone();
        match result {
            Ok(resp) => {
                snap.apply_response(&resp, now);
                snap.status = Status::Ok;
                snap.stale = false;
                snap.message = None;
                snap.retry_in_sec = None;
                if let Ok(mut b) = state.backoff.lock() {
                    b.reset();
                }
                if let Err(e) = usage_api::save_cache(&UsageCache { fetched_at: now, response: resp }) {
                    tracing::warn!("cache write failed: {e}");
                }
                tracing::info!(
                    "usage ok: 5h={:?} 7d={:?}",
                    snap.five_hour.as_ref().map(|w| w.utilization),
                    snap.seven_day.as_ref().map(|w| w.utilization)
                );
            }
            Err(FetchError::RateLimited { retry_after_sec }) => {
                let delay = state.backoff.lock().map(|mut b| b.next_delay()).unwrap_or(interval);
                let delay = match retry_after_sec {
                    Some(s) => delay.max(Duration::from_secs(s)),
                    None => delay,
                };
                wait = delay + jitter;
                snap.status = Status::RateLimited;
                snap.stale = true;
                snap.retry_in_sec = Some(delay.as_secs());
                snap.message = Some(st.msg_rate_limited((delay.as_secs() + 59) / 60));
                tracing::warn!("rate limited; backing off {}s", delay.as_secs());
            }
            Err(FetchError::Unauthorized(code)) => {
                snap.status = Status::TokenExpired;
                snap.stale = true;
                snap.retry_in_sec = Some(wait.as_secs());
                snap.message = Some(st.msg_unauthorized().into());
                tracing::warn!("unauthorized ({code})");
            }
            Err(e) if e.is_offline() => {
                // The request never reached the API: retry at the floor
                // interval so data comes back soon after the network does.
                wait = Duration::from_secs(config::MIN_POLL_INTERVAL_SEC) + jitter;
                snap.status = Status::Offline;
                snap.stale = true;
                snap.retry_in_sec = Some(wait.as_secs());
                snap.message = Some(st.msg_offline().into());
                tracing::warn!("offline: {e}");
            }
            Err(e) => {
                snap.status = Status::Error;
                snap.stale = true;
                snap.retry_in_sec = Some(wait.as_secs());
                snap.message = Some(st.msg_error().into());
                tracing::error!("usage fetch failed: {e}");
            }
        }
    }

    publish(app);
    wait
}

/// Update the snapshot for a credential problem. Returns an override wait.
fn handle_credential_error(state: &AppState, err: CredentialError) -> Option<Duration> {
    let st = state.settings().strings();
    let Ok(mut snap) = state.snapshot.lock() else { return None };
    snap.stale = true;
    snap.retry_in_sec = None;
    match err {
        CredentialError::NotFound => {
            snap.status = Status::NoCredentials;
            snap.plan = None;
            snap.five_hour = None;
            snap.seven_day = None;
            snap.scoped.clear();
            snap.spend = None;
            snap.message = Some(st.msg_no_creds().into());
            tracing::info!("no credentials");
        }
        CredentialError::Expired { subscription_type, .. } => {
            snap.status = Status::TokenExpired;
            snap.plan = subscription_type;
            snap.message = Some(st.msg_expired().into());
            tracing::info!("token expired; skipping request");
        }
        CredentialError::Malformed(m) => {
            snap.status = Status::Error;
            snap.message = Some(st.msg_cred_malformed().into());
            tracing::error!("credentials malformed: {m}");
        }
    }
    // Credentials can reappear any time (user runs `claude`); check again soon
    // but not aggressively. No HTTP request is made in this path.
    Some(Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_serializes_camel_case() {
        let s = Snapshot::initial(&Settings::default());
        let v = serde_json::to_value(&s).unwrap();
        assert!(v.get("fiveHour").is_some());
        assert!(v.get("lastUpdated").is_some());
        assert_eq!(v["status"], "ok");
        assert_eq!(v["pollIntervalSec"], 300);
    }

    #[test]
    fn apply_response_maps_windows() {
        let mut s = Snapshot::initial(&Settings::default());
        let resp = UsageResponse {
            five_hour: Some(usage_api::UsageWindow {
                utilization: Some(20.0),
                resets_at: Some("2026-09-22T18:00:00Z".into()),
            }),
            seven_day: Some(usage_api::UsageWindow { utilization: None, resets_at: None }),
            ..Default::default()
        };
        s.apply_response(&resp, Utc::now());
        assert_eq!(s.five_hour.as_ref().unwrap().utilization, 20.0);
        assert!(s.five_hour.as_ref().unwrap().resets_at_utc().is_some());
        assert!(s.seven_day.is_none());
    }
}
