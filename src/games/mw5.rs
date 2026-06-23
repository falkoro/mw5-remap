//! MechWarrior 5: Mercenaries provider. Reads/writes joystick bindings directly
//! in GameUserSettings.ini: axes are `InputTypeToAxisKeyList=` lines; buttons live
//! in the LAST ("Joystick") section of the giant `InputTypeToActionKeyMap=` line
//! as `(ActionName="X",BoundedKeys=((Key=K)))` tuples (replaced by paren-depth scan
//! so nested keys are safe; keyboard/gamepad sections are never touched).

use super::{Action, Binding, GameProvider, Kind, Role, SaveReport};
use crate::input::Device;

const JOY_MARKER: &str = "Joystick, (ActionKeyMaps=";
// Known MOZA hardware -> MW5 role (deterministic; falls back to enum order).
const BASE: (u16, u16) = (0x346E, 0x1002); // MOZA AB6 FFB Base  -> Joystick (aim)
const PEDALS: (u16, u16) = (0x346E, 0x1200); // MOZA MRP Rudder Pedals -> Throttle

pub struct Mw5;

impl Mw5 {
    pub fn new() -> Self { Mw5 }

    fn backup_dir() -> std::path::PathBuf {
        let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
        std::path::Path::new(&base).join("MW5-Remap").join("backups")
    }
}

