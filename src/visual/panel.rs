//! Sidebar chrome that isn't a device image: the per-device "in view" hide toggles and
//! the collapsible raw-axis readout. Both reuse the shared live-mute set, so hiding a
//! controller you're not using (e.g. a button box) drops its image, axes AND live glow
//! from the Bind panel in one click — purely a display concern, never HidHide. Kept out
//! of `mod.rs` so the orchestrator stays within the size budget.

use crate::games::GameProvider;
use crate::input::Device;
use eframe::egui;
use std::collections::HashSet;

/// Per-device show/hide toggles. Click a controller's name to drop it from the panel
/// (its image + axes) and the live glow, or to bring it back. Reuses the shared `muted`
/// set. Shown devices read as lit (selected) pills; hidden ones are struck-through and
/// dimmed but stay listed, so a hidden device is always one click from returning.
pub(super) fn visibility(ui: &mut egui::Ui, devices: &[Device], muted: &mut HashSet<(u16, u16)>) {
    if devices.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("In view").strong().small()).on_hover_text(
            "Hide a controller you're not using — its image, axes and live glow drop out of this panel. Click a name to toggle.",
        );
        for d in devices {
            let id = (d.vid, d.pid);
            let shown = !muted.contains(&id);
            let rt = egui::RichText::new(&d.name).size(12.0);
            let rt = if shown { rt } else { rt.strikethrough().weak() };
            if ui
                .selectable_label(shown, rt)
                .on_hover_text(if shown { "Showing — click to hide this controller" } else { "Hidden — click to show this controller" })
                .clicked()
            {
                if shown {
                    muted.insert(id);
                } else {
                    muted.remove(&id);
                }
            }
        }
    });
}

/// Live raw-axis readout: one bar per detected axis per device showing the actual winmm
/// value (0..65535) and the token it binds to. Hidden (muted) devices are skipped. Always
/// shows the axes Windows actually reports so devices with gaps (the MRP uses X/Y/Rz) still
/// read correctly — unused slots sit at 0. Ground-truth "is the tool seeing my axis"; it
/// ignores deadzones/bindings entirely.
pub(super) fn live_axes(ui: &mut egui::Ui, devices: &[Device], p: &dyn GameProvider, muted: &HashSet<(u16, u16)>) {
    const NAMES: [&str; 8] = ["X", "Y", "Z", "Rx", "Ry", "Rz", "S0", "S1"];
    let shown: Vec<(usize, &Device)> = devices
        .iter()
        .enumerate()
        .filter(|(_, d)| !muted.contains(&(d.vid, d.pid)))
        .collect();
    if shown.is_empty() {
        ui.label(egui::RichText::new("no joysticks in view").weak());
        return;
    }
    for (di, d) in shown {
        ui.label(egui::RichText::new(&d.name).strong().small());
        for i in 0..8 {
            if !d.present[i] {
                continue; // only the axes Windows actually detects
            }
            let v = d.axes[i];
            let tok = p.axis_token(d, i, di).unwrap_or_default();
            let tok = tok.strip_prefix("Joystick_").or_else(|| tok.strip_prefix("Throttle_")).unwrap_or(&tok);
            let label = if tok.is_empty() { NAMES[i].to_string() } else { format!("{} ({})", NAMES[i], tok) };
            let w = ui.available_width().min(360.0);
            ui.add(
                egui::ProgressBar::new(v as f32 / 65535.0)
                    .desired_width(w)
                    .text(egui::RichText::new(format!("{label}  {v}")).small()),
            );
        }
        ui.add_space(3.0);
    }
}
