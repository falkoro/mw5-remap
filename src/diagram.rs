//! Generates a self-contained HTML infographic of the current control map — now
//! built around the REAL device photos (embedded as base64) with numbered badges
//! laid over each control, mirroring the in-app panel. Re-export after an edit and
//! it reflects the change. No external files, so it opens offline in any browser.

use crate::games::{Action, Binding};

const STICK_PNG: &[u8] = include_bytes!("../assets/mhg_stick.png");
const BASE_PNG: &[u8] = include_bytes!("../assets/ab6_base.png");
const PEDALS_JPG: &[u8] = include_bytes!("../assets/mrp_pedals.jpg");

/// Minimal base64 (no crate) for inlining the images as data URIs.
fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = ((c[0] as u32) << 16) | ((*c.get(1).unwrap_or(&0) as u32) << 8) | (*c.get(2).unwrap_or(&0) as u32);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if c.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn color(cat: &str) -> &'static str {
    match cat {
        "Weapons" => "#e0533d",
        "Aiming" => "#3d8be0",
        "Movement" => "#3dba6b",
        "Targeting" => "#d9a13b",
        "Systems" => "#9b6bd9",
        _ => "#888888",
    }
}

fn short(token: &str) -> String {
    token.trim_start_matches("Joystick_").trim_start_matches("Throttle_").to_string()
}

/// A numbered badge over a device image: position (%), the number shown, and the
/// token it represents (looked up against the live bindings for its action name).
struct Spot { nx: f32, ny: f32, num: &'static str, token: &'static str }

const BASE_SPOTS: &[Spot] = &[
    Spot { nx: 46.0, ny: 30.0, num: "1", token: "Joystick_Axis1" },
    Spot { nx: 55.0, ny: 40.0, num: "2", token: "Joystick_Axis2" },
];
const PEDAL_SPOTS: &[Spot] = &[
    Spot { nx: 50.0, ny: 74.0, num: "1", token: "Throttle_Axis1" },
    Spot { nx: 30.0, ny: 40.0, num: "2", token: "Throttle_Axis2" },
];
const STICK_SPOTS: &[Spot] = &[
    Spot { nx: 50.0, ny: 27.0, num: "H", token: "Joystick_Hat_1" },
];

/// Render one device image with its badges overlaid + a numbered legend beneath.
fn device(title: &str, role: &str, mime: &str, bytes: &[u8], width: u32, spots: &[Spot],
          tok2label: &std::collections::HashMap<String, (String, String)>) -> String {
    let mut badges = String::new();
    let mut legend = String::new();
    for s in spots {
        let (label, cat) = tok2label.get(s.token).cloned()
            .unwrap_or_else(|| ("not bound".into(), "".into()));
        let c = color(&cat);
        badges.push_str(&format!(
            "<span class='badge' style='left:{}%;top:{}%;background:{}'>{}</span>",
            s.nx, s.ny, c, s.num
        ));
        legend.push_str(&format!(
            "<div class='lg'><span class='n' style='background:{}'>{}</span><b>{}</b><code>{}</code></div>",
            c, s.num, label, short(s.token)
        ));
    }
    format!(
        "<div class='dev'><h2>{title}</h2><p class='role'>{role}</p>\
         <div class='imgwrap' style='width:{width}px'>\
         <img src='data:{mime};base64,{img}'>{badges}</div>{legend}</div>",
        img = b64(bytes)
    )
}

