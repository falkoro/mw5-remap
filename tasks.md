# MW5-Remap — backlog

Persistent task list (committed to the repo so it survives reboots). The in-session
task tracker mirrors these. Check items off as they ship.

---

## ★ v0.4 PLAN (proposed 2026-06-28 — review before building)

Goal: stop fighting per-game `.Remap` quirks, support more hardware, and let people
share working setups. Build order is bottom-up so each step is testable on its own.

### A. vJoy virtual-throttle combiner (THE proper throttle fix — multi-game)
The root problem all session: two unipolar toe pedals can't drive one bipolar throttle
in a game that reads a single axis line (MW5 doesn't sum; reverse via a button is a
workaround). The clean, game-agnostic fix is a **virtual axis**:
1. **Install vJoy** (https://github.com/njz3/vJoy — maintained fork; or jshafer817/vJoy).
   Kernel driver — do this WITH the user present (admin install). App should detect it
   and guide the user if missing.
2. App runs a tiny **background combiner** (own thread, while playing): read both physical
   toes via DirectInput (already have `dinput.rs`), compute `throttle = right - left`
   centred (centre=stop, right=forward, left=reverse), and feed it to a **vJoy axis** via
   `vJoyInterface.dll` (FFI, same style as winmm/dinput). 
3. Games (MW5, AC7, SC) bind their throttle to the **vJoy axis** — one clean bipolar axis,
   no per-game offset/deadzone hacks. MW5 `.Remap` then maps `vJoy axis → Throttle_Axis2`.
4. Optional: also expose combined rudder etc. The combiner is the foundation for any
   "merge N physical inputs into one virtual control" need.
Risks: vJoy must be installed + app must run during play; FFI for vJoyInterface; a config
toggle to enable/disable. Ship behind a clear "Enable virtual throttle (needs vJoy)" button.

### B. VKB device support
User now has a VKB (Gladiator EVO, per [[hardware_flightsim]]). Add it to the shared
registry (`src/devices.rs`): need its VID/PID + axis/button layout — get them by plugging
it in and running `--devices` (DirectInput now shows all 8 axes). Then it "just works"
across MW5/AC7/SC like the MOZA gear. Use the `add-joystick` skill.

### C. Profiles + downloadable community bindings (headline)
1. **Local profiles** (foundation): replace "↺ Reset to defaults" with a profile manager —
   built-in read-only "App Defaults", plus user create/save/load/duplicate/delete per game,
   stored as JSON in `%LOCALAPPDATA%\MW5-Remap\profiles\<game>\<name>.json`. Loading fills
   the grid (Save still required to write to game). [Task #16]
2. **Community bindings (downloadable):** a curated GitHub repo (e.g. `falkoro/mw5-remap-profiles`)
   holding shared `.json` profiles. In-app "Browse community profiles" → list via the GitHub
   API (reuse `update.rs`'s WinHTTP), download a chosen profile into the user's profile folder.
   "Share my profile" = instructions / PR link (or a gist upload later). Profiles are just the
   binding JSON, so they're safe to share.

### D. Code cleanup (ongoing)
- Done: quieted intentional `dinput.rs` FFI lints; annotated the kept-alive COM handles.
- "Corrupted .Remap": addressed — `write_hotas_mappings` now writes ONLY connected devices
  (drops stale Thrustmaster/Warthog blocks). A fresh "🎮 Fix HOTAS file" gives a clean file.
- Consider folding "Fix HOTAS file" into 💾 Save [Task #20] once the throttle/buttons are stable.

### E. Tests (v0.4.1+) — keep growing
`cargo test` now covers the logic that historically broke (vjoy combine/scale, profiles, hotas
strip/retain/producible-tokens/**no-orphans**, parse read_buttons/set_action, export). Add more as
features land. The `no_orphan_default_bindings` test is the guard against dead bindings.

### F. OPEN QUESTION — does MW5 even need the .Remap? [Task #24]
User reports MW5's OWN bind UI captures joystick buttons (you can bind them) but in GAMEPLAY none
fire — and our GameUserSettings binds also don't fire. Possible that MW5 (this Mercenaries install)
has native joystick support that CONFLICTS with our `.Remap` tokens. Reset path applied (.Remap
deleted + config unlocked) so the user can try native binding. If native works, the whole `.Remap`
approach may be obsolete for this install — re-evaluate before more `.Remap` work.

### NOT done autonomously (need the user / clarification)
- **vJoy install** — kernel driver; do it together (admin, hard to undo).
- **"headroom / gate / ponytail (on github)"** — could not identify these; will NOT install
  unidentified software. Need names/links from the user.

---

## Open

### v0.4 — Profiles system
Replace the single "↺ Reset to defaults" button with a profile manager. Ship a built-in,
read-only **"App Defaults"** profile (the current `data::default_bindings`). Users can
create / name / save / load / duplicate / delete their own profiles per game, stored in
`%LOCALAPPDATA%\MW5-Remap\profiles\<game>\<name>.json`. Loading a profile fills the grid
(not saved to game until 💾 Save). "App Defaults" is non-deletable. This is the headline
0.4 feature.

### Multi-input markers on the diagram ("add a bullet")
A single physical control can carry several inputs (a hat = left / right / up / down /
press; a rocker = in / out). On the device diagram, show **ONE arrow/marker** for that
control with an **"+ add input"** affordance, so the user can attach multiple bindings to
the same marker instead of one binding per circle. Touches `src/visual/` (marker model +
draw) and the binding model (marker → list of token/action). *(This is what "add a bullet"
meant — confirmed by the user.)*

### 5-second capture-all → save a profile
A one-click flow: open a ~5s window, the user actuates every control, the app records each
one and writes them into a profile. Fast profile authoring. Depends on the Profiles system.

### Draggable callout circles
Let the user drag/reposition the diagram markers when one is placed wrong (currently fixed
coords in `src/visual/`). Persist adjusted positions per device. Edit mode so normal
clicking still binds.

### Remove / fold the "🎮 Fix HOTAS file" button
User finds the separate button confusing. Fold the `HOTASMappings.Remap` write into the
normal 💾 Save path (or auto-run on save) so there's one save action. Keep the `--write-hotas`
CLI for power users.

## Recently shipped (context)

- DirectInput input layer (`src/dinput.rs`) — exposes Rx/Ry/sliders that winmm can't see.
- Throttle: right toe = forward (rest = stop), reverse on **button 3** (MW5 won't sum two toes).
- Inverted analog-hat look; Live-axes shows only the axes Windows detects.
- Update notification → themed top-right toast.
- `CLAUDE.md` with build/release/architecture/hardware reference.
