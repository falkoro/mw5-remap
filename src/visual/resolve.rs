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
use crate::vjoy_map::{Source, VjoyMap};
use std::collections::HashMap;

/// The DIRECT MW5 token a physical control emits with no vJoy (the provider's contract).
/// `src` is the physical button BIT (0-based) or axis index; `idx` the device's slot.
fn direct_token(p: &dyn GameProvider, dev: &Device, idx: usize, src: Source) -> Option<String> {
    match src {
        Source::Button(b) => p.button_token(dev, b as u32 + 1, idx), // bit -> 1-based number
        Source::Axis(a) => p.axis_token(dev, a as usize, idx),
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