const TEMPLATE: &str = r##"<!doctype html><html><head><meta charset="utf-8">
<title>MW5 Control Map</title><style>
*{box-sizing:border-box}body{margin:0;background:#0f1420;color:#e8edf6;font:15px/1.4 'Segoe UI',system-ui,sans-serif}
.wrap{max-width:1160px;margin:0 auto;padding:28px}
h1{font-size:26px;margin:0 0 2px}.sub{color:#9fb0c8;margin:0 0 20px}
.devs{display:grid;grid-template-columns:1fr 1fr 1fr;gap:18px}
@media(max-width:900px){.devs{grid-template-columns:1fr}}
.dev{background:#161d2e;border:1px solid #243049;border-radius:12px;padding:16px}
.dev h2{margin:0 0 2px;font-size:17px}.dev .role{color:#9fb0c8;font-size:12px;margin:0 0 12px}
.imgwrap{position:relative;margin:0 auto 12px;max-width:100%}
.imgwrap img{width:100%;height:auto;display:block;border-radius:8px}
.badge{position:absolute;transform:translate(-50%,-50%);min-width:20px;height:20px;padding:0 5px;
 border-radius:11px;border:2px solid #0f1420;color:#0b0e16;font:700 12px/20px 'Segoe UI',sans-serif;text-align:center;box-shadow:0 1px 4px rgba(0,0,0,.5)}
.lg{display:flex;align-items:center;gap:8px;background:#1c2336;border-radius:7px;padding:6px 9px;margin:6px 0;font-size:13px}
.lg .n{flex:0 0 auto;width:20px;height:20px;border-radius:10px;color:#0b0e16;font-weight:700;text-align:center;line-height:20px}
.lg code{margin-left:auto;color:#9fb0c8;font:12px 'Consolas',monospace}
.panel{background:#161d2e;border:1px solid #243049;border-radius:12px;padding:16px;margin-top:20px}
.panel h2{margin:0 0 10px;font-size:17px}
.btns{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:8px}
.card{display:flex;align-items:center;gap:10px;background:#1c2336;border-left:4px solid #888;border-radius:8px;padding:7px 10px}
.card.muted{opacity:.5;border-left-color:#3a4a66}
.badge2{flex:0 0 auto;min-width:70px;text-align:center;font:600 12px/1 'Consolas',monospace;color:#fff;background:#555;border-radius:6px;padding:6px 8px}
.t b{display:block;font-weight:600}.t small{color:#9fb0c8;font-size:12px}
.legend{display:flex;flex-wrap:wrap;gap:14px;margin:18px 0 0;color:#9fb0c8;font-size:12px}
.legend span{display:inline-flex;align-items:center;gap:6px}.dot{width:10px;height:10px;border-radius:3px;display:inline-block}
.foot{color:#7e8ba3;font-size:12px;margin-top:18px;border-top:1px solid #243049;padding-top:12px}
</style></head><body><div class="wrap">
<h1>MechWarrior 5 — Your Control Map</h1>
<p class="sub">%%SUB%%</p>
<div class="devs">%%DEVS%%</div>
<div class="panel"><h2>Stick buttons</h2><div class="btns">%%BTNS%%</div></div>
%%UNBSEC%%
<div class="legend">
 <span><i class="dot" style="background:#3d8be0"></i>Aiming</span>
 <span><i class="dot" style="background:#3dba6b"></i>Movement</span>
 <span><i class="dot" style="background:#e0533d"></i>Weapons</span>
 <span><i class="dot" style="background:#d9a13b"></i>Targeting</span>
 <span><i class="dot" style="background:#9b6bd9"></i>Systems</span>
</div>
<p class="foot">Numbered badges sit on the real control; the number is the in-game token index (<b>Aim Up/Down = Axis 1</b>). Open the app's left panel and press a stick button to see its number light up. Re-export after editing to refresh.</p>
</div></body></html>"##;

fn card(num: &str, label: &str, cat: &str) -> String {
    format!(
        "<div class='card' style='border-left-color:{c}'><span class='badge2' style='background:{c}'>{num}</span><div class='t'><b>{label}</b><small>{cat}</small></div></div>",
        c = color(cat)
    )
}

pub fn render(actions: &[Action], bindings: &[Binding]) -> String {
    // token -> (action label, category), and id -> action (for labels).
    let amap: std::collections::HashMap<&str, &Action> = actions.iter().map(|a| (a.id.as_str(), a)).collect();
    let mut tok2label = std::collections::HashMap::new();
    for b in bindings {
        if b.token.is_empty() { continue; }
        if let Some(a) = amap.get(b.id.as_str()) {
            tok2label.entry(b.token.clone()).or_insert((a.label.clone(), a.category.clone()));
        }
    }

    let devs = format!(
        "{}{}{}",
        device("Aim stick", "MOZA AB6 FFB Base · \"Joystick\"", "image/png", BASE_PNG, 300, BASE_SPOTS, &tok2label),
        device("Pedals", "MOZA MRP Rudder Pedals · \"Throttle\"", "image/jpeg", PEDALS_JPG, 300, PEDAL_SPOTS, &tok2label),
        device("Grip", "MHG · buttons + POV hat", "image/png", STICK_PNG, 300, STICK_SPOTS, &tok2label),
    );

    // Stick buttons grid: Joystick_Button1..20 -> action (number = button #).
    let mut btns = String::new();
    let bound_btn: std::collections::HashMap<u32, &Action> = bindings.iter()
        .filter_map(|b| b.token.strip_prefix("Joystick_Button").and_then(|s| s.parse::<u32>().ok()).map(|n| (n, b)))
        .filter_map(|(n, b)| amap.get(b.id.as_str()).map(|a| (n, *a)))
        .collect();
    for n in 1..=20u32 {
        if let Some(a) = bound_btn.get(&n) {
            btns.push_str(&card(&n.to_string(), &a.label, &a.category));
        } else {
            btns.push_str(&format!("<div class='card muted'><span class='badge2'>{n}</span><div class='t'><b>—</b><small>free</small></div></div>"));
        }
    }

    // Unbound actions section.
    let mut unb = String::new();
    let mut nunb = 0;
    for a in actions {
        let tok = bindings.iter().find(|b| b.id == a.id).map(|b| b.token.as_str()).unwrap_or("");
        if tok.is_empty() {
            nunb += 1;
            unb.push_str(&format!("<div class='card muted'><span class='badge2'>—</span><div class='t'><b>{}</b><small>{} · not bound</small></div></div>", a.label, a.category));
        }
    }
    let unbsec = if unb.is_empty() { String::new() } else {
        format!("<div class='panel'><h2>Not bound yet ({nunb})</h2><div class='btns'>{unb}</div></div>")
    };

    let bound_total = bindings.iter().filter(|b| !b.token.is_empty()).count();
    let sub = format!("{} of {} actions bound · live from your config + HOTAS mappings.", bound_total, actions.len());
    TEMPLATE
        .replace("%%SUB%%", &sub)
        .replace("%%DEVS%%", &devs)
        .replace("%%BTNS%%", &btns)
        .replace("%%UNBSEC%%", &unbsec)
}
