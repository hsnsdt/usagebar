//! Windows toast notifications with per-window dedupe persisted to disk.
//! (Named `toasts` because `notify` is the file-watcher crate.)

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
    seven_day_reset: Option<String>,
    seven_day_fired: Vec<u8>,
    context_session: Option<String>,
    context_fired: bool,
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

    if let Some(w) = snap.five_hour.as_ref().filter(|_| fresh) {
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
        let settings = Settings::default();
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
        s.context = Some(ContextSnap { used: 140_000, usable: 155_000, project: None, model: None });
        assert_eq!(evaluate(&mut st, &s, &settings, Some("f1")).len(), 1);
        assert_eq!(evaluate(&mut st, &s, &settings, Some("f1")).len(), 0);
        // Different session file re-arms.
        assert_eq!(evaluate(&mut st, &s, &settings, Some("f2")).len(), 1);
        // Context freed (compaction) re-arms within the same session.
        s.context = Some(ContextSnap { used: 20_000, usable: 155_000, project: None, model: None });
        assert_eq!(evaluate(&mut st, &s, &settings, Some("f2")).len(), 0);
        s.context = Some(ContextSnap { used: 150_000, usable: 155_000, project: None, model: None });
        assert_eq!(evaluate(&mut st, &s, &settings, Some("f2")).len(), 1);
    }
}
