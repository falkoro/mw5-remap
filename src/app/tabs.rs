//! The two top-level tabs. `bind_tab` is the default press-to-bind editor (device
//! diagram SidePanel + Cockpit Bindings grid + toolbar/footers); `vjoy_setup_tab` is
//! the SOLE home of the vJoy routing UI plus a live connected-stick list. Split out of
//! `mod.rs` so the egui shell stays within the per-module size budget. Each fn takes
//! only the app state it touches (mirrors how `panels`/`toolbar` are wired).

use super::theme;
use super::widgets::{persist, Capture};
use super::vjoy_ui::VjoyCapture;
use super::{community_ui, export_ui, panels, toolbar, vjoy_ui, ExportOpts};
use crate::community::{CommunityState, DownloadState};
use crate::games::{Action, Binding, GameProvider};
use crate::vjoy_map::VjoyMap;
use crate::{hidhide, input};
use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

type UpdateCell = Arc<Mutex<Option<(String, String)>>>;
type CommunityCell = Arc<Mutex<CommunityState>>;
type DownloadCell = Arc<Mutex<DownloadState>>;

/// The default Bind tab: update banner, toolbar, optional community dialog, the
/// left device-diagram SidePanel, the central Cockpit Bindings grid, footers, and
/// the device-sheet export flow. NO vJoy routing here — that lives in the vJoy
/// Setup tab now (vJoy is just a normal DirectInput stick, bound in the grid).
/// Returns `true` if the user requested a reload (game switch / Load current).
#[allow(clippy::too_many_arguments)]
pub(super) fn bind_tab(
    ctx: &egui::Context,
    games: &mut Vec<Box<dyn GameProvider>>,
    selected: &mut usize,
    actions: &[Action],
    rows: &mut Vec<Binding>,
    devices: &[input::Device],
    capture: &mut Option<Capture>,
    status: &mut String,
    elevated: bool,
    hidden: &mut Vec<String>,
    hide_state: &PathBuf,
    textures: &Option<crate::visual::Textures>,
    show_labels: &mut bool,
    update: &UpdateCell,
    show_export_dialog: &mut bool,
    export_opts: &mut ExportOpts,
    pending_export: &mut Option<ExportOpts>,
    export_shot_sent: &mut bool,
    last_panel_rect: &mut egui::Rect,
    profile: &mut String,
    profile_input: &mut String,
    show_community: &mut bool,
    community: &CommunityCell,
    community_dl: &DownloadCell,
    vjoy_map: &VjoyMap,
    bound: &HashMap<String, String>,
    groups: &[(String, Vec<usize>)],
    live_muted: &mut std::collections::HashSet<(u16, u16)>,
    notif_log: &mut Vec<String>,
) -> bool {
    panels::update_banner(ctx, status, update);
    let reload = toolbar::top_bar(ctx, games, selected, rows, actions, status, elevated, hidden, hide_state, show_export_dialog, profile, profile_input, show_community, community, notif_log);
    if *show_community {
        community_ui::dialog(ctx, show_community, community, community_dl, &games[*selected].name().to_string(), status);
    }

    egui::SidePanel::left("devices").resizable(true).default_width(440.0).show(ctx, |ui| {
        *last_panel_rect = ui.max_rect();
        if let Some(tex) = textures.as_ref() {
            let filter = pending_export.as_ref();
            crate::visual::sidebar(ui, tex, devices, games[*selected].as_ref(), show_labels, bound, vjoy_map, live_muted, filter);
        } else {
            ui.label("Loading device images…");
        }
    });

    panels::central(ctx, games, *selected, textures, devices, capture, rows, actions, status, vjoy_map, groups, live_muted);
    panels::footers(ctx);

    // Export device-sheet flow: dialog -> (filtered repaint) -> screenshot -> crop + write.
    // We arm `pending_export` first so the NEXT frame paints the side panel with the
    // chosen device filter, THEN issue the Screenshot command — so the captured frame
    // already reflects the selected devices.
    if *show_export_dialog && export_ui::dialog(ctx, show_export_dialog, export_opts, show_labels) {
        *show_export_dialog = false;
        *pending_export = Some(export_opts.clone());
        *export_shot_sent = false;
        ctx.request_repaint();
    }
    if let Some(opts) = pending_export.clone() {
        if !*export_shot_sent {
            // This frame painted the filtered panel; now capture it.
            *export_shot_sent = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            ctx.request_repaint();
        } else if let Some(shot) = export_ui::take_screenshot(ctx) {
            *pending_export = None;
            *export_shot_sent = false;
            match export_ui::crop_to_image(&shot, *last_panel_rect, ctx.pixels_per_point()) {
                Some(img) => *status = export_ui::write_files(&img, &opts),
                None => *status = "Export failed: device panel not visible.".into(),
            }
        } else {
            ctx.request_repaint(); // wait for the screenshot event to arrive
        }
    }
    reload
}

