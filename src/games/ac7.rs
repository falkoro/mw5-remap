//! Ace Combat 7: Skies Unknown provider. Unlike MW5, AC7 binds a device's axis or
//! button DIRECTLY to a flight action inside per-device `[Joystick-GUID]` sections
//! of `Config/Input.ini` — there's no token->action indirection. So a binding token
//! here is `"VVVVPPPP|input"` (USB ids + the AC7 input, e.g. `"044F0402|Y:R"`).
//!
//! Reminder for users: AC7 ignores joysticks unless **Steam Input is disabled**.

use super::{Action, Binding, GameProvider, Kind, Role, SaveReport};
use crate::devices;
use crate::input::Device;

pub struct Ac7;
impl Ac7 {
    pub fn new() -> Self { Ac7 }
}

fn ac7_dir() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
    std::path::Path::new(&base).join("BANDAI NAMCO Entertainment/ACE COMBAT 7/Config")
}

/// (engine key, friendly label, category, kind). Keys are AC7's own Input.ini names.
fn catalog() -> Vec<Action> {
    let a = |id: &str, label: &str, cat: &str, kind: Kind| Action {
        id: id.into(), label: label.into(), category: cat.into(), kind,
    };
    use Kind::*;
    vec![
        a("Flight_Pitch", "Pitch (nose up/down)", "Flight", Axis),
        a("Flight_Roll", "Roll (bank L/R)", "Flight", Axis),
        a("Flight_Yaw", "Yaw (rudder)", "Flight", Axis),
        a("Flight_Throttle", "Throttle", "Flight", Axis),
        a("Flight_CameraPitch", "Look Up/Down", "Camera", Axis),
        a("Flight_CameraYaw", "Look Left/Right", "Camera", Axis),
        a("Flight_Gun", "Machine Gun", "Weapons", Button),
        a("Flight_Missile", "Missile", "Weapons", Button),
        a("Flight_Weapon", "Special Weapon", "Weapons", Button),
        a("Flight_Target", "Change Target", "Targeting", Button),
        a("Flight_Radar", "Radar / Map", "Systems", Button),
        a("Flight_Flare", "Flares / Counter", "Systems", Button),
        a("Flight_View", "Change View", "Camera", Button),
        a("Flight_AutoPilot", "Auto-Pilot", "Systems", Button),
        a("Flight_Pause", "Pause", "Systems", Button),
        a("Flight_HatSwitchUp", "Hat Up", "Camera", Button),
        a("Flight_HatSwitchDown", "Hat Down", "Camera", Button),
        a("Flight_HatSwitchLeft", "Hat Left", "Camera", Button),
        a("Flight_HatSwitchRight", "Hat Right", "Camera", Button),
    ]
}

/// `"VVVVPPPP|input"` for a known device.
fn tok(d: &devices::KnownDevice, input: &str) -> String {
    format!("{:04X}{:04X}|{}", d.vid, d.pid, input)
}

/// Split a token into (vid, pid, input). None if malformed.
fn parse_token(t: &str) -> Option<(u16, u16, &str)> {
    let (ids, input) = t.split_once('|')?;
    if ids.len() < 8 { return None; }
    let vid = u16::from_str_radix(&ids[0..4], 16).ok()?;
    let pid = u16::from_str_radix(&ids[4..8], 16).ok()?;
    Some((vid, pid, input))
}

/// AC7 input string for one of a device's axes (adds `:R` when reversed).
fn axis_input(a: &devices::AxisMap) -> String {
    if a.reverse { format!("{}:R", a.ac7) } else { a.ac7.to_string() }
}

impl GameProvider for Ac7 {
    fn name(&self) -> &str { "Ace Combat 7" }
    fn available(&self) -> bool { ac7_dir().parent().map(|p| p.exists()).unwrap_or(false) }

