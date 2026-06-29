//! HOTASMappings.Remap — the SECOND file MW5 needs for joystick input.
//!
//! GameUserSettings.ini maps token -> action; this file maps the *physical*
//! device input -> token, keyed per device by VID/PID. Without a block for a
//! device, none of its buttons/axes reach the game, so the GUS bindings are
//! dead. The game ships this file with whatever HOTAS it first saw (here: stale
//! Thrustmaster blocks) and has no full in-game binding UI — you edit the file.
//! Format + vocabulary are from Piranha's official HOTAS Remapping PDF.

use super::{Mw5, Role};
use crate::games::GameProvider; // brings Mw5::config_path (a trait method) into scope
use std::sync::atomic::{AtomicBool, Ordering};

/// GUI config-lock flag (default ON). When on, `mw5::save()` re-applies the read-only
/// flag to GameUserSettings.ini AFTER writing, because a read-only file is exactly what
/// PREVENTS MW5 from resetting the joystick bindings back to stock on its next launch.
/// Mirrors `vjoy::set_active`/`is_active`.
static CONFIG_LOCKED: AtomicBool = AtomicBool::new(true);

/// Toggle the GUI config-lock flag (🔒 Lock config). Pure in-process state; the live
/// file's read-only bit is (re)applied by `save()` when locked, and cleared by the
/// caller (toolbar) the moment the lock is turned OFF so MW5's own menu can edit it.
pub fn set_config_locked(lock: bool) { CONFIG_LOCKED.store(lock, Ordering::Relaxed); }

/// Is the config-lock on? Default `true`.
pub fn is_config_locked() -> bool { CONFIG_LOCKED.load(Ordering::Relaxed) }

/// Set/clear the read-only flag on a file. Read-only does NOT make MW5 ignore the file —
/// it stops MW5 from rewriting the joystick bindings to stock on launch.
pub fn set_readonly(path: &std::path::Path, ro: bool) -> Result<(), String> {
    let mut perm = std::fs::metadata(path).map_err(|e| e.to_string())?.permissions();
    perm.set_readonly(ro);
    std::fs::set_permissions(path, perm).map_err(|e| e.to_string())
}

/// Lock/unlock the live GameUserSettings file directly and sync the flag — the
/// `--lock`/`--unlock` CLI + recovery path.
pub fn set_config_readonly(lock: bool) -> Result<(), String> {
    set_config_locked(lock);
    set_readonly(&Mw5::new().config_path(), lock)
}

/// Path to the live HOTASMappings.Remap (MW5_HOTAS overrides it, for tests).
pub fn hotas_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("MW5_HOTAS") {
        if !p.is_empty() { return std::path::PathBuf::from(p); }
    }
    let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
    std::path::Path::new(&base).join("MW5Mercs/Saved/SavedHOTAS/HOTASMappings.Remap")
}

/// Read `key` (e.g. "VID:") as a u16 from a hex `0x....` line inside a block.
fn parse_hex_field(block: &str, key: &str) -> Option<u16> {
    for line in block.lines() {
        if let Some(rest) = line.trim().strip_prefix(key) {
            let v = rest.trim().trim_start_matches("0x").trim_start_matches("0X");
            return u16::from_str_radix(v, 16).ok();
        }
    }
    None
}

/// Drop every START_BIND block whose (VID,PID) is in `targets`, keep the rest
/// byte-for-byte. Lets us refresh our MOZA blocks without disturbing a user's
/// other devices (e.g. the stock Thrustmaster entries).
fn strip_device_blocks(text: &str, targets: &[(u16, u16)]) -> String {
    let bytes = text.as_bytes();
    let mut starts = Vec::new();
    let mut from = 0;
    while let Some(rel) = text[from..].find("START_BIND") {
        let s = from + rel;
        if s == 0 || bytes[s - 1] == b'\n' { starts.push(s); }
        from = s + "START_BIND".len();
    }
    if starts.is_empty() { return text.to_string(); }
    let mut out = String::new();
    out.push_str(&text[..starts[0]]); // preamble before the first block
    for (i, &st) in starts.iter().enumerate() {
        let en = if i + 1 < starts.len() { starts[i + 1] } else { text.len() };
        let block = &text[st..en];
        let matched = match (parse_hex_field(block, "VID:"), parse_hex_field(block, "PID:")) {
            (Some(v), Some(p)) => targets.iter().any(|&(tv, tp)| tv == v && tp == p),
            _ => false,
        };
        if !matched { out.push_str(block); }
    }
    out
}

