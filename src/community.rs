//! Community-profiles browser client: lists and downloads shared binding profiles
//! from a public GitHub repo, so users can grab layouts others have shared (and learn
//! how to share their own). Reuses the WinHTTP GET + hand-rolled JSON parsing from
//! `update.rs` — NO new crates, NO serde. Per-game folders in the repo mirror the
//! local profiles folders (`profiles::game_key`), e.g. `MechWarrior_5`.

use crate::update::{http_get, http_get_bytes};
use std::sync::{Arc, Mutex};

/// GitHub repo that holds the shared `.profile` files (easy to point elsewhere).
pub const OWNER: &str = "falkoro";
pub const REPO: &str = "mw5-remap-profiles";

/// State of the async listing fetch, shared between the worker thread and the UI.
#[derive(Clone)]
pub enum CommunityState {
    Idle,
    Loading,
    /// (display name, raw download_url) pairs — empty means "none shared yet".
    Loaded(Vec<(String, String)>),
    Failed(String),
}

/// `github.com/<owner>/<repo>` — shown in the UI as the place to PR your profile.
pub fn share_url() -> String { format!("github.com/{OWNER}/{REPO}") }

/// Kick off a background fetch of the profile listing for `game`. The result lands
/// in `state` (Loaded/Failed); the UI shows "Loading…" until then. Never blocks.
pub fn start_load(state: &Arc<Mutex<CommunityState>>, game: &str) {
    *state.lock().unwrap() = CommunityState::Loading;
    let st = state.clone();
    let game = game.to_string();
    std::thread::spawn(move || {
        let next = match list(&game) {
            Ok(v) => CommunityState::Loaded(v),
            Err(e) => CommunityState::Failed(e),
        };
        *st.lock().unwrap() = next;
    });
}

/// List shared `.profile` files for `game` via the GitHub contents API. Returns an
/// empty Vec when the repo/folder doesn't exist yet (404) — caller shows "be the
/// first to share". Errors only on a hard network failure.
pub fn list(game: &str) -> Result<Vec<(String, String)>, String> {
    let folder = crate::profiles::game_key(game);
    let url = format!("https://api.github.com/repos/{OWNER}/{REPO}/contents/{folder}");
    let body = http_get(&url, "Accept: application/vnd.github+json\r\n")
        .ok_or("No network — couldn't reach GitHub.")?;
    // A missing repo/folder returns a JSON object ({"message":"Not Found"}); a real
    // listing is a JSON array. Anything that isn't an array → treat as "none yet".
    if !body.trim_start().starts_with('[') {
        return Ok(Vec::new());
    }
    Ok(parse_listing(&body))
}

/// Download one profile's raw bytes and write it into the user's profiles folder for
/// `game`. Returns the saved name on success.
pub fn download(name: &str, url: &str, game: &str) -> Result<String, String> {
    let bytes = http_get_bytes(url, "").ok_or("Download failed (no network?).")?;
    if bytes.is_empty() { return Err("Downloaded file was empty.".into()); }
    let dest = crate::profiles::import_path(game, name)?;
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    Ok(name.to_string())
}

/// Hand-parse the contents-API JSON array for `.profile` entries. Each array element
/// is an object whose `"name"` is followed (within the same object) by its
/// `"download_url"`. We bound the download_url search to the current object so a
/// directory entry (whose download_url is `null`) can't steal a later file's URL.
fn parse_listing(s: &str) -> Vec<(String, String)> {
    const NAME: &str = "\"name\":\"";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = s[from..].find(NAME) {
        let i = from + rel + NAME.len();
        let rest = &s[i..];
        let end = match rest.find('"') { Some(e) => e, None => break };
        let name = &rest[..end];
        from = i + end;
        // Bound to this object: everything up to the next "name" key.
        let seg_end = s[from..].find(NAME).map(|r| from + r).unwrap_or(s.len());
        if name.ends_with(".profile") {
            if let Some(url) = json_field(&s[from..seg_end], "\"download_url\":\"") {
                let disp = name.trim_end_matches(".profile").to_string();
                out.push((disp, url));
            }
        }
    }
    out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    out
}

/// First quoted string value following `needle` in `s` (None if absent or `null`).
fn json_field(s: &str, needle: &str) -> Option<String> {
    let i = s.find(needle)? + needle.len();
    let rest = &s[i..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_listing_picks_profile_files_and_urls() {
        let json = r#"[
          {"name":"Falkoro MOZA.profile","path":"MechWarrior_5/Falkoro MOZA.profile",
           "download_url":"https://raw.githubusercontent.com/o/r/main/MechWarrior_5/Falkoro%20MOZA.profile","type":"file"},
          {"name":"readme.md","download_url":"https://raw/x/readme.md","type":"file"},
          {"name":"sub","download_url":null,"type":"dir"},
          {"name":"VKB.profile","download_url":"https://raw/x/VKB.profile","type":"file"}
        ]"#;
        let v = parse_listing(json);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].0, "Falkoro MOZA"); // .profile stripped, sorted
        assert!(v[0].1.ends_with("Falkoro%20MOZA.profile"));
        assert_eq!(v[1].0, "VKB");
    }

    #[test]
    fn directory_null_download_url_is_ignored() {
        // A dir entry with null download_url must NOT borrow the following file's url.
        let json = r#"[{"name":"folder","download_url":null,"type":"dir"},
                       {"name":"A.profile","download_url":"https://x/A.profile","type":"file"}]"#;
        let v = parse_listing(json);
        assert_eq!(v, vec![("A".to_string(), "https://x/A.profile".to_string())]);
    }
}
