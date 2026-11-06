# Handoff — chixie-rs

Rust rewrite of **ChiXie3**, a PyQt5/Nuitka handwriting-plotter controller the team relies on
but has no source for. Goal: faster native client, same cloud backend (team owns it; normal
user login). Reverse-engineered black-box, verified on the real machine.

## State (branch `master`, 7 commits, clean tree)

Proven end to end, **drawn on hardware**: CLI → login → font fetch → Fernet decrypt → glyph
parse → text layout → Grbl G-code → streamed → physical 你好 (COM4, 111 lines, no errors).

**Workspace (since GUI M0, 2026-11-06):** `crates/chixie-core` (lib `chixie_core`, no UI deps),
`crates/chixie-cli` (bin `chixie`), `crates/chixie-gui` (bin `chixie-gui`, gpui-kit spike).
GUI plan: `C:\Users\Symb0x76\.claude\plans\d-projects-rust-chixie-rs-docs-handoff-fluttering-iverson.md`.

## GUI M0 results (2026-11-06)
- **UI stack:** `gpui-kit = "=0.9.0"` (longbridge). It bundles `gpui-pre =0.3.11` (aliased `gpui`),
  `gpui-pre-platform =0.3.11` and `gpui-component 0.9.0` as one pin. That is the same
  gpui-component/gpui-pre the plan named, through the official umbrella crate, so we don't wire the
  platform crate ourselves.
- **Crate identity check passed:** `cargo tree -p chixie-gui -e normal` shows exactly one
  `gpui-pre v0.3.11`, and no `gpui-fast` or upstream `gpui`. Re-run it on any gpui-kit bump.
- **Spike results** (run `CHIXIE_SPIKE_AUTOTEST=1 target/release/chixie-gui.exe`, which prints one
  `AUTOTEST` line and exits):
  - CJK renders in labels, Input, Textarea and Button with the default theme font (system fallback).
    No bundled font is needed.
  - `Textarea` (multi-line) and `Input::mask_toggle()` (password) exist in gpui-component, so no
    wrapper is needed.
  - `canvas()` + `PathBuilder::stroke` draws 5,000 polylines, rebuilding the full path every frame:
    release avg 6.2 ms, worst 7.5 ms (about 160 fps); debug about 24 ms.
  - Worker thread → `async-channel` → `cx.spawn` → `Entity::update`: 21/21 updates arrive. Theme
    switch via `Theme::change(ThemeMode::Dark, None, cx)` works.
  - MSVC release build: 2m45s cold for the gui. `target/` is 8.6 GB after debug + release.
    Exe is 25 MB.
- **Not yet verified:** pan (drag) and zoom (wheel) on the preview. The handlers are wired, but
  synthetic Win32 clicks aren't delivered reliably to the gpui window, so check them by hand.
- **API notes (gpui-pre 0.3.11):** `on_scroll_wheel`/`ScrollWheelEvent` are deprecated in favour of
  `on_scroll`/`ScrollEvent`. `h_flex()` centers the cross axis, so add `.items_stretch().min_h_0()`
  for full-height panes. Window entry is `gpui_kit::open_window` (mounts `Root`). Call
  `gpui_kit::init(cx)` first.
- **Golden:** `crates/chixie-core/tests/golden.rs` pins the pre-refactor `text_to_gcode` output for a
  hand-built font fixture (`tests/fixtures/`, LF-locked by `.gitattributes`). Only regenerate it
  with `BLESS=1` on purpose.

## GUI M1/M2 progress (2026-11-06)
- **M1 capture path:** ChiXie3 is installed at `%LOCALAPPDATA%\Programs\ChiXie3\start.exe`
  (config in `%USERPROFILE%\ChiXie3\conf`). The binary has `MainWindow.set_export_gcode_btn_visible`,
  so there is an **Export G-code button**. What controls its visibility (role or mode?) can't be
  recovered from strings. Next step: open ChiXie3 and look for it. If it's hidden, fall back to
  com0com, then calipers.
- **Layout/emit split (done):**
  - `writing.rs` is replaced by `layout.rs` (`layout(text, &Font, &PageParams) -> Document`,
    machine-mm strokes, `missing` chars, optional `PageBox` wrap/paging) and `gcode.rs`
    (`emit(&Page, &DeviceProfile)`, `emit_document`).
  - With effects off it's byte-identical to the old pipeline: the golden test passes, and the
    `layout::legacy` oracle matches across 24 text×param combos.
  - Remove the oracle after a hardware check; keep the golden.
- **Grbl (done):** `Control::{pause, resume, reset, send_realtime}`, `?` polling every 250 ms
  (`status_interval`), `GrblStatus::parse`, and `stream(.., on_event: FnMut(StreamEvent))`.
  The CLI `stream`/`write` now also poll `?`. **Not hardware-tested yet.**