/// (engine id, friendly label, category, kind). The curated, important actions.
fn catalog() -> Vec<Action> {
    let a = |id: &str, label: &str, cat: &str, kind: Kind| Action {
        id: id.into(), label: label.into(), category: cat.into(), kind,
    };
    use Kind::*;
    vec![
        a("JoystickLookVertical", "Aim Up/Down (stick)", "Aiming", Axis),
        a("JoystickLookHorizontal", "Aim Left/Right (stick)", "Aiming", Axis),
        // POV hat -> looking. A digital hat key drives the look axis at +/- scale.
        // The id is "AxisName@HatKey" so several keys can share one axis (MW5 allows
        // multiple InputTypeToAxisKeyList lines per AxisName).
        a("JoystickLookVertical@Joystick_Hat_1", "Look Up (POV hat)", "Aiming", Axis),
        a("JoystickLookVertical@Joystick_Hat_5", "Look Down (POV hat)", "Aiming", Axis),
        a("JoystickLookHorizontal@Joystick_Hat_3", "Look Right (POV hat)", "Aiming", Axis),
        a("JoystickLookHorizontal@Joystick_Hat_7", "Look Left (POV hat)", "Aiming", Axis),
        a("JoystickThrottle", "Throttle / Gas", "Movement", Axis),
        a("JoystickStrafeRight", "Strafe Left/Right", "Movement", Axis),
        a("JoystickStrafeForward", "Move Fwd/Back", "Movement", Axis),
        a("JoystickLegRotation", "Leg Turn", "Movement", Axis),
        a("FireWeaponGroup1", "Fire Weapon Group 1", "Weapons", Button),
        a("FireWeaponGroup2", "Fire Weapon Group 2", "Weapons", Button),
        a("FireWeaponGroup3", "Fire Weapon Group 3", "Weapons", Button),
        a("FireWeaponGroup4", "Fire Weapon Group 4", "Weapons", Button),
        a("FireWeaponGroup5", "Fire Weapon Group 5", "Weapons", Button),
        a("FireWeaponGroup6", "Fire Weapon Group 6", "Weapons", Button),
        a("ToggleWeaponGroup", "Toggle Weapon Group", "Weapons", Button),
        a("SelectPreviousWeapon", "Previous Weapon", "Weapons", Button),
        a("SelectNextWeapon", "Next Weapon", "Weapons", Button),
        a("SelectPreviousWeaponGroup", "Previous Weapon Group", "Weapons", Button),
        a("SelectNextWeaponGroup", "Next Weapon Group", "Weapons", Button),
        a("ActivateJumpJets", "Jump Jets", "Movement", Button),
        a("CenterTorso", "Center Torso", "Movement", Button),
        a("CenterLegs", "Center Legs", "Movement", Button),
        a("TargetNearestHostileToCrosshair", "Target Under Crosshair", "Targeting", Button),
        a("TargetNextHostile", "Target Next Hostile", "Targeting", Button),
        a("TogglePower", "Toggle Power", "Systems", Button),
        a("ToggleOverride", "Toggle Override (heat)", "Systems", Button),
        a("ToggleBattleGridPanel", "Battle Grid", "Systems", Button),
        a("ToggleNightVision", "Night Vision", "Systems", Button),
        // Camera — the 20 buttons are all used, so these default onto the free hat
        // diagonals (Hat_2/4/8). ToggleView switches 1st-person cockpit <-> 3rd-person.
        a("ToggleView", "1st / 3rd Person View", "Camera", Button),
        a("ToggleFreeLook", "Free Look (hold)", "Camera", Button),
        a("ToggleFreeCamera", "Free Camera", "Camera", Button),
        a("CycleZoom", "Cycle Zoom", "Camera", Button),
        a("IncreaseZoom", "Zoom In", "Camera", Button),
        a("DecreaseZoom", "Zoom Out", "Camera", Button),
        // Essentials found in the audit (were missing). ChainFire gets the last free
        // hat diagonal; the rest are catalogued for binding (keyboard keeps working).
        a("ToggleChainFire", "Chain Fire", "Weapons", Button),
        a("ClearTarget", "Clear Target", "Targeting", Button),
        a("TargetNearestHostile", "Target Nearest Hostile", "Targeting", Button),
        a("DispatchLance", "Lance: Attack My Target", "Command", Button),
        a("CancelOrders", "Lance: Cancel Orders", "Command", Button),
        a("MoveAtFormationSpeed", "Lance: Move at Formation Speed", "Command", Button),
        a("DispatchLanceMate1", "Lance: Order Mate 1", "Command", Button),
        a("DispatchLanceMate2", "Lance: Order Mate 2", "Command", Button),
        a("DispatchLanceMate3", "Lance: Order Mate 3", "Command", Button),
        a("DispatchLanceMate4", "Lance: Order Mate 4", "Command", Button),
        // Throttle/movement (discrete) — MW5's forward/back is the single bipolar
        // JoystickThrottle axis; these are the keyboard-style step controls + stop.
        a("ThrottleIncrease", "Throttle Up (W)", "Movement", Button),
        a("ThrottleDecrease", "Throttle Down / Reverse (S)", "Movement", Button),
        a("Stop", "Full Stop", "Movement", Button),
        a("ToggleThrottleDecay", "Toggle Throttle Decay", "Movement", Button),
        a("ActivateMASC", "MASC / Speed Boost (hold)", "Movement", Button),
        a("ToggleMASC", "Toggle MASC", "Movement", Button),
        a("ToggleJumpJets", "Toggle Jump Jets", "Movement", Button),
        // More targeting
        a("TargetPrevHostile", "Target Previous Hostile", "Targeting", Button),
        a("TargetNearestFriendlyToCrosshair", "Target Friendly Under Crosshair", "Targeting", Button),
        a("TargetNextFriendly", "Target Next Friendly", "Targeting", Button),
        a("TargetPrevFriendly", "Target Previous Friendly", "Targeting", Button),
        // More systems
        a("ToggleHUD", "Toggle HUD", "Systems", Button),
        a("ToggleObjectivePanel", "Objectives Panel", "Systems", Button),
        a("CycleECMMode", "Cycle ECM Mode", "Systems", Button),
        a("PermanentToggleArmLock", "Arm Lock (toggle)", "Systems", Button),
        a("TemporaryToggleArmLock", "Arm Lock (hold)", "Systems", Button),
        a("EjectPilot", "Eject", "Systems", Button),
    ]
}

fn role_for(dev: &Device, enum_index: usize) -> Role {
    match (dev.vid, dev.pid) {
        BASE => Role::Joystick,
        PEDALS => Role::Throttle,
        _ => match enum_index { 0 => Role::Joystick, 1 => Role::Throttle, _ => Role::Ignored },
    }
}

// ---- tiny parse helpers (no regex dependency) ----

