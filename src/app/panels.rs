//! Chrome around the binding grid: the update banner, notification feed, the central
//! bindings panel (header + legend + balanced grid columns), and the floating footers.
//! Split out of `update()` to keep each file under the size budget.

use super::widgets::{binding_row, mute_chips, Capture, TEXT_MAIN};
use std::collections::HashSet;
use crate::games::{Action, Binding, GameProvider};
use crate::input;
use eframe::egui;
use std::sync::{Arc, Mutex};

type UpdateCell = Arc<Mutex<Option<(String, String)>>>;

/// A floating "Update available" toast pinned TOP-RIGHT (dark card + green LIVE accent).
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

/// Footer overlay: a tiny version stamp floated bottom-right (a real bottom panel gets
/// overpainted by the central columns). Status now lives in the top-right feed.
pub(super) fn footers(ctx: &egui::Context) {
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
}

/// The TOP-RIGHT notification feed (newest at top). Collapsed = a slim pill (green dot +
/// count, "● 3"); expanded = clean dark cards, newest with a bright green accent dot +
/// brighter text, older ones dimmer, each with a `✕` plus a `Clear`/`Collapse` footer.
/// `shifted` drops it below the update banner. Floats over both tabs.
pub(super) fn notif_feed(ctx: &egui::Context, log: &mut Vec<String>, collapsed: &mut bool, shifted: bool) {
    if log.is_empty() { return; }
    let accent = super::widgets::LIVE;
    let card = egui::Color32::from_rgb(30, 34, 46); // same dark fill as the update toast / popups
    let card_dim = egui::Color32::from_rgb(24, 27, 37);
    let rim = egui::Color32::from_rgb(52, 58, 74);
    let top = if shifted { 102.0 } else { 12.0 };

    // Collapsed: a slim rounded pill — green status dot + history count.
    if *collapsed {
        egui::Area::new(egui::Id::new("notif_feed"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, top))
            .show(ctx, |ui| {
                let resp = egui::Frame::none()
                    .fill(card)
                    .stroke(egui::Stroke::new(1.0, rim))
                    .rounding(egui::Rounding::same(13.0)) // high rounding => pill
                    .inner_margin(egui::Margin::symmetric(11.0, 5.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("●").size(10.0).color(accent));
                            ui.add_space(1.0);
                            ui.label(egui::RichText::new(log.len().to_string()).size(12.5).strong()
                                .color(egui::Color32::from_rgb(210, 218, 228)));
                        });
                    })
                    .response
                    .interact(egui::Sense::click())
                    .on_hover_text("Show notifications");
                if resp.clicked() { *collapsed = false; }
            });
        return;
    }

    let mut dismiss: Option<usize> = None;
    let mut clear_all = false;
    egui::Area::new(egui::Id::new("notif_feed"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, top))
        .show(ctx, |ui| {
            ui.set_max_width(314.0);
            // newest first: iterate the tail in reverse, brightest card on top.
            for (depth, msg) in log.iter().rev().take(6).enumerate() {
                let newest = depth == 0;
                let fade = 1.0 - (depth as f32 * 0.13);
                let txt_col = if newest {
                    egui::Color32::from_rgb(230, 236, 244)
                } else {
                    let g = (175.0 * fade).clamp(120.0, 205.0) as u8;
                    egui::Color32::from_rgb(g, g.saturating_add(8), g.saturating_add(20))
                };
                let dot = if newest { accent } else { egui::Color32::from_rgb(92, 100, 118) };
                let stroke = if newest { egui::Stroke::new(1.5, accent) } else { egui::Stroke::new(1.0, rim) };
                let size = if newest { 13.0 } else { 12.0 };
                egui::Frame::none()
                    .fill(if newest { card } else { card_dim })
                    .stroke(stroke)
                    .rounding(egui::Rounding::same(9.0))
                    .inner_margin(egui::Margin::symmetric(11.0, 7.0))
                    .show(ui, |ui| {
                        ui.set_max_width(300.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("●").size(if newest { 9.0 } else { 7.0 }).color(dot));
                            ui.add_space(3.0);
                            let rt = egui::RichText::new(msg.as_str()).size(size).color(txt_col);
                            ui.label(if newest { rt.strong() } else { rt });
                            // subtle dismiss pinned to the right.
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("✕").on_hover_text("Dismiss this notification").clicked() {
                                    dismiss = Some(log.len() - 1 - depth);
                                }
                            });
                        });
                    });
                ui.add_space(5.0);
            }
            // Footer controls: clear the history or collapse the feed back to the pill.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Collapse").on_hover_text("Hide the feed").clicked() { *collapsed = true; }
                ui.add_space(4.0);
                if ui.small_button("Clear").on_hover_text("Dismiss all notifications").clicked() { clear_all = true; }
            });
        });
    if clear_all { log.clear(); *collapsed = true; }
    else if let Some(i) = dismiss { if i < log.len() { log.remove(i); } }
}

/// The central "Cockpit Bindings" panel: header + legend, then the categories laid out
/// across balanced columns with one `binding_row` each (`groups` from `mod.rs`).
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
    vjoy_map: &crate::vjoy_map::VjoyMap,
    groups: &[(String, Vec<usize>)],
    live_muted: &mut HashSet<(u16, u16)>,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if !games[selected].available() {
            ui.centered_and_justified(|ui| { ui.label("This game isn't supported yet — pick MechWarrior 5."); });
            return;
        }
        let p = games[selected].as_ref();
        // Live set of tokens being pressed/moved right now — drives the green glow.
        // Muted devices are skipped so you can test one stick at a time.
        let hot = crate::visual::hot_tokens(devices, p, vjoy_map, live_muted);

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
        legend(ui); // which colour = which physical device
        mute_chips(ui, devices, live_muted); // per-stick LIVE mute (display-only)
        ui.separator();
        // Spread the categories across N balanced columns (by row count) so the whole
        // control map fits with less scrolling; ~500px per column to suit the width.
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

        // ONE outer vertical ScrollArea (bounded by the central panel, so it clips and
        // never paints over the footer) with set_max_width, so ui.columns splits the real
        // width into equal halves — wrapping ui.columns inside a ScrollArea would leave the
        // inner ui horizontally unbounded and land the 2nd column off-screen.
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
                                binding_row(ui, i, actions, rows, capture, devices, p, status, &hot, vjoy_map);
                            }
                        });
                    }
                }
            });
        });
    });
}

/// The device colour legend below the "Cockpit Bindings" heading.
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
        chip(ui, super::widgets::LIVE, "lit = in use");
    });
}
