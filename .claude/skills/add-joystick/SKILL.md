---
name: add-joystick
description: Research a new joystick/HOTAS/pedal and add it to MW5-Remap's shared device registry so it "just works" across MechWarrior 5, Ace Combat 7, and Star Citizen. Use when the user plugs in or names a controller the app doesn't know yet (e.g. "add my X-56 throttle", "the pedals aren't recognised", "support device Y").
---

# Add a joystick to MW5-Remap

Every supported game renders its config from ONE shared table: `src/devices.rs`
(`KnownDevice` + `AxisMap`). Describe a device there once and MW5 (`HOTASMappings.Remap`),
AC7 (`Input.ini`) and SC (`actionmaps.xml`) all pick it up. This skill is the
repeatable recipe for filling in a new row correctly — the hard part is getting
the *facts* (USB ids, which physical axis is which, hat way-count), not the code.

## 0. Golden rule

Never guess axis indices, button counts, or hat ways. Confirm each from BOTH:
1. **The hardware itself** — `MW5-Remap.exe --devices` (live winmm dump), and
2. **The vendor manual/product page** — for labels (which axis is throttle vs rudder)
   and the hat way-count (4-way vs 8-way), which the live dump can't tell you.

If the two disagree, the live dump wins for *indices* and the manual wins for *meaning*.

## 1. Get the live facts from the device

Plug the device in, then run the headless dump (no GUI, prints to console):

```
cd C:\Users\falk\MW5-Remap-rs
cargo run -- --devices        # debug build shows the console
```

For each connected controller it prints:
```
#<id> [Role] <name>  VID_xxxx&PID_xxxx  N axes M btns  has_pov=true/false
    axes X.. Y.. Z.. R.. U.. V..   pov=....
    pressed button K -> token ...        (while you hold a button)
    pov octant O -> token ...            (while you hold the hat)
```

Record from this: **VID, PID, button count, has_pov**, and — by wiggling ONE
control at a time and watching which of the six axis slots moves — **which winmm
axis slot (X=0, Y=1, Z=2, R=3, U=4, V=5) each physical axis lives on.**

> winmm only ever exposes six axes in the fixed order `[X, Y, Z, R(=Rz), U, V]`.
> A device's "Rz" rudder twist almost always lands in slot **R (index 3)**.
> Self-centering axes rest at ~32767; a toe pedal / lever usually rests at 0 (or 65535).
> Note the REST value of each axis — it decides `Offset`/`Invert` later.

## 2. Research the meaning (vendor docs)

Use WebSearch/WebFetch on the official product page + PDF manual. Confirm:
- **Role**: is it a stick (aim) or a throttle/pedals (movement)? → `Role::Joystick` / `Role::Throttle`.
- **Each axis' job**: pitch / roll / yaw(rudder) / throttle → the `Sem` enum.
- **Hat way-count**: 4-way (cardinals only) or 8-way (diagonals too)? Stored as the
  `u8` in `visual.rs` `*_HATS`. MW5 hat tokens are `Joystick_Hat_1..8` (1=up, 3=right,
  5=down, 7=left, even = diagonals). **Only bind diagonals on a true 8-way hat** — on a
  4-way hat the diagonal tokens never fire.
- **Button count cap**: MW5 only honours `Joystick_Button1..20`; higher buttons are
  dead tokens. Set `buttons` to the real count but the writer caps emission at 20.

## 3. Add the registry row (`src/devices.rs`)

```rust
// <Vendor Model>: <one line on axis layout, from §1+§2>
const FOO_AXES: &[AxisMap] = &[
    ax(Pitch,    "HOTAS_YAxis",  "Y",  true),   // sem, MW5 InAxis, AC7 letter, reverse?
    ax(Roll,     "HOTAS_XAxis",  "X",  false),
    ax(Throttle, "HOTAS_ZAxis",  "Z",  false),
];
// in REGISTRY:
KnownDevice { name: "Vendor Model", vid: 0xXXXX, pid: 0xYYYY,
              role: Role::Joystick, buttons: N, has_hat: true/false,
              axes: FOO_AXES, custom: false },
```

- `hotas` = the MW5 `.Remap` `InAxis` name. The map is by MEANING, but the actual
  axis read in-game depends on the physical slot, so pick the `HOTAS_*Axis` that the
  vendor/HID actually emits for that physical axis (Y→pitch, X→roll, R→`HOTAS_RZAxis`,
  Z→`HOTAS_ZAxis`). Verify in-game.
- `ac7` = the AC7 `Input.ini` axis letter (`X`,`Y`,`Z`,`Rx`,`Ry`,`Rz`); `reverse:true`
  appends `:R`.
- `custom: true` ONLY for a template with placeholder `0x0000` ids (written once, never
  auto-managed). Real devices are `false`.

## 4. (Optional) Add it to the visual panel (`src/visual.rs`)

If you have an image asset (`assets/<dev>.png`), add `*_MARKERS` (numbered axis
callouts whose `token` matches the MW5 token so they light up live) and, for hats,
a `*_HATS` entry `(nx, ny, ways)` with the **correct way-count from §2**. Add an
`image_block(...)` call in `sidebar`. Axis highlight needs the device's (vid,pid)
+ slot index wired into `axis_deflected`.

## 5. Build, verify detection, verify in-game

```
# gnullvm toolchain (Git Bash):
export PATH="/c/Program Files/Rustlang/bin:$HOME/.cargo/bin:$PATH"
export PATH="$PATH:$(cygpath 'C:\Users\falk\AppData\Local\Programs\LLVM-MinGW-UCRT\bin')"
cargo build                       # debug (console)  | --release for shipping
cargo run -- --devices            # confirm the new row's name shows next to its VID/PID
cargo run -- --write-hotas        # MW5: emit the device block into HOTASMappings.Remap
cargo run -- --selftest           # MW5 config round-trip must stay PASS
cargo run -- --sc-test            # SC actionmaps round-trip
```

Then launch each target game and confirm the axes move the right way (flip `reverse`
/ the in-app Invert if not) and the hat fires every bound way.

## Pitfalls (learned the hard way)

- **Hat "doesn't work" → check the vendor's hat MODE first.** A MOZA hat (and many
  others) can be set in vendor software (MOZA Cockpit) to **POV mode** OR **discrete
  button mode**. In POV mode it emits a continuous hat angle (winmm `pov`, → MW5
  `Joystick_Hat_1..8`); in button mode it emits plain button presses (→ `Joystick_ButtonN`).
  Bindings written for one mode are inert in the other. Use the app's live panel: press
  the hat — if the hat SPOKES light, it's POV mode; if a BUTTON number lights, it's in
  button mode. Match the binding (or the hardware mode) accordingly.
- **Two separate pedal axes can't be merged** into one MW5 throttle axis in `.Remap`
  (one `InAxis`→one `OutAxis`). For "right pedal = forward, left = reverse" the pedals
  must be combined into a single split axis in the vendor driver (e.g. MOZA Pit House),
  OR put reverse on a button.
- **4-way hat bound on diagonals = dead controls.** Check the way-count first.
- **Button > 20 = dead token** in MW5.
- A device missing from `HOTASMappings.Remap` makes its `GameUserSettings.ini` tokens
  inert in-game — always run `--write-hotas` after adding an MW5 device.
