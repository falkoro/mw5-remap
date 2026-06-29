//! Visual building blocks for the binding grid: the per-device colour scheme,
//! token prettifiers, and the single-row renderer. Kept out of `mod.rs` so the
//! app shell stays small. Everything here is `pub(super)` for the app module.

use crate::games::{Action, Binding, GameProvider, Kind};
use crate::input;
use crate::vjoy_map::VjoyMap;
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

/// A row of "Live: [chip] [chip] …" device toggles: click a stick's chip to soft-mute it
/// from the LIVE display (green glow + Detected readout) so you can test one stick at a
/// time. Muted chips render dimmed + struck-through. UI-only — never touches HidHide.
pub(super) fn mute_chips(ui: &mut egui::Ui, devices: &[input::Device], muted: &mut HashSet<(u16, u16)>) {
    if devices.is_empty() { return; }
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Live:").color(TEXT_MAIN))
            .on_hover_text("Mute a stick from this app's live glow + Detected readout (display only).");
        for d in devices {
            let id = (d.vid, d.pid);
            let is_muted = muted.contains(&id);
            let mut rt = egui::RichText::new(&d.name).size(12.0);
            rt = if is_muted { rt.strikethrough().color(UNBOUND_COL) } else { rt.color(TEXT_MAIN) };
            if ui.selectable_label(!is_muted, rt)
                .on_hover_text("Click to mute/unmute this stick in the live display.")
                .clicked()
            {
                if is_muted { muted.remove(&id); } else { muted.insert(id); }
            }
        }
    });
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
    vjoy_map: &VjoyMap,
) {
    let capturing = capture.as_ref().map(|c| c.row == i).unwrap_or(false);
    let token = rows[i].token.clone();
    let live = !token.is_empty() && hot.iter().any(|h| h == &token);

    // action label — green while its control is live, amber while (re)binding
    let lbl_col = if capturing { CAP_TXT } else if live { LIVE_TXT } else { TEXT_MAIN };
    ui.colored_label(lbl_col, &actions[i].label);

    // The chip: a polished, rounded, role-coloured button. Colour identifies the device;
    // bound / unbound / live / capturing each get a distinct fill + border; click = re-bind.
    let empty = token.is_empty() && !capturing;
    let (text, base) = if capturing {
        ("press a control…".to_string(), CAPTURING)
    } else if live {
        (pretty_token(&token), LIVE)
    } else if token.is_empty() {
        ("＋ bind".to_string(), UNBOUND_COL)
    } else {
        (pretty_token(&token), device_color(&token))
    };
    // Unbound chips read as a quiet empty "slot"; bound chips are richly filled; a LIVE
    // chip switches to a vivid, unmistakable green so an active control jumps out instantly.
    let fill = if empty { egui::Color32::from_rgb(44, 48, 60) }
               else if live { egui::Color32::from_rgb(46, 232, 120) }
               else { base };
    let txt_col = if empty { egui::Color32::from_rgb(150, 158, 175) } else { egui::Color32::from_rgb(12, 24, 16) };
    // Lighter, role-tinted border lifts the chip off the row; LIVE gets a thick bright rim
    // (plus a blooming glow painted in chip_polish) so it can't be missed.
    let stroke = if live { egui::Stroke::new(3.0, egui::Color32::from_rgb(205, 255, 220)) }
                 else if empty { egui::Stroke::new(1.0, egui::Color32::from_rgb(92, 100, 118)) }
                 else { egui::Stroke::new(1.5, lighten(base, 0.35)) };
    let chip = egui::Button::new(egui::RichText::new(text).color(txt_col).strong().size(14.0))
        .fill(fill)
        .stroke(stroke)
        .rounding(egui::Rounding::same(9.0))
        .min_size(egui::vec2(158.0, 30.0));
    // chip + a dim "which joystick" hint share one grid cell (keeps the 4-column layout).
    let clicked = ui.horizontal(|ui| {
        let resp = ui.add(chip).on_hover_text("Click, then press the control / move the axis. Esc cancels.");
        chip_polish(ui, &resp, fill, live, empty); // faux-gradient sheen + hover brighten
        if !token.is_empty() && !capturing {
            if let Some(dev) = crate::visual::token_device(&token, vjoy_map, devices) {
                ui.label(egui::RichText::new(format!("· {dev}")).size(10.5).color(egui::Color32::from_rgb(120, 128, 145)))
                    .on_hover_text("Which physical joystick feeds this binding.");
            }
        }
        resp.clicked()
    }).inner;
    if clicked {
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

/// Lighten a colour toward white by `t` (0..1) — for chip borders / the gloss sheen.
pub(super) fn lighten(c: egui::Color32, t: f32) -> egui::Color32 {
    let mix = |a: u8| (a as f32 + (255.0 - a as f32) * t) as u8;
    egui::Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

/// Paint a faint top-edge bevel (a cheap faux-gradient sheen) and a hover brighten over a
/// chip that's already been drawn — gives the flat egui button a richer, raised feel
/// without a real gradient mesh. No-op styling only; never affects click/capture.
fn chip_polish(ui: &egui::Ui, resp: &egui::Response, fill: egui::Color32, live: bool, empty: bool) {
    let r = resp.rect;
    // LIVE: bloom a few concentric translucent-green rings OUTSIDE the chip so an active
    // control reads as a glowing button at a glance (painter_at would clip these, so use
    // the unclipped ui painter for the halo).
    if live {
        let glow = ui.painter();
        for &(grow, alpha) in &[(2.0f32, 70u8), (5.0, 40), (8.0, 20), (11.0, 9)] {
            glow.rect_stroke(
                r.expand(grow),
                egui::Rounding::same(9.0 + grow),
                egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(130, 255, 175, alpha)),
            );
        }
    }
    let p = ui.painter_at(r);
    if !empty && r.width() > 18.0 {
        let hl = lighten(fill, if live { 0.5 } else { 0.32 }).linear_multiply(0.7);
        p.hline(egui::Rangef::new(r.min.x + 7.0, r.max.x - 7.0), r.min.y + 2.0, egui::Stroke::new(1.5, hl));
    }
    if resp.hovered() {
        p.rect_filled(r, egui::Rounding::same(9.0), egui::Color32::from_rgba_unmultiplied(255, 255, 255, 16));
    }
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
