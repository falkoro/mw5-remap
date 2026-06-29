//! Per-device callout-position overrides for the device panel, so a user can DRAG a
//! mis-placed dot and have it stick. Persisted (no serde) to
//! `%LOCALAPPDATA%\MW5-Remap\marker_layout.txt`, one override per line, tab-separated:
//!   `device-key<TAB>marker-id<TAB>x<TAB>y`   (x/y are 0..1 normalized image coords)
//! e.g. `stick\tTrigger\t0.4600\t0.4500`. The built-in default coords are used whenever
//! a marker has no saved override. This module also owns the in-memory edit-mode toggle
//! + override cache (thread-local — the egui panel only ever touches it from the UI
//! thread) and the drag interaction that edits it.

use super::{Marker, ACCENT, HOT};
use eframe::egui;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

/// (device-key, marker-id) -> normalized (x, y).
pub type Overrides = HashMap<(String, String), (f32, f32)>;

fn file_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(base).join("MW5-Remap").join("marker_layout.txt")
}

/// Serialise overrides to the tab-separated text format (sorted for stable output).
pub fn encode(ov: &Overrides) -> String {
    let mut keys: Vec<_> = ov.keys().collect();
    keys.sort();
    let mut s = String::new();
    for k in keys {
        let (x, y) = ov[k];
        s.push_str(&format!("{}\t{}\t{:.4}\t{:.4}\r\n", k.0, k.1, x, y));
    }
    s
}

/// Parse the tab-separated text format back into overrides (skips malformed lines,
/// clamps coords to 0..1). Marker-ids may contain spaces/unicode — only TAB splits.
pub fn parse(text: &str) -> Overrides {
    let mut ov = Overrides::new();
    for line in text.lines() {
        if line.trim().is_empty() { continue; }
        let mut it = line.split('\t');
        let dev = it.next().unwrap_or("").trim().to_string();
        let id = it.next().unwrap_or("").trim().to_string();
        let x = it.next().unwrap_or("").trim().parse::<f32>();
        let y = it.next().unwrap_or("").trim().parse::<f32>();
        if dev.is_empty() || id.is_empty() { continue; }
        if let (Ok(x), Ok(y)) = (x, y) {
            ov.insert((dev, id), (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
        }
    }
    ov
}

fn load_file() -> Overrides {
    std::fs::read_to_string(file_path()).map(|t| parse(&t)).unwrap_or_default()
}

fn save_file(ov: &Overrides) {
    let p = file_path();
    if let Some(dir) = p.parent() { let _ = std::fs::create_dir_all(dir); }
    let _ = std::fs::write(&p, encode(ov));
}

struct State { edit: bool, ov: Overrides }
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State { edit: false, ov: load_file() });
}

/// Is "Edit layout" mode on? (Drag repositions markers; clicks otherwise pass through.)
pub fn edit_enabled() -> bool { STATE.with(|s| s.borrow().edit) }
pub fn set_edit(on: bool) { STATE.with(|s| s.borrow_mut().edit = on); }

/// The saved override for a marker, or None to fall back to its built-in coord.
fn pos(device_key: &str, marker_id: &str) -> Option<(f32, f32)> {
    STATE.with(|s| s.borrow().ov.get(&(device_key.to_string(), marker_id.to_string())).copied())
}

/// Effective (overridden-or-default) normalized position for a marker.
pub(super) fn resolved_pos(device_key: &str, mk: &Marker) -> (f32, f32) {
    pos(device_key, mk.label).unwrap_or((mk.nx, mk.ny))
}

fn set(device_key: &str, marker_id: &str, p: (f32, f32)) {
    STATE.with(|s| { s.borrow_mut().ov.insert((device_key.to_string(), marker_id.to_string()), p); });
}

/// Persist the current overrides (call once a drag finishes).
fn flush() { STATE.with(|s| save_file(&s.borrow().ov)); }

/// Clear every override (back to the built-in coords) and persist.
pub fn reset_all() {
    STATE.with(|s| {
        s.borrow_mut().ov.clear();
        save_file(&s.borrow().ov);
    });
}

/// Edit-mode: turn each callout dot into a draggable handle (drawn as a white ring).
/// Dragging updates that marker's per-device override and saves it on release.
/// `markers` carry their already-resolved positions (so the handle sits on the dot).
pub(super) fn drag_markers(ui: &egui::Ui, painter: &egui::Painter, rect: egui::Rect, device_key: &str, markers: &[Marker]) {
    for (i, mk) in markers.iter().enumerate() {
        let dot = rect.min + egui::vec2(mk.nx * rect.width(), mk.ny * rect.height());
        let handle = egui::Rect::from_center_size(dot, egui::vec2(16.0, 16.0));
        let id = egui::Id::new(("marker_drag", device_key, i));
        let resp = ui.interact(handle, id, egui::Sense::drag());
        let lit = resp.hovered() || resp.dragged();
        painter.circle_stroke(dot, if lit { 10.0 } else { 8.0 },
            egui::Stroke::new(2.0, if lit { HOT } else { ACCENT }));
        painter.circle_stroke(dot, if lit { 10.0 } else { 8.0 },
            egui::Stroke::new(1.0, egui::Color32::WHITE));
        if resp.dragged() {
            let d = resp.drag_delta();
            let nx = (mk.nx + d.x / rect.width()).clamp(0.0, 1.0);
            let ny = (mk.ny + d.y / rect.height()).clamp(0.0, 1.0);
            set(device_key, mk.label, (nx, ny));
        }
        if resp.drag_stopped() { flush(); }
        let _ = resp.on_hover_cursor(egui::CursorIcon::Grab);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_overrides() {
        let mut ov = Overrides::new();
        ov.insert(("stick".into(), "Trigger".into()), (0.46, 0.45));
        // spaces + unicode in the marker-id must survive (it's the callout label)
        ov.insert(("pedals".into(), "Right toe → forward".into()), (0.6601, 0.40));
        let back = parse(&encode(&ov));
        assert_eq!(back.len(), 2);
        for (k, &(x, y)) in &ov {
            let &(bx, by) = back.get(k).expect("key round-tripped");
            assert!((x - bx).abs() < 1e-3 && (y - by).abs() < 1e-3, "coord mismatch for {k:?}");
        }
    }

    #[test]
    fn parse_skips_garbage_and_clamps() {
        let ov = parse("stick\tTrigger\t0.5\t0.5\nrubbish line\n\nbase\tPitch\t1.5\t-0.2\n");
        assert_eq!(ov.len(), 2);
        assert_eq!(ov[&("base".into(), "Pitch".into())], (1.0, 0.0)); // clamped to 0..1
    }
}
