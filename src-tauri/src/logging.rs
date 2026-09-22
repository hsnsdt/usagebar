//! File logger under `%APPDATA%\UsageTray\logs`. Tokens never reach here:
//! every module masks/scrubs before logging.

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub struct LogGuard(#[allow(dead_code)] Option<tracing_appender::non_blocking::WorkerGuard>);

pub fn init(to_stderr: bool) -> LogGuard {
    let filter = EnvFilter::try_from_env("USAGETRAY_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let (file_layer, guard) = match crate::config::ensure_app_data_dir() {
        Ok(dir) => {
            let appender =
                tracing_appender::rolling::daily(dir.join(crate::config::LOG_DIR), "usagetray.log");
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let layer = fmt::layer().with_ansi(false).with_target(false).with_writer(writer);
            (Some(layer), Some(guard))
        }
        Err(_) => (None, None),
    };

    let stderr_layer = to_stderr.then(|| fmt::layer().with_ansi(false).with_target(false));

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .init();

    LogGuard(guard)
}
