// CLI/headless helper commands. These are the non-GUI entry points dispatched
// from main() based on argv flags. Pure code-move out of main.rs — no logic changes.
use crate::games::{self, GameProvider};
use crate::{diagram, input};

/// Live input monitor: polls winmm for ~25s and prints whenever a button, the POV
/// hat, or an axis changes — so we can see exactly how a control (e.g. the MHG hat)
/// shows up (button vs POV vs analog axis). Run it, then wiggle the control.
pub fn monitor() {
    use std::time::{Duration, SystemTime};
    println!("== input monitor: move your controls now (25s) ==");
    println!("(axis array = [X Y Z R U V], 0..65535, centre ~32767; pov in centi-deg, 65535=centred)\n");
    // remember last-printed state per device id to only log changes
    let mut last: std::collections::HashMap<u32, (u32, u32, [u32; 6])> = std::collections::HashMap::new();
    let start = SystemTime::now();
    let axis_names = ["X", "Y", "Z", "R", "U", "V"];
    while start.elapsed().map(|d| d.as_secs()).unwrap_or(99) < 25 {
        for d in input::poll() {
            let prev = last.get(&d.id).copied();
            let mut msgs: Vec<String> = Vec::new();
            // buttons
            let pressed = d.buttons;
            if prev.map(|p| p.0) != Some(pressed) {
                let list: Vec<u32> = d.pressed_buttons();
                msgs.push(format!("buttons={list:?}"));
            }
            // pov
            if prev.map(|p| p.1) != Some(d.pov) {
                let oct = d.pov_octant().map(|o| o.to_string()).unwrap_or_else(|| "-".into());
                msgs.push(format!("POV={} (octant {})", d.pov, oct));
            }
            // axes that moved > ~5% from their previous reading
            if let Some(p) = prev {
                for i in 0..6 {
                    if (d.axes[i] as i64 - p.2[i] as i64).abs() > 3000 {
                        msgs.push(format!("axis {}={}", axis_names[i], d.axes[i]));
                    }
                }
            }
            if !msgs.is_empty() {
                println!("[{} {:04X}:{:04X}] {}", d.name, d.vid, d.pid, msgs.join("  "));
            }
            last.insert(d.id, (pressed, d.pov, d.axes));
        }
        std::thread::sleep(Duration::from_millis(60));
    }
    println!("\n== monitor done ==");
}

/// Fill every UNBOUND action with the known-good default layout, then save.
/// Non-destructive: anything already bound (e.g. your fire groups) is left alone.
pub fn apply_defaults() {
    let p = games::mw5::Mw5::new();
    let mut rows = match p.load() { Ok(r) => r, Err(e) => { println!("LOAD FAILED: {e}"); return; } };
    let defaults: std::collections::HashMap<String, games::Binding> =
        p.default_bindings().into_iter().map(|b| (b.id.clone(), b)).collect();
    let mut filled = Vec::new();
    let mut kept = Vec::new();
    for r in rows.iter_mut() {
        if r.token.is_empty() {
            if let Some(d) = defaults.get(&r.id) {
                r.token = d.token.clone();
                r.scale = d.scale;
                filled.push(format!("{} -> {}", r.id, d.token));
            }
        } else {
            kept.push(format!("{} = {}", r.id, r.token));
        }
    }
    match p.save(&rows) {
        Ok(rep) => {
            println!("Saved. Backup: {}", rep.backup);
            println!("\nAlready bound (kept as-is) — {}:", kept.len());
            for k in &kept { println!("   {k}"); }
            println!("\nNewly filled from defaults — {}:", filled.len());
            for f in &filled { println!("   {f}"); }
        }
        Err(e) => println!("SAVE FAILED: {e}"),
    }
}

/// Overwrite EVERY catalog action with the known-good default layout, then save.
/// Use this to repair a config that has bindings pointing at controls the real
/// hardware doesn't have (e.g. throttle buttons on pedals that have none).
pub fn force_defaults() {
    let p = games::mw5::Mw5::new();
    let mut rows = match p.load() { Ok(r) => r, Err(e) => { println!("LOAD FAILED: {e}"); return; } };
    let defaults: std::collections::HashMap<String, games::Binding> =
        p.default_bindings().into_iter().map(|b| (b.id.clone(), b)).collect();
    let mut changed = Vec::new();
    for r in rows.iter_mut() {
        if let Some(d) = defaults.get(&r.id) {
            if r.token != d.token || (r.scale - d.scale).abs() > 0.001 {
                changed.push(format!("{}: {} -> {} (x{:.1})",
                    r.id, if r.token.is_empty() { "(unbound)" } else { &r.token }, d.token, d.scale));
            }
            r.token = d.token.clone();
            r.scale = d.scale;
        }
    }
    match p.save(&rows) {
        Ok(rep) => {
            println!("Saved. Backup: {}", rep.backup);
            println!("\nChanged {} binding(s):", changed.len());
            for c in &changed { println!("   {c}"); }
            if changed.is_empty() { println!("   (config already matches the known-good defaults)"); }
        }
        Err(e) => println!("SAVE FAILED: {e}"),
    }
}

