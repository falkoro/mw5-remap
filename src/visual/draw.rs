//! Pure painting helpers for the device panel: numbered callouts, hat spokes, and
//! the "is this token live?" test. No state here — every fn just paints into a
//! `&egui::Painter` from coordinates/markers handed in by `super`.

use super::{Marker, MultiMarker, ACCENT, HOT};
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

/// Draw multi-input markers: ONE box per control listing every bound input on it
/// (a hat's 8 directions, a rocker's in/out), each row glowing green when its token
/// is live. The leader line + box-border light if ANY row is hot. Tokens resolve
/// through `remap` (vJoy-aware) just like single callouts, so it lights correctly
/// while the vJoy feeder is active too.
pub(super) fn draw_multi_callouts(painter: &egui::Painter, img: egui::Rect, markers: &[MultiMarker], hot: &[String], bound: &HashMap<String, String>, remap: &HashMap<String, String>) {
    let head_font = egui::FontId::proportional(11.0);
    let row_font = egui::FontId::proportional(10.5);
    let pad = egui::vec2(7.0, 5.0);
    let row_gap = 2.0;
    let indent = 13.0; // room for each row's status dot

    for mk in markers {
        let dot = img.min + egui::vec2(mk.nx * img.width(), mk.ny * img.height());

        // Lay out the header + one galley per input (colour baked = green when hot).
        let header = painter.layout_no_wrap(mk.label.to_string(), head_font.clone(), ACCENT);
        let mut rows: Vec<(std::sync::Arc<egui::Galley>, bool)> = Vec::with_capacity(mk.inputs.len());
        let mut any_lit = false;
        for &(sub, raw_token) in mk.inputs {
            let token = remap.get(raw_token).map(|s| s.as_str()).unwrap_or(raw_token);
            let lit = token_hot(token, hot);
            any_lit |= lit;
            let text = match bound.get(token) {
                Some(action) => format!("{sub} · {action}"),
                None => format!("{sub} · (unbound)"),
            };
            let colour = if lit { HOT } else { egui::Color32::from_gray(225) };
            rows.push((painter.layout_no_wrap(text, row_font.clone(), colour), lit));
        }

        // Box geometry: width = widest of header / (indented) rows; height = stacked.
        let mut content_w = header.size().x;
        let mut content_h = header.size().y + row_gap;
        for (g, _) in &rows {
            content_w = content_w.max(g.size().x + indent);
            content_h += g.size().y + row_gap;
        }
        let box_size = egui::vec2(content_w, content_h - row_gap) + pad * 2.0;

        // Sit the box beside the dot (right unless the dot hugs the right margin), then
        // clamp fully inside the image so nothing clips.
        let on_right = mk.nx < 0.62;
        let raw_x = if on_right { dot.x + 16.0 } else { dot.x - 16.0 - box_size.x };
        let bx = raw_x.clamp(img.left() + 2.0, (img.right() - box_size.x - 2.0).max(img.left() + 2.0));
        let by = (dot.y - box_size.y * 0.5).clamp(img.top() + 2.0, (img.bottom() - box_size.y - 2.0).max(img.top() + 2.0));
        let bg = egui::Rect::from_min_size(egui::pos2(bx, by), box_size);

        let accent = if any_lit { HOT } else { ACCENT };
        let anchor = egui::pos2(
            if on_right { bg.left() } else { bg.right() },
            dot.y.clamp(bg.top(), bg.bottom()),
        );
        painter.line_segment([dot, anchor], egui::Stroke::new(if any_lit { 2.5 } else { 1.5 }, accent));
        painter.circle_filled(dot, if any_lit { 5.0 } else { 4.0 }, accent);
        painter.circle_stroke(dot, if any_lit { 5.0 } else { 4.0 }, egui::Stroke::new(1.0, egui::Color32::BLACK));

        painter.rect_filled(bg, 4.0, egui::Color32::from_rgba_unmultiplied(18, 20, 28, 230));
        painter.rect_stroke(bg, 4.0, egui::Stroke::new(if any_lit { 2.0 } else { 1.0 }, accent));

        // Header, then each input row with a left status dot (green when that row is hot).
        let mut y = bg.min.y + pad.y;
        let hx = bg.min.x + pad.x;
        let hh = header.size().y;
        painter.galley(egui::pos2(hx, y), header, ACCENT);
        y += hh + row_gap;
        for (g, lit) in rows {
            let rh = g.size().y;
            let cdot = egui::pos2(hx + 4.0, y + rh * 0.5);
            painter.circle_filled(cdot, 3.0, if lit { HOT } else { egui::Color32::from_gray(90) });
            painter.galley(egui::pos2(hx + indent, y), g, egui::Color32::WHITE);
            y += rh + row_gap;
        }
    }
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
