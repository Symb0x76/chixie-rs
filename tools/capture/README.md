# ChiXie3 capture harness

Records the exact G-code ChiXie3 streams for a text, with no hardware and no UAC prompt. The
captures are the reference ("golden") data for chixie-rs's parity checks.

```powershell
cargo build -p chixie-rs-cli
./tools/capture/capture.ps1 -Text "你好赤写 A1" -Out captures/harness/ref1.gcode
./target/debug/chixie-rs-cli capture-diff captures/harness/ref1.gcode captures/harness/ref2.gcode
```

`captures/` is git-ignored (fonts are account data).

## How it works

1. `chixie-rs-cli fake-grbl <out>` listens on `127.0.0.1:23` like a Grbl_ESP32 Telnet port. It
   records every line, answers `ok` (5 ms apart: ChiXie3 counts one `ok` per socket read and stalls
   when they arrive together) and an Idle status to `?`. It stops at ChiXie3's end marker
   `G90G0 X0Y0`.
2. `capture.ps1` starts a **fresh** ChiXie3 un-elevated (`__COMPAT_LAYER=RunAsInvoker`), opens
   设备管理 › 无线, connects `[自定义] guangku`, pastes the text into the first cell, presses ▶, and
   quits ChiXie3 when the capture ends. ChiXie3 is force-closed if needed: after a job its Telnet
   thread often hangs, and a reconnect in the same session never reaches a new server.
3. `-Config @{ 'Section.key' = 'value' }` patches `%USERPROFILE%\ChiXie3\conf\config.ini` for
   the run and restores it afterwards (e.g. deformation, spacing, template).

## One-time setup

- ChiXie3 installed, logged in with automatic login, window maximized on the primary screen.
- The wireless device: 设备管理 › 无线 › **+** › IP `127.0.0.1`, port `23` (shows as
  `[自定义] guangku`, the first entry of the list).
- Leave the template you want active (the harness pastes into the first cell).

## Limits

- Click positions are for a 3840×2160 screen at 200% scaling (the same layout as 1920×1080 at
  100%). Use `-Shots` to save a screenshot after every step and re-measure if your layout differs.
- Keep the mouse and keyboard still while it runs (about 40 s). Every input checks that ChiXie3 is
  in front and refuses otherwise.
- Keep jobs short (a dozen characters, < ~200 lines); ChiXie3's Telnet streaming stalls on long
  jobs.
- **ChiXie3 output is not repeatable.** Even with 字符变形 off, every layout moves each stroke by
  0.1–1 mm (independently per stroke, not a scale). Compare captures structurally with
  `capture-diff` (stroke count, points per stroke) and statistically for shapes; byte equality is
  impossible. See docs/protocol.md.
