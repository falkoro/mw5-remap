//! Visual building blocks for the binding grid: the per-device colour scheme,
//! token prettifiers, and the single-row renderer. Kept out of `mod.rs` so the
//! app shell stays small. Everything here is `pub(super)` for the app module.

use crate::games::{Action, Binding, GameProvider, Kind};
use crate::input;
use eframe::egui;
use std::collections::{HashMap, HashSet};

/// An in-progress "press a control to bind it" capture. Lives here because
/// `binding_row` both starts one and reads it; `mod.rs` resolves it each frame.
#[derive(Clone)]
pub(super) struct Capture {
    pub row: usize,
    pub kind: Kind,
    pub ignore: HashSet<String>,        // controls already held when capture began
    pub baseline: HashMap<u32, [u32; 8]>, // axis rest values per device id
}

pub(super) const CAPTURING: egui::Color32 = egui::Color32::from_rgb(235, 170, 45); // orange: listening
pub(super) const LIVE: egui::Color32 = egui::Color32::from_rgb(70, 210, 110);      // green: control active
pub(super) const STICK_COL: egui::Color32 = egui::Color32::from_rgb(86, 156, 235); // Joystick-role device
pub(super) const THROTTLE_COL: egui::Color32 = egui::Color32::from_rgb(235, 150, 60); // Throttle-role device
pub(super) const UNBOUND_COL: egui::Color32 = egui::Color32::from_rgb(120, 128, 145);
pub(super) const TEXT_MAIN: egui::Color32 = egui::Color32::from_rgb(38, 42, 54);   // dark: readable on the light panel
pub(super) const LIVE_TXT: egui::Color32 = egui::Color32::from_rgb(20, 140, 72);   // green readable on light
pub(super) const CAP_TXT: egui::Color32 = egui::Color32::from_rgb(180, 110, 0);    // amber readable on light
// extra colours for games with many physical devices (AC7/SC: one per VID/PID).
pub(super) const DEV_PALETTE: [egui::Color32; 6] = [
    egui::Color32::from_rgb(86, 156, 235), egui::Color32::from_rgb(235, 150, 60),
    egui::Color32::from_rgb(120, 200, 120), egui::Color32::from_rgb(200, 120, 220),
    egui::Color32::from_rgb(230, 110, 110), egui::Color32::from_rgb(110, 205, 210),
];

/// The colour that identifies which physical device a token belongs to.
pub(super) fn device_color(token: &str) -> egui::Color32 {
    if token.starts_with("Throttle") {
        THROTTLE_COL
    } else if token.starts_with("Joystick") {
        STICK_COL
    } else if let Some((id, _)) = token.split_once('|') {
        // AC7/SC "VVVVPPPP|input": stable colour per device id.
        let h = id.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
        DEV_PALETTE[(h as usize) % DEV_PALETTE.len()]
    } else {
        UNBOUND_COL
    }
}

/// Friendly control name without the device/role prefix: "Joystick_Button1" ->
/// "Button 1", "Throttle_Axis2" -> "Axis 2", "Joystick_Hat_3" -> "Hat 3", and
/// "044F0402|Y:R" -> "Y:R". The device is shown by colour instead of the prefix.
pub(super) fn pretty_token(token: &str) -> String {
    if token.is_empty() { return "unbound".into(); }
    let body = token
        .strip_prefix("Joystick_")
        .or_else(|| token.strip_prefix("Throttle_"))
        .map(|s| s.replace('_', " "))
        .or_else(|| token.split_once('|').map(|(_, i)| i.to_string()))
        .unwrap_or_else(|| token.to_string());
    // insert a space before a trailing number run ("Button1" -> "Button 1").
    match body.find(|c: char| c.is_ascii_digit()) {
        Some(p) if p > 0 && body.as_bytes()[p - 1] != b' ' => format!("{} {}", &body[..p], &body[p..]),
        _ => body,
    }
}

