//! Left-hand device panel shown NEXT TO the binding grid: the real MOZA devices
//! (MHG stick, AB6 base, MRP pedals) as a visual reference with NUMBERED callouts,
//! a live button board + callouts that LIGHT UP when you press/move a control, and
//! a toggle. Images embedded so the panel is self-contained.
//!
//! Numbering: each axis callout shows its token index (Aim Up/Down = Axis 1 = "①"),
//! which is exactly what the grid shows as the token — so the picture and the list
//! line up. Buttons are discovered with the live board (press one, its number lights).

use crate::games::GameProvider;
use crate::input::Device;
use eframe::egui;

const STICK_PNG: &[u8] = include_bytes!("../assets/mhg_stick.png");
const BASE_PNG: &[u8] = include_bytes!("../assets/ab6_base.png");
const PEDALS_JPG: &[u8] = include_bytes!("../assets/mrp_pedals.jpg");

// MOZA device ids (must match games::mw5 — used to read the right axis for highlight).
const AB6: (u16, u16) = (0x346E, 0x1002);
const MRP: (u16, u16) = (0x346E, 0x1200);

const ACCENT: egui::Color32 = egui::Color32::from_rgb(240, 170, 40);
const HOT: egui::Color32 = egui::Color32::from_rgb(70, 210, 110); // lit when pressed/moved

/// One labelled point on a device image. `num` is the badge shown in the dot
/// ("" = no number); `token` is the MW5 token it emits so the callout can light
/// up live ("" = reference only, never lights).
struct Marker {
    nx: f32,
    ny: f32,
    num: &'static str,
    label: &'static str,
    token: &'static str,
}
const fn m(nx: f32, ny: f32, num: &'static str, label: &'static str, token: &'static str) -> Marker {
    Marker { nx, ny, num, label, token }
}

// MHG grip: physical reference labels. Button numbers differ per firmware, so we
// don't guess them here — use the live board below to map a button to its number.
// The POV hat does light up (we can read the hat octant directly).
const MHG_MARKERS: &[Marker] = &[
    m(0.41, 0.21, "", "Red button", ""),
    // POV hat: one label per WAY, stacked non-overlapping; each lights individually.
    // Cardinals = look; diagonals = camera/chain-fire (see hat spokes).
    m(0.485, 0.245, "", "Hat ↑ Look Up", "Joystick_Hat_1"),
    m(0.475, 0.275, "", "Hat ← Look Left", "Joystick_Hat_7"),
    m(0.525, 0.255, "", "Hat → Look Right", "Joystick_Hat_3"),
    m(0.515, 0.295, "", "Hat ↓ Look Down", "Joystick_Hat_5"),
    m(0.585, 0.205, "", "Thumb hat (5-way: 4 + push)", ""),
    m(0.645, 0.215, "", "Green button", ""),
    m(0.45, 0.315, "", "Rocker switch", ""),
    m(0.55, 0.305, "", "Thumb button", ""),
    m(0.46, 0.43, "", "Trigger — fire", ""),
    m(0.37, 0.47, "", "Pinky flip (DEF)", ""),
];

// AB6 gimbal -> the two aim axes. Numbers = the Joystick_Axis index (= the token).
const BASE_MARKERS: &[Marker] = &[
    m(0.46, 0.30, "1", "Aim Up / Down  (pitch)", "Joystick_Axis1"),
    m(0.55, 0.40, "2", "Aim Left / Right  (yaw)", "Joystick_Axis2"),
    m(0.50, 0.72, "", "FFB gimbal — \"Joystick\"", ""),
];

// MRP pedals -> Throttle axes. Number = the Throttle_Axis index (= the token).
const PEDAL_MARKERS: &[Marker] = &[
    m(0.50, 0.74, "1", "Turn Left / Right  (rudder slide)", "Throttle_Axis1"),
    m(0.30, 0.40, "2", "Throttle — press toe to go forward", "Throttle_Axis2"),
];

pub struct Textures {
    pub stick: egui::TextureHandle,
    pub base: egui::TextureHandle,
    pub pedals: egui::TextureHandle,
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
    })
}

/// Displayed size of a texture capped to width `w` (keeps aspect ratio).
fn disp_size(tex: &egui::TextureHandle, w: f32) -> egui::Vec2 {
    let size = tex.size_vec2();
    size * (w / size.x).min(1.0)
}

/// Is `token` currently active (in the live hot set)? Hat markers match any octant.
fn token_hot(token: &str, hot: &[String]) -> bool {
    if token.is_empty() { return false; }
    hot.iter().any(|h| h == token || (token == "Joystick_Hat" && h.starts_with("Joystick_Hat")))
}

