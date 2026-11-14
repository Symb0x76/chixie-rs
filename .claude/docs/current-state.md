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
  FAKE mode uses `sim.rs` (simulated Grbl; `CHIXIE_RS_SIM_DELAY_MS`); AUTOTEST runs the
  2-page job incl. sheet change → Done.
- VERIFY on hardware (BOSS): connect COM4 via 自动, jog/pen, full job, pause (head stops),
  stop (pen lifts), USB unplug mid-job → toast + reconnect via 自动.
- After hardware test (2026-11-07), BOSS asked for three things; all are done:
  - The pen parks at X0 Y0 after a finished job (`device.rs` run_job).
  - Page templates:
    - `template.rs`: blank / ruled (fixed or random rows, dual line, columns) / grid (作文格);
    - `layout::Ruling` + per-page guides, which are never plotted;
    - `templates.toml`;
    - ChiXie3 `.cx` pickle import (`chixie3_templates.rs`).
    - Formats and VERIFY items are in docs/protocol.md.
  - Word-style shell:
    - quick access toolbar, filled 文件 tab → Backstage (新建 from template, 打开, 保存, 另存为, 导出,
      模板管理, 账户, 选项);
    - ribbon with large and small buttons, captions and a ↘ launcher (手写效果 ranges dialog);
    - 布局 tab with paper / orientation / margin menus and a template gallery;
    - font `Select`;
    - Word-blue accent (`ribbon::set_theme`).
  - AUTOTEST covers both templates and the backstage. Screenshots are in the scratchpad (t3-*.png).
- Template capture done (2026-11-07): ruled-row placement fixed to match, grid already matched.
- VERIFY still (not captured): random-row semantics,
  dual-line use, column fill order.
- Login is optional (2026-11-07):
  - The app opens straight to the editor.
  - Login is under 文件 › 账户 (or the 登录 button in the tab strip) and is only needed to
    download fonts.
  - Downloaded fonts are saved decrypted in `<config>/fonts/<id>.json` + `index.json`
    (`font_store.rs`). 字体库 has a 本地字体 tab (使用 / 删除), and the font picker lists local
    fonts plus collected fonts marked 云端.
  - The last font reloads from disk at start. Logging out keeps the document and the local fonts.
- Device: 参数设置 tab (profile, ChiXie3 import, $$ machine specs); Stop and Done both park at
  X0 Y0.
- Word 365 "Colorful" chrome (2026-11-07):
  - custom blue title bar (`appears_transparent`): app mark, quick access toolbar, centred
    "文档名 - 赤写", 登录/account, own min/max/close via `WindowControlArea` (Windows hit-test);
    the backstage keeps the bar.
  - light tab strip with plain 文件 tab; ribbon is a white rounded card; groups split by inset rules.
  - grey desk with drop-shadowed pages; 设备 task pane with titled header + close.
  - status bar: hover cells (字体 opens 字体库, 设备 opens the pane), job progress bar, Word-style
    two-sided zoom slider (100 % in the middle), click % to reset.
  - palette in `ribbon::chrome` (light + dark). AUTOTEST passes; clippy clean.
  - VERIFY (BOSS): window drag/snap/resize on the custom caption, maximise/restore icon swap,
    min size 760×480. Non-Windows: caption buttons rely on Windows hit-test areas only.
