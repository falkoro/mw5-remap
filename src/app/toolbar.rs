//! The top toolbar: the game picker plus every action button (Save, Fix HOTAS,
//! Lock, Export, Hide/Restore, Launch, Run-as-admin). Split out of `panels` so
//! each file stays within the size budget; it touches only the app state passed in.

use super::widgets::{file_name, persist};
use crate::games::{Action, Binding, GameProvider};
use crate::{hidhide, sys};
use eframe::egui;
use std::path::PathBuf;

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
) -> bool {
    let mut reload = false;
    egui::TopBottomPanel::top("top").show(ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
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
            if ui.add_enabled(avail, egui::Button::new("⟳ Load current")).clicked() { reload = true; }
            if ui.add_enabled(avail, egui::Button::new("↺ Reset to defaults"))
                .on_hover_text("Replace the grid with the known-good default layout. Not saved until you click 💾 Save to game.")
                .clicked()
            {
                let defaults: std::collections::HashMap<String, (String, f32)> = games[*selected]
                    .default_bindings().into_iter().map(|b| (b.id, (b.token, b.scale))).collect();
                for r in rows.iter_mut() {
                    match defaults.get(&r.id) {
                        Some((tok, sc)) => { r.token = tok.clone(); r.scale = *sc; }
                        None => { r.token.clear(); r.scale = 1.0; }
                    }
                }
                *status = "Reset to the default layout — review it, then click 💾 Save to game.".into();
            }
            if ui.add_enabled(avail, egui::Button::new("💾 Save to game")).clicked() {
                let p = games[*selected].as_ref();
                if sys::any_process_running(&p.running_processes()) {
                    *status = "Close MW5 first — it overwrites the config on exit.".into();
                } else {
                    match p.save(rows) {
                        Ok(r) => *status = format!("Saved ✓  backup {}  ({} change(s){})",
                            file_name(&r.backup), r.changed.len(),
                            if r.missing.is_empty() { String::new() } else { format!(", {} skipped", r.missing.len()) }),
                        Err(e) => *status = format!("Save failed: {}", e),
                    }
                }
            }
            // MW5 needs a SECOND file (HOTASMappings.Remap) mapping the physical
            // MOZA stick/pedals -> tokens, or the bindings above do nothing in-game.
            // Static (device->token), so this is a run-once button, not per-save.
            if avail && games[*selected].name().contains("MechWarrior")
                && ui.add(egui::Button::new("🎮 Fix HOTAS file"))
                    .on_hover_text("Write SavedHOTAS\\HOTASMappings.Remap so MW5 reads your MOZA AB6 + pedals. Run once (and after plugging in a new stick).")
                    .clicked()
            {
                let p = games[*selected].as_ref();
                if sys::any_process_running(&p.running_processes()) {
                    *status = "Close MW5 first — it overwrites HOTAS mappings on exit.".into();
                } else {
                    match crate::games::mw5::write_hotas_mappings() {
                        Ok(b) => *status = format!("HOTAS file written ✓  MOZA stick+pedals now mapped  (backup {})", file_name(&b)),
                        Err(e) => *status = format!("HOTAS write failed: {}", e),
                    }
                }
            }
            // Lock toggle: MW5 rewrites GameUserSettings (resetting joystick
            // bindings to stock) on launch. Read-only stops that. Trade-off:
            // other in-game settings won't save until unlocked.
            if avail && games[*selected].name().contains("MechWarrior") {
                let locked = crate::games::mw5::config_is_locked();
                let label = if locked { "🔓 Unlock config" } else { "🔒 Lock config" };
                let hover = if locked {
                    "Config is LOCKED so MW5 can't reset your bindings. Click to unlock (lets graphics/audio settings save again)."
                } else {
                    "Make GameUserSettings read-only so MW5 stops resetting your joystick bindings on launch."
                };
                if ui.add(egui::Button::new(label)).on_hover_text(hover).clicked() {
                    match crate::games::mw5::set_config_locked(!locked) {
                        Ok(()) => *status = if locked { "Config unlocked — MW5 can save settings again.".into() }
                                             else { "Config LOCKED ✓  MW5 can no longer reset your bindings.".into() },
                        Err(e) => *status = format!("Lock toggle failed: {}", e),
                    }
                }
            }
            if ui.add_enabled(avail, egui::Button::new("📊 Export diagram"))
                .on_hover_text("Export the device images (with callouts) as PNG and/or PDF").clicked()
            {
                *show_export_dialog = true;
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
        });
        ui.add_space(4.0);
    });
    reload
}
