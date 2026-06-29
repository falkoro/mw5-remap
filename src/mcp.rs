//! Minimal Model Context Protocol (MCP) server, exposed as `--mcp`. Speaks JSON-RPC
//! 2.0 over STDIO, NEWLINE-DELIMITED (one compact JSON object per line). Lets an
//! external AI agent (Claude Desktop/Code) read the user's joysticks and drive the
//! built-in vJoy routing. No serde / no new crates — requests are parsed with the
//! same hand-rolled string scanning as `community.rs`, responses built with `format!`
//! + a JSON string-escaper. CRITICAL: stdout carries ONLY protocol JSON; all logging
//! goes to stderr.

use crate::vjoy_map::{Target, VjoyMap};
use std::io::{BufRead, Write};

/// Read JSON-RPC request lines from stdin, write one response line per request to
/// stdout (flushed). Blocks until stdin closes. Notifications produce no response.
pub fn run() {
    eprintln!("mw5-remap MCP server v{} ready (stdio, newline-delimited)", env!("CARGO_PKG_VERSION"));
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        if line.trim().is_empty() { continue; }
        if let Some(resp) = handle(&line) {
            let mut out = stdout.lock();
            let _ = writeln!(out, "{resp}");
            let _ = out.flush();
        }
    }
}

/// Dispatch one request line to a response (None for notifications / missing id).
fn handle(line: &str) -> Option<String> {
    let method = str_field(line, "method")?;
    let id = extract_id(line)?; // no id => notification => no response
    match method.as_str() {
        "initialize" => Some(ok(&id, &init_result())),
        "tools/list" => Some(ok(&id, &format!("{{\"tools\":{TOOLS}}}"))),
        "tools/call" => {
            let text = call(line);
            let result = format!("{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}]}}", esc(&text));
            Some(ok(&id, &result))
        }
        _ => Some(err(&id, -32601, "Method not found")),
    }
}

fn init_result() -> String {
    format!(
        "{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{\"tools\":{{}}}},\
         \"serverInfo\":{{\"name\":\"mw5-remap\",\"version\":\"{}\"}}}}",
        env!("CARGO_PKG_VERSION")
    )
}

/// Tool definitions (compact one-line JSON — must not contain embedded newlines).
const TOOLS: &str = "[\
{\"name\":\"list_devices\",\"description\":\"List all connected joystick/HOTAS devices with their name, VID, PID, axis count, button count and POV-hat presence.\",\"inputSchema\":{\"type\":\"object\",\"properties\":{},\"required\":[]}},\
{\"name\":\"get_vjoy_routing\",\"description\":\"Return the current physical-device -> vJoy routing table (source, target, invert per mapping).\",\"inputSchema\":{\"type\":\"object\",\"properties\":{},\"required\":[]}},\
{\"name\":\"auto_route\",\"description\":\"Auto-route a connected device onto vJoy: every button -> sequential vJoy buttons, each present axis -> the next free vJoy axis. Saves the routing.\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"vid\":{\"type\":\"string\",\"description\":\"Device vendor id as 4 hex digits, e.g. 346E\"},\"pid\":{\"type\":\"string\",\"description\":\"Device product id as 4 hex digits, e.g. 1002\"}},\"required\":[\"vid\",\"pid\"]}},\
{\"name\":\"clear_routing\",\"description\":\"Clear the entire vJoy routing table (saves an empty map).\",\"inputSchema\":{\"type\":\"object\",\"properties\":{},\"required\":[]}}\
]";

/// Dispatch a `tools/call` to the named tool; returns the human/JSON text result.
fn call(line: &str) -> String {
    match str_field(line, "name").unwrap_or_default().as_str() {
        "list_devices" => tool_list_devices(),
        "get_vjoy_routing" => tool_get_routing(),
        "auto_route" => tool_auto_route(line),
        "clear_routing" => tool_clear(),
        other => format!("Error: unknown tool \"{other}\"."),
    }
}

fn tool_list_devices() -> String {
    let items: Vec<String> = crate::input::poll().iter().map(|d| format!(
        "{{\"name\":\"{}\",\"vid\":\"{:04X}\",\"pid\":\"{:04X}\",\"num_axes\":{},\"num_buttons\":{},\"has_pov\":{}}}",
        esc(&d.name), d.vid, d.pid, d.num_axes, d.num_buttons, d.has_pov
    )).collect();
    format!("[{}]", items.join(","))
}

