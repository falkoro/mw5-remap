//! Persisted top-to-bottom ORDER of the device images in the sidebar, so the user can move
//! a device up/down (the ▲/▼ buttons on each header) and have it stick. Mirrors `layout.rs`'s
//! persistence pattern: a thread-local cache + a tiny `%LOCALAPPDATA%\MW5-Remap\device_order.txt`
//! file (one device-key per line). Hidden devices keep their saved slot; the UI only ever
//! swaps two VISIBLE neighbours (it passes the two keys to `swap`).

use std::cell::RefCell;
use std::path::PathBuf;

fn file_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(base).join("MW5-Remap").join("device_order.txt")
}

fn load_file() -> Vec<String> {
    std::fs::read_to_string(file_path())
        .map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

fn save_file(order: &[String]) {
    let p = file_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&p, order.join("\r\n"));
}

thread_local! {
    static ORDER: RefCell<Vec<String>> = RefCell::new(load_file());
}

/// The `known` device keys arranged by the saved order, with any key not yet in the saved
/// order appended in its given position — so a brand-new device shows up at the bottom of
/// the list until the user moves it. Always returns exactly the `known` keys.
pub(super) fn ordered(known: &[&str]) -> Vec<String> {
    ORDER.with(|o| {
        let saved = o.borrow();
        let mut out: Vec<String> =
            saved.iter().filter(|k| known.contains(&k.as_str())).cloned().collect();
        for k in known {
            if !out.iter().any(|s| s == k) {
                out.push((*k).to_string());
            }
        }
        out
    })
}

/// Swap two device keys in the saved order (the ▲/▼ buttons pass a key and its neighbour),
/// then persist. Keys not yet present are inserted first so the swap always has both ends.
pub(super) fn swap(a: &str, b: &str) {
    ORDER.with(|o| {
        let mut v = o.borrow_mut();
        for k in [a, b] {
            if !v.iter().any(|s| s == k) {
                v.push(k.to_string());
            }
        }
        let ia = v.iter().position(|s| s == a);
        let ib = v.iter().position(|s| s == b);
        if let (Some(ia), Some(ib)) = (ia, ib) {
            v.swap(ia, ib);
            save_file(&v);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_appends_unknown_and_keeps_known_only() {
        // With nothing saved, order is exactly the given (default) order.
        let got = ordered(&["stick", "base", "pedals", "vkb"]);
        assert_eq!(got, vec!["stick", "base", "pedals", "vkb"]);
    }

    #[test]
    fn swap_then_ordered_reflects_new_order() {
        // Operate on the thread-local directly (no file IO assertions — just the in-memory
        // ordering logic). Seed a known order, swap two, confirm the result.
        ORDER.with(|o| *o.borrow_mut() = vec!["stick".into(), "base".into(), "pedals".into()]);
        swap("stick", "base");
        let got = ordered(&["stick", "base", "pedals"]);
        assert_eq!(got, vec!["base", "stick", "pedals"]);
    }
}