/// Read axis bindings: id -> (key, scale).
fn read_axes(text: &str) -> std::collections::HashMap<String, (String, f32)> {
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let l = line.trim();
        let p = "InputTypeToAxisKeyList=(AxisName=\"";
        if let Some(rest) = l.strip_prefix(p) {
            if let Some(qend) = rest.find('"') {
                let axis = &rest[..qend];
                let after = &rest[qend..];
                let scale = field(after, "Scale=").and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);
                if let Some(key) = field(after, "Key=") {
                    // Keep the FIRST line per AxisName (the primary/analog one); extra
                    // keys on the same axis (e.g. POV-hat look) are loaded separately.
                    map.entry(axis.to_string())
                        .or_insert((key.trim_end_matches(')').to_string(), scale));
                }
            }
        }
    }
    map
}

/// Split an axis row id into (AxisName, optional fixed Key). "Axis@Key" -> a
/// multi-key row (one of several keys sharing an axis, e.g. POV hat -> look);
/// plain "Axis" -> the primary single-line axis.
fn split_axis_id(id: &str) -> (&str, Option<&str>) {
    match id.split_once('@') {
        Some((axis, key)) => (axis, Some(key)),
        None => (id, None),
    }
}

/// Byte span of the axis line for `axis` whose Key is exactly `key` (multi-key aware).
fn axis_line_span_keyed(text: &str, axis: &str, key: &str) -> Option<(usize, usize)> {
    let prefix = format!("InputTypeToAxisKeyList=(AxisName=\"{}\"", axis);
    let suffix = format!(",Key={})", key);
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(rel) = text[from..].find(&prefix) {
        let s = from + rel;
        if s == 0 || bytes[s - 1] == b'\n' {
            let mut e = text[s..].find('\n').map(|i| s + i).unwrap_or(text.len());
            if e > s && bytes[e - 1] == b'\r' { e -= 1; }
            if text[s..e].ends_with(&suffix) { return Some((s, e)); }
        }
        from = s + prefix.len();
    }
    None
}

/// Read the Scale of the axis line for `axis` whose Key is exactly `key`.
fn axis_line_scale(text: &str, axis: &str, key: &str) -> Option<f32> {
    let (s, e) = axis_line_span_keyed(text, axis, key)?;
    field(&text[s..e], "Scale=").and_then(|v| v.parse::<f32>().ok())
}

