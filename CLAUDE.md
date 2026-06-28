# MW5-Remap-rs

A visual joystick/HOTAS binding editor (Rust + egui) for **MechWarrior 5: Mercenaries**,
**Ace Combat 7**, and **Star Citizen**. Press a chip in the grid, actuate a control, and it
binds — chips glow green live as you move axes/press buttons. Built for a MOZA AB6 + MRP
rig but device-agnostic via a shared registry.

## Build

gnullvm (LLVM-MinGW) toolchain, fully **offline** (no new crate deps — keep it that way):

```bash
export PATH="/c/Program Files/Rustlang/bin:$HOME/.cargo/bin:$PATH"
export PATH="$PATH:$(cygpath 'C:\Users\falk\AppData\Local\Programs\LLVM-MinGW-UCRT\bin')"
cargo build --release        # -> target/release/MW5-Remap.exe
```

`build.rs` bakes `GIT_BRANCH`/`GIT_HASH` into the binary (shown in the footer version stamp).

## Release process

1. Bump the version in **`Cargo.toml`** AND **`installer.iss`** (`MyAppVersion`) — keep them equal.
2. `cargo build --release`.
3. **Kill any running `*MW5-Remap*` process first** (it locks the exe → busy-file on copy):
   `Get-Process | ? { $_.ProcessName -like '*MW5-Remap*' } | Stop-Process -Force`
4. `cp target/release/MW5-Remap.exe ./MW5-Remap.exe` (repo root, for the installer).
5. Build installer: `& "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" installer.iss` → `dist\MW5-Remap-Setup.exe`.
6. `git commit` + `git push`.
7. `gh release create vX.Y.Z ./MW5-Remap.exe dist/MW5-Remap-Setup.exe libunwind.dll --title vX.Y.Z --notes "..."`.
8. Verify: `./MW5-Remap.exe --testhttp` → `own repo latest` should match the new version.

Auto-update (`src/update.rs`) pulls the latest GitHub release via WinHTTP; the banner only
shows when a newer version exists. Repo: `falkoro/mw5-remap`.

## Architecture

- **`src/main.rs`** — entry, module list, CLI flag dispatch, GUI launch, window icon.
- **`src/cli.rs`** — headless helpers: `--selftest`, `--devices`, `--monitor`, `--apply-defaults`,
  `--force-defaults`, `--write-hotas`, `--lock`/`--unlock`, `--diagram`, `--ac7-setup`, `--sc-test`, `--testhttp`.
- **Input layer (read controllers):**
  - **`src/dinput.rs`** — **DirectInput8** reader (PREFERRED). Pure COM-vtable FFI, custom data
    format, `GetDeviceState`, handles kept in a thread-local. Exposes all **8 axes
    `[X,Y,Z,Rx,Ry,Rz,Slider0,Slider1]`** (0..65535). This is what the Windows "Game Controllers"
    panel uses.
  - **`src/input.rs`** — winmm fallback + the shared `Device` struct (`axes: [u32;8]`). `poll()`
    tries DirectInput first, falls back to winmm. **winmm is hard-capped at 6 slots
    `[X,Y,Z,R(=Rz),U,V]` and CANNOT report Rx/Ry** — that's why the analog hat was invisible
    until we switched to DirectInput.
- **`src/devices.rs`** — shared device registry (VID/PID, role, axis semantics) used by all games.
- **Games (`src/games/`)** — each implements the `GameProvider` trait (`mod.rs`):
  - **`mw5/`** — `mod.rs` (provider, `axis_token`/`button_token`/`pov_token`), `data.rs` (action
    catalog + default layout), `hotas.rs` (writes `HOTASMappings.Remap` + config lock), `parse.rs`.
  - **`ac7.rs`**, **`sc.rs`** — Ace Combat 7 `Input.ini`, Star Citizen `actionmaps.xml`.
- **`src/visual/`** — `mod.rs` device panel: `hot_tokens`/`axis_deflected` (live green), `live_axes`
  (raw axis bars), device-image markers; `draw.rs` painters. **`axis_deflected` indices follow the
  DirectInput 8-axis layout.**