/// A MOZA axis is "engaged": centered axes (aim, rudder) when pushed off centre;
/// the toe-throttle (rests at 0) when pressed in. Indices match games::mw5 .Remap.
fn axis_deflected(devices: &[Device], token: &str) -> bool {
    let (id, idx, centered) = match token {
        "Joystick_Axis1" => (AB6, 1, true),  // HOTAS_YAxis
        "Joystick_Axis2" => (AB6, 0, true),  // HOTAS_XAxis
        "Throttle_Axis1" => (MRP, 3, true),  // HOTAS_RZAxis (rudder)
        "Throttle_Axis2" => (MRP, 1, false), // a toe pedal (rests at 0)
        _ => return false,
    };
    match devices.iter().find(|d| (d.vid, d.pid) == id) {
        Some(d) => {
            let v = d.axes.get(idx).copied().unwrap_or(32767) as i32;
            if centered { (v - 32767).abs() > 9000 } else { v > 12000 }
        }
        None => false,
    }
}

/// Draw numbered, non-overlapping callouts; a callout turns green when its token
/// is live. Labels stack in the margin nearest each dot, ordered by height.
fn draw_callouts(painter: &egui::Painter, img: egui::Rect, markers: &[Marker], hot: &[String]) {
    let font = egui::FontId::proportional(11.0);
    let numfont = egui::FontId::proportional(10.0);

    let mut left: Vec<&Marker> = markers.iter().filter(|m| m.nx < 0.5).collect();
    let mut right: Vec<&Marker> = markers.iter().filter(|m| m.nx >= 0.5).collect();
    left.sort_by(|a, b| a.ny.partial_cmp(&b.ny).unwrap_or(std::cmp::Ordering::Equal));
    right.sort_by(|a, b| a.ny.partial_cmp(&b.ny).unwrap_or(std::cmp::Ordering::Equal));

    let place = |col: &[&Marker], on_left: bool| {
        let n = col.len();
        for (i, mk) in col.iter().enumerate() {
            let lit = token_hot(mk.token, hot);
            let col_accent = if lit { HOT } else { ACCENT };
            let dot = img.min + egui::vec2(mk.nx * img.width(), mk.ny * img.height());

            let ry = img.top() + img.height() * (i as f32 + 1.0) / (n as f32 + 1.0);
            let galley = painter.layout_no_wrap(mk.label.to_string(), font.clone(), egui::Color32::WHITE);
            let pad = egui::vec2(6.0, 3.0);
            let box_size = galley.size() + pad * 2.0;
            let box_min = if on_left {
                egui::pos2(img.left() + 3.0, ry - box_size.y * 0.5)
            } else {
                egui::pos2(img.right() - 3.0 - box_size.x, ry - box_size.y * 0.5)
            };
            let bg = egui::Rect::from_min_size(box_min, box_size);
            let anchor = if on_left { egui::pos2(bg.right(), bg.center().y) } else { egui::pos2(bg.left(), bg.center().y) };

            painter.line_segment([dot, anchor], egui::Stroke::new(if lit { 2.5 } else { 1.5 }, col_accent));

            // The dot: a numbered badge when the marker has a number, else a small dot.
            if mk.num.is_empty() {
                painter.circle_filled(dot, 4.0, col_accent);
                painter.circle_stroke(dot, 4.0, egui::Stroke::new(1.0, egui::Color32::BLACK));
            } else {
                let r = if lit { 11.0 } else { 9.0 };
                painter.circle_filled(dot, r, col_accent);
                painter.circle_stroke(dot, r, egui::Stroke::new(1.5, egui::Color32::BLACK));
                painter.text(dot, egui::Align2::CENTER_CENTER, mk.num, numfont.clone(), egui::Color32::BLACK);
            }

            painter.rect_filled(bg, 3.0, egui::Color32::from_rgba_unmultiplied(18, 20, 28, 225));
            painter.rect_stroke(bg, 3.0, egui::Stroke::new(if lit { 2.0 } else { 1.0 }, col_accent));
            painter.galley(bg.min + pad, galley, egui::Color32::WHITE);
        }
    };
    place(&left, true);
    place(&right, false);
}

