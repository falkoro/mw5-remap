# MW5-Remap — backlog

Persistent task list (committed to the repo so it survives reboots). The in-session
task tracker mirrors these. Check items off as they ship.

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
