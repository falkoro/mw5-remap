//! Left-hand device panel shown NEXT TO the binding grid: the real MOZA devices
//! (MHG stick, AB6 base, MRP pedals) as a visual reference with NUMBERED callouts,
//! a live button board + callouts that LIGHT UP when you press/move a control, and
//! a toggle. Images embedded so the panel is self-contained.
//!
//! Numbering: each axis callout shows its token index (Aim Up/Down = Axis 1 = "①"),
//! which is exactly what the grid shows as the token — so the picture and the list
//! line up. Buttons are discovered with the live board (press one, its number lights).
//!
//! Layout: this `mod.rs` owns the public API, asset/marker tables, and orchestration;
//! the pure painting helpers live in `draw`.

mod devices_markers;
mod draw;
mod layout;
mod resolve;

use crate::games::GameProvider;
use crate::input::Device;
use crate::vjoy_map::VjoyMap;
use devices_markers::{BASE_MARKERS, MHG_HATS, MHG_MARKERS, MHG_MULTI, PEDAL_MARKERS, VKB_HATS, VKB_MARKERS};
use draw::{draw_callouts, draw_hats, draw_multi_callouts};
pub use resolve::token_device;
use eframe::egui;
use std::collections::{HashMap, HashSet};

const STICK_PNG: &[u8] = include_bytes!("../../assets/mhg_stick.png");
const BASE_PNG: &[u8] = include_bytes!("../../assets/ab6_base.png");
const PEDALS_JPG: &[u8] = include_bytes!("../../assets/mrp_pedals.jpg");
const VKB_JPG: &[u8] = include_bytes!("../../assets/vkb_evo.jpg");
const LOGO_PNG: &[u8] = include_bytes!("../../assets/logo.png");

// MOZA device ids (must match games::mw5 — used to read the right axis for highlight).
const AB6: (u16, u16) = (0x346E, 0x1002);
const MRP: (u16, u16) = (0x346E, 0x1200);
// VKB Gladiator EVO (must match the devices.rs registry row).
const VKB: (u16, u16) = (0x231D, 0x0201);

const ACCENT: egui::Color32 = egui::Color32::from_rgb(240, 170, 40);
const HOT: egui::Color32 = egui::Color32::from_rgb(70, 210, 110); // lit when pressed/moved

/// One labelled point on a device image. `num` is the badge shown in the dot
/// ("" = no number); `token` is the MW5 token it emits so the callout can light
/// up live ("" = reference only, never lights).
pub(crate) struct Marker {
    nx: f32,
    ny: f32,
    num: &'static str,
    label: &'static str,
    token: &'static str,
}
pub(crate) const fn m(nx: f32, ny: f32, num: &'static str, label: &'static str, token: &'static str) -> Marker {
    Marker { nx, ny, num, label, token }
}

/// One physical control that carries SEVERAL inputs (a hat's directions, a rocker's
/// in/out). Rendered ONCE as a compact stacked list of its `inputs` — each entry is
/// `(sub-label, token)` and glows individually when its token is live. `label` is the
/// control name shown as the box header; `(nx, ny)` is the dot on the image (draggable
/// via `layout`, keyed on `label`, exactly like a single `Marker`).
pub(crate) struct MultiMarker {
    nx: f32,
    ny: f32,
    label: &'static str,
    inputs: &'static [(&'static str, &'static str)],
}
pub(crate) const fn mm(nx: f32, ny: f32, label: &'static str, inputs: &'static [(&'static str, &'static str)]) -> MultiMarker {
    MultiMarker { nx, ny, label, inputs }
}

pub struct Textures {
    pub stick: egui::TextureHandle,
    pub base: egui::TextureHandle,
    pub pedals: egui::TextureHandle,
    pub vkb: egui::TextureHandle,
    pub logo: egui::TextureHandle,
}

fn decode(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture(name, color, egui::TextureOptions::LINEAR))
}

pub fn load_textures(ctx: &egui::Context) -> Option<Textures> {
    Some(Textures {
        stick: decode(ctx, "mhg_stick", STICK_PNG)?,
        base: decode(ctx, "ab6_base", BASE_PNG)?,
        pedals: decode(ctx, "mrp_pedals", PEDALS_JPG)?,
        vkb: decode(ctx, "vkb_evo", VKB_JPG)?,
        logo: decode(ctx, "logo", LOGO_PNG)?,
    })
}

