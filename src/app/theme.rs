//! THE design system. One refined-light palette + a global egui `Visuals` setup (so panels,
//! popups, combos, scrollbars and text edits all share one look) + the shared building blocks
//! (`card`, `section`, `pill_button`, `chip`, `status_pill`). EVERY widget pulls its colours
//! from here — nothing hardcodes colours elsewhere. See the `improve-ui` skill for the rules.

use eframe::egui::{self, Color32, Margin, Response, Rounding, Stroke, Vec2};

// ── Refined light palette ───────────────────────────────────────────────────────────────
pub const BG: Color32 = Color32::from_rgb(236, 238, 243); // window / panel backdrop
pub const SURFACE: Color32 = Color32::from_rgb(247, 248, 251); // raised surface (toolbar, headers)
pub const CARD: Color32 = Color32::from_rgb(255, 255, 255); // cards, chips, popups
pub const CARD_ALT: Color32 = Color32::from_rgb(240, 242, 247); // striped rows / the unbound "slot"
pub const RIM: Color32 = Color32::from_rgb(214, 218, 227); // hairline borders
pub const RIM_STRONG: Color32 = Color32::from_rgb(193, 199, 211);

pub const ACCENT: Color32 = Color32::from_rgb(38, 184, 104); // LIVE green (fills)
pub const ACCENT_DK: Color32 = Color32::from_rgb(21, 130, 73); // green text/lines on light
pub const STICK: Color32 = Color32::from_rgb(70, 132, 210); // Joystick-role device
pub const THROTTLE: Color32 = Color32::from_rgb(214, 136, 52); // Throttle-role device
pub const CAPTURING: Color32 = Color32::from_rgb(226, 150, 32); // "listening" (amber)
pub const CAP_DK: Color32 = Color32::from_rgb(168, 104, 0); // readable dark amber on light (capturing label / warnings)

pub const TEXT: Color32 = Color32::from_rgb(38, 43, 56); // primary
pub const TEXT_DIM: Color32 = Color32::from_rgb(108, 116, 132); // secondary
pub const TEXT_FAINT: Color32 = Color32::from_rgb(150, 158, 174); // faint hints
pub const ON_ACCENT: Color32 = Color32::from_rgb(255, 255, 255); // text on a filled accent chip
pub const DANGER: Color32 = Color32::from_rgb(196, 64, 60); // error text on light

pub const R_CHIP: f32 = 7.0;
pub const R_CARD: f32 = 9.0;
pub const CHIP_H: f32 = 28.0; // binding chip height; the row's clear/invert/scale cells align to it

// AC7/SC palette: a stable colour per physical device id (used by `device_color`).
pub const DEV_PALETTE: [Color32; 6] = [
    STICK, THROTTLE,
    Color32::from_rgb(96, 178, 110), Color32::from_rgb(170, 110, 198),
    Color32::from_rgb(206, 96, 104), Color32::from_rgb(86, 176, 190),
];

/// Lighten a colour toward white by `t` (0..1) — soft role-tinted chip fills, gloss, etc.
pub fn tint(c: Color32, t: f32) -> Color32 {
    let m = |a: u8| (a as f32 + (255.0 - a as f32) * t) as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// A painted filled circle (a status dot). Font-independent, so it never tofus like a "●"
/// glyph does in egui's default font. Use this everywhere a coloured status dot is wanted.
pub fn dot(ui: &mut egui::Ui, color: Color32, d: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(d), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), d * 0.5, color);
}