/// Keep only the START_BIND blocks whose (VID,PID) satisfies `keep` (blocks with no
/// parseable VID/PID are kept for safety). Used to drop absent/stale device blocks so
/// MW5 doesn't assign the Joystick slot to a device that isn't plugged in.
fn retain_blocks(text: &str, keep: impl Fn(u16, u16) -> bool) -> String {
    let bytes = text.as_bytes();
    let mut starts = Vec::new();
    let mut from = 0;
    while let Some(rel) = text[from..].find("START_BIND") {
        let s = from + rel;
        if s == 0 || bytes[s - 1] == b'\n' { starts.push(s); }
        from = s + "START_BIND".len();
    }
    if starts.is_empty() { return text.to_string(); }
    let mut out = String::new();
    out.push_str(&text[..starts[0]]);
    for (i, &st) in starts.iter().enumerate() {
        let en = if i + 1 < starts.len() { starts[i + 1] } else { text.len() };
        let block = &text[st..en];
        let keep_it = match (parse_hex_field(block, "VID:"), parse_hex_field(block, "PID:")) {
            (Some(v), Some(p)) => keep(v, p),
            _ => true, // unidentifiable -> keep
        };
        if keep_it { out.push_str(block); }
    }
    out
}

/// The MW5 OutAxis token for a device axis, by role + meaning (None = no slot).
/// Matches the GameUserSettings token->action contract.
fn out_axis_token(role: Role, sem: crate::devices::Sem) -> Option<&'static str> {
    use crate::devices::Sem::*;
    match (role, sem) {
        (Role::Joystick, Pitch) => Some("Joystick_Axis1"),
        (Role::Joystick, Roll) => Some("Joystick_Axis2"),
        (Role::Joystick, Yaw) => Some("Joystick_Axis3"),
        (Role::Throttle, Yaw) => Some("Throttle_Axis1"),    // rudder/pedal -> leg turn
        (Role::Throttle, Throttle) => Some("Throttle_Axis2"), // toe/lever -> forward
        _ => None,
    }
}

/// MOZA MRP rudder pedals: rudder swing-arm + TWO independent toe brakes. MW5's
/// throttle (`Throttle_Axis2`) is a single ABSOLUTE BIPOLAR axis (centre = stop,
/// up = forward, below centre = reverse), so we drive it from BOTH toes with two
/// AXIS lines onto the same OutAxis: right toe forward (Invert=FALSE, Offset=-1.0)
/// and left toe reverse (Invert=TRUE, Offset=+1.0). Confirmed live via --monitor on
/// this MRP: the two toe brakes are winmm axis X and Y (DirectInput Axis1/Axis2),
/// the rudder swing-arm is axis R (HOTAS_RZAxis). If forward/reverse come out swapped
/// just flip which toe is Axis1 vs Axis2. Pattern is the community "gas + brake" setup.
fn mrp_pedal_block() -> String {
    let mut s = String::from("START_BIND\r\nNAME: MOZA MRP Rudder Pedals\r\nVID: 0x346E\r\nPID: 0x1200\r\n");
    // rudder swing-arm (Rz, centred) -> leg turn
    s.push_str("AXIS: InAxis=HOTAS_RZAxis, OutAxis=Throttle_Axis1, Invert=FALSE, Offset=-0.5, DeadZoneMin=-0.05, DeadZoneMax=0.05, MapToDeadZone=TRUE\r\n");
    // GAS toe = Ry = GenericUSBController_Axis2 (user-confirmed: Rx/Axis1 is the WRONG toe
    // and crawled forward). Forward only; the toe rests at the LOW end, so DEADZONE the whole
    // bottom (DeadZoneMin=-1.0 .. DeadZoneMax=0.15, MapToDeadZone=TRUE, Offset=0.0) => rest =
    // STOP whether MW5 maps the axis 0..1 or -1..1; pressing past ~15% ramps forward.
    // Reverse = button 3 (ThrottleDecrease).
    s.push_str("AXIS: InAxis=GenericUSBController_Axis2, OutAxis=Throttle_Axis2, Invert=FALSE, Offset=0.0, DeadZoneMin=-1.0, DeadZoneMax=0.15, MapToDeadZone=TRUE\r\n");
    s
}

