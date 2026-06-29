//! Shared on-brand styling for the vJoy Setup tab (the routing panel + the connected-
//! sticks list): a dark palette with the app's LIVE-green accent, plus the small "card"
//! section, pill button, and live-status pill building blocks. Kept separate so both
//! `vjoy_ui` and `tabs` reuse one look and each stays within the module size budget.

use crate::vjoy_map::VjoyMap;
use eframe::egui;

pub(super) const ACCENT: egui::Color32 = egui::Color32::from_rgb(70, 210, 110);
pub(super) const BG: egui::Color32 = egui::Color32::from_rgb(22, 25, 34);
pub(super) const CARD: egui::Color32 = egui::Color32::from_rgb(32, 36, 48);
pub(super) const CARD2: egui::Color32 = egui::Color32::from_rgb(40, 45, 59);
pub(super) const RIM: egui::Color32 = egui::Color32::from_rgb(52, 58, 74);
pub(super) const MUTED: egui::Color32 = egui::Color32::from_rgb(150, 165, 190);
pub(super) const TXT: egui::Color32 = egui::Color32::from_rgb(214, 222, 232);

/// A titled dark "card" section — the building block of the redesigned vJoy panel.
pub(super) fn section<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0, RIM))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(11.0, 8.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).strong().size(11.5).color(ACCENT));
            ui.add_space(5.0);
            add(ui)
        })
        .inner
}

/// A styled pill button: `accent` = filled green call-to-action, else a quiet dark button.
pub(super) fn pill_button(ui: &mut egui::Ui, enabled: bool, text: &str, accent: bool) -> egui::Response {
    let (fill, txt) = if accent { (ACCENT, egui::Color32::from_rgb(12, 20, 14)) } else { (egui::Color32::from_rgb(42, 47, 61), TXT) };
    let b = egui::Button::new(egui::RichText::new(text).strong().color(txt))
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, RIM))
        .rounding(egui::Rounding::same(6.0));
    ui.add_enabled(enabled, b)
}

/// The live "vJoy feeding / idle · driver-state" pill shown at the top-right of the header.
pub(super) fn status_pill(ui: &mut egui::Ui, paused: bool, map: &VjoyMap, vjoy_ok: bool) {
    let active = vjoy_ok && !paused && !map.mappings.is_empty();
    let drv = match crate::vjoy::status() { 0 => "own", 1 => "free", 2 => "busy", 3 => "miss", _ => "?" };
    let (dot, label) = if active { (ACCENT, "feeding") } else { (MUTED, "idle") };
    egui::Frame::none()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0, RIM))
        .rounding(egui::Rounding::same(12.0))
        .inner_margin(egui::Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("●").size(10.0).color(dot));
                ui.label(egui::RichText::new(format!("vJoy {label}")).strong().color(TXT));
                ui.label(egui::RichText::new(format!("· {drv}")).size(11.0).color(MUTED));
            });
        });
}