- **`src/app/`** — egui shell: `mod.rs` (state, capture resolve), `panels.rs` (banner/central/footers),
  `toolbar.rs` (Save/Fix HOTAS/Lock/Reset/Export/Launch), `widgets.rs` (binding row + chip colours),
  `export_ui.rs` (PNG/PDF export dialog).
- **`src/export.rs`**, **`src/diagram.rs`** — PNG/PDF sheet + HTML infographic.
- **`src/hidhide.rs`**, **`src/sys.rs`**, **`src/update.rs`** — HidHide, elevation/process utils, auto-update.

## MechWarrior 5 binding model (the important part)

MW5 joystick input is a **TWO-FILE** system. Editing only `GameUserSettings.ini` does nothing in-game.

1. **`HOTASMappings.Remap`** (`%LOCALAPPDATA%\MW5Mercs\Saved\SavedHOTAS\`) — maps *physical device input → token*,
   keyed by VID/PID. Written by **🎮 Fix HOTAS file** / `--write-hotas`. `AXIS:`/`BUTTON:` lines.
2. **`GameUserSettings.ini`** (`...\Saved\Config\WindowsNoEditor\`) — maps *token → action*. Written by **💾 Save**.

Chain: physical → (Remap) → token → (GameUserSettings) → action. **Both must agree on the same token.**

Gotchas:
- **OutButton hard cap = `Joystick_Button1..20`** (Button21+ is dead).
- **Throttle is ONE bipolar axis** (`Throttle_Axis2` → `JoystickThrottle`): centre=stop, up=forward,
  below-centre=reverse. Two unipolar toe pedals drive it via two `AXIS:` lines onto the same OutAxis
  (right toe forward, left toe reverse). **There is no separate "reverse" binding.**
- **MW5 rewrites `GameUserSettings.ini` joystick bindings back to STOCK on launch** unless the file is
  **read-only** → use **🔒 Lock config** / `--lock`. (Classic failure: throttle action gets reset to a
  stock axis that the `.Remap` doesn't feed, so the throttle goes dead. Lock prevents this.)
- **MW5 does NOT recognise `RX`/`RY` axis names.** An axis Windows shows as X-Rotation/Y-Rotation must
  be addressed in the `.Remap` as `GenericUSBController_AxisN` (raw HID index), not `HOTAS_RXAxis`.

## Hardware (the dev's rig) — confirmed live via `--devices`/`--monitor`

DirectInput 8-axis layout `[X,Y,Z,Rx,Ry,Rz,Slider0,Slider1]`:
- **MOZA AB6 FFB Base** (`VID 0x346E PID 0x1002`, Joystick role): gimbal **X/Y**; analog thumb/POV hat
  = **Rx (vertical) / Ry (horizontal)**, centred ~32767 (→ `Joystick_Axis4`/`Axis5`); **Slider0** rests
  at 65535 (a lever, currently unmapped), **Slider1** ~3378.
- **MOZA MRP Rudder Pedals** (`PID 0x1200`, Throttle role): toes = **X (right) / Y (left)**, UNIPOLAR
  resting at **0**, both onto `Throttle_Axis2`; rudder swing-arm = **Rz** → `Throttle_Axis1` (leg turn).

## Testing

- `--selftest` — config round-trip on a TEMP copy (never touches the real file); prints the loaded
  layout + `ROUND-TRIP: PASS/FAIL`. Run after any change to `data.rs`/`parse.rs`/`mod.rs`.
- `--devices` — one-shot dump of every controller + all 8 axis values (note: winmm returns 32767 on a
  COLD first read; real rest values appear once polling continuously — use `--monitor` for live deltas).
- `--monitor` — 25s live change log (the key hardware-diagnosis tool).

## Conventions

- Match surrounding style; keep modules ≤ ~250 lines (the codebase was refactored to that budget).
- No new crate dependencies (offline toolchain).
- After hardware findings, update the persistent memory note `mw5-hotas-remap`.
