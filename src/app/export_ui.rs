//! The "Export device sheet" feature: a small options dialog, plus the
//! screenshot-capture / crop / write pipeline. The device panel is captured via
//! an egui viewport screenshot (so it reuses the existing callout rendering —
//! no separate redraw), then cropped to the device `SidePanel`'s screen rect and
//! written out as PNG and/or PDF. Split out of `app::mod` to keep it short.

use eframe::egui;

/// What the user chose in the export dialog.
#[derive(Clone)]
pub struct ExportOpts {
    pub stick: bool,
    pub base: bool,
    pub pedals: bool,
    pub png: bool,
    pub pdf: bool,
}

impl Default for ExportOpts {
    fn default() -> Self {
        ExportOpts { stick: true, base: true, pedals: true, png: true, pdf: true }
    }
}

/// Draw the "Export device sheet" window. On "Export" it returns `true` (the
/// caller then requests a screenshot and stashes a pending export); on "Cancel"
/// or close it just hides the window. Mutates `opts`/`show_labels`/`open`.
pub fn dialog(
    ctx: &egui::Context,
    open: &mut bool,
    opts: &mut ExportOpts,
    show_labels: &mut bool,
) -> bool {
    let mut do_export = false;
    let mut cancel = false;
    let mut keep_open = *open;
    egui::Window::new("Export device sheet")
        .open(&mut keep_open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label("Choose what to include in the exported sheet:");
            ui.add_space(4.0);
            ui.checkbox(&mut opts.stick, "Flight stick");
            ui.checkbox(&mut opts.base, "Base");
            ui.checkbox(&mut opts.pedals, "Pedals");
            ui.add_space(4.0);
            ui.checkbox(show_labels, "Show callout labels");
            ui.add_space(6.0);
            ui.label("Format:");
            ui.checkbox(&mut opts.png, "PNG");
            ui.checkbox(&mut opts.pdf, "PDF");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let any_fmt = opts.png || opts.pdf;
                let any_dev = opts.stick || opts.base || opts.pedals;
                if ui.add_enabled(any_fmt && any_dev, egui::Button::new("Export")).clicked() {
                    do_export = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    *open = keep_open && !do_export && !cancel;
    do_export
}

/// Pull a screenshot out of THIS frame's events (egui delivers the captured image
/// as an `Event::Screenshot` the frame after the command is sent).
pub fn take_screenshot(ctx: &egui::Context) -> Option<std::sync::Arc<egui::ColorImage>> {
    ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    })
}

/// Crop the full-window screenshot to `rect` (logical points), scaling by the
/// pixel ratio, and convert to an `image::RgbaImage`.
pub fn crop_to_image(
    shot: &egui::ColorImage,
    rect: egui::Rect,
    ppp: f32,
) -> Option<image::RgbaImage> {
    let (iw, ih) = (shot.width() as f32, shot.height() as f32);
    let x0 = (rect.min.x * ppp).max(0.0).floor() as u32;
    let y0 = (rect.min.y * ppp).max(0.0).floor() as u32;
    let x1 = (rect.max.x * ppp).min(iw).floor() as u32;
    let y1 = (rect.max.y * ppp).min(ih).floor() as u32;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (cw, ch) = (x1 - x0, y1 - y0);
    let mut out = image::RgbaImage::new(cw, ch);
    let sw = shot.width();
    for y in 0..ch {
        for x in 0..cw {
            let px = shot.pixels[(y0 + y) as usize * sw + (x0 + x) as usize];
            out.put_pixel(x, y, image::Rgba([px.r(), px.g(), px.b(), px.a()]));
        }
    }
    Some(out)
}

/// Write the cropped image to PNG and/or PDF next to the exe per `opts`, open the
/// first file written, and return a status string.
pub fn write_files(img: &image::RgbaImage, opts: &ExportOpts) -> String {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut written: Vec<String> = Vec::new();
    let mut first: Option<std::path::PathBuf> = None;

    if opts.png {
        let p = dir.join("MW5-DeviceSheet.png");
        match crate::export::write_png(img, &p) {
            Ok(()) => { first.get_or_insert(p.clone()); written.push(".png".into()); }
            Err(e) => return format!("PNG export failed: {e}"),
        }
    }
    if opts.pdf {
        let p = dir.join("MW5-DeviceSheet.pdf");
        match crate::export::write_pdf(img, &p) {
            Ok(()) => { first.get_or_insert(p.clone()); written.push(".pdf".into()); }
            Err(e) => return format!("PDF export failed: {e}"),
        }
    }
    if let Some(p) = first {
        crate::sys::open_uri(&p.to_string_lossy());
    }
    format!("Exported MW5-DeviceSheet{}", written.join(" + "))
}