/// One row of the Cockpit Bindings grid: action label, a colour-coded "chip"
/// showing the bound control (click it to re-bind), a clear button, and (for axes)
/// invert/scale. The chip turns green the instant its control is physically active.
#[allow(clippy::too_many_arguments)]
pub(super) fn binding_row(
    ui: &mut egui::Ui,
    i: usize,
    actions: &[Action],
    rows: &mut [Binding],
    capture: &mut Option<Capture>,
    devices: &[input::Device],
    p: &dyn GameProvider,
    status: &mut String,
    hot: &[String],
) {
    let capturing = capture.as_ref().map(|c| c.row == i).unwrap_or(false);
    let token = rows[i].token.clone();
    let live = !token.is_empty() && hot.iter().any(|h| h == &token);

    // action label — green while its control is live, amber while (re)binding
    let lbl_col = if capturing { CAP_TXT } else if live { LIVE_TXT } else { TEXT_MAIN };
    ui.colored_label(lbl_col, &actions[i].label);

    // the chip: a big colour-coded button. Colour = which device; click = re-bind.
    let (text, fill) = if capturing {
        ("press a control…".to_string(), CAPTURING)
    } else if live {
        (pretty_token(&token), LIVE)
    } else if token.is_empty() {
        ("＋ bind".to_string(), UNBOUND_COL)
    } else {
        (pretty_token(&token), device_color(&token))
    };
    let txt_col = if token.is_empty() && !capturing { egui::Color32::from_rgb(120, 128, 145) } else { egui::Color32::from_rgb(15, 18, 24) };
    // a rounded, slightly raised "chip" — nicer than a flat box
    let stroke = if live { egui::Stroke::new(2.0, egui::Color32::from_rgb(30, 120, 60)) }
                 else if token.is_empty() { egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 158, 175)) }
                 else { egui::Stroke::new(1.0, fill.linear_multiply(0.6)) };
    let chip = egui::Button::new(egui::RichText::new(text).color(txt_col).strong().size(14.0))
        .fill(fill)
        .stroke(stroke)
        .rounding(egui::Rounding::same(8.0))
        .min_size(egui::vec2(158.0, 30.0));
    if ui.add(chip).on_hover_text("Click, then press the control / move the axis. Esc cancels.").clicked() {
        if capturing {
            *capture = None;
        } else {
            let mut ignore = HashSet::new();
            for (di, dev) in devices.iter().enumerate() {
                for b in dev.pressed_buttons() { if let Some(t) = p.button_token(dev, b, di) { ignore.insert(t); } }
                if let Some(o) = dev.pov_octant() { if let Some(t) = p.pov_token(dev, o, di) { ignore.insert(t); } }
            }
            let baseline = devices.iter().map(|d| (d.id, d.axes)).collect();
            *capture = Some(Capture { row: i, kind: actions[i].kind, ignore, baseline });
            *status = format!("Listening… do the control for \"{}\" (Esc to cancel)", actions[i].label);
        }
    }

    // clear button (only when bound)
    if !token.is_empty() {
        if ui.small_button("✕").on_hover_text("Clear this binding").clicked() {
            rows[i].token.clear();
        }
    } else {
        ui.label("");
    }

    if actions[i].kind == Kind::Axis {
        ui.horizontal(|ui| {
            let mut inv = rows[i].scale < 0.0;
            if ui.checkbox(&mut inv, "Inv").changed() {
                rows[i].scale = rows[i].scale.abs() * if inv { -1.0 } else { 1.0 };
            }
            let mut mag = rows[i].scale.abs();
            if ui.add(egui::DragValue::new(&mut mag).speed(0.1).range(0.1..=10.0).prefix("x")).changed() {
                let sign = if rows[i].scale < 0.0 { -1.0 } else { 1.0 };
                rows[i].scale = mag * sign;
            }
        });
    } else {
        ui.label("");
    }
    ui.end_row();
}

pub(super) const AXIS_NAMES: [&str; 8] = ["X", "Y", "Z", "Rx", "Ry", "Rz", "Slider0", "Slider1"];
const HAT_ARROWS: [&str; 8] = ["↑", "↗", "→", "↘", "↓", "↙", "←", "↖"];

/// Live "what control is actuated right now" for the readout under the tab bar.
/// Mirrors `resolve_capture`'s detection order (buttons → hat → most-moved axis)
/// but device-agnostic (no game token). Axis motion is judged against the previous
/// frame (`prev`, keyed by VID/PID) instead of a capture baseline, so a stick
/// resting off-centre (e.g. toe pedals at 0) is NOT a false positive — only an
/// axis the user is actively moving shows. Returns the body that follows
/// "Detected: " (e.g. `MOZA AB6 — Button 5`), or None when nothing is actuated.
pub(super) fn detect_input(devices: &[input::Device], prev: &mut HashMap<(u16, u16), [u32; 8]>) -> Option<String> {
    let mut found: Option<String> = None;
    for d in devices {
        let last = prev.insert((d.vid, d.pid), d.axes); // always refresh, for every device
        if found.is_some() { continue; }
        if let Some(&b) = d.pressed_buttons().first() {
            found = Some(format!("{} — Button {}", d.name, b));
            continue;
        }
        if let Some(oct) = d.pov_octant() {
            found = Some(format!("{} — Hat {}", d.name, HAT_ARROWS[(oct as usize - 1) & 7]));
            continue;
        }
        if let Some(base) = last {
            let mut best = (0i64, 0usize);
            for ax in 0..8 {
                if !d.present[ax] { continue; }
                let delta = (d.axes[ax] as i64 - base[ax] as i64).abs();
                if delta > best.0 { best = (delta, ax); }
            }
            if best.0 > 3000 {
                found = Some(format!("{} — axis {}", d.name, AXIS_NAMES[best.1]));
            }
        }
    }
    found
}

/// Last path component of a config path, for friendly status messages.
pub(super) fn file_name(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.into())
}

/// Write the list of currently-hidden device paths so a crash can recover them.
pub(super) fn persist(path: &std::path::PathBuf, paths: &[String]) {
    if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
    let _ = std::fs::write(path, paths.join("\r\n"));
}
