---
name: fetch-joystick-diagram
description: Fetch a clean product image for a joystick/HOTAS/pedal and add it to MW5-Remap's device diagram (image asset + callout markers + visual wiring) so its controls show live in the panel. Use when a device is in the registry but has no diagram picture, or the user says "add an image for device X", "fetch the diagram", "get the picture for my VKB/X56/Warthog", or "support the most popular sticks visually".
---

# Fetch a joystick diagram into MW5-Remap

Companion to [[add-joystick]] (which fills the `src/devices.rs` registry row). THIS skill
is the repeatable recipe for the *visual* half: get a clean device IMAGE, place the
callout markers on it, and wire it into `src/visual/`. The MOZA gear was done this way
(`assets/ab6_base.png`, `mhg_stick.png`, `mrp_pedals.jpg`); follow the same path for any
other stick.

## 0. Golden rules
- **Top-down or straight-on product render, all controls visible.** A flat manufacturer
  render beats an angled photo — markers land accurately and stay put. Avoid cluttered
  marketing shots with hands/backgrounds.
- **Use the manufacturer's own product/press image** (their site, store page, press kit).
  These are made to be reproduced. Note the source URL in the commit message. Don't grab
  random watermarked stock.
- **Never guess the button layout** — confirm numbering from BOTH the vendor manual AND
  the live `--devices` dump (see [[add-joystick]] §1). A marker's `token` must match the
  device's DIRECT MW5 token so the resolver lights it (works in direct AND vJoy mode).

## 1. Priority list — the most-sold sticks to support first
Knock these out in roughly this order (sales + how often they're asked about):
- Thrustmaster: **T.16000M**, **TWCS Throttle**, **T.Flight HOTAS X/One**, **Warthog** (stick+throttle)
- Logitech/Saitek: **X52 / X52 Pro**, **X56 Rhino**, **Extreme 3D Pro**
- VKB: **Gladiator EVO** (the user's), **Gunfighter**
- VirPil: **Constellation Alpha**, **CM3 throttle**
- Honeycomb **Alpha/Bravo**, Turtle Beach **VelocityOne**, **WinWing** panels
- MOZA **AB9** (sibling of the supported AB6)

## 2. Find a clean image
```
WebSearch  "<exact model> top view png"   and   "<model> product render transparent"
```
Open candidates with WebFetch on the manufacturer product page; pick a top-down/straight
render where every button + the hat + axes are visible and unobstructed. Grab its direct
image URL (right-click → copy image address, or read it from the page HTML).

## 3. Download it into assets/
```bash
cd C:\Users\falk\MW5-Remap-rs
curl -L "<image-url>" -o assets/<dev>.png          # or .jpg
./MW5-Remap.exe --imgcheck                          # decodes the embedded assets; add a line for the new one OR:
# quick validity check without rebuilding:
file assets/<dev>.png ; identify assets/<dev>.png 2>/dev/null || true
```
Reject it if: < ~600px on the long edge, transparent-but-tiny, angled so buttons overlap,
or it fails to decode. Re-pick. Prefer PNG; JPG is fine for photos (see `mrp_pedals.jpg`).

## 4. Add markers — and use Edit-layout to place them LIVE (the easy way)
Markers are normalized `(x, y)` 0..1 callouts whose `token` matches the control's MW5
token. Don't eyeball pixel coords by hand — use the app:
1. In `src/visual/mod.rs` add a `*_MARKERS: &[Marker]` array with rough positions and the
   correct tokens (button N → the device's `button_token`; axes → its `axis_token`; hats →
   a `*_HATS (nx,ny,ways)` entry with the **real way-count** from the manual). Add the
   `include_bytes!`/texture in `load_textures` and an `image_block(...)` call in `sidebar`.
2. `cargo build --release`, run the app, open the device panel, click **✥ Edit layout**,
   and **drag each marker onto its real button**. Positions auto-save to
   `%LOCALAPPDATA%\MW5-Remap\marker_layout.txt` (device-key, marker-id, x, y).
3. Read that file back and bake the tuned `(x,y)` into the `*_MARKERS` defaults (so it ships
   correct for everyone, not just whoever dragged it). Then `Reset layout` to clear the
   per-user override.

## 5. Wire the live glow
A marker only lights when the app can tell its control is active:
- Its `token` must be the device's DIRECT token (the resolver in `src/visual/resolve.rs`
  re-maps it through vJoy automatically when the user routes that stick — you do nothing
  extra for vJoy).
- For axis callouts, wire the device's `(vid,pid)` + axis slot into `axis_deflected`
  (see how the MOZA AB6 indices are handled there).

## 6. Verify
```bash
export PATH="/c/Program Files/Rustlang/bin:$HOME/.cargo/bin:$PATH"
export PATH="$PATH:$(cygpath 'C:\Users\falk\AppData\Local\Programs\LLVM-MinGW-UCRT\bin')"
cargo build --release && cargo test
./MW5-Remap.exe --imgcheck          # the new asset decodes
```
Then run the app: the image renders, every marker sits on its control, and pressing a
button / moving an axis lights the matching callout green (in both direct and vJoy mode).

## Pitfalls
- **Markers drift if you swap the image later** — they're normalized to the image rect, so
  a different crop/aspect moves everything. Re-tune with Edit-layout after any image change.
- **Hat way-count** still matters (4-way diagonals are dead) — carry it from [[add-joystick]] §2.
- **Licensing**: manufacturer product renders are fine to embed for a hardware-companion
  tool; cite the source URL. If only an angled photo exists, a clean schematic you draw
  (see `cli::genlogo` for the analytic-AA drawing approach) is an acceptable fallback.
- Keep `src/visual/mod.rs` within the ~250-line budget — if it grows, move the marker
  tables into a small `src/visual/devices_markers.rs`.
