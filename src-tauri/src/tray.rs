//! Dynamic tray icon (ring rendered with tiny-skia), tooltip, context menu
//! and popup placement.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;
use tiny_skia::{Color, LineCap, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Stroke, Transform};

use crate::state::{AppState, Snapshot, Status};
use crate::util;

pub const TRAY_ID: &str = "main";
const ICON_SIZE: u32 = 32;
const RING_WIDTH: f32 = 4.0;
const FONT_BYTES: &[u8] = include_bytes!("../assets/Inter-SemiBold.ttf");

/// Gap between popup and taskbar / screen edge, in logical px.
const POPUP_MARGIN: f64 = 12.0;

/// A tray click arriving this soon after a focus-loss hide is the click that
/// *caused* the hide; ignore it instead of re-opening the popup.
const REOPEN_GUARD: Duration = Duration::from_millis(250);

/// A programmatic show (first run, `--show`, tray menu) may not be allowed to
/// take the foreground; ignore focus-loss for this long after showing so the
/// popup does not vanish before the user sees it.
const SHOW_GRACE: Duration = Duration::from_millis(1500);

// ---------------------------------------------------------------- colors

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Green,
    Yellow,
    Orange,
    Red,
    Gray,
}

impl Tone {
    pub fn for_utilization(pct: f64) -> Tone {
        if pct < 50.0 {
            Tone::Green
        } else if pct < 75.0 {
            Tone::Yellow
        } else if pct < 90.0 {
            Tone::Orange
        } else {
            Tone::Red
        }
    }
    fn rgb(self) -> (u8, u8, u8) {
        match self {
            Tone::Green => (0x4A, 0xDE, 0x80),
            Tone::Yellow => (0xFA, 0xCC, 0x15),
            Tone::Orange => (0xFB, 0x92, 0x3C),
            Tone::Red => (0xF8, 0x71, 0x71),
            Tone::Gray => (0x94, 0xA3, 0xB8),
        }
    }
}

/// Everything that influences the rendered bitmap. Compared before
/// re-rendering so `set_icon` only runs when something visible changed.
#[derive(Clone, PartialEq, Debug)]
pub struct IconSpec {
    pub tone: Tone,
    /// 0..=1 filled fraction. `None` draws the error glyph.
    pub fraction: Option<f32>,
    pub percent_text: Option<String>,
}

impl IconSpec {
    pub fn from_snapshot(snap: &Snapshot, show_percent_text: bool) -> IconSpec {
        let usable = matches!(snap.status, Status::Ok | Status::RateLimited | Status::Offline)
            || (snap.status == Status::TokenExpired && snap.five_hour.is_some());
        match (&snap.five_hour, usable) {
            (Some(w), true) => {
                let pct = w.utilization.clamp(0.0, 100.0);
                let text = show_percent_text.then(|| {
                    let p = pct.round() as u32;
                    if p >= 100 {
                        "99+".to_string()
                    } else {
                        p.to_string()
                    }
                });
                IconSpec {
                    tone: if snap.status == Status::TokenExpired {
                        Tone::Gray
                    } else {
                        Tone::for_utilization(pct)
                    },
                    fraction: Some((pct / 100.0) as f32),
                    percent_text: text,
                }
            }
            _ => IconSpec { tone: Tone::Gray, fraction: None, percent_text: None },
        }
    }
}

// ------------------------------------------------------------- rendering

