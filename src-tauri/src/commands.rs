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
    saved
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
