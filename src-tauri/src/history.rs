//! Local usage history: one JSON line per recorded poll in
//! `%APPDATA%\UsageTray\usage-history.jsonl`. Only fresh API data is
//! recorded; unchanged values are written at most every RECORD_EVERY.
//! Nothing leaves the machine except an explicit CSV export.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::PathBuf;

use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::state::Snapshot;

/// Keep this much history on disk.
const RETENTION_DAYS: i64 = 35;
/// Write an unchanged sample again after this long (keeps gaps honest).
const RECORD_EVERY_MIN: i64 = 30;
/// Max points returned for one chart.
const MAX_POINTS: usize = 240;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sample {
    pub t: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub five: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub week: Option<f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scoped: BTreeMap<String, f64>,
}

impl Sample {
    fn from_snapshot(snap: &Snapshot, t: DateTime<Utc>) -> Sample {
        Sample {
            t,
            five: snap.five_hour.as_ref().map(|w| w.utilization),
            week: snap.seven_day.as_ref().map(|w| w.utilization),
            scoped: snap.scoped.iter().map(|l| (l.label.clone(), l.utilization)).collect(),
        }
    }
    fn same_values(&self, o: &Sample) -> bool {
        self.five == o.five && self.week == o.week && self.scoped == o.scoped
    }
}

/// Chart point: max of each bucket. `None` = no data in that bucket (gap).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    /// Bucket start, ms since epoch.
    pub t: i64,
    pub five: Option<f64>,
    pub week: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryView {
    pub from: i64,
    pub to: i64,
    pub points: Vec<Point>,
    pub peak_five: Option<f64>,
    pub peak_week: Option<f64>,
    /// Oldest sample on disk, ms since epoch (to say "recording since ...").
    pub first_sample: Option<i64>,
}

pub fn path() -> PathBuf {
    config::app_data_dir().join(config::HISTORY_FILE)
}

fn read_all() -> Vec<Sample> {
    let Ok(text) = std::fs::read_to_string(path()) else { return Vec::new() };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Sample>(l).ok())
        .collect()
}

/// Remembers the last written sample so recording is cheap.
#[derive(Default)]
pub struct History {
    last: Option<Sample>,
}

impl History {
    /// Load the tail and drop samples past retention (rewrites the file only
    /// when something was dropped).
    pub fn load() -> History {
        let all = read_all();
        let cutoff = Utc::now() - Duration::days(RETENTION_DAYS);
        let kept: Vec<Sample> = all.iter().filter(|s| s.t >= cutoff).cloned().collect();
        if kept.len() != all.len() {
            let body: String = kept
                .iter()
                .filter_map(|s| serde_json::to_string(s).ok())
                .map(|l| l + "\n")
                .collect();
            if let Err(e) = config::write_atomic(&path(), body.as_bytes()) {
                tracing::warn!("history prune failed: {e}");
            }
        }
        History { last: kept.last().cloned() }
    }

    /// Append a sample for fresh API data, unless it repeats the last one
    /// within RECORD_EVERY_MIN.
    pub fn record(&mut self, snap: &Snapshot, now: DateTime<Utc>) {
        let s = Sample::from_snapshot(snap, now);
        if s.five.is_none() && s.week.is_none() {
            return;
        }
        if let Some(last) = &self.last {
            if last.same_values(&s) && now - last.t < Duration::minutes(RECORD_EVERY_MIN) {
                return;
            }
        }
        let line = match serde_json::to_string(&s) {
            Ok(l) => l,
            Err(_) => return,
        };
        let res = std::fs::create_dir_all(config::app_data_dir()).and_then(|_| {
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path())?;
            writeln!(f, "{line}")
        });
        match res {
            Ok(()) => self.last = Some(s),
            Err(e) => tracing::warn!("history append failed: {e}"),
        }
    }
}

/// Bucketed view of the last `hours` hours.
pub fn view(hours: u32, now: DateTime<Utc>) -> HistoryView {
    build_view(&read_all(), hours, now)
}

