// MW5 Remap — visual joystick binding editor (egui). Hides the console in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cli;
mod devices;
mod diagram;
mod export;
mod games;
mod hidhide;
mod input;
mod sys;
mod update;
mod visual;

use eframe::egui;

fn main() -> eframe::Result<()> {
    update::cleanup(); // remove leftover .old.exe from a previous self-update
    if std::env::args().any(|a| a == "--selftest") {
        cli::selftest();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--devices") {
        cli::dump_devices();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--monitor") {
        cli::monitor();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--apply-defaults") {
        cli::apply_defaults();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--force-defaults") {
        cli::force_defaults();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--write-hotas") {
        cli::write_hotas();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--ac7-setup") {
        cli::ac7_setup();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--sc-test") {
        cli::sc_test();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--lock" || a == "--unlock") {
        let lock = std::env::args().any(|a| a == "--lock");
        match games::mw5::set_config_locked(lock) {
            Ok(()) => println!("GameUserSettings.ini is now {}.", if lock { "LOCKED (read-only) — MW5 can't reset your bindings" } else { "unlocked" }),
            Err(e) => println!("FAILED: {e}"),
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--diagram") {
        cli::make_diagram();
        return Ok(());
    }
    if std::env::args().any(|a| a == "--testhttp") {
        match update::debug_tag("cli", "cli") {
            Some(tag) => println!("WinHTTP OK — cli/cli latest release tag: {tag}"),
            None => println!("WinHTTP FAILED — no response/parse"),
        }
        println!("current version: {}", update::current_version());
        match update::latest() {
            Some((ver, url)) => {
                println!("own repo latest: v{ver}");
                println!("  update asset:  {url}");
                println!("  newer than current? {}", update::is_newer(&ver));
            }
            None => println!("own repo latest: none / OWNER unset"),
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--imgcheck") {
        for (n, b) in [("ab6_base", include_bytes!("../assets/ab6_base.png").as_slice()),
                       ("mhg_stick", include_bytes!("../assets/mhg_stick.png").as_slice()),
                       ("mrp_pedals", include_bytes!("../assets/mrp_pedals.jpg").as_slice())] {
            match image::load_from_memory(b) {
                Ok(img) => println!("{n}: OK {}x{} ({} bytes embedded)", img.width(), img.height(), b.len()),
                Err(e) => println!("{n}: DECODE FAILED: {e}"),
            }
        }
        return Ok(());
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1500.0, 940.0])
        .with_min_inner_size([1000.0, 640.0])
        .with_title("MW5 Remap — joystick binding editor");
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "MW5 Remap",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

/// The window/taskbar icon, decoded from the embedded MiniMax logo (None on failure).
fn load_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/logo.png")).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width, height })
}
