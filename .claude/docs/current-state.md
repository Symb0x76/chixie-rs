# Current state — chixie-rs (2026-11-07)

## Status
- Plan: `C:\Users\Symb0x76\.claude\plans\d-projects-rust-chixie-rs-docs-handoff-fluttering-iverson.md`.
- M0 done: workspace, golden, gpui-kit spike.
- M1 done: calibration (mm/unit = size/100) plus jitter, deformation and multi-line layout rules,
  all captured from ChiXie3 via a fake Grbl. See docs/protocol.md and memory `chixie3-capture-method`.
- M2 done (core side):
  - layout/gcode split; Grbl pause/status; device actor;
  - calibration; handwriting jitter; deformation; style TOML; CLI flags.
- Tests: 52 core (+1 opt-in keyring) + 2 goldens; GUI autotest passes; clippy clean.

## Open / VERIFY
- Space width; wrap rule (glyph origin vs right edge); first-line indent mode; `row_offset_style`
  variants.
- Exact deformation stroke-shift and shape-noise formulas.
- On hardware: `?` polling while streaming, the stop sequence, the USB-drop path.
- ChiXie3's Telnet stalls on long jobs, so keep capture jobs short.

## Next
- Done since: config.rs + ChiXie3 importer, credentials.rs (keyring), API heartbeat/logout.
- M3 done: GUI shell, login (keyring remember-me), fonts browser; offline FAKE/AUTOTEST modes.
- Next: M4 write view (textarea, params panel, preview canvas, export), then M5 device view.