/// vJoy device — the SINGLE clean device the whole MOZA rig is mirrored onto (evilC
/// approach), so MW5 reads tidy 20 buttons / 6 centred axes instead of the AB6's 128
/// buttons (which it collapses to "Button 1"). Buttons 1..20 = AB6 stick buttons;
/// X/Y = gimbal aim, Rx/Ry = thumb-hat look, Z = combined throttle, Rz = rudder. All
/// axes are CENTRED by the feeder, so Offset=-0.5. Maps to BOTH Joystick+Throttle roles.
fn vjoy_block() -> String {
    use crate::vjoy_map::Target;
    let mut s = String::from("START_BIND\r\nNAME: vJoy Device\r\nVID: 0x1234\r\nPID: 0xBEAD\r\n");
    // vJoy buttons 1..20 -> Joystick_Button*, 21..32 -> Throttle_Button1..12 (evilC scheme)
    // so EVERY physical button reaches MW5 even above DI bit 19. The OutButtons token comes
    // from vjoy_target_token (the single source of truth) so the .Remap and the device-
    // diagram resolver can never drift apart.
    for n in 1..=32u8 {
        let tok = vjoy_target_token(&Target::Button(n)).expect("vjoy button 1..32 maps to a token");
        s.push_str(&format!("BUTTON: InButton=GenericUSBController_Button{n}, OutButtons={tok}\r\n"));
    }
    // vJoy POV hat -> Joystick_Hat_1..8 (8 directions). Lets a physical coolie hat
    // routed onto the vJoy POV reach MW5 as a hat, with the physical device hidden.
    for n in 1..=8u8 {
        s.push_str(&format!("BUTTON: InButton=GenericUSBController_Hat{n}, OutButtons=Joystick_Hat_{n}\r\n"));
    }
    let dz = "Invert=FALSE, Offset=-0.5, DeadZoneMin=-0.05, DeadZoneMax=0.05, MapToDeadZone=TRUE";
    // (vJoy HID input name -> vJoy axis usage). MW5 can't address Rx/Ry by name, so those two
    // enter as the raw GenericUSBController_Axis4/5; the OutAxis token is vjoy_target_token's.
    let axes = [
        ("HOTAS_XAxis", crate::vjoy::HID_X),
        ("HOTAS_YAxis", crate::vjoy::HID_Y),
        ("GenericUSBController_Axis4", crate::vjoy::HID_RX),
        ("GenericUSBController_Axis5", crate::vjoy::HID_RY),
        ("HOTAS_ZAxis", crate::vjoy::HID_Z),
        ("HOTAS_RZAxis", crate::vjoy::HID_RZ),
    ];
    for (inaxis, usage) in axes {
        let tok = vjoy_target_token(&Target::Axis(usage)).expect("vjoy axis maps to a token");
        s.push_str(&format!("AXIS: InAxis={inaxis}, OutAxis={tok}, {dz}\r\n"));
    }
    s
}

/// SINGLE SOURCE OF TRUTH for "vJoy Target -> MW5 token". `vjoy_block()` EMITS exactly
/// these tokens, and the device-diagram resolver READS them back, so the .Remap file and
/// the on-screen labels can never drift. Mirrors the evilC scheme: vJoy buttons 1..20 ->
/// Joystick_Button1..20, 21..32 -> Throttle_Button1..12; vJoy axes X/Y/Rx/Ry/Z/Rz ->
/// Joystick_Axis1/2/4/5 + Throttle_Axis2/Axis1. `None` for a Target outside that range.
pub fn vjoy_target_token(t: &crate::vjoy_map::Target) -> Option<String> {
    use crate::vjoy_map::Target;
    match *t {
        Target::Button(n) if (1..=20).contains(&n) => Some(format!("Joystick_Button{n}")),
        Target::Button(n) if (21..=32).contains(&n) => Some(format!("Throttle_Button{}", n - 20)),
        Target::Button(_) => None,
        // The vJoy POV's 8 directions are emitted directly as Joystick_Hat_1..8 by
        // vjoy_block(); there is no single token for the whole hat.
        Target::Pov => None,
        Target::Axis(u) => Some(
            match u {
                crate::vjoy::HID_X => "Joystick_Axis1",
                crate::vjoy::HID_Y => "Joystick_Axis2",
                crate::vjoy::HID_RX => "Joystick_Axis4",
                crate::vjoy::HID_RY => "Joystick_Axis5",
                crate::vjoy::HID_Z => "Throttle_Axis2",
                crate::vjoy::HID_RZ => "Throttle_Axis1",
                _ => return None,
            }
            .to_string(),
        ),
    }
}