/// Write/refresh the MOZA blocks in HOTASMappings.Remap — the file MW5 actually
/// reads for joystick input (maps physical device -> token). Without this, the
/// GameUserSettings token bindings are inert in-game.
pub fn write_hotas() {
    match games::mw5::write_hotas_mappings() {
        Ok(backup) => {
            println!("Wrote HOTAS mappings: {}", games::mw5::hotas_path().display());
            println!("Backup: {}", backup);
            println!("\nMOZA AB6 base -> Joystick (aim + Button1..20 + Hat_1..8)");
            println!("MOZA MRP pedals -> Throttle (rudder = leg turn)");
        }
        Err(e) => println!("WRITE FAILED: {e}"),
    }
}

/// Round-trip test of the Star Citizen actionmaps.xml writer on a temp file.
pub fn sc_test() {
    let tmp = std::env::temp_dir().join("sc_test_actionmaps.xml");
    let _ = std::fs::remove_file(&tmp);
    std::env::set_var("SC_CONFIG", &tmp);
    let p = games::sc::Sc::new();
    let mk = |id: &str, tok: &str| games::Binding { id: id.into(), token: tok.into(), scale: 1.0 };
    // Two pretend VKB sticks (231D:0200 / 231D:0201).
    let rows = vec![
        mk("v_pitch", "231D0200|y"), mk("v_yaw", "231D0200|rotz"), mk("v_roll", "231D0200|x"),
        mk("v_attack1_group1", "231D0200|button1"),
        mk("v_throttle_abs", "231D0201|z"), mk("v_target_nearest_hostile", "231D0201|button2"),
    ];
    match p.save(&rows) {
        Ok(rep) => {
            println!("saved {} binding(s)\n----- actionmaps.xml -----", rep.changed.len());
            println!("{}", std::fs::read_to_string(&tmp).unwrap_or_default());
            println!("----- reload round-trip -----");
            for b in p.load().unwrap_or_default().iter().filter(|b| !b.token.is_empty()) {
                println!("  {:<26} = {}", b.id, b.token);
            }
        }
        Err(e) => println!("SC TEST FAILED: {e}"),
    }
    let _ = std::fs::remove_file(&tmp);
}

/// Apply the default Ace Combat 7 HOTAS layout and write Config/Input.ini.
pub fn ac7_setup() {
    let p = games::ac7::Ac7::new();
    let rows = p.default_bindings();
    match p.save(&rows) {
        Ok(rep) => {
            println!("Wrote AC7 Input.ini: {}", p.config_path().display());
            println!("Backup: {}", rep.backup);
            println!("\nBound {} actions across your HOTAS. Reminder: DISABLE Steam Input for AC7.", rep.changed.len());
            for c in &rep.changed { println!("   {c}"); }
        }
        Err(e) => println!("AC7 SETUP FAILED: {e}"),
    }
}

/// Write the HTML control-map infographic next to the exe and report its path.
pub fn make_diagram() {
    let p = games::mw5::Mw5::new();
    let actions = p.actions();
    let rows = match p.load() { Ok(r) => r, Err(e) => { println!("LOAD FAILED: {e}"); return; } };
    let html = diagram::render(&actions, &rows);
    let out = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("MW5-Controls.html")))
        .unwrap_or_else(|| std::path::PathBuf::from("MW5-Controls.html"));
    match std::fs::write(&out, html) {
        Ok(_) => println!("Wrote infographic: {}", out.display()),
        Err(e) => println!("WRITE FAILED: {e}"),
    }
}

/// Headless dump of live joysticks: role + axes + any pressed control as the exact
/// MW5 token the press-to-bind grid would capture. Proves the winmm layer works.
pub fn dump_devices() {
    let mw5 = games::mw5::Mw5::new();
    let devs = input::poll();
    println!("== Live joysticks ({}) ==", devs.len());
    for (idx, d) in devs.iter().enumerate() {
        let role = mw5.role_of(d, idx);
        println!("#{} [{}] {}  VID_{:04X}&PID_{:04X}  {} axes {} btns  has_pov={}",
            d.id, role.label(), d.name, d.vid, d.pid, d.num_axes, d.num_buttons, d.has_pov);
        println!("    axes X{} Y{} Z{} R{} U{} V{}  pov={}",
            d.axes[0], d.axes[1], d.axes[2], d.axes[3], d.axes[4], d.axes[5], d.pov);
        for b in d.pressed_buttons() {
            if let Some(t) = mw5.button_token(d, b, idx) { println!("    pressed button {} -> token {}", b, t); }
        }
        if let Some(oct) = d.pov_octant() {
            if let Some(t) = mw5.pov_token(d, oct, idx) { println!("    pov octant {} -> token {}", oct, t); }
        }
    }
    if devs.is_empty() { println!("(no joysticks detected by winmm)"); }
}

