//! Chrome around the binding grid: the update banner, the central bindings panel
//! (header + legend + the balanced grid columns), and the floating footers. The
//! toolbar lives in its own `toolbar` module. Split out of `update()` purely to
//! keep each file under the size budget — every helper takes only the app state
//! it touches.

use super::widgets::{binding_row, Capture, TEXT_MAIN};
use crate::games::{Action, Binding, GameProvider};
use crate::input;
use eframe::egui;
use std::sync::{Arc, Mutex};

type UpdateCell = Arc<Mutex<Option<(String, String)>>>;

/// A floating "Update available" toast pinned to the TOP-RIGHT, styled to match the
/// app chrome (dark card + the green LIVE accent) — the template for future toasts.
/// Shown only when the background check found a newer release.
pub(super) fn update_banner(ctx: &egui::Context, status: &mut String, update: &UpdateCell) {
    let (ver, url) = match update.lock().unwrap().clone() { Some(p) => p, None => return };
    let accent = super::widgets::LIVE;
    egui::Area::new(egui::Id::new("update_toast"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(egui::Color32::from_rgb(30, 34, 46))
                .stroke(egui::Stroke::new(1.0, accent))
                .rounding(egui::Rounding::same(10.0))
                .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                .show(ui, |ui| {
                    ui.set_max_width(250.0);
                    ui.label(egui::RichText::new("⬆  Update available").strong().color(accent));
                    ui.label(egui::RichText::new(format!("v{ver}  ·  you have v{}", crate::update::current_version()))
                        .size(12.0).color(egui::Color32::from_rgb(150, 165, 190)));
                    ui.add_space(7.0);
                    ui.horizontal(|ui| {
                        let now = egui::Button::new(egui::RichText::new("Update now").strong().color(egui::Color32::from_rgb(12, 18, 14)))
                            .fill(accent).rounding(egui::Rounding::same(6.0));
                        if ui.add(now).clicked() {
                            *status = format!("Downloading v{ver}… the app will relaunch.");
                            if let Err(e) = crate::update::apply(&url) { *status = format!("Update failed: {e}"); }
                        }
                        if ui.add(egui::Button::new(egui::RichText::new("Later").color(egui::Color32::from_rgb(190, 200, 215)))
                            .fill(egui::Color32::from_rgb(48, 54, 70)).rounding(egui::Rounding::same(6.0))).clicked()
                        {
                            *update.lock().unwrap() = None;
                        }
                    });
                });
        });
}

/// Footer overlays. A real TopBottomPanel::bottom gets overpainted by the central
/// columns here, so we float these on top of everything instead: a tiny, unobtrusive
/// version stamp at the bottom-right (no box, no button — updates surface only via the
/// banner when one is actually available), status at the bottom-left.
pub(super) fn footers(ctx: &egui::Context, status: &mut String) {
    egui::Area::new(egui::Id::new("footer_build"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-8.0, -4.0))
        .show(ctx, |ui| {
            // Small inline version, the way polished apps do it: just a faint stamp.
            let branch = match env!("GIT_BRANCH") { "" => "local", b => b };
            let hash = env!("GIT_HASH");
            let tag = if hash.is_empty() { format!("v{}", crate::update::current_version()) }
                      else { format!("v{} · {branch}@{hash}", crate::update::current_version()) };
            ui.label(egui::RichText::new(tag).monospace().size(9.5).color(egui::Color32::from_rgb(120, 130, 150)));
        });
    if status.trim().is_empty() { return; }
    egui::Area::new(egui::Id::new("footer_status"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0))
        .show(ctx, |ui| {
            // Readable toast: dark card, green accent border, bright high-contrast text.
            egui::Frame::popup(ui.style())
                .fill(egui::Color32::from_rgb(30, 34, 46))
                .stroke(egui::Stroke::new(1.0, super::widgets::LIVE))
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::symmetric(14.0, 8.0))
                .show(ui, |ui| {
                    ui.set_max_width(720.0);
                    ui.label(egui::RichText::new(status.as_str()).strong().size(13.5)
                        .color(egui::Color32::from_rgb(228, 234, 242)));
                });
        });
}

