//! Sidebar chrome that isn't a device image: the per-device "in view" hide toggles and
//! the collapsible raw-axis readout. Both reuse the shared live-mute set, so hiding a
//! controller you're not using (e.g. a button box) drops its image, axes AND live glow
//! from the Bind panel in one click — purely a display concern, never HidHide. Kept out
//! of `mod.rs` so the orchestrator stays within the size budget.

use super::devices_markers::{BASE_MARKERS, MHG_HATS, MHG_MARKERS, MHG_MULTI, PEDAL_MARKERS, VKB_HATS, VKB_MARKERS};
use super::{axes_state, image_block, order, Marker, MultiMarker, Textures};
use crate::app::theme;
use crate::games::GameProvider;
use crate::input::Device;
use eframe::egui;
use std::collections::{HashMap, HashSet};

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

/// The scrolling sidebar body. LIVE: each visible device is one full-width, REORDERABLE
/// block (▲/▼ on its header persist the order via `order`), followed by the collapsible
/// "Live axes" readout sitting BELOW all the pictures. EXPORT (capture frame): the original
/// MOZA sheet layout (stick full-width + base/pedals 2-up) is kept so the exported PNG/PDF
/// is unchanged. Split out of `mod.rs::sidebar` to keep that orchestrator within budget.
#[allow(clippy::too_many_arguments)]
pub(super) fn device_scroll(
    ui: &mut egui::Ui,
    tex: &Textures,
    devices: &[Device],
    p: &dyn GameProvider,
    hot: &[String],
    bound: &HashMap<String, String>,
    remap: &HashMap<String, String>,
    oct: Option<u32>,
    _vkb_oct: Option<u32>, // VKB hat octant — unused now the device pictures don't show live glow
    markers_visible: bool,
    edit: bool,
    live: bool,
    muted: &HashSet<(u16, u16)>,
    want: (bool, bool, bool),
) {
    let (want_stick, want_base, want_pedals) = want;
    egui::ScrollArea::vertical().show(ui, |ui| {
        let iw = ui.available_width();
        ui.set_max_width(iw); // bound the inner ui so ui.columns splits correctly
        if !live {
            // Export-capture frame: keep the original sheet layout (stick + base/pedals 2-up).
            if want_stick {
                image_block(ui, "MHG Flight Stick", &tex.stick, iw, MHG_MARKERS, MHG_MULTI, MHG_HATS, hot, oct, markers_visible, bound, remap, "stick", edit);
                ui.add_space(6.0);
            }
            if want_base || want_pedals {
                ui.columns(2, |cols| {
                    let cw = (iw - 12.0) / 2.0;
                    if want_base {
                        image_block(&mut cols[0], "AB6 Base", &tex.base, cw, BASE_MARKERS, &[], &[], hot, None, markers_visible, bound, remap, "base", edit);
                    }
                    if want_pedals {
                        image_block(&mut cols[1], "MRP Pedals", &tex.pedals, cw, PEDAL_MARKERS, &[], &[], hot, None, markers_visible, bound, remap, "pedals", edit);
                    }
                });
            }
            return;
        }

        // LIVE: one full-width, reorderable block per VISIBLE device.
        struct Dev<'a> {
            id: (u16, u16),
            key: &'static str,
            caption: &'static str,
            tex: &'a egui::TextureHandle,
            markers: &'static [Marker],
            multi: &'static [MultiMarker],
            hats: &'static [(f32, f32, u8)],
        }
        let all = [
            Dev { id: super::AB6, key: "stick", caption: "MHG Flight Stick", tex: &tex.stick, markers: MHG_MARKERS, multi: MHG_MULTI, hats: MHG_HATS },
            Dev { id: super::AB6, key: "base", caption: "AB6 Base", tex: &tex.base, markers: BASE_MARKERS, multi: &[], hats: &[] },
            Dev { id: super::MRP, key: "pedals", caption: "MRP Pedals", tex: &tex.pedals, markers: PEDAL_MARKERS, multi: &[], hats: &[] },
            Dev { id: super::VKB, key: "vkb", caption: "VKB Gladiator EVO", tex: &tex.vkb, markers: VKB_MARKERS, multi: &[], hats: VKB_HATS },
        ];
        let visible: Vec<&Dev> = all
            .iter()
            .filter(|d| {
                let on = !muted.contains(&d.id);
                match d.key {
                    "stick" => want_stick && on,
                    "base" => want_base && on,
                    "pedals" => want_pedals && on,
                    _ => on, // vkb is always shown live unless its own device is muted
                }
            })
            .collect();
        let keys: Vec<&str> = visible.iter().map(|d| d.key).collect();
        let order = order::ordered(&keys);
        for (pos, key) in order.iter().enumerate() {
            let Some(d) = visible.iter().find(|d| d.key == key.as_str()) else { continue };
            let up = if pos > 0 { Some(order[pos - 1].as_str()) } else { None };
            let down = if pos + 1 < order.len() { Some(order[pos + 1].as_str()) } else { None };
            reorder_header(ui, d.caption, d.key, up, down);
            // Live glow lives on the cockpit CHIPS only (user choice): pass an empty hot set +
            // no hat-octant so the device picture shows its static markers but never the
            // duplicate green press-glow. `bound`/`remap` stay for the labelled callouts.
            image_block(ui, "", d.tex, iw, d.markers, d.multi, d.hats, &[], None, markers_visible, bound, remap, d.key, edit);
            ui.add_space(6.0);
        }

        // Raw-axis readout sits BELOW all the pictures. Open/closed is remembered PER GAME
        // (axes_state, keyed on p.name()): we fully CONTROL the header's open state so it
        // honours the saved value even when switching games within a session, and persist
        // the new value on each toggle. Default OPEN for a game we've not seen.
        ui.add_space(2.0);
        ui.separator();
        let game = p.name();
        let mut axes_open = axes_state::is_open(game);
        let resp = egui::CollapsingHeader::new(egui::RichText::new("Live axes").strong())
            .open(Some(axes_open))
            .show(ui, |ui| live_axes(ui, devices, p, muted));
        if resp.header_response.clicked() {
            axes_open = !axes_open;
            axes_state::set_open(game, axes_open);
        }
        resp.header_response
            .on_hover_text("Raw value of every axis on every device in view — find your axis while testing.");
    });
}

