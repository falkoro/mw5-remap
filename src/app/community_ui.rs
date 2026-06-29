//! The "🌐 Community profiles" modal: lists shared binding profiles fetched from a
//! GitHub repo (loaded off-thread into `CommunityState`) and lets the user download
//! one into their local profiles folder. Mirrors the `export_ui` dialog style.

use crate::community::{self, CommunityState};
use eframe::egui;
use std::sync::{Arc, Mutex};

/// Draw the community-profiles window. Renders whatever the worker thread has put in
/// `state` ("Loading…" until the list arrives); on a Download click it writes the
/// profile and sets `status`. Closing just hides the window.
pub fn dialog(
    ctx: &egui::Context,
    open: &mut bool,
    state: &Arc<Mutex<CommunityState>>,
    game: &str,
    status: &mut String,
) {
    let mut keep_open = *open;
    let mut to_download: Option<(String, String)> = None;
    egui::Window::new("🌐 Community profiles")
        .open(&mut keep_open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(400.0);
            ui.label(format!("Shared binding profiles for {game}:"));
            ui.add_space(6.0);

            let snapshot = state.lock().unwrap().clone();
            match snapshot {
                CommunityState::Idle | CommunityState::Loading => {
                    ui.horizontal(|ui| { ui.spinner(); ui.label("Loading…"); });
                }
                CommunityState::Failed(e) => {
                    ui.colored_label(egui::Color32::from_rgb(230, 120, 120), e);
                }
                CommunityState::Loaded(list) if list.is_empty() => {
                    ui.label("No community profiles yet — be the first to share!");
                    ui.add_space(2.0);
                    ui.hyperlink_to(
                        format!("Open a PR to {}", community::share_url()),
                        format!("https://{}", community::share_url()),
                    );
                }
                CommunityState::Loaded(list) => {
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for (name, url) in &list {
                            ui.horizontal(|ui| {
                                if ui.button("⬇ Download").clicked() {
                                    to_download = Some((name.clone(), url.clone()));
                                }
                                ui.label(name);
                            });
                        }
                    });
                }
            }

            ui.add_space(8.0);
            ui.separator();
            ui.label("Share yours: 💾 Save a profile, then open a PR adding the");
            ui.label(format!("'{}.profile' file under the game's folder at:", crate::profiles::game_key(game)));
            ui.hyperlink_to(
                community::share_url(),
                format!("https://{}", community::share_url()),
            );
        });

    if let Some((name, url)) = to_download {
        match community::download(&name, &url, game) {
            Ok(n) => *status = format!("Downloaded \"{n}\" — pick it in the Profile dropdown."),
            Err(e) => *status = format!("Download failed: {e}"),
        }
    }
    *open = keep_open;
}
