//! Left-hand device panel shown NEXT TO the binding grid: the real MOZA devices
//! (MHG stick, AB6 base, MRP pedals) as a visual reference, a live "pressed now"
//! token readout, and toggleable arrow callouts on the MHG stick. Images embedded.

use crate::games::GameProvider;
use crate::input::Device;
use eframe::egui;

const STICK_PNG: &[u8] = include_bytes!("../assets/mhg_stick.png");
const BASE_PNG: &[u8] = include_bytes!("../assets/ab6_base.png");
const PEDALS_JPG: &[u8] = include_bytes!("../assets/mrp_pedals.jpg");

// Approximate (normalised x,y, label) positions of the MHG controls in the photo.
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

fn sized(tex: &egui::TextureHandle, w: f32) -> egui::load::SizedTexture {
    let size = tex.size_vec2();
    egui::load::SizedTexture::new(tex.id(), size * (w / size.x).min(1.0))
}

fn image_block(ui: &mut egui::Ui, caption: &str, tex: &egui::TextureHandle, w: f32) {
    ui.add_space(8.0);
    ui.strong(caption);
    ui.add(egui::Image::new(sized(tex, w)));
}

/// Overlay arrow callouts on the MHG image rect.
fn draw_markers(ui: &egui::Ui, rect: egui::Rect) {
    let painter = ui.painter_at(rect);
    let accent = egui::Color32::from_rgb(240, 170, 40);
    for (nx, ny, label) in MHG_MARKERS {
        let p = rect.min + egui::vec2(nx * rect.width(), ny * rect.height());
        let tp = p + egui::vec2(10.0, -9.0);
        painter.line_segment([p, tp], egui::Stroke::new(1.5, accent));
        painter.circle_filled(p, 4.0, accent);
        painter.circle_stroke(p, 4.0, egui::Stroke::new(1.0, egui::Color32::BLACK));
        let galley = painter.layout_no_wrap(label.to_string(), egui::FontId::proportional(11.0), egui::Color32::WHITE);
        let bg = egui::Rect::from_min_size(tp, galley.size() + egui::vec2(6.0, 3.0));
        painter.rect_filled(bg, 3.0, egui::Color32::from_rgba_unmultiplied(18, 20, 28, 210));
        painter.galley(tp + egui::vec2(3.0, 1.0), galley, egui::Color32::WHITE);
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
        if ui.selectable_label(*show_labels, "🏷 Arrows").on_hover_text("Toggle control callouts on the stick").clicked() {
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
        ui.add_space(8.0);
        ui.strong("MHG Flight Stick");
        let resp = ui.add(egui::Image::new(sized(&tex.stick, 360.0)));
        if *show_labels {
            draw_markers(ui, resp.rect);
        }
        image_block(ui, "AB6 FFB Base", &tex.base, 300.0);
        image_block(ui, "MRP Rudder Pedals", &tex.pedals, 360.0);
    });
}
