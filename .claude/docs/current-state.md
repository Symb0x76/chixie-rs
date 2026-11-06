# Current state — chixie-rs (2026-11-06)

## Status
- GUI rewrite plan approved: gpui-kit (gpui-component) desktop client in a three-crate workspace.
  Plan file: `C:\Users\Symb0x76\.claude\plans\d-projects-rust-chixie-rs-docs-handoff-fluttering-iverson.md`.
- **M0 done (uncommitted):** workspace restructure, pre-refactor G-code golden, gpui identity check
  passed (single `gpui-pre 0.3.11`), spike proves CJK, Textarea, password Input, 5k-polyline canvas
  (~160 fps release), worker→channel→entity, MSVC release build. Details: `docs/HANDOFF.md`.
- Tests: 12 unit + 1 golden pass; `cargo clippy --workspace --all-targets -D warnings` clean.

## Known issues
- Preview pan/zoom handlers not exercised automatically (synthetic clicks unreliable) — manual check.
- MSVC `linker_messages` warning (cosmetic, pre-existing).
- `target/` is 8.6 GB with gpui debug+release.

## Next
- M1: calibration capture from ChiXie3 (effects-off capture A, single-effect captures B, or the
  caliper fallback).
- M2: core refactor (`layout.rs`/`gcode.rs`, handwriting, deformation, device actor, config, keyring).
