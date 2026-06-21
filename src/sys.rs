//! Small Win32 helpers: admin check, self-elevation, open a URI, and check whether
//! a process is running (used to refuse writing the config while MW5 is open).

use std::os::raw::c_void;
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut c_void,
        lp_operation: *const u16,
        lp_file: *const u16,
        lp_parameters: *const u16,
        lp_directory: *const u16,
        n_show_cmd: i32,
    ) -> isize;
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: *mut c_void, desired: u32, token: *mut *mut c_void) -> i32;
    fn GetTokenInformation(
        token: *mut c_void,
        class: i32,
        info: *mut c_void,
        len: u32,
        ret_len: *mut u32,
    ) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn CloseHandle(h: *mut c_void) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// True if the current process has an elevated (admin) token.
pub fn is_elevated() -> bool {
    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_ELEVATION: i32 = 20; // TokenElevation
    unsafe {
        let mut token: *mut c_void = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation: u32 = 0;
        let mut ret_len: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TOKEN_ELEVATION,
            &mut elevation as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation != 0
    }
}

/// Relaunch this exe with the "runas" verb (UAC elevation). Returns true if the
/// elevated process was launched (caller should then exit).
pub fn relaunch_elevated() -> bool {
    let exe = match std::env::current_exe() {
        Ok(p) => p, Err(_) => return false,
    };
    let verb = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let r = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1, // SW_SHOWNORMAL
        )
    };
    r > 32
}

/// Open a URI / file with the shell (used for steam:// launch).
pub fn open_uri(uri: &str) -> bool {
    let file = wide(uri);
    let r = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            std::ptr::null(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    r > 32
}

/// Whether any of the given process base-names (without .exe) is running.
pub fn any_process_running(names: &[String]) -> bool {
    if names.is_empty() {
        return false;
    }
    let out = std::process::Command::new("tasklist")
        .args(["/fo", "csv", "/nh"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    if let Ok(o) = out {
        let text = String::from_utf8_lossy(&o.stdout).to_lowercase();
        names.iter().any(|n| {
            let exe = format!("{}.exe", n.to_lowercase());
            text.contains(&exe)
        })
    } else {
        false
    }
}