/// The central "Cockpit Bindings" panel: header + legend, then the categories
/// laid out across balanced columns with one `binding_row` each. `groups` is the
/// category -> action-index grouping computed once per frame in `mod.rs`.
#[allow(clippy::too_many_arguments)]
pub(super) fn central(
    ctx: &egui::Context,
    games: &[Box<dyn GameProvider>],
    selected: usize,
    textures: &Option<crate::visual::Textures>,
    devices: &[input::Device],
    capture: &mut Option<Capture>,
    rows: &mut [Binding],
    actions: &[Action],
    status: &mut String,
    groups: &[(String, Vec<usize>)],
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if !games[selected].available() {
            ui.centered_and_justified(|ui| { ui.label("This game isn't supported yet — pick MechWarrior 5."); });
            return;
        }
        let p = games[selected].as_ref();
        // Live set of tokens being pressed/moved right now — drives the green glow.
        let hot = crate::visual::hot_tokens(devices, p);

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            if let Some(tex) = textures.as_ref() {
                ui.add(egui::Image::new(&tex.logo).fit_to_exact_size(egui::vec2(30.0, 30.0)).rounding(6.0));
            }
            ui.heading("Cockpit Bindings");
            ui.label(egui::RichText::new(
                "— click a chip, then press the control / move the axis (Esc cancels). A chip turns green when you use it.",
            ).color(egui::Color32::from_rgb(95, 100, 115)));
        });
        // Device colour legend (which colour = which physical device).
        legend(ui);
        ui.separator();

        // Spread the categories across N balanced columns (by row count) so the
        // whole control map fits with far less scrolling. One column row needs
        // ~470px; pick the column count that fits the current central width.
        let avail_w = ui.available_width();
        let avail_h = ui.available_height(); // bound each column's scroll so it can't paint over the footer
        let ncols = ((avail_w / 500.0).floor() as usize).clamp(1, 3);
        let col_w = avail_w / ncols as f32 - 16.0; // minus scrollbar/gutter
        let mut col_groups: Vec<Vec<&(String, Vec<usize>)>> = vec![Vec::new(); ncols];
        let mut heights = vec![0usize; ncols];
        for g in groups {
            let c = (0..ncols).min_by_key(|&c| heights[c]).unwrap_or(0);
            col_groups[c].push(g);
            heights[c] += g.1.len() + 2; // +heading overhead
        }

        // Columns at the bounded central-panel level (so each gets an equal,
        // on-screen half), each with its own vertical scroll. Wrapping ui.columns
        // *inside* one ScrollArea breaks: the scroll area's inner ui is
        // horizontally unbounded, so the split lands the 2nd column off-screen.
        // ONE outer vertical ScrollArea (directly in the central panel, so its
        // height is bounded — it clips/scrolls and never paints over the footer),
        // with set_max_width so ui.columns splits the real width into equal halves.
        let _ = (avail_h, col_w);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(avail_w);
            ui.columns(ncols, |cols| {
                for (col, groups_in_col) in cols.iter_mut().zip(&col_groups) {
                    for (cat, idxs) in groups_in_col {
                        col.add_space(3.0);
                        col.strong(cat);
                        egui::Grid::new(format!("grid_{cat}")).num_columns(4).spacing([8.0, 3.0]).striped(true).show(col, |ui| {
                            for &i in idxs.iter() {
                                binding_row(ui, i, actions, rows, capture, devices, p, status, &hot);
                            }
                        });
                    }
                }
            });
        });
    });
}

/// The device colour legend below the "Cockpit Bindings" heading. Pulled here so
/// the central panel in `mod.rs` stays short.
pub(super) fn legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let chip = |ui: &mut egui::Ui, col: egui::Color32, txt: &str| {
            egui::Frame::none().fill(col).inner_margin(egui::Margin::symmetric(7.0, 2.0)).rounding(4.0).show(ui, |ui| {
                ui.label(egui::RichText::new(txt).color(egui::Color32::BLACK).strong());
            });
        };
        ui.label(egui::RichText::new("Devices:").color(TEXT_MAIN));
        chip(ui, super::widgets::STICK_COL, "Stick / Joystick");
        chip(ui, super::widgets::THROTTLE_COL, "Throttle / Pedals");
        chip(ui, super::widgets::LIVE, "active now");
    });
}
