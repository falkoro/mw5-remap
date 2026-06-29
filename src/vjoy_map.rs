//! Config-driven physical-joystick -> vJoy routing — a built-in Joystick-Gremlin
//! replacement. The user builds mappings in the UI (or auto-routes a whole stick);
//! each maps one physical button bit / axis of a device (by VID/PID) onto a vJoy
//! button or axis. Bindings live in a TEXT file (no serde, no code edits), so ANY
//! stick can be routed onto vJoy device 1 without touching the source.
//!
//! File: `%LOCALAPPDATA%\MW5-Remap\vjoy_map.txt`, one mapping per line, tab-separated:
//!   `VID<TAB>PID<TAB>SRC<TAB>TGT<TAB>INV`  (VID/PID hex; SRC/TGT `B<n>`|`A<n>`; INV 0/1)
//! e.g. `346E\t1002\tB0\tB1\t0` = device 346E:1002 button-bit 0 -> vJoy Button 1.

use crate::input::Device;
use crate::vjoy;
use std::path::PathBuf;

/// A physical input on a device: a button BIT index (0..31) or an axis index (0..7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Button(u8),
    Axis(u8),
    /// Two axes combined into ONE bipolar output: (forward/positive index, reverse/negative index).
    /// e.g. two toe pedals -> one centred throttle (forward toe up, reverse toe down).
    Pair(u8, u8),
    /// The device's whole digital POV hat (no index) -> the vJoy POV.
    Pov,
}

/// A vJoy output: a button (1..128) or an axis (by HID usage id, e.g. `HID_X`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Button(u8),
    Axis(u32),
    /// The vJoy device's POV (no index). Fed from a physical POV hat.
    Pov,
}

/// One physical-input -> vJoy-output routing for a device identified by VID/PID.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Mapping {
    pub vid: u16,
    pub pid: u16,
    pub source: Source,
    pub target: Target,
    pub invert: bool,
}

/// The whole routing table.
#[derive(Default)]
pub struct VjoyMap {
    pub mappings: Vec<Mapping>,
}

/// The six vJoy axes we auto-route onto, in order (X,Y,Z,Rx,Ry,Rz).
pub const VJOY_AXES: [u32; 6] = [vjoy::HID_X, vjoy::HID_Y, vjoy::HID_Z, vjoy::HID_RX, vjoy::HID_RY, vjoy::HID_RZ];

/// Friendly axis name for a vJoy HID usage id.
pub fn axis_name(usage: u32) -> String {
    match usage {
        vjoy::HID_X => "X".into(),
        vjoy::HID_Y => "Y".into(),
        vjoy::HID_Z => "Z".into(),
        vjoy::HID_RX => "Rx".into(),
        vjoy::HID_RY => "Ry".into(),
        vjoy::HID_RZ => "Rz".into(),
        u => format!("U{u}"),
    }
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Source::Button(b) => format!("Button {}", b + 1),
            Source::Axis(a) => format!("Axis {}", a + 1),
            Source::Pair(p, n) => format!("Axes {}+ / {}-", p + 1, n + 1),
            Source::Pov => "POV hat".into(),
        }
    }
    fn encode(&self) -> String {
        match self {
            Source::Button(b) => format!("B{b}"),
            Source::Axis(a) => format!("A{a}"),
            Source::Pair(p, n) => format!("P{p}.{n}"),
            Source::Pov => "H".into(),
        }
    }
    fn decode(s: &str) -> Option<Source> {
        // `H` = the whole digital POV hat.
        if s == "H" { return Some(Source::Pov); }
        // `P{p}.{n}` = bipolar axis pair (both indices 0..7).
        if let Some(rest) = s.strip_prefix('P') {
            let (p, n) = rest.split_once('.')?;
            let (p, n) = (p.parse::<u8>().ok()?, n.parse::<u8>().ok()?);
            if p >= 8 || n >= 8 { return None; }
            return Some(Source::Pair(p, n));
        }
        let n = s.get(1..)?.parse::<u8>().ok()?;
        match s.as_bytes().first()? {
            // Reject button bits >= 32: resolve()'s `1u32 << bit` would overflow/UB.
            b'B' if n < 32 => Some(Source::Button(n)),
            b'A' => Some(Source::Axis(n)),
            _ => None,
        }
    }
}

