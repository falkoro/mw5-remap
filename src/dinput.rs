//! DirectInput8 joystick reader — the API the Windows "Game Controllers" panel
//! uses. Unlike winmm (`input.rs`), which is hard-capped at 6 axis slots
//! [X,Y,Z,R(=Rz),U,V] and CANNOT report Rx/Ry, DirectInput exposes the full set
//! X,Y,Z,Rx,Ry,Rz,Slider0,Slider1 — so analog hats that Windows shows as
//! "X-Rotation / Y-Rotation" actually read here. We poll with a CUSTOM data format
//! (no SDK c_dfDIJoystick2 global needed) and GetDeviceState (non-blocking), and
//! keep the device handles alive across frames in a thread-local.
//!
//! Everything is pure FFI (no extra crates). If init fails, `poll()` returns empty
//! and the caller falls back to winmm.

#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

use std::cell::RefCell;
use std::os::raw::c_void;

type HRESULT = i32;
type BOOL = i32;
type HWND = *mut c_void;
type HINSTANCE = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct GUID { data1: u32, data2: u16, data3: u16, data4: [u8; 8] }

// All DirectInput object-type GUIDs share the same tail; only data1 differs.
const fn axis_guid(d1: u32) -> GUID {
    GUID { data1: d1, data2: 0xC9F3, data3: 0x11CF, data4: [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00] }
}
const GUID_XAxis: GUID = axis_guid(0xA36D02E0);
const GUID_YAxis: GUID = axis_guid(0xA36D02E1);
const GUID_ZAxis: GUID = axis_guid(0xA36D02E2);
const GUID_RxAxis: GUID = axis_guid(0xA36D02F4);
const GUID_RyAxis: GUID = axis_guid(0xA36D02F5);
const GUID_RzAxis: GUID = axis_guid(0xA36D02E3);
const GUID_Slider: GUID = axis_guid(0xA36D02E4);
const GUID_POV: GUID = axis_guid(0xA36D02F2);
// IID_IDirectInput8W
const IID_IDirectInput8W: GUID = GUID {
    data1: 0xBF798031, data2: 0x483A, data3: 0x4DA2, data4: [0xAA, 0x99, 0x5D, 0x64, 0xED, 0x36, 0x97, 0x00],
};

const DIRECTINPUT_VERSION: u32 = 0x0800;
const DI8DEVCLASS_GAMECTRL: u32 = 4;
const DIEDFL_ATTACHEDONLY: u32 = 1;
const DIENUM_CONTINUE: BOOL = 1;
const DIDF_ABSAXIS: u32 = 0x00000001;
const DIDFT_AXIS: u32 = 0x00000003;
const DIDFT_POV: u32 = 0x00000010;
const DIDFT_BUTTON: u32 = 0x0000000C;
const DIDFT_ANYINSTANCE: u32 = 0x00FFFF00;
const DIDFT_OPTIONAL: u32 = 0x80000000;
const DIDOI_ASPECTPOSITION: u32 = 0x00000100;
const DIPH_DEVICE: u32 = 0;
const DISCL_BACKGROUND: u32 = 0x00000010;
const DISCL_NONEXCLUSIVE: u32 = 0x00000002;
const DIPROP_RANGE: *const GUID = 4 as *const GUID; // MAKEDIPROP(4)
const HWND_MESSAGE: HWND = -3isize as HWND;
const DIERR_INPUTLOST: HRESULT = 0x8007001E_u32 as i32;
const DIERR_NOTACQUIRED: HRESULT = 0x8007000C_u32 as i32;

#[repr(C)]
struct DIOBJECTDATAFORMAT { pguid: *const GUID, dwOfs: u32, dwType: u32, dwFlags: u32 }
#[repr(C)]
struct DIDATAFORMAT {
    dwSize: u32, dwObjSize: u32, dwFlags: u32, dwDataSize: u32, dwNumObjs: u32,
    rgodf: *const DIOBJECTDATAFORMAT,
}
#[repr(C)]
struct DIPROPHEADER { dwSize: u32, dwHeaderSize: u32, dwObj: u32, dwHow: u32 }
#[repr(C)]
struct DIPROPRANGE { diph: DIPROPHEADER, lMin: i32, lMax: i32 }
#[repr(C)]
struct DIDEVCAPS {
    dwSize: u32, dwFlags: u32, dwDevType: u32, dwAxes: u32, dwButtons: u32, dwPOVs: u32,
    dwFFSamplePeriod: u32, dwFFMinTimeResolution: u32, dwFirmwareRevision: u32,
    dwHardwareRevision: u32, dwFFDriverVersion: u32,
}
#[repr(C)]
struct DIDEVICEINSTANCEW {
    dwSize: u32,
    guidInstance: GUID,
    guidProduct: GUID,
    dwDevType: u32,
    tszInstanceName: [u16; 260],
    tszProductName: [u16; 260],
    guidFFDriver: GUID,
    wUsagePage: u16,
    wUsage: u16,
}

