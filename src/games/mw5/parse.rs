//! GameUserSettings.ini parse/edit helpers (no regex dependency). Axes are
//! `InputTypeToAxisKeyList=` lines; buttons live in the LAST ("Joystick") section
//! of the giant `InputTypeToActionKeyMap=` line.

const JOY_MARKER: &str = "Joystick, (ActionKeyMaps=";

/// Read axis bindings: id -> (key, scale).
pub(super) fn read_axes(text: &str) -> std::collections::HashMap<String, (String, f32)> {
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let l = line.trim();
        let p = "InputTypeToAxisKeyList=(AxisName=\"";
        if let Some(rest) = l.strip_prefix(p) {
            if let Some(qend) = rest.find('"') {
                let axis = &rest[..qend];
                let after = &rest[qend..];
                let scale = field(after, "Scale=").and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);
                if let Some(key) = field(after, "Key=") {
                    // Keep the FIRST line per AxisName (the primary/analog one); extra
                    // keys on the same axis (e.g. POV-hat look) are loaded separately.
                    map.entry(axis.to_string())
                        .or_insert((key.trim_end_matches(')').to_string(), scale));
                }
            }
        }
    }
    map
}

/// Split an axis row id into (AxisName, optional fixed Key). "Axis@Key" -> a
/// multi-key row (one of several keys sharing an axis, e.g. POV hat -> look);
/// plain "Axis" -> the primary single-line axis.
pub(super) fn split_axis_id(id: &str) -> (&str, Option<&str>) {
    match id.split_once('@') {
        Some((axis, key)) => (axis, Some(key)),
        None => (id, None),
    }
}

/// Byte span of the axis line for `axis` whose Key is exactly `key` (multi-key aware).
pub(super) fn axis_line_span_keyed(text: &str, axis: &str, key: &str) -> Option<(usize, usize)> {
    let prefix = format!("InputTypeToAxisKeyList=(AxisName=\"{}\"", axis);
    let suffix = format!(",Key={})", key);
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(rel) = text[from..].find(&prefix) {
        let s = from + rel;
        if s == 0 || bytes[s - 1] == b'\n' {
            let mut e = text[s..].find('\n').map(|i| s + i).unwrap_or(text.len());
            if e > s && bytes[e - 1] == b'\r' { e -= 1; }
            if text[s..e].ends_with(&suffix) { return Some((s, e)); }
        }
        from = s + prefix.len();
    }
    None
}

/// Read the Scale of the axis line for `axis` whose Key is exactly `key`.
pub(super) fn axis_line_scale(text: &str, axis: &str, key: &str) -> Option<f32> {
    let (s, e) = axis_line_span_keyed(text, axis, key)?;
    field(&text[s..e], "Scale=").and_then(|v| v.parse::<f32>().ok())
}

/// Extract a `name=VALUE` field up to the next ',' or ')'.
pub(super) fn field(s: &str, name: &str) -> Option<String> {
    let i = s.find(name)? + name.len();
    let rest = &s[i..];
    let end = rest.find(|c| c == ',' || c == ')').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Byte span (start, end-excluding-newline) of the line that begins with `needle`.
pub(super) fn line_span(text: &str, needle: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(rel) = text[from..].find(needle) {
        let s = from + rel;
        if s == 0 || bytes[s - 1] == b'\n' {
            let mut e = text[s..].find('\n').map(|i| s + i).unwrap_or(text.len());
            if e > s && bytes[e - 1] == b'\r' { e -= 1; }
            return Some((s, e));
        }
        from = s + needle.len();
    }
    None
}

/// Byte index just AFTER the last axis line (incl. its newline) — insertion point.
pub(super) fn last_axis_insert_point(text: &str) -> Option<usize> {
    let prefix = "InputTypeToAxisKeyList=(AxisName=\"";
    let bytes = text.as_bytes();
    let mut last = None;
    let mut from = 0;
    while let Some(rel) = text[from..].find(prefix) {
        let s = from + rel;
        if s == 0 || bytes[s - 1] == b'\n' {
            let e = text[s..].find('\n').map(|i| s + i + 1).unwrap_or(text.len());
            last = Some(e);
        }
        from = s + prefix.len();
    }
    last
}

/// The Joystick section is the last one on the map line. Returns (head, body).
pub(super) fn split_joy_section(line: &str) -> Option<(String, String)> {
    let idx = line.find(JOY_MARKER)?;
    Some((line[..idx].to_string(), line[idx..].to_string()))
}

/// Read button bindings from the Joystick section: id -> key.
pub(super) fn read_buttons(text: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let line = match text.lines().find(|l| l.starts_with("InputTypeToActionKeyMap=")) {
        Some(l) => l, None => return map,
    };
    let body = match split_joy_section(line) { Some((_, b)) => b, None => return map };
    let mut search = body.as_str();
    let needle = "(ActionName=\"";
    while let Some(rel) = search.find(needle) {
        let after = &search[rel + needle.len()..];
        if let Some(qend) = after.find('"') {
            let name = &after[..qend];
            // does this tuple have a bound key?
            let tail = &after[qend..];
            if let Some(k) = bounded_key(tail) {
                map.insert(name.to_string(), k);
            }
            search = &after[qend..];
        } else { break; }
    }
    map
}

/// Given text starting at the closing quote of an ActionName, return its bound
/// Key if the tuple is `",BoundedKeys=((Key=K))..."`, else None (unbound).
pub(super) fn bounded_key(tail: &str) -> Option<String> {
    let p = "\",BoundedKeys=((Key=";
    if let Some(rest) = tail.strip_prefix(p) {
        let end = rest.find(')')?;
        return Some(rest[..end].to_string());
    }
    None
}

/// Replace one action tuple in `body` by paren-depth scan. Returns true if found.
pub(super) fn set_action(body: &mut String, action: &str, key: &str) -> bool {
    let needle = format!("(ActionName=\"{}\"", action);
    let start = match body.find(&needle) { Some(s) => s, None => return false };
    let bytes = body.as_bytes();
    let mut depth = 0i32;
    let mut end = None;
    for i in start..bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => { depth -= 1; if depth == 0 { end = Some(i); break; } }
            _ => {}
        }
    }
    let end = match end { Some(e) => e, None => return false };
    let replacement = if key.is_empty() || key == "None" {
        format!("(ActionName=\"{}\")", action)
    } else {
        format!("(ActionName=\"{}\",BoundedKeys=((Key={})))", action, key)
    };
    body.replace_range(start..=end, &replacement);
    true
}
