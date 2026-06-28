//! Live joystick polling via winmm (joyGetNumDevs / joyGetDevCapsW / joyGetPosEx).
//! Produces raw per-device state; translating a press into a *game* token is the
//! game provider's job (token format differs per game). Mirrors the proven
//! PowerShell approach (same structs, JOY_RETURNALL flags, VID/PID from wMid/wPid).

use std::os::raw::c_void;

#[repr(C)]
struct JoyCapsW {
    w_mid: u16,
    w_pid: u16,
    sz_pname: [u16; 32],
    w_xmin: u32, w_xmax: u32,
    w_ymin: u32, w_ymax: u32,
    w_zmin: u32, w_zmax: u32,
    w_num_buttons: u32,
    w_period_min: u32, w_period_max: u32,
    w_rmin: u32, w_rmax: u32,
    w_umin: u32, w_umax: u32,
    w_vmin: u32, w_vmax: u32,
    w_caps: u32,
    w_max_axes: u32,
    w_num_axes: u32,
    w_max_buttons: u32,
    sz_reg_key: [u16; 32],
    sz_oem_vxd: [u16; 260],
}

#[repr(C)]
struct JoyInfoEx {
    dw_size: u32,
    dw_flags: u32,
    dw_xpos: u32, dw_ypos: u32, dw_zpos: u32,
    dw_rpos: u32, dw_upos: u32, dw_vpos: u32,
    dw_buttons: u32,
    dw_button_number: u32,
    dw_pov: u32,
    dw_reserved1: u32, dw_reserved2: u32,
}

const JOY_RETURNALL: u32 = 0x0000_00FF;
const JOYERR_NOERROR: u32 = 0;
const JOY_POVCENTERED: u32 = 0xFFFF;
const JOYCAPS_HASZ: u32 = 0x0000_0001;
const JOYCAPS_HASR: u32 = 0x0000_0002;
const JOYCAPS_HASU: u32 = 0x0000_0004;
const JOYCAPS_HASV: u32 = 0x0000_0008;
const JOYCAPS_HASPOV: u32 = 0x0000_0010; // device actually has a POV hat

#[link(name = "winmm")]
extern "system" {
    fn joyGetNumDevs() -> u32;
    fn joyGetDevCapsW(u_joy_id: usize, pjc: *mut JoyCapsW, cbjc: u32) -> u32;
    fn joyGetPosEx(u_joy_id: u32, pji: *mut JoyInfoEx) -> u32;
}

const HKEY_LOCAL_MACHINE: isize = -2147483646; // 0x80000002
const RRF_RT_REG_SZ: u32 = 0x0000_0002;

#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(
        hkey: isize,
        lp_sub_key: *const u16,
        lp_value: *const u16,
        dw_flags: u32,
        pdw_type: *mut u32,
        pv_data: *mut c_void,
        pcb_data: *mut u32,
    ) -> i32;
}

/// One physical controller. `axes` is 8 slots: when read via DirectInput (preferred)
/// they are [X,Y,Z,Rx,Ry,Rz,Slider0,Slider1]; the winmm fallback fills [X,Y,Z,_,_,Rz,U,V]
/// (no Rx/Ry — that's winmm's limitation). All 0..65535, centre ~32767.
#[derive(Clone, Debug)]
pub struct Device {
    pub id: u32,
    pub vid: u16,
    pub pid: u16,
    pub name: String,
    pub num_axes: u32,
    pub num_buttons: u32,
    pub axes: [u32; 8],
    pub present: [bool; 8], // which of the 8 axis slots the device actually reports
    pub buttons: u32, // bitmask, bit b => button b+1 pressed
    pub pov: u32,     // centidegrees, or 0xFFFF when centered
    pub has_pov: bool, // false => ignore pov (device has no hat; some report 0)
}