/// Draw a hat as radial spokes (way-count visible); the live octant lights green.
fn draw_hats(painter: &egui::Painter, img: egui::Rect, hats: &[(f32, f32, u8)], active_octant: Option<u32>) {
    // direction vectors indexed by octant 1..8 (1=up,2=NE,3=right,...). y is down.
    const OCT: [(f32, f32); 8] = [
        (0.0, -1.0), (0.707, -0.707), (1.0, 0.0), (0.707, 0.707),
        (0.0, 1.0), (-0.707, 0.707), (-1.0, 0.0), (-0.707, -0.707),
    ];
    for &(nx, ny, ways) in hats {
        let c = img.min + egui::vec2(nx * img.width(), ny * img.height());
        let r = 12.0_f32;
        // 4-way & 5-way: cardinals only (octants 1,3,5,7). 8-way: all.
        let octs: &[usize] = if ways >= 8 { &[0, 1, 2, 3, 4, 5, 6, 7] } else { &[0, 2, 4, 6] };
        for &o in octs {
            let (dx, dy) = OCT[o];
            let d = egui::vec2(dx, dy);
            let lit = active_octant == Some(o as u32 + 1);
            let stroke = egui::Stroke::new(if lit { 3.0 } else { 1.5 }, if lit { HOT } else { ACCENT });
            let end = c + d * r;
            painter.line_segment([c, end], stroke);
            let perp = egui::vec2(-dy, dx);
            let back = end - d * 4.0;
            painter.line_segment([end, back + perp * 3.0], stroke);
            painter.line_segment([end, back - perp * 3.0], stroke);
        }
        painter.circle_stroke(c, 3.0, egui::Stroke::new(1.5, ACCENT));
        if ways == 5 { painter.circle_filled(c, 2.0, ACCENT); }
    }
}

/// Hat octant currently held on the AB6 (for the live spoke highlight).
fn ab6_octant(devices: &[Device]) -> Option<u32> {
    devices.iter().find(|d| (d.vid, d.pid) == AB6).and_then(|d| d.pov_octant())
}

/// The live set of tokens currently active: pressed buttons, the POV octant, and
/// deflected/pressed axes. Shared by the device panel AND the Cockpit Bindings
/// list so a binding row lights up the instant you touch its control.
pub fn hot_tokens(devices: &[Device], p: &dyn GameProvider) -> Vec<String> {
    let mut hot: Vec<String> = Vec::new();
    for (i, d) in devices.iter().enumerate() {
        for b in d.pressed_buttons() {
            if let Some(t) = p.button_token(d, b, i) { hot.push(t); }
        }
        if let Some(o) = d.pov_octant() {
            if let Some(t) = p.pov_token(d, o, i) { hot.push(t); }
        }
    }
    for tok in ["Joystick_Axis1", "Joystick_Axis2", "Throttle_Axis1", "Throttle_Axis2"] {
        if axis_deflected(devices, tok) { hot.push(tok.to_string()); }
    }
    hot
}

// Main POV hat = 8-way (confirmed: MW5 Joystick_Hat_1..8, MOZA hat configurable
// 8/4-way in MOZA Cockpit). Thumb control = a 5-way switch (4 dirs + center push).
const MHG_HATS: &[(f32, f32, u8)] = &[(0.50, 0.27, 8), (0.585, 0.205, 5)];

/// Draw a captioned image at width `w` with optional callouts laid over it.
fn image_block(
    ui: &mut egui::Ui, caption: &str, tex: &egui::TextureHandle, w: f32,
    markers: &[Marker], hats: &[(f32, f32, u8)], hot: &[String], octant: Option<u32>, show: bool,
) {
    ui.add_space(8.0);
    ui.strong(caption);
    let size = disp_size(tex, w);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    if show {
        draw_hats(&painter, rect, hats, octant);
        draw_callouts(&painter, rect, markers, hot);
    }
}

/// Render the device reference panel: live readout, button board, numbered images.
pub fn sidebar(ui: &mut egui::Ui, tex: &Textures, devices: &[Device], p: &dyn GameProvider, show_labels: &mut bool) {
    // Build the live "hot" token set: pressed buttons, POV octant, deflected axes.
    let hot = hot_tokens(devices, p);
    let mut readout = hot.clone();
    readout.sort();
    readout.dedup();

    ui.horizontal(|ui| {
        ui.strong("Devices");
        if ui.selectable_label(*show_labels, "🏷 Arrows").on_hover_text("Toggle numbered callouts on the images").clicked() {
            *show_labels = !*show_labels;
        }
    });
    ui.add_space(2.0);
    ui.strong("Active now");
    if readout.is_empty() {
        ui.label(egui::RichText::new("press a button or move an axis…").weak());
    } else {
        ui.horizontal_wrapped(|ui| {
            for t in &readout {
                ui.label(egui::RichText::new(format!("🟢 {t}")).color(HOT));
            }
        });
    }
    ui.separator();

    let oct = ab6_octant(devices);
    egui::ScrollArea::vertical().show(ui, |ui| {
        image_block(ui, "MHG Flight Stick", &tex.stick, 360.0, MHG_MARKERS, MHG_HATS, &hot, oct, *show_labels);
        image_block(ui, "AB6 FFB Base", &tex.base, 320.0, BASE_MARKERS, &[], &hot, None, *show_labels);
        image_block(ui, "MRP Rudder Pedals", &tex.pedals, 360.0, PEDAL_MARKERS, &[], &hot, None, *show_labels);
    });
}
