//! MechWarrior 5: Mercenaries provider. Reads/writes joystick bindings directly
//! in GameUserSettings.ini: axes are `InputTypeToAxisKeyList=` lines; buttons live
//! in the LAST ("Joystick") section of the giant `InputTypeToActionKeyMap=` line
//! as `(ActionName="X",BoundedKeys=((Key=K)))` tuples (replaced by paren-depth scan
//! so nested keys are safe; keyboard/gamepad sections are never touched).

use super::{Action, Binding, GameProvider, Kind, Role, SaveReport};
use crate::input::Device;

mod data;
mod hotas;
mod parse;

use data::catalog;
use parse::{
    axis_line_scale, axis_line_span_keyed, last_axis_insert_point, line_span, read_axes,
    read_buttons, set_action, split_axis_id, split_joy_section,
};

// Keep the public path stable: games::mw5::write_hotas_mappings etc.
pub use hotas::{hotas_path, producible_tokens, set_config_locked, write_hotas_mappings};

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

fn role_for(dev: &Device, enum_index: usize) -> Role {
    match (dev.vid, dev.pid) {
        BASE => Role::Joystick,
        PEDALS => Role::Throttle,
        _ => match enum_index { 0 => Role::Joystick, 1 => Role::Throttle, _ => Role::Ignored },
    }
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

    fn default_bindings(&self) -> Vec<Binding> { data::default_bindings() }

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

        // Always keep the config WRITABLE (the lock feature was removed). A read-only
        // GameUserSettings can make MW5 ignore it and fall back to stock bindings.
        let _ = hotas::set_readonly(&path, false);

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
            // AB6: map each winmm axis to the OutAxis the .Remap actually routes it to,
            // so press-to-bind captures a token that works in-game (incl. the analog
            // thumb hat on winmm U/V -> Joystick_Axis4/Axis5).
            // DirectInput 8-axis layout [X,Y,Z,Rx,Ry,Rz,S0,S1]. AB6 gimbal X=0/Y=1;
            // the analog hat is Rx=3 (vertical) / Ry=4 (horizontal).
            Role::Joystick if (dev.vid, dev.pid) == BASE => {
                let n = match axis_index {
                    1 => 1, // Y gimbal -> pitch
                    0 => 2, // X gimbal -> roll
                    3 => 4, // Rx analog hat vertical  -> Joystick_Axis4
                    4 => 5, // Ry analog hat horizontal -> Joystick_Axis5
                    other => other + 1,
                };
                Some(format!("Joystick_Axis{n}"))
            }
            // MRP pedals: confirmed live the toes are Rx(3)/Ry(4) (NOT X/Y) and the
            // rudder swing-arm is Rz(5). Both toes -> Throttle_Axis2 (bipolar throttle,
            // left toe = reverse half); rudder -> Throttle_Axis1. Capture must produce
            // the SAME tokens or a press-to-bind lands on a dead slot.
            Role::Throttle if (dev.vid, dev.pid) == PEDALS => {
                let n = match axis_index {
                    3 | 4 => 2, // right/left toe (Rx/Ry) -> throttle (fwd / reverse)
                    5 => 1,     // rudder swing-arm (Rz) -> leg turn
                    other => other + 1,
                };
                Some(format!("Throttle_Axis{n}"))
            }
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