impl Device {
    /// Pressed buttons as 1-based numbers.
    pub fn pressed_buttons(&self) -> Vec<u32> {
        (0..self.num_buttons.min(32))
            .filter(|&b| self.buttons & (1u32 << b) != 0)
            .map(|b| b + 1)
            .collect()
    }
    /// 8-way POV octant 1..8 (1=up,3=right,5=down,7=left), or None when centered or
    /// when the device has no real hat (some hat-less sticks report pov=0).
    pub fn pov_octant(&self) -> Option<u32> {
        if !self.has_pov || self.pov > 36000 || self.pov == JOY_POVCENTERED {
            None
        } else {
            Some(((self.pov as f32 / 4500.0).round() as u32 % 8) + 1)
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn pcwstr_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// DirectInput friendly name from the OEM registry (nicer than szPname). Returns
/// None on any failure so the caller can fall back to the product name.
fn oem_name(vidpid: &str) -> Option<String> {
    let subkey = wide(&format!(
        "SYSTEM\\CurrentControlSet\\Control\\MediaProperties\\PrivateProperties\\Joystick\\OEM\\{}",
        vidpid
    ));
    let value = wide("OEMName");
    let mut buf = [0u16; 256];
    let mut len = (buf.len() * 2) as u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut c_void,
            &mut len,
        )
    };
    if rc == 0 {
        let s = pcwstr_to_string(&buf);
        if !s.is_empty() { return Some(s); }
    }
    None
}

/// Poll every connected joystick. Cheap enough to call each UI frame. Prefers
/// DirectInput (exposes Rx/Ry/sliders — what the Windows Game Controllers panel
/// shows), and falls back to winmm only if DirectInput yields nothing.
pub fn poll() -> Vec<Device> {
    let di = crate::dinput::poll();
    if !di.is_empty() {
        return di.into_iter().enumerate().map(|(i, d)| Device {
            id: i as u32,
            vid: d.vid,
            pid: d.pid,
            name: d.name,
            num_axes: d.num_axes,
            num_buttons: d.num_buttons,
            axes: d.axes, // [X,Y,Z,Rx,Ry,Rz,S0,S1]
            present: d.present,
            buttons: d.buttons,
            pov: d.pov,
            has_pov: d.has_pov,
        }).collect();
    }
    poll_winmm()
}

/// winmm fallback. Maps the 6 winmm slots into the 8-axis layout: X,Y,Z keep their
/// index, winmm R(=Rz)->5, and U/V (winmm's 5th/6th) land in the slider slots 6/7.
/// Rx/Ry (3/4) stay 0 — winmm can't see them; that's why we prefer DirectInput.
fn poll_winmm() -> Vec<Device> {
    let mut out = Vec::new();
    let n = unsafe { joyGetNumDevs() };
    for id in 0..n {
        let mut caps: JoyCapsW = unsafe { std::mem::zeroed() };
        let sz = std::mem::size_of::<JoyCapsW>() as u32;
        if unsafe { joyGetDevCapsW(id as usize, &mut caps, sz) } != JOYERR_NOERROR {
            continue;
        }
        let mut info: JoyInfoEx = unsafe { std::mem::zeroed() };
        info.dw_size = std::mem::size_of::<JoyInfoEx>() as u32;
        info.dw_flags = JOY_RETURNALL;
        if unsafe { joyGetPosEx(id, &mut info) } != JOYERR_NOERROR {
            continue; // not connected
        }
        let vidpid = format!("VID_{:04X}&PID_{:04X}", caps.w_mid, caps.w_pid);
        let name = oem_name(&vidpid).unwrap_or_else(|| {
            let p = pcwstr_to_string(&caps.sz_pname);
            if p.is_empty() { vidpid.clone() } else { p }
        });
        // winmm caps flags: which optional axes exist. X/Y always present; no Rx/Ry.
        let c = caps.w_caps;
        let present = [
            true, true, c & JOYCAPS_HASZ != 0, false, false,
            c & JOYCAPS_HASR != 0, c & JOYCAPS_HASU != 0, c & JOYCAPS_HASV != 0,
        ];
        out.push(Device {
            id,
            vid: caps.w_mid,
            pid: caps.w_pid,
            name,
            num_axes: caps.w_num_axes,
            num_buttons: caps.w_num_buttons,
            axes: [info.dw_xpos, info.dw_ypos, info.dw_zpos, 0, 0, info.dw_rpos, info.dw_upos, info.dw_vpos],
            present,
            buttons: info.dw_buttons,
            pov: info.dw_pov,
            has_pov: caps.w_caps & JOYCAPS_HASPOV != 0,
        });
    }
    out
}
