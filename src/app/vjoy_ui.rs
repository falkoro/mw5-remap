//! The "Route to vJoy" panel: a built-in Joystick-Gremlin replacement. Pick a
//! connected stick, then either AUTO-ROUTE its whole button/axis set onto vJoy, or
//! CAPTURE-bind a single physical control to a chosen vJoy target. Mappings live in
//! `vjoy_map.txt`; feeding is active whenever ≥1 mapping exists (unless paused).

use super::vjoy_style::{pill_button, section, status_pill, ACCENT, BG, MUTED, TXT};
use crate::input::Device;
use crate::vjoy_map::{axis_name, Mapping, Source, Target, VjoyMap, VJOY_AXES};
use eframe::egui;

/// An in-progress "click a vJoy target, then actuate a physical control" capture.
pub(super) struct VjoyCapture {
    pub target: Target,
    pub vid: u16,
    pub pid: u16,
    pub base_axes: [u32; 8],
    pub base_buttons: u32,
}

/// Resolve a pending vJoy capture: detect the first newly-pressed button or moved
/// axis on the captured device and record the mapping. Called each frame from mod.rs.
pub(super) fn resolve_capture(
    capture: &mut Option<VjoyCapture>,
    devices: &[Device],
    map: &mut VjoyMap,
    status: &mut String,
) {
    let cap = match capture.as_ref() { Some(c) => c, None => return };
    let dev = match devices.iter().find(|d| d.vid == cap.vid && d.pid == cap.pid) {
        Some(d) => d, None => return,
    };
    // newly-pressed button bit (not held when capture began)
    let fresh = dev.buttons & !cap.base_buttons;
    let source = if fresh != 0 {
        Some(Source::Button(fresh.trailing_zeros() as u8))
    } else {
        // else the most-moved axis (>12000 raw, same threshold as the grid capture)
        let mut best = (12_000i64, None);
        for ax in 0..8 {
            let d = (dev.axes[ax] as i64 - cap.base_axes[ax] as i64).abs();
            if d > best.0 { best = (d, Some(ax as u8)); }
        }
        best.1.map(Source::Axis)
    };
    if let Some(source) = source {
        let m = Mapping { vid: cap.vid, pid: cap.pid, source, target: cap.target, invert: false };
        status.clear();
        status.push_str(&format!("Routed {} -> {}.", source.label(), cap.target.label()));
        map.add(m);
        let _ = map.save();
        *capture = None;
    }
}