/// Headless verification of the MW5 config round-trip. Operates on a TEMP copy of
/// the real config (via the MW5_CONFIG override) so the real file is never touched.
pub fn selftest() {
    let mw5 = games::mw5::Mw5::new();

    println!("== Read current bindings (real config, read-only) ==");
    let real = mw5.config_path();
    println!("config: {}", real.display());
    match mw5.load() {
        Ok(rows) => {
            for r in &rows {
                println!("   {:<32} {:<22} x{:.1}", r.id, if r.token.is_empty() { "(unbound)" } else { &r.token }, r.scale);
            }
            println!("   {} actions in catalog", rows.len());
        }
        Err(e) => { println!("LOAD FAILED: {e}"); return; }
    }

    println!("\n== Round-trip write to a TEMP copy ==");
    let tmp = std::env::temp_dir().join("mw5_selftest_GUS.ini");
    if std::fs::copy(&real, &tmp).is_err() {
        println!("(no real config to copy — skipping write test)");
        return;
    }
    std::env::set_var("MW5_CONFIG", &tmp);
    let p = games::mw5::Mw5::new();
    let mut rows = p.load().expect("reload temp");
    // mutate: assign an axis (proves Key=None -> Joystick_Axis1 + negative scale) and
    // retarget a button, to prove both write paths apply.
    let mut touched: Vec<String> = Vec::new();
    for r in rows.iter_mut() {
        if r.id == "JoystickLookVertical" { r.token = "Joystick_Axis1".into(); r.scale = -2.5; touched.push("JoystickLookVertical -> Joystick_Axis1 x-2.5".into()); break; }
    }
    for r in rows.iter_mut() {
        if r.id == "FireWeaponGroup1" { r.token = "Joystick_Button9".into(); touched.push("FireWeaponGroup1 -> Joystick_Button9".into()); break; }
    }
    match p.save(&rows) {
        Ok(rep) => println!("saved: backup {} | {} change(s) | {} missing", rep.backup, rep.changed.len(), rep.missing.len()),
        Err(e) => { println!("SAVE FAILED: {e}"); return; }
    }
    // reload and verify the mutations stuck
    let after = p.load().expect("reload after save");
    let fw1 = after.iter().find(|r| r.id == "FireWeaponGroup1").map(|r| r.token.clone()).unwrap_or_default();
    let axis = after.iter().find(|r| r.id == "JoystickLookVertical").cloned().unwrap_or_else(|| games::Binding { id: String::new(), token: String::new(), scale: 0.0 });
    println!("changes intended: {touched:?}");
    println!("after reload: FireWeaponGroup1 = {fw1}   JoystickLookVertical = {} x{:.1}", axis.token, axis.scale);
    let round_trip = fw1 == "Joystick_Button9" && axis.token == "Joystick_Axis1" && axis.scale < 0.0;

    // structural integrity: keyboard/gamepad sections must be untouched.
    println!("\n== Structural integrity ==");
    let orig = std::fs::read_to_string(&real).unwrap_or_default();
    let now = std::fs::read_to_string(&tmp).unwrap_or_default();
    let same_lines = orig.lines().count() == now.lines().count();
    let one_map = now.lines().filter(|l| l.starts_with("InputTypeToActionKeyMap=")).count() == 1;
    let kb_intact = now.contains("(ActionName=\"FireWeaponGroup1\",BoundedKeys=((Key=One)");
    let sections_ok = now.contains("Keyboard_Mouse, (ActionKeyMaps=") && now.contains("GamePad, (ActionKeyMaps=");
    println!("line count unchanged: {same_lines}");
    println!("exactly one ActionKeyMap line: {one_map}");
    println!("keyboard FireWeaponGroup1 still One/LeftMouse: {kb_intact}");
    println!("Keyboard_Mouse + GamePad sections present: {sections_ok}");

    let pass = round_trip && same_lines && one_map && kb_intact && sections_ok;
    println!("\nROUND-TRIP: {}", if round_trip { "PASS" } else { "FAIL" });
    println!("OVERALL:    {}", if pass { "PASS" } else { "FAIL" });
    let _ = std::fs::remove_file(&tmp);
}
