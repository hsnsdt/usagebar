//! Small formatting helpers shared by tray tooltip and notifications.
//! (The popup UI formats on its own side; keep these in sync with `src/format.ts`.)

use chrono::{DateTime, Local, Utc};

/// `4sa 54dk` / `12dk` / `<1dk` : Turkish short duration until `until`.
pub fn remaining_tr(until: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (until - now).num_seconds();
    if secs <= 0 {
        return "şimdi".into();
    }
    let mins = (secs + 59) / 60;
    let days = mins / 1440;
    let hours = (mins % 1440) / 60;
    let m = mins % 60;
    if days > 0 {
        format!("{days}g {hours}sa")
    } else if hours > 0 {
        format!("{hours}sa {m}dk")
    } else if m > 0 {
        format!("{m}dk")
    } else {
        "<1dk".into()
    }
}

/// Local clock string for a reset time, e.g. `Pzt 14:59`.
pub fn local_clock_tr(at: DateTime<Utc>) -> String {
    let local = at.with_timezone(&Local);
    let day = match local.format("%a").to_string().as_str() {
        "Mon" => "Pzt",
        "Tue" => "Sal",
        "Wed" => "Çar",
        "Thu" => "Per",
        "Fri" => "Cum",
        "Sat" => "Cmt",
        "Sun" => "Paz",
        other => return format!("{other} {}", local.format("%H:%M")),
    };
    format!("{day} {}", local.format("%H:%M"))
}

pub fn percent_label(utilization: f64) -> String {
    format!("%{}", utilization.round().clamp(0.0, 999.0) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn remaining_formats() {
        let now = Utc::now();
        assert_eq!(remaining_tr(now + Duration::minutes(294), now), "4sa 54dk");
        assert_eq!(remaining_tr(now + Duration::minutes(12), now), "12dk");
        assert_eq!(remaining_tr(now + Duration::seconds(20), now), "1dk");
        assert_eq!(remaining_tr(now - Duration::seconds(20), now), "şimdi");
        assert_eq!(remaining_tr(now + Duration::hours(49), now), "2g 1sa");
    }

    #[test]
    fn percent_rounds() {
        assert_eq!(percent_label(19.6), "%20");
        assert_eq!(percent_label(0.0), "%0");
    }
}
