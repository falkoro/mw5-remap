//! Shared registry of known HOTAS devices. Each game renders its own config from
//! these (MW5 -> HOTASMappings.Remap tokens; AC7 -> Input.ini axis/button lines),
//! so a device is described once here and "just works" in every supported game.

use crate::games::Role;

/// What a physical axis controls. Drives both the MW5 token and the AC7 action.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sem { Pitch, Roll, Yaw, Throttle }

/// One physical axis on a device, named for each game's config format.
#[derive(Clone, Copy)]
pub struct AxisMap {
    pub sem: Sem,
    pub hotas: &'static str, // MW5 .Remap InAxis (e.g. "HOTAS_YAxis")
    pub ac7: &'static str,   // AC7 Input.ini axis letter (e.g. "Y", "Rz")
    pub reverse: bool,       // natural orientation needs reversing (AC7 ":R")
}
const fn ax(sem: Sem, hotas: &'static str, ac7: &'static str, reverse: bool) -> AxisMap {
    AxisMap { sem, hotas, ac7, reverse }
}

/// A known controller: identity, role, and how its axes/buttons map.
pub struct KnownDevice {
    pub name: &'static str,
    pub vid: u16,
    pub pid: u16,
    pub role: Role,
    pub buttons: u32,
    pub has_hat: bool,
    pub axes: &'static [AxisMap],
    /// A placeholder template (real IDs unknown) — written once, never auto-managed.
    pub custom: bool,
}

use Sem::*;

// AB6 gimbal: Y=pitch, X=roll. 32 buttons (only 1..20 usable as MW5 tokens), POV hat.
const AB6_AXES: &[AxisMap] = &[ax(Pitch, "HOTAS_YAxis", "Y", true), ax(Roll, "HOTAS_XAxis", "X", false)];
// MRP pedals: rudder slide (Rz) = yaw/turn; a toe (Y) = throttle/forward.
const MRP_AXES: &[AxisMap] = &[ax(Yaw, "HOTAS_RZAxis", "Rz", false), ax(Throttle, "HOTAS_YAxis", "Y", false)];
// Warthog stick: X=roll, Y=pitch, POV hat, 19 buttons.
const WH_STICK_AXES: &[AxisMap] = &[ax(Pitch, "HOTAS_YAxis", "Y", true), ax(Roll, "HOTAS_XAxis", "X", false)];
// Warthog throttle: main throttle lever on Z; lots of buttons; no usable hat axis here.
const WH_THR_AXES: &[AxisMap] = &[ax(Throttle, "HOTAS_ZAxis", "Z", false)];
// Custom pedal template: assume a single self-centering rudder axis = yaw/turn.
const CUSTOM_AXES: &[AxisMap] = &[ax(Yaw, "HOTAS_RZAxis", "Rz", false)];

const REGISTRY: &[KnownDevice] = &[
    KnownDevice { name: "MOZA AB6 FFB Base", vid: 0x346E, pid: 0x1002, role: Role::Joystick, buttons: 32, has_hat: true, axes: AB6_AXES, custom: false },
    KnownDevice { name: "MOZA MRP Rudder Pedals", vid: 0x346E, pid: 0x1200, role: Role::Throttle, buttons: 0, has_hat: false, axes: MRP_AXES, custom: false },
    KnownDevice { name: "Thrustmaster Warthog Joystick", vid: 0x044F, pid: 0x0402, role: Role::Joystick, buttons: 19, has_hat: true, axes: WH_STICK_AXES, custom: false },
    KnownDevice { name: "Thrustmaster Warthog Throttle", vid: 0x044F, pid: 0x0404, role: Role::Throttle, buttons: 19, has_hat: false, axes: WH_THR_AXES, custom: false },
    KnownDevice { name: "Custom Pedal (edit IDs)", vid: 0x0000, pid: 0x0000, role: Role::Throttle, buttons: 0, has_hat: false, axes: CUSTOM_AXES, custom: true },
];

/// All known devices.
pub fn registry() -> &'static [KnownDevice] { REGISTRY }

/// Look up a known device by its friendly name.
pub fn by_name(name: &str) -> Option<&'static KnownDevice> {
    REGISTRY.iter().find(|d| d.name == name)
}

/// Look up a known device by USB ids (returns its friendly name if known).
pub fn name_for(vid: u16, pid: u16) -> Option<&'static str> {
    REGISTRY.iter().find(|d| d.vid == vid && d.pid == pid).map(|d| d.name)
}

/// The DirectInput product GUID used by AC7's section id and SC's `Product=`:
/// PID then VID as 8 hex chars + the fixed "PIDVID" (504944564944) tail.
pub fn dinput_guid(vid: u16, pid: u16) -> String {
    format!("{:04X}{:04X}-0000-0000-0000-504944564944", pid, vid)
}