fn tool_get_routing() -> String {
    let items: Vec<String> = VjoyMap::load().mappings.iter().map(|m| format!(
        "{{\"vid\":\"{:04X}\",\"pid\":\"{:04X}\",\"source\":\"{}\",\"target\":\"{}\",\"invert\":{}}}",
        m.vid, m.pid, esc(&m.source.label()), esc(&m.target.label()), m.invert
    )).collect();
    format!("[{}]", items.join(","))
}

fn tool_auto_route(line: &str) -> String {
    let (vs, ps) = match (str_field(line, "vid"), str_field(line, "pid")) {
        (Some(v), Some(p)) => (v, p),
        _ => return "Error: auto_route requires \"vid\" and \"pid\" hex strings.".into(),
    };
    let vid = match u16::from_str_radix(vs.trim(), 16) { Ok(v) => v, Err(_) => return format!("Error: invalid vid hex \"{vs}\".") };
    let pid = match u16::from_str_radix(ps.trim(), 16) { Ok(v) => v, Err(_) => return format!("Error: invalid pid hex \"{ps}\".") };
    let devs = crate::input::poll();
    let dev = match devs.iter().find(|d| d.vid == vid && d.pid == pid) {
        Some(d) => d,
        None => return format!("Error: no connected device {vid:04X}:{pid:04X}."),
    };
    let mut map = VjoyMap::load();
    map.auto_route(dev);
    let nb = map.mappings.iter().filter(|m| m.vid == vid && m.pid == pid && matches!(m.target, Target::Button(_))).count();
    let na = map.mappings.iter().filter(|m| m.vid == vid && m.pid == pid && matches!(m.target, Target::Axis(_))).count();
    match map.save() {
        Ok(()) => format!("Routed {nb} buttons + {na} axes for {vid:04X}:{pid:04X}."),
        Err(e) => format!("Error saving routing: {e}"),
    }
}

fn tool_clear() -> String {
    match VjoyMap::default().save() {
        Ok(()) => "Cleared".into(),
        Err(e) => format!("Error: {e}"),
    }
}

// ---- JSON-RPC envelope helpers --------------------------------------------

fn ok(id: &str, result: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}")
}

fn err(id: &str, code: i32, message: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":{code},\"message\":\"{}\"}}}}", esc(message))
}

// ---- tiny hand-rolled JSON helpers (no serde) -----------------------------

/// First string value of top-level `"key":"..."` (tolerates whitespace after `:`).
fn str_field(s: &str, key: &str) -> Option<String> {
    let k = format!("\"{key}\"");
    let i = s.find(&k)? + k.len();
    let rest = s[i..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// The raw `id` token (verbatim, so a string id keeps its quotes and a number stays
/// a number) to echo back unchanged. None when there is no `id` (a notification).
fn extract_id(line: &str) -> Option<String> {
    let i = line.find("\"id\"")? + 4;
    let after = line[i..].trim_start().strip_prefix(':')?.trim_start();
    if let Some(inner) = after.strip_prefix('"') {
        let end = inner.find('"')?;
        Some(format!("\"{}\"", &inner[..end]))
    } else {
        let end = after.find([',', '}']).unwrap_or(after.len());
        let v = after[..end].trim();
        if v.is_empty() { None } else { Some(v.to_string()) }
    }
}

/// Escape a string for inclusion as a JSON string value.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_string_and_id() {
        let l = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"auto_route","arguments":{"vid":"346E","pid":"1002"}}}"#;
        assert_eq!(str_field(l, "method").as_deref(), Some("tools/call"));
        assert_eq!(str_field(l, "name").as_deref(), Some("auto_route"));
        assert_eq!(str_field(l, "vid").as_deref(), Some("346E"));
        assert_eq!(extract_id(l).as_deref(), Some("7"));
    }

    #[test]
    fn string_id_keeps_quotes_and_notification_has_none() {
        assert_eq!(extract_id(r#"{"id":"abc","method":"x"}"#).as_deref(), Some("\"abc\""));
        assert_eq!(extract_id(r#"{"method":"notifications/initialized"}"#), None);
    }

    #[test]
    fn notification_yields_no_response() {
        assert_eq!(handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None);
    }

    #[test]
    fn unknown_method_is_error() {
        let r = handle(r#"{"jsonrpc":"2.0","id":1,"method":"bogus"}"#).unwrap();
        assert!(r.contains("-32601"), "got {r}");
        assert!(r.contains("\"id\":1"));
    }

    #[test]
    fn initialize_reports_protocol_and_name() {
        let r = handle(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).unwrap();
        assert!(r.contains("2024-11-05"));
        assert!(r.contains("\"name\":\"mw5-remap\""));
    }

    #[test]
    fn esc_handles_quotes_and_controls() {
        assert_eq!(esc("a\"b\\c\n"), "a\\\"b\\\\c\\n");
    }
}
