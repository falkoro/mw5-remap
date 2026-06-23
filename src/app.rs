//! The egui application: a press-to-bind grid editor over a pluggable game
//! provider. Each frame we poll joysticks; if a capture is active, the next new
//! button/axis becomes the binding. Devices/HidHide/launch live in the side modules.

use crate::games::{self, Action, Binding, GameProvider, Kind};
use crate::{hidhide, input, sys};
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
struct Capture {
    row: usize,
    kind: Kind,
    ignore: HashSet<String>,         // controls already held when capture began
    baseline: HashMap<u32, [u32; 6]>, // axis rest values per device id
}

pub struct App {
    games: Vec<Box<dyn GameProvider>>,
    selected: usize,
    actions: Vec<Action>,
    rows: Vec<Binding>,
    devices: Vec<input::Device>,
    capture: Option<Capture>,
    status: String,
    elevated: bool,
    hidden: Vec<String>,
    hide_state: PathBuf,
    textures: Option<crate::visual::Textures>,
    show_labels: bool,
    update: Arc<Mutex<Option<(String, String)>>>, // (version, exe_url) when an update is found
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let games = games::all();
        let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let hide_state = PathBuf::from(base).join("MW5-Remap").join("hidden-devices.txt");

        // background update check (GitHub Releases) — never blocks the UI
        let update = Arc::new(Mutex::new(None));
        {
            let u = update.clone();
            std::thread::spawn(move || {
                if let Some((ver, url)) = crate::update::latest() {
                    if crate::update::is_newer(&ver) {
                        *u.lock().unwrap() = Some((ver, url));
                    }
                }
            });
        }
        let mut app = App {
            games,
            selected: 0,
            actions: Vec::new(),
            rows: Vec::new(),
            devices: Vec::new(),
            capture: None,
            status: "Ready.".into(),
            elevated: sys::is_elevated(),
            hidden: Vec::new(),
            hide_state,
            textures: None,
            show_labels: true,
            update,
        };
        app.load_selected();
        app.crash_recover();
        app
    }

    fn load_selected(&mut self) {
        let p = self.games[self.selected].as_ref();
        if !p.available() {
            self.actions.clear();
            self.rows.clear();
            self.status = format!("{} support is coming soon.", p.name());
            return;
        }
        self.actions = p.actions();
        match p.load() {
            Ok(b) => { self.rows = b; self.status = format!("Loaded current bindings from {}.", p.name()); }
            Err(e) => { self.rows.clear(); self.status = e; }
        }
    }

    fn crash_recover(&mut self) {
        if self.elevated && self.hide_state.exists() {
            if let Ok(txt) = std::fs::read_to_string(&self.hide_state) {
                let paths: Vec<String> = txt.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect();
                if !paths.is_empty() {
                    let _ = hidhide::restore(&paths);
                    let _ = std::fs::remove_file(&self.hide_state);
                    self.status = format!("Recovered: freed {} device(s) left hidden by a previous run.", paths.len());
                }
            }
        }
    }

    /// If a capture is active, look for the first NEW control and bind it.
    fn resolve_capture(&mut self) {
        let cap = match self.capture.clone() { Some(c) => c, None => return };
        let p = self.games[self.selected].as_ref();
        let mut found: Option<String> = None;
        for (idx, dev) in self.devices.iter().enumerate() {
            match cap.kind {
                Kind::Button => {
                    for b in dev.pressed_buttons() {
                        if let Some(t) = p.button_token(dev, b, idx) {
                            if !cap.ignore.contains(&t) { found = Some(t); break; }
                        }
                    }
                    if found.is_none() {
                        if let Some(oct) = dev.pov_octant() {
                            if let Some(t) = p.pov_token(dev, oct, idx) {
                                if !cap.ignore.contains(&t) { found = Some(t); }
                            }
                        }
                    }
                }
                Kind::Axis => {
                    let base = cap.baseline.get(&dev.id).copied().unwrap_or(dev.axes);
                    let mut best = (0i64, 0usize);
                    for ax in 0..6 {
                        let d = (dev.axes[ax] as i64 - base[ax] as i64).abs();
                        if d > best.0 { best = (d, ax); }
                    }
                    if best.0 > 12000 {
                        if let Some(t) = p.axis_token(dev, best.1, idx) { found = Some(t); }
                    }
                }
            }
            if found.is_some() { break; }
        }
        if let Some(tok) = found {
            self.rows[cap.row].token = tok.clone();
            self.status = format!("Bound \"{}\" -> {}", self.actions[cap.row].label, tok);
            self.capture = None;
        }
    }
}

