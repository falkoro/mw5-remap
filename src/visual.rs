//! Left-hand device panel shown NEXT TO the binding grid: the real MOZA devices
//! (MHG stick, AB6 base, MRP pedals) as a visual reference, a live "pressed now"
//! token readout, and toggleable arrow callouts on ALL three images. Images embedded.
//!
//! Callout layout: dots sit on the control; labels are stacked in the left/right
//! margin (left-half controls -> left column, right-half -> right column), evenly
//! spaced and sorted by height, so leader lines and label boxes never overlap.

use crate::games::GameProvider;
use crate::input::Device;
use eframe::egui;

const STICK_PNG: &[u8] = include_bytes!("../assets/mhg_stick.png");
const BASE_PNG: &[u8] = include_bytes!("../assets/ab6_base.png");
const PEDALS_JPG: &[u8] = include_bytes!("../assets/mrp_pedals.jpg");

// Approximate (normalised x,y, label) positions of each device's controls.
// x<0.5 -> label goes to the left column, x>=0.5 -> right column.
const MHG_MARKERS: &[(f32, f32, &str)] = &[
    (0.41, 0.21, "Red button"),
    (0.50, 0.27, "POV hat"),
    (0.585, 0.205, "4-way hat"),
    (0.645, 0.215, "Green button"),
    (0.45, 0.315, "Rocker"),
    (0.55, 0.305, "Thumb button"),
    (0.46, 0.43, "Trigger"),
    (0.37, 0.47, "DEF flip"),
];

// AB6 base: the gimbal provides the two aim axes (role = Joystick).
const BASE_MARKERS: &[(f32, f32, &str)] = &[
    (0.46, 0.30, "Aim Up/Down — Axis1"),
    (0.54, 0.40, "Aim Left/Right — Axis2"),
    (0.50, 0.70, "FFB base (Joystick)"),
];

// MRP pedals (role = Throttle): axes only — no buttons, no D-pad.
const PEDAL_MARKERS: &[(f32, f32, &str)] = &[
    (0.30, 0.45, "Throttle — Axis1"),
    (0.70, 0.45, "Strafe — Axis2"),
    (0.50, 0.72, "Rudder — Axis3"),
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

/// Draw non-overlapping callouts over `img` rect. Labels stack in the margin on
/// the side nearest each dot, evenly spaced and ordered by height.
fn draw_callouts(painter: &egui::Painter, img: egui::Rect, markers: &[(f32, f32, &str)]) {
    let accent = egui::Color32::from_rgb(240, 170, 40);
    let font = egui::FontId::proportional(11.0);

    // Split into left/right columns (by dot x), then order each by dot y so the
    // stacked rows mirror the vertical order of the dots — leader lines don't cross.
    let mut left: Vec<&(f32, f32, &str)> = markers.iter().filter(|m| m.0 < 0.5).collect();
    let mut right: Vec<&(f32, f32, &str)> = markers.iter().filter(|m| m.0 >= 0.5).collect();
    left.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    right.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let place = |col: &[&(f32, f32, &str)], on_left: bool| {
        let n = col.len();
        for (i, m) in col.iter().enumerate() {
            let (nx, ny, label) = **m;
            let dot = img.min + egui::vec2(nx * img.width(), ny * img.height());

            // Row centre for this label, evenly spaced down the image height.
            let ry = img.top() + img.height() * (i as f32 + 1.0) / (n as f32 + 1.0);
            let galley = painter.layout_no_wrap(label.to_string(), font.clone(), egui::Color32::WHITE);
            let pad = egui::vec2(6.0, 3.0);
            let box_size = galley.size() + pad * 2.0;

            // Anchor the box just inside the left or right edge of the image.
            let box_min = if on_left {
                egui::pos2(img.left() + 3.0, ry - box_size.y * 0.5)
            } else {
                egui::pos2(img.right() - 3.0 - box_size.x, ry - box_size.y * 0.5)
            };
            let bg = egui::Rect::from_min_size(box_min, box_size);

            // Leader line from the dot to the inner edge of the label box.
            let anchor = if on_left {
                egui::pos2(bg.right(), bg.center().y)
            } else {
                egui::pos2(bg.left(), bg.center().y)
            };
            painter.line_segment([dot, anchor], egui::Stroke::new(1.5, accent));
            painter.circle_filled(dot, 4.0, accent);
            painter.circle_stroke(dot, 4.0, egui::Stroke::new(1.0, egui::Color32::BLACK));

            painter.rect_filled(bg, 3.0, egui::Color32::from_rgba_unmultiplied(18, 20, 28, 225));
            painter.rect_stroke(bg, 3.0, egui::Stroke::new(1.0, accent));
            painter.galley(bg.min + pad, galley, egui::Color32::WHITE);
        }
    };
    place(&left, true);
    place(&right, false);
}

/// Draw a captioned image at width `w` with optional callouts laid over it.
fn image_block(ui: &mut egui::Ui, caption: &str, tex: &egui::TextureHandle, w: f32, markers: &[(f32, f32, &str)], show: bool) {
    ui.add_space(8.0);
    ui.strong(caption);
    let size = disp_size(tex, w);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.image(
        tex.id(),
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
    if show {
        draw_callouts(&painter, rect, markers);
    }
}

/// Render the device reference panel: pressed-token readout, labels toggle, images.
pub fn sidebar(ui: &mut egui::Ui, tex: &Textures, devices: &[Device], p: &dyn GameProvider, show_labels: &mut bool) {
    let mut pressed: Vec<String> = Vec::new();
    for (i, d) in devices.iter().enumerate() {
        for b in d.pressed_buttons() {
            if let Some(t) = p.button_token(d, b, i) { pressed.push(t); }
        }
        if let Some(o) = d.pov_octant() {
            if let Some(t) = p.pov_token(d, o, i) { pressed.push(t); }
        }
    }
    pressed.sort();
    pressed.dedup();

    ui.horizontal(|ui| {
        ui.strong("Devices");
        if ui.selectable_label(*show_labels, "🏷 Arrows").on_hover_text("Toggle control callouts on the images").clicked() {
            *show_labels = !*show_labels;
        }
    });
    ui.add_space(2.0);
    ui.strong("Pressed now");
    if pressed.is_empty() {
        ui.label(egui::RichText::new("press a button to identify it…").weak());
    } else {
        for t in &pressed {
            ui.label(egui::RichText::new(format!("🟢 {t}")).color(egui::Color32::from_rgb(80, 200, 120)));
        }
    }
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        image_block(ui, "MHG Flight Stick", &tex.stick, 360.0, MHG_MARKERS, *show_labels);
        image_block(ui, "AB6 FFB Base", &tex.base, 320.0, BASE_MARKERS, *show_labels);
        image_block(ui, "MRP Rudder Pedals", &tex.pedals, 360.0, PEDAL_MARKERS, *show_labels);
    });
}