/// Caption row for a device image in the LIVE sidebar: the device name plus painted ▲/▼
/// buttons that move it up/down in the persisted order. `up`/`down` carry the neighbour key
/// to swap with (None = already at that end, so the arrow is greyed and the click ignored).
fn reorder_header(ui: &mut egui::Ui, caption: &str, key: &str, up: Option<&str>, down: Option<&str>) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.strong(caption);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::arrow_button(ui, false, down.is_some()).on_hover_text("Move this device down").clicked() {
                if let Some(d) = down { order::swap(key, d); }
            }
            if theme::arrow_button(ui, true, up.is_some()).on_hover_text("Move this device up").clicked() {
                if let Some(u) = up { order::swap(key, u); }
            }
        });
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
    // Fixed columns so every row lines up: [ axis name | bar | raw value ]. The name is a
    // constant-width cell, the bars all start at the same x and share one width, and the
    // monospace value sits in its own right-hand cell — no ragged edges.
    const LW: f32 = 104.0; // axis-name column ("Rx (Axis5)")
    const VW: f32 = 48.0; // raw-value column (monospace, up to 65535)
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
            let barw = (ui.available_width() - LW - VW - 12.0).clamp(60.0, 300.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.add_sized([LW, 16.0], egui::Label::new(egui::RichText::new(label).small()).truncate());
                ui.add(egui::ProgressBar::new(v as f32 / 65535.0).desired_width(barw));
                ui.allocate_ui_with_layout(
                    egui::vec2(VW, 16.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| ui.label(egui::RichText::new(format!("{v}")).small().monospace()),
                );
            });
        }
        ui.add_space(3.0);
    }
}
