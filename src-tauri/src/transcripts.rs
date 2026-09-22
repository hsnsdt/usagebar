//! Incremental scanner for Claude Code transcripts
//! (`~/.claude/projects/**/*.jsonl`).
//!
//! * Never re-reads a file from the start: a byte offset per file is kept in
//!   `scan-state.json`; a shrunken file resets its offset.
//! * Only `"type":"assistant"` lines matter. Broken / partial lines are skipped;
//!   an unterminated trailing line is left for the next pass.
//! * Per-day totals for the last `HISTORY_DAYS` days, deduped on
//!   `message.id + requestId`.
//! * Context = last non-sidechain assistant line of the most recently
//!   modified file.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use chrono::{DateTime, Days, Local, NaiveDate};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::config;
use crate::state::{AppState, DayStat, LocalStats};

const DEBOUNCE: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_secs(60);
/// When the newest file has no recorded last line, read at most this much of
/// its tail to find one.
const TAIL_BYTES: u64 = 512 * 1024;
/// Days of history kept (today included).
pub const HISTORY_DAYS: usize = 7;
/// Bump when the persisted layout changes; old files are discarded.
const STATE_VERSION: u32 = 2;

// ---------------------------------------------------------------- state

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LastUsage {
    pub used: u64,
    pub model: Option<String>,
    pub cwd: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct FileState {
    offset: u64,
    last: Option<LastUsage>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DayTotals {
    pub messages: u64,
    pub tokens: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanState {
    version: u32,
    files: HashMap<String, FileState>,
    /// YYYY-MM-DD (local) -> totals.
    days: BTreeMap<String, DayTotals>,
    /// YYYY-MM-DD -> dedupe keys seen for that day.
    seen: HashMap<String, HashSet<String>>,
}

fn state_path() -> PathBuf {
    config::app_data_dir().join(config::SCAN_STATE_FILE)
}

fn load_state() -> ScanState {
    let st: ScanState = std::fs::read(state_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if st.version == STATE_VERSION {
        st
    } else {
        ScanState { version: STATE_VERSION, ..Default::default() }
    }
}

pub fn projects_dir() -> Option<PathBuf> {
    config::claude_config_dir().map(|d| d.join("projects"))
}

// ---------------------------------------------------------------- lines

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "isSidechain")]
    is_sidechain: Option<bool>,
    message: Option<Msg>,
}

#[derive(Deserialize)]
struct Msg {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Usage {
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_input_tokens: u64,
    cache_read_input_tokens: u64,
}

impl Usage {
    fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_creation_input_tokens + self.cache_read_input_tokens
    }
}

/// Parsed view of one assistant line.
struct Assistant {
    key: String,
    day: Option<String>,
    timestamp: String,
    sidechain: bool,
    used: u64,
    model: Option<String>,
    cwd: Option<String>,
}

fn parse_assistant(raw: &[u8]) -> Option<Assistant> {
    // Cheap reject before JSON parsing; most lines are user/tool/progress.
    if !contains(raw, b"\"assistant\"") {
        return None;
    }
    let line: Line = serde_json::from_slice(raw).ok()?;
    if line.kind.as_deref() != Some("assistant") {
        return None;
    }
    let msg = line.message?;
    let usage = msg.usage?;
    let id = msg.id.unwrap_or_default();
    let req = line.request_id.unwrap_or_default();
    if id.is_empty() && req.is_empty() {
        return None;
    }
    let timestamp = line.timestamp.unwrap_or_default();
    let day = DateTime::parse_from_rfc3339(&timestamp)
        .ok()
        .map(|d| d.with_timezone(&Local).format("%Y-%m-%d").to_string());
    Some(Assistant {
        key: format!("{id}|{req}"),
        day,
        timestamp,
        sidechain: line.is_sidechain.unwrap_or(false),
        used: usage.total(),
        model: msg.model,
        cwd: line.cwd,
    })
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn day_string(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

// -------------------------------------------------------------- scanner

pub struct Scanner {
    state: ScanState,
}

impl Scanner {
    pub fn new() -> Self {
        Self { state: load_state() }
    }

    #[cfg(test)]
    fn with_state(state: ScanState) -> Self {
        Self { state }
    }

    pub fn scan(&mut self) -> LocalStats {
        let stats = match projects_dir() {
            Some(root) => self.scan_root(&root, Local::now().date_naive()),
            None => LocalStats::default(),
        };
        if let Err(e) = config::write_json_atomic(&state_path(), &self.state) {
            tracing::warn!("scan-state write failed: {e}");
        }
        stats
    }

    pub fn scan_root(&mut self, root: &Path, today: NaiveDate) -> LocalStats {
        let started = std::time::Instant::now();
        let first_day = today
            .checked_sub_days(Days::new((HISTORY_DAYS - 1) as u64))
            .unwrap_or(today);
        let cutoff = day_string(first_day);

        // Drop days that fell out of the window.
        self.state.days.retain(|d, _| d.as_str() >= cutoff.as_str());
        self.state.seen.retain(|d, _| d.as_str() >= cutoff.as_str());

        let window_start = first_day
            .and_hms_opt(0, 0, 0)
            .and_then(|t| t.and_local_timezone(Local).single());

        let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let pattern = root.join("**").join("*.jsonl");
        if let Ok(paths) = glob::glob(&pattern.to_string_lossy()) {
            for p in paths.flatten() {
                if let Ok(md) = std::fs::metadata(&p) {
                    if md.is_file() {
                        files.push((p, md.len(), md.modified().unwrap_or(std::time::UNIX_EPOCH)));
                    }
                }
            }
        }

        let mut present: HashSet<String> = HashSet::with_capacity(files.len());
        let mut newest: Option<(std::time::SystemTime, String)> = None;
        let mut jobs: Vec<(String, PathBuf, u64)> = Vec::new();

        for (path, len, mtime) in &files {
            let key = path.to_string_lossy().to_string();
            present.insert(key.clone());
            if newest.as_ref().is_none_or(|(t, _)| mtime > t) {
                newest = Some((*mtime, key.clone()));
            }
            let is_new = !self.state.files.contains_key(&key);
            let entry = self.state.files.entry(key.clone()).or_default();
            if *len < entry.offset {
                tracing::debug!("{key}: truncated, rescanning");
                entry.offset = 0;
                entry.last = None;
            }
            if is_new {
                // A file untouched inside the window cannot add to it: skip its body.
                let in_window = window_start
                    .map(|w| DateTime::<Local>::from(*mtime) >= w)
                    .unwrap_or(true);
                if !in_window {
                    entry.offset = *len;
                    continue;
                }
            }
            if *len > entry.offset {
                jobs.push((key.clone(), path.clone(), entry.offset));
            }
        }

        // Read all pending regions in parallel (cold start can be hundreds of MB),
        // then merge sequentially so dedupe stays exact.
        let results = read_parallel(&jobs, &cutoff);
        for (key, consumed, last, adds) in results {
            if let Some(entry) = self.state.files.get_mut(&key) {
                entry.offset += consumed;
                if last.is_some() {
                    entry.last = last;
                }
            }
            apply_adds(&mut self.state, adds);
        }

        // Forget files that disappeared.
        self.state.files.retain(|k, _| present.contains(k));

        // Context from the newest file. If we skipped it (old file, fresh state)
        // read only its tail once.
        let mut context = None;
        if let Some((_, key)) = &newest {
            if let Some(fs) = self.state.files.get_mut(key) {
                if fs.last.is_none() {
                    fs.last = read_tail_last(Path::new(key));
                }
                context = fs.last.clone().map(|l| (l, key.clone()));
            }
        }

        // Zero-filled day series, oldest -> today.
        let mut days = Vec::with_capacity(HISTORY_DAYS);
        for i in 0..HISTORY_DAYS {
            let d = first_day.checked_add_days(Days::new(i as u64)).unwrap_or(first_day);
            let ds = day_string(d);
            let t = self.state.days.get(&ds).copied().unwrap_or_default();
            days.push(DayStat { date: ds, messages: t.messages, tokens: t.tokens });
        }
        let today_t = days.last().copied_totals();

        tracing::debug!(
            "scan: {} files, {} msgs / {} tokens today, {:?}",
            files.len(),
            today_t.messages,
            today_t.tokens,
            started.elapsed()
        );

        LocalStats {
            context_used: context.as_ref().map(|(l, _)| l.used),
            project: context.as_ref().and_then(|(l, key)| project_label(l.cwd.as_deref(), key)),
            model: context.as_ref().and_then(|(l, _)| l.model.clone()),
            session_file: context.as_ref().map(|(_, key)| key.clone()),
            today_messages: today_t.messages,
            today_tokens: today_t.tokens,
            days,
        }
    }
}

trait CopiedTotals {
    fn copied_totals(&self) -> DayTotals;
}
impl CopiedTotals for Option<&DayStat> {
    fn copied_totals(&self) -> DayTotals {
        match self {
            Some(d) => DayTotals { messages: d.messages, tokens: d.tokens },
            None => DayTotals::default(),
        }
    }
}

/// (day, dedupe key, tokens) for one assistant line inside the window.
type Add = (String, String, u64);

/// Apply collected lines to the state, deduping on message id + request id.
fn apply_adds(st: &mut ScanState, adds: Vec<Add>) {
    for (day, key, used) in adds {
        if st.seen.entry(day.clone()).or_default().insert(key) {
            let t = st.days.entry(day).or_default();
            t.messages += 1;
            t.tokens += used;
        }
    }
}

/// Run `read_region` for every job on a small thread pool.
fn read_parallel(jobs: &[(String, PathBuf, u64)], cutoff: &str) -> Vec<(String, u64, Option<LastUsage>, Vec<Add>)> {
    if jobs.is_empty() {
        return Vec::new();
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 8).min(jobs.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::with_capacity(jobs.len()));
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some((key, path, offset)) = jobs.get(i) else { break };
                let (consumed, last, adds) = read_region(path, *offset, cutoff);
                if let Ok(mut o) = out.lock() {
                    o.push((key.clone(), consumed, last, adds));
                }
            });
        }
    });
    out.into_inner().unwrap_or_default()
}