/// Our custom device-state struct (matches the data format below).
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct DiState { axes: [i32; 8], pov: u32, buttons: [u8; 32] }

// ---- COM vtables (only the slots we call need correct signatures; the rest are
// pointer-sized placeholders so the indices line up). ----
#[repr(C)]
struct IDirectInput8Vtbl {
    QueryInterface: *const c_void,
    AddRef: *const c_void,
    Release: unsafe extern "system" fn(*mut c_void) -> u32,
    CreateDevice: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void, *mut c_void) -> HRESULT,
    EnumDevices: unsafe extern "system" fn(*mut c_void, u32, EnumCb, *mut c_void, u32) -> HRESULT,
}
type EnumCb = unsafe extern "system" fn(*const DIDEVICEINSTANCEW, *mut c_void) -> BOOL;

#[repr(C)]
struct IDirectInputDevice8Vtbl {
    QueryInterface: *const c_void,
    AddRef: *const c_void,
    Release: unsafe extern "system" fn(*mut c_void) -> u32,
    GetCapabilities: unsafe extern "system" fn(*mut c_void, *mut DIDEVCAPS) -> HRESULT,
    EnumObjects: *const c_void,
    GetProperty: *const c_void,
    SetProperty: unsafe extern "system" fn(*mut c_void, *const GUID, *const DIPROPHEADER) -> HRESULT,
    Acquire: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    Unacquire: *const c_void,
    GetDeviceState: unsafe extern "system" fn(*mut c_void, u32, *mut c_void) -> HRESULT,
    GetDeviceData: *const c_void,
    SetDataFormat: unsafe extern "system" fn(*mut c_void, *const DIDATAFORMAT) -> HRESULT,
    SetEventNotification: *const c_void,
    SetCooperativeLevel: unsafe extern "system" fn(*mut c_void, HWND, u32) -> HRESULT,
    GetObjectInfo: *const c_void,
    GetDeviceInfo: *const c_void,
    RunControlPanel: *const c_void,
    Initialize: *const c_void,
    CreateEffect: *const c_void,
    EnumEffects: *const c_void,
    GetEffectInfo: *const c_void,
    GetForceFeedbackState: *const c_void,
    SendForceFeedbackCommand: *const c_void,
    EnumCreatedEffectObjects: *const c_void,
    Escape: *const c_void,
    Poll: unsafe extern "system" fn(*mut c_void) -> HRESULT,
}

