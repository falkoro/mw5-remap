# AGENTS.md — MW5-Remap-rs

Agent-oriented guide for this repo. Human/architecture detail also lives in `CLAUDE.md`;
this file is the quick reference + the conventions an agent must follow.

## What this is
A Rust + **egui/eframe 0.29** desktop app: a visual joystick/HOTAS binding editor for
**MechWarrior 5** (primary), **Ace Combat 7**, and **Star Citizen**. Click a chip in the
grid, actuate a control, it binds; chips glow green while the control is live. Built around a
MOZA AB6 + MRP rig but device-agnostic via a shared registry. Ships as one `.exe` + an Inno
Setup installer and auto-updates from GitHub releases (`falkoro/mw5-remap`).

## Build & test (OFFLINE — no new crates)
gnullvm / LLVM-MinGW toolchain. Always export the PATH first:
```bash
export PATH="/c/Program Files/Rustlang/bin:$HOME/.cargo/bin:$PATH"
export PATH="$PATH:$(cygpath 'C:\Users\falk\AppData\Local\Programs\LLVM-MinGW-UCRT\bin')"
cargo build --release        # -> target/release/MW5-Remap.exe
cargo test                   # dev profile ONLY (release has panic=abort, which breaks tests)
```
- **No new crate dependencies** (the toolchain is offline). Keep modules **≤ ~250 lines**.

## ⚠️ CRITICAL: rust-analyzer diagnostics are unreliable here
Mid-edit, rust-analyzer emits **stale, false** errors — most often `&[T; N]` vs `&[T]`
(E0308), plus "missing field"/"wrong arg count" while a multi-file change is in flight.
**Do not trust the squiggles. Verify with `cargo build --release` (+ `cargo test`).** A clean
cargo build means the code is correct. To force real warnings to surface after editing,
`touch` the changed files before building (cargo caches aggressively — a 0.1s "Finished"
means it didn't recompile).

## Architecture (where things live)
- `src/main.rs` — entry, CLI flag dispatch, GUI launch.
- `src/cli.rs` — headless helpers: `--devices`, `--selftest`, `--monitor`, `--write-hotas`,
  `--lock`/`--unlock`, `--diagram`, …
- **Input:** `src/dinput.rs` (DirectInput8, PREFERRED, all 8 axes `[X,Y,Z,Rx,Ry,Rz,Sl0,Sl1]`);
  `src/input.rs` (winmm fallback + the shared `Device` struct). `poll()` tries DI then winmm.
- `src/devices.rs` — shared device registry (VID/PID, role, axis semantics).
- `src/games/` — one `GameProvider` per game. `mw5/` = `mod.rs` (provider, `*_token`),
  `data.rs` (action catalog + defaults), `hotas.rs` (writes the `.Remap` + the config lock),
  `parse.rs`. Also `ac7.rs`, `sc.rs`.
- `src/visual/` — the device-image panel, the live "hot token" glow (`hot_tokens`), and the
  vJoy-aware resolver (`resolve.rs`, traces physical → vJoy → token).
- `src/vjoy.rs` / `src/vjoy_map.rs` — the built-in Joystick-Gremlin. `vjoy.rs` feeds vJoy
  (runtime-loaded `vJoyInterface.dll`); `vjoy_map.rs` is the config-driven routing table
  (`Source`→`Target`, persisted to `%LOCALAPPDATA%\MW5-Remap\vjoy_map.txt`, pure `resolve()`).
- `src/app/` — the egui shell: `mod.rs` (state + `update()` loop), `panels.rs`, `toolbar.rs`,
  `widgets.rs` (binding chips), `tabs.rs` (Bind / vJoy Setup), **`theme.rs` (the design
  system — the single source of palette + UI helpers)**.
- `src/hidhide.rs`, `src/update.rs` — device hiding, auto-update.

## MechWarrior 5 binding model (the crux — get this right)
Joystick input is a **TWO-FILE** system; both files must agree on the same token:
1. `HOTASMappings.Remap` (`%LOCALAPPDATA%\MW5Mercs\Saved\SavedHOTAS\`) — *physical input →
   token*, keyed by VID/PID. Written by `mw5::write_hotas_mappings`.
2. `GameUserSettings.ini` (`…\Saved\Config\WindowsNoEditor\`) — *token → action*. Written by
   `GameProvider::save`.

Chain: **physical → (Remap) → token → (GameUserSettings) → action.**

Gotchas:
- OutButton hard cap = `Joystick_Button1..20` (21+ is dead). Extra buttons go to `Throttle_*`.
- Throttle is ONE bipolar axis (`Throttle_Axis2`): centre=stop, up=fwd, below-centre=reverse.
- MW5 does **not** recognise `RX`/`RY` axis names → address them as `GenericUSBController_AxisN`.
- **MW5 resets `GameUserSettings` joystick bindings to STOCK on launch unless the file is
  read-only.** The config lock (`mw5::set_config_locked`, default ON) re-applies read-only
  after each `save()`. (Read-only PREVENTS the reset; it does NOT make MW5 ignore the file.)
- Per the evilC/MW5HOTAS guide, the reliable path with vJoy is to bind the vJoy inputs in
  **MW5's own controls menu** (the `.Remap` makes vJoy appear as a clean "Joystick").

## The vJoy approach (works around the MOZA's 128 buttons)
Mirror the whole rig onto **ONE** clean vJoy device and bind that. **Only one vJoy device** —
two share `VID 1234 / PID BEAD` and are indistinguishable to MW5, so it reads the unfed one
→ dead buttons (`vJoyConfig -d <n>` to delete extras). The `.Remap` maps vJoy buttons 1-20 →
`Joystick_Button`, 21-32 → `Throttle_Button`; axes X/Y/Rx/Ry/Z/Rz → the matching MW5 axes.

## Conventions
- Match the surrounding style; modules **≤ ~250 lines** (split into a helper module if a file
  would exceed it — see how `vjoy_style`/`theme` were extracted).
- **All UI styling goes through `src/app/theme.rs`** — the single palette + the `card` /
  `chip` / `pill_button` / `section` helpers + `theme::apply(ctx)` (global egui `Visuals`).
  Never hardcode colors in a widget. See the **`improve-ui`** skill for the rules.
- After a hardware finding, update the persistent memory note.

## Release process
1. Bump version in **`Cargo.toml`** AND **`installer.iss`** (`MyAppVersion`) — keep equal.
2. `cargo build --release`.
3. Kill running `*MW5-Remap*` procs (they lock the exe), then `cp target/release/MW5-Remap.exe
   ./MW5-Remap.exe`.
4. Build installer: `ISCC.exe installer.iss` → `dist\MW5-Remap-Setup.exe`.
5. `git commit` + `git push`; `gh release create vX.Y.Z ./MW5-Remap.exe
   dist/MW5-Remap-Setup.exe libunwind.dll`.

Co-author trailer for commits: `Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
