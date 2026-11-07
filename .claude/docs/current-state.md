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
- M4 replaced by the Word-style WYSIWYG editor (plan revision 2026-11-07):
  - W1 core: `editor.rs` (EditDoc, undo, IME marked range, UTF-16), per-character-id randomness,
    CharBox/caret/hit-test in layout.
  - W2–W5 GUI: ribbon (文件/开始/页面布局/手写效果/设备/视图) + status bar; `doc_view.rs` page
    canvas (multi-page, zoom, caret, selection, mouse, keys, clipboard, undo, IME via
    `EntityInputHandler`); 字体库 opens FontsView in a Dialog.
  - W6: `file_ops.rs` (.chx with ids+seed, .txt import, G-code export), draft autosave
    (`draft.chx` next to settings, restored at start).
  - AUTOTEST covers IME composition/commit, Delete/Undo via dispatched actions, export, draft.
- Known/deviations: no gpui-component Select/Slider (unverified APIs) — font picker is a button
  opening the 字体库 dialog, zoom is −/+/100%/页宽 buttons. Debug-build stack: keep ribbon tabs in
  separate `#[inline(never)]` fns (one big fn overflowed the 1 MB main stack).
- Manual by BOSS: Microsoft Pinyin on the page, mouse selection feel, clipboard with other apps,
  real-account login.
- M5 done (UI side): 设备 ribbon tab (连接/断开, 开始书写, 暂停/继续, 停止, 已换纸继续, 面板开关),
  right task pane `device.rs` (port choice 自动/COMx ★, jog pad 0.1/1/10 mm, 抬笔/落笔, 设为原点,
  回原点, 解锁, live state + position, console, job progress), start confirm with extents
  (300 mm travel warning), 换纸 dialog, disconnect toast with page/line.
  FAKE mode uses `sim.rs` (simulated Grbl; `CHIXIE_GUI_SIM_DELAY_MS`); AUTOTEST runs the
  2-page job incl. sheet change → Done.
- VERIFY on hardware (BOSS): connect COM4 via 自动, jog/pen, full job, pause (head stops),
  stop (pen lifts), USB unplug mid-job → toast + reconnect via 自动.
- Next: M6 (settings persistence polish, i18n, window geometry, icon, release build).