- **Device actor (done, `device.rs`):**
  - `spawn(opener) -> (DeviceHandle, async_channel::Receiver<Event>, JoinHandle)`.
  - Commands: Connect/Send/Jog/Pen/Unlock/BackOrigin/Zero/Run(Job pages)/NextSheet/Stop.
    Pause/Resume/Stop act on a shared `Control`.
  - Stop sequence: `!` → wait for `Hold:0`/`Idle` → `0x18` → `$X` → pen up.
  - An I/O error emits `Disconnected{page,last_line}` with no auto-resume.
  - 6 tests run against `grbl::fake::Fake`. **Not hardware-tested:** stop/reset sequence, `?`
    during streaming, USB-drop path.

Modules (`crates/chixie-core/src/`; CLI is `crates/chixie-cli/src/main.rs`):
- `transport.rs` — serial + telnet line links, CH340/GD32 port auto-detect.
- `grbl.rs` — char-counting streamer, error/alarm/cancel, realtime cmds.
- `esp3d.rs` — Grbl_ESP32 HTTP (SD list/upload/delete/run, status) — **untested** (board in BT mode).
- `discovery.rs` — arp + UPnP `description.xml` probe — **untested**.
- `font.rs` — manual Fernet decode + glyph parse (`[advance, X, Y]`).
- `writing.rs` — text → mm layout (Y-flip, pen cycles), origin, `--zero` G92.
- `api.rs` — backend client (rustls): login, collected_fonts, online_fonts, font.
- `main.rs` — CLI: `ports|discover|send|stream|fonts|write|sd`.

CLI highlights: `write "..." --font-id N --scale S --zero [--origin-x/-y] [--out f] [--port/--ip]`
(also offline `--blob-file --key-file` for testing without login); `fonts [--online] [--page N]
[--search s]` (online catalog is 1-based, 9/page). 12 unit tests pass, clippy clean.

**API-shape discovery method** (reused twice): run mitmproxy in local mode against `start.exe`,
patch then restore `ChiXie3\certifi\cacert.pem`, trigger the feature in the app, read the request
body. This is how get_font_access and get_online_fonts shapes were found.

## Key references (do not re-derive)
- Device protocol: [protocol.md](protocol.md) — firmware, Grbl streaming, pen lift, ESP32 cmds.
- Backend API + font format: [server-api.md](server-api.md) — endpoints, login fields, and the
  three decrypt gotchas (base64 junk after `=`, future timestamp, `[advance,X,Y]` glyph shape).
- Hardware quirk: memory `chixie-board-usb-drop` — CH340 drops under motor load, returns on a
  new COM number; use auto-detect.
- Verified device: Grbl_ESP32 1.3a, 115200 baud, 80 steps/mm, 300mm travel, pen `G1G90 Z0/Z2`.

## Open work (rough priority)
1. **Scale calibration** — `--scale` stands in for the compiled `PXScale` constant. Capture one
   page of G-code from the old app for known text; match X advance to pin mm-per-unit.
2. **Page layout** — multi-line wrap, char-spacing jitter, origin modes (`StartPointOfPaper/
   Frame/Custom`) from `%USERPROFILE%\ChiXie3\conf\config.ini` `[DeviceConfig]`.
3. **Multi-color** (`change_pen_signal` / pen-change pauses), **Marlin** streaming path.
4. **Wi-Fi paths** — telnet/HTTP-SD/discovery need the board switched STA (`[ESP110]STA` + creds);
   won't test until then.
5. **Cosmetic** — silence MSVC `linker_messages` warning.

## How to run (PowerShell; never put the password on the command line)
Creds via env `CHIXIE_ACCOUNT` (phone) / `CHIXIE_PASSWORD` (prompt with `Read-Host -AsSecureString`).
```
cargo run -p chixie-cli -- fonts                 # list fonts
cargo run -p chixie-cli -- write "你好" --font-id 10 --scale 3 --zero --out draw.gcode
cargo run -p chixie-cli -- stream draw.gcode     # auto-detects the CH340 port
cargo run -p chixie-gui                          # GUI spike
```
Inspect generated G-code extents before streaming; the board draws +X/+Y from the pen's start.

## Safety / redaction
- Account is a phone number; password only via secure prompt, never stored or logged.
- Font blobs are Fernet tokens tied to the session `backup_token`; captured raw fonts and live
  tokens are kept **out of the repo** and were deleted from scratch after use. The exposed login
  token lapses in ~5 days — log out in ChiXie3 to invalidate sooner.
- Traffic was captured with mitmproxy against the user's own account; the app's `certifi\cacert.pem`
  was patched then restored.

## Suggested skills for the next session
- `superpowers:systematic-debugging` — for any decrypt/stream regressions (it cracked the base64/
  timestamp/glyph issues this round).
- `rust` — idiomatic Rust patterns while extending layout/api.
- `superpowers:brainstorming` — before designing the GUI layer (egui/Slint/Tauri) or page-layout model.
- `context7` — for crate/API docs (reqwest, serialport, RustCrypto) when adding features.