fn arc_path(cx: f32, cy: f32, r: f32, start_deg: f32, sweep_deg: f32) -> Option<tiny_skia::Path> {
    if sweep_deg <= 0.0 {
        return None;
    }
    let steps = ((sweep_deg / 3.0).ceil() as usize).max(2);
    let mut pb = PathBuilder::new();
    for i in 0..=steps {
        let a = (start_deg + sweep_deg * (i as f32 / steps as f32)).to_radians();
        let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.finish()
}

/// Render the tray bitmap as straight (non-premultiplied) RGBA.
pub fn render(spec: &IconSpec) -> Vec<u8> {
    let size = ICON_SIZE as f32;
    let mut pixmap = Pixmap::new(ICON_SIZE, ICON_SIZE).expect("pixmap");
    let (r, g, b) = spec.tone.rgb();
    let center = size / 2.0;
    let radius = center - RING_WIDTH / 2.0 - 1.0;

    let stroke = Stroke { width: RING_WIDTH, line_cap: LineCap::Butt, ..Default::default() };
    let mut paint = Paint::default();
    paint.anti_alias = true;

    // Track (25% opacity full ring).
    paint.set_color(Color::from_rgba8(r, g, b, 64));
    if let Some(track) = PathBuilder::from_circle(center, center, radius) {
        pixmap.stroke_path(&track, &paint, &stroke, Transform::identity(), None);
    }

    match spec.fraction {
        Some(f) => {
            // Filled arc: 12 o'clock, clockwise.
            let sweep = (f.clamp(0.0, 1.0)) * 360.0;
            paint.set_color(Color::from_rgba8(r, g, b, 255));
            if sweep >= 359.5 {
                if let Some(full) = PathBuilder::from_circle(center, center, radius) {
                    pixmap.stroke_path(&full, &paint, &stroke, Transform::identity(), None);
                }
            } else if let Some(arc) = arc_path(center, center, radius, -90.0, sweep) {
                pixmap.stroke_path(&arc, &paint, &stroke, Transform::identity(), None);
            }
            if let Some(text) = &spec.percent_text {
                draw_text(&mut pixmap, text, (r, g, b));
            }
        }
        None => {
            // Exclamation mark in the middle.
            paint.set_color(Color::from_rgba8(r, g, b, 255));
            let bar = tiny_skia::Rect::from_xywh(center - 1.75, center - 8.0, 3.5, 10.0).unwrap();
            pixmap.fill_rect(bar, &paint, Transform::identity(), None);
            if let Some(dot) = PathBuilder::from_circle(center, center + 6.0, 2.0) {
                pixmap.fill_path(&dot, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }
        }
    }

    // Demultiply for the tray API.
    let mut out = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

fn draw_text(pixmap: &mut Pixmap, text: &str, (r, g, b): (u8, u8, u8)) {
    let Ok(font) = FontRef::try_from_slice(FONT_BYTES) else { return };
    let px = if text.len() >= 3 { 12.0 } else { 15.0 };
    let scale = PxScale::from(px);
    let scaled = font.as_scaled(scale);

    let glyphs: Vec<_> = text.chars().map(|c| scaled.scaled_glyph(c)).collect();
    let width: f32 = glyphs.iter().map(|gl| scaled.h_advance(gl.id)).sum();
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let size = ICON_SIZE as f32;
    let mut x = (size - width) / 2.0;
    // Vertically center cap-height-ish text.
    let y = size / 2.0 + (ascent + descent) / 2.0 - descent * 0.35;

    let w = ICON_SIZE as i32;
    let h = ICON_SIZE as i32;
    for mut gl in glyphs {
        gl.position = ab_glyph::point(x, y);
        let adv = scaled.h_advance(gl.id);
        if let Some(outlined) = font.outline_glyph(gl) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, cov| {
                let px_x = bounds.min.x as i32 + gx as i32;
                let px_y = bounds.min.y as i32 + gy as i32;
                if px_x < 0 || px_y < 0 || px_x >= w || px_y >= h {
                    return;
                }
                let a = (cov * 255.0).round().clamp(0.0, 255.0) as u8;
                if a == 0 {
                    return;
                }
                let idx = (px_y * w + px_x) as usize;
                let dst = pixmap.pixels_mut()[idx];
                let src_a = a as u32;
                let inv = 255 - src_a;
                let blend = |s: u8, d: u8| ((s as u32 * src_a + d as u32 * inv) / 255) as u8;
                // Source is premultiplied by src_a already (s*a).
                let out_r = ((r as u32 * src_a) / 255) as u8 + ((dst.red() as u32 * inv) / 255) as u8;
                let out_g = ((g as u32 * src_a) / 255) as u8 + ((dst.green() as u32 * inv) / 255) as u8;
                let out_b = ((b as u32 * src_a) / 255) as u8 + ((dst.blue() as u32 * inv) / 255) as u8;
                let out_a = blend(255, dst.alpha());
                if let Some(c) = PremultipliedColorU8::from_rgba(out_r, out_g, out_b, out_a) {
                    pixmap.pixels_mut()[idx] = c;
                }
            });
        }
        x += adv;
    }
}

// ------------------------------------------------------------- tray setup

pub struct TrayState {
    last_spec: Mutex<Option<IconSpec>>,
    last_tooltip: Mutex<String>,
    last_hidden: Mutex<Option<Instant>>,
    last_shown: Mutex<Option<Instant>>,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            last_spec: Mutex::new(None),
            last_tooltip: Mutex::new(String::new()),
            last_hidden: Mutex::new(None),
            last_shown: Mutex::new(None),
        }
    }
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let refresh = MenuItem::with_id(app, "refresh", "Yenile", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Ayarlar", true, None::<&str>)?;
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart =
        CheckMenuItem::with_id(app, "autostart", "Başlangıçta çalıştır", true, autostart_on, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Çıkış", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&refresh, &settings, &autostart, &PredefinedMenuItem::separator(app)?, &quit],
    )?;

    let initial = IconSpec { tone: Tone::Gray, fraction: None, percent_text: None };
    let rgba = render(&initial);

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::new_owned(rgba, ICON_SIZE, ICON_SIZE))
        .tooltip("UsageTray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "refresh" => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.request_refresh();
                }
            }
            "settings" => {
                show_popup(app);
                let _ = app.emit("navigate", "settings");
            }
            "autostart" => {
                let enabled = autostart.is_checked().unwrap_or(false);
                let res = if enabled { app.autolaunch().enable() } else { app.autolaunch().disable() };
                if let Err(e) = res {
                    tracing::warn!("autostart toggle failed: {e}");
                }
                if let Some(state) = app.try_state::<AppState>() {
                    state.set_start_with_windows(enabled);
                }
            }
            "quit" => {
                tracing::info!("quit from tray menu");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popup(tray.app_handle());
            }
        })
        .build(app)?;

    if let Some(mut lock) = app.state::<TrayState>().last_spec.lock().ok() {
        *lock = Some(initial);
    }
    Ok(())
}

