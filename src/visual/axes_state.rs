//! Persisted open/closed state of the sidebar "Live axes" CollapsingHeader, remembered
//! PER GAME so each game keeps its own preference. Mirrors `order.rs`'s persistence
//! pattern: a thread-local cache + a tiny `%LOCALAPPDATA%\MW5-Remap\axes_open.txt` file,
//! one `gamename=true|false` per line. Unknown games default to OPEN.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

fn file_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(base).join("MW5-Remap").join("axes_open.txt")
}

fn load_file() -> HashMap<String, bool> {
    std::fs::read_to_string(file_path())
        .map(|t| {
            t.lines()
                .filter_map(|l| {
                    let (g, v) = l.split_once('=')?;
                    Some((g.trim().to_string(), v.trim() == "true"))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn save_file(map: &HashMap<String, bool>) {
    let p = file_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let body: String = map.iter().map(|(g, v)| format!("{g}={v}\r\n")).collect();
    let _ = std::fs::write(&p, body);
}

thread_local! {
    static OPEN: RefCell<HashMap<String, bool>> = RefCell::new(load_file());
}

/// Is the "Live axes" section open for `game`? Defaults to OPEN for an unknown game.
pub(super) fn is_open(game: &str) -> bool {
    OPEN.with(|o| o.borrow().get(game).copied().unwrap_or(true))
}

/// Remember the open/closed state of "Live axes" for `game`, then persist.
pub(super) fn set_open(game: &str, open: bool) {
    OPEN.with(|o| {
        let mut m = o.borrow_mut();
        m.insert(game.to_string(), open);
        save_file(&m);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_game_defaults_open_known_value_honoured() {
        // Operate on the thread-local directly (no file IO) so the test is pure and never
        // clobbers a real axes_open.txt — exactly the in-memory lookup `is_open` does.
        OPEN.with(|o| {
            let mut m = o.borrow_mut();
            m.clear();
            m.insert("Ace Combat 7".into(), false);
        });
        assert!(!is_open("Ace Combat 7"), "stored value honoured");
        assert!(is_open("Star Citizen"), "unknown game defaults to OPEN");
    }
}
