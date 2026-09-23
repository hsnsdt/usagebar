//! Context window sizing.
//!
//! Claude Code does not record the model's context window in the transcript,
//! only the model id. We map the id to a window, then subtract the share that
//! autocompact reserves. A session that has already used more than the mapped
//! window proves the mapping wrong, so we escalate to the next tier instead of
//! showing a stuck 100%.

use crate::settings::Settings;

/// Standard window sizes, ascending. Used when escalating a wrong guess.
const TIERS: [u64; 4] = [200_000, 500_000, 1_000_000, 2_000_000];

/// Fallback when the model id says nothing useful.
pub const DEFAULT_WINDOW: u64 = 200_000;

/// Share of the window usable before autocompact kicks in.
/// 200_000 * 0.775 = 155_000, the figure the app shipped with.
const USABLE_RATIO: f64 = 0.775;

/// First one- or two-digit segment of a model id: the major version.
/// `claude-opus-5` -> 5, `claude-haiku-4-5-20251001` -> 4, `claude-3-5-sonnet` -> 3.
fn major_version(model: &str) -> Option<u32> {
    model
        .split(|c: char| !c.is_ascii_alphanumeric())
        .find(|p| (1..=2).contains(&p.len()) && p.chars().all(|c| c.is_ascii_digit()))
        .and_then(|p| p.parse().ok())
}

/// Total context window for a model id.
pub fn window_for_model(model: Option<&str>) -> u64 {
    let Some(m) = model else { return DEFAULT_WINDOW };
    let m = m.to_ascii_lowercase();
    if m.is_empty() {
        return DEFAULT_WINDOW;
    }
    // Explicit long-context variants, e.g. "claude-sonnet-4-5[1m]".
    if m.contains("1m") {
        return 1_000_000;
    }
    match major_version(&m) {
        Some(v) if v >= 5 => 1_000_000,
        Some(_) => 200_000,
        None => DEFAULT_WINDOW,
    }
}

/// Usable budget for a window, rounded down to 5K.
pub fn usable_from_window(window: u64) -> u64 {
    let raw = (window as f64 * USABLE_RATIO) as u64;
    (raw / 5_000) * 5_000
}

/// Effective usable budget: the manual setting, or the model-derived one.
/// `used` only matters as evidence that the mapping was too small.
pub fn resolve_usable(settings: &Settings, model: Option<&str>, used: u64) -> u64 {
    if !settings.auto_context_window {
        return settings.usable_context_tokens;
    }
    let mut window = window_for_model(model);
    if used > window {
        window = TIERS
            .iter()
            .copied()
            .find(|t| usable_from_window(*t) > used)
            .unwrap_or_else(|| TIERS[TIERS.len() - 1].max(used));
        tracing::debug!("context window escalated to {window} (used {used})");
    }
    usable_from_window(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auto() -> Settings {
        Settings { auto_context_window: true, ..Settings::default() }
    }

    #[test]
    fn maps_known_model_ids() {
        assert_eq!(window_for_model(Some("claude-opus-5")), 1_000_000);
        assert_eq!(window_for_model(Some("claude-fable-5-1")), 1_000_000);
        assert_eq!(window_for_model(Some("claude-sonnet-5")), 1_000_000);
        assert_eq!(window_for_model(Some("claude-haiku-4-5-20251001")), 200_000);
        assert_eq!(window_for_model(Some("claude-opus-4-1-20250805")), 200_000);
        assert_eq!(window_for_model(Some("claude-3-5-sonnet-20241022")), 200_000);
        assert_eq!(window_for_model(Some("claude-sonnet-4-5[1m]")), 1_000_000);
    }

    #[test]
    fn unknown_or_missing_falls_back() {
        assert_eq!(window_for_model(None), DEFAULT_WINDOW);
        assert_eq!(window_for_model(Some("")), DEFAULT_WINDOW);
        assert_eq!(window_for_model(Some("some-future-model")), DEFAULT_WINDOW);
    }

    #[test]
    fn usable_keeps_the_original_default() {
        assert_eq!(usable_from_window(200_000), 155_000);
        assert_eq!(usable_from_window(1_000_000), 775_000);
    }

    #[test]
    fn manual_setting_wins_when_auto_is_off() {
        let s = Settings { auto_context_window: false, usable_context_tokens: 123_000, ..Settings::default() };
        assert_eq!(resolve_usable(&s, Some("claude-opus-5"), 10), 123_000);
    }

    #[test]
    fn auto_uses_the_model() {
        assert_eq!(resolve_usable(&auto(), Some("claude-opus-5"), 423_472), 775_000);
        assert_eq!(resolve_usable(&auto(), Some("claude-haiku-4-5"), 50_000), 155_000);
    }

    #[test]
    fn overshooting_the_window_escalates() {
        // A 200K mapping cannot be right for a session that used 423K.
        let usable = resolve_usable(&auto(), Some("claude-mystery-4"), 423_472);
        assert_eq!(usable, 775_000);
        assert!(usable > 423_472);
        // Still inside the window: no escalation, 100% is the honest answer.
        assert_eq!(resolve_usable(&auto(), Some("claude-mystery-4"), 180_000), 155_000);
    }
}