- Follow-up (2026-11-07): every `Switch` → Word-style `Checkbox` (accent via `tokens.primary`);
  font picker items are `font_preview::FontItem` (own-stroke preview of the name, or a sample
  when the font lacks a name character; 云端 badge), previews also in 字体库 rows, cached in
  `AppState.previews` and built off-thread. `字体 #N` cause: the last font is fetched at login
  before the font lists arrive, so the placeholder was saved; now `FontStore::rename` repairs
  names whenever a list arrives (BOSS's store has `1 → 字体 #1`; fixed on next login).
- 字体库 as a 3×3 card grid (2026-11-07): every tab pages by 9 (local/collected client-side,
  online server-side), no scrolling. Card = preview band + name/action + tags · 字数 · ★收藏;
  description as tooltip. Un-downloaded fonts: `cloud_preview.rs` fetches `font_preview` PNG
  off-thread, crops to ink, 120 px ink mask cached in `<fonts>/previews/<id>.png` (~8 KB),
  tinted per theme; skeleton while loading, "暂无预览" fallback. Font picker uses the same
  `font_preview::face`. New dep `image` (same instance as gpui's).
- Fixed: title bar buttons (QAT, 登录) were swallowed as caption drags on Windows — gpui's
  hit-test reports the Drag area if any ancestor hitbox is hovered; buttons now `.occlude()`.
- Platform-aware chrome (2026-11-07): Windows keeps the blue caption; macOS/Linux use
  gpui-component `TitleBar` (traffic lights / CSD buttons, Linux `window_border`, client
  decorations), a light caption, ⌘ shortcuts and macOS text navigation, keychain wording.
  Ctrl/⌘+S now saves (was only a tooltip). Windows autotest passes; macOS/Linux NOT compiled
  here (wayland pkg-config / aws-lc need native toolchains) — check on those machines.
- ChiXie3 feature spec (2026-11-07): `docs/chixie3-features.md` (140 features, status/priority/
  effort map, per-feature "recommended implementation", roadmap R1–R12, settings/endpoint
  appendices, conflicts). Evidence + extraction tools in `docs/chixie3-analysis/`.
  Designer labels of all 62 forms mined into `docs/chixie3-analysis/ui-labels.md` and merged
  (`[ui-form]` tag). 任务编程/固件烧录/多设备群控 are placeholders (no handler, inert as admin).
  Key conflicts to capture: 之/弓 path-planning meaning, `M3S` servo vs captured `G1G90 Z`,
  `G4 P` dwell after pen-down, 高矮 default 98–102 vs protocol.md.
- Reimplementation plan (2026-11-08): `docs/reimplementation-plan.md`. Product name **chixie-rs**
  (published company-wide; it was "SAST Writer" for a few hours on 2026-11-08). BOSS decisions:
  - writing first (Phase 1 = M0–M10, ~27–37 eng-weeks), then plotting/laser (Phase 2), 百宝箱
    tools (Phase 3), users' own fonts (Phase 4: handwriting capture + zi2zi-JiT/DA-Font generation
    → centre-line vectoriser; NO system-font scan);
  - common formats only: documents DOCX (+ customXml part for seed/ids/template), fonts SVG fonts
    (single-line), templates SVG (+ XLSX for tables), settings/profiles TOML, bundles ZIP;
  - English folder names: `Documents/chixie-rs/Fonts/`, `.../Templates/` (movable, extra folders,
    watched);
  - cloud = fonts + template library + 参数同步, local-first, everything exportable/importable;
  - no AI, no commerce, no `.cdoc`/encrypted ChiXie3 formats; local table detector; internal tool
    (no signing/installers/self-update); Windows/macOS/Linux from day one.
- **M0 done (2026-11-08), branch `m0-foundations`** (not merged; no git remote yet):
  - Naming: crates `chixie-rs-{core,cli,gui}`, binaries `chixie-rs` / `chixie-rs-cli`, env
    `CHIXIE_RS_{FAKE,AUTOTEST,SIM_DELAY_MS,ACCOUNT,PASSWORD}`, data dir and keychain service
    `chixie-rs` (unchanged from before, no migration). Hardware/ChiXie3 names unchanged.
  - CI `.github/workflows/ci.yml`: Windows/macOS/Linux build, clippy -D warnings, tests, GUI
    autotest (non-blocking until seen green), debug artifacts. NOT run yet: needs a GitHub remote.
  - Capture harness `tools/capture/` (`chixie-rs-cli fake-grbl`, `capture-diff`,
    `capture.ps1`): fresh un-elevated ChiXie3 per run; reproduces protocol.md structure exactly.
    Finding: pasted text varies 0.1–1.3 mm per stroke between identical runs (deform off).
  - Cloud API: `config/font_config` GET shape captured; ChiXie3 has no template-library backend;
    endpoint proposal in `docs/cloud-api-proposal.md` (needs backend team). `api_log.py` addon.
  - `docs/hardware-checklist.md` (14 checks).
- M0 merged to master (`a16c02c`), CI green on all three OSes.
- **M1 code done (2026-11-14), merged to master (`cc759b6`) before the hardware run:**
  - New crate `chixie-rs-device` (transport, grbl, device actor, esp3d, discovery, job).
    `DEFAULT_BAUD` lives in core `config`.
  - Stall fix: status replies used to reset the silence timer, so a lost `ok` hung forever. Now:
    no ack for 5 s + fresh `Idle` → resync (via `Bf:` or a `$G` marker), `Event::Resynced` toast.
  - Grbl error/alarm texts in Chinese (`grbl::error_text` / `alarm_text`); alarm toast with 解锁.
  - `Job.first_page`; ribbon 书写此页 / 从此页起 (caret page); `DeviceProfile.end_position`
    (写完位置: origin / origin-left 20 mm / stay / custom, through the axis map).
  - USB-drop recovery (BOSS decision: re-zero, then resume from a stroke): actor rescans 60 s,
    detects reboot (banner, or (0,0) after elsewhere), `Recovered { line, rebooted }`; resume line =
    pen-up ≥ 16 lines (planner depth) before the last ack; jog / 设为原点 work while waiting;
    a drop during the sheet prompt re-asks for the sheet.
  - 巡边 (3 modes × written/template area) and 绘制框架线 (merged, boustrophedon, dashes).
  - Auto-connect (`DeviceSettings.auto_connect`, 2 s poll, retries only when the port set changes).
  - Sim knobs `CHIXIE_RS_SIM_DROP_AT`, `CHIXIE_RS_SIM_DROP_OK`; autotest prints
    `AUTOTEST_DEVICE … recoveries=` and `AUTOTEST_PATROL`.
- Open: hardware checklist #1–#15 (BOSS), especially Telnet (#12, #15), the 20-page job (#14),
  and whether a real USB drop reboots the board (decides how often #11 needs a re-zero).
- Next: BOSS hardware run (fixes go straight on master) → M2 (document model v2 + resume).

