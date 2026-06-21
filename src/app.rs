//! The egui application: a press-to-bind grid editor over a pluggable game
//! provider. Each frame we poll joysticks; if a capture is active, the next new
//! button/axis becomes the binding. Devices/HidHide/launch live in the side modules.

use crate::games::{self, Action, Binding, GameProvider, Kind, Role};
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

        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.label(egui::RichText::new(status.as_str()).strong());
            ui.horizontal_wrapped(|ui| {
                for (idx, dev) in devices.iter().enumerate() {
                    let role = games[*selected].role_of(dev, idx);
                    let tag = match role { Role::Joystick => "Joystick", Role::Throttle => "Throttle", Role::Ignored => "ignored" };
                    let pressed: Vec<String> = dev.pressed_buttons().iter().map(|b| b.to_string()).collect();
                    let extra = if pressed.is_empty() { String::new() } else { format!("  btn {}", pressed.join(",")) };
                    ui.label(format!("#{} [{}] {}{}   |", dev.id, tag, dev.name, extra));
                }
            });
            ui.add_space(2.0);
        });

        egui::SidePanel::left("devices").resizable(true).default_width(420.0).show(ctx, |ui| {
            if let Some(tex) = textures.as_ref() {
                crate::visual::sidebar(ui, tex, devices, games[*selected].as_ref(), show_labels);
            } else {
                ui.label("Loading device images…");
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if !games[*selected].available() {
                ui.centered_and_justified(|ui| { ui.label("This game isn't supported yet — pick MechWarrior 5."); });
                return;
            }
            ui.add_space(4.0);
            ui.label("Click Bind, then physically move the axis or press the button. Esc cancels. You can also type a token directly.");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (cat, idxs) in &groups {
                    ui.add_space(6.0);
                    ui.strong(cat);
                    egui::Grid::new(format!("grid_{cat}")).num_columns(4).spacing([12.0, 6.0]).striped(true).show(ui, |ui| {
                        for &i in idxs {
                            let capturing = capture.as_ref().map(|c| c.row == i).unwrap_or(false);
                            if capturing { ui.colored_label(egui::Color32::from_rgb(230, 170, 40), format!("● {}", actions[i].label)); }
                            else { ui.label(&actions[i].label); }

                            ui.add_sized([250.0, 22.0], egui::TextEdit::singleline(&mut rows[i].token).hint_text("unbound"));

                            let btn_label = if capturing { "press…" } else { "Bind" };
                            if ui.button(btn_label).clicked() {
                                if capturing { *capture = None; }
                                else {
                                    let p = games[*selected].as_ref();
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

                            if actions[i].kind == Kind::Axis {
                                ui.horizontal(|ui| {
                                    let mut inv = rows[i].scale < 0.0;
                                    if ui.checkbox(&mut inv, "Invert").changed() {
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
                    });
                }
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
