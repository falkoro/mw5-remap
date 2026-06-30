//! Star Citizen provider. Bindings live in CryEngine/Lumberyard XML at
//! `<install>\LIVE\user\client\0\Profiles\default\actionmaps.xml`: per-device
//! `<options type="joystick" instance="N" Product="… {GUID}"/>` plus
//! `<actionmap name="…"><action name="…"><rebind input="jsN_…"/></action>`.
//! The Product GUID is the same DirectInput layout AC7 uses, so we build it from a
//! device's USB ids — no hardcoded VKB ids needed (press-to-bind captures them).
//!
//! Token form here = `"VVVVPPPP|input"` (input without the `jsN_` prefix, e.g.
//! `231D0200|y` or `231D0200|button5`); save() assigns the `jsN` instance numbers.

use super::{Action, Binding, GameProvider, Kind, Role, SaveReport};
use crate::devices;
use crate::input::Device;

pub struct Sc;
impl Sc {
    pub fn new() -> Self { Sc }
}

/// SC install candidates (LIVE channel). First existing actionmaps.xml wins.
fn sc_candidates() -> Vec<std::path::PathBuf> {
    let tail = "StarCitizen/LIVE/user/client/0/Profiles/default/actionmaps.xml";
    let mut v = Vec::new();
    for root in ["C:/Program Files/Roberts Space Industries", "D:/Roberts Space Industries", "E:/Roberts Space Industries"] {
        v.push(std::path::Path::new(root).join(tail));
    }
    v
}

/// (actionmap, action, label, category, kind). Curated core flight controls.
fn catalog_full() -> Vec<(&'static str, Action)> {
    let a = |map: &'static str, id: &str, label: &str, cat: &str, kind: Kind| {
        (map, Action { id: id.into(), label: label.into(), category: cat.into(), kind })
    };
    use Kind::*;
    vec![
        a("spaceship_movement", "v_pitch", "Pitch", "Flight", Axis),
        a("spaceship_movement", "v_yaw", "Yaw", "Flight", Axis),
        a("spaceship_movement", "v_roll", "Roll", "Flight", Axis),
        a("spaceship_movement", "v_throttle_abs", "Throttle (absolute)", "Flight", Axis),
        a("spaceship_movement", "v_strafe_lateral", "Strafe L/R", "Flight", Axis),
        a("spaceship_movement", "v_strafe_vertical", "Strafe U/D", "Flight", Axis),
        a("spaceship_movement", "v_strafe_longitudinal", "Strafe Fwd/Back", "Flight", Axis),
        a("spaceship_movement", "v_boost", "Boost", "Flight", Button),
        a("spaceship_movement", "v_afterburner", "Afterburner", "Flight", Button),
        a("spaceship_movement", "v_toggle_landing_system", "Landing Gear", "Flight", Button),
        a("spaceship_weapons", "v_attack1_group1", "Fire Group 1", "Weapons", Button),
        a("spaceship_weapons", "v_attack1_group2", "Fire Group 2", "Weapons", Button),
        a("spaceship_missiles", "v_weapon_launch_missile", "Launch Missile", "Weapons", Button),
        a("spaceship_targeting", "v_target_nearest_hostile", "Target Nearest Hostile", "Targeting", Button),
        a("spaceship_targeting", "v_target_cycle_hostile_fwd", "Cycle Hostile Target", "Targeting", Button),
        a("spaceship_targeting", "v_target_cycle_all_fwd", "Cycle Any Target", "Targeting", Button),
        a("spaceship_targeting", "v_toggle_weapon_gimbal_lock", "Gimbal Lock", "Weapons", Button),
        a("spaceship_power", "v_power_focus_group_1", "Power -> Weapons", "Systems", Button),
        a("spaceship_power", "v_power_focus_group_3", "Power -> Engines", "Systems", Button),
        // --- essential extras (audited) ---
        a("spaceship_general", "v_flightready", "Flight Ready (engines on)", "Systems", Button),
        a("spaceship_movement", "v_toggle_qdrive_engagement", "Quantum Travel", "Flight", Button),
        a("spaceship_movement", "v_ifcs_toggle_vector_decoupling", "Decoupled Mode", "Flight", Button),
        a("spaceship_defensive", "v_weapon_launch_countermeasure", "Countermeasure", "Systems", Button),
        a("spaceship_view", "v_view_freelook_mode", "Free Look (hold)", "Camera", Button),
        a("spaceship_view", "v_view_look_behind", "Look Behind", "Camera", Button),
    ]
}

