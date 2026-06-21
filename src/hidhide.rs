//! Cloak conflicting sticks (VirPil/VKB) from games via HidHide (Nefarius).
//! Built-in PnP disable fails on these HID sticks, so we drive HidHideCLI.exe.
//! NOTE: HidHideCLI writes its output to STDERR, and after install Windows needs
//! a reboot before the filter driver answers (else "Access is denied").

use std::os::windows::process::CommandExt;
use std::path::PathBuf;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn cli_path() -> Option<PathBuf> {
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:/Program Files".into());
    let cands = [
        format!("{}/Nefarius Software Solutions/HidHide/x64/HidHideCLI.exe", pf),
        format!("{}/Nefarius Software Solutions/HidHide/HidHideCLI.exe", pf),
    ];
    cands.iter().map(PathBuf::from).find(|p| p.exists())
}

pub fn installed() -> bool {
    cli_path().is_some()
}

/// Run HidHideCLI and return combined stdout+stderr (it mostly uses stderr).
fn run(args: &[&str]) -> Result<String, String> {
    let cli = cli_path().ok_or("HidHide is not installed.")?;
    let out = std::process::Command::new(cli)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok(s)
}

/// Is the driver answering (not pending a reboot)?
pub fn driver_ready() -> bool {
    match run(&["--cloak-state"]) {
        Ok(s) => !s.contains("Access is denied") && !s.contains("FilterDriverProxy"),
        Err(_) => false,
    }
}

/// Device instance paths of gaming devices whose VID is in `vids`. Parsed from
/// `--dev-gaming` JSON by scanning for "deviceInstancePath" values (no serde dep).
pub fn hide_targets(vids: &[String]) -> Vec<String> {
    let json = match run(&["--dev-gaming"]) {
        Ok(s) => s, Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    let key = "\"deviceInstancePath\":";
    let mut rest = json.as_str();
    while let Some(i) = rest.find(key) {
        rest = &rest[i + key.len()..];
        if let Some(q1) = rest.find('"') {
            let after = &rest[q1 + 1..];
            if let Some(q2) = after.find('"') {
                let path = &after[..q2];
                let up = path.to_uppercase();
                if vids.iter().any(|v| up.contains(&format!("VID_{}", v.to_uppercase()))) {
                    out.push(path.to_string());
                }
                rest = &after[q2 + 1..];
                continue;
            }
        }
        break;
    }
    out.sort();
    out.dedup();
    out
}

pub struct HideResult {
    pub hidden: Vec<String>,
    pub message: String,
}

/// Cloak the conflict devices. Returns paths hidden (caller persists them so we
/// can restore on close / crash recovery).
pub fn hide(vids: &[String]) -> Result<HideResult, String> {
    if !installed() {
        return Err("HidHide isn't installed. Get it free from github.com/nefarius/HidHide, \
                    then click Hide again. (Your VirPil enumerates last, so MW5 likely ignores it anyway.)".into());
    }
    if !driver_ready() {
        return Err("HidHide is installed but needs a Windows RESTART to activate (driver pending reboot). \
                    Reboot, then click Hide again.".into());
    }
    let targets = hide_targets(vids);
    if targets.is_empty() {
        return Ok(HideResult { hidden: Vec::new(), message: "No conflicting devices present to hide.".into() });
    }
    let _ = run(&["--cloak-on"]);
    for t in &targets {
        let _ = run(&["--dev-hide", t]);
    }
    Ok(HideResult {
        message: format!("Hidden {} device(s) from the game via HidHide.", targets.len()),
        hidden: targets,
    })
}

/// Un-hide the given paths and turn cloaking off.
pub fn restore(paths: &[String]) -> Result<usize, String> {
    if !installed() {
        return Ok(0);
    }
    let mut n = 0;
    for p in paths {
        if run(&["--dev-unhide", p]).is_ok() {
            n += 1;
        }
    }
    let _ = run(&["--cloak-off"]);
    Ok(n)
}