/// Displayed size of a texture capped to width `w` (keeps aspect ratio).
fn disp_size(tex: &egui::TextureHandle, w: f32) -> egui::Vec2 {
    let size = tex.size_vec2();
    size * (w / size.x).min(1.0)
}

/// A MOZA axis is "engaged": centered axes (aim, rudder) when pushed off centre;
/// the toe-throttle (rests at 0) when pressed in. Indices match games::mw5 .Remap.
fn axis_deflected(devices: &[Device], token: &str, muted: &HashSet<(u16, u16)>) -> bool {
    // The throttle (Throttle_Axis2) is driven by BOTH toe brakes — confirmed live on
    // this MRP: right toe = winmm X(0), left toe = winmm Y(1), BOTH UNIPOLAR resting
    // at ~0 (the 32767 seen on a cold first read is a winmm artifact; once polling
    // they sit at 0 and press up to ~64000). Engaged when either toe is pressed in.
    if token == "Throttle_Axis2" {
        // The MRP toe brakes are unipolar, resting at 0 — confirmed live they are
        // Rx(3) / Ry(4) (NOT X/Y). Either toe pressed in = throttle engaged.
        if muted.contains(&MRP) { return false; }
        return match devices.iter().find(|d| (d.vid, d.pid) == MRP) {
            Some(d) => {
                d.axes.get(3).copied().unwrap_or(0) > 12000
                    || d.axes.get(4).copied().unwrap_or(0) > 12000
            }
            None => false,
        };
    }
    // Centred axes in the DirectInput 8-axis layout [X,Y,Z,Rx,Ry,Rz,S0,S1]: AB6 gimbal
    // X=0/Y=1, AB6 analog hat Rx=3 (vertical) / Ry=4 (horizontal), MRP rudder Rz=5.
    // A token can be claimed by more than one device (the generic VKB joystick reuses
    // Joystick_Axis1/2), so collect every (device, slot) it maps to and light if ANY is
    // pushed past ~14% off centre.
    let mut wired: Vec<((u16, u16), usize)> = match token {
        "Joystick_Axis1" => vec![(AB6, 1)], // AB6 gimbal pitch (Y)
        "Joystick_Axis2" => vec![(AB6, 0)], // AB6 gimbal roll (X)
        "Joystick_Axis4" => vec![(AB6, 3)], // AB6 analog hat vertical (Rx)
        "Joystick_Axis5" => vec![(AB6, 4)], // AB6 analog hat horizontal (Ry)
        "Throttle_Axis1" => vec![(MRP, 5)], // MRP rudder swing-arm (Rz)
        _ => Vec::new(),
    };
    // VKB Gladiator EVO (generic joystick): X(0)->Axis1 roll, Y(1)->Axis2 pitch,
    // Rz(5)->Axis6 twist. Centred axes, same ~14% threshold.
    match token {
        "Joystick_Axis1" => wired.push((VKB, 0)),
        "Joystick_Axis2" => wired.push((VKB, 1)),
        "Joystick_Axis6" => wired.push((VKB, 5)),
        _ => {}
    }
    wired.iter().any(|&(id, idx)| {
        !muted.contains(&id) && devices.iter().find(|d| (d.vid, d.pid) == id).is_some_and(|d| {
            ((d.axes.get(idx).copied().unwrap_or(32767) as i32) - 32767).abs() > 9000
        })
    })
}

/// Hat octant currently held on the AB6 (for the live spoke highlight).
fn ab6_octant(devices: &[Device]) -> Option<u32> {
    devices.iter().find(|d| (d.vid, d.pid) == AB6).and_then(|d| d.pov_octant())
}

/// Hat octant currently held on the VKB Gladiator EVO (for its live spoke highlight).
fn vkb_octant(devices: &[Device]) -> Option<u32> {
    devices.iter().find(|d| (d.vid, d.pid) == VKB).and_then(|d| d.pov_octant())
}