fn catalog() -> Vec<Action> { catalog_full().into_iter().map(|(_, a)| a).collect() }

fn parse_token(t: &str) -> Option<(u16, u16, &str)> {
    let (ids, input) = t.split_once('|')?;
    if ids.len() < 8 { return None; }
    let vid = u16::from_str_radix(&ids[0..4], 16).ok()?;
    let pid = u16::from_str_radix(&ids[4..8], 16).ok()?;
    Some((vid, pid, input))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

impl GameProvider for Sc {
    fn name(&self) -> &str { "Star Citizen" }
    fn available(&self) -> bool { self.config_path().exists() }

    fn config_path(&self) -> std::path::PathBuf {
        if let Ok(p) = std::env::var("SC_CONFIG") {
            if !p.is_empty() { return std::path::PathBuf::from(p); }
        }
        sc_candidates().into_iter().find(|p| p.exists())
            .unwrap_or_else(|| sc_candidates().remove(0))
    }

    fn actions(&self) -> Vec<Action> { catalog() }

    fn load(&self) -> Result<Vec<Binding>, String> {
        let text = std::fs::read_to_string(self.config_path()).unwrap_or_default();
        let keys: std::collections::HashSet<String> = catalog().into_iter().map(|a| a.id).collect();

        // instance -> (vid,pid) from the <options ... Product="… {PPPPVVVV-…}"/> lines.
        let mut inst: std::collections::HashMap<u32, (u16, u16)> = std::collections::HashMap::new();
        for line in text.lines() {
            let l = line.trim();
            if l.starts_with("<options") && l.contains("type=\"joystick\"") {
                let n = attr(l, "instance").and_then(|s| s.parse::<u32>().ok());
                let ids = attr(l, "Product").and_then(|p| guid_ids(&p));
                if let (Some(n), Some((vid, pid))) = (n, ids) { inst.insert(n, (vid, pid)); }
            }
        }
        // <action name="X"> ... <rebind input="jsN_input"/>
        let mut found: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut cur_action: Option<String> = None;
        for line in text.lines() {
            let l = line.trim();
            if l.starts_with("<action ") {
                cur_action = attr(l, "name");
            }
            if l.contains("<rebind") {
                if let (Some(act), Some(input)) = (cur_action.clone(), attr(l, "input")) {
                    // input = "jsN_key" (e.g. "js1_y", "js2_button5", "js1_hat1_up")
                    if let Some((ns, key)) = input.strip_prefix("js").and_then(|s| s.split_once('_')) {
                        if let Ok(n) = ns.parse::<u32>() {
                            if let Some(&(vid, pid)) = inst.get(&n) {
                                if keys.contains(&act) {
                                    found.insert(act, format!("{:04X}{:04X}|{}", vid, pid, key));
                                }
                            }
                        }
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

        let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let dir = std::path::Path::new(&base).join("MW5-Remap").join("backups");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let mut backup = String::from("(no prior file)");
        if path.exists() {
            let b = dir.join(format!("SC_actionmaps_{}.xml", stamp));
            std::fs::copy(&path, &b).map_err(|e| e.to_string())?;
            backup = b.display().to_string();
        }

        // Assign js instance numbers in first-seen device order; remember ids for Product.
        let mut order: Vec<(u16, u16)> = Vec::new();
        let bmap: std::collections::HashMap<&str, (u16, u16, String)> = bindings.iter()
            .filter(|b| !b.token.is_empty())
            .filter_map(|b| parse_token(&b.token).map(|(v, p, i)| (b.id.as_str(), (v, p, i.to_string()))))
            .collect();
        for a in catalog() { // stable order
            if let Some((v, p, _)) = bmap.get(a.id.as_str()) {
                if !order.contains(&(*v, *p)) { order.push((*v, *p)); }
            }
        }
        let inst_of = |v: u16, p: u16| order.iter().position(|&x| x == (v, p)).map(|i| i as u32 + 1).unwrap_or(1);
        // Friendly names from the live devices, falling back to the registry / generic.
        let live = crate::input::poll();
        let name_of = |v: u16, p: u16| -> String {
            live.iter().find(|d| d.vid == v && d.pid == p).map(|d| d.name.clone())
                .or_else(|| devices::name_for(v, p).map(|s| s.to_string()))
                .unwrap_or_else(|| "Joystick".into())
        };

        // Build XML.
        let mut x = String::from("<ActionMaps>\r\n <ActionProfiles version=\"1\" optionsVersion=\"2\" rebindVersion=\"2\" profileName=\"default\">\r\n");
        x.push_str("  <options type=\"keyboard\" instance=\"1\" Product=\"Keyboard  {6F1D2B61-D5A0-11CF-BFC7-444553540000}\"/>\r\n");
        for &(v, p) in &order {
            x.push_str(&format!(
                "  <options type=\"joystick\" instance=\"{}\" Product=\"{} {{{}}}\"/>\r\n",
                inst_of(v, p), xml_escape(&name_of(v, p)), devices::dinput_guid(v, p)
            ));
        }
        // Group bound actions by actionmap, in catalog order.
        let mut changed = Vec::new();
        let mut maps: Vec<&str> = Vec::new();
        for (m, _) in catalog_full() { if !maps.contains(&m) { maps.push(m); } }
        for m in maps {
            let acts: Vec<(String, String)> = catalog_full().into_iter()
                .filter(|(am, _)| *am == m)
                .filter_map(|(_, a)| bmap.get(a.id.as_str()).map(|(v, p, input)| {
                    (a.id.clone(), format!("js{}_{}", inst_of(*v, *p), input))
                }))
                .collect();
            if acts.is_empty() { continue; }
            x.push_str(&format!("  <actionmap name=\"{}\">\r\n", m));
            for (id, input) in acts {
                x.push_str(&format!("   <action name=\"{}\"><rebind input=\"{}\"/></action>\r\n", id, input));
                changed.push(format!("{} -> {}", id, input));
            }
            x.push_str("  </actionmap>\r\n");
        }
        x.push_str("  <modifiers />\r\n </ActionProfiles>\r\n</ActionMaps>\r\n");

        std::fs::write(&path, x).map_err(|e| e.to_string())?;
        Ok(SaveReport { backup, changed, missing: Vec::new() })
    }

    // --- press-to-bind: capture device ids + the SC input name ---
    fn role_of(&self, _dev: &Device, _idx: usize) -> Role { Role::Joystick }

    fn button_token(&self, dev: &Device, button: u32, _idx: usize) -> Option<String> {
        Some(format!("{:04X}{:04X}|button{}", dev.vid, dev.pid, button))
    }
    fn axis_token(&self, dev: &Device, axis_index: usize, _idx: usize) -> Option<String> {
        let name = ["x", "y", "z", "rotz", "rotx", "roty"].get(axis_index).copied()?;
        Some(format!("{:04X}{:04X}|{}", dev.vid, dev.pid, name))
    }
    fn pov_token(&self, dev: &Device, octant: u32, _idx: usize) -> Option<String> {
        let dir = match octant { 1 => "up", 3 => "right", 5 => "down", 7 => "left", _ => return None };
        Some(format!("{:04X}{:04X}|hat1_{}", dev.vid, dev.pid, dir))
    }

    fn launch_uri(&self) -> Option<String> { None }
    fn running_processes(&self) -> Vec<String> { vec!["StarCitizen".into(), "RSI Launcher".into()] }
}

/// Read an XML attribute value: `name="value"`.
fn attr(line: &str, name: &str) -> Option<String> {
    let needle = format!("{}=\"", name);
    let i = line.find(&needle)? + needle.len();
    let rest = &line[i..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Pull (vid,pid) from a Product string's `{PPPPVVVV-0000-0000-0000-504944564944}`.
fn guid_ids(product: &str) -> Option<(u16, u16)> {
    let open = product.rfind('{')? + 1;
    let g = &product[open..];
    if g.len() < 8 { return None; }
    let pid = u16::from_str_radix(&g[0..4], 16).ok()?;
    let vid = u16::from_str_radix(&g[4..8], 16).ok()?;
    Some((vid, pid))
}
