//! Tauri application entry: plugins, state, tray, window events, poll loop.

use tauri::{Manager, WindowEvent};

use crate::state::AppState;
use crate::tray::{self, TrayState};

pub fn run() {
    let autostart_args: Vec<&str> = vec!["--minimized"];

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(autostart_args)
                .app_name("UsageTray")
                .build(),
        )
        .manage(AppState::new())
        .manage(TrayState::<tauri::Wry>::default())
        .invoke_handler(tauri::generate_handler![
            crate::commands::get_snapshot,
            crate::commands::get_settings,
            crate::commands::save_settings,
            crate::commands::refresh_now,
            crate::commands::is_first_run,
            crate::commands::hide_popup,
            crate::commands::autostart_enabled,
            crate::commands::open_logs_dir,
            crate::commands::app_version,
            crate::commands::open_url,
            crate::commands::quit_app,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            tray::build(&handle)?;

            // Keep the registry autostart entry in sync with settings.json.
            sync_autostart(&handle);

            crate::state::spawn_poll_loop(handle.clone());
            crate::transcripts::spawn(handle.clone());
            if !crate::commands::has_main_window(&handle) {
                tracing::error!("main window missing from tauri.conf.json");
            }
            // `--show` / `--settings`: open the popup immediately (debugging).
            // First run: open it so the welcome screen is seen.
            let args: Vec<String> = std::env::args().collect();
            let first_run = handle.state::<AppState>().first_run;
            let want_settings = args.iter().any(|a| a == "--settings");
            let want_about = args.iter().any(|a| a == "--about");
            if first_run || want_settings || want_about || args.iter().any(|a| a == "--show") {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                    tray::show_popup(&h);
                    use tauri::Emitter;
                    if want_settings {
                        let _ = h.emit("navigate", "settings");
                    } else if want_about {
                        let _ = h.emit("navigate", "about");
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Focused(focused) => {
                tracing::debug!("window {} focused={focused}", window.label());
                if !*focused && window.label() == "main" {
                    // WebView2's child HWND taking focus also arrives here; only
                    // hide when the foreground really left our process.
                    let w = window.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        if crate::win::foreground_is_ours() {
                            tracing::debug!("focus moved inside our process; keeping popup");
                            return;
                        }
                        if tray::in_show_grace(w.app_handle()) {
                            tracing::debug!("focus lost right after show; keeping popup");
                            return;
                        }
                        // `--pin`: debugging aid, never auto-hide (screenshots).
                        if std::env::args().any(|a| a == "--pin") {
                            return;
                        }
                        let (pid, title) = crate::win::foreground_info();
                        tracing::debug!("hiding popup; foreground pid={pid} title={title:?}");
                        let _ = w.hide();
                        tray::note_hidden(w.app_handle());
                    });
                }
            }
            WindowEvent::CloseRequested { api, .. } => {
                // Never destroy the popup; hiding keeps the webview warm.
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                // Closing the (hidden) window must not exit; only explicit quit does.
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

fn sync_autostart(app: &tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let want = app.state::<AppState>().settings().start_with_windows;
    let have = app.autolaunch().is_enabled().unwrap_or(false);
    if want != have {
        let res = if want { app.autolaunch().enable() } else { app.autolaunch().disable() };
        if let Err(e) = res {
            tracing::warn!("autostart sync failed: {e}");
        }
    }
}
