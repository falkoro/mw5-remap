---
name: improve-ui
description: Use when changing or adding ANY UI in this egui app — binding chips, panels, notifications, buttons, the vJoy tab, dialogs, the toolbar — so it stays consistent with the ONE design system in src/app/theme.rs. Trigger it when the user says the UI "looks off / retarded / inconsistent / ugly", asks to restyle/polish/clean a panel, or you're adding a new widget or screen. Always screenshot-verify the result.
---

# Improve / extend the MW5-Remap UI

This app has ONE design system: **`src/app/theme.rs`**. Every colour, every shared widget
shape lives there. The cardinal sin (and the source of every past "looks retarded") is
hardcoding `Color32::from_rgb(...)` in a widget, or hand-darkening a card so one surface
clashes with the rest. Don't. Pull from `theme::`.

## 0. Golden rules
- **No hardcoded colours outside `theme.rs`.** Use the palette consts. If you need a new
  shade, add it to `theme.rs` with a name, then use it.
- **One light look.** The app is a refined LIGHT theme. `theme::apply(ctx)` runs every frame
  (set in `app/mod.rs::update`) and styles panels, combos, popups, scrollbars and text edits.
  Don't fight it with dark frames.
- **Reuse the building blocks** instead of re-rolling a frame: `theme::card`, `theme::section`,
  `theme::pill_button`, `theme::chip`, `theme::status_pill`.
- **LIVE must be unmistakable.** When a control is active its chip goes vivid green
  (`ChipState::Live`). Never make the active state subtle — the user must SEE the press.
- Keep every module **≤ ~250 lines** (split a helper out if needed).

## 1. The palette (src/app/theme.rs)
| Const | Use |
|---|---|
| `BG` | window / panel backdrop |
| `SURFACE` | raised surfaces (toolbar, headers, hover) |
| `CARD` | cards, chips, popups (white) |
| `CARD_ALT` | striped rows + the unbound chip "slot" |
| `RIM` / `RIM_STRONG` | hairline / stronger borders |
| `ACCENT` / `ACCENT_DK` | LIVE green (fills) / green text+lines on light |
| `STICK` / `THROTTLE` | Joystick-role / Throttle-role device colour |
| `CAPTURING` / `CAP_DK` | "listening" amber fill / readable amber text |
| `TEXT` / `TEXT_DIM` / `TEXT_FAINT` | primary / secondary / faint text |
| `ON_ACCENT` | text on a filled accent chip |
| `device_color(token)` | the colour identifying a token's device |
| `tint(c, t)` | lighten a colour toward white (soft fills) |

## 2. Recipe — style a new widget/panel
1. Wrap grouped content in `theme::card(ui, |ui| { … })` or `theme::section(ui, "TITLE", …)`.
2. Buttons: `theme::pill_button(ui, enabled, "Label", accent)` — `accent=true` for the one
   primary call-to-action, `false` for everything else. (Plain `ui.button` is fine too; the
   global visuals already style it.)
3. A control "chip" (something that shows a bound value + role colour): `theme::chip(ui, text,
   state)` with `ChipState::{Unbound, Bound(device_color), Live, Capturing}`.
4. Text: `theme::TEXT` for primary, `TEXT_DIM` for secondary, `TEXT_FAINT` for hints. Never
   pure black/grey literals.
5. Rounding: `theme::R_CHIP` (controls) / `theme::R_CARD` (containers). Keep it consistent.

## 3. Verify it LOOKS right — screenshot the running app
You cannot judge UI from code. Build, run the staged exe, and capture the window (the repo
root `MW5-Remap.exe` is HidHide-whitelisted and ships `libunwind.dll` beside it). Kill any
running instance first (it locks the exe), then:
```powershell
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;using System.Runtime.InteropServices;
public class W{[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out R r);
[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")]public static extern bool ShowWindow(IntPtr h,int c);
[StructLayout(LayoutKind.Sequential)]public struct R{public int L,T,Ri,B;}}
"@
Get-Process | ? { $_.ProcessName -like 'MW5-Remap*' } | % { try{$_.Kill()}catch{} }; Start-Sleep 2
Copy-Item .\target\release\MW5-Remap.exe .\MW5-Remap.exe -Force
$p=Start-Process .\MW5-Remap.exe -PassThru; $h=[IntPtr]::Zero
while($h -eq [IntPtr]::Zero){Start-Sleep -m 500;$p.Refresh();$h=$p.MainWindowHandle}
[W]::ShowWindow($h,3)|Out-Null;[W]::SetForegroundWindow($h)|Out-Null;Start-Sleep 3
$r=New-Object 'W+R';[W]::GetWindowRect($h,[ref]$r)|Out-Null
$b=New-Object System.Drawing.Bitmap(($r.Ri-$r.L),($r.B-$r.T))
[System.Drawing.Graphics]::FromImage($b).CopyFromScreen($r.L,$r.T,0,0,$b.Size)
$b.Save("$env:TEMP\mw5_ui.png");$p.Kill()
```
Then **Read `%TEMP%\mw5_ui.png`** and judge it. Iterate on `theme.rs` (one place) until it
reads clean. The `Detected:` line + chip glow need a real button press, so only layout/colour
is verifiable headless — that's enough for styling.

## 4. Build truth, not squiggles
rust-analyzer lies mid-refactor (stale `&[T;N]` vs `&[T]`, false missing-field/arg-count).
ALWAYS confirm with `cargo build --release` + `cargo test`. `touch src/app/*.rs` before the
build so real warnings actually surface (a 0.1s "Finished" means it used the cache).

## Pitfalls
- Adding a dark `Frame` "because it pops" — it breaks cohesion. Use `theme::card`.
- Over-painting the live state (glow stacks, gradient sheens, hover washes) — it reads as
  noise. `theme::chip` already gives LIVE one clean vivid fill + a single soft ring. Leave it.
- Forgetting `theme::apply` runs each frame — if a popup looks default-styled, it's drawn in a
  context the visuals didn't reach; pass colours explicitly via the `theme::` consts.