/// Read `[offset, EOF)` line by line, collecting assistant lines for days
/// `>= cutoff`. Returns (bytes consumed, last assistant usage, lines). A
/// trailing line without `\n` is not consumed.
fn read_region(path: &Path, offset: u64, cutoff: &str) -> (u64, Option<LastUsage>, Vec<Add>) {
    let Ok(mut f) = std::fs::File::open(path) else { return (0, None, Vec::new()) };
    if f.seek(SeekFrom::Start(offset)).is_err() {
        return (0, None, Vec::new());
    }
    let mut reader = BufReader::with_capacity(256 * 1024, f);
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut consumed: u64 = 0;
    let mut last: Option<LastUsage> = None;
    let mut adds: Vec<Add> = Vec::new();

    loop {
        buf.clear();
        let n = match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        if buf.last() != Some(&b'\n') {
            // Partial line still being written; re-read next time.
            break;
        }
        consumed += n as u64;
        let Some(a) = parse_assistant(&buf) else { continue };
        if !a.sidechain {
            last = Some(LastUsage {
                used: a.used,
                model: a.model.clone(),
                cwd: a.cwd.clone(),
                timestamp: a.timestamp.clone(),
            });
        }
        if let Some(day) = a.day {
            if day.as_str() >= cutoff {
                adds.push((day, a.key, a.used));
            }
        }
    }
    (consumed, last, adds)
}

