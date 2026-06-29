//! Visual building blocks for the binding grid: the per-device colour scheme,
//! token prettifiers, and the single-row renderer. Kept out of `mod.rs` so the
//! app shell stays small. Everything here is `pub(super)` for the app module.

use super::theme;
use crate::games::{Action, Binding, GameProvider, Kind};
use crate::input;
use crate::vjoy_map::VjoyMap;
use eframe::egui;
use std::collections::{HashMap, HashSet};

// Fixed grid-cell widths so every binding row lines up DEAD STRAIGHT down the column,
// whatever a given row contains: the action label, the chip, the "· device" hint, the
// clear ×, and the invert/scale controls each own a constant-width, vertically-centred
// cell (see `cell`). A row with no hint / a button row (no trim) keeps the same columns.
const W_LABEL: f32 = 152.0;
const W_CHIP: f32 = 154.0; // the chip is 150 wide (theme::chip min_size) + a little slack
const W_HINT: f32 = 76.0; // the dim "· device" hint sits in its OWN cell, never behind the chip
const W_CLEAR: f32 = 22.0;
const W_TRIM: f32 = 92.0; // Inv checkbox + scale DragValue (axis rows only)

/// A fixed-width cell, full row-height, with its content vertically CENTRED — the building
/// block that keeps the binding-grid columns aligned regardless of a row's contents.
fn cell<R>(ui: &mut egui::Ui, w: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        egui::vec2(w, theme::CHIP_H),
        egui::Layout::left_to_right(egui::Align::Center),
        add,
    )
    .inner
}

/// An in-progress "press a control to bind it" capture. Lives here because
/// `binding_row` both starts one and reads it; `mod.rs` resolves it each frame.
#[derive(Clone)]
pub(super) struct Capture {
    pub row: usize,
    pub kind: Kind,
    pub ignore: HashSet<String>,        // controls already held when capture began
    pub baseline: HashMap<u32, [u32; 8]>, // axis rest values per device id
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
    vjoy_map: &VjoyMap,
) {
    let capturing = capture.as_ref().map(|c| c.row == i).unwrap_or(false);
    let token = rows[i].token.clone();
    let live = !token.is_empty() && hot.iter().any(|h| h == &token);

    // action label — green while its control is live, amber while (re)binding. Truncates
    // inside its fixed cell (full name on hover) so a long label never wraps the row height.
    let lbl_col = if capturing { theme::CAP_DK } else if live { theme::ACCENT_DK } else { theme::TEXT };
    cell(ui, W_LABEL, |ui| {
        ui.add(egui::Label::new(egui::RichText::new(&actions[i].label).color(lbl_col)).truncate())
            .on_hover_text(actions[i].label.as_str());
    });

    // The chip: one clean, rounded, role-aware button (theme::chip). The state picks its
    // fill/border/text in one place; the device colour rides along in Bound; click = re-bind.
    let (state, text) = if capturing {
        (theme::ChipState::Capturing, "press a control…".to_string())
    } else if live {
        (theme::ChipState::Live, pretty_token(&token))
    } else if token.is_empty() {
        (theme::ChipState::Unbound, "+ bind".to_string())
    } else {
        (theme::ChipState::Bound(theme::device_color(&token)), pretty_token(&token))
    };
    let clicked = cell(ui, W_CHIP, |ui| {
        theme::chip(ui, &text, state)
            .on_hover_text("Click, then press the control / move the axis. Esc cancels.")
            .clicked()
    });

    // The dim "which joystick" hint gets its OWN cell to the RIGHT of the chip — never
    // behind it, never clipping the chip. Truncates long device names (full name on hover).
    cell(ui, W_HINT, |ui| {
        if !token.is_empty() && !capturing {
            if let Some(dev) = crate::visual::token_device(&token, vjoy_map, devices) {
                ui.add(
                    egui::Label::new(egui::RichText::new(format!("· {dev}")).size(10.5).color(theme::TEXT_FAINT))
                        .truncate(),
                )
                .on_hover_text(format!("Fed by {dev}"));
            }
        }
    });

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

    // clear button (only when bound) — a PAINTED, font-safe × (never a tofu glyph),
    // centred to the chip so the column stays aligned. Empty cell keeps the row height.
    cell(ui, W_CLEAR, |ui| {
        if !token.is_empty() && theme::clear_button(ui).on_hover_text("Clear this binding").clicked() {
            rows[i].token.clear();
        }
    });

    // invert + scale (axes only); an empty cell holds the column for button rows.
    cell(ui, W_TRIM, |ui| {
        ui.spacing_mut().item_spacing.x = 4.0; // keep "Inv" + the scale field inside the cell
        if actions[i].kind == Kind::Axis {
            let mut inv = rows[i].scale < 0.0;
            if ui.checkbox(&mut inv, "Inv").changed() {
                rows[i].scale = rows[i].scale.abs() * if inv { -1.0 } else { 1.0 };
            }
            let mut mag = rows[i].scale.abs();
            if ui.add(egui::DragValue::new(&mut mag).speed(0.1).range(0.1..=10.0).prefix("x")).changed() {
                let sign = if rows[i].scale < 0.0 { -1.0 } else { 1.0 };
                rows[i].scale = mag * sign;
            }
        }
    });
    ui.end_row();
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
