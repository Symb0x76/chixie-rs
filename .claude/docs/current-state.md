# Current state — chixie-rs (2026-11-06)

## Status
- GUI rewrite plan approved: gpui-kit (gpui-component) desktop client in a three-crate workspace.
  Plan file: `C:\Users\Symb0x76\.claude\plans\d-projects-rust-chixie-rs-docs-handoff-fluttering-iverson.md`.
- M0 done and committed (afafaba): workspace, golden, gpui spike.
- M2 partial: `layout.rs` + `gcode.rs` (byte-exact vs golden + legacy oracle), Grbl
  pause/realtime/status, `device.rs` actor. 26 core tests + golden pass, clippy clean.
- M1 capture path found: ChiXie3 has an Export G-code button (visibility gate unknown).

## Known issues / unverified on hardware
- `?` polling during streams, stop sequence (`!` → hold → `0x18` → `$X` → pen up), USB-drop path.
- Preview pan/zoom in the spike not exercised automatically.
- MSVC `linker_messages` warning (cosmetic).

## Next
- BOSS: find Export G-code in ChiXie3, export capture A (effects off) + captures B; or the caliper fallback.
- Hardware smoke: `chixie stream` a small file, then device actor stop/pause on the plotter.
- M2 rest: calibration, handwriting, deformation, config/keyring, CLI flags (after M1 semantics).
