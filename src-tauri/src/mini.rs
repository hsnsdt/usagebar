//! Optional always-on-top mini window: a small draggable gauge the user can
//! leave anywhere on screen. Created on demand, destroyed when turned off.
//! Its position is remembered in settings (physical pixels).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, PhysicalPosition, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::settings;
use crate::state::AppState;

pub const LABEL: &str = "mini";
const WIDTH: f64 = 216.0;
const HEIGHT: f64 = 64.0;
/// Gap from the screen edge for the default spot, logical px.
const EDGE_MARGIN: f64 = 24.0;
/// Save the position this long after the last move event.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(600);

static MOVE_GEN: AtomicU64 = AtomicU64::new(0);

/// Show or destroy the mini window to match `on`.
pub fn apply<R: Runtime>(app: &AppHandle<R>, on: bool) {
    let existing = app.get_webview_window(LABEL);
    if !on {
        if let Some(w) = existing {
            let _ = w.destroy();
        }
        return;
    }
    if existing.is_some() {
        return;
    }
    // Never build a window on the calling thread: from a sync command or a
    // tray menu handler that deadlocks the Windows event loop (wry#583).
    let app = app.clone();
    std::thread::spawn(move || {
        if app.get_webview_window(LABEL).is_some() {
            return;
        }
        match create(&app) {
            Ok(w) => {
                place(&app, &w);
                let _ = w.show();
            }
            Err(e) => tracing::warn!("mini window failed: {e}"),
        }
    });
}

fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("UsageTray mini")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        // The OS shadow is rectangular and shows around the rounded corners.
        .shadow(false)
        .focused(false)
        .visible(false)
        .build()
}

/// Saved position if it is still on a connected monitor, else top-right of
/// the primary work area.
fn place<R: Runtime>(app: &AppHandle<R>, w: &WebviewWindow<R>) {
    let saved = app.state::<AppState>().settings().mini_pos;
    if let Some([x, y]) = saved {
        let on_screen = app.available_monitors().unwrap_or_default().iter().any(|m| {
            let (p, s) = (m.position(), m.size());
            x >= p.x && y >= p.y && x < p.x + s.width as i32 - 40 && y < p.y + s.height as i32 - 20
        });
        if on_screen {
            let _ = w.set_position(PhysicalPosition::new(x, y));
            return;
        }
        tracing::info!("saved mini position ({x},{y}) is off-screen; using default");
    }
    let Ok(Some(m)) = app.primary_monitor() else { return };
    let wa = m.work_area();
    let scale = m.scale_factor();
    let x = wa.position.x + wa.size.width as i32 - ((WIDTH + EDGE_MARGIN) * scale) as i32;
    let y = wa.position.y + (EDGE_MARGIN * scale) as i32;
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

/// Window moved (user drag): remember it, write settings once it settles.
pub fn on_moved<R: Runtime>(app: &AppHandle<R>, pos: PhysicalPosition<i32>) {
    let state = app.state::<AppState>();
    if let Ok(mut s) = state.settings.lock() {
        s.mini_pos = Some([pos.x, pos.y]);
    }
    let gen = MOVE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SAVE_DEBOUNCE).await;
        if MOVE_GEN.load(Ordering::SeqCst) != gen {
            return;
        }
        let s = app.state::<AppState>().settings();
        if let Err(e) = settings::save(&s) {
            tracing::warn!("mini position save failed: {e}");
        }
    });
}
