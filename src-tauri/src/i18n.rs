//! Strings that live on the Rust side (tray menu, tooltip, toasts, status
//! messages). Mirrors `src/i18n.ts`; keep both in sync.

use chrono::{DateTime, Local, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Tr,
    En,
}

/// "system" | "tr" | "en" -> effective language.
pub fn resolve(setting: &str) -> Lang {
    match setting {
        "tr" => Lang::Tr,
        "en" => Lang::En,
        _ => {
            let loc = sys_locale::get_locale().unwrap_or_default().to_lowercase();
            if loc.starts_with("tr") {
                Lang::Tr
            } else {
                Lang::En
            }
        }
    }
}

pub struct Strings {
    pub lang: Lang,
}

impl Strings {
    pub fn new(setting: &str) -> Self {
        Self { lang: resolve(setting) }
    }

    fn pick<'a>(&self, tr: &'a str, en: &'a str) -> &'a str {
        match self.lang {
            Lang::Tr => tr,
            Lang::En => en,
        }
    }

    // tray menu
    pub fn menu_refresh(&self) -> &'static str {
        self.pick("Yenile", "Refresh")
    }
    pub fn menu_settings(&self) -> &'static str {
        self.pick("Ayarlar", "Settings")
    }
    pub fn menu_autostart(&self) -> &'static str {
        self.pick("Başlangıçta çalıştır", "Start with Windows")
    }
    pub fn menu_quit(&self) -> &'static str {
        self.pick("Çıkış", "Quit")
    }

    // tooltip
    pub fn tip_no_creds(&self) -> &'static str {
        self.pick("UsageTray — Claude Code bulunamadı", "UsageTray — Claude Code not found")
    }
    pub fn tip_expired(&self) -> &'static str {
        self.pick("UsageTray — token süresi dolmuş", "UsageTray — token expired")
    }
    pub fn tip_error(&self) -> &'static str {
        self.pick("UsageTray — veri alınamadı", "UsageTray — couldn't fetch usage")
    }
    pub fn tip_five(&self, pct: &str, remaining: Option<String>) -> String {
        let head = self.pick("5s", "5h");
        match remaining {
            Some(r) => format!("{head}: {pct} · {}", self.fmt(&r, "{r} sonra sıfırlanır", "resets in {r}")),
            None => format!("{head}: {pct}"),
        }
    }
    pub fn tip_week(&self, pct: &str) -> String {
        format!("{}: {pct}", self.pick("Haftalık", "Weekly"))
    }
    pub fn tip_stale(&self) -> &'static str {
        self.pick("(bayat veri)", "(stale data)")
    }

    // status messages shown in the popup banner
    pub fn msg_rate_limited(&self, mins: u64) -> String {
        match self.lang {
            Lang::Tr => format!("Anthropic hız sınırı — {mins} dk sonra tekrar denenecek. Aşağıdaki veri bayat."),
            Lang::En => format!("Anthropic rate limit — retrying in {mins} min. Data below is stale."),
        }
    }
    pub fn msg_unauthorized(&self) -> &'static str {
        self.pick(
            "Token geçersiz. Terminalde bir kez `claude` çalıştır.",
            "Token rejected. Run `claude` once in a terminal.",
        )
    }
    pub fn msg_offline(&self) -> &'static str {
        self.pick("Bağlantı yok, son bilinen veri gösteriliyor.", "No connection, showing last known data.")
    }
    pub fn msg_error(&self) -> &'static str {
        self.pick("Kullanım verisi alınamadı. (detay için log)", "Couldn't fetch usage data. (see logs)")
    }
    pub fn msg_no_creds(&self) -> &'static str {
        self.pick(
            "Claude Code bulunamadı. Terminalde `claude` çalıştırıp giriş yap.",
            "Claude Code not found. Run `claude` in a terminal and sign in.",
        )
    }
    pub fn msg_expired(&self) -> &'static str {
        self.pick(
            "Token süresi dolmuş. Terminalde bir kez `claude` çalıştır.",
            "Token expired. Run `claude` once in a terminal.",
        )
    }
    pub fn msg_cred_malformed(&self) -> &'static str {
        self.pick("Credential dosyası okunamadı. (detay için log)", "Couldn't read the credential file. (see logs)")
    }

    // toasts
    pub fn toast_five_title(&self, pct: u8) -> String {
        match self.lang {
            Lang::Tr => format!("Claude — 5 saatlik limit %{pct}"),
            Lang::En => format!("Claude — 5-hour limit {pct}%"),
        }
    }
    pub fn toast_five_body(&self, remaining: Option<String>) -> String {
        match (self.lang, remaining) {
            (_, Some(r)) if is_now(&r) => self.pick("Sıfırlanmak üzere.", "About to reset.").to_string(),
            (Lang::Tr, Some(r)) => format!("{r} sonra sıfırlanıyor. İşini toparla."),
            (Lang::En, Some(r)) => format!("Resets in {r}. Wrap things up."),
            (Lang::Tr, None) => "İşini toparla.".into(),
            (Lang::En, None) => "Wrap things up.".into(),
        }
    }
    pub fn toast_week_title(&self, pct: u8) -> String {
        match self.lang {
            Lang::Tr => format!("Claude — haftalık limit %{pct}"),
            Lang::En => format!("Claude — weekly limit {pct}%"),
        }
    }
    pub fn toast_week_body(&self, at: Option<DateTime<Utc>>) -> String {
        match (self.lang, at) {
            (Lang::Tr, Some(at)) => format!("{}'da sıfırlanıyor.", self.local_clock(at)),
            (Lang::En, Some(at)) => format!("Resets {}.", self.local_clock(at)),
            (Lang::Tr, None) => "Haftalık kotan azalıyor.".into(),
            (Lang::En, None) => "Your weekly quota is running low.".into(),
        }
    }
    pub fn toast_context_title(&self) -> &'static str {
        self.pick("Claude — context azalıyor", "Claude — context running low")
    }
    pub fn toast_context_body(&self, remaining_k: u64) -> String {
        match self.lang {
            Lang::Tr => format!("{remaining_k}K token kaldı. Yakında autocompact tetiklenecek."),
            Lang::En => format!("{remaining_k}K tokens left. Autocompact will trigger soon."),
        }
    }

    // formatting
    pub fn percent(&self, utilization: f64) -> String {
        let n = utilization.round().clamp(0.0, 999.0) as u32;
        match self.lang {
            Lang::Tr => format!("%{n}"),
            Lang::En => format!("{n}%"),
        }
    }

    /// `4sa 54dk` / `4h 54m`; `şimdi` / `now` when in the past.
    pub fn remaining(&self, until: DateTime<Utc>, now: DateTime<Utc>) -> String {
        let secs = (until - now).num_seconds();
        if secs <= 0 {
            return self.pick("şimdi", "now").into();
        }
        let mins = (secs + 59) / 60;
        let days = mins / 1440;
        let hours = (mins % 1440) / 60;
        let m = mins % 60;
        let (d, h, mm) = match self.lang {
            Lang::Tr => ("g", "sa", "dk"),
            Lang::En => ("d", "h", "m"),
        };
        if days > 0 {
            format!("{days}{d} {hours}{h}")
        } else if hours > 0 {
            format!("{hours}{h} {m}{mm}")
        } else if m > 0 {
            format!("{m}{mm}")
        } else {
            format!("<1{mm}")
        }
    }

    /// `Pzt 14:59` / `Mon 14:59` local time.
    pub fn local_clock(&self, at: DateTime<Utc>) -> String {
        let local = at.with_timezone(&Local);
        let en = local.format("%a").to_string();
        let day = match self.lang {
            Lang::En => en,
            Lang::Tr => match en.as_str() {
                "Mon" => "Pzt",
                "Tue" => "Sal",
                "Wed" => "Çar",
                "Thu" => "Per",
                "Fri" => "Cum",
                "Sat" => "Cmt",
                "Sun" => "Paz",
                other => other,
            }
            .to_string(),
        };
        format!("{day} {}", local.format("%H:%M"))
    }

    fn fmt(&self, r: &str, tr: &str, en: &str) -> String {
        self.pick(tr, en).replace("{r}", r)
    }
}

