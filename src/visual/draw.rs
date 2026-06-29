//! Pure painting helpers for the device panel: numbered callouts, hat spokes, and
//! the "is this token live?" test. No state here — every fn just paints into a
//! `&egui::Painter` from coordinates/markers handed in by `super`.

use super::{Marker, ACCENT, HOT};
use eframe::egui;
use std::collections::HashMap;

/// Is `token` currently active (in the live hot set)? Hat markers match any octant.
pub(super) fn token_hot(token: &str, hot: &[String]) -> bool {
    if token.is_empty() { return false; }
    hot.iter().any(|h| h == token || (token == "Joystick_Hat" && h.starts_with("Joystick_Hat")))
}

/// Draw numbered, non-overlapping callouts; a callout turns green when its token
/// is live. Labels stack in the margin nearest each dot, ordered by height.
pub(super) fn draw_callouts(painter: &egui::Painter, img: egui::Rect, markers: &[Marker], hot: &[String], bound: &HashMap<String, String>, remap: &HashMap<String, String>) {
    let font = egui::FontId::proportional(11.0);
    let numfont = egui::FontId::proportional(10.0);

    let mut left: Vec<&Marker> = markers.iter().filter(|m| m.nx < 0.5).collect();
    let mut right: Vec<&Marker> = markers.iter().filter(|m| m.nx >= 0.5).collect();
    left.sort_by(|a, b| a.ny.partial_cmp(&b.ny).unwrap_or(std::cmp::Ordering::Equal));
    right.sort_by(|a, b| a.ny.partial_cmp(&b.ny).unwrap_or(std::cmp::Ordering::Equal));

    let place = |col: &[&Marker], on_left: bool| {
        let n = col.len();
        for (i, mk) in col.iter().enumerate() {
            // Resolve the marker's hardcoded DIRECT token to the token MW5 actually
            // receives: identity unless the vJoy feeder routes this control elsewhere.
            // Both the green glow and the bound-action label key off the resolved token.
            let token = remap.get(mk.token).map(|s| s.as_str()).unwrap_or(mk.token);
            let lit = token_hot(token, hot);
            let col_accent = if lit { HOT } else { ACCENT };
            let dot = img.min + egui::vec2(mk.nx * img.width(), mk.ny * img.height());

            let ry = img.top() + img.height() * (i as f32 + 1.0) / (n as f32 + 1.0);
            // Show WHAT is bound: "<control> · <action>", or "(unbound)" for a
            // bindable control with nothing on it. Reference-only dots (no token)
            // just show their physical name.
            let text = if token.is_empty() {
                mk.label.to_string()
            } else if let Some(action) = bound.get(token) {
                format!("{} · {}", mk.label, action)
            } else {
                format!("{} · (unbound)", mk.label)
            };
            let galley = painter.layout_no_wrap(text, font.clone(), egui::Color32::WHITE);
            let pad = egui::vec2(6.0, 3.0);
            let box_size = galley.size() + pad * 2.0;
            let box_min = if on_left {
                egui::pos2(img.left() + 3.0, ry - box_size.y * 0.5)
            } else {
                egui::pos2(img.right() - 3.0 - box_size.x, ry - box_size.y * 0.5)
            };
            let bg = egui::Rect::from_min_size(box_min, box_size);
            let anchor = if on_left { egui::pos2(bg.right(), bg.center().y) } else { egui::pos2(bg.left(), bg.center().y) };

            painter.line_segment([dot, anchor], egui::Stroke::new(if lit { 2.5 } else { 1.5 }, col_accent));

            // The dot: a numbered badge when the marker has a number, else a small dot.
            if mk.num.is_empty() {
                painter.circle_filled(dot, 4.0, col_accent);
                painter.circle_stroke(dot, 4.0, egui::Stroke::new(1.0, egui::Color32::BLACK));
            } else {
                let r = if lit { 11.0 } else { 9.0 };
                painter.circle_filled(dot, r, col_accent);
                painter.circle_stroke(dot, r, egui::Stroke::new(1.5, egui::Color32::BLACK));
                painter.text(dot, egui::Align2::CENTER_CENTER, mk.num, numfont.clone(), egui::Color32::BLACK);
            }

            painter.rect_filled(bg, 3.0, egui::Color32::from_rgba_unmultiplied(18, 20, 28, 225));
            painter.rect_stroke(bg, 3.0, egui::Stroke::new(if lit { 2.0 } else { 1.0 }, col_accent));
            painter.galley(bg.min + pad, galley, egui::Color32::WHITE);
        }
    };
    place(&left, true);
    place(&right, false);
}

/// Draw a hat as radial spokes (way-count visible); the live octant lights green.
pub(super) fn draw_hats(painter: &egui::Painter, img: egui::Rect, hats: &[(f32, f32, u8)], active_octant: Option<u32>) {
    // direction vectors indexed by octant 1..8 (1=up,2=NE,3=right,...). y is down.
    const OCT: [(f32, f32); 8] = [
        (0.0, -1.0), (0.707, -0.707), (1.0, 0.0), (0.707, 0.707),
        (0.0, 1.0), (-0.707, 0.707), (-1.0, 0.0), (-0.707, -0.707),
    ];
    for &(nx, ny, ways) in hats {
        let c = img.min + egui::vec2(nx * img.width(), ny * img.height());
        let r = 12.0_f32;
        // 4-way & 5-way: cardinals only (octants 1,3,5,7). 8-way: all.
        let octs: &[usize] = if ways >= 8 { &[0, 1, 2, 3, 4, 5, 6, 7] } else { &[0, 2, 4, 6] };
        for &o in octs {
            let (dx, dy) = OCT[o];
            let d = egui::vec2(dx, dy);
            let lit = active_octant == Some(o as u32 + 1);
            let stroke = egui::Stroke::new(if lit { 3.0 } else { 1.5 }, if lit { HOT } else { ACCENT });
            let end = c + d * r;
            painter.line_segment([c, end], stroke);
            let perp = egui::vec2(-dy, dx);
            let back = end - d * 4.0;
            painter.line_segment([end, back + perp * 3.0], stroke);
            painter.line_segment([end, back - perp * 3.0], stroke);
        }
        painter.circle_stroke(c, 3.0, egui::Stroke::new(1.5, ACCENT));
        if ways == 5 { painter.circle_filled(c, 2.0, ACCENT); }
    }
}