impl Target {
    pub fn label(&self) -> String {
        match self {
            Target::Button(b) => format!("vJoy Button {b}"),
            Target::Axis(u) => format!("vJoy Axis {}", axis_name(*u)),
            Target::Pov => "vJoy POV".into(),
        }
    }
    fn encode(&self) -> String {
        match self {
            Target::Button(b) => format!("B{b}"),
            Target::Axis(u) => format!("A{u}"),
            Target::Pov => "H".into(),
        }
    }
    fn decode(s: &str) -> Option<Target> {
        if s == "H" { return Some(Target::Pov); }
        let rest = s.get(1..)?;
        match s.as_bytes().first()? {
            b'B' => Some(Target::Button(rest.parse().ok()?)),
            b'A' => Some(Target::Axis(rest.parse().ok()?)),
            _ => None,
        }
    }
}

/// One concrete vJoy call apply() would make — pure, so apply's logic is testable
/// without a real vJoy driver.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Call {
    Button(u8, bool),
    Axis(u32, i32),
    /// Continuous POV value: centi-degrees 0..=35999, or -1 to centre.
    Pov(i32),
}

fn map_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(base).join("MW5-Remap").join("vjoy_map.txt")
}

fn encode_line(m: &Mapping) -> String {
    format!("{:04X}\t{:04X}\t{}\t{}\t{}", m.vid, m.pid, m.source.encode(), m.target.encode(), m.invert as u8)
}

fn parse_line(line: &str) -> Option<Mapping> {
    let mut it = line.split('\t');
    let vid = u16::from_str_radix(it.next()?.trim(), 16).ok()?;
    let pid = u16::from_str_radix(it.next()?.trim(), 16).ok()?;
    let source = Source::decode(it.next()?.trim())?;
    let target = Target::decode(it.next()?.trim())?;
    let invert = it.next().unwrap_or("0").trim() != "0";
    Some(Mapping { vid, pid, source, target, invert })
}

impl VjoyMap {
    /// Load the routing table from disk (empty if missing/unreadable).
    pub fn load() -> VjoyMap {
        let mut map = VjoyMap::default();
        if let Ok(text) = std::fs::read_to_string(map_path()) {
            for line in text.lines() {
                if line.trim().is_empty() { continue; }
                if let Some(m) = parse_line(line) { map.mappings.push(m); }
            }
        }
        map
    }

    /// Persist the routing table.
    pub fn save(&self) -> Result<(), String> {
        let p = map_path();
        if let Some(dir) = p.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
        let mut s = String::new();
        for m in &self.mappings { s.push_str(&encode_line(m)); s.push_str("\r\n"); }
        std::fs::write(&p, s).map_err(|e| e.to_string())
    }

    /// Add a mapping, replacing any existing one for the same device + physical source
    /// (so re-binding a control overwrites it). Does NOT save — the caller does.
    pub fn add(&mut self, m: Mapping) {
        self.mappings.retain(|x| !(x.vid == m.vid && x.pid == m.pid && x.source == m.source));
        self.mappings.push(m);
    }

    /// Remove the mapping at `idx` (no-op if out of range).
    pub fn remove(&mut self, idx: usize) {
        if idx < self.mappings.len() { self.mappings.remove(idx); }
    }

    /// The next unused vJoy button number (max used + 1, or 1 if none).
    pub fn next_free_button(&self) -> u8 {
        self.mappings.iter()
            .filter_map(|m| if let Target::Button(b) = m.target { Some(b) } else { None })
            .max().map(|b| b.saturating_add(1).min(128)).unwrap_or(1)
    }

