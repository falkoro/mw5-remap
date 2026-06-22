# Contributing to MW5 Remap

Thanks for taking a look! This is a small Rust + [egui](https://github.com/emilk/egui) desktop app for Windows. Contributions — bug reports, new game providers, hardware mappings, fixes — are welcome.

## Toolchain

The project builds against the **GNU/LLVM (gnullvm) Windows toolchain** (no MSVC needed):

```powershell
winget install Rustlang.Rust.GNU.LLVM
winget install MartinStorsjo.LLVM-MinGW.UCRT
```

The llvm-mingw `bin` directory must be on `PATH` for the linker. In a bash shell:

```sh
export PATH="/c/Program Files/Rustlang/bin:$HOME/.cargo/bin:$PATH"
export PATH="$PATH:$(cygpath 'C:\Users\<you>\AppData\Local\Programs\LLVM-MinGW-UCRT\bin')"
```

## Build & run

```sh
cargo build              # debug — keeps a console window so CLI flags print output
cargo build --release    # release — windows_subsystem=windows (no console)
cargo run -- --selftest  # run a CLI subcommand
```

Use the **debug** binary (`target/debug/MW5-Remap.exe`) when you need to see `println!` output; the release build hides the console.

## Project layout

```
src/
  main.rs        entry point + CLI subcommands (--selftest, --write-hotas, --lock, …)
  app.rs         the egui app: top bar, binding grid, buttons, state
  visual.rs      left device panel: photos, numbered callouts, live highlight, button board
  diagram.rs     HTML control-map export (embeds the device photos as base64)
  input.rs       winmm joystick polling (joyGetPosEx) → raw Device state
  update.rs      GitHub Releases auto-update over WinHTTP
  hidhide.rs     optional conflicting-device hiding
  sys.rs         small Windows helpers (process check, open URI, elevation)
  games/
    mod.rs       the GameProvider trait + registry
    mw5.rs       MechWarrior 5 provider (the one fully implemented)
assets/          embedded device photos (include_bytes!)
```

## Architecture

The core abstraction is **`GameProvider`** (`games/mod.rs`): each game knows its action catalog, how to read/write its config, and how to turn a raw joystick press into that game's token. Star Citizen and MSFS are registered as "coming soon" stubs.

Two key facts the MW5 provider encodes (see [README](README.md#why-this-exists)):
- Joystick input needs **two** files: `HOTASMappings.Remap` (device→token) and `GameUserSettings.ini` (token→action). Both are written and kept consistent.
- MW5 only has `Joystick_Button1..20`, and it resets `GameUserSettings.ini` on launch (hence the read-only **lock**).

### Adding a new game

1. Add a module under `src/games/` implementing `GameProvider`.
2. Register it in `games::all()`.
3. Provide `actions()`, `load()`, `save()`, the press-to-token methods, and ideally `default_bindings()`.

### Adding / fixing a hardware mapping

Device IDs and the canonical MOZA token mapping live in `games/mw5.rs` (`BASE`/`PEDALS` consts, `moza_blocks()`). The HOTAS `.Remap` vocabulary (allowed `InAxis`/`InButton`/`OutAxis`/`OutButton`) comes from Piranha's official [HOTAS Remapping PDF](https://static.mw5mercs.com/docs/MW5HotasRemappingDocumentation.pdf).

## Testing

There's no game required for the core checks — they operate on a **temp copy** of the real config via the `MW5_CONFIG` / `MW5_HOTAS` env overrides:

```sh
cargo run -- --selftest   # load → mutate → save → reload, plus structural integrity (must print OVERALL: PASS)
cargo run -- --devices    # confirm winmm sees your sticks and prints the right tokens
```

Please run `--selftest` (must be **PASS**) before opening a PR that touches `games/mw5.rs`.

## Release process

1. Bump the version in **both** `Cargo.toml` and `installer.iss` (`MyAppVersion`).
2. `cargo build --release`
3. Copy `target/release/MW5-Remap.exe` (and `libunwind.dll` if present) next to `installer.iss`, then build the installer:
   `& "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" installer.iss` → `dist/MW5-Remap-Setup.exe`
4. `gh release create vX.Y.Z MW5-Remap.exe dist/MW5-Remap-Setup.exe --title vX.Y.Z --notes "…"`

The in-app updater (`update.rs`) looks for the asset named exactly `MW5-Remap.exe` and never the `-Setup.exe`, so always attach both.

## Code style

Match the surrounding code: terse, comment the *why* (especially Windows/MW5 quirks), no new dependencies without a good reason (the release intentionally avoids an unwinder and TLS crates). Keep the gnullvm build clean — no new warnings.
