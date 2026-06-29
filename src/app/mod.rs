//! The egui application: a press-to-bind grid editor over a pluggable game
//! provider. Each frame we poll joysticks; if a capture is active, the next new
//! button/axis becomes the binding. Devices/HidHide/launch live in the side modules.
//! Toolbar/banner/footer chrome lives in `panels`; the grid row + colours in `widgets`.

mod community_ui;
mod detect;
mod export_ui;
mod panels;
mod tabs;
mod toolbar;
mod vjoy_ui;
mod widgets;

pub(crate) use export_ui::ExportOpts;

/// Top-level view selector. The Bind tab is the press-to-bind grid editor; the
/// vJoy Setup tab is the sole home of the vJoy routing UI.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Bind,
    VjoySetup,
}

use crate::games::{self, Action, Binding, GameProvider, Kind};
use crate::vjoy_map::VjoyMap;
use crate::{hidhide, input, sys};
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vjoy_ui::VjoyCapture;
use widgets::Capture;

pub struct App {
    games: Vec<Box<dyn GameProvider>>,
    selected: usize,
    tab: Tab, // which top-level tab is showing (Bind = default editor, VjoySetup = routing)
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
    show_export_dialog: bool,
    export_opts: ExportOpts,
    pending_export: Option<ExportOpts>, // set when Export clicked; consumed when the screenshot arrives
    export_shot_sent: bool,             // true once the Screenshot cmd is issued (so the filtered frame paints first)
    last_panel_rect: egui::Rect,        // screen rect of the device SidePanel, captured during render
    profile: String,                    // currently selected binding profile (default: App Defaults)
    profile_input: String,              // "new profile name" text field
    vjoy_map: VjoyMap,                  // config-driven physical-stick -> vJoy routing table
    vjoy_sel: Option<(u16, u16)>,       // physical stick selected in the Route-to-vJoy panel
    vjoy_capture: Option<VjoyCapture>,  // pending "actuate a control to bind it" capture
    vjoy_btn_pick: u8,                  // vJoy button number the next bind targets
    vjoy_axis_pick: u32,                // vJoy axis (HID usage) the next bind targets
    vjoy_pair_fwd: u8,                  // "combine 2 axes" forward (positive) source axis index
    vjoy_pair_rev: u8,                  // "combine 2 axes" reverse (negative) source axis index
    vjoy_paused: bool,                  // pause feeding without deleting mappings
    show_community: bool,               // "🌐 Community profiles" browser open?
    community: Arc<Mutex<crate::community::CommunityState>>, // async listing fetch result
    community_dl: Arc<Mutex<crate::community::DownloadState>>, // async profile download result
    notif_log: Vec<String>, // history of status changes for the top-right notification feed (newest last)
    notif_collapsed: bool, // when true the feed shows only a small 🔔 badge (history preserved)
    live_muted: HashSet<(u16, u16)>, // devices soft-muted from the LIVE display (glow + Detected); UI-only, no HidHide
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
            tab: Tab::Bind,
            actions: Vec::new(),
            rows: Vec::new(),
            devices: Vec::new(),
            capture: None,
            status: "Ready.".into(),
            elevated: sys::is_elevated(),
            hidden: Vec::new(),
            hide_state,
            textures: None,
            show_labels: false, // images CLEAN by default; user clicks 🏷 Arrows to overlay callouts
            update,
            show_export_dialog: false,
            export_opts: ExportOpts::default(),
            pending_export: None,
            export_shot_sent: false,
            last_panel_rect: egui::Rect::NOTHING,
            profile: crate::profiles::APP_DEFAULTS.to_string(),
            profile_input: String::new(),
            vjoy_map: VjoyMap::load(),
            vjoy_sel: None,
            vjoy_capture: None,
            vjoy_btn_pick: 1,
            vjoy_axis_pick: crate::vjoy::HID_X,
            vjoy_pair_fwd: 0,
            vjoy_pair_rev: 1,
            vjoy_paused: false,
            show_community: false,
            community: Arc::new(Mutex::new(crate::community::CommunityState::Idle)),
            community_dl: Arc::new(Mutex::new(crate::community::DownloadState::Idle)),
            notif_log: Vec::new(),
            notif_collapsed: true, // start collapsed: show only the 🔔 badge until clicked
            live_muted: HashSet::new(),
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
                    for ax in 0..8 {
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

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.devices = input::poll();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.capture = None;
            self.vjoy_capture = None;
            self.status = "Capture cancelled.".into();
        }
        self.resolve_capture();
        vjoy_ui::resolve_capture(&mut self.vjoy_capture, &self.devices, &mut self.vjoy_map, &mut self.status);
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