/// The vJoy Setup tab: the routing panel (auto-route / capture-bind / combine /
/// mappings list) plus a short live list of connected sticks so the user sees what
/// they can route. This is the sole home of the routing UI.
#[allow(clippy::too_many_arguments)]
pub(super) fn vjoy_setup_tab(
    ctx: &egui::Context,
    devices: &[input::Device],
    vjoy_map: &mut VjoyMap,
    vjoy_capture: &mut Option<VjoyCapture>,
    vjoy_sel: &mut Option<(u16, u16)>,
    vjoy_btn_pick: &mut u8,
    vjoy_axis_pick: &mut u32,
    vjoy_pair_fwd: &mut u8,
    vjoy_pair_rev: &mut u8,
    vjoy_paused: &mut bool,
    status: &mut String,
    elevated: bool,
    hidden: &mut Vec<String>,
    hide_state: &PathBuf,
) {
    vjoy_ui::panel(ctx, devices, vjoy_map, vjoy_capture, vjoy_sel, vjoy_btn_pick, vjoy_axis_pick, vjoy_pair_fwd, vjoy_pair_rev, vjoy_paused, status);
    let frame = egui::Frame::central_panel(&ctx.style()).fill(theme::BG).inner_margin(egui::Margin::symmetric(14.0, 10.0));
    egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
        ui.label(egui::RichText::new("How it works").strong().size(12.0).color(theme::ACCENT_DK));
        ui.label(egui::RichText::new(
            "Route any connected stick onto the virtual vJoy device above, then bind vJoy's buttons/axes over in the 🎮 Bind tab — it shows up as a normal joystick.",
        ).color(theme::TEXT_DIM));
        ui.add_space(10.0);

        egui::Frame::none()
            .fill(theme::CARD)
            .stroke(egui::Stroke::new(1.0, theme::RIM))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("CONNECTED STICKS").strong().size(11.5).color(theme::ACCENT_DK));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(
                    "Hide a physical stick from games so MW5 sees only vJoy. This app stays whitelisted in HidHide, so it keeps reading the hidden stick to feed vJoy.",
                ).size(11.5).color(theme::TEXT_DIM));
                if !elevated {
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("⚠ Run as admin to hide devices (use the “Run as admin” button in the Bind tab).")
                        .size(11.5).color(theme::CAP_DK));
                }
                ui.add_space(6.0);
                if devices.is_empty() {
                    ui.label(egui::RichText::new("No controllers detected.").color(theme::TEXT_DIM));
                } else {
                    for d in devices {
                        stick_row(ui, d, elevated, hidden, hide_state, status);
                    }
                }
            });
    });
}

/// vJoy enumerates under this VID; MW5 MUST keep seeing it, so it's never hideable.
const VJOY_VID: u16 = 0x1234;

/// One line in the "Connected sticks" list: the device, plus a per-device
/// Hide-from-MW5 / Show toggle (HidHide). Hiding is VID-level (HidHide cloaks by
/// device instance, which we select via the stick's VID); reflects the live hidden
/// state by checking whether any cloaked path carries this VID.
fn stick_row(
    ui: &mut egui::Ui,
    d: &input::Device,
    elevated: bool,
    hidden: &mut Vec<String>,
    hide_state: &PathBuf,
    status: &mut String,
) {
    let is_vjoy = d.vid == VJOY_VID;
    let tag = format!("VID_{:04X}", d.vid);
    let is_hidden = !is_vjoy && hidden.iter().any(|p| p.to_uppercase().contains(&tag));
    egui::Frame::none()
        .fill(theme::CARD_ALT)
        .stroke(egui::Stroke::new(1.0, if is_hidden { theme::ACCENT } else { theme::RIM }))
        .rounding(egui::Rounding::same(7.0))
        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| {
    ui.horizontal(|ui| {
        let dot = if is_vjoy { theme::ACCENT } else if is_hidden { theme::TEXT_FAINT } else { theme::STICK };
        theme::dot(ui, dot, 10.0);
        ui.label(egui::RichText::new(&d.name).strong().color(theme::TEXT));
        ui.label(egui::RichText::new(format!("{:04X}:{:04X}", d.vid, d.pid)).size(11.0).color(theme::TEXT_DIM));
        if is_vjoy {
            ui.label(egui::RichText::new("← vJoy (MW5 must see this)").size(11.5).color(theme::ACCENT_DK));
            return;
        }
        if is_hidden {
            ui.label(egui::RichText::new("hidden from MW5").size(11.0).color(theme::ACCENT_DK));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if !elevated {
            ui.add_enabled(false, egui::Button::new("Hide from MW5"));
        } else if is_hidden {
            if ui.button("Show").on_hover_text("Un-cloak this stick so games see it again").clicked() {
                let mine: Vec<String> = hidden.iter().filter(|p| p.to_uppercase().contains(&tag)).cloned().collect();
                let n = hidhide::unhide(&mine);
                hidden.retain(|p| !p.to_uppercase().contains(&tag));
                if hidden.is_empty() { hidhide::set_cloak(false); } // nothing left cloaked
                persist(hide_state, hidden);
                *status = format!("Restored {n} device(s).");
            }
        } else if ui.button("Hide from MW5")
            .on_hover_text("Cloak this stick from games via HidHide so MW5 sees only vJoy")
            .clicked()
        {
            match hidhide::hide(&[format!("{:04X}", d.vid)]) {
                Ok(r) => {
                    for p in &r.hidden { if !hidden.contains(p) { hidden.push(p.clone()); } }
                    persist(hide_state, hidden);
                    *status = r.message;
                }
                Err(e) => *status = e,
            }
        }
        }); // right_to_left (hide/show controls)
    });
        });
    ui.add_space(5.0);
}
