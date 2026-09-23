//! `%APPDATA%\UsageTray\settings.json`. Corrupt file -> defaults, never a crash.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct NotificationSettings {
    pub enabled: bool,
    pub five_hour: Vec<u8>,
    pub seven_day: Vec<u8>,
    pub context_low_tokens: u64,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            five_hour: vec![50, 75, 90],
            seven_day: vec![80, 95],
            context_low_tokens: 20_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub poll_interval_sec: u64,
    pub show_percent_text: bool,
    pub usable_context_tokens: u64,
    /// Derive the usable context from the session's model instead of using
    /// `usable_context_tokens`.
    pub auto_context_window: bool,
    pub start_with_windows: bool,
    /// "system" | "dark" | "light"
    pub theme: String,
    /// "system" | "tr" | "en"
    pub language: String,
    pub notifications: NotificationSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll_interval_sec: config::DEFAULT_POLL_INTERVAL_SEC,
            show_percent_text: false,
            usable_context_tokens: config::DEFAULT_USABLE_CONTEXT_TOKENS,
            auto_context_window: true,
            start_with_windows: true,
            theme: "dark".into(),
            language: "system".into(),
            notifications: NotificationSettings::default(),
        }
    }
}

impl Settings {
    /// Apply hard limits regardless of what the file / UI says.
    pub fn sanitized(mut self) -> Self {
        self.poll_interval_sec = self.poll_interval_sec.max(config::MIN_POLL_INTERVAL_SEC);
        if self.usable_context_tokens < 10_000 {
            self.usable_context_tokens = config::DEFAULT_USABLE_CONTEXT_TOKENS;
        }
        if !matches!(self.theme.as_str(), "system" | "dark" | "light") {
            self.theme = "dark".into();
        }
        if !matches!(self.language.as_str(), "system" | "tr" | "en") {
            self.language = "system".into();
        }
        let clamp = |v: Vec<u8>| -> Vec<u8> {
            let mut v: Vec<u8> = v.into_iter().filter(|p| (1..=100).contains(p)).collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        self.notifications.five_hour = clamp(self.notifications.five_hour);
        self.notifications.seven_day = clamp(self.notifications.seven_day);
        self
    }

    pub fn strings(&self) -> crate::i18n::Strings {
        crate::i18n::Strings::new(&self.language)
    }

    /// Effective poll interval in seconds, always >= MIN_POLL_INTERVAL_SEC.
    pub fn poll_interval(&self) -> u64 {
        self.poll_interval_sec.max(config::MIN_POLL_INTERVAL_SEC)
    }
}

pub fn settings_path() -> PathBuf {
    config::app_data_dir().join(config::SETTINGS_FILE)
}

pub fn load() -> Settings {
    match std::fs::read(settings_path()) {
        Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
            Ok(s) => s.sanitized(),
            Err(e) => {
                tracing::warn!("settings.json unreadable ({e}); using defaults");
                Settings::default()
            }
        },
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> anyhow::Result<()> {
    config::write_json_atomic(&settings_path(), &settings.clone().sanitized())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_interval_floor() {
        let s = Settings { poll_interval_sec: 10, ..Default::default() }.sanitized();
        assert_eq!(s.poll_interval_sec, config::MIN_POLL_INTERVAL_SEC);
        assert_eq!(Settings { poll_interval_sec: 0, ..Default::default() }.poll_interval(), 180);
    }

    #[test]
    fn partial_json_uses_defaults() {
        let s: Settings = serde_json::from_str(r#"{"showPercentText": true}"#).unwrap();
        assert!(s.show_percent_text);
        assert_eq!(s.poll_interval_sec, 300);
        assert_eq!(s.notifications.five_hour, vec![50, 75, 90]);
    }

    #[test]
    fn thresholds_are_cleaned() {
        let mut s = Settings::default();
        s.notifications.five_hour = vec![90, 0, 50, 150, 50];
        let s = s.sanitized();
        assert_eq!(s.notifications.five_hour, vec![50, 90]);
    }
}
