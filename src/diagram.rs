//! Generates a self-contained HTML infographic of the current control map.
//! No external files/images, so it opens offline in any browser. Built from the
//! live bindings, so re-exporting after an edit reflects the change ("see it visual").

use crate::games::{Action, Binding, Kind};

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

fn card(a: &Action, token: &str, scale: f32) -> String {
    if token.is_empty() {
        return format!(
            "<div class='card muted'><span class='badge'>—</span><div class='t'><b>{}</b><small>{} · not bound</small></div></div>",
            a.label, a.category
        );
    }
    let mut meta = a.category.to_string();
    if a.kind == Kind::Axis {
        meta.push_str(&format!(" · ×{:.1}", scale.abs()));
        if scale < 0.0 { meta.push_str(" · reversed"); }
    }
    format!(
        "<div class='card' style='border-left-color:{}'><span class='badge' style='background:{}'>{}</span><div class='t'><b>{}</b><small>{}</small></div></div>",
        color(&a.category), color(&a.category), short(token), a.label, meta
    )
}

const TEMPLATE: &str = r##"<!doctype html><html><head><meta charset="utf-8">
<title>MW5 Control Map</title><style>
*{box-sizing:border-box}body{margin:0;background:#0f1420;color:#e8edf6;font:15px/1.4 'Segoe UI',system-ui,sans-serif}
.wrap{max-width:1080px;margin:0 auto;padding:28px}
h1{font-size:26px;margin:0 0 2px}.sub{color:#9fb0c8;margin:0 0 20px}
.cols{display:grid;grid-template-columns:1fr 1fr;gap:22px}
@media(max-width:760px){.cols{grid-template-columns:1fr}}
.panel{background:#161d2e;border:1px solid #243049;border-radius:12px;padding:16px}
.panel h2{margin:0 0 4px;font-size:17px}.panel .role{color:#9fb0c8;font-size:12px;margin:0 0 12px}
svg{display:block;margin:0 auto 8px}
.card{display:flex;align-items:center;gap:10px;background:#1c2336;border-left:4px solid #888;border-radius:8px;padding:8px 10px;margin:7px 0}
.card.muted{opacity:.5;border-left-color:#3a4a66}
.badge{flex:0 0 auto;min-width:74px;text-align:center;font:600 12px/1 'Consolas',monospace;color:#fff;background:#555;border-radius:6px;padding:6px 8px}
.card.muted .badge{background:#33405c}
.t b{display:block;font-weight:600}.t small{color:#9fb0c8;font-size:12px}
.legend{display:flex;flex-wrap:wrap;gap:14px;margin:18px 0 0;color:#9fb0c8;font-size:12px}
.legend span{display:inline-flex;align-items:center;gap:6px}.dot{width:10px;height:10px;border-radius:3px;display:inline-block}
.foot{color:#7e8ba3;font-size:12px;margin-top:18px;border-top:1px solid #243049;padding-top:12px}
</style></head><body><div class="wrap">
<h1>MechWarrior 5 — Your Control Map</h1>
<p class="sub">%%SUB%%</p>
<div class="cols">
 <div class="panel">
  <h2>Aim stick</h2><p class="role">MOZA AB6 FFB Base · "Joystick" role</p>
  <svg width="120" height="120" viewBox="0 0 120 120"><ellipse cx="60" cy="104" rx="34" ry="9" fill="#243049"/><rect x="53" y="52" width="14" height="50" rx="6" fill="#33405c"/><circle cx="60" cy="44" r="26" fill="#3d8be0"/><circle cx="60" cy="30" r="5" fill="#0f1420"/><circle cx="49" cy="50" r="4" fill="#0f1420"/><circle cx="71" cy="50" r="4" fill="#0f1420"/></svg>
  %%JOY%%
 </div>
 <div class="panel">
  <h2>Pedals / Throttle</h2><p class="role">MOZA MRP Rudder Pedals · "Throttle" role</p>
  <svg width="160" height="120" viewBox="0 0 160 120"><rect x="20" y="30" width="46" height="64" rx="8" fill="#3dba6b" transform="rotate(-8 43 62)"/><rect x="94" y="30" width="46" height="64" rx="8" fill="#3dba6b" transform="rotate(8 117 62)"/><rect x="14" y="96" width="132" height="10" rx="4" fill="#243049"/></svg>
  %%THR%%
 </div>
</div>
%%UNBSEC%%
<div class="legend">
 <span><i class="dot" style="background:#3d8be0"></i>Aiming</span>
 <span><i class="dot" style="background:#3dba6b"></i>Movement</span>
 <span><i class="dot" style="background:#e0533d"></i>Weapons</span>
 <span><i class="dot" style="background:#d9a13b"></i>Targeting</span>
 <span><i class="dot" style="background:#9b6bd9"></i>Systems</span>
</div>
<p class="foot">Badges show the in-game token (e.g. <b>Button1</b>, <b>Axis2</b>, <b>Hat_3</b>). To see which physical button is which number, use <b>Identify</b> / the live device strip in the app. Re-export this after editing to refresh it.</p>
</div></body></html>"##;

pub fn render(actions: &[Action], bindings: &[Binding]) -> String {
    let bmap: std::collections::HashMap<&str, &Binding> =
        bindings.iter().map(|b| (b.id.as_str(), b)).collect();
    let (mut joy, mut thr, mut unb) = (String::new(), String::new(), String::new());
    let mut bound = 0;
    for a in actions {
        let (tok, scale) = bmap.get(a.id.as_str()).map(|b| (b.token.as_str(), b.scale)).unwrap_or(("", 1.0));
        if tok.is_empty() {
            unb.push_str(&card(a, "", 1.0));
        } else if tok.starts_with("Joystick_") {
            bound += 1;
            joy.push_str(&card(a, tok, scale));
        } else {
            bound += 1;
            thr.push_str(&card(a, tok, scale));
        }
    }
    let unbsec = if unb.is_empty() {
        String::new()
    } else {
        format!("<div class=\"panel\" style=\"margin-top:22px\"><h2>Not bound yet</h2><p class=\"role\">Bind these in the app (click Bind, move/press the control)</p>{}</div>", unb)
    };
    let sub = format!("{} of {} actions bound. Generated from your live config.", bound, actions.len());
    TEMPLATE
        .replace("%%SUB%%", &sub)
        .replace("%%JOY%%", &joy)
        .replace("%%THR%%", &thr)
        .replace("%%UNBSEC%%", &unbsec)
}