/// Extract a `name=VALUE` field up to the next ',' or ')'.
fn field(s: &str, name: &str) -> Option<String> {
    let i = s.find(name)? + name.len();
    let rest = &s[i..];
    let end = rest.find(|c| c == ',' || c == ')').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Byte span (start, end-excluding-newline) of the line that begins with `needle`.
fn line_span(text: &str, needle: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(rel) = text[from..].find(needle) {
        let s = from + rel;
        if s == 0 || bytes[s - 1] == b'\n' {
            let mut e = text[s..].find('\n').map(|i| s + i).unwrap_or(text.len());
            if e > s && bytes[e - 1] == b'\r' { e -= 1; }
            return Some((s, e));
        }
        from = s + needle.len();
    }
    None
}

/// Byte index just AFTER the last axis line (incl. its newline) — insertion point.
fn last_axis_insert_point(text: &str) -> Option<usize> {
    let prefix = "InputTypeToAxisKeyList=(AxisName=\"";
    let bytes = text.as_bytes();
    let mut last = None;
    let mut from = 0;
    while let Some(rel) = text[from..].find(prefix) {
        let s = from + rel;
        if s == 0 || bytes[s - 1] == b'\n' {
            let e = text[s..].find('\n').map(|i| s + i + 1).unwrap_or(text.len());
            last = Some(e);
        }
        from = s + prefix.len();
    }
    last
}

/// The Joystick section is the last one on the map line. Returns (head, body).
fn split_joy_section(line: &str) -> Option<(String, String)> {
    let idx = line.find(JOY_MARKER)?;
    Some((line[..idx].to_string(), line[idx..].to_string()))
}

/// Read button bindings from the Joystick section: id -> key.
fn read_buttons(text: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let line = match text.lines().find(|l| l.starts_with("InputTypeToActionKeyMap=")) {
        Some(l) => l, None => return map,
    };
    let body = match split_joy_section(line) { Some((_, b)) => b, None => return map };
    let mut search = body.as_str();
    let needle = "(ActionName=\"";
    while let Some(rel) = search.find(needle) {
        let after = &search[rel + needle.len()..];
        if let Some(qend) = after.find('"') {
            let name = &after[..qend];
            // does this tuple have a bound key?
            let tail = &after[qend..];
            if let Some(k) = bounded_key(tail) {
                map.insert(name.to_string(), k);
            }
            search = &after[qend..];
        } else { break; }
    }
    map
}

/// Given text starting at the closing quote of an ActionName, return its bound
/// Key if the tuple is `",BoundedKeys=((Key=K))..."`, else None (unbound).
fn bounded_key(tail: &str) -> Option<String> {
    let p = "\",BoundedKeys=((Key=";
    if let Some(rest) = tail.strip_prefix(p) {
        let end = rest.find(')')?;
        return Some(rest[..end].to_string());
    }
    None
}

/// Replace one action tuple in `body` by paren-depth scan. Returns true if found.
fn set_action(body: &mut String, action: &str, key: &str) -> bool {
    let needle = format!("(ActionName=\"{}\"", action);
    let start = match body.find(&needle) { Some(s) => s, None => return false };
    let bytes = body.as_bytes();
    let mut depth = 0i32;
    let mut end = None;
    for i in start..bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => { depth -= 1; if depth == 0 { end = Some(i); break; } }
            _ => {}
        }
    }
    let end = match end { Some(e) => e, None => return false };
    let replacement = if key.is_empty() || key == "None" {
        format!("(ActionName=\"{}\")", action)
    } else {
        format!("(ActionName=\"{}\",BoundedKeys=((Key={})))", action, key)
    };
    body.replace_range(start..=end, &replacement);
    true
}

impl GameProvider for Mw5 {
    fn name(&self) -> &str { "MechWarrior 5" }
    fn available(&self) -> bool { true }

    fn config_path(&self) -> std::path::PathBuf {
        // Test/override hook: point at a copy so --selftest never touches the real file.
        if let Ok(p) = std::env::var("MW5_CONFIG") {
            if !p.is_empty() { return std::path::PathBuf::from(p); }
        }
        let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
        std::path::Path::new(&base)
            .join("MW5Mercs/Saved/Config/WindowsNoEditor/GameUserSettings.ini")
    }

    fn actions(&self) -> Vec<Action> { catalog() }