/// A small, FONT-SAFE "clear" (×) button: the cross is PAINTED (two strokes), so it
/// never renders as a tofu □ the way a "✕" glyph does in egui's default font. Quiet by
/// default (dim), danger-tinted on hover. Returns the click `Response`. Use it wherever
/// a one-tap "remove this" control is wanted in a row.
pub fn clear_button(ui: &mut egui::Ui) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
    let hov = resp.hovered();
    let p = ui.painter();
    if hov {
        p.rect_filled(rect.shrink(1.0), Rounding::same(R_CHIP), tint(DANGER, 0.8));
    }
    let c = rect.center();
    let r = 4.5;
    let s = Stroke::new(1.7, if hov { DANGER } else { TEXT_DIM });
    p.line_segment([c + Vec2::new(-r, -r), c + Vec2::new(r, r)], s);
    p.line_segment([c + Vec2::new(r, -r), c + Vec2::new(-r, r)], s);
    resp
}

/// A small, FONT-SAFE up/down reorder arrow (the triangle is PAINTED, never a tofu glyph),
/// styled to match `clear_button`: quiet by default, accent-tinted on hover. `enabled=false`
/// greys it out (the caller ignores its click). Used to move a device image up/down in the
/// sidebar. Returns the click `Response`.
pub fn arrow_button(ui: &mut egui::Ui, up: bool, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
    let hov = enabled && resp.hovered();
    let p = ui.painter();
    if hov {
        p.rect_filled(rect.shrink(1.0), Rounding::same(R_CHIP), tint(ACCENT, 0.72));
    }
    let c = rect.center();
    let col = if !enabled { RIM_STRONG } else if hov { ACCENT_DK } else { TEXT_DIM };
    let (w, h) = (4.0, 3.0);
    let pts = if up {
        vec![c + Vec2::new(-w, h), c + Vec2::new(w, h), c + Vec2::new(0.0, -h)]
    } else {
        vec![c + Vec2::new(-w, -h), c + Vec2::new(w, -h), c + Vec2::new(0.0, h)]
    };
    p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
    resp
}

// ── Global visuals ──────────────────────────────────────────────────────────────────────
/// Apply the theme to the whole context (call once per frame; cheap + idempotent). This is
/// what makes the central panel, the toolbar, combo-boxes, popups and scrollbars cohere.
pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.override_text_color = Some(TEXT);
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.window_stroke = Stroke::new(1.0, RIM);
    v.window_rounding = Rounding::same(R_CARD);
    v.menu_rounding = Rounding::same(R_CARD);
    v.popup_shadow.color = Color32::from_black_alpha(40);
    v.window_shadow.color = Color32::from_black_alpha(40);
    v.extreme_bg_color = CARD_ALT; // text-edit backgrounds
    v.faint_bg_color = CARD_ALT; // striped grid rows
    v.selection.bg_fill = tint(ACCENT, 0.6);
    v.selection.stroke = Stroke::new(1.0, ACCENT_DK);
    v.hyperlink_color = ACCENT_DK;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0, RIM);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    for ws in [&mut w.inactive, &mut w.hovered, &mut w.active] {
        ws.rounding = Rounding::same(R_CHIP);
    }
    w.inactive.bg_fill = CARD;
    w.inactive.weak_bg_fill = CARD;
    w.inactive.bg_stroke = Stroke::new(1.0, RIM);
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.hovered.bg_fill = SURFACE;
    w.hovered.weak_bg_fill = SURFACE;
    w.hovered.bg_stroke = Stroke::new(1.0, RIM_STRONG);
    w.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    w.active.bg_fill = tint(ACCENT, 0.45);
    w.active.weak_bg_fill = tint(ACCENT, 0.45);
    w.active.bg_stroke = Stroke::new(1.0, ACCENT_DK);
    w.active.fg_stroke = Stroke::new(1.0, TEXT);
    ctx.set_visuals(v);
}

// ── Building blocks ─────────────────────────────────────────────────────────────────────
/// A white card with a soft rim — the base container for grouped content.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, RIM))
        .rounding(Rounding::same(R_CARD))
        .inner_margin(Margin::symmetric(11.0, 9.0))
        .show(ui, add)
        .inner
}

/// A titled card section (accent caption above the body).
pub fn section<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card(ui, |ui| {
        ui.label(egui::RichText::new(title).strong().size(11.5).color(ACCENT_DK));
        ui.add_space(5.0);
        add(ui)
    })
}

