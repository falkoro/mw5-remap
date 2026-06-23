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

/// Set/clear the read-only flag on a file.
pub(super) fn set_readonly(path: &std::path::Path, ro: bool) -> Result<(), String> {
    let mut perm = std::fs::metadata(path).map_err(|e| e.to_string())?.permissions();
    perm.set_readonly(ro);
    std::fs::set_permissions(path, perm).map_err(|e| e.to_string())
}

/// True if GameUserSettings is currently locked (read-only) against MW5 resets.
pub fn config_is_locked() -> bool {
    Mw5::new().config_path().metadata().map(|m| m.permissions().readonly()).unwrap_or(false)
}

/// Lock/unlock GameUserSettings. Locking (read-only) stops MW5 from rewriting your
/// joystick bindings back to its stock defaults when it launches. Trade-off: other
/// in-game settings (graphics/audio) also won't save until you unlock.
pub fn set_config_locked(lock: bool) -> Result<(), String> {
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
    // rudder swing-arm (axis R = Rz, centred) -> leg turn
    s.push_str("AXIS: InAxis=HOTAS_RZAxis, OutAxis=Throttle_Axis1, Invert=FALSE, Offset=-0.5, DeadZoneMin=-0.05, DeadZoneMax=0.05, MapToDeadZone=TRUE\r\n");
    // right toe (axis X = Axis1) -> forward half of the bipolar throttle
    s.push_str("AXIS: InAxis=GenericUSBController_Axis1, OutAxis=Throttle_Axis2, Invert=FALSE, Offset=-1.0, DeadZoneMin=-0.1, DeadZoneMax=0.1, MapToDeadZone=FALSE\r\n");
    // left toe (axis Y = Axis2) -> reverse half of the same throttle
    s.push_str("AXIS: InAxis=GenericUSBController_Axis2, OutAxis=Throttle_Axis2, Invert=TRUE, Offset=1.0, DeadZoneMin=-0.1, DeadZoneMax=0.1, MapToDeadZone=FALSE\r\n");
    s
}

/// Build one START_BIND block for a known device (buttons capped at MW5's 20,
/// hat -> Hat_1..8, axes -> the role's tokens; throttle axes aren't centered).
fn device_block(d: &crate::devices::KnownDevice) -> String {
    if (d.vid, d.pid) == (0x346E, 0x1200) { return mrp_pedal_block(); }
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
    s
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

    // Strip the blocks we manage (real devices, by VID/PID) and re-emit them.
    let targets: Vec<(u16, u16)> = crate::devices::registry().iter()
        .filter(|d| !d.custom).map(|d| (d.vid, d.pid)).collect();
    let mut out = strip_device_blocks(&existing, &targets).trim_end().to_string();
    for d in crate::devices::registry().iter().filter(|d| !d.custom) {
        append_block(&mut out, &device_block(d));
    }
    // Custom-pedal template: add once and never clobber the user's later edits.
    if !existing.contains("Custom Pedal") {
        for d in crate::devices::registry().iter().filter(|d| d.custom) {
            append_block(&mut out, &device_block(d));
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(backup)
}