/// Last non-sidechain assistant line within the final TAIL_BYTES of a file.
fn read_tail_last(path: &Path) -> Option<LastUsage> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut data = Vec::with_capacity((len - start) as usize);
    f.read_to_end(&mut data).ok()?;
    let mut last = None;
    for line in data.split(|b| *b == b'\n') {
        if let Some(a) = parse_assistant(line) {
            if !a.sidechain {
                last = Some(LastUsage { used: a.used, model: a.model, cwd: a.cwd, timestamp: a.timestamp });
            }
        }
    }
    last
}

/// "usegebar" from `C:\Projelerim\usegebar`, falling back to the encoded dir name.
fn project_label(cwd: Option<&str>, file_key: &str) -> Option<String> {
    if let Some(cwd) = cwd {
        let trimmed = cwd.trim_end_matches(['\\', '/']);
        if let Some(name) = trimmed.rsplit(['\\', '/']).next() {
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    Path::new(file_key)
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
}

// -------------------------------------------------------------- watcher

pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    std::thread::Builder::new()
        .name("transcripts".into())
        .spawn(move || run(app))
        .expect("spawn transcripts thread");
}

fn run<R: Runtime>(app: AppHandle<R>) {
    let mut scanner = Scanner::new();
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher: Option<notify::RecommendedWatcher> = None;

    apply(&app, scanner.scan());

    loop {
        // (Re)attach the watcher when the projects dir exists.
        if watcher.is_none() {
            if let Some(root) = projects_dir().filter(|p| p.is_dir()) {
                let tx = tx.clone();
                match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                    if res.is_ok() {
                        let _ = tx.send(());
                    }
                }) {
                    Ok(mut w) => match w.watch(&root, RecursiveMode::Recursive) {
                        Ok(()) => {
                            tracing::info!("watching {}", root.display());
                            watcher = Some(w);
                        }
                        Err(e) => tracing::warn!("watch failed: {e}"),
                    },
                    Err(e) => tracing::warn!("watcher init failed: {e}"),
                }
            }
        }

        match rx.recv_timeout(TICK) {
            Ok(()) => {
                // Debounce: swallow the burst, then scan once.
                loop {
                    match rx.recv_timeout(DEBOUNCE) {
                        Ok(()) => continue,
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        apply(&app, scanner.scan());
    }
}

fn apply<R: Runtime>(app: &AppHandle<R>, stats: LocalStats) {
    if app.state::<AppState>().set_local(stats) {
        crate::state::publish(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn line(id: &str, req: &str, ts: &str, used: (u64, u64, u64, u64), side: bool) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","requestId":"{req}","cwd":"C:\\Projelerim\\demo","isSidechain":{side},"message":{{"id":"{id}","model":"claude-x","usage":{{"input_tokens":{},"output_tokens":{},"cache_creation_input_tokens":{},"cache_read_input_tokens":{}}}}}}}"#,
            used.0, used.1, used.2, used.3
        )
    }

    fn ts_days_ago(n: u64) -> String {
        (Local::now() - chrono::Duration::days(n as i64)).to_rfc3339()
    }

    fn fresh() -> ScanState {
        ScanState { version: STATE_VERSION, ..Default::default() }
    }

    #[test]
    fn parses_assistant_and_rejects_others() {
        let l = line("m1", "r1", "2026-09-22T12:40:11.123Z", (12, 840, 18234, 120480), false);
        let a = parse_assistant(l.as_bytes()).unwrap();
        assert_eq!(a.used, 12 + 840 + 18234 + 120480);
        assert_eq!(a.key, "m1|r1");
        assert_eq!(a.cwd.as_deref(), Some("C:\\Projelerim\\demo"));
        assert!(parse_assistant(br#"{"type":"user","message":{"role":"user"}}"#).is_none());
        assert!(parse_assistant(b"{broken").is_none());
        assert!(parse_assistant(b"").is_none());
    }

    #[test]
    fn incremental_read_dedupes_and_skips_partial() {
        let dir = std::env::temp_dir().join(format!("usagetray-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("s.jsonl");
        let ts = ts_days_ago(0);
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(f, "{}", line("m1", "r1", &ts, (1, 1, 0, 0), false)).unwrap();
        writeln!(f, "{}", line("m1", "r1", &ts, (1, 1, 0, 0), false)).unwrap(); // dup
        writeln!(f, r#"{{"type":"user","x":1}}"#).unwrap();
        write!(f, "{}", line("m2", "r2", &ts, (5, 5, 0, 0), false)).unwrap(); // no newline
        drop(f);

        let today = day_string(Local::now().date_naive());
        let mut st = fresh();
        let (consumed, last, adds) = read_region(&file, 0, "0000-00-00");
        apply_adds(&mut st, adds);
        assert_eq!(st.days[&today], DayTotals { messages: 1, tokens: 2 });
        assert_eq!(last.unwrap().used, 2);
        let len = std::fs::metadata(&file).unwrap().len();
        assert!(consumed < len);

        // Finish the line; only the remainder is read.
        let mut f = std::fs::OpenOptions::new().append(true).open(&file).unwrap();
        writeln!(f).unwrap();
        drop(f);
        let (consumed2, last2, adds2) = read_region(&file, consumed, "0000-00-00");
        apply_adds(&mut st, adds2);
        assert_eq!(consumed + consumed2, std::fs::metadata(&file).unwrap().len());
        assert_eq!(st.days[&today], DayTotals { messages: 2, tokens: 12 });
        assert_eq!(last2.unwrap().used, 10);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_root_builds_week_and_handles_truncation() {
        let dir = std::env::temp_dir().join(format!("usagetray-scan-{}", std::process::id()));
        let proj = dir.join("C--Projelerim-demo");
        let _ = std::fs::create_dir_all(&proj);
        let file = proj.join("a.jsonl");
        let today = Local::now().date_naive();
        std::fs::write(
            &file,
            format!(
                "{}\n{}\n{}\n{}\n",
                line("m4", "r4", &ts_days_ago(30), (999, 0, 0, 0), false), // outside window
                line("m3", "r3", &ts_days_ago(2), (7, 0, 0, 0), false),
                line("m2", "r2", &ts_days_ago(0), (20, 0, 0, 0), true),
                line("m1", "r1", &ts_days_ago(0), (10, 0, 0, 0), false),
            ),
        )
        .unwrap();

        let mut sc = Scanner::with_state(fresh());
        let s = sc.scan_root(&dir, today);
        assert_eq!(s.today_messages, 2);
        assert_eq!(s.today_tokens, 30);
        assert_eq!(s.days.len(), HISTORY_DAYS);
        assert_eq!(s.days[HISTORY_DAYS - 1].tokens, 30);
        assert_eq!(s.days[HISTORY_DAYS - 3].tokens, 7);
        assert_eq!(s.days.iter().map(|d| d.tokens).sum::<u64>(), 37);
        // sidechain line does not define context
        assert_eq!(s.context_used, Some(10));
        assert_eq!(s.project.as_deref(), Some("demo"));

        // Truncate + rewrite: offset resets, totals keep deduped keys.
        std::fs::write(&file, format!("{}\n", line("m5", "r5", &ts_days_ago(0), (7, 0, 0, 0), false))).unwrap();
        let s = sc.scan_root(&dir, today);
        assert_eq!(s.today_messages, 3);
        assert_eq!(s.context_used, Some(7));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_state_version_is_discarded() {
        let st: ScanState = serde_json::from_str(r#"{"day":"2026-01-01","files":{}}"#).unwrap();
        assert_ne!(st.version, STATE_VERSION);
    }

    #[test]
    fn label_falls_back_to_dir_name() {
        assert_eq!(project_label(None, "C:\\x\\C--Projelerim-demo\\s.jsonl").as_deref(), Some("C--Projelerim-demo"));
        assert_eq!(project_label(Some("/home/u/proj/"), "").as_deref(), Some("proj"));
    }
}