    fn default_bindings(&self) -> Vec<Binding> {
        // Known-good MW5 layout matched to the REAL hardware: the MOZA MRP pedals
        // (Throttle role) expose ONLY axes — 0 buttons, no D-pad — so every button
        // action lives on the AB6 base/MHG grip (Joystick role, 32 buttons + hat).
        // Pedals carry just the throttle + strafe axes. Verify axis direction in-game.
        let b = |id: &str, token: &str, scale: f32| Binding { id: id.into(), token: token.into(), scale };
        vec![
            // --- axes ---
            b("JoystickLookVertical", "Joystick_Axis1", 2.0),
            b("JoystickLookHorizontal", "Joystick_Axis2", 3.0),
            // POV hat -> looking (4 ways). Token == the hat key; sign sets direction,
            // magnitude sets look speed. Flip the sign in the GUI if a way is reversed.
            b("JoystickLookVertical@Joystick_Hat_1", "Joystick_Hat_1", 2.0),   // up
            b("JoystickLookVertical@Joystick_Hat_5", "Joystick_Hat_5", -2.0),  // down
            b("JoystickLookHorizontal@Joystick_Hat_3", "Joystick_Hat_3", 3.0), // right
            b("JoystickLookHorizontal@Joystick_Hat_7", "Joystick_Hat_7", -3.0),// left
            // MOZA MRP pedals (Throttle role): RIGHT toe = move forward (the throttle
            // axis), rudder swing-arm = turn the legs. The right toe rests at 0 and
            // presses to max -> JoystickThrottle goes 0..forward (offset 0 in .Remap).
            // REVERSE on the left toe: two separate toe axes can't merge into one MW5
            // throttle here — combine them into a single split axis in MOZA Pit House
            // (center=stop, right=fwd, left=rev) and it maps straight onto Throttle_Axis2,
            // OR put reverse on a button. Use Bind + the live panel to confirm which
            // physical axis is the right toe before trusting these. "" = unbind.
            b("JoystickLegRotation", "Throttle_Axis1", 1.0),   // rudder slide -> turn L/R
            b("JoystickThrottle", "Throttle_Axis2", 1.0),      // RIGHT toe press -> forward
            b("JoystickStrafeRight", "", 1.0),
            // --- weapons: all on the AB6 (Joystick) buttons/hat ---
            b("FireWeaponGroup1", "Joystick_Button1", 1.0),
            b("FireWeaponGroup2", "Joystick_Button2", 1.0),
            b("FireWeaponGroup3", "Joystick_Button3", 1.0),
            b("FireWeaponGroup4", "Joystick_Button4", 1.0),
            b("FireWeaponGroup5", "Joystick_Button5", 1.0),
            b("FireWeaponGroup6", "Joystick_Button6", 1.0),
            b("ToggleWeaponGroup", "Joystick_Button7", 1.0),
            b("ActivateJumpJets", "Joystick_Button9", 1.0),
            b("SelectPreviousWeapon", "Joystick_Button14", 1.0),
            b("SelectNextWeapon", "Joystick_Button15", 1.0),
            b("SelectPreviousWeaponGroup", "Joystick_Button16", 1.0),
            b("SelectNextWeaponGroup", "Joystick_Button17", 1.0),
            b("CenterTorso", "Joystick_Button18", 1.0),
            b("CenterLegs", "Joystick_Button19", 1.0),
            // targeting moved OFF the hat (the hat now looks) onto free AB6 buttons.
            // NB: MW5 only has Joystick_Button1..20 — Button21 is an invalid/dead token.
            b("TargetNearestHostileToCrosshair", "Joystick_Button20", 1.0),
            b("TargetNextHostile", "Joystick_Button12", 1.0),
            b("TogglePower", "Joystick_Button13", 1.0),
            b("ToggleOverride", "Joystick_Button10", 1.0),
            b("ToggleBattleGridPanel", "Joystick_Button8", 1.0),
            b("ToggleNightVision", "Joystick_Button11", 1.0),
            // Camera on the free hat diagonals (cardinals 1/3/5/7 already drive look).
            b("ToggleView", "Joystick_Hat_2", 1.0),     // hat ↗ = 1st/3rd person
            b("ToggleFreeLook", "Joystick_Hat_8", 1.0), // hat ↖ = free look
            b("CycleZoom", "Joystick_Hat_4", 1.0),      // hat ↘ = zoom
            b("ToggleChainFire", "Joystick_Hat_6", 1.0),// hat ↙ = chain fire (last free way)
        ]
    }