    fn config_path(&self) -> std::path::PathBuf {
        if let Ok(p) = std::env::var("AC7_CONFIG") {
            if !p.is_empty() { return std::path::PathBuf::from(p); }
        }
        ac7_dir().join("Input.ini")
    }

    fn actions(&self) -> Vec<Action> { catalog() }

    fn default_bindings(&self) -> Vec<Binding> {
        // A full HOTAS layout: Warthog stick (aim + guns + hat-look), Warthog
        // throttle (throttle + systems), pedals (rudder). Axis bindings come
        // straight from the device registry; button numbers are best-guess and
        // easy to re-bind live. Other sticks are bound by press-to-bind.
        let mut out = Vec::new();
        let b = |id: &str, token: String| Binding { id: id.into(), token, scale: 1.0 };

        // Axis bindings, generated from each device's registry axis map.
        let axis_action = |sem: devices::Sem| -> Option<&'static str> {
            use devices::Sem::*;
            match sem {
                Pitch => Some("Flight_Pitch"),
                Roll => Some("Flight_Roll"),
                Yaw => Some("Flight_Yaw"),
                Throttle => Some("Flight_Throttle"),
            }
        };
        // Order matters: the pedals come BEFORE the throttle so the Warthog throttle
        // lever wins Flight_Throttle (the MRP's toe axis would otherwise grab it).
        for name in ["Thrustmaster Warthog Joystick", "MOZA MRP Rudder Pedals", "Thrustmaster Warthog Throttle"] {
            if let Some(d) = devices::by_name(name) {
                for a in d.axes {
                    if let Some(act) = axis_action(a.sem) {
                        out.push(b(act, tok(d, &axis_input(a))));
                    }
                }
            }
        }