/// Build one START_BIND block for a known device (buttons capped at MW5's 20,
/// hat -> Hat_1..8, axes -> the role's tokens; throttle axes aren't centered).
fn device_block(d: &crate::devices::KnownDevice) -> String {
    if (d.vid, d.pid) == (0x346E, 0x1200) { return mrp_pedal_block(); }
    if (d.vid, d.pid) == (0x1234, 0xBEAD) { return vjoy_block(); }
    let role = d.role.label(); // "Joystick" | "Throttle"
    let mut s = String::new();
    s.push_str("START_BIND\r\n");
    s.push_str(&format!("NAME: {}\r\n", d.name));
    s.push_str(&format!("VID: 0x{:04X}\r\n", d.vid));
    s.push_str(&format!("PID: 0x{:04X}\r\n", d.pid));
    for i in 1..=d.buttons.min(20) {
        s.push_str(&format!("BUTTON: InButton=GenericUSBController_Button{i}, OutButtons={role}_Button{i}\r\n"));
    }
    if d.has_hat {
        for i in 1..=8 {
            s.push_str(&format!("BUTTON: InButton=GenericUSBController_Hat{i}, OutButtons={role}_Hat_{i}\r\n"));
        }
    }
    for a in d.axes {
        if let Some(tok) = out_axis_token(d.role, a.sem) {
            let throttle = a.sem == crate::devices::Sem::Throttle;
            let (offset, dz) = if throttle { (0.0, 0.02) } else { (-0.5, 0.05) };
            s.push_str(&format!(
                "AXIS: InAxis={}, OutAxis={}, Invert=FALSE, Offset={:.1}, DeadZoneMin=-{:.2}, DeadZoneMax={:.2}, MapToDeadZone=TRUE\r\n",
                a.hotas, tok, offset, dz, dz
            ));
        }
    }
    // MHG analog thumb hat / POV: Windows Game Controllers shows it as X-Rotation /
    // Y-Rotation (Rx/Ry); winmm surfaces them as U/V. MW5 does NOT recognise RX/RY by
    // name (confirmed: "MechWarrior 5 does not recognize RX and RY mappings"), and
    // they're not sliders either — so HOTAS_RXAxis/HOTAS_Slider1 do nothing. The
    // working route is the raw HID index GenericUSBController_AxisN: on the AB6 the
    // axis order is X,Y,Z,Rx,Ry,Rz, so Rx=Axis4, Ry=Axis5. BEST-GUESS ordinal —
    // verify in-game; if look doesn't move, try Axis5/Axis6.
    if (d.vid, d.pid) == (0x346E, 0x1002) {
        s.push_str("AXIS: InAxis=GenericUSBController_Axis4, OutAxis=Joystick_Axis4, Invert=FALSE, Offset=-0.5, DeadZoneMin=-0.05, DeadZoneMax=0.05, MapToDeadZone=TRUE\r\n");
        s.push_str("AXIS: InAxis=GenericUSBController_Axis5, OutAxis=Joystick_Axis5, Invert=FALSE, Offset=-0.5, DeadZoneMin=-0.05, DeadZoneMax=0.05, MapToDeadZone=TRUE\r\n");
    }
    s
}

/// Every OutButton/OutAxis token that some known device's .Remap block can PRODUCE.
/// The selftest checks each default binding against this, to catch a binding pointed
/// at a token that no physical control feeds (the "mapped in-game but dead" bug).
pub fn producible_tokens() -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for d in crate::devices::registry().iter().filter(|d| !d.custom) {
        for line in device_block(d).lines() {
            for key in ["OutButtons=", "OutAxis="] {
                if let Some(i) = line.find(key) {
                    let rest = &line[i + key.len()..];
                    let end = rest.find([',', ' ', '\r', '\t']).unwrap_or(rest.len());
                    out.insert(rest[..end].trim().to_string());
                }
            }
        }
    }
    out
}