    fn used_axes(&self) -> Vec<u32> {
        self.mappings.iter().filter_map(|m| if let Target::Axis(u) = m.target { Some(u) } else { None }).collect()
    }

    /// Auto-route a whole stick: every button -> sequential free vJoy buttons, and each
    /// present axis -> the next free vJoy axis (X,Y,Z,Rx,Ry,Rz). The fast path.
    pub fn auto_route(&mut self, dev: &Device) {
        let mut next = self.next_free_button();
        for bit in 0..dev.num_buttons.min(32) as u8 {
            if next > 128 { break; }
            self.add(Mapping { vid: dev.vid, pid: dev.pid, source: Source::Button(bit), target: Target::Button(next), invert: false });
            next += 1;
        }
        let mut used = self.used_axes();
        for slot in 0..8u8 {
            if !dev.present[slot as usize] { continue; }
            let free = VJOY_AXES.iter().copied().find(|u| !used.contains(u));
            if let Some(usage) = free {
                self.add(Mapping { vid: dev.vid, pid: dev.pid, source: Source::Axis(slot), target: Target::Axis(usage), invert: false });
                used.push(usage);
            }
        }
    }

    /// Pure: the vJoy calls this map produces for the given device state.
    pub fn resolve(&self, devices: &[Device]) -> Vec<Call> {
        let mut out = Vec::new();
        for m in &self.mappings {
            let dev = match devices.iter().find(|d| d.vid == m.vid && d.pid == m.pid) {
                Some(d) => d, None => continue,
            };
            let pressed = |bit: u8| dev.buttons & (1u32 << bit) != 0;
            let axis = |ax: u8| dev.axes.get(ax as usize).copied().unwrap_or(0);
            out.push(match (m.source, m.target) {
                (Source::Button(b), Target::Button(t)) => Call::Button(t, pressed(b) ^ m.invert),
                (Source::Axis(a), Target::Axis(u)) => {
                    let v = axis(a);
                    Call::Axis(u, vjoy::scale(if m.invert { 65535 - v } else { v }))
                }
                (Source::Axis(a), Target::Button(t)) => Call::Button(t, (axis(a) > 32767) ^ m.invert),
                (Source::Button(b), Target::Axis(u)) => {
                    let on = pressed(b) ^ m.invert;
                    Call::Axis(u, vjoy::scale(if on { 65535 } else { 0 }))
                }
                // Two axes -> one centred bipolar axis. combine_toes already returns the
                // vJoy-scaled value, so DON'T wrap in scale(). invert swaps fwd/rev.
                (Source::Pair(p, n), Target::Axis(u)) => {
                    let (fwd, rev) = (axis(p), axis(n));
                    Call::Axis(u, if m.invert { vjoy::combine_toes(rev, fwd) } else { vjoy::combine_toes(fwd, rev) })
                }
                // Whole digital POV hat -> the vJoy POV. Centred (0xFFFFFFFF / >35999)
                // becomes -1 (vJoy "centre"); a real angle passes through unchanged.
                (Source::Pov, Target::Pov) => {
                    let value = if dev.pov == 0xFFFF_FFFF || dev.pov > 35999 { -1 } else { dev.pov as i32 };
                    Call::Pov(value)
                }
                // Any other combination involving Pair/Pov is meaningless; skip it.
                _ => continue,
            });
        }
        out
    }

