//! Tauri commands exposed to the popup. The frontend never fetches anything
//! itself; it reads state and pokes the loop.

use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_autostart::ManagerExt;

use crate::settings::Settings;
use crate::state::{AppState, Snapshot};
use crate::tray;

#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> Snapshot {
    state.snapshot()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn save_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    settings: Settings,
) -> Settings {
    let before = state.settings();
    let saved = state.update_settings(settings);
    if before.start_with_windows != saved.start_with_windows {
        let res = if saved.start_with_windows {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        if let Err(e) = res {
            tracing::warn!("autostart change failed: {e}");
        }
    }
    if before.mini_window != saved.mini_window {
        crate::mini::apply(&app, saved.mini_window);
        tray::rebuild_menu(&app);
    }
    if before.show_status != saved.show_status {
        let h = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::status_page::refresh(&h).await;
            crate::state::publish(&h);
        });
    }
    if before.hotkey != saved.hotkey {
        crate::hotkey::apply(&app, &saved.hotkey);
    }
    if before.language != saved.language {
        tray::rebuild_menu(&app);
    }
    // Re-derive context.usable + tray text without waiting for the next poll.
    if let Ok(mut snap) = state.snapshot.lock() {
        if let Some(ctx) = snap.context.as_mut() {
            ctx.usable = crate::context::resolve_usable(&saved, ctx.model.as_deref(), ctx.used);
            ctx.auto = saved.auto_context_window;
        }
    }
    crate::state::publish(&app);
    use tauri::Emitter;
    let _ = app.emit("settings-changed", &saved);
    saved
}

/// Turn the mini window on or off (its own close button, tray menu).
#[tauri::command]
pub fn set_mini_window<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, on: bool) {
    set_mini(&app, &state, on);
}

pub fn set_mini<R: Runtime>(app: &AppHandle<R>, state: &AppState, on: bool) {
    state.set_mini_window(on);
    crate::mini::apply(app, on);
    tray::rebuild_menu(app);
    use tauri::Emitter;
    let _ = app.emit("settings-changed", &state.settings());
}

/// Bucketed usage history for the last `hours` hours.
#[tauri::command]
pub fn get_history(hours: u32) -> crate::history::HistoryView {
    crate::history::view(hours, chrono::Utc::now())
}

/// Export the whole history as CSV into Downloads and select it in Explorer.
#[tauri::command]
pub fn export_history_csv() -> Result<String, String> {
    let file = crate::history::export_csv()?;
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", file.display()))
        .spawn();
    Ok(file.display().to_string())
}

/// Open the main popup (double-click / button on the mini window).
#[tauri::command]
pub fn open_popup<R: Runtime>(app: AppHandle<R>) {
    tray::show_popup(&app);
}

#[tauri::command]
pub fn is_first_run(state: State<'_, AppState>) -> bool {
    state.first_run
}

#[tauri::command]
pub fn refresh_now(state: State<'_, AppState>) {
    state.request_refresh();
}

#[tauri::command]
pub fn hide_popup<R: Runtime>(app: AppHandle<R>) {
    tray::hide_popup(&app);
}

#[tauri::command]
pub fn autostart_enabled<R: Runtime>(app: AppHandle<R>) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn open_logs_dir() {
    let dir = crate::config::app_data_dir().join(crate::config::LOG_DIR);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
}

/// Open a link in the default browser / mail client. Only https and mailto
/// are allowed so the webview cannot launch arbitrary programs.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("mailto:")) {
        return Err("unsupported url".into());
    }
    std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", &url])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn quit_app<R: Runtime>(app: AppHandle<R>) {
    app.exit(0);
}

/// Helper used by app setup: is there a main window?
pub fn has_main_window<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.get_webview_window("main").is_some()
}
