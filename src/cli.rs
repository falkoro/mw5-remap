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
    println!("(axis array = [X Y Z Rx Ry Rz S0 S1] via DirectInput, 0..65535, centre ~32767; pov in centi-deg, 65535=centred)\n");
    // remember last-printed state per device id to only log changes
    let mut last: std::collections::HashMap<u32, (u32, u32, [u32; 8])> = std::collections::HashMap::new();
    let start = SystemTime::now();
    let axis_names = ["X", "Y", "Z", "Rx", "Ry", "Rz", "S0", "S1"];
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
                for i in 0..8 {
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

/// Verify the vJoy feeder FFI: sweep vJoy device-1 axis X centre -> forward -> reverse.
/// Watch it in vJoyMonitor (or re-run --devices) to confirm the axis moves.
pub fn vjoytest() {
    use std::{thread::sleep, time::Duration};
    println!("== vJoy feeder test ==");
    if !crate::vjoy::available() {
        println!("vJoy NOT available — is the driver installed & enabled? (vJoyInterface.dll not found / vJoyEnabled()=false)");
        return;
    }
    let st = crate::vjoy::status();
    let label = match st { 0 => "OWN (we hold it)", 1 => "FREE (acquirable)", 2 => "BUSY (another app owns it)", 3 => "MISS (device 1 not configured)", _ => "UNKNOWN" };
    println!("vJoy driver enabled ✓   device-1 status = {st} ({label})");
    // sanity: rest=centre, right toe full=forward, left toe full=reverse
    println!("combine sanity: rest={}, right={}, left={}",
        crate::vjoy::combine_toes(0, 0), crate::vjoy::combine_toes(65535, 0), crate::vjoy::combine_toes(0, 65535));
    println!("sweeping device 1, axis X:");
    for v in [crate::vjoy::VJOY_CENTRE, crate::vjoy::VJOY_MAX, 1, crate::vjoy::VJOY_CENTRE] {
        let ok = crate::vjoy::feed_throttle(v);
        println!("   SetAxis X = {v:5}  -> {}", if ok { "ok" } else { "FAILED (device 1 busy / not acquirable)" });
        sleep(Duration::from_millis(500));
    }
    println!("Done. Re-run --devices (or open vJoyMonitor) to confirm vJoy X moved.");
}

/// Self-contained vJoy round-trip: FEED a known button+axis pattern to vJoy device 1,
/// then READ it back through our own DirectInput layer. Proves the feeder works AND that
/// our button reading works — WITHOUT MechWarrior or any physical input. If the fed
/// buttons read back, the in-game failure is downstream (MW5 config / device visibility);
/// if they DON'T, the bug is in our feeder or DI button reading.
pub fn vjoy_verify() {
    use std::{thread::sleep, time::Duration};
    println!("== vJoy round-trip verify (feed -> DirectInput read-back) ==");
    if !crate::vjoy::available() {
        println!("vJoy NOT available — install + enable vJoy, configure device 1 (>=20 buttons, axes X Y Z Rx Ry Rz).");
        return;
    }
    let want: [u8; 5] = [1, 4, 7, 12, 18];
    println!("status(1) before feed = {}", crate::vjoy::status());
    let bres: Vec<bool> = want.iter().map(|&b| crate::vjoy::feed_button(b, true)).collect();
    let ax = crate::vjoy::feed(crate::vjoy::HID_X, 28000);
    let az = crate::vjoy::feed(crate::vjoy::HID_Z, 6000);
    println!("feed_button results = {bres:?}   feed X = {ax}  feed Z = {az}   status now = {}", crate::vjoy::status());
    // Read back a few times, RE-FEEDING each round (vJoy state should hold, but make sure).
    let mut ok = false;
    for round in 0..4 {
        for &b in &want { crate::vjoy::feed_button(b, true); }
        crate::vjoy::feed(crate::vjoy::HID_X, 28000);
        crate::vjoy::feed(crate::vjoy::HID_Z, 6000);
        sleep(Duration::from_millis(120));
        let devs = crate::input::poll();
        for (i, d) in devs.iter().filter(|d| (d.vid, d.pid) == (0x1234, 0xBEAD)).enumerate() {
            let pressed: Vec<u8> = (0..32u8).filter(|&b| d.buttons & (1u32 << b) != 0).map(|b| b + 1).collect();
            println!("  r{round} dev{i}: buttons={pressed:?}  X={} Z={}  ({} btns/{} axes)",
                d.axes[0], d.axes[2], d.num_buttons, d.num_axes);
            if want.iter().all(|w| pressed.contains(w)) { ok = true; }
        }
        if ok { break; }
    }
    for &b in &want { crate::vjoy::feed_button(b, false); }
    if ok {
        println!("✓ ROUND-TRIP OK — the feeder AND our button reading both work. Any in-game failure is");
        println!("  downstream: MW5 must see ONLY vJoy (HidHide the MOZA, whitelist this app), the .Remap");
        println!("  vJoy block must be saved (toggle vJoy mode ON, then Save), control mode = Classic/Mech.");
    } else {
        println!("✗ fed buttons did NOT read back. Either DI button reading is broken or rid(1) isn't the");
        println!("  enumerated device. This is the real bug to fix before MW5 can ever work.");
    }
}

/// Generate a clean, flat MechWarrior-style targeting-reticle logo to assets/logo.png —
/// drawn programmatically with analytic anti-aliasing (no external image tools). The app
/// embeds assets/logo.png at build time, so run this once, then rebuild.
pub fn genlogo() {
    use image::{Rgba, RgbaImage};
    let n: u32 = 512;
    let nf = n as f32;
    let c = nf / 2.0;
    let mut img = RgbaImage::new(n, n);

    let smooth = |e0: f32, e1: f32, x: f32| { let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) };
    let band = |v: f32, half: f32| 1.0 - smooth(half - 1.0, half + 1.0, v); // soft 1px-AA bar/ring edge
    let range = |v: f32, lo: f32, hi: f32| smooth(lo - 1.0, lo + 1.0, v) * (1.0 - smooth(hi - 1.0, hi + 1.0, v));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;

    let green = (61.0_f32, 217.0, 138.0);
    let amber = (245.0_f32, 168.0, 64.0);

    for y in 0..n {
        for x in 0..n {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut col = [0.0f32, 0.0, 0.0, 0.0]; // straight RGBA, alpha 0..1
            let mut over = |c3: (f32, f32, f32), cov: f32| {
                if cov <= 0.0 { return; }
                col[0] = lerp(col[0], c3.0, cov);
                col[1] = lerp(col[1], c3.1, cov);
                col[2] = lerp(col[2], c3.2, cov);
                col[3] = col[3].max(cov);
            };

            // rounded-rect tile (SDF) with a vertical navy gradient
            let half = nf / 2.0 - 22.0;
            let radius = 110.0;
            let qx = (px - c).abs() - half + radius;
            let qy = (py - c).abs() - half + radius;
            let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - radius;
            let tile = 1.0 - smooth(-1.0, 1.0, d);
            let t = py / nf;
            over((lerp(40.0, 20.0, t), lerp(48.0, 25.0, t), lerp(71.0, 38.0, t)), tile);

            // reticle
            let dc = ((px - c).powi(2) + (py - c).powi(2)).sqrt();
            let ax = (px - c).abs();
            let ay = (py - c).abs();
            let gap = band(ax, 17.0).max(band(ay, 17.0)); // break the ring where the crosshair crosses
            let ring = band((dc - 160.0).abs(), 7.0) * (1.0 - gap);
            let vbar = band(ax, 6.5) * range(ay, 40.0, 192.0);
            let hbar = band(ay, 6.5) * range(ax, 40.0, 192.0);
            over(green, ring.max(vbar).max(hbar));
            // inner diamond brackets (HUD detail) at the diagonals, r~108
            let diag = ((ax - ay).abs() < 9.0) as i32 as f32 * range(dc, 96.0, 120.0);
            over(green, diag * 0.8);
            // centre pip (amber accent)
            over(amber, band(dc, 17.0));

            img.put_pixel(x, y, Rgba([col[0] as u8, col[1] as u8, col[2] as u8, (col[3] * 255.0) as u8]));
        }
    }
    img.save("assets/logo.png").expect("write assets/logo.png");
    println!("wrote assets/logo.png ({n}x{n}) — rebuild to embed it.");
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
        println!("    axes X{} Y{} Z{} Rx{} Ry{} Rz{} S0{} S1{}  pov={}",
            d.axes[0], d.axes[1], d.axes[2], d.axes[3], d.axes[4], d.axes[5], d.axes[6], d.axes[7], d.pov);
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

    // mapping consistency: every default-bound token must be PRODUCIBLE by a device's
    // .Remap block — else it's an orphan ("bound in-game but nothing feeds it", the
    // exact class of bug behind the dead buttons / dead throttle).
    println!("\n== Mapping consistency (default layout) ==");
    let producible = games::mw5::producible_tokens();
    let orphans: Vec<String> = mw5.default_bindings().into_iter()
        .filter(|b| !b.token.is_empty() && !producible.contains(&b.token))
        .map(|b| format!("{} -> {}", b.id, b.token))
        .collect();
    if orphans.is_empty() {
        println!("every default binding maps to a producible token ✓");
    } else {
        println!("ORPHAN bindings (no device .Remap produces the token):");
        for o in &orphans { println!("   {o}"); }
    }
    let no_orphans = orphans.is_empty();

    let pass = round_trip && same_lines && one_map && kb_intact && sections_ok && no_orphans;
    println!("\nROUND-TRIP: {}", if round_trip { "PASS" } else { "FAIL" });
    println!("MAPPING:    {}", if no_orphans { "PASS" } else { "FAIL" });
    println!("OVERALL:    {}", if pass { "PASS" } else { "FAIL" });
    let _ = std::fs::remove_file(&tmp);
}
