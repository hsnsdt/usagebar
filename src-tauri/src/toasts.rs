//! Windows toast notifications with per-window dedupe persisted to disk.
//! (Named `toasts` because `notify` is the file-watcher crate.)

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::settings::Settings;
use crate::state::{AppState, Snapshot, WindowSnap};
use crate::config;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ToastState {
    five_hour_reset: Option<String>,
    five_hour_fired: Vec<u8>,
    /// Highest 5-hour utilization seen in the window `five_hour_reset` names.
    five_hour_peak: f64,
    /// Reset time of the last window whose reset was announced.
    reset_announced: Option<String>,
    seven_day_reset: Option<String>,
    seven_day_fired: Vec<u8>,
    context_session: Option<String>,
    context_fired: bool,
    /// Per model/surface weekly limit (key: label): same dedupe as seven_day.
    scoped: BTreeMap<String, ScopedFired>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct ScopedFired {
    reset: Option<String>,
    fired: Vec<u8>,
}

fn path() -> PathBuf {
    config::app_data_dir().join(config::NOTIFY_STATE_FILE)
}

pub fn load() -> ToastState {
    std::fs::read(path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(s: &ToastState) {
    if let Err(e) = config::write_json_atomic(&path(), s) {
        tracing::warn!("notify-state write failed: {e}");
    }
}

struct Toast {
    title: String,
    body: String,
}

/// Decide which toasts to fire for this snapshot. Pure: mutates `st`, returns
/// toasts. Each threshold fires once per reset window; a jump across several
/// thresholds at once fires only the highest.
fn evaluate(st: &mut ToastState, snap: &Snapshot, settings: &Settings, session_file: Option<&str>) -> Vec<Toast> {
    let mut out = Vec::new();
    if !settings.notifications.enabled {
        return out;
    }
    let now = chrono::Utc::now();
    let st_lang = settings.strings();

    // API windows: only judge fresh data. Cached/stale numbers may belong to a
    // window that has already reset.
    let fresh = !snap.stale;

    // Reset first: window_toast below overwrites the stored reset time.
    if fresh && settings.notifications.on_reset {
        let min_peak = settings.notifications.five_hour.first().copied().unwrap_or(50) as f64;
        if reset_due(st, now, min_peak) {
            out.push(Toast {
                title: st_lang.toast_reset_title().into(),
                body: st_lang.toast_reset_body().into(),
            });
        }
    }

    if let Some(w) = snap.five_hour.as_ref().filter(|_| fresh) {
        if w.resets_at != st.five_hour_reset {
            st.five_hour_peak = 0.0;
        }
        st.five_hour_peak = st.five_hour_peak.max(w.utilization);
        if let Some(t) = window_toast(
            w,
            &settings.notifications.five_hour,
            &mut st.five_hour_reset,
            &mut st.five_hour_fired,
        ) {
            let body = st_lang.toast_five_body(w.resets_at_utc().map(|at| st_lang.remaining(at, now)));
            out.push(Toast { title: st_lang.toast_five_title(t), body });
        }
    }

    if let Some(w) = snap.seven_day.as_ref().filter(|_| fresh) {
        if let Some(t) = window_toast(
            w,
            &settings.notifications.seven_day,
            &mut st.seven_day_reset,
            &mut st.seven_day_fired,
        ) {
            out.push(Toast {
                title: st_lang.toast_week_title(t),
                body: st_lang.toast_week_body(w.resets_at_utc()),
            });
        }
    }

    // Model/surface limits share the weekly thresholds. Labels that vanish
    // from the response are forgotten so the state file cannot grow forever.
    if fresh {
        st.scoped.retain(|k, _| snap.scoped.iter().any(|l| &l.label == k));
        for l in &snap.scoped {
            let entry = st.scoped.entry(l.label.clone()).or_default();
            let w = WindowSnap { utilization: l.utilization, resets_at: l.resets_at.clone() };
            if let Some(t) = window_toast(&w, &settings.notifications.seven_day, &mut entry.reset, &mut entry.fired) {
                out.push(Toast {
                    title: st_lang.toast_scoped_title(&l.label, t),
                    body: st_lang.toast_week_body(w.resets_at_utc()),
                });
            }
        }
    }

    // Context low: once per session file; re-armed when context frees up.
    if let Some(ctx) = &snap.context {
        if st.context_session.as_deref() != session_file {
            st.context_session = session_file.map(str::to_string);
            st.context_fired = false;
        }
        let remaining = ctx.usable.saturating_sub(ctx.used);
        let low = settings.notifications.context_low_tokens;
        if low > 0 && remaining < low {
            if !st.context_fired {
                st.context_fired = true;
                out.push(Toast {
                    title: st_lang.toast_context_title().into(),
                    body: st_lang.toast_context_body(remaining / 1000),
                });
            }
        } else if remaining > low.saturating_mul(2) {
            st.context_fired = false;
        }
    }

    out
}

/// A reset is announced once, only for a window that got busy (peak at or
/// above the lowest 5-hour threshold) and only if we notice within an hour;
/// an app started long after the reset stays quiet.
fn reset_due(st: &mut ToastState, now: chrono::DateTime<chrono::Utc>, min_peak: f64) -> bool {
    let Some(stored) = st.five_hour_reset.clone() else { return false };
    let Ok(at) = chrono::DateTime::parse_from_rfc3339(&stored) else { return false };
    let since = now.signed_duration_since(at);
    if since < chrono::Duration::zero() || st.reset_announced.as_deref() == Some(stored.as_str()) {
        return false;
    }
    st.reset_announced = Some(stored);
    let due = st.five_hour_peak >= min_peak && since < chrono::Duration::hours(1);
    st.five_hour_peak = 0.0;
    due
}

/// Returns the threshold to announce, if any, and updates dedupe state.
fn window_toast(
    w: &WindowSnap,
    thresholds: &[u8],
    stored_reset: &mut Option<String>,
    fired: &mut Vec<u8>,
) -> Option<u8> {
    if w.resets_at != *stored_reset {
        *stored_reset = w.resets_at.clone();
        fired.clear();
    }
    let crossed: Vec<u8> = thresholds
        .iter()
        .copied()
        .filter(|t| w.utilization >= *t as f64 && !fired.contains(t))
        .collect();
    let top = crossed.iter().copied().max()?;
    fired.extend(crossed);
    fired.sort_unstable();
    fired.dedup();
    Some(top)
}

/// Called after every publish. Cheap when nothing crossed.
pub fn check<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let snap = state.snapshot();
    let settings = state.settings();
    let session_file = state.local.lock().ok().and_then(|l| l.session_file.clone());

    let toasts = {
        let Ok(mut st) = state.toasts.lock() else { return };
        let before = st.clone();
        let toasts = evaluate(&mut st, &snap, &settings, session_file.as_deref());
        if *st != before {
            save(&st);
        }
        toasts
    };

    for t in toasts {
        tracing::info!("toast: {}", t.title);
        if let Err(e) = app.notification().builder().title(&t.title).body(&t.body).show() {
            tracing::warn!("toast failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ContextSnap, Status, TodaySnap};

    fn snap(five: f64, reset: &str) -> Snapshot {
        Snapshot {
            status: Status::Ok,
            plan: None,
            five_hour: Some(WindowSnap { utilization: five, resets_at: Some(reset.into()) }),
            seven_day: None,
            scoped: Vec::new(),
            spend: None,
            service: None,
            context: None,
            today: TodaySnap::default(),
            week: Vec::new(),
            last_updated: None,
            stale: false,
            message: None,
            retry_in_sec: None,
            poll_interval_sec: 300,
        }
    }

    #[test]
    fn fires_once_per_threshold_and_resets_on_new_window() {
        // Pin the language: CI runners are English and would render "50%".
        let settings = Settings { language: "tr".into(), ..Settings::default() };
        let mut st = ToastState::default();
        assert!(evaluate(&mut st, &snap(40.0, "A"), &settings, None).is_empty());
        let t = evaluate(&mut st, &snap(55.0, "A"), &settings, None);
        assert_eq!(t.len(), 1);
        assert!(t[0].title.ends_with("%50"));
        assert!(evaluate(&mut st, &snap(60.0, "A"), &settings, None).is_empty());
        // Jump over 75 straight to 92: only %90 announced, both marked.
        let t = evaluate(&mut st, &snap(92.0, "A"), &settings, None);
        assert_eq!(t.len(), 1);
        assert!(t[0].title.ends_with("%90"));
        assert!(evaluate(&mut st, &snap(95.0, "A"), &settings, None).is_empty());
        // New window -> flags cleared.
        let t = evaluate(&mut st, &snap(76.0, "B"), &settings, None);
        assert_eq!(t.len(), 1);
        assert!(t[0].title.ends_with("%75"));
    }

    #[test]
    fn reset_announced_once_after_busy_window() {
        let settings = Settings { language: "en".into(), ..Settings::default() };
        let mut st = ToastState::default();
        let past = (chrono::Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
        let future = (chrono::Utc::now() + chrono::Duration::hours(4)).to_rfc3339();
        // Busy window whose reset time has just passed.
        let t = evaluate(&mut st, &snap(80.0, &past), &settings, None);
        assert_eq!(t.len(), 1);
        assert!(t[0].title.ends_with("75%"));
        // Next poll notices the reset.
        let t = evaluate(&mut st, &snap(2.0, &future), &settings, None);
        assert_eq!(t.len(), 1);
        assert!(t[0].title.contains("reset"));
        // No repeat, and the new window starts quietly.
        assert!(evaluate(&mut st, &snap(3.0, &future), &settings, None).is_empty());
    }

    #[test]
    fn quiet_window_reset_is_silent() {
        let settings = Settings::default();
        let mut st = ToastState::default();
        let past = (chrono::Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
        let future = (chrono::Utc::now() + chrono::Duration::hours(4)).to_rfc3339();
        assert!(evaluate(&mut st, &snap(10.0, &past), &settings, None).is_empty());
        assert!(evaluate(&mut st, &snap(1.0, &future), &settings, None).is_empty());
        // Busy window, but the reset toast is switched off.
        let mut off = Settings::default();
        off.notifications.on_reset = false;
        let mut st = ToastState::default();
        assert_eq!(evaluate(&mut st, &snap(95.0, &past), &off, None).len(), 1);
        assert!(evaluate(&mut st, &snap(1.0, &future), &off, None).is_empty());
    }

    #[test]
    fn scoped_limits_use_weekly_thresholds() {
        use crate::state::ScopedSnap;
        let settings = Settings { language: "en".into(), ..Settings::default() };
        let mut st = ToastState::default();
        let mut s = snap(0.0, "A");
        s.scoped = vec![ScopedSnap { label: "Fable".into(), utilization: 70.0, resets_at: Some("W".into()) }];
        assert!(evaluate(&mut st, &s, &settings, None).is_empty());
        s.scoped[0].utilization = 82.0;
        let t = evaluate(&mut st, &s, &settings, None);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].title, "Claude — Fable weekly limit 80%");
        assert!(evaluate(&mut st, &s, &settings, None).is_empty());
        // Label gone from the response: state is dropped.
        s.scoped.clear();
        evaluate(&mut st, &s, &settings, None);
        assert!(st.scoped.is_empty());
    }

    #[test]
    fn stale_data_is_silent() {
        let settings = Settings::default();
        let mut st = ToastState::default();
        let mut s = snap(99.0, "A");
        s.stale = true;
        assert!(evaluate(&mut st, &s, &settings, None).is_empty());
        s.stale = false;
        assert_eq!(evaluate(&mut st, &s, &settings, None).len(), 1);
    }

    #[test]
    fn disabled_means_silent() {
        let mut settings = Settings::default();
        settings.notifications.enabled = false;
        let mut st = ToastState::default();
        assert!(evaluate(&mut st, &snap(99.0, "A"), &settings, None).is_empty());
    }

    #[test]
    fn context_low_once_per_session() {
        let settings = Settings::default();
        let mut st = ToastState::default();
        let mut s = snap(0.0, "A");
        s.context = Some(ContextSnap { used: 140_000, usable: 155_000, project: None, model: None, auto: true });
        assert_eq!(evaluate(&mut st, &s, &settings, Some("f1")).len(), 1);
        assert_eq!(evaluate(&mut st, &s, &settings, Some("f1")).len(), 0);
        // Different session file re-arms.
        assert_eq!(evaluate(&mut st, &s, &settings, Some("f2")).len(), 1);
        // Context freed (compaction) re-arms within the same session.
        s.context = Some(ContextSnap { used: 20_000, usable: 155_000, project: None, model: None, auto: true });
        assert_eq!(evaluate(&mut st, &s, &settings, Some("f2")).len(), 0);
        s.context = Some(ContextSnap { used: 150_000, usable: 155_000, project: None, model: None, auto: true });
        assert_eq!(evaluate(&mut st, &s, &settings, Some("f2")).len(), 1);
    }
}
