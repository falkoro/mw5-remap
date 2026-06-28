//! Feed a vJoy virtual-joystick axis at runtime. This lets the app COMBINE two
//! physical toe pedals into ONE bipolar throttle axis (centre = stop, right toe =
//! forward, left toe = reverse) on vJoy device 1 — the clean, game-agnostic fix for
//! "two unipolar toes -> one bipolar axis" (no Joystick Gremlin needed).
//!
//! `vJoyInterface.dll` is loaded DYNAMICALLY (LoadLibrary/GetProcAddress) from the
//! vJoy install dir, so there is NO compile-time dependency: if vJoy isn't installed
//! the feeder simply stays unavailable.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::os::raw::c_void;

type HModule = *mut c_void;
type FarProc = *mut c_void;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> HModule;
    fn GetProcAddress(module: HModule, name: *const u8) -> FarProc;
}

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }

/// HID usage ids for the vJoy axes.
pub const HID_X: u32 = 0x30;
pub const HID_Y: u32 = 0x31;
pub const HID_Z: u32 = 0x32;
pub const HID_RX: u32 = 0x33;
pub const HID_RY: u32 = 0x34;
pub const HID_RZ: u32 = 0x35;
/// vJoy axis values run 1..=32768; centre is 16384.
pub const VJOY_MAX: i32 = 32768;
pub const VJOY_CENTRE: i32 = 16384;

/// Scale a winmm/DirectInput axis value (0..=65535) to the vJoy range (1..=32768).
pub fn scale(v: u32) -> i32 { (v as i32 * (VJOY_MAX - 1) / 65535 + 1).clamp(1, VJOY_MAX) }

// vJoyInterface uses the C ABI (single calling convention on x64).
type FnEnabled = unsafe extern "C" fn() -> i32;
type FnStatus = unsafe extern "C" fn(u32) -> i32; // 0=Own,1=Free,2=Busy,3=Miss,4=Unknown
type FnAcquire = unsafe extern "C" fn(u32) -> i32;
type FnSetAxis = unsafe extern "C" fn(i32, u32, u32) -> i32;
type FnSetBtn = unsafe extern "C" fn(i32, u32, u8) -> i32; // (value, rID, nBtn 1-based)

struct Api {
    enabled: FnEnabled,
    status: FnStatus,
    acquire: FnAcquire,
    set_axis: FnSetAxis,
    set_btn: FnSetBtn,
}

fn load_api() -> Option<Api> {
    let paths = [
        "C:\\Program Files\\vJoy\\x64\\vJoyInterface.dll",
        "C:\\Program Files\\vJoy\\x86\\vJoyInterface.dll",
        "vJoyInterface.dll",
    ];
    unsafe {
        let mut h: HModule = std::ptr::null_mut();
        for p in paths {
            h = LoadLibraryW(wide(p).as_ptr());
            if !h.is_null() { break; }
        }
        if h.is_null() { return None; }
        let get = |name: &str| -> FarProc {
            let c = std::ffi::CString::new(name).unwrap();
            GetProcAddress(h, c.as_ptr() as *const u8)
        };
        let (e, s, a, sa) = (get("vJoyEnabled"), get("GetVJDStatus"), get("AcquireVJD"), get("SetAxis"));
        let sb = get("SetBtn");
        if e.is_null() || s.is_null() || a.is_null() || sa.is_null() || sb.is_null() { return None; }
        Some(Api {
            enabled: std::mem::transmute::<FarProc, FnEnabled>(e),
            status: std::mem::transmute::<FarProc, FnStatus>(s),
            acquire: std::mem::transmute::<FarProc, FnAcquire>(a),
            set_axis: std::mem::transmute::<FarProc, FnSetAxis>(sa),
            set_btn: std::mem::transmute::<FarProc, FnSetBtn>(sb),
        })
    }
}

/// Set by the GUI toggle; read by `write_hotas_mappings` so the vJoy throttle block
/// (and the MRP skip) only happen when we're ACTUALLY feeding vJoy.
static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_active(on: bool) { ACTIVE.store(on, std::sync::atomic::Ordering::Relaxed); }
pub fn is_active() -> bool { ACTIVE.load(std::sync::atomic::Ordering::Relaxed) }