/// A pill button: `accent` = filled green call-to-action, else a quiet white button.
pub fn pill_button(ui: &mut egui::Ui, enabled: bool, text: &str, accent: bool) -> Response {
    let (fill, txt, rim) = if accent { (ACCENT, ON_ACCENT, ACCENT_DK) } else { (CARD, TEXT, RIM) };
    let b = egui::Button::new(egui::RichText::new(text).strong().color(txt))
        .fill(fill)
        .stroke(Stroke::new(1.0, rim))
        .rounding(Rounding::same(R_CHIP));
    ui.add_enabled(enabled, b)
}

/// State of a binding chip — drives its fill/border/text in one place.
#[derive(Clone, Copy)]
pub enum ChipState {
    Unbound,
    Capturing,
    Live,
    Bound(Color32), // carries the device/role colour
}

/// The binding chip: one clean, rounded, role-aware button. Unbound = a quiet grey slot;
/// bound = a soft role-tinted card with a role border; LIVE = a vivid green fill that can't
/// be missed; capturing = amber. Returns the `Response` (click = re-bind). No glow-spam.
pub fn chip(ui: &mut egui::Ui, text: &str, state: ChipState) -> Response {
    let (fill, txt, border, bw) = match state {
        ChipState::Unbound => (CARD_ALT, TEXT_FAINT, RIM, 1.0),
        ChipState::Capturing => (tint(CAPTURING, 0.55), TEXT, CAPTURING, 1.5),
        ChipState::Live => (ACCENT, ON_ACCENT, ACCENT_DK, 2.0),
        ChipState::Bound(role) => (tint(role, 0.62), TEXT, role, 1.5),
    };
    let b = egui::Button::new(egui::RichText::new(text).color(txt).strong().size(13.5))
        .fill(fill)
        .stroke(Stroke::new(bw, border))
        .rounding(Rounding::same(R_CHIP))
        .min_size(Vec2::new(150.0, CHIP_H));
    let resp = ui.add(b);
    // LIVE: one soft outer ring so an active control reads as a glowing chip at a glance.
    if matches!(state, ChipState::Live) {
        ui.painter().rect_stroke(
            resp.rect.expand(2.5),
            Rounding::same(R_CHIP + 2.5),
            Stroke::new(2.0, Color32::from_rgba_unmultiplied(38, 184, 104, 90)),
        );
    }
    resp
}

/// The live "vJoy feeding / idle · driver-state" pill (vJoy Setup header).
pub fn status_pill(ui: &mut egui::Ui, paused: bool, map: &crate::vjoy_map::VjoyMap, vjoy_ok: bool) {
    let active = vjoy_ok && !paused && !map.mappings.is_empty();
    let drv = match crate::vjoy::status() { 0 => "own", 1 => "free", 2 => "busy", 3 => "miss", _ => "?" };
    let (dotc, label) = if active { (ACCENT, "feeding") } else { (TEXT_FAINT, "idle") };
    egui::Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, RIM))
        .rounding(Rounding::same(12.0))
        .inner_margin(Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                dot(ui, dotc, 10.0);
                ui.label(egui::RichText::new(format!("vJoy {label}")).strong().color(TEXT));
                ui.label(egui::RichText::new(format!("· {drv}")).size(11.0).color(TEXT_DIM));
            });
        });
}

/// Which colour identifies the device a token belongs to (role for MW5; per-id for AC7/SC).
pub fn device_color(token: &str) -> Color32 {
    if token.starts_with("Throttle") {
        THROTTLE
    } else if token.starts_with("Joystick") {
        STICK
    } else if let Some((id, _)) = token.split_once('|') {
        let h = id.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
        DEV_PALETTE[(h as usize) % DEV_PALETTE.len()]
    } else {
        TEXT_FAINT
    }
}