        // Stick buttons + POV hat (Warthog Joystick).
        if let Some(s) = devices::by_name("Thrustmaster Warthog Joystick") {
            out.push(b("Flight_Gun", tok(s, "Button1")));
            out.push(b("Flight_Missile", tok(s, "Button2")));
            out.push(b("Flight_Weapon", tok(s, "Button3")));
            out.push(b("Flight_Target", tok(s, "Button4")));
            out.push(b("Flight_HatSwitchUp", tok(s, "POV_U1")));
            out.push(b("Flight_HatSwitchDown", tok(s, "POV_D1")));
            out.push(b("Flight_HatSwitchLeft", tok(s, "POV_L1")));
            out.push(b("Flight_HatSwitchRight", tok(s, "POV_R1")));
        }
        // Throttle buttons (Warthog Throttle).
        if let Some(t) = devices::by_name("Thrustmaster Warthog Throttle") {
            out.push(b("Flight_Flare", tok(t, "Button1")));
            out.push(b("Flight_Radar", tok(t, "Button2")));
            out.push(b("Flight_View", tok(t, "Button3")));
            out.push(b("Flight_AutoPilot", tok(t, "Button4")));
        }
        out
    }

    fn load(&self) -> Result<Vec<Binding>, String> {
        let path = self.config_path();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let keys: std::collections::HashSet<String> = catalog().into_iter().map(|a| a.id).collect();

        // Parse: track the current device section's ids; collect action lines.
        let mut found: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut cur: Option<(u16, u16)> = None;
        for line in text.lines() {
            let l = line.trim();
            if let Some(rest) = l.strip_prefix("[Joystick-") {
                // header id is "PPPPVVVV-..." (PID then VID, per AC7's GUID layout).
                cur = rest.split('-').next().filter(|h| h.len() >= 8).and_then(|h| {
                    let pid = u16::from_str_radix(&h[0..4], 16).ok()?;
                    let vid = u16::from_str_radix(&h[4..8], 16).ok()?;
                    Some((vid, pid))
                });
            } else if l.starts_with('[') {
                cur = None;
            } else if let Some((k, v)) = l.split_once('=') {
                if let Some((vid, pid)) = cur {
                    if keys.contains(k.trim()) {
                        found.insert(k.trim().to_string(), format!("{:04X}{:04X}|{}", vid, pid, v.trim()));
                    }
                }
            }
        }
        Ok(catalog().into_iter()
            .map(|a| Binding { token: found.get(&a.id).cloned().unwrap_or_default(), id: a.id, scale: 1.0 })
            .collect())
    }

    fn save(&self, bindings: &[Binding]) -> Result<SaveReport, String> {
        let path = self.config_path();
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }

        // backup
        let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let dir = std::path::Path::new(&base).join("MW5-Remap").join("backups");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let mut backup = String::from("(no prior file)");
        if path.exists() {
            let b = dir.join(format!("AC7_Input_{}.ini", stamp));
            std::fs::copy(&path, &b).map_err(|e| e.to_string())?;
            backup = b.display().to_string();
        }

        // Group bound actions by device, preserving catalog order.
        let order: Vec<String> = catalog().into_iter().map(|a| a.id).collect();
        let bmap: std::collections::HashMap<&str, &Binding> = bindings.iter().map(|b| (b.id.as_str(), b)).collect();
        let mut devs: Vec<(u16, u16)> = Vec::new();
        let mut lines: std::collections::HashMap<(u16, u16), Vec<String>> = std::collections::HashMap::new();
        let mut changed = Vec::new();
        for id in &order {
            if let Some(b) = bmap.get(id.as_str()) {
                if b.token.is_empty() { continue; }
                if let Some((vid, pid, input)) = parse_token(&b.token) {
                    if !devs.contains(&(vid, pid)) { devs.push((vid, pid)); }
                    lines.entry((vid, pid)).or_default().push(format!("{}={}", id, input));
                    changed.push(format!("{} -> {}", id, b.token));
                }
            }
        }

        let mut out = String::from(
            "[JoystickSetting]\r\nEnableJoystick=True\r\nEnableDeviceJoystick=True\r\nEnableDeviceFlight=True\r\nEnableDevice1stPerson=True\r\n",
        );
        for (vid, pid) in &devs {
            let name = devices::name_for(*vid, *pid).unwrap_or("Joystick");
            out.push_str(&format!("\r\n[Joystick-{}]\r\n", devices::dinput_guid(*vid, *pid)));
            out.push_str(&format!("ProductName={}\r\n", name));
            for line in &lines[&(*vid, *pid)] { out.push_str(line); out.push_str("\r\n"); }
        }

        std::fs::write(&path, out).map_err(|e| e.to_string())?;
        Ok(SaveReport { backup, changed, missing: Vec::new() })
    }

    // --- press-to-bind: capture the device + AC7 input ---
    fn role_of(&self, _dev: &Device, _idx: usize) -> Role { Role::Joystick }

    fn button_token(&self, dev: &Device, button: u32, _idx: usize) -> Option<String> {
        Some(format!("{:04X}{:04X}|Button{}", dev.vid, dev.pid, button))
    }

    fn axis_token(&self, dev: &Device, axis_index: usize, _idx: usize) -> Option<String> {
        // winmm slot -> AC7 axis letter (X,Y,Z,Rz,Rx,Ry).
        let letter = ["X", "Y", "Z", "Rz", "Rx", "Ry"].get(axis_index).copied()?;
        Some(format!("{:04X}{:04X}|{}", dev.vid, dev.pid, letter))
    }

    fn pov_token(&self, dev: &Device, octant: u32, _idx: usize) -> Option<String> {
        let dir = match octant { 1 => "U", 3 => "R", 5 => "D", 7 => "L", _ => return None };
        Some(format!("{:04X}{:04X}|POV_{}1", dev.vid, dev.pid, dir))
    }

    fn launch_uri(&self) -> Option<String> { Some("steam://rungameid/502500".into()) }
    fn running_processes(&self) -> Vec<String> { vec!["Ace7Game-Win64-Shipping".into(), "Ace7Game".into()] }
}
