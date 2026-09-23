//! Global shortcut that toggles the popup. The combination comes from
//! settings (`hotkey`, empty = off); a combination another app already owns
//! just fails to register and is logged.

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                crate::tray::toggle_popup(app);
            }
        })
        .build()
}

/// Replace whatever is registered with `hotkey`.
pub fn apply<R: Runtime>(app: &AppHandle<R>, hotkey: &str) {
    let gs = app.global_shortcut();
    if let Err(e) = gs.unregister_all() {
        tracing::warn!("hotkey unregister failed: {e}");
    }
    if hotkey.is_empty() {
        return;
    }
    match gs.register(hotkey) {
        Ok(()) => tracing::info!("hotkey registered: {hotkey}"),
        Err(e) => tracing::warn!("hotkey {hotkey} not registered (taken by another app?): {e}"),
    }
}