/// The vJoy routing panel (a top strip under the main toolbar).
#[allow(clippy::too_many_arguments)]
pub(super) fn panel(
    ctx: &egui::Context,
    devices: &[Device],
    map: &mut VjoyMap,
    capture: &mut Option<VjoyCapture>,
    sel: &mut Option<(u16, u16)>,
    btn_pick: &mut u8,
    axis_pick: &mut u32,
    pair_fwd: &mut u8,
    pair_rev: &mut u8,
    paused: &mut bool,
    status: &mut String,
) {
    let frame = egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(12.0, 9.0));
    egui::TopBottomPanel::top("vjoy_route").frame(frame).show(ctx, |ui| {
        let vjoy_ok = crate::vjoy::available();
        // keep the selected device valid (default to the first connected stick)
        if sel.map(|s| !devices.iter().any(|d| (d.vid, d.pid) == s)).unwrap_or(true) {
            *sel = devices.first().map(|d| (d.vid, d.pid));
        }

        // Header: title + live status pill.
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🕹  Route to vJoy").heading().color(ACCENT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                status_pill(ui, *paused, map, vjoy_ok);
            });
        });
        ui.add_space(6.0);

        if !vjoy_ok {
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(58, 44, 28))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 120, 60)))
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::symmetric(11.0, 9.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("⚠  vJoy not detected").strong().color(egui::Color32::from_rgb(235, 180, 120)));
                    ui.label(egui::RichText::new("Install vJoy and configure device 1, then reopen this tab.").color(MUTED));
                });
            ui.add_space(4.0);
            return;
        }

        let cur = sel.and_then(|s| devices.iter().find(|d| (d.vid, d.pid) == s));

        // SOURCE STICK + auto-route.
        section(ui, "SOURCE STICK", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Stick").color(MUTED));
                let cur_name = cur.map(|d| d.name.clone()).unwrap_or_else(|| "(no stick)".into());
                egui::ComboBox::from_id_salt("vjoy_stick").selected_text(cur_name).width(210.0).show_ui(ui, |ui| {
                    for d in devices { ui.selectable_value(sel, Some((d.vid, d.pid)), &d.name); }
                });
                if pill_button(ui, cur.is_some(), "⚡ Auto-route whole stick", true)
                    .on_hover_text("Map ALL of this stick's buttons and axes onto vJoy (sequential buttons, free X/Y/Z/Rx/Ry/Rz axes).")
                    .clicked()
                {
                    if let Some(d) = cur {
                        map.auto_route(d);
                        let _ = map.save();
                        *status = format!("Auto-routed \"{}\" onto vJoy ({} mappings total).", d.name, map.mappings.len());
                    }
                }
                ui.checkbox(paused, "⏸ Pause feeding").on_hover_text("Stop feeding vJoy without deleting mappings.");
            });
        });
        ui.add_space(6.0);

        // BIND ONE CONTROL — pick a vJoy target, click Bind, then actuate the control.
        section(ui, "BIND ONE CONTROL", |ui| {
            let capturing = capture.is_some();
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("vJoy button").color(MUTED));
                ui.add_enabled(!capturing, egui::DragValue::new(btn_pick).range(1..=32));
                if pill_button(ui, sel.is_some() && !capturing, "● Bind button", false).clicked() {
                    start_capture(capture, sel, devices, Target::Button(*btn_pick), status);
                }
                ui.separator();
                ui.label(egui::RichText::new("vJoy axis").color(MUTED));
                egui::ComboBox::from_id_salt("vjoy_axis").selected_text(axis_name(*axis_pick)).show_ui(ui, |ui| {
                    for u in VJOY_AXES { ui.selectable_value(axis_pick, u, axis_name(u)); }
                });
                if pill_button(ui, sel.is_some() && !capturing, "● Bind axis", false).clicked() {
                    start_capture(capture, sel, devices, Target::Axis(*axis_pick), status);
                }
            });
            if capturing {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("⏺ press a control on the stick…  (Esc cancels)")
                    .strong().color(egui::Color32::from_rgb(235, 180, 90)));
            }
        });
        ui.add_space(6.0);

        // COMBINE two physical axes into ONE bipolar vJoy axis (two toe pedals -> one
        // forward/reverse throttle: centre=stop, fwd axis up, rev axis down).
        section(ui, "COMBINE TWO AXES → ONE", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Target").color(MUTED));
                egui::ComboBox::from_id_salt("vjoy_pair_axis").selected_text(axis_name(*axis_pick)).show_ui(ui, |ui| {
                    for u in VJOY_AXES { ui.selectable_value(axis_pick, u, axis_name(u)); }
                });
                ui.label(egui::RichText::new("fwd").color(MUTED));
                axis_index_combo(ui, "vjoy_pair_fwd", pair_fwd, cur);
                ui.label(egui::RichText::new("rev").color(MUTED));
                axis_index_combo(ui, "vjoy_pair_rev", pair_rev, cur);
                let ok = sel.is_some() && pair_fwd != pair_rev;
                if pill_button(ui, ok, "➕ Add combine", false)
                    .on_hover_text("Map the forward axis (up) + reverse axis (down) onto ONE centred bipolar vJoy axis.")
                    .clicked()
                {
                    if let Some((vid, pid)) = *sel {
                        map.add(Mapping { vid, pid, source: Source::Pair(*pair_fwd, *pair_rev), target: Target::Axis(*axis_pick), invert: false });
                        let _ = map.save();
                        *status = format!("Combined Axis {}+/Axis {}- → vJoy Axis {}.", *pair_fwd + 1, *pair_rev + 1, axis_name(*axis_pick));
                    }
                }
            });
        });

        // MAPPINGS — live list with per-row remove + invert toggle.
        if !map.mappings.is_empty() {
            ui.add_space(6.0);
            let mut remove: Option<usize> = None;
            section(ui, &format!("MAPPINGS  ({})", map.mappings.len()), |ui| {
                egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                    for i in 0..map.mappings.len() {
                        ui.horizontal(|ui| {
                            if ui.small_button("✕").on_hover_text("Remove this mapping").clicked() { remove = Some(i); }
                            let m = &mut map.mappings[i];
                            let mut inv = m.invert;
                            if ui.checkbox(&mut inv, "Inv").changed() { m.invert = inv; map_dirty(map, status); }
                            let m = &map.mappings[i];
                            ui.label(egui::RichText::new(format!("{:04X}:{:04X}", m.vid, m.pid)).size(11.5).color(MUTED));
                            ui.label(egui::RichText::new(m.source.label()).color(TXT));
                            ui.label(egui::RichText::new("→").color(ACCENT));
                            ui.label(egui::RichText::new(m.target.label()).strong().color(ACCENT));
                        });
                    }
                });
            });
            if let Some(i) = remove {
                map.remove(i);
                let _ = map.save();
                *status = "Removed a vJoy mapping.".into();
            }
        }
        ui.add_space(2.0);
    });
}

/// Re-save after an in-place edit (the borrow of `map.mappings[i]` ended before this).
fn map_dirty(map: &VjoyMap, status: &mut String) {
    let _ = map.save();
    *status = "Updated vJoy mapping.".into();
}

/// A small "Axis N" picker over a stick's present axes (falls back to all 8 slots).
fn axis_index_combo(ui: &mut egui::Ui, id: &str, idx: &mut u8, cur: Option<&Device>) {
    egui::ComboBox::from_id_salt(id).selected_text(format!("Axis {}", *idx + 1)).show_ui(ui, |ui| {
        for a in 0..8u8 {
            if cur.map(|d| d.present[a as usize]).unwrap_or(true) {
                ui.selectable_value(idx, a, format!("Axis {}", a + 1));
            }
        }
    });
}

fn start_capture(
    capture: &mut Option<VjoyCapture>,
    sel: &Option<(u16, u16)>,
    devices: &[Device],
    target: Target,
    status: &mut String,
) {
    let (vid, pid) = match sel { Some(s) => *s, None => return };
    let dev = devices.iter().find(|d| (d.vid, d.pid) == (vid, pid));
    let (base_axes, base_buttons) = dev.map(|d| (d.axes, d.buttons)).unwrap_or(([0; 8], 0));
    *capture = Some(VjoyCapture { target, vid, pid, base_axes, base_buttons });
    *status = format!("Listening… actuate a control to bind {}.", target.label());
}