    /// Feed the resolved calls to vJoy device 1.
    pub fn apply(&self, devices: &[Device]) {
        for c in self.resolve(devices) {
            match c {
                Call::Button(b, on) => { vjoy::feed_button(b, on); }
                Call::Axis(u, v) => { vjoy::feed(u, v); }
                Call::Pov(v) => { vjoy::feed_pov(v); }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(vid: u16, pid: u16) -> Device {
        Device { id: 0, vid, pid, name: "t".into(), num_axes: 8, num_buttons: 32,
            axes: [0; 8], present: [true; 8], buttons: 0, pov: 0xFFFF, has_pov: false }
    }

    #[test]
    fn line_round_trip() {
        let cases = [
            Mapping { vid: 0x346E, pid: 0x1002, source: Source::Button(0), target: Target::Button(1), invert: false },
            Mapping { vid: 0x231D, pid: 0x0201, source: Source::Axis(4), target: Target::Axis(vjoy::HID_Z), invert: true },
        ];
        for m in cases {
            let line = encode_line(&m);
            let back = parse_line(&line).expect("parse");
            assert_eq!(back, m, "round-trip failed for line {line:?}");
        }
    }

    #[test]
    fn pair_line_round_trip() {
        let m = Mapping { vid: 0x346E, pid: 0x1200, source: Source::Pair(0, 1),
            target: Target::Axis(vjoy::HID_X), invert: false };
        let line = encode_line(&m);
        assert_eq!(line.split('\t').nth(2), Some("P0.1"), "pair encodes as P{{p}}.{{n}}");
        assert_eq!(parse_line(&line).expect("parse"), m, "round-trip failed for {line:?}");
        // Out-of-range indices are rejected.
        assert_eq!(Source::decode("P8.0"), None);
    }

    #[test]
    fn resolve_pair_combines_toes() {
        let map = VjoyMap { mappings: vec![
            Mapping { vid: 0x346E, pid: 0x1200, source: Source::Pair(0, 1),
                target: Target::Axis(vjoy::HID_X), invert: false },
        ] };
        let axis_val = |d: Device| match map.resolve(&[d]).as_slice() {
            [Call::Axis(_, v)] => *v, other => panic!("expected one axis call, got {other:?}"),
        };
        // both toes at rest -> centre
        assert_eq!(axis_val(dev(0x346E, 0x1200)), vjoy::VJOY_CENTRE);
        // forward axis high -> above centre
        let mut fwd = dev(0x346E, 0x1200); fwd.axes[0] = 65535;
        assert!(axis_val(fwd) > vjoy::VJOY_CENTRE, "forward toe should push above centre");
        // reverse axis high -> below centre
        let mut rev = dev(0x346E, 0x1200); rev.axes[1] = 65535;
        assert!(axis_val(rev) < vjoy::VJOY_CENTRE, "reverse toe should push below centre");
    }

    #[test]
    fn pov_line_round_trips() {
        let m = Mapping { vid: 0x346E, pid: 0x1002, source: Source::Pov,
            target: Target::Pov, invert: false };
        let line = encode_line(&m);
        assert_eq!(line.split('\t').nth(2), Some("H"), "POV source encodes as H");
        assert_eq!(line.split('\t').nth(3), Some("H"), "POV target encodes as H");
        assert_eq!(parse_line(&line).expect("parse"), m, "round-trip failed for {line:?}");
    }

    #[test]
    fn resolve_pov_centre_and_angle() {
        let map = VjoyMap { mappings: vec![
            Mapping { vid: 0x346E, pid: 0x1002, source: Source::Pov,
                target: Target::Pov, invert: false },
        ] };
        // centered hat (0xFFFFFFFF) -> -1
        let mut c = dev(0x346E, 0x1002); c.pov = 0xFFFF_FFFF;
        assert_eq!(map.resolve(&[c]).as_slice(), &[Call::Pov(-1)]);
        // a real angle passes through unchanged
        let mut a = dev(0x346E, 0x1002); a.pov = 9000;
        assert_eq!(map.resolve(&[a]).as_slice(), &[Call::Pov(9000)]);
    }

    #[test]
    fn parse_skips_garbage() {
        assert!(parse_line("not a mapping").is_none());
        assert!(parse_line("").is_none());
    }

    #[test]
    fn parse_rejects_button_bit_ge_32() {
        // A hand-edited line with B40 would overflow resolve()'s `1u32 << bit` — reject it.
        assert!(parse_line("346E\t1002\tB40\tB1\t0").is_none());
        assert_eq!(Source::decode("B32"), None);
        assert_eq!(Source::decode("B31"), Some(Source::Button(31)));
    }

    #[test]
    fn next_free_button_caps_at_128() {
        let mut map = VjoyMap::default();
        map.add(Mapping { vid: 1, pid: 2, source: Source::Button(0), target: Target::Button(255), invert: false });
        assert_eq!(map.next_free_button(), 128);
        let mut map = VjoyMap::default();
        map.add(Mapping { vid: 1, pid: 2, source: Source::Button(0), target: Target::Button(128), invert: false });
        assert_eq!(map.next_free_button(), 128);
    }

    #[test]
    fn apply_resolves_button_and_axis() {
        let mut d = dev(0x346E, 0x1002);
        d.buttons = 1 << 3;       // button bit 3 held
        d.axes[0] = 65535;        // axis 0 full
        let map = VjoyMap { mappings: vec![
            Mapping { vid: 0x346E, pid: 0x1002, source: Source::Button(3), target: Target::Button(5), invert: false },
            Mapping { vid: 0x346E, pid: 0x1002, source: Source::Button(2), target: Target::Button(6), invert: false },
            Mapping { vid: 0x346E, pid: 0x1002, source: Source::Axis(0), target: Target::Axis(vjoy::HID_X), invert: false },
            Mapping { vid: 0x346E, pid: 0x1002, source: Source::Axis(0), target: Target::Axis(vjoy::HID_Y), invert: true },
        ] };
        let calls = map.resolve(&[d]);
        assert_eq!(calls[0], Call::Button(5, true));   // held
        assert_eq!(calls[1], Call::Button(6, false));  // not held
        assert_eq!(calls[2], Call::Axis(vjoy::HID_X, vjoy::scale(65535)));
        assert_eq!(calls[3], Call::Axis(vjoy::HID_Y, vjoy::scale(0))); // inverted full -> 0
    }

    #[test]
    fn resolve_ignores_absent_device() {
        let map = VjoyMap { mappings: vec![
            Mapping { vid: 0xDEAD, pid: 0xBEEF, source: Source::Button(0), target: Target::Button(1), invert: false },
        ] };
        assert!(map.resolve(&[dev(0x0001, 0x0002)]).is_empty());
    }

    #[test]
    fn auto_route_assigns_sequential_buttons_and_free_axes() {
        let mut d = dev(0x346E, 0x1002);
        d.num_buttons = 3;
        d.present = [true, true, false, false, false, false, false, false];
        let mut map = VjoyMap::default();
        map.auto_route(&d);
        // 3 buttons -> vJoy 1,2,3 ; 2 present axes -> X,Y
        let btns: Vec<_> = map.mappings.iter().filter_map(|m| if let Target::Button(b) = m.target { Some(b) } else { None }).collect();
        assert_eq!(btns, vec![1, 2, 3]);
        let axes: Vec<_> = map.mappings.iter().filter_map(|m| if let Target::Axis(u) = m.target { Some(u) } else { None }).collect();
        assert_eq!(axes, vec![vjoy::HID_X, vjoy::HID_Y]);
        assert_eq!(map.next_free_button(), 4);
    }

    #[test]
    fn add_replaces_same_source() {
        let mut map = VjoyMap::default();
        map.add(Mapping { vid: 1, pid: 2, source: Source::Button(0), target: Target::Button(1), invert: false });
        map.add(Mapping { vid: 1, pid: 2, source: Source::Button(0), target: Target::Button(9), invert: false });
        assert_eq!(map.mappings.len(), 1);
        assert_eq!(map.mappings[0].target, Target::Button(9));
    }
}