/// Push a new snapshot to the tray: icon (only if changed) and tooltip.
pub fn apply<R: Runtime>(app: &AppHandle<R>, snap: &Snapshot, show_percent_text: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let tray_state = app.state::<TrayState>();

    let spec = IconSpec::from_snapshot(snap, show_percent_text);
    let changed = tray_state.last_spec.lock().map(|l| l.as_ref() != Some(&spec)).unwrap_or(true);
    if changed {
        let rgba = render(&spec);
        if let Err(e) = tray.set_icon(Some(Image::new_owned(rgba, ICON_SIZE, ICON_SIZE))) {
            tracing::warn!("set_icon failed: {e}");
        }
        if let Ok(mut l) = tray_state.last_spec.lock() {
            *l = Some(spec);
        }
    }

    let tooltip = tooltip_for(snap);
    let tooltip_changed = tray_state.last_tooltip.lock().map(|l| *l != tooltip).unwrap_or(true);
    if tooltip_changed {
        let _ = tray.set_tooltip(Some(&tooltip));
        if let Ok(mut l) = tray_state.last_tooltip.lock() {
            *l = tooltip;
        }
    }
}

pub fn tooltip_for(snap: &Snapshot) -> String {
    let now = chrono::Utc::now();
    match snap.status {
        Status::NoCredentials => return "UsageTray — Claude Code bulunamadı".into(),
        Status::TokenExpired if snap.five_hour.is_none() => {
            return "UsageTray — token süresi dolmuş".into()
        }
        Status::Error if snap.five_hour.is_none() => return "UsageTray — veri alınamadı".into(),
        _ => {}
    }
    let mut lines = Vec::new();
    if let Some(w) = &snap.five_hour {
        let mut s = format!("5s: {}", util::percent_label(w.utilization));
        if let Some(at) = w.resets_at_utc() {
            s.push_str(&format!(" · {} sonra sıfırlanır", util::remaining_tr(at, now)));
        }
        lines.push(s);
    }
    if let Some(w) = &snap.seven_day {
        lines.push(format!("Haftalık: {}", util::percent_label(w.utilization)));
    }
    if snap.stale {
        lines.push("(bayat veri)".into());
    }
    if lines.is_empty() {
        "UsageTray".into()
    } else {
        lines.join("\n")
    }
}

// ------------------------------------------------------------------ popup

