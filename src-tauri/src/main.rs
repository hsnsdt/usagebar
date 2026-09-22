#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod config;
mod credentials;
mod logging;
mod probe;
mod settings;
mod state;
mod toasts;
mod transcripts;
mod tray;
mod usage_api;
mod util;
mod win;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--probe") {
        let _guard = logging::init(false);
        std::process::exit(probe::run());
    }

    let _guard = logging::init(cfg!(debug_assertions));
    tracing::info!("UsageTray {} starting", env!("CARGO_PKG_VERSION"));
    app::run();
}