fn build_view(all: &[Sample], hours: u32, now: DateTime<Utc>) -> HistoryView {
    let hours = hours.clamp(1, (RETENTION_DAYS * 24) as u32) as i64;
    let from = now - Duration::hours(hours);
    let span_ms = hours * 3_600_000;
    let bucket_ms = (span_ms / MAX_POINTS as i64).max(60_000);
    let n = ((span_ms + bucket_ms - 1) / bucket_ms) as usize;
    let from_ms = from.timestamp_millis();

    let mut points: Vec<Point> = (0..n)
        .map(|i| Point { t: from_ms + i as i64 * bucket_ms, five: None, week: None })
        .collect();
    let max = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    };
    let (mut peak_five, mut peak_week) = (None, None);
    for s in all.iter().filter(|s| s.t >= from && s.t <= now) {
        let i = (((s.t.timestamp_millis() - from_ms) / bucket_ms) as usize).min(n.saturating_sub(1));
        points[i].five = max(points[i].five, s.five);
        points[i].week = max(points[i].week, s.week);
        peak_five = max(peak_five, s.five);
        peak_week = max(peak_week, s.week);
    }
    HistoryView {
        from: from_ms,
        to: now.timestamp_millis(),
        points,
        peak_five,
        peak_week,
        first_sample: all.first().map(|s| s.t.timestamp_millis()),
    }
}

/// Write every sample to a CSV in the user's Downloads folder and return
/// its path. Model-specific limits get one column each.
pub fn export_csv() -> Result<PathBuf, String> {
    let all = read_all();
    if all.is_empty() {
        return Err("no history yet".into());
    }
    let dir = directories::UserDirs::new()
        .and_then(|u| u.download_dir().map(|d| d.to_path_buf()))
        .unwrap_or_else(config::app_data_dir);
    let file = dir.join(format!("usagetray-history-{}.csv", Local::now().format("%Y-%m-%d")));
    std::fs::write(&file, to_csv(&all)).map_err(|e| e.to_string())?;
    Ok(file)
}

fn to_csv(all: &[Sample]) -> String {
    let labels: BTreeSet<&String> = all.iter().flat_map(|s| s.scoped.keys()).collect();
    let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let num = |v: Option<f64>| v.map(|x| format!("{x}")).unwrap_or_default();
    let mut out = String::from("time_local,time_utc,five_hour_pct,weekly_pct");
    for l in &labels {
        out.push(',');
        out.push_str(&quote(&format!("{l}_weekly_pct")));
    }
    out.push('\n');
    for s in all {
        out.push_str(&s.t.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string());
        out.push(',');
        out.push_str(&s.t.to_rfc3339());
        out.push(',');
        out.push_str(&num(s.five));
        out.push(',');
        out.push_str(&num(s.week));
        for l in &labels {
            out.push(',');
            out.push_str(&num(s.scoped.get(*l).copied()));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(mins_ago: i64, five: f64, week: f64, now: DateTime<Utc>) -> Sample {
        Sample { t: now - Duration::minutes(mins_ago), five: Some(five), week: Some(week), scoped: BTreeMap::new() }
    }

    #[test]
    fn view_buckets_keep_peaks_and_gaps() {
        let now = Utc::now();
        let all = vec![s(600, 10.0, 1.0, now), s(599, 40.0, 1.0, now), s(30, 5.0, 2.0, now)];
        let v = build_view(&all, 24, now);
        assert_eq!(v.points.len(), MAX_POINTS);
        let filled: Vec<&Point> = v.points.iter().filter(|p| p.five.is_some()).collect();
        assert_eq!(filled.len(), 2, "two samples 1 min apart share a 6-min bucket");
        assert_eq!(filled[0].five, Some(40.0));
        assert_eq!(v.peak_five, Some(40.0));
        assert_eq!(v.peak_week, Some(2.0));
        // Out of range samples are ignored.
        let v = build_view(&all, 1, now);
        assert_eq!(v.peak_five, Some(5.0));
    }

    #[test]
    fn csv_has_scoped_columns() {
        let now = Utc::now();
        let mut a = s(10, 12.5, 3.0, now);
        a.scoped.insert("Fable".into(), 7.0);
        let b = Sample { five: None, ..s(5, 0.0, 4.0, now) };
        let csv = to_csv(&[a, b]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "time_local,time_utc,five_hour_pct,weekly_pct,\"Fable_weekly_pct\"");
        assert!(lines[1].ends_with(",12.5,3,7"));
        assert!(lines[2].ends_with(",,4,"));
    }

    #[test]
    fn same_values_detects_repeats() {
        let now = Utc::now();
        assert!(s(1, 5.0, 1.0, now).same_values(&s(2, 5.0, 1.0, now)));
        assert!(!s(1, 5.0, 1.0, now).same_values(&s(2, 6.0, 1.0, now)));
    }
}