fn append_block(out: &mut String, blk: &str) {
    if !out.is_empty() { out.push_str("\r\n\r\n\r\n"); }
    out.push_str(blk);
}

/// Write/refresh every known device's block in HOTASMappings.Remap (preserving
/// any other devices), backing up the existing file first. Returns the backup path.
pub fn write_hotas_mappings() -> Result<String, String> {
    let path = hotas_path();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    let mut backup = String::from("(no prior file)");
    if path.exists() {
        let dir = Mw5::backup_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let b = dir.join(format!("HOTASMappings_{}.Remap", stamp));
        std::fs::copy(&path, &b).map_err(|e| e.to_string())?;
        backup = b.display().to_string();
    }

    // Only emit blocks for devices that are actually CONNECTED. Multiple Joystick-role
    // blocks (the stale Thrustmaster entries MW5 ships with, or an absent Warthog) make
    // the game assign the "Joystick" device slot to the WRONG device, which leaves the
    // present stick's BUTTONS dead (axes can still work off the sole Throttle device).
    // So: keep existing blocks only for connected devices we DON'T manage (preserve a
    // user's unknown stick), drop everything absent/stale, and re-emit our registry
    // devices that are connected.
    let connected: Vec<(u16, u16)> = crate::input::poll().iter().map(|d| (d.vid, d.pid)).collect();
    let registry_ids: Vec<(u16, u16)> = crate::devices::registry().iter().map(|d| (d.vid, d.pid)).collect();

    let mut out = if connected.is_empty() {
        // Safety: nothing detected — don't wipe the file; just manage our blocks in place.
        strip_device_blocks(&existing, &registry_ids).trim_end().to_string()
    } else {
        retain_blocks(&existing, |vid, pid| {
            connected.contains(&(vid, pid)) && !registry_ids.contains(&(vid, pid))
        }).trim_end().to_string()
    };
    // When the vJoy feeder is ACTIVE, the user's whole rig is mirrored onto the one vJoy
    // device, so vJoy is the sole Joystick AND Throttle: write ONLY the vJoy block and SKIP
    // every physical device (else they'd fight vJoy for MW5's slots, or re-collapse buttons).
    // When the feeder is off, write the physical blocks and NOT vJoy — a vJoy install never
    // breaks a normal setup. So: a device is emitted iff (it is vJoy) == (vJoy is active).
    let vjoy_id = (0x1234u16, 0xBEADu16);
    let vjoy_active = crate::vjoy::is_active() && connected.contains(&vjoy_id);
    for d in crate::devices::registry().iter().filter(|d| {
        !d.custom
            && (connected.is_empty() || connected.contains(&(d.vid, d.pid)))
            && (((d.vid, d.pid) == vjoy_id) == vjoy_active) // vJoy-only when feeding, physical-only otherwise
    }) {
        append_block(&mut out, &device_block(d));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two stale Thrustmaster-style blocks + a MOZA block, to exercise the filters.
    const SAMPLE: &str = "START_BIND\r\nNAME: Stale TM\r\nVID: 0x044F\r\nPID: 0xB10A\r\nBUTTON: InButton=GenericUSBController_Button1, OutButtons=Joystick_Button1\r\n\r\n\r\nSTART_BIND\r\nNAME: MOZA AB6 FFB Base\r\nVID: 0x346E\r\nPID: 0x1002\r\n";

    #[test]
    fn strip_drops_only_targeted() {
        let out = strip_device_blocks(SAMPLE, &[(0x044F, 0xB10A)]);
        assert!(!out.contains("Stale TM"), "stale block should be dropped");
        assert!(out.contains("MOZA AB6"), "MOZA block should remain");
    }

    #[test]
    fn retain_keeps_only_matching() {
        // keep only the MOZA (simulates "connected & unmanaged"); drop the stale TM.
        let out = retain_blocks(SAMPLE, |vid, pid| (vid, pid) == (0x346E, 0x1002));
        assert!(out.contains("MOZA AB6"));
        assert!(!out.contains("Stale TM"));
    }

    #[test]
    fn ab6_and_mrp_produce_the_tokens_defaults_need() {
        let toks = producible_tokens();
        // fire buttons + hat + look (AB6) and throttle + rudder (MRP)
        for t in ["Joystick_Button1", "Joystick_Button20", "Joystick_Hat_1",
                  "Joystick_Axis1", "Joystick_Axis4", "Throttle_Axis1", "Throttle_Axis2"] {
            assert!(toks.contains(t), "no device .Remap produces {t}");
        }
    }

    #[test]
    fn no_orphan_default_bindings() {
        let toks = producible_tokens();
        let orphans: Vec<_> = super::super::data::default_bindings().into_iter()
            .filter(|b| !b.token.is_empty() && !toks.contains(&b.token))
            .map(|b| format!("{}->{}", b.id, b.token))
            .collect();
        assert!(orphans.is_empty(), "every default binding must map to a producible token, got orphans: {orphans:?}");
    }

    #[test]
    fn vjoy_target_token_round_trips_and_agrees_with_vjoy_block() {
        use crate::vjoy_map::Target;
        // key cases
        let cases: &[(Target, &str)] = &[
            (Target::Button(1), "Joystick_Button1"),
            (Target::Button(20), "Joystick_Button20"),
            (Target::Button(21), "Throttle_Button1"),
            (Target::Button(32), "Throttle_Button12"),
            (Target::Axis(crate::vjoy::HID_X), "Joystick_Axis1"),
            (Target::Axis(crate::vjoy::HID_Y), "Joystick_Axis2"),
            (Target::Axis(crate::vjoy::HID_RX), "Joystick_Axis4"),
            (Target::Axis(crate::vjoy::HID_RY), "Joystick_Axis5"),
            (Target::Axis(crate::vjoy::HID_Z), "Throttle_Axis2"),
            (Target::Axis(crate::vjoy::HID_RZ), "Throttle_Axis1"),
        ];
        for (t, want) in cases {
            assert_eq!(vjoy_target_token(t).as_deref(), Some(*want), "wrong token for {t:?}");
        }
        // out of range -> None
        assert_eq!(vjoy_target_token(&Target::Button(0)), None);
        assert_eq!(vjoy_target_token(&Target::Button(33)), None);

        // AGREEMENT: every OutButtons=/OutAxis= token vjoy_block() emits must be one
        // vjoy_target_token produces over buttons 1..32 + the six routed axes.
        let mut produced = std::collections::HashSet::new();
        for n in 1..=32u8 {
            if let Some(t) = vjoy_target_token(&Target::Button(n)) { produced.insert(t); }
        }
        for u in [crate::vjoy::HID_X, crate::vjoy::HID_Y, crate::vjoy::HID_RX,
                  crate::vjoy::HID_RY, crate::vjoy::HID_Z, crate::vjoy::HID_RZ] {
            if let Some(t) = vjoy_target_token(&Target::Axis(u)) { produced.insert(t); }
        }
        // vjoy_block() also emits the 8 POV-hat directions directly (no single Target).
        for n in 1..=8 { produced.insert(format!("Joystick_Hat_{n}")); }
        for line in vjoy_block().lines() {
            for key in ["OutButtons=", "OutAxis="] {
                if let Some(i) = line.find(key) {
                    let rest = &line[i + key.len()..];
                    let end = rest.find([',', ' ', '\r', '\t']).unwrap_or(rest.len());
                    let tok = rest[..end].trim().to_string();
                    assert!(produced.contains(&tok), "vjoy_block emits {tok}, not reproducible from vjoy_target_token");
                }
            }
        }
    }

    #[test]
    fn mrp_throttle_is_a_single_centred_friendly_line() {
        // the throttle toe line must target Throttle_Axis2 (what JoystickThrottle binds to)
        let blk = mrp_pedal_block();
        assert!(blk.contains("OutAxis=Throttle_Axis2"), "MRP must drive Throttle_Axis2");
        assert!(blk.contains("OutAxis=Throttle_Axis1"), "MRP rudder must drive Throttle_Axis1");
    }
}