fn file_name(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.into())
}

fn persist(path: &PathBuf, paths: &[String]) {
    if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
    let _ = std::fs::write(path, paths.join("\r\n"));
}

const CAPTURING: egui::Color32 = egui::Color32::from_rgb(235, 170, 45); // orange: listening
const LIVE: egui::Color32 = egui::Color32::from_rgb(70, 210, 110);      // green: control active
const STICK_COL: egui::Color32 = egui::Color32::from_rgb(86, 156, 235); // Joystick-role device
const THROTTLE_COL: egui::Color32 = egui::Color32::from_rgb(235, 150, 60); // Throttle-role device
const UNBOUND_COL: egui::Color32 = egui::Color32::from_rgb(120, 128, 145);
const TEXT_MAIN: egui::Color32 = egui::Color32::from_rgb(38, 42, 54);   // dark: readable on the light panel
const LIVE_TXT: egui::Color32 = egui::Color32::from_rgb(20, 140, 72);   // green readable on light
const CAP_TXT: egui::Color32 = egui::Color32::from_rgb(180, 110, 0);    // amber readable on light
// extra colours for games with many physical devices (AC7/SC: one per VID/PID).
const DEV_PALETTE: [egui::Color32; 6] = [
    egui::Color32::from_rgb(86, 156, 235), egui::Color32::from_rgb(235, 150, 60),
    egui::Color32::from_rgb(120, 200, 120), egui::Color32::from_rgb(200, 120, 220),
    egui::Color32::from_rgb(230, 110, 110), egui::Color32::from_rgb(110, 205, 210),
];

/// The colour that identifies which physical device a token belongs to.
fn device_color(token: &str) -> egui::Color32 {
    if token.starts_with("Throttle") {
        THROTTLE_COL
    } else if token.starts_with("Joystick") {
        STICK_COL
    } else if let Some((id, _)) = token.split_once('|') {
        // AC7/SC "VVVVPPPP|input": stable colour per device id.
        let h = id.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
        DEV_PALETTE[(h as usize) % DEV_PALETTE.len()]
    } else {
        UNBOUND_COL
    }
}

/// Friendly control name without the device/role prefix: "Joystick_Button1" ->
/// "Button 1", "Throttle_Axis2" -> "Axis 2", "Joystick_Hat_3" -> "Hat 3", and
/// "044F0402|Y:R" -> "Y:R". The device is shown by colour instead of the prefix.
fn pretty_token(token: &str) -> String {
    if token.is_empty() { return "unbound".into(); }
    let body = token
        .strip_prefix("Joystick_")
        .or_else(|| token.strip_prefix("Throttle_"))
        .map(|s| s.replace('_', " "))
        .or_else(|| token.split_once('|').map(|(_, i)| i.to_string()))
        .unwrap_or_else(|| token.to_string());
    // insert a space before a trailing number run ("Button1" -> "Button 1").
    match body.find(|c: char| c.is_ascii_digit()) {
        Some(p) if p > 0 && body.as_bytes()[p - 1] != b' ' => format!("{} {}", &body[..p], &body[p..]),
        _ => body,
    }
}