pub fn main_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window("main")
}

pub fn toggle_popup<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = main_window(app) else { return };
    let recently_hidden = app
        .state::<TrayState>()
        .last_hidden
        .lock()
        .ok()
        .and_then(|l| *l)
        .is_some_and(|t| t.elapsed() < REOPEN_GUARD);
    if win.is_visible().unwrap_or(false) {
        hide_popup(app);
    } else if !recently_hidden {
        show_popup(app);
    }
}

pub fn hide_popup<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = main_window(app) {
        let _ = win.hide();
    }
    if let Ok(mut l) = app.state::<TrayState>().last_hidden.lock() {
        *l = Some(Instant::now());
    }
}

/// True while a just-shown popup should survive a focus loss.
pub fn in_show_grace<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.state::<TrayState>()
        .last_shown
        .lock()
        .ok()
        .and_then(|l| *l)
        .is_some_and(|t| t.elapsed() < SHOW_GRACE)
}

/// Called from the window focus-loss handler.
pub fn note_hidden<R: Runtime>(app: &AppHandle<R>) {
    if let Ok(mut l) = app.state::<TrayState>().last_hidden.lock() {
        *l = Some(Instant::now());
    }
}

pub fn show_popup<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = main_window(app) else {
        tracing::warn!("show_popup: no main window");
        return;
    };
    position_popup(&win);
    if let Ok(mut l) = app.state::<TrayState>().last_shown.lock() {
        *l = Some(Instant::now());
    }
    if let Err(e) = win.show() {
        tracing::warn!("show failed: {e}");
    }
    if let Err(e) = win.set_focus() {
        tracing::warn!("set_focus failed: {e}");
    }
    tracing::debug!(
        "popup shown at {:?} visible={:?}",
        win.outer_position().ok(),
        win.is_visible().ok()
    );
    let _ = app.emit("popup-shown", ());
}

/// Bottom-right of the work area of the monitor under the cursor,
/// POPUP_MARGIN above the taskbar and from the right edge.
fn position_popup<R: Runtime>(win: &WebviewWindow<R>) {
    let cursor = win.cursor_position().ok();
    let monitor = cursor
        .and_then(|c| win.monitor_from_point(c.x, c.y).ok().flatten())
        .or_else(|| win.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        tracing::warn!("position_popup: no monitor found");
        return;
    };
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let size = match win.outer_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let margin = (POPUP_MARGIN * scale).round() as i32;
    let x = work.position.x + work.size.width as i32 - size.width as i32 - margin;
    let y = work.position.y + work.size.height as i32 - size.height as i32 - margin;
    let x = x.max(work.position.x);
    let y = y.max(work.position.y);
    tracing::debug!("position_popup: work_area={:?} size={:?} -> ({x},{y})", work, size);
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_thresholds() {
        assert_eq!(Tone::for_utilization(0.0), Tone::Green);
        assert_eq!(Tone::for_utilization(49.9), Tone::Green);
        assert_eq!(Tone::for_utilization(50.0), Tone::Yellow);
        assert_eq!(Tone::for_utilization(74.9), Tone::Yellow);
        assert_eq!(Tone::for_utilization(75.0), Tone::Orange);
        assert_eq!(Tone::for_utilization(89.9), Tone::Orange);
        assert_eq!(Tone::for_utilization(90.0), Tone::Red);
    }

    #[test]
    fn render_produces_rgba_of_right_size() {
        let px = render(&IconSpec { tone: Tone::Green, fraction: Some(0.33), percent_text: None });
        assert_eq!(px.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        // Something non-transparent got drawn.
        assert!(px.chunks(4).any(|c| c[3] > 200));
        let err = render(&IconSpec { tone: Tone::Gray, fraction: None, percent_text: None });
        assert!(err.chunks(4).any(|c| c[3] > 200));
        let txt = render(&IconSpec { tone: Tone::Red, fraction: Some(1.0), percent_text: Some("99+".into()) });
        assert_eq!(txt.len(), px.len());
    }

    #[test]
    fn arc_is_empty_for_zero_sweep() {
        assert!(arc_path(16.0, 16.0, 10.0, -90.0, 0.0).is_none());
        assert!(arc_path(16.0, 16.0, 10.0, -90.0, 90.0).is_some());
    }
}
