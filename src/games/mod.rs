//! Game-provider abstraction. Each supported game implements `GameProvider`:
//! it knows its action catalog, how to read/write its config, and how to turn a
//! raw joystick press into that game's binding token. MW5 is the first provider;
//! Star Citizen and MSFS 2024 are registered as "coming soon" stubs.

pub mod ac7;
pub mod mw5;
pub mod sc;

use crate::input::Device;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Axis,
    Button,
}

/// One bindable action in a game, with a human-friendly label.
#[derive(Clone, Debug)]
pub struct Action {
    pub id: String,       // engine id, e.g. "JoystickLookVertical" / "FireWeaponGroup1"
    pub label: String,    // friendly, e.g. "Aim Up/Down"
    pub category: String, // for grouping in the UI, e.g. "Aiming", "Weapons"
    pub kind: Kind,
}

/// A current/edited binding row: an action plus the assigned token and (axes) scale.
#[derive(Clone, Debug)]
pub struct Binding {
    pub id: String,
    pub token: String, // "" = unbound
    pub scale: f32,    // axes only; sign = direction, magnitude = sensitivity
}

#[derive(Clone, Debug, Default)]
pub struct SaveReport {
    pub backup: String,
    pub changed: Vec<String>,
    pub missing: Vec<String>,
}

/// What MW5 calls a device's "role". Other games may ignore this.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Joystick,
    Throttle,
    Ignored,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Joystick => "Joystick",
            Role::Throttle => "Throttle",
            Role::Ignored => "ignored",
        }
    }
}

pub trait GameProvider {
    fn name(&self) -> &str;
    fn available(&self) -> bool;
    fn config_path(&self) -> std::path::PathBuf;

    fn actions(&self) -> Vec<Action>;
    fn load(&self) -> Result<Vec<Binding>, String>;
    fn save(&self, bindings: &[Binding]) -> Result<SaveReport, String>;
    /// A sensible known-good starting layout (used to fill unbound actions).
    fn default_bindings(&self) -> Vec<Binding> { Vec::new() }

    // --- press-to-bind translation (raw device input -> this game's token) ---
    fn role_of(&self, dev: &Device, enum_index: usize) -> Role;
    fn button_token(&self, dev: &Device, button_1based: u32, idx: usize) -> Option<String>;
    fn axis_token(&self, dev: &Device, axis_index: usize, idx: usize) -> Option<String>;
    fn pov_token(&self, dev: &Device, octant: u32, idx: usize) -> Option<String>;

    // --- dashboard extras (defaults: not supported) ---
    fn launch_uri(&self) -> Option<String> { None }
    /// VID strings (e.g. "3344") of sticks that conflict and may be hidden.
    fn conflict_vids(&self) -> Vec<String> { Vec::new() }
    /// Process names that lock the config while running.
    fn running_processes(&self) -> Vec<String> { Vec::new() }
}

/// The registry of known games. Index 0 is the default selection.
pub fn all() -> Vec<Box<dyn GameProvider>> {
    vec![
        Box::new(mw5::Mw5::new()),
        Box::new(ac7::Ac7::new()),
        Box::new(sc::Sc::new()),
    ]
}