        let App { games, selected, tab, actions, rows, devices, capture, status, elevated, hidden, hide_state, textures, show_labels, update, show_export_dialog, export_opts, pending_export, export_shot_sent, last_panel_rect, profile, profile_input, vjoy_map, vjoy_sel, vjoy_capture, vjoy_btn_pick, vjoy_axis_pick, vjoy_pair_fwd, vjoy_pair_rev, vjoy_paused, show_community, community, community_dl, notif_log, notif_collapsed, live_muted } = self;

        // Capture every status change into the notification feed history (newest last,
        // capped). The top-right feed below renders this so old notifications stay visible.
        if !status.trim().is_empty() && notif_log.last().map(|s| s != status).unwrap_or(true) {
            notif_log.push(status.clone());
            if notif_log.len() > 8 { notif_log.remove(0); }
        }

        // token -> bound action label, so the device diagram can show WHAT is bound
        // to each control (not just the control's name).
        let bound: HashMap<String, String> = rows.iter()
            .filter(|b| !b.token.is_empty())
            .filter_map(|b| actions.iter().find(|a| a.id == b.id).map(|a| (b.token.clone(), a.label.clone())))
            .collect();

        // Config-driven vJoy feed: route ANY physical stick onto vJoy device 1 from the
        // user-built mapping table (no device-specific code). Feeding is ACTIVE whenever
        // there's ≥1 mapping and the user hasn't paused; that same flag gates the vJoy
        // .Remap block via write_hotas_mappings.
        let vjoy_active = !*vjoy_paused && !vjoy_map.mappings.is_empty();
        crate::vjoy::set_active(vjoy_active);
        if vjoy_active { vjoy_map.apply(devices); }

        // Live "Detected:" readout — which stick + control is actuated this frame,
        // resolved through vJoy to the MW5 token + bound action. Shown under the tab
        // bar (below), so it's visible in BOTH tabs at once.
        let detected = detect::detect_input(devices, live_muted, games[*selected].as_ref(), vjoy_map, &bound);

        // Top-level tab selector — ABOVE everything else. Bind = the editor; vJoy
        // Setup = the routing UI. The feed loop above runs regardless of tab.
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.selectable_value(tab, Tab::Bind, "🎮 Bind");
                ui.selectable_value(tab, Tab::VjoySetup, "🕹 vJoy Setup");
                ui.separator();
                match &detected {
                    Some(s) => ui.label(egui::RichText::new(format!("Detected: {s}"))
                        .strong().color(egui::Color32::from_rgb(70, 210, 110))),
                    None => ui.label(egui::RichText::new("Detected: —")
                        .color(egui::Color32::from_rgb(120, 128, 145))),
                };
            });
            ui.add_space(3.0);
        });

        let reload = match tab {
            Tab::Bind => tabs::bind_tab(
                ctx, games, selected, actions, rows, devices, capture, status, *elevated, hidden,
                hide_state, textures, show_labels, update, show_export_dialog, export_opts,
                pending_export, export_shot_sent, last_panel_rect, profile, profile_input,
                show_community, community, community_dl, vjoy_map, &bound, &groups, live_muted,
            ),
            Tab::VjoySetup => {
                tabs::vjoy_setup_tab(
                    ctx, devices, vjoy_map, vjoy_capture, vjoy_sel, vjoy_btn_pick, vjoy_axis_pick,
                    vjoy_pair_fwd, vjoy_pair_rev, vjoy_paused, status, *elevated, hidden, hide_state,
                );
                false
            }
        };

        // Top-right notification feed (history) — drawn last so it floats over both
        // tabs; offset down when the update banner is also occupying the top-right.
        panels::notif_feed(ctx, notif_log, notif_collapsed, update.lock().unwrap().is_some());

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
