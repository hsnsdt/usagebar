//! Single place for every constant that talks to the outside world.
//! Change the endpoint / headers / limits here and nowhere else.

use std::path::PathBuf;

/// Undocumented, community-discovered endpoint. May change without notice.
pub const USAGE_ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";

/// Required beta header for the OAuth usage endpoint.
pub const ANTHROPIC_BETA: &str = "oauth-2025-04-20";

/// Required. Without a Claude Code style user agent the endpoint returns a
/// permanent 429 for the token. Never remove, never change casually.
pub const USER_AGENT: &str = "claude-code/2.0.31";

/// Hard floor for the poll interval. Enforced in settings *and* in the loop.
pub const MIN_POLL_INTERVAL_SEC: u64 = 180;
pub const DEFAULT_POLL_INTERVAL_SEC: u64 = 300;

/// Random 0..=JITTER_MAX_SEC added to every poll wait.
pub const JITTER_MAX_SEC: u64 = 15;

/// Exponential backoff schedule after a 429 (seconds). Last value repeats.
pub const BACKOFF_STEPS_SEC: [u64; 4] = [300, 600, 1200, 1800];

/// HTTP timeout for a single usage request.
pub const HTTP_TIMEOUT_SEC: u64 = 20;

/// Default usable context (autocompact share already subtracted).
pub const DEFAULT_USABLE_CONTEXT_TOKENS: u64 = 155_000;

/// Directory name under %APPDATA%.
pub const APP_DIR_NAME: &str = "UsageTray";

pub const CACHE_FILE: &str = "usage-cache.json";
pub const SETTINGS_FILE: &str = "settings.json";
pub const SCAN_STATE_FILE: &str = "scan-state.json";
pub const NOTIFY_STATE_FILE: &str = "notify-state.json";
pub const LOG_DIR: &str = "logs";

/// `%APPDATA%\UsageTray`. Created on demand by callers.
pub fn app_data_dir() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join(APP_DIR_NAME);
    }
    directories::BaseDirs::new()
        .map(|b| b.config_dir().join(APP_DIR_NAME))
        .unwrap_or_else(|| PathBuf::from(".").join(APP_DIR_NAME))
}

pub fn ensure_app_data_dir() -> std::io::Result<PathBuf> {
    let dir = app_data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Home directory (`%USERPROFILE%`).
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf()))
}

/// `~/.claude` or `$CLAUDE_CONFIG_DIR`.
pub fn claude_config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    home_dir().map(|h| h.join(".claude"))
}

/// Write JSON atomically (tmp + rename) so a crash never leaves a half file.
pub fn write_json_atomic<T: serde::Serialize>(path: &std::path::Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(value)?;
    std::fs::write(&tmp, data)?;
    // On Windows rename fails if the target exists; remove first.
    let _ = std::fs::remove_file(path);
    std::fs::rename(&tmp, path)?;
    Ok(())
}
