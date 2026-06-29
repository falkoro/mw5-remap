//! The "Route to vJoy" panel: a built-in Joystick-Gremlin replacement. Pick a
//! connected stick, then either AUTO-ROUTE its whole button/axis set onto vJoy, or
//! CAPTURE-bind a single physical control to a chosen vJoy target. Mappings live in
//! `vjoy_map.txt`; feeding is active whenever ≥1 mapping exists (unless paused).

use crate::input::Device;
use crate::vjoy_map::{Mapping, Source, Target, VjoyMap, VJOY_AXES};
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
    paused: &mut bool,
    status: &mut String,
) {
    egui::TopBottomPanel::top("vjoy_route").show(ctx, |ui| {
        ui.add_space(2.0);
        let vjoy_ok = crate::vjoy::available();
        // keep the selected device valid (default to the first connected stick)
        if sel.map(|s| !devices.iter().any(|d| (d.vid, d.pid) == s)).unwrap_or(true) {
            *sel = devices.first().map(|d| (d.vid, d.pid));
        }
        ui.horizontal_wrapped(|ui| {
            ui.strong("🕹 Route to vJoy:");
            if !vjoy_ok {
                ui.colored_label(egui::Color32::from_rgb(200, 120, 60),
                    "vJoy not detected — install vJoy and configure device 1 to use this.");
                return;
            }
            ui.label("Stick:");
            let cur = sel.and_then(|s| devices.iter().find(|d| (d.vid, d.pid) == s));
            let cur_name = cur.map(|d| d.name.clone()).unwrap_or_else(|| "(no stick)".into());
            egui::ComboBox::from_id_salt("vjoy_stick").selected_text(cur_name).show_ui(ui, |ui| {
                for d in devices {
                    ui.selectable_value(sel, Some((d.vid, d.pid)), &d.name);
                }
            });

            // Auto-route: map the whole selected stick onto the next free vJoy slots.
            if ui.add_enabled(cur.is_some(), egui::Button::new("⚡ Auto-route this stick"))
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
            let st = crate::vjoy::status();
            let active = !*paused && !map.mappings.is_empty();
            ui.label(format!("vJoy: {}  ({})",
                if active { "feeding" } else { "idle" },
                match st { 0 => "own", 1 => "free", 2 => "busy", 3 => "miss", _ => "?" }));
        });

        // Precise bind: pick a vJoy target, click Bind, then actuate the control.
        ui.horizontal_wrapped(|ui| {
            let capturing = capture.is_some();
            ui.label("Bind one →  vJoy Button");
            ui.add_enabled(!capturing, egui::DragValue::new(btn_pick).range(1..=32));
            if ui.add_enabled(vjoy_ok && sel.is_some() && !capturing, egui::Button::new("● Bind button")).clicked() {
                start_capture(capture, sel, devices, Target::Button(*btn_pick), status);
            }
            ui.separator();
            ui.label("or Axis");
            let axis_label = crate::vjoy_map::axis_name(*axis_pick);
            egui::ComboBox::from_id_salt("vjoy_axis").selected_text(axis_label).show_ui(ui, |ui| {
                for u in VJOY_AXES { ui.selectable_value(axis_pick, u, crate::vjoy_map::axis_name(u)); }
            });
            if ui.add_enabled(vjoy_ok && sel.is_some() && !capturing, egui::Button::new("● Bind axis")).clicked() {
                start_capture(capture, sel, devices, Target::Axis(*axis_pick), status);
            }
            if capturing {
                ui.colored_label(egui::Color32::from_rgb(235, 170, 45), "press a control on the stick… (Esc cancels)");
            }
        });

        // Live mappings list with per-row remove + invert toggle.
        if !map.mappings.is_empty() {
            ui.add_space(2.0);
            let mut remove: Option<usize> = None;
            egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                for i in 0..map.mappings.len() {
                    ui.horizontal(|ui| {
                        if ui.small_button("✕").on_hover_text("Remove this mapping").clicked() { remove = Some(i); }
                        let m = &mut map.mappings[i];
                        let mut inv = m.invert;
                        if ui.checkbox(&mut inv, "Inv").changed() { m.invert = inv; map_dirty(map, status); }
                        let m = &map.mappings[i];
                        ui.label(format!("{:04X}:{:04X}  {}  →  {}", m.vid, m.pid, m.source.label(), m.target.label()));
                    });
                }
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