/// The live set of tokens currently active: pressed buttons, the POV octant, and
/// deflected/pressed axes. Shared by the device panel AND the Cockpit Bindings
/// list so a binding row lights up the instant you touch its control.
pub fn hot_tokens(devices: &[Device], p: &dyn GameProvider, vjoy_map: &VjoyMap, muted: &HashSet<(u16, u16)>) -> Vec<String> {
    let mut hot: Vec<String> = Vec::new();
    for (i, d) in devices.iter().enumerate() {
        if muted.contains(&(d.vid, d.pid)) { continue; } // soft-muted from the LIVE display
        for b in d.pressed_buttons() {
            if let Some(t) = p.button_token(d, b, i) { hot.push(t); }
        }
        if let Some(o) = d.pov_octant() {
            if let Some(t) = p.pov_token(d, o, i) { hot.push(t); }
        }
    }
    for tok in ["Joystick_Axis1", "Joystick_Axis2", "Joystick_Axis4", "Joystick_Axis5", "Joystick_Axis6", "Throttle_Axis1", "Throttle_Axis2"] {
        if axis_deflected(devices, tok, muted) { hot.push(tok.to_string()); }
    }
    // While the vJoy feeder is active a physical control's input reaches MW5 as its vJoy
    // Target's token, so route every live DIRECT token through the resolver — a marker /
    // binding row then lights on the token MW5 actually receives. Identity (no-op) when
    // vJoy is off, so the hot set is byte-for-byte what it was before.
    let remap = resolve::vjoy_token_remap(p, devices, vjoy_map);
    if remap.is_empty() { return hot; }
    hot.into_iter().map(|t| remap.get(&t).cloned().unwrap_or(t)).collect()
}

/// Live raw-axis readout: one bar per axis per device showing the actual winmm
/// value (0..65535) and the token it binds to. Always shows all six winmm slots
/// [X Y Z R U V] so devices with gaps (the MRP uses X/Y/R) still read correctly —
/// the unused slots simply sit at 0. This is the ground-truth "is the tool seeing
/// my axis" display; it ignores deadzones/bindings entirely.
fn live_axes(ui: &mut egui::Ui, devices: &[Device], p: &dyn GameProvider) {
    const NAMES: [&str; 8] = ["X", "Y", "Z", "Rx", "Ry", "Rz", "S0", "S1"];
    if devices.is_empty() {
        ui.label(egui::RichText::new("no joysticks detected").weak());
        return;
    }
    for (di, d) in devices.iter().enumerate() {
        ui.label(egui::RichText::new(&d.name).strong().small());
        for i in 0..8 {
            if !d.present[i] { continue; } // only the axes Windows actually detects
            let v = d.axes[i];
            let tok = p.axis_token(d, i, di).unwrap_or_default();
            let tok = tok.strip_prefix("Joystick_").or_else(|| tok.strip_prefix("Throttle_")).unwrap_or(&tok);
            let label = if tok.is_empty() { NAMES[i].to_string() } else { format!("{} ({})", NAMES[i], tok) };
            let w = ui.available_width().min(360.0);
            ui.add(egui::ProgressBar::new(v as f32 / 65535.0)
                .desired_width(w)
                .text(egui::RichText::new(format!("{label}  {v}")).small()));
        }
        ui.add_space(3.0);
    }
}

/// Draw a captioned image at width `w` with optional callouts laid over it.
#[allow(clippy::too_many_arguments)]
fn image_block(
    ui: &mut egui::Ui, caption: &str, tex: &egui::TextureHandle, w: f32,
    markers: &[Marker], multi: &[MultiMarker], hats: &[(f32, f32, u8)], hot: &[String], octant: Option<u32>, show: bool,
    bound: &HashMap<String, String>, remap: &HashMap<String, String>, device_key: &str, edit: bool,
) {
    ui.add_space(8.0);
    ui.strong(caption);
    let size = disp_size(tex, w);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    if show {
        // Apply any saved drag overrides, then paint/interact at the resolved positions.
        let resolved: Vec<Marker> = markers.iter().map(|mk| {
            let (nx, ny) = layout::resolved_pos(device_key, mk);
            m(nx, ny, mk.num, mk.label, mk.token)
        }).collect();
        // Multi-input markers (a hat's directions, a rocker's in/out) carry a LIST of
        // tokens and render once as a stacked, individually-glowing list.
        let resolved_multi: Vec<MultiMarker> = multi.iter().map(|mk| {
            let (nx, ny) = layout::resolved_pos_multi(device_key, mk);
            mm(nx, ny, mk.label, mk.inputs)
        }).collect();
        draw_hats(&painter, rect, hats, octant);
        draw_callouts(&painter, rect, &resolved, hot, bound, remap);
        draw_multi_callouts(&painter, rect, &resolved_multi, hot, bound, remap);
        if edit {
            layout::drag_markers(ui, &painter, rect, device_key, &resolved);
            layout::drag_multi(ui, &painter, rect, device_key, &resolved_multi);
        }
    }
}