struct Feeder { api: Api, rid: u32, acquired: bool }

thread_local! {
    static FEEDER: RefCell<Option<Feeder>> = const { RefCell::new(None) };
}

fn with_feeder<T>(f: impl FnOnce(&mut Feeder) -> T, miss: T) -> T {
    FEEDER.with(|c| {
        let mut g = c.borrow_mut();
        if g.is_none() {
            match load_api() {
                Some(api) => *g = Some(Feeder { api, rid: 1, acquired: false }),
                None => return miss,
            }
        }
        f(g.as_mut().unwrap())
    })
}

/// True if the vJoy driver is installed AND enabled (DLL loads + vJoyEnabled()).
pub fn available() -> bool {
    with_feeder(|fd| unsafe { (fd.api.enabled)() != 0 }, false)
}

/// Raw GetVJDStatus(1): 0=Own, 1=Free, 2=Busy, 3=Miss, 4=Unknown (-1 = no vJoy).
pub fn status() -> i32 {
    with_feeder(|fd| unsafe { (fd.api.status)(fd.rid) }, -1)
}

/// Acquire vJoy device 1 if needed and push `value` (1..=32768) onto `usage` (an
/// HID axis id). Returns false if vJoy is unavailable or can't be acquired.
pub fn feed(usage: u32, value: i32) -> bool {
    with_feeder(|fd| unsafe {
        if (fd.api.enabled)() == 0 { return false; }
        if !fd.acquired {
            let st = (fd.api.status)(fd.rid); // 0=Own, 1=Free are usable
            if st == 0 || st == 1 { fd.acquired = (fd.api.acquire)(fd.rid) != 0; }
            if !fd.acquired { return false; }
        }
        (fd.api.set_axis)(value.clamp(1, VJOY_MAX), fd.rid, usage) != 0
    }, false)
}

/// Set vJoy device-1 button `btn` (1-based) pressed/released. Acquires if needed.
pub fn feed_button(btn: u8, pressed: bool) -> bool {
    with_feeder(|fd| unsafe {
        if (fd.api.enabled)() == 0 { return false; }
        if !fd.acquired {
            let st = (fd.api.status)(fd.rid);
            if st == 0 || st == 1 { fd.acquired = (fd.api.acquire)(fd.rid) != 0; }
            if !fd.acquired { return false; }
        }
        (fd.api.set_btn)(if pressed { 1 } else { 0 }, fd.rid, btn) != 0
    }, false)
}

/// Convenience: push a combined throttle value onto vJoy axis X.
pub fn feed_throttle(value: i32) -> bool { feed(HID_X, value) }

/// Combine two unipolar toe values (each 0..=65535, rest at 0) into a centred vJoy
/// throttle value: rest -> centre, right toe -> forward (up), left toe -> reverse.
pub fn combine_toes(right: u32, left: u32) -> i32 {
    let delta = right as i32 - left as i32; // -65535..=65535
    VJOY_CENTRE + delta * (VJOY_CENTRE - 1) / 65535
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine_rest_is_centre() {
        assert_eq!(combine_toes(0, 0), VJOY_CENTRE);
    }

    #[test]
    fn combine_directions() {
        assert!(combine_toes(65535, 0) > VJOY_CENTRE, "right toe should push forward");
        assert!(combine_toes(0, 65535) < VJOY_CENTRE, "left toe should push reverse");
        // both toes together cancel back to centre
        assert_eq!(combine_toes(65535, 65535), VJOY_CENTRE);
    }

    #[test]
    fn combine_stays_in_range() {
        for (r, l) in [(0, 0), (65535, 0), (0, 65535), (65535, 65535), (12345, 54321)] {
            let v = combine_toes(r, l).clamp(1, VJOY_MAX);
            assert!((1..=VJOY_MAX).contains(&v));
        }
    }

    #[test]
    fn scale_maps_full_range() {
        assert_eq!(scale(0), 1);
        assert!(scale(65535) >= VJOY_MAX - 1);
        assert!((scale(32767) - VJOY_CENTRE).abs() <= 2, "mid should be ~centre");
    }
}
