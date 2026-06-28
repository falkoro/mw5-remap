//! Named binding profiles saved to disk, so a user can keep several layouts per game
//! and switch between them (and share them — a profile is just text). The built-in
//! "App Defaults" is virtual (comes from the game provider) and can't be overwritten
//! or deleted. Format: one binding per line, `id<TAB>token<TAB>scale`. No serde — the
//! schema is trivial and the toolchain stays dependency-free.

use crate::games::Binding;
use std::path::PathBuf;

/// The built-in, read-only profile name (served from `default_bindings`, not disk).
pub const APP_DEFAULTS: &str = "App Defaults";

fn root() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(base).join("MW5-Remap").join("profiles")
}

/// Filesystem-safe folder name for a game (e.g. "MechWarrior 5" -> "MechWarrior_5").
fn game_key(game: &str) -> String {
    game.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

fn dir(game: &str) -> PathBuf { root().join(game_key(game)) }

/// Sanitised, trimmed profile name (keeps letters/digits/space/-/_).
pub fn safe_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .trim()
        .to_string()
}

/// Saved profile names for a game (excludes the built-in App Defaults), sorted.
pub fn list(game: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir(game)) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("profile") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    out.push(stem.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

/// Save the current bindings as a named profile (overwrites if it exists).
pub fn save(game: &str, name: &str, rows: &[Binding]) -> Result<(), String> {
    let name = safe_name(name);
    if name.is_empty() { return Err("Enter a profile name.".into()); }
    if name == APP_DEFAULTS { return Err("\"App Defaults\" is built-in — pick another name.".into()); }
    let d = dir(game);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let mut s = String::new();
    for b in rows {
        s.push_str(&format!("{}\t{}\t{}\r\n", b.id, b.token, b.scale));
    }
    std::fs::write(d.join(format!("{name}.profile")), s).map_err(|e| e.to_string())
}

/// Load a named profile's saved bindings, or None if missing/unreadable.
pub fn load(game: &str, name: &str) -> Option<Vec<Binding>> {
    let text = std::fs::read_to_string(dir(game).join(format!("{}.profile", safe_name(name)))).ok()?;
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.splitn(3, '\t');
        let id = it.next().unwrap_or("").trim().to_string();
        let token = it.next().unwrap_or("").trim().to_string();
        let scale = it.next().unwrap_or("1").trim().parse::<f32>().unwrap_or(1.0);
        if !id.is_empty() { out.push(Binding { id, token, scale }); }
    }
    Some(out)
}

/// Delete a saved profile.
pub fn delete(game: &str, name: &str) -> Result<(), String> {
    let name = safe_name(name);
    if name == APP_DEFAULTS { return Err("\"App Defaults\" is built-in — can't delete it.".into()); }
    std::fs::remove_file(dir(game).join(format!("{name}.profile"))).map_err(|e| e.to_string())
}

/// Apply a sparse binding set (from a profile or defaults) onto the full action-aligned
/// `rows`: each row gets its profile token/scale, or is cleared if the profile omits it.
/// Keeps `rows` aligned to the action list so new actions still show up.
pub fn apply(rows: &mut [Binding], from: &[Binding]) {
    use std::collections::HashMap;
    let map: HashMap<&str, &Binding> = from.iter().map(|b| (b.id.as_str(), b)).collect();
    for r in rows.iter_mut() {
        match map.get(r.id.as_str()) {
            Some(b) => { r.token = b.token.clone(); r.scale = b.scale; }
            None => { r.token.clear(); r.scale = 1.0; }
        }
    }
}