    fn load(&self) -> Result<Vec<Binding>, String> {
        let path = self.config_path();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("Can't read config ({}). Launch MW5 once first.\n{}", path.display(), e))?;
        let axes = read_axes(&text);
        let btns = read_buttons(&text);
        let mut out = Vec::new();
        for act in catalog() {
            match act.kind {
                Kind::Axis => {
                    let (axisname, fixed) = split_axis_id(&act.id);
                    if let Some(key) = fixed {
                        // multi-key row (e.g. POV hat -> look): bound iff its exact line exists.
                        match axis_line_scale(&text, axisname, key) {
                            Some(sc) => out.push(Binding { id: act.id.clone(), token: key.to_string(), scale: if sc == 0.0 { 1.0 } else { sc } }),
                            None => out.push(Binding { id: act.id, token: String::new(), scale: 1.0 }),
                        }
                    } else {
                        let (token, scale) = axes.get(axisname).cloned().unwrap_or_default();
                        let token = if token == "None" { String::new() } else { token }; // show unbound, not literal "None"
                        out.push(Binding { id: act.id, token, scale: if scale == 0.0 { 1.0 } else { scale } });
                    }
                }
                Kind::Button => {
                    let token = btns.get(&act.id).cloned().unwrap_or_default();
                    out.push(Binding { id: act.id, token, scale: 1.0 });
                }
            }
        }
        Ok(out)
    }

    fn save(&self, bindings: &[Binding]) -> Result<SaveReport, String> {
        let path = self.config_path();
        let mut text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;

        // If the user locked the config (read-only, to stop MW5 resetting it), clear
        // the flag just for our write and restore it after, so saving still works.
        let was_locked = std::fs::metadata(&path).map(|m| m.permissions().readonly()).unwrap_or(false);
        if was_locked { let _ = set_readonly(&path, false); }

        // backup
        let dir = Mw5::backup_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let backup = dir.join(format!("GameUserSettings_{}.ini", stamp));
        std::fs::copy(&path, &backup).map_err(|e| e.to_string())?;

        let kinds: std::collections::HashMap<String, Kind> =
            catalog().into_iter().map(|a| (a.id, a.kind)).collect();
        let mut changed = Vec::new();
        let mut missing = Vec::new();

        // ---- axes: in-place line edit (preserves the rest of the file byte-for-byte) ----
        // Multi-key rows ("Axis@Key", e.g. POV hat -> look) are located by their FIXED
        // key (from the id) so a rebind rewrites that one line; primary rows match the
        // first line for their AxisName. Missing lines are appended after the axis block.
        for b in bindings.iter().filter(|b| kinds.get(&b.id) == Some(&Kind::Axis) && !b.token.is_empty()) {
            let (axisname, fixed) = split_axis_id(&b.id);
            let want = format!(
                "InputTypeToAxisKeyList=(AxisName=\"{}\",Scale={:.6},Key={})",
                axisname, b.scale, b.token
            );
            let locate = match fixed {
                Some(fixed_key) => axis_line_span_keyed(&text, axisname, fixed_key),
                None => line_span(&text, &format!("InputTypeToAxisKeyList=(AxisName=\"{}\"", axisname)),
            };
            if let Some((s, e)) = locate {
                if &text[s..e] != want.as_str() { changed.push(format!("{} -> {} (x{:.1})", b.id, b.token, b.scale)); }
                text.replace_range(s..e, &want);
            } else if let Some(at) = last_axis_insert_point(&text) {
                text.insert_str(at, &format!("{}\r\n", want));
                changed.push(format!("{} -> {} [added]", b.id, b.token));
            } else {
                missing.push(b.id.clone());
            }
        }

        // ---- axes: unbind (remove the line) for rows explicitly set to empty ----
        for b in bindings.iter().filter(|b| kinds.get(&b.id) == Some(&Kind::Axis) && b.token.is_empty()) {
            let (axisname, fixed) = split_axis_id(&b.id);
            let locate = match fixed {
                Some(fixed_key) => axis_line_span_keyed(&text, axisname, fixed_key),
                None => line_span(&text, &format!("InputTypeToAxisKeyList=(AxisName=\"{}\"", axisname)),
            };
            if let Some((s, e)) = locate {
                let bytes = text.as_bytes();
                let mut end = e;
                if end < bytes.len() && bytes[end] == b'\r' { end += 1; }
                if end < bytes.len() && bytes[end] == b'\n' { end += 1; }
                text.replace_range(s..end, "");
                changed.push(format!("{} -> (unbound)", b.id));
            }
        }

        // ---- buttons: paren-depth replacement inside the Joystick section ----
        if let Some(map_line) = text.lines().find(|l| l.starts_with("InputTypeToActionKeyMap=")) {
            let map_line = map_line.to_string();
            if let Some((head, mut body)) = split_joy_section(&map_line) {
                for b in bindings.iter().filter(|b| kinds.get(&b.id) == Some(&Kind::Button)) {
                    if set_action(&mut body, &b.id, &b.token) {
                        changed.push(format!("{} -> {}", b.id, if b.token.is_empty() { "None" } else { &b.token }));
                    } else {
                        missing.push(b.id.clone());
                    }
                }
                let new_line = format!("{}{}", head, body);
                text = text.replace(&map_line, &new_line);
            }
        }

        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        if was_locked { let _ = set_readonly(&path, true); } // re-lock as the user left it
        Ok(SaveReport { backup: backup.display().to_string(), changed, missing })
    }

    fn role_of(&self, dev: &Device, idx: usize) -> Role { role_for(dev, idx) }

    fn button_token(&self, dev: &Device, button: u32, idx: usize) -> Option<String> {
        match role_for(dev, idx) {
            Role::Ignored => None,
            r => Some(format!("{}_Button{}", r.label(), button)),
        }
    }

    fn axis_token(&self, dev: &Device, axis_index: usize, idx: usize) -> Option<String> {
        match role_for(dev, idx) {
            Role::Ignored => None,
            r => Some(format!("{}_Axis{}", r.label(), axis_index + 1)),
        }
    }

    fn pov_token(&self, dev: &Device, octant: u32, idx: usize) -> Option<String> {
        match role_for(dev, idx) {
            Role::Joystick => Some(format!("Joystick_Hat_{}", octant)),
            Role::Throttle => {
                let dir = match octant { 1 => "Up", 3 => "Right", 5 => "Down", 7 => "Left", _ => return None };
                Some(format!("Throttle_DPad1_{}", dir))
            }
            Role::Ignored => None,
        }
    }

    fn launch_uri(&self) -> Option<String> { Some("steam://rungameid/784080".into()) }
    fn conflict_vids(&self) -> Vec<String> { vec!["3344".into(), "231D".into()] }
    fn running_processes(&self) -> Vec<String> {
        vec!["MechWarrior-Win64-Shipping".into(), "MW5Mercs".into()]
    }
}