/// One row of the Cockpit Bindings grid: action label, a colour-coded "chip"
/// showing the bound control (click it to re-bind), a clear button, and (for axes)
/// invert/scale. The chip turns green the instant its control is physically active.
#[allow(clippy::too_many_arguments)]
fn binding_row(
    ui: &mut egui::Ui,
    i: usize,
    actions: &[Action],
    rows: &mut [Binding],
    capture: &mut Option<Capture>,
    devices: &[input::Device],
    p: &dyn GameProvider,
    status: &mut String,
    hot: &[String],
) {
    let capturing = capture.as_ref().map(|c| c.row == i).unwrap_or(false);
    let token = rows[i].token.clone();
    let live = !token.is_empty() && hot.iter().any(|h| h == &token);

    // action label — green while its control is live, amber while (re)binding
    let lbl_col = if capturing { CAP_TXT } else if live { LIVE_TXT } else { TEXT_MAIN };
    ui.colored_label(lbl_col, &actions[i].label);

    // the chip: a big colour-coded button. Colour = which device; click = re-bind.
    let (text, fill) = if capturing {
        ("press a control…".to_string(), CAPTURING)
    } else if live {
        (pretty_token(&token), LIVE)
    } else if token.is_empty() {
        ("＋ bind".to_string(), UNBOUND_COL)
    } else {
        (pretty_token(&token), device_color(&token))
    };
    let txt_col = if token.is_empty() && !capturing { egui::Color32::from_rgb(120, 128, 145) } else { egui::Color32::from_rgb(15, 18, 24) };
    // a rounded, slightly raised "chip" — nicer than a flat box
    let stroke = if live { egui::Stroke::new(2.0, egui::Color32::from_rgb(30, 120, 60)) }
                 else if token.is_empty() { egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 158, 175)) }
                 else { egui::Stroke::new(1.0, fill.linear_multiply(0.6)) };
    let chip = egui::Button::new(egui::RichText::new(text).color(txt_col).strong().size(14.0))
        .fill(fill)
        .stroke(stroke)
        .rounding(egui::Rounding::same(8.0))
        .min_size(egui::vec2(158.0, 30.0));
    if ui.add(chip).on_hover_text("Click, then press the control / move the axis. Esc cancels.").clicked() {
        if capturing {
            *capture = None;
        } else {
            let mut ignore = HashSet::new();
            for (di, dev) in devices.iter().enumerate() {
                for b in dev.pressed_buttons() { if let Some(t) = p.button_token(dev, b, di) { ignore.insert(t); } }
                if let Some(o) = dev.pov_octant() { if let Some(t) = p.pov_token(dev, o, di) { ignore.insert(t); } }
            }
            let baseline = devices.iter().map(|d| (d.id, d.axes)).collect();
            *capture = Some(Capture { row: i, kind: actions[i].kind, ignore, baseline });
            *status = format!("Listening… do the control for \"{}\" (Esc to cancel)", actions[i].label);
        }
    }

    // clear button (only when bound)
    if !token.is_empty() {
        if ui.small_button("✕").on_hover_text("Clear this binding").clicked() {
            rows[i].token.clear();
        }
    } else {
        ui.label("");
    }

    if actions[i].kind == Kind::Axis {
        ui.horizontal(|ui| {
            let mut inv = rows[i].scale < 0.0;
            if ui.checkbox(&mut inv, "Inv").changed() {
                rows[i].scale = rows[i].scale.abs() * if inv { -1.0 } else { 1.0 };
            }
            let mut mag = rows[i].scale.abs();
            if ui.add(egui::DragValue::new(&mut mag).speed(0.1).range(0.1..=10.0).prefix("x")).changed() {
                let sign = if rows[i].scale < 0.0 { -1.0 } else { 1.0 };
                rows[i].scale = mag * sign;
            }
        });
    } else {
        ui.label("");
    }
    ui.end_row();
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.devices = input::poll();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.capture = None;
            self.status = "Capture cancelled.".into();
        }
        self.resolve_capture();
        if self.textures.is_none() {
            self.textures = crate::visual::load_textures(ctx);
        }

        // group action indices by category (merge same-category, first-seen order, so
        // each category is one section with a unique Grid id — no ID clashes)
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, a) in self.actions.iter().enumerate() {
            if let Some(g) = groups.iter_mut().find(|(c, _)| c == &a.category) {
                g.1.push(i);
            } else {
                groups.push((a.category.clone(), vec![i]));
            }
        }

        let App { games, selected, actions, rows, devices, capture, status, elevated, hidden, hide_state, textures, show_labels, update } = self;
        let mut reload = false;

        // token -> bound action label, so the device diagram can show WHAT is bound
        // to each control (not just the control's name).
        let bound: HashMap<String, String> = rows.iter()
            .filter(|b| !b.token.is_empty())
            .filter_map(|b| actions.iter().find(|a| a.id == b.id).map(|a| (b.token.clone(), a.label.clone())))
            .collect();

        let pending_update = update.lock().unwrap().clone();
        if let Some((ver, url)) = pending_update {
            egui::TopBottomPanel::top("update_banner").show(ctx, |ui| {
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("🔔 Update available: v{ver}  (you have v{})", crate::update::current_version()))
                        .strong().color(egui::Color32::from_rgb(40, 150, 60)));
                    if ui.button("Update now").clicked() {
                        *status = format!("Downloading v{ver}… the app will relaunch.");
                        if let Err(e) = crate::update::apply(&url) { *status = format!("Update failed: {e}"); }
                    }
                    if ui.button("Later").clicked() { *update.lock().unwrap() = None; }
                });
                ui.add_space(3.0);
            });
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Game:");
                let mut want = *selected;
                egui::ComboBox::from_id_salt("game")
                    .selected_text(games[*selected].name().to_string())
                    .show_ui(ui, |ui| {
                        for (i, g) in games.iter().enumerate() {
                            ui.selectable_value(&mut want, i, g.name());
                        }
                    });
                if want != *selected { *selected = want; reload = true; }

                let avail = games[*selected].available();
                ui.separator();
                if ui.add_enabled(avail, egui::Button::new("⟳ Load current")).clicked() { reload = true; }
                if ui.add_enabled(avail, egui::Button::new("💾 Save to game")).clicked() {
                    let p = games[*selected].as_ref();
                    if sys::any_process_running(&p.running_processes()) {
                        *status = "Close MW5 first — it overwrites the config on exit.".into();
                    } else {
                        match p.save(rows) {
                            Ok(r) => *status = format!("Saved ✓  backup {}  ({} change(s){})",
                                file_name(&r.backup), r.changed.len(),
                                if r.missing.is_empty() { String::new() } else { format!(", {} skipped", r.missing.len()) }),
                            Err(e) => *status = format!("Save failed: {}", e),
                        }
                    }
                }
                // MW5 needs a SECOND file (HOTASMappings.Remap) mapping the physical
                // MOZA stick/pedals -> tokens, or the bindings above do nothing in-game.
                // Static (device->token), so this is a run-once button, not per-save.
                if avail && games[*selected].name().contains("MechWarrior")
                    && ui.add(egui::Button::new("🎮 Fix HOTAS file"))
                        .on_hover_text("Write SavedHOTAS\\HOTASMappings.Remap so MW5 reads your MOZA AB6 + pedals. Run once (and after plugging in a new stick).")
                        .clicked()
                {
                    let p = games[*selected].as_ref();
                    if sys::any_process_running(&p.running_processes()) {
                        *status = "Close MW5 first — it overwrites HOTAS mappings on exit.".into();
                    } else {
                        match crate::games::mw5::write_hotas_mappings() {
                            Ok(b) => *status = format!("HOTAS file written ✓  MOZA stick+pedals now mapped  (backup {})", file_name(&b)),
                            Err(e) => *status = format!("HOTAS write failed: {}", e),
                        }
                    }
                }
                // Lock toggle: MW5 rewrites GameUserSettings (resetting joystick
                // bindings to stock) on launch. Read-only stops that. Trade-off:
                // other in-game settings won't save until unlocked.
                if avail && games[*selected].name().contains("MechWarrior") {
                    let locked = crate::games::mw5::config_is_locked();
                    let label = if locked { "🔓 Unlock config" } else { "🔒 Lock config" };
                    let hover = if locked {
                        "Config is LOCKED so MW5 can't reset your bindings. Click to unlock (lets graphics/audio settings save again)."
                    } else {
                        "Make GameUserSettings read-only so MW5 stops resetting your joystick bindings on launch."
                    };
                    if ui.add(egui::Button::new(label)).on_hover_text(hover).clicked() {
                        match crate::games::mw5::set_config_locked(!locked) {
                            Ok(()) => *status = if locked { "Config unlocked — MW5 can save settings again.".into() }
                                                 else { "Config LOCKED ✓  MW5 can no longer reset your bindings.".into() },
                            Err(e) => *status = format!("Lock toggle failed: {}", e),
                        }
                    }
                }
                if ui.add_enabled(avail, egui::Button::new("📊 Export diagram")).clicked() {
                    let html = crate::diagram::render(actions, rows);
                    let out = std::env::current_exe().ok()
                        .and_then(|e| e.parent().map(|d| d.join("MW5-Controls.html")))
                        .unwrap_or_else(|| std::path::PathBuf::from("MW5-Controls.html"));
                    match std::fs::write(&out, html) {
                        Ok(_) => { sys::open_uri(&out.to_string_lossy()); *status = format!("Exported + opened: {}", out.display()); }
                        Err(e) => *status = format!("Export failed: {}", e),
                    }
                }
                ui.separator();
                let p = games[*selected].as_ref();
                if ui.add_enabled(*elevated, egui::Button::new("🛡 Hide conflicts")).clicked() {
                    match hidhide::hide(&p.conflict_vids()) {
                        Ok(r) => { *hidden = r.hidden.clone(); persist(hide_state, hidden); *status = r.message; }
                        Err(e) => *status = e,
                    }
                }
                if ui.add_enabled(*elevated && !hidden.is_empty(), egui::Button::new("Restore devices")).clicked() {
                    let n = hidhide::restore(hidden).unwrap_or(0);
                    hidden.clear();
                    let _ = std::fs::remove_file(&hide_state);
                    *status = format!("Restored {} device(s).", n);
                }
                if let Some(uri) = p.launch_uri() {
                    if ui.add_enabled(avail, egui::Button::new("▶ Launch")).clicked() {
                        let mut ok = true;
                        if !sys::any_process_running(&p.running_processes()) {
                            if let Err(e) = p.save(rows) { *status = format!("Save before launch failed: {}", e); ok = false; }
                        }
                        if ok {
                            if *elevated { if let Ok(r) = hidhide::hide(&p.conflict_vids()) { *hidden = r.hidden.clone(); persist(hide_state, hidden); } }
                            sys::open_uri(&uri);
                            *status = "Launching… keep this app open; closing it frees hidden devices.".into();
                        }
                    }
                }
                if !*elevated {
                    ui.separator();
                    if ui.button("Run as admin").on_hover_text("Needed for Hide/Restore").clicked() {
                        if sys::relaunch_elevated() { std::process::exit(0); }
                    }
                }
            });
            ui.add_space(4.0);
        });

        egui::SidePanel::left("devices").resizable(true).default_width(440.0).show(ctx, |ui| {
            if let Some(tex) = textures.as_ref() {
                crate::visual::sidebar(ui, tex, devices, games[*selected].as_ref(), show_labels, &bound);
            } else {
                ui.label("Loading device images…");
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if !games[*selected].available() {
                ui.centered_and_justified(|ui| { ui.label("This game isn't supported yet — pick MechWarrior 5."); });
                return;
            }
            let p = games[*selected].as_ref();
            // Live set of tokens being pressed/moved right now — drives the green glow.
            let hot = crate::visual::hot_tokens(devices, p);

            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if let Some(tex) = textures.as_ref() {
                    ui.add(egui::Image::new(&tex.logo).fit_to_exact_size(egui::vec2(30.0, 30.0)).rounding(6.0));
                }
                ui.heading("Cockpit Bindings");
                ui.label(egui::RichText::new(
                    "— click a chip, then press the control / move the axis (Esc cancels). A chip turns green when you use it.",
                ).color(egui::Color32::from_rgb(95, 100, 115)));
            });
            // Device colour legend (which colour = which physical device).
            ui.horizontal(|ui| {
                let chip = |ui: &mut egui::Ui, col: egui::Color32, txt: &str| {
                    egui::Frame::none().fill(col).inner_margin(egui::Margin::symmetric(7.0, 2.0)).rounding(4.0).show(ui, |ui| {
                        ui.label(egui::RichText::new(txt).color(egui::Color32::BLACK).strong());
                    });
                };
                ui.label(egui::RichText::new("Devices:").color(TEXT_MAIN));
                chip(ui, STICK_COL, "Stick / Joystick");
                chip(ui, THROTTLE_COL, "Throttle / Pedals");
                chip(ui, LIVE, "active now");
            });
            ui.separator();

            // Spread the categories across N balanced columns (by row count) so the
            // whole control map fits with far less scrolling. One column row needs
            // ~470px; pick the column count that fits the current central width.
            let avail_w = ui.available_width();
            let avail_h = ui.available_height(); // bound each column's scroll so it can't paint over the footer
            let ncols = ((avail_w / 500.0).floor() as usize).clamp(1, 3);
            let col_w = avail_w / ncols as f32 - 16.0; // minus scrollbar/gutter
            let mut col_groups: Vec<Vec<&(String, Vec<usize>)>> = vec![Vec::new(); ncols];
            let mut heights = vec![0usize; ncols];
            for g in &groups {
                let c = (0..ncols).min_by_key(|&c| heights[c]).unwrap_or(0);
                col_groups[c].push(g);
                heights[c] += g.1.len() + 2; // +heading overhead
            }

            // Columns at the bounded central-panel level (so each gets an equal,
            // on-screen half), each with its own vertical scroll. Wrapping ui.columns
            // *inside* one ScrollArea breaks: the scroll area's inner ui is
            // horizontally unbounded, so the split lands the 2nd column off-screen.
            // ONE outer vertical ScrollArea (directly in the central panel, so its
            // height is bounded — it clips/scrolls and never paints over the footer),
            // with set_max_width so ui.columns splits the real width into equal halves.
            let _ = (avail_h, col_w);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(avail_w);
                ui.columns(ncols, |cols| {
                    for (col, groups_in_col) in cols.iter_mut().zip(&col_groups) {
                        for (cat, idxs) in groups_in_col {
                            col.add_space(3.0);
                            col.strong(cat);
                            egui::Grid::new(format!("grid_{cat}")).num_columns(4).spacing([8.0, 3.0]).striped(true).show(col, |ui| {
                                for &i in idxs.iter() {
                                    binding_row(ui, i, actions, rows, capture, devices, p, status, &hot);
                                }
                            });
                        }
                    }
                });
            });
        });

        // Footer overlays. A real TopBottomPanel::bottom gets overpainted by the
        // central columns here, so we float these on top of everything instead:
        // build branch/version + manual update at the bottom-right, status at the
        // bottom-left.
        egui::Area::new(egui::Id::new("footer_build"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -8.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgb(28, 32, 44)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let branch = match env!("GIT_BRANCH") { "" => "local", b => b };
                        let hash = env!("GIT_HASH");
                        let tag = if hash.is_empty() { format!("{branch} · v{}", crate::update::current_version()) }
                                  else { format!("{branch}@{hash} · v{}", crate::update::current_version()) };
                        ui.label(egui::RichText::new(tag).monospace().color(egui::Color32::from_rgb(150, 165, 190)));
                        if ui.button("⟳ Update").on_hover_text("Check GitHub for a newer release and install it").clicked() {
                            match crate::update::latest() {
                                Some((ver, url)) if crate::update::is_newer(&ver) => {
                                    *update.lock().unwrap() = Some((ver.clone(), url));
                                    *status = format!("Update available: v{ver} — click \"Update now\" in the banner above.");
                                }
                                Some((ver, _)) => *status = format!("You're up to date (v{}). Latest published is v{ver}.", crate::update::current_version()),
                                None => *status = "Update check failed — no connection, or no release published yet.".into(),
                            }
                        }
                    });
                });
            });
        egui::Area::new(egui::Id::new("footer_status"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(450.0, -8.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgb(28, 32, 44)).show(ui, |ui| {
                    ui.label(egui::RichText::new(status.as_str()).strong());
                });
            });

        if reload { self.load_selected(); }
        ctx.request_repaint_after(Duration::from_millis(30));
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if self.elevated && !self.hidden.is_empty() {
            let _ = hidhide::restore(&self.hidden);
            let _ = std::fs::remove_file(&self.hide_state);
        }
    }
}