#[link(name = "dinput8")]
extern "system" {
    fn DirectInput8Create(hinst: HINSTANCE, version: u32, riid: *const GUID, ppv: *mut *mut c_void, outer: *mut c_void) -> HRESULT;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
}
#[link(name = "user32")]
extern "system" {
    fn CreateWindowExW(ex: u32, class: *const u16, name: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: HWND, menu: *mut c_void, inst: HINSTANCE, param: *mut c_void) -> HWND;
    fn GetDesktopWindow() -> HWND;
}

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
fn wstr(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

unsafe fn vtbl<T>(obj: *mut c_void) -> *const T { *(obj as *const *const T) }

/// The data format object table — 8 axes, one POV, 32 buttons. Built and leaked so
/// the pointer stays valid for the whole process (DirectInput keeps a reference).
/// Only called during one-time init, so the leak happens once. Not cached in a
/// static because DIDATAFORMAT holds raw pointers (not Sync).
fn data_format() -> &'static DIDATAFORMAT {
    let axis_guids: [&GUID; 8] = [
        &GUID_XAxis, &GUID_YAxis, &GUID_ZAxis, &GUID_RxAxis, &GUID_RyAxis, &GUID_RzAxis, &GUID_Slider, &GUID_Slider,
    ];
    let mut objs: Vec<DIOBJECTDATAFORMAT> = Vec::with_capacity(41);
    for (i, g) in axis_guids.iter().enumerate() {
        objs.push(DIOBJECTDATAFORMAT {
            pguid: *g as *const GUID,
            dwOfs: (i * 4) as u32, // axes[i]
            dwType: DIDFT_AXIS | DIDFT_ANYINSTANCE | DIDFT_OPTIONAL,
            dwFlags: DIDOI_ASPECTPOSITION,
        });
    }
    objs.push(DIOBJECTDATAFORMAT { pguid: &GUID_POV as *const GUID, dwOfs: 32, dwType: DIDFT_POV | DIDFT_ANYINSTANCE | DIDFT_OPTIONAL, dwFlags: 0 });
    for b in 0..32 {
        objs.push(DIOBJECTDATAFORMAT { pguid: std::ptr::null(), dwOfs: 36 + b, dwType: DIDFT_BUTTON | DIDFT_ANYINSTANCE | DIDFT_OPTIONAL, dwFlags: 0 });
    }
    let objs: &'static [DIOBJECTDATAFORMAT] = Box::leak(objs.into_boxed_slice());
    let fmt = DIDATAFORMAT {
        dwSize: std::mem::size_of::<DIDATAFORMAT>() as u32,
        dwObjSize: std::mem::size_of::<DIOBJECTDATAFORMAT>() as u32,
        dwFlags: DIDF_ABSAXIS,
        dwDataSize: std::mem::size_of::<DiState>() as u32, // 68, multiple of 4
        dwNumObjs: objs.len() as u32,
        rgodf: objs.as_ptr(),
    };
    Box::leak(Box::new(fmt))
}

struct DiDevice {
    dev: *mut c_void,
    vid: u16,
    pid: u16,
    name: String,
    num_axes: u32,
    num_buttons: u32,
    has_pov: bool,
}

struct DiContext {
    di: *mut c_void,
    hwnd: HWND,
    devices: Vec<DiDevice>,
}

thread_local! {
    static CTX: RefCell<Option<DiContext>> = const { RefCell::new(None) };
}

/// Enum callback: collect (guidInstance, vid, pid, name) into the Vec at `pv`.
unsafe extern "system" fn enum_cb(inst: *const DIDEVICEINSTANCEW, pv: *mut c_void) -> BOOL {
    let list = &mut *(pv as *mut Vec<(GUID, u16, u16, String)>);
    let di = &*inst;
    let vid = (di.guidProduct.data1 & 0xFFFF) as u16;
    let pid = ((di.guidProduct.data1 >> 16) & 0xFFFF) as u16;
    list.push((di.guidInstance, vid, pid, wstr(&di.tszProductName)));
    DIENUM_CONTINUE
}

unsafe fn init() -> Option<DiContext> {
    let hinst = GetModuleHandleW(std::ptr::null());
    let mut di: *mut c_void = std::ptr::null_mut();
    if DirectInput8Create(hinst, DIRECTINPUT_VERSION, &IID_IDirectInput8W, &mut di, std::ptr::null_mut()) < 0 || di.is_null() {
        return None;
    }
    // a message-only window for SetCooperativeLevel (background, non-exclusive).
    let mut hwnd = CreateWindowExW(0, wide("STATIC").as_ptr(), std::ptr::null(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), hinst, std::ptr::null_mut());
    if hwnd.is_null() { hwnd = GetDesktopWindow(); }

    let v = vtbl::<IDirectInput8Vtbl>(di);
    let mut found: Vec<(GUID, u16, u16, String)> = Vec::new();
    let rc = ((*v).EnumDevices)(di, DI8DEVCLASS_GAMECTRL, enum_cb, &mut found as *mut _ as *mut c_void, DIEDFL_ATTACHEDONLY);
    if rc < 0 { ((*v).Release)(di); return None; }

    let fmt = data_format();
    let mut devices = Vec::new();
    for (guid, vid, pid, name) in found {
        let mut dev: *mut c_void = std::ptr::null_mut();
        if ((*v).CreateDevice)(di, &guid, &mut dev, std::ptr::null_mut()) < 0 || dev.is_null() { continue; }
        let dv = vtbl::<IDirectInputDevice8Vtbl>(dev);
        if ((*dv).SetDataFormat)(dev, fmt) < 0 { ((*dv).Release)(dev); continue; }
        let _ = ((*dv).SetCooperativeLevel)(dev, hwnd, DISCL_BACKGROUND | DISCL_NONEXCLUSIVE);
        // normalise every axis to 0..65535 so values match winmm semantics.
        let mut range = DIPROPRANGE {
            diph: DIPROPHEADER { dwSize: std::mem::size_of::<DIPROPRANGE>() as u32, dwHeaderSize: std::mem::size_of::<DIPROPHEADER>() as u32, dwObj: 0, dwHow: DIPH_DEVICE },
            lMin: 0, lMax: 65535,
        };
        let _ = ((*dv).SetProperty)(dev, DIPROP_RANGE, &mut range.diph);
        let mut caps: DIDEVCAPS = std::mem::zeroed();
        caps.dwSize = std::mem::size_of::<DIDEVCAPS>() as u32;
        let _ = ((*dv).GetCapabilities)(dev, &mut caps);
        let _ = ((*dv).Acquire)(dev);
        devices.push(DiDevice { dev, vid, pid, name, num_axes: caps.dwAxes, num_buttons: caps.dwButtons, has_pov: caps.dwPOVs > 0 });
    }
    if devices.is_empty() { ((*v).Release)(di); return None; }
    Some(DiContext { di, hwnd, devices })
}