// ===========================================================================
// HOTASMappings.Remap — the SECOND file MW5 needs for joystick input.
//
// GameUserSettings.ini maps token -> action; this file maps the *physical*
// device input -> token, keyed per device by VID/PID. Without a block for a
// device, none of its buttons/axes reach the game, so the GUS bindings are
// dead. The game ships this file with whatever HOTAS it first saw (here: stale
// Thrustmaster blocks) and has no full in-game binding UI — you edit the file.
// Format + vocabulary are from Piranha's official HOTAS Remapping PDF.
// ===========================================================================

/// Set/clear the read-only flag on a file.
fn set_readonly(path: &std::path::Path, ro: bool) -> Result<(), String> {
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
/// and left toe reverse (Invert=TRUE, Offset=+1.0). The toe brakes have no HOTAS_*
/// name (they're rX/rY) so they're addressed as GenericUSBController_AxisN; the
/// ordinals (4/5/6 for rX/rY/rZ) can vary per device — verify in-game and swap if
/// forward/reverse are flipped. Pattern is the proven community "gas + brake" setup.
fn mrp_pedal_block() -> String {
    let mut s = String::from("START_BIND\r\nNAME: MOZA MRP Rudder Pedals\r\nVID: 0x346E\r\nPID: 0x1200\r\n");
    // rudder swing-arm (rZ, centred) -> leg turn
    s.push_str("AXIS: InAxis=HOTAS_RZAxis, OutAxis=Throttle_Axis1, Invert=FALSE, Offset=-0.5, DeadZoneMin=-0.05, DeadZoneMax=0.05, MapToDeadZone=TRUE\r\n");
    // right toe (rX = Axis4) -> forward half of the bipolar throttle
    s.push_str("AXIS: InAxis=GenericUSBController_Axis4, OutAxis=Throttle_Axis2, Invert=FALSE, Offset=-1.0, DeadZoneMin=-0.1, DeadZoneMax=0.1, MapToDeadZone=FALSE\r\n");
    // left toe (rY = Axis5) -> reverse half of the same throttle
    s.push_str("AXIS: InAxis=GenericUSBController_Axis5, OutAxis=Throttle_Axis2, Invert=TRUE, Offset=1.0, DeadZoneMin=-0.1, DeadZoneMax=0.1, MapToDeadZone=FALSE\r\n");
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
