//! vJoy-aware token resolution for the device diagram.
//!
//! A physical control's DIRECT MW5 token (what the provider emits with no vJoy) is only
//! correct when the vJoy feeder is OFF. While feeding, the whole rig is mirrored onto the
//! single vJoy device, so MW5 actually receives the token the vJoy *Target* emits — the
//! chain is two hops (physical -> vJoy -> MW5), not one. These helpers turn a control's
//! DIRECT token into the token MW5 really gets, using the routing table.
//!
//! Everything here is IDENTITY when vJoy is inactive => the diagram and the live "hot"
//! set are byte-for-byte what they were on the direct path.

use crate::games::GameProvider;
use crate::input::Device;
use crate::vjoy_map::{Source, Target, VjoyMap, VJOY_AXES};
use std::collections::HashMap;

/// Reverse of `vjoy_target_token`: which vJoy `Target` produces this MW5 token (if any).
/// Searches the producible space (buttons 1..32 + the six routed axes) so it can never
/// drift from the forward mapping — the single source of truth stays in `hotas.rs`.
fn token_to_target(token: &str) -> Option<Target> {
    for n in 1..=32u8 {
        if crate::games::mw5::vjoy_target_token(&Target::Button(n)).as_deref() == Some(token) {
            return Some(Target::Button(n));
        }
    }
    for u in VJOY_AXES {
        if crate::games::mw5::vjoy_target_token(&Target::Axis(u)).as_deref() == Some(token) {
            return Some(Target::Axis(u));
        }
    }
    None
}

/// Which physical joystick a bound token actually comes from, for a dim hint next to the
/// chip in the binding grid. If the token is a vJoy-produced token AND the routing table
/// maps something onto that vJoy Target, returns just `"vJoy"` (the user binds vJoy and
/// only cares that it's the vJoy route — no arrow glyph, no physical source name).
/// Otherwise falls back to the connected device whose ROLE matches the token
/// (Joystick_*/Throttle_*), or None when nothing matches.
pub fn token_device(token: &str, vjoy_map: &VjoyMap, devices: &[Device]) -> Option<String> {
    if let Some(target) = token_to_target(token) {
        if vjoy_map.mappings.iter().any(|m| m.target == target) {
            return Some("vJoy".to_string());
        }
    }
    // No vJoy mapping: name the connected device whose registry role matches the token.
    let role = if token.starts_with("Joystick") { crate::games::Role::Joystick }
               else if token.starts_with("Throttle") { crate::games::Role::Throttle }
               else { return None };
    devices.iter().find_map(|d| {
        crate::devices::registry().iter()
            .find(|kd| kd.vid == d.vid && kd.pid == d.pid && kd.role == role)
            .map(|_| d.name.clone())
    })
}

/// The DIRECT MW5 token a physical control emits with no vJoy (the provider's contract).
/// `src` is the physical button BIT (0-based) or axis index; `idx` the device's slot.
fn direct_token(p: &dyn GameProvider, dev: &Device, idx: usize, src: Source) -> Option<String> {
    match src {
        Source::Button(b) => p.button_token(dev, b as u32 + 1, idx), // bit -> 1-based number
        Source::Axis(a) => p.axis_token(dev, a as usize, idx),
        // A combined bipolar pair has no single direct physical token to remap.
        Source::Pair(..) => None,
    }
}

/// The MW5 token a physical control ULTIMATELY produces: if the vJoy feeder is active AND
/// the routing table maps this (device, source), the token comes from the vJoy Target
/// (`vjoy_target_token`); otherwise it falls back to the DIRECT provider token.
pub fn resolved_token(p: &dyn GameProvider, dev: &Device, idx: usize, src: Source, map: &VjoyMap) -> Option<String> {
    if crate::vjoy::is_active() {
        if let Some(m) = map.mappings.iter().find(|m| m.vid == dev.vid && m.pid == dev.pid && m.source == src) {
            return crate::games::mw5::vjoy_target_token(&m.target);
        }
    }
    direct_token(p, dev, idx, src)
}

/// Build a DIRECT-token -> RESOLVED-token lookup for every routed control, so callers
/// keyed on direct tokens (the diagram markers, the live `hot` set) can show/glow the
/// token MW5 actually receives while feeding. EMPTY when vJoy is inactive — so callers
/// short-circuit to their original, unchanged behaviour.
pub fn vjoy_token_remap(p: &dyn GameProvider, devices: &[Device], map: &VjoyMap) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if !crate::vjoy::is_active() {
        return out;
    }
    for m in &map.mappings {
        let Some((idx, dev)) = devices.iter().enumerate().find(|(_, d)| d.vid == m.vid && d.pid == m.pid) else {
            continue;
        };
        if let (Some(direct), Some(resolved)) =
            (direct_token(p, dev, idx, m.source), resolved_token(p, dev, idx, m.source, map))
        {
            if direct != resolved {
                out.insert(direct, resolved);
            }
        }
    }
    out
}
