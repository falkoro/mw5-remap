//! Auto-update via GitHub Releases. Uses the native WinHTTP API (no TLS crates —
//! safer on this gnullvm toolchain) to read the latest release JSON, compare
//! versions, download the new exe, swap it in, and relaunch.
//!
//! Set OWNER/REPO below to your GitHub repo; publish a Release whose tag is the new
//! version (e.g. v0.1.1) with MW5-Remap.exe attached as an asset.

use std::os::raw::c_void;

pub const OWNER: &str = "REPLACE_ME";
pub const REPO: &str = "mw5-remap";

type HInternet = *mut c_void;
const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;

#[link(name = "winhttp")]
extern "system" {
    fn WinHttpOpen(agent: *const u16, access: u32, proxy: *const u16, bypass: *const u16, flags: u32) -> HInternet;
    fn WinHttpConnect(session: HInternet, server: *const u16, port: u16, reserved: u32) -> HInternet;
    fn WinHttpOpenRequest(conn: HInternet, verb: *const u16, object: *const u16, version: *const u16, referrer: *const u16, accept: *const *const u16, flags: u32) -> HInternet;
    fn WinHttpSendRequest(req: HInternet, headers: *const u16, headers_len: u32, optional: *const c_void, optional_len: u32, total_len: u32, context: usize) -> i32;
    fn WinHttpReceiveResponse(req: HInternet, reserved: *mut c_void) -> i32;
    fn WinHttpQueryDataAvailable(req: HInternet, avail: *mut u32) -> i32;
    fn WinHttpReadData(req: HInternet, buf: *mut c_void, to_read: u32, read: *mut u32) -> i32;
    fn WinHttpCloseHandle(h: HInternet) -> i32;
}

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }

/// HTTPS GET via WinHTTP (follows redirects, validates certs). Returns the body.
fn https_get(host: &str, path: &str, extra_headers: &str) -> Option<Vec<u8>> {
    unsafe {
        let session = WinHttpOpen(wide("MW5-Remap").as_ptr(), 0, std::ptr::null(), std::ptr::null(), 0);
        if session.is_null() { return None; }
        let conn = WinHttpConnect(session, wide(host).as_ptr(), 443, 0);
        if conn.is_null() { WinHttpCloseHandle(session); return None; }
        let req = WinHttpOpenRequest(conn, wide("GET").as_ptr(), wide(path).as_ptr(),
            std::ptr::null(), std::ptr::null(), std::ptr::null(), WINHTTP_FLAG_SECURE);
        if req.is_null() { WinHttpCloseHandle(conn); WinHttpCloseHandle(session); return None; }

        let hdr_w = wide(extra_headers);
        let (hptr, hlen) = if extra_headers.is_empty() {
            (std::ptr::null(), 0)
        } else {
            (hdr_w.as_ptr(), extra_headers.encode_utf16().count() as u32)
        };
        let mut body = None;
        if WinHttpSendRequest(req, hptr, hlen, std::ptr::null(), 0, 0, 0) != 0
            && WinHttpReceiveResponse(req, std::ptr::null_mut()) != 0
        {
            let mut buf = Vec::new();
            loop {
                let mut avail: u32 = 0;
                if WinHttpQueryDataAvailable(req, &mut avail) == 0 || avail == 0 { break; }
                let mut chunk = vec![0u8; avail as usize];
                let mut read: u32 = 0;
                if WinHttpReadData(req, chunk.as_mut_ptr() as *mut c_void, avail, &mut read) == 0 { break; }
                chunk.truncate(read as usize);
                buf.extend_from_slice(&chunk);
            }
            body = Some(buf);
        }
        WinHttpCloseHandle(req);
        WinHttpCloseHandle(conn);
        WinHttpCloseHandle(session);
        body
    }
}

fn json_str(s: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let i = s.find(&needle)? + needle.len();
    let rest = &s[i..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// First release asset URL ending in .exe.
fn find_exe_asset(s: &str) -> Option<String> {
    let key = "\"browser_download_url\":\"";
    let mut from = 0;
    while let Some(rel) = s[from..].find(key) {
        let i = from + rel + key.len();
        let rest = &s[i..];
        if let Some(end) = rest.find('"') {
            let url = &rest[..end];
            if url.to_lowercase().ends_with(".exe") { return Some(url.to_string()); }
            from = i + end;
        } else { break; }
    }
    None
}

fn parse_ver(v: &str) -> (u32, u32, u32) {
    let mut it = v.trim().trim_start_matches('v').split('.').map(|x| x.trim().parse::<u32>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

pub fn current_version() -> &'static str { env!("CARGO_PKG_VERSION") }
pub fn is_newer(latest: &str) -> bool { parse_ver(latest) > parse_ver(current_version()) }

/// Query GitHub for the latest release: returns (version, exe_download_url).
pub fn latest() -> Option<(String, String)> {
    if OWNER == "REPLACE_ME" { return None; }
    let path = format!("/repos/{OWNER}/{REPO}/releases/latest");
    let body = https_get("api.github.com", &path, "Accept: application/vnd.github+json\r\n")?;
    let s = String::from_utf8_lossy(&body);
    let tag = json_str(&s, "tag_name")?;
    let url = find_exe_asset(&s)?;
    Some((tag.trim_start_matches('v').to_string(), url))
}

fn split_url(url: &str) -> Option<(String, String)> {
    let u = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let slash = u.find('/')?;
    Some((u[..slash].to_string(), u[slash..].to_string()))
}

/// Download the new exe and swap it in, then relaunch. Never returns on success.
pub fn apply(url: &str) -> Result<(), String> {
    let (host, path) = split_url(url).ok_or("bad asset url")?;
    let bytes = https_get(&host, &path, "").ok_or("download failed")?;
    if bytes.len() < 200_000 {
        return Err(format!("downloaded file too small ({} bytes) — aborting", bytes.len()));
    }
    let cur = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = cur.parent().ok_or("no exe dir")?;
    let newp = dir.join("MW5-Remap.new.exe");
    let oldp = dir.join("MW5-Remap.old.exe");
    std::fs::write(&newp, &bytes).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&oldp);
    std::fs::rename(&cur, &oldp).map_err(|e| format!("rename current exe: {e}"))?;
    std::fs::rename(&newp, &cur).map_err(|e| format!("install new exe: {e}"))?;
    std::process::Command::new(&cur).spawn().map_err(|e| e.to_string())?;
    std::process::exit(0);
}

/// Remove the leftover .old.exe from a previous update (call on startup).
pub fn cleanup() {
    if let Ok(cur) = std::env::current_exe() {
        if let Some(dir) = cur.parent() {
            let _ = std::fs::remove_file(dir.join("MW5-Remap.old.exe"));
        }
    }
}
