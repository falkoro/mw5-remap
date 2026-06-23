//! Static MW5 data: the curated action catalog and the known-good default layout.

use super::{Action, Binding, Kind};

/// (engine id, friendly label, category, kind). The curated, important actions.
pub(super) fn catalog() -> Vec<Action> {
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

pub(super) fn default_bindings() -> Vec<Binding> {
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