pub fn is_now(s: &str) -> bool {
    s == "şimdi" || s == "now"
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn remaining_both_languages() {
        let now = Utc::now();
        let tr = Strings { lang: Lang::Tr };
        let en = Strings { lang: Lang::En };
        assert_eq!(tr.remaining(now + Duration::minutes(294), now), "4sa 54dk");
        assert_eq!(en.remaining(now + Duration::minutes(294), now), "4h 54m");
        assert_eq!(en.remaining(now + Duration::hours(49), now), "2d 1h");
        assert_eq!(tr.remaining(now - Duration::seconds(5), now), "şimdi");
        assert_eq!(en.remaining(now - Duration::seconds(5), now), "now");
    }

    #[test]
    fn percent_order() {
        assert_eq!(Strings { lang: Lang::Tr }.percent(19.6), "%20");
        assert_eq!(Strings { lang: Lang::En }.percent(19.6), "20%");
    }

    #[test]
    fn explicit_setting_wins() {
        assert_eq!(resolve("tr"), Lang::Tr);
        assert_eq!(resolve("en"), Lang::En);
    }

    #[test]
    fn toast_bodies() {
        let en = Strings { lang: Lang::En };
        assert_eq!(en.toast_five_body(Some("now".into())), "About to reset.");
        assert_eq!(en.toast_five_body(Some("4h 2m".into())), "Resets in 4h 2m. Wrap things up.");
        assert_eq!(en.toast_five_title(90), "Claude — 5-hour limit 90%");
    }
}