/// One controller as DirectInput sees it. `axes` is [X,Y,Z,Rx,Ry,Rz,S0,S1] 0..65535.
/// `present[i]` is true only for axes the device actually reports (detected by a
/// sentinel pre-fill — DIDFT_OPTIONAL means absent axes aren't written).
pub struct DiAxes {
    pub vid: u16,
    pub pid: u16,
    pub name: String,
    pub axes: [u32; 8],
    pub present: [bool; 8],
    pub buttons: u32,
    pub pov: u32,
    pub num_axes: u32,
    pub num_buttons: u32,
    pub has_pov: bool,
}

/// Poll every DirectInput game controller. Returns empty if DirectInput is
/// unavailable (caller then falls back to winmm). Inits lazily and reuses handles.
pub fn poll() -> Vec<DiAxes> {
    CTX.with(|cell| {
        let mut ctx = cell.borrow_mut();
        if ctx.is_none() {
            *ctx = unsafe { init() };
        }
        let ctx = match ctx.as_mut() { Some(c) => c, None => return Vec::new() };
        let mut out = Vec::new();
        for d in &ctx.devices {
            unsafe {
                let dv = vtbl::<IDirectInputDevice8Vtbl>(d.dev);
                // Pre-fill axes with a sentinel: DIDFT_OPTIONAL means axes the device
                // doesn't have are left untouched by GetDeviceState, so anything still
                // at the sentinel afterwards is an axis Windows does NOT detect.
                let mut st = DiState { axes: [i32::MIN; 8], pov: 0, buttons: [0; 32] };
                let _ = ((*dv).Poll)(d.dev);
                let rc = ((*dv).GetDeviceState)(d.dev, std::mem::size_of::<DiState>() as u32, &mut st as *mut _ as *mut c_void);
                if rc == DIERR_INPUTLOST || rc == DIERR_NOTACQUIRED {
                    let _ = ((*dv).Acquire)(d.dev);
                    let _ = ((*dv).Poll)(d.dev);
                    st.axes = [i32::MIN; 8];
                    let _ = ((*dv).GetDeviceState)(d.dev, std::mem::size_of::<DiState>() as u32, &mut st as *mut _ as *mut c_void);
                }
                let mut axes = [0u32; 8];
                let mut present = [false; 8];
                for i in 0..8 {
                    if st.axes[i] != i32::MIN {
                        present[i] = true;
                        axes[i] = st.axes[i].clamp(0, 65535) as u32;
                    }
                }
                let mut buttons = 0u32;
                for b in 0..32 { if st.buttons[b] & 0x80 != 0 { buttons |= 1 << b; } }
                out.push(DiAxes {
                    vid: d.vid, pid: d.pid, name: d.name.clone(),
                    axes, present, buttons, pov: st.pov,
                    num_axes: d.num_axes, num_buttons: d.num_buttons, has_pov: d.has_pov,
                });
            }
        }
        out
    })
}
