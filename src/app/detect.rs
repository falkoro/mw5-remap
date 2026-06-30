//! The live "Detected:" readout shown under the tab bar: which physical control is
//! actuated right now, resolved through the vJoy routing table to the vJoy target +
//! the bound MW5 action. Split out of `widgets` so that module stays within the size
//! budget. Buttons + hat only — NO axis detection: the app feeds the vJoy device's
//! axes every frame and idle/noisy physical axes jitter, so a most-moved-axis test
//! false-positives constantly.

use super::widgets::pretty_token;
use crate::games::GameProvider;
use crate::input;
use crate::vjoy_map::{Source, Target, VjoyMap};
use std::collections::{HashMap, HashSet};

const HAT_ARROWS: [&str; 8] = ["↑", "↗", "→", "↘", "↓", "↙", "←", "↖"];

/// The body that follows "Detected: " — e.g. `MOZA AB6 — Button 5  →  vJoy Button 5 ·
/// Fire Weapon Group 1`. When the pressed control is ROUTED in the vJoy table we ALWAYS
/// show its vJoy target (even for a 1:1 map); otherwise we show the direct provider token.
/// None when nothing is actuated.
pub(super) fn detect_input(
    devices: &[input::Device],
    muted: &HashSet<(u16, u16)>,
    p: &dyn GameProvider,
    vjoy_map: &VjoyMap,
    bound: &HashMap<String, String>,
) -> Option<String> {
    for (idx, d) in devices.iter().enumerate() {
        if muted.contains(&(d.vid, d.pid)) { continue; } // soft-muted from the LIVE display
        if (d.vid, d.pid) == (0x1234, 0xBEAD) { continue; } // skip the vJoy device: the app FEEDS it,
        // so it mirrors the physical press — detecting it too is the confusing "double" readout.
        if let Some(&b) = d.pressed_buttons().first() {
            let head = format!("{} — Button {}", d.name, b);
            let src = Source::Button(b.saturating_sub(1) as u8);
            return Some(with_result(head, p.button_token(d, b, idx), routed(d, src, vjoy_map), bound));
        }
        if let Some(oct) = d.pov_octant() {
            let head = format!("{} — Hat {}", d.name, HAT_ARROWS[(oct as usize - 1) & 7]);
            return Some(with_result(head, p.pov_token(d, oct, idx), routed(d, Source::Pov, vjoy_map), bound));
        }
    }
    None
}

/// The vJoy `Target` this physical `(device, source)` is routed onto, if a mapping exists.
/// Based on the MAPPING EXISTING — independent of `vjoy::is_active()` — so the Detected
/// line shows the route even when the routed token is identical to the direct one.
fn routed(d: &input::Device, src: Source, vjoy_map: &VjoyMap) -> Option<Target> {
    vjoy_map.mappings.iter()
        .find(|m| m.vid == d.vid && m.pid == d.pid && m.source == src)
        .map(|m| m.target)
}

/// Append the RESULT to a physical-control readout. A ROUTED control ALWAYS shows
/// `→ vJoy {target}` (even a 1:1 map), then `· {action}` when the resolved MW5 token is
/// bound. An unrouted control falls back to the direct provider token + its action.
fn with_result(head: String, direct: Option<String>, target: Option<Target>, bound: &HashMap<String, String>) -> String {
    if let Some(t) = target {
        let name = match t {
            Target::Button(n) => format!("Button {n}"),
            Target::Axis(u) => crate::vjoy_map::axis_name(u),
            Target::Pov => "POV".into(),
        };
        let action = crate::games::mw5::vjoy_target_token(&t).and_then(|tok| bound.get(&tok).cloned());
        return match action {
            Some(a) => format!("{head}  →  vJoy {name} · {a}"),
            None => format!("{head}  →  vJoy {name}"),
        };
    }
    let tok = match direct { Some(t) if !t.is_empty() => t, _ => return head };
    match bound.get(&tok) {
        Some(a) => format!("{head}  →  {} · {a}", pretty_token(&tok)),
        None => format!("{head}  →  {}", pretty_token(&tok)),
    }
}
