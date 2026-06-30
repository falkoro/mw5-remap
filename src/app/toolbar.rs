//! The top toolbar: the game picker plus every action button (Save — which for
//! MW5 also writes HOTASMappings.Remap, Export, Hide/Restore, Launch, Run-as-admin).
//! Split out of `panels` so
//! each file stays within the size budget; it touches only the app state passed in.

use super::widgets::{file_name, persist};
use crate::community::CommunityState;
use crate::games::{Action, Binding, GameProvider};
use crate::{hidhide, sys};
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// The top toolbar: game picker plus every action button. Returns `true` if the
/// user requested a reload (game switch or "Load current"), which the caller acts
/// on after the frame.
#[allow(clippy::too_many_arguments)]
pub(super) fn top_bar(
    ctx: &egui::Context,
    games: &[Box<dyn GameProvider>],
    selected: &mut usize,
    rows: &mut [Binding],
    _actions: &[Action],
    status: &mut String,
    elevated: bool,
    hidden: &mut Vec<String>,
    hide_state: &PathBuf,
    show_export_dialog: &mut bool,
    profile: &mut String,
    profile_input: &mut String,
    show_community: &mut bool,
    community: &Arc<Mutex<CommunityState>>,
    notif_log: &mut Vec<String>,
) -> bool {
    let mut reload = false;
    egui::TopBottomPanel::top("top").show(ctx, |ui| {
        ui.add_space(4.0);
        // horizontal_wrapped: on a narrow window the buttons flow onto a second row
        // instead of clipping, so every control stays reachable.
        ui.horizontal_wrapped(|ui| {
            // Consistent gaps so the wrapped rows don't read as ragged (esp. the vertical
            // gap when buttons flow onto a second line on a narrow window).
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            ui.label("Game:");
            let mut want = *selected;
            egui::ComboBox::from_id_salt("game")
                .selected_text(games[*selected].name().to_string())
                .show_ui(ui, |ui| {
                    for (i, g) in games.iter().enumerate() {
                        ui.selectable_value(&mut want, i, g.name());
                    }
                });
            if want != *selected { *selected = want; reload = true; }

            let avail = games[*selected].available();
            ui.separator();
            if ui.add_enabled(avail, egui::Button::new("Load current")).clicked() { reload = true; }

            // Profiles: pick the built-in "App Defaults" (= reset) or a saved layout to
            // fill the grid, save the current grid as a named profile, or delete one.
            // Loading only fills the grid — 💾 Save to game still writes it.
            let game_name = games[*selected].name().to_string();
            ui.label("Profile:");
            let mut pick = profile.clone();
            egui::ComboBox::from_id_salt("profile").selected_text(profile.as_str()).show_ui(ui, |ui| {
                ui.selectable_value(&mut pick, crate::profiles::APP_DEFAULTS.to_string(), crate::profiles::APP_DEFAULTS);
                for name in crate::profiles::list(&game_name) {
                    ui.selectable_value(&mut pick, name.clone(), name);
                }
            });
            if avail && pick != *profile {
                *profile = pick.clone();
                let from = if pick == crate::profiles::APP_DEFAULTS {
                    games[*selected].default_bindings()
                } else {
                    crate::profiles::load(&game_name, &pick).unwrap_or_default()
                };
                crate::profiles::apply(rows, &from);
                *status = format!("Loaded profile \"{pick}\" — review, then 💾 Save to game.");
            }
            ui.add(egui::TextEdit::singleline(profile_input).hint_text("new profile name").desired_width(110.0));
            if ui.add_enabled(avail, egui::Button::new("💾 Save as"))
                .on_hover_text("Save the current grid as a named profile (shareable text file)").clicked()
            {
                match crate::profiles::save(&game_name, profile_input, rows) {
                    Ok(()) => { *profile = crate::profiles::safe_name(profile_input); *status = format!("Saved profile \"{}\".", profile); profile_input.clear(); }
                    Err(e) => *status = e,
                }
            }
            if avail && *profile != crate::profiles::APP_DEFAULTS
                && ui.button("🗑").on_hover_text("Delete the selected profile").clicked()
            {
                match crate::profiles::delete(&game_name, profile) {
                    Ok(()) => { *status = format!("Deleted profile \"{}\".", profile); *profile = crate::profiles::APP_DEFAULTS.to_string(); }
                    Err(e) => *status = e,
                }
            }
            // ONE Save: writes GameUserSettings (token->action) AND, for MW5, the
            // SECOND file HOTASMappings.Remap (physical device->token). Both are
            // required in-game, so they're a single action now (no separate button).
            if ui.add_enabled(avail, egui::Button::new("💾 Save to game"))
                .on_hover_text("Writes your bindings to the game. For MW5 this writes BOTH files (GameUserSettings + HOTASMappings.Remap).")
                .clicked()
            {
                let p = games[*selected].as_ref();
                if sys::any_process_running(&p.running_processes()) {
                    *status = "Close MW5 first — it overwrites the config on exit.".into();
                } else {
                    match p.save(rows) {
                        Ok(r) => {
                            let hotas = if games[*selected].name().contains("MechWarrior") {
                                match crate::games::mw5::write_hotas_mappings() {
                                    Ok(_) => "  + HOTAS file ✓".to_string(),
                                    Err(e) => format!("  (HOTAS write failed: {e})"),
                                }
                            } else { String::new() };
                            *status = format!("Saved ✓{}  backup {}  ({} change(s){})",
                                hotas, file_name(&r.backup), r.changed.len(),
                                if r.missing.is_empty() { String::new() } else { format!(", {} skipped", r.missing.len()) });
                        }
                        Err(e) => *status = format!("Save failed: {}", e),
                    }
                }
            }
            // vJoy routing now lives in its own "🕹 vJoy Setup" tab (src/app/tabs.rs ->
            // vjoy_ui.rs): config-driven, any stick -> vJoy, no routing UI in this view.
            if ui.add_enabled(avail, egui::Button::new("📊 Export diagram"))
                .on_hover_text("Export the device images (with callouts) as PNG and/or PDF").clicked()
            {
                *show_export_dialog = true;
            }
            if ui.button("🌐 Community profiles")
                .on_hover_text("Browse & download binding profiles shared by other players").clicked()
            {
                *show_community = true;
                crate::community::start_load(community, &game_name);
            }
            ui.separator();
            let p = games[*selected].as_ref();
            if ui.add_enabled(elevated, egui::Button::new("🛡 Hide conflicts")).clicked() {
                match hidhide::hide(&p.conflict_vids()) {
                    Ok(r) => { *hidden = r.hidden.clone(); persist(hide_state, hidden); *status = r.message; }
                    Err(e) => *status = e,
                }
            }
            if ui.add_enabled(elevated && !hidden.is_empty(), egui::Button::new("Restore devices")).clicked() {
                let n = hidhide::restore(hidden).unwrap_or(0);
                hidden.clear();
                let _ = std::fs::remove_file(hide_state);
                *status = format!("Restored {} device(s).", n);
            }
            if let Some(uri) = p.launch_uri() {
                if ui.add_enabled(avail, egui::Button::new("▶ Launch")).clicked() {
                    let mut ok = true;
                    if !sys::any_process_running(&p.running_processes()) {
                        if let Err(e) = p.save(rows) { *status = format!("Save before launch failed: {}", e); ok = false; }
                    }
                    if ok {
                        if elevated { if let Ok(r) = hidhide::hide(&p.conflict_vids()) { *hidden = r.hidden.clone(); persist(hide_state, hidden); } }
                        sys::open_uri(&uri);
                        *status = "Launching… keep this app open; closing it frees hidden devices.".into();
                    }
                }
            }
            if !elevated {
                ui.separator();
                if ui.button("Run as admin").on_hover_text("Needed for Hide/Restore").clicked() {
                    if sys::relaunch_elevated() { std::process::exit(0); }
                }
            }
            // Inline notification indicator — sits at the right end of the toolbar,
            // next to the elevation control. Replaces the old floating top-right feed.
            ui.separator();
            super::panels::notif_chip(ui, notif_log);
        });
        ui.add_space(4.0);
    });
    reload
}
