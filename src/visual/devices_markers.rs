//! Device-image callout marker tables (normalized 0..1 positions + the MW5 token each
//! control emits). Split out of `mod.rs` to keep that file within the ~250-line budget.
//! Positions are rough defaults — fine-tune them live with the ✥ Edit layout drag, which
//! persists per-device to `marker_layout.txt`.

use super::{m, mm, Marker, MultiMarker};

// MHG grip: physical reference labels. Button numbers differ per firmware, so we
// don't guess them here — use the live board below to map a button to its number.
// The POV hat does light up (we can read the hat octant directly).
pub(super) const MHG_MARKERS: &[Marker] = &[
    // Analog thumb / POV hat = two axes: winmm U(4) = vertical, V(5) = horizontal.
    // Two markers so BOTH directions light when you sweep it.
    m(0.645, 0.215, "", "Thumb/POV hat ↕ (look)", "Joystick_Axis4"),
    m(0.672, 0.232, "", "Thumb/POV hat ↔ (look)", "Joystick_Axis5"),
    // Buttons show the bound action ("Trigger · Fire Weapon Group 1"). The button
    // NUMBER per physical control is firmware-dependent, so these follow the app's
    // default layout (Button1..6 = fire groups) — press one to confirm via the live
    // green light, and rebind in the list if a number is off.
    m(0.46, 0.45, "", "Trigger", "Joystick_Button1"),
    m(0.41, 0.21, "", "Red button", "Joystick_Button2"),
    m(0.55, 0.335, "", "Thumb button", "Joystick_Button4"),
    m(0.45, 0.345, "", "Rocker switch", "Joystick_Button5"),
    m(0.37, 0.49, "", "Pinky flip", "Joystick_Button6"),
];

// MULTI-INPUT markers: ONE physical control that carries several inputs, shown as a
// single stacked callout (one box, every direction listed) that glows whichever
// direction is live — instead of N separate dots crowding the same spot.
//
// MOZA MHG coolie/POV hat: a single 8-way hat emitting Joystick_Hat_1..8. The diagram's
// spoke ring (MHG_HATS) shows WHICH way is pressed; this marker lists WHAT each way is
// bound to and lights that row live. Drag the whole group by its dot (keyed "Coolie hat").
pub(super) const MHG_HAT_INPUTS: &[(&str, &str)] = &[
    ("↑  N", "Joystick_Hat_1"),
    ("↗  NE", "Joystick_Hat_2"),
    ("→  E", "Joystick_Hat_3"),
    ("↘  SE", "Joystick_Hat_4"),
    ("↓  S", "Joystick_Hat_5"),
    ("↙  SW", "Joystick_Hat_6"),
    ("←  W", "Joystick_Hat_7"),
    ("↖  NW", "Joystick_Hat_8"),
];
pub(super) const MHG_MULTI: &[MultiMarker] = &[mm(0.50, 0.27, "Coolie hat", MHG_HAT_INPUTS)];

// AB6 gimbal -> the two aim axes. Numbers = the Joystick_Axis index (= the token).
pub(super) const BASE_MARKERS: &[Marker] = &[
    m(0.46, 0.30, "1", "Pitch ↕", "Joystick_Axis1"),
    m(0.55, 0.40, "2", "Roll ↔", "Joystick_Axis2"),
    m(0.50, 0.72, "", "FFB gimbal — \"Joystick\"", ""),
];

// MRP pedals -> Throttle axes. Number = the Throttle_Axis index (= the token).
pub(super) const PEDAL_MARKERS: &[Marker] = &[
    m(0.50, 0.78, "1", "Rudder (turn legs)", "Throttle_Axis1"),
    m(0.66, 0.40, "2", "Right toe → forward", "Throttle_Axis2"),
    m(0.34, 0.40, "2", "Left toe → reverse", "Throttle_Axis2"),
];

// Main POV hat = 8-way (confirmed: MW5 Joystick_Hat_1..8, MOZA hat configurable
// 8/4-way in MOZA Cockpit). Thumb control = a 5-way switch (4 dirs + center push).
pub(super) const MHG_HATS: &[(f32, f32, u8)] = &[(0.50, 0.27, 8), (0.585, 0.205, 5)];

// VKB Gladiator NXT EVO. Image is the right-hand product render (mirror of the user's
// left-hand stick) — controls are functionally identical. As a GENERIC joystick the
// provider emits Joystick_Axis{slot+1}: X(0)->Axis1 roll, Y(1)->Axis2 pitch, Rz(5)->
// Axis6 twist. Buttons/hat numbering is firmware-dependent — these are rough defaults;
// drag them onto the real controls with ✥ Edit layout and rebind in the list.
pub(super) const VKB_MARKERS: &[Marker] = &[
    // Gimbal aim axes (X/Y) at the stick pivot.
    m(0.50, 0.60, "1", "Roll ↔ (X)", "Joystick_Axis1"),
    m(0.50, 0.66, "2", "Pitch ↕ (Y)", "Joystick_Axis2"),
    // Lockable stick twist (Rz) -> yaw / leg-turn.
    m(0.44, 0.50, "6", "Twist → yaw (Rz)", "Joystick_Axis6"),
    // 8-way top hat — bare "Joystick_Hat" lights on ANY octant (the spokes show which).
    m(0.40, 0.14, "", "Top hat (8-way)", "Joystick_Hat"),
    // Analog mini-stick on the grip head (axes are firmware-dependent: reference only).
    m(0.47, 0.11, "", "Analog mini-stick", ""),
    // Trigger + main grip buttons.
    m(0.30, 0.28, "", "Trigger", "Joystick_Button1"),
    m(0.37, 0.21, "", "Red button", "Joystick_Button2"),
    m(0.45, 0.35, "", "Thumb wheel/encoder", "Joystick_Button3"),
    // Base buttons + rotary switches.
    m(0.67, 0.68, "", "Base buttons (F1/F2/F3)", "Joystick_Button4"),
    m(0.69, 0.80, "", "Base rotaries (Sw)", "Joystick_Button7"),
];

// VKB top hat = true 8-way castle hat (diagonals fire).
pub(super) const VKB_HATS: &[(f32, f32, u8)] = &[(0.40, 0.14, 8)];