/// Render the device reference panel: live readout + numbered images whose callouts
/// show what ACTION is bound to each control (`bound`: token -> action label).
#[allow(clippy::too_many_arguments)]
pub fn sidebar(
    ui: &mut egui::Ui,
    tex: &Textures,
    devices: &[Device],
    p: &dyn GameProvider,
    show_labels: &mut bool,
    bound: &HashMap<String, String>,
    vjoy_map: &VjoyMap,
    muted: &HashSet<(u16, u16)>,
    filter: Option<&crate::app::ExportOpts>,
) {
    // During an export capture, `filter` selects which devices to render so the
    // screenshot only contains the chosen ones. Normal frames pass `None` (all on).
    let (want_stick, want_base, want_pedals) = match filter {
        Some(f) => (f.stick, f.base, f.pedals),
        None => (true, true, true),
    };
    // Build the live "hot" token set: pressed buttons, POV octant, deflected axes.
    let hot = hot_tokens(devices, p, vjoy_map, muted);
    // vJoy-aware DIRECT-token -> RESOLVED-token map: while feeding, a marker's hardcoded
    // direct token is two hops from MW5, so resolve it to what the game really receives.
    // EMPTY when vJoy is off => callouts behave exactly as before.
    let remap = resolve::vjoy_token_remap(p, devices, vjoy_map);

    let edit = layout::edit_enabled();
    ui.horizontal(|ui| {
        ui.strong("Devices");
        if ui.selectable_label(*show_labels, "🏷 Arrows").on_hover_text("Toggle numbered callouts on the images").clicked() {
            *show_labels = !*show_labels;
        }
        if ui.selectable_label(edit, "✥ Edit layout").on_hover_text("Drag callout dots to reposition them; saved per device").clicked() {
            layout::set_edit(!edit);
        }
        if edit && ui.button("Reset layout").on_hover_text("Restore all callout dots to their built-in positions").clicked() {
            layout::reset_all();
        }
    });
    ui.add_space(2.0);
    // FINDABLE raw-axis readout: a clear, always-visible collapsible header pinned right
    // under the Devices/Arrows row (collapsed by default). Expand it to watch the live
    // value of every axis while testing — no need to hunt to the bottom of the scroll.
    // The single live "what am I pressing" readout now lives only in the top Detected line.
    if filter.is_none() {
        egui::CollapsingHeader::new(egui::RichText::new("Live axes").strong())
            .default_open(false)
            .show(ui, |ui| live_axes(ui, devices, p))
            .header_response
            .on_hover_text("Raw value of every axis on every device — find your axis while testing.");
    }
    ui.separator();

    // Callouts are drawn whenever labels are on OR we're editing (you need to see the
    // dots to drag them).
    let markers_visible = *show_labels || edit;
    let oct = ab6_octant(devices);
    let vkb_oct = vkb_octant(devices);
    egui::ScrollArea::vertical().show(ui, |ui| {
        let iw = ui.available_width();
        ui.set_max_width(iw); // bound the inner ui so ui.columns splits correctly
        // Main flight stick: full-width and prominent (it carries the most controls).
        if want_stick {
            image_block(ui, "MHG Flight Stick", &tex.stick, iw, MHG_MARKERS, MHG_MULTI, MHG_HATS, &hot, oct, markers_visible, bound, &remap, "stick", edit);
            ui.add_space(6.0);
        }
        // Secondary devices in a 2-up grid (scales as more get added: base, pedals,
        // throttle, …). Each takes half the width.
        if want_base || want_pedals {
            ui.columns(2, |cols| {
                let cw = (iw - 12.0) / 2.0;
                if want_base {
                    image_block(&mut cols[0], "AB6 Base", &tex.base, cw, BASE_MARKERS, &[], &[], &hot, None, markers_visible, bound, &remap, "base", edit);
                }
                if want_pedals {
                    image_block(&mut cols[1], "MRP Pedals", &tex.pedals, cw, PEDAL_MARKERS, &[], &[], &hot, None, markers_visible, bound, &remap, "pedals", edit);
                }
            });
        }
        // VKB Gladiator EVO: full-width like the MHG (it carries many controls). Shown on
        // the live panel only; the export capture (`filter`) targets the MOZA rig sheet.
        if filter.is_none() {
            ui.add_space(6.0);
            image_block(ui, "VKB Gladiator EVO", &tex.vkb, iw, VKB_MARKERS, &[], VKB_HATS, &hot, vkb_oct, markers_visible, bound, &remap, "vkb", edit);
        }
    });
}
