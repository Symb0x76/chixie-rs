# chixie-rs: full ChiXie3 reimplementation plan

Status: draft, 2026-11-08. The product and the repository are both **chixie-rs** (published
company-wide; briefly called "SAST Writer" on 2026-11-08).
Input: [chixie3-features.md](chixie3-features.md), the ChiXie3 spec. It has 140 features, their
status in this codebase, and how to build each one. The spec's roadmap R1–R12 is superseded by
this plan; §9 maps the old numbers to the new ones.

**Decisions (BOSS, 2026-11-08)**

- **Name: chixie-rs.** Window title, about box, binaries, app id and data folders all use it.
  Folder and file names are English only (UI text stays Chinese-first).
- **Common file formats, no home-made ones.** Wherever a standard format fits, we use it:

  | Data | Format | Why it fits |
  |---|---|---|
  | Documents | **DOCX** (Office Open XML). Text, paragraphs, fonts, sizes and colours map to Word runs; chixie-rs extras (seed, template, character ids, per-character tuning) live in a custom XML part (`customXml/chixie-rs.xml`) that Word preserves | opens in Word / WPS / LibreOffice; a Word file opened in chixie-rs is just a document without extras |
  | Handwriting fonts | **SVG font** (`<font>` / `<glyph d="…">` with open single-line paths, `horiz-adv-x` advances, `unicode` per glyph; metadata in `<metadata>`) | the de facto format for single-stroke plotter fonts (Inkscape Hershey Text, EMS); stroke order = path order |
  | Page templates | **SVG** page (the ruling / grid / table drawn at real size in mm) with the template's parameters as `data-*` attributes; tables also export to **XLSX** | any browser or Inkscape shows the template; chixie-rs reads the parameters back to keep it editable |
  | Settings, profiles, device profiles | **TOML** | already used for settings |
  | Bundles (several fonts / templates, full backup) | **ZIP** of the files above | universal |
  | Output | **G-code** (`.gcode`), **PDF** for print | standard |

- **Scope: writing first.** Phase 1 brings the 书写 side to full parity: editor, templates and
  tables, fonts, handwriting effects, device, and output. Plotting/laser, the 百宝箱 tools and the
  cloud extras come in later phases.
- **No AI.** 炽写AI (小炽), 拟人 and AI绘图 are out of scope.
- **Windows, macOS and Linux from day one.** Every milestone must build, test and package on all
  three before it counts as done.
- **Same or better.** Each feature matches ChiXie3's behaviour or improves on it. We don't copy
  its defects (see spec §4).
- **Internal tool.** No code signing, notarisation, installers with signatures, or self-update.
  Builds are distributed internally as archives / AppImage.
- **Cloud = fonts, templates and parameter sync.** No upgrade/pay/invitations and no use of the
  backend's image-to-table parser. Your team owns the backend, so where ChiXie3's transport is
  unknown or awkward (encrypted payloads, web pages), we add a documented JSON endpoint rather than
  mimicking ChiXie3.
- **Local first: whatever syncs is also a local file and can be exported.** The cloud is a sync
  target, never the only copy. Fonts, templates and the settings profile are saved locally as
  plain files the user can see, back up, export and import, and the app works fully from them
  offline:

  | Synced item | Local copy | Export / import |
  |---|---|---|
  | Fonts | SVG font files in `Documents/chixie-rs/Fonts/` (M4) | copy the file, or 导出 / 导入 on the font card |
  | Templates (note / grid / table, later drawing / laser) | SVG files in `Documents/chixie-rs/Templates/` (movable, extra folders, watched; same mechanics as fonts) | 导出 / 导入 in 模板管理 and the library cards; tables also as XLSX; bulk export as a ZIP |
  | Settings profile (参数同步) | `profile.toml` next to the settings | 选项 › 导出设置 / 导入设置 (a `.toml` file) |
- **No reverse-engineered ChiXie3 formats.** We don't read `.cdoc` documents or ChiXie3's
  encrypted table templates (both need ChiXie3's embedded keys). We still import the plain
  formats we already handle: `.cx` page templates, `config.ini`, and `.cset` device profiles.
- **Later: users' own fonts** (Phase 4): generate personal fonts from a few hundred samples of
  the user's handwriting with a generative model.
- **Not ported, because ChiXie3 has no working version or there is a safer replacement:**
  - AI (炽写AI / 小炽, 拟人, AI绘图), account commerce, self-update;
  - `.cdoc` and ChiXie3 table-template import;
  - placeholder tools 任务编程, 固件烧录, 多设备群控 (no handler in ChiXie3);
  - in-process Python plugins (replaced by external filters, Phase 3);
  - the CH340 driver installer;
  - 大文本模式 and 屏幕分辨率检测 (not needed natively);
  - 专属配置, 缺字上报 and 退出信息.

---

## 1. Where we stand

| Area | Done | Partial | Missing |
|---|---|---|---|
| Editor (text flow, caret, IME, undo, clipboard text) | core editing | font per selection, delete in tables | find/replace, context menu, alignment, formats, insert menu |
| Templates | 信纸, 作文格, `.cx` import | 双线, 分栏, 模板管理, 书写原点 | tables (the biggest gap), table import, data sources, thumbnails |
| Fonts | offline store, picker previews, cloud previews | library filters, missing handling | collect, 主字体 slots, mixing, daily free |
| Effects | spacing, offsets, base deformation | tails, TPS shape, smart offset | 字符微调, glyph clean-ups, 书写优化 |
| Typesetting | wrap, ruled and grid placement | 作文格 segmentation | 避头尾, alignment, indent applied, full/half width, rationality check, cell overflow |
| Device | USB, Telnet core, jog, params, sheet change | resume, alarms, network devices, `$$` | auto connect, error texts, N1 card, ESP32 provisioning, advanced settings |
| Output | G-code emit, export, park at origin | finish position, pen modes | 之/弓 planning, start-point modes, rotation, multicolour, 巡边, SD upload |
| Shell | Word-style ribbon, backstage, status bar, title bar per OS | settings, backstage | i18n, tray, window memory, packaging, updates |
| Plotting, 百宝箱, cloud extras | — | remote TF core | almost everything (Phases 2–3) |

**Strengths to keep:** seeded per-character randomness, an offline-first font store, one actor that
owns each device, golden tests pinned to ChiXie3 captures, and a fake Grbl backend with an
autotest.

**Weak points that block parity:**
1. `EditDoc` is one flat character sequence. It has no character attributes, paragraphs, cells or
   multiple flows.
2. `layout.rs` assumes one font and one text flow per page.
3. Job output is "pages of strokes → G-code" with no planning or transform stage.
4. Nothing is built or tested on macOS or Linux yet.

---

## 2. Target architecture

### 2.1 Crates

Split `chixie-rs-core` along the boundaries the features need. Each crate is introduced by the first
milestone that needs it, never as a big-bang move.

| Crate | Owns | Introduced |
|---|---|---|
| `chixie-rs-doc` | Document model (flows, runs, paragraphs, cells, papers), edit commands + undo, DOCX read/write with the custom XML part, `.chx` v1 migration | M2 |
| `chixie-rs-font` | font parsing/decrypt, local store, roles + mixing + fallback, glyph clean-ups | M4 (moves `font.rs`, `font_store.rs`) |
| `chixie-rs-layout` | typesetting (kinsoku, alignment, indent, width classes, grid, cells, overflow), handwriting effects, deformation | M3 (moves `layout.rs`, `handwriting.rs`, `deformation.rs`) |
| `chixie-rs-template` | template kinds (ruled, grid, table), editor ops, `.cx` / table import, data sources | M7 (moves `template.rs`, `chixie3_templates.rs`) |
| `chixie-rs-output` | stroke pipeline: transform → order/plan → pen/colour segments → G-code dialects; job description | M6 (moves `gcode.rs`, `calibration.rs`) |
| `chixie-rs-device` | transports (serial, Telnet, ESP3D HTTP), Grbl streamer, device actor, discovery, firmware dialects (Grbl_ESP32, ChiXie N1) | M1 (moves `transport.rs`, `grbl.rs`, `device.rs`, `esp3d.rs`, `discovery.rs`) |
| `chixie-rs-cloud` | API client, session, cloud fonts, cloud template library, parameter sync | M4 (moves `api.rs`, `session.rs`, `credentials.rs`) |
| `chixie-rs-import` | txt/docx/pdf/xlsx, table-frame detection from PDF/images, later DXF/SVG/images | M7–M8 |
| `chixie-rs-fontgen` | Phase 4: raster → stroke vectoriser, handwriting capture, import of model-generated glyphs | F1 |
| `chixie-rs-plot` | Phase 2 vector document and tools | P1 |
| `chixie-rs-gui`, `chixie-rs-cli` | apps | existing |

Rule: the domain crates know nothing about gpui. Everything a view needs comes from the
domain crates.

### 2.2 Document model (`chixie-rs-doc`, M2)

```
Document { id, seed, template: TemplateRef, origin: Origin, papers: Vec<Paper>, settings: DocSettings }
Paper    { kind: Note | Grid | Table, flows: Vec<FlowId> }          // one flow per page body or per table cell
Flow     { chars: Vec<Char>, paras: Vec<Para> }
Char     { id: u64, ch: char, attrs: AttrsId }                      // ids already exist (per-char randomness)
Attrs    { font_role|font_id, size, colour (pen), v_offset, tuning: Option<Tuning>, half_space, ... }
Para     { start, align, first_indent, ... }
```

- **Edits** are commands (insert, delete, set attrs, split/merge cell, …) with inverse operations
  for undo/redo. Undo is grouped like Word's (typing bursts, IME commit).
- **File format = DOCX** (decision above). Writer: runs carry font (role name), size, colour (pen)
  and character style; paragraphs carry alignment and first-line indent; table templates map to
  Word tables where they can. The custom XML part stores the document seed, template reference
  (embedded SVG), character ids (run-length encoded), tuning and origin. Reader: our own files
  round-trip exactly; plain Word files open as new documents (fresh ids and seed). Crate:
  `docx-rs` (read + write, verify) or a small OOXML writer on `zip` + `quick-xml`.
- **Migration:** today's `.chx` (v1) files open and are saved as DOCX; tested with fixtures.
- **Determinism:** the same document + seed + fonts gives byte-identical G-code. Golden tests pin
  this.

### 2.3 Output pipeline (`chixie-rs-output`, M6)

```
layout(doc) → PageStrokes (mm, per char id, per pen colour)
  → transform  (page rotation 奇/偶/全 180°, origin mode 纸张/框线/自定义, axis directions)
  → order      (之/弓 row order, optional travel minimisation, stroke simplification 书写优化)
  → segments   (pen colour groups → pen-change stops; sheet boundaries)
  → emit       (dialect: Grbl Z / servo / stepper-lift, Marlin, Bambu; pause-after-down, speeds)
  → Job        (pages × segments × lines, with a resume map line → (page, char id, stroke))
```

The **resume map** makes line-exact break-point resume possible. It is the core of M1/M2.

### 2.4 Device layer (`chixie-rs-device`)

- One actor per device, as today. Transports: serial (CH340/GD32/Bluetooth SPP), Telnet, and
  ESP3D HTTP (SD).
- Dialects: plain Grbl, Grbl_ESP32, and ChiXie N1 (its `$` settings card and advanced
  registers).
- **Streaming:**
  - character-counting send with `?` polling;
  - stall detection and resync from `Bf:`;
  - feed-hold pause, and stop as `!` → `0x18` → `$X` → pen up;
  - USB-drop recovery by re-scanning VID/PID and resuming from the map.

### 2.5 Cross-platform rules (all milestones)

- **CI:** GitHub Actions matrix on native `windows-latest`, `macos-latest` (arm64) and
  `ubuntu-latest` runners (no cross-compiling: aws-lc and wayland need native toolchains). Each
  job runs build, clippy, unit/golden tests and the GUI autotest; Linux runs it under `xvfb`.
- **Serial:**
  - Windows: COM ports;
  - macOS: `/dev/cu.*` (CH340 needs the vendor driver on older macOS; document it);
  - Linux: `/dev/ttyUSB*` / `ttyACM*` plus a `dialout` membership hint in the UI.
- **Credentials:** the `keyring` crate covers Windows Credential Manager, the macOS Keychain and
  Linux Secret Service. Fall back to "don't remember" when no secret service is running.
- **Chrome and shortcuts:** title bar per OS (done), ⌘ vs Ctrl key maps, macOS menu-bar basics.
- **Packaging (M10), unsigned internal builds** produced by CI as downloadable artifacts:
  - Windows: portable zip (exe + assets);
  - macOS: `.app` in a zip (first launch: right-click → 打开, or `xattr -dr com.apple.quarantine`);
  - Linux: AppImage.
- **Paths:** `directories` everywhere (already used). No hard-coded Windows paths.

---

## 3. Phase 1: writing parity

There are ten milestones. Each lists its scope, then its **done when** checks. Estimates are
engineer-weeks for one full-time engineer working with agent help; treat them as ±30%.

### M0 Foundations (1–2 wk) — done 2026-11-08 except CI on macOS/Linux (no remote yet)
- Commit the current work, then set up the CI matrix (§2.5) and make macOS and Linux build and
  pass the autotest.
- **Naming:** window title and about box `chixie-rs`, binaries `chixie-rs` and `chixie-rs-cli`,
  crates `chixie-rs-*`, env vars `CHIXIE_RS_*`, data directory `chixie-rs` (unchanged, so no
  migration).
- **Capture harness:** a scripted ChiXie3 run (`__COMPAT_LAYER=RunAsInvoker` with fake Grbl
  Telnet, see `docs/chixie3-analysis/README.md`). It takes a `.cx` / text / settings case and
  stores ChiXie3's G-code as a golden. Keep jobs under ~200 lines (Telnet stalls). This harness
  feeds every later parity check.
- **Hardware checklist** file for BOSS: connect, jog, job, pause, stop, USB drop, resume.
- **API capture for the cloud features** (mitmproxy against ChiXie3, as in HANDOFF.md): the template
  library (list / filter / sort / search / collect / 我的模板 upload / download) and
  `config/font_config` GET/POST. Write the shapes into `server-api.md`. If templates arrive
  encrypted with ChiXie3's embedded key, or only as a web page, agree a plain JSON endpoint with
  the backend team instead.
- **Done when:** CI is green on 3 OSes; the harness reproduces the existing protocol.md captures;
  the template and font_config APIs are documented (or the replacement endpoints agreed).

### M1 Reliable job loop (2–3 wk) — spec R1
- Auto-connect; Grbl error and alarm texts translated with an unlock action; stall detection and
  resync.
- USB-drop recovery by auto-rescan; finish position (写完位置: origin / custom / stay).
- 书写此页 and "from page N"; 工作区巡边 (3 pen modes × 2 ranges) and 绘制框架线.
- Wireless (Telnet) path tested on hardware for the first time (the core exists but is untested).
- Introduce `chixie-rs-device`.
- **Done when:**
  - fake link drops `ok` → resync without hanging;
  - pulling USB mid-job → reconnect prompt → continue;
  - a 20-page job runs on hardware with only sheet changes (BOSS).

### M2 Document model v2 + resume (3–4 wk) — spec R2, R3 (model part)
- `chixie-rs-doc` (§2.2): runs, paragraphs, flows, commands and undo; DOCX as the document format,
  with migration from today's `.chx`.
- Resume map (§2.3) and **断点续写**:
  - from the interrupted line;
  - from a clicked character (续写起始字符);
  - with "re-zero, then resume".
- **Done when:**
  - `.chx` v1 fixtures open identically and save as DOCX;
  - our DOCX round-trips byte-identical G-code; the file opens in Word, WPS and LibreOffice with the
    text, fonts and colours intact, and survives a save from Word with the extras preserved;
  - property tests show undo/redo inverts every command;
  - fake link killed at line k → resumed stream = pen up + travel + identical tail;
  - hardware: unplug mid-page, resume, no duplicated or missing stroke.

### M3 Typesetting parity and better (2–3 wk) — spec R3, R6
- Alignment (left/center/right/justify) and first-line indent (Tab and button, random width
  15.5–17 mm).
- **Real 避头尾** with hanging punctuation in 作文格; ChiXie3 only warns.
- Full/half-width classes; half space (半格空格); two-digit cells; 合并内容 (several characters in
  one cell).
- 作文格 word segmentation (`jieba-rs`, verify).
- Rationality check badge (位置合理性检测) as a status-bar item.
- Introduce `chixie-rs-layout`.
- **Done when:**
  - goldens for each alignment and indent;
  - 1,000-character fuzz: no line starts with a closing mark;
  - 作文格 and 信纸 captures still match within 0.3 mm.

### M4 Fonts (2–3 wk) — spec R4
- Font roles 主线/英文/符号/数字 (right-click card → 设为…); per-character fallback chain;
  weighted mixing keyed by character id.
- 收藏/取消收藏 (30-font cap is a server rule; surface the message).
- Library filters (中文/外文/手写/标准/公式符号/潦草/智能笔录制) and sort (收藏最多/时间最新/字数最多);
  今日试用字体.
- Missing glyphs: badge, list dialog, report.
- 字体优化 glyph clean-ups (统一字符尺寸, 重复轨迹优化, 异常宽度优化).
- **Font library folders.** Handwriting fonts live as plain files the user can find, back up and
  share, instead of being hidden in the app's config directory:
  - **default folder** in an easy-to-reach place: `Documents/chixie-rs/Fonts/` (Windows
    `%USERPROFILE%\Documents`, macOS `~/Documents`, Linux `$XDG_DOCUMENTS_DIR`), created on first
    run;
  - **customisable:** 选项 › 字体 lets the user move the default folder (fonts are moved with it)
    and add extra library folders (a shared network drive, a USB stick, a team folder);
  - one **SVG font** file per font (`<name>.svg`: one `<glyph>` per character with open
    single-line paths in stroke order and its advance; name, source (cloud / generated /
    imported), character count and cloud id in `<metadata>`), so copying a file is enough to
    share a font, and Inkscape's Hershey Text extension can use it too. Cloud downloads are
    converted to this format when saved. Read/write with `quick-xml` (verify);
  - folders are re-scanned on start and watched while running (`notify`, verify), so a font
    dropped into a folder appears in the 字体库 本地字体 tab without restarting;
  - "在文件夹中显示" on each font card; duplicate fonts (same cloud id or content hash) are shown
    once, with the folder they came from;
  - migration: fonts already in the current `font_store` directory move to the default folder on
    first run.
  - System-installed fonts (TTF/OTF) are deliberately **not** scanned: they are screen fonts made
    of filled outlines, not pen strokes, and write badly.
- 导出 / 导入 on font cards (an SVG font file, or several as a ZIP).
- Introduce `chixie-rs-font` and `chixie-rs-cloud` (fonts first; templates in M7, sync in M10).
- **Done when:**
  - a 1,000-character mix ratio is within ±3% of the target;
  - a missing character falls through the chain;
  - the collect request matches a mitmproxy capture;
  - a font file copied into an extra folder shows up within 2 s; moving the default folder keeps
    every font usable; the same tests pass on all 3 OSes.

### M5 Handwriting effects fidelity (2 wk) — spec R8
- Random tail removal (随机删除小尾巴), TPS shape deformation (字形变形 0–10), smart vertical
  offset (同向偏移).
- **字符微调** panel: per-character nudge, size and deform, stored in `Attrs.tuning`.
- 书写优化 (扩张数量/优化规则).
- 使用推荐参数 resets to ChiXie3's defaults.
- **Done when:**
  - deformation captures at levels 3/7/10 reproduce ChiXie3's magnitudes;
  - effects add < 50 µs per glyph;
  - the seeded golden is stable.

### M6 Output options (2–3 wk) — spec R5
- `chixie-rs-output` pipeline (§2.3): 之/弓 path planning, start-point modes (纸张/框线/自定义 with a
  marker on the page), and axis directions.
- Page rotation 不旋转 / 奇数页 / 偶数页 / 全部 180°.
- Pen-up/down per device type (Z, servo `M3S`, stepper lift), pause after pen down.
- **多色绘制:** colour per run, then pen-change stops (manual prompt) or auto pen change where the
  device supports it.
- Upload to SD and run (ESP3D).
- 笔锋测试 (Z lift along stroke tails), behind an experimental toggle.
- **Done when:**
  - captures for 弓 and for rotation match ours: glyph order and mirrored coordinates;
  - a two-colour page pauses once per colour switch;
  - the conflicts listed in spec §6.5 (`M3S` vs `G1G90 Z`, the `G4 P` dwell) are resolved by
    capture before shipping.

### M7 Templates and tables (6–8 wk) — spec R7, the largest milestone
- `chixie-rs-template`.
- Template editor parity: 笔记 columns, dual line, 分隔线/虚线; 作文格 parameters; quick sizes;
  the 书写原点 marker.
- **Tables:**
  - insert, draw and screenshot-frame tables;
  - merge/split cells; cell attributes (four paddings, alignment, rows); padding detection
    (内边距识别 / 占位符识别); same-type marks;
  - cell data sources (default, random number, random pick, reference);
  - per-cell layout with an **overflow policy** (方案1 shrink / 方案2 add rows / 方案3 ignore)
    instead of a modal state;
  - batch overflow handling.
- Import: Excel (`calamine`, verify) with merged cells.
- **Local table-frame detector** (PDF / image / screenshot → table template), in `chixie-rs-import`:
  1. Vector PDFs first: read the page's line and rectangle drawing operators (`lopdf`, verify)
     → exact segments in mm, no rasterising.
  2. Raster input (images, screenshots, scanned PDFs): grayscale → adaptive threshold →
     morphological opening with long horizontal / vertical kernels → line masks → Hough-free
     run-length segments (`image` + `imageproc`, verify).
  3. Both paths: snap near-collinear segments, intersect them into a grid, build cells by flood
     fill over the grid, and detect merged cells where an inner line is missing.
  4. Scale by known paper size or a user-drawn reference length; show the result for correction
     before saving.
  5. Placeholder text in cells (占位符识别) stays manual (no OCR in scope).
- ChiXie3's own encrypted table templates are **not** imported (decision: no reverse-engineered
  formats). Users re-create them with the detector or the editor.
- 批量导入 / 批量名单建页 (mail merge): one page per row.
- Page thumbnail panel.
- **Cloud template library (写字模板)** in 主页 › 模板, Word-style card grid like the 字体库:
  - 在线模板 / 我的模板 / 我的收藏; tags (全部模板 / 人物角色 / 教育学习 / 艺术结构 / 建筑风景 /
    动物植物 / 科技科学 / 交通玩具 / 其他模板); sort 时间最新 / 收藏最多 / 点赞最多; search; pages;
  - preview thumbnails rendered locally from the template data (no server image needed);
  - **template library folder** (default `Documents/chixie-rs/Templates/`, movable, extra
    folders, watched), one **SVG** file per template (the page drawn at real size in mm, the
    template parameters in `data-*` attributes), replacing today's single `templates.toml`;
    table templates also export to and import from XLSX; 导出 / 导入 per template or as a ZIP;
    cloud downloads are saved in this format;
  - 使用 downloads into the local template folder; 收藏 / 点赞; 上传到我的模板 for the user's own
    note / grid / table templates; 文件 › 从模板库新建;
  - offline: downloaded templates keep working; the online tabs show a login / offline notice;
  - the 绘图模板 / 激光模板 categories arrive with Phase 2 (P6).
- **Done when:**
  - a 2×2 form capture matches within 0.3 mm;
  - a cloud template downloads, renders the same as in ChiXie3, and an uploaded template appears
    under 我的模板 on a second machine;
  - the detector recovers every cell (merged cells included) on a fixture set of 10 forms:
    5 vector PDFs, 3 photos/screenshots, 2 scans;
  - shrink makes overflow fit;
  - xlsx with merges maps correctly;
  - a 30-row batch makes 30 pages.

### M8 Files and editor UX (3–4 wk) — spec R3 (UX part)
- Find/replace with options; Word-style context menu.
- Rich clipboard: Excel table paste and WPS RTF super/subscript.
- 字体/字号 per selection, 格式刷, character format (colour = pen, strike/underline as ChiXie3 has
  them).
- Single/double page view; show frame lines, line numbers and page numbers.
- Import txt/docx/pdf/xlsx into note, grid and table papers (`chixie-rs-import`). No `.cdoc`.
- **Print:** render pages to PDF (`printpdf`/own, verify) and hand it to the OS print dialog; 打印文字
  vs 打印背景图 (template only) modes.
- **Done when:**
  - import fixtures for each format;
  - the round trip DOCX → PDF is visually checked on 3 OSes;
  - replace keeps untouched character ids.

### M9 Device extras (2–3 wk)
- `.cset` import/export (pickle opcode reader only, never executes).
- Wireless discovery, manual network device, ESP32 Wi-Fi provisioning (配网).
- Firmware `$$` editor with labels.
- ChiXie N1 card (speed modes, 4 direction modes, 5 pen weights, beep, auto origin) and N1
  advanced settings (TMC currents, microsteps, stall detection, homing order, A-axis mode).
- Custom command buttons; command window.
- **Done when:**
  - mock ESP3D and N1 fixtures pass;
  - BOSS checks provisioning and one `$` write on hardware.

### M10 Shell, settings, sync and release (3–4 wk) — spec R11
- Settings dialog: all 7 pages at parity (常规, 写字偏好, 多色绘制, 绘图与激光, 字符变形, 主题与样式,
  页面旋转).
- **i18n:** zh-CN source plus zh-TW, ja, ru (ChiXie3's set) and en.
- Tray, close confirm, window geometry memory; backstage parity (账户 info page; 教程/反馈 links;
  更新日志).
- Crash and panic reporting with rotating logs; "关于" shows the git commit of the build.
- **参数同步 (parameter sync)**, 选项 › 常规 › 参数同步 开启/关闭:
  - what syncs: font roles (主线/英文/符号/数字), handwriting style (spacing, offsets, indent, path
    planning), deformation, page defaults, and the active template id. Device profiles stay local
    (they are machine-specific);
  - the ChiXie3-compatible subset (main font ids, deformation) goes through `config/font_config`,
    so a user switching between ChiXie3 and chixie-rs stays consistent; the full profile goes in a
    versioned JSON document (new backend endpoint, agreed in M0);
  - pull at login, push on change (debounced); each side stores `updated_at` and a revision. If
    both changed since the last sync, show a small dialog (使用云端 / 使用本机 / 查看差异) instead of
    silently overwriting;
  - works offline: changes queue and push at the next login;
  - the synced profile is always kept locally (`profile.toml`), and 选项 › 导出设置 / 导入设置
    writes and reads it as a `.toml` file, with or without an account.
- Unsigned internal builds for 3 OSes from CI (§2.5); no installer, no self-update.
- Migration assistant from ChiXie3 (plain formats only):
  - `config.ini` → settings;
  - `.cx` page templates, `.cset` device profiles.
- **Done when:**
  - CI artifacts start on fresh Windows/macOS/Linux machines;
  - the migration assistant imports BOSS's real ChiXie3 profile;
  - a setting changed on machine A appears on machine B after login; ChiXie3 picks up the font
    roles and deformation written by chixie-rs; a two-sided change shows the conflict dialog;
  - export → import of a profile, a template zip and a font zip on a second machine with no
    account gives the same output G-code.

**Phase 1 total: about 27–37 engineer-weeks** (cloud templates and parameter sync add about 2;
signing and self-update dropped). Internal release
**1.0 "writing"** after M10.

---

## 4. Phase 2: plotting and laser (about 10–14 engineer-weeks)

| # | Milestone | Scope | Done when |
|---|---|---|---|
| P1 | Plot canvas | `chixie-rs-plot` vector model; ribbon tab 绘图与激光; paper types (A4… custom); rulers, grid, snap, select/pan; basic shapes (point, line, polyline, curve, polygon, rectangle, circle, 3-point and centre arcs) with 图形属性 (position, line type, colour); layers (visible, lock, plot) | shape goldens; travel-ordered output |
| P2 | Imports | SVG (`usvg`), DXF (`dxf` crate; 作为整体导入 / 拆分导入), image → strokes (binarise + centre-line trace or hatch; `vtracer`/own, verify) | DXF/SVG fixtures; chord tolerance ≤ 0.05 mm |
| P3 | Text on canvas | 文本 / 文本框 / 表格 / 填充文本域 reuse `chixie-rs-layout` | text boxes write identically to the writing mode |
| P4 | Generators | 印章 (seal), 迷宫 (maze); 分形 / 立体字 optional | deterministic by seed |
| P4b | 插入 into writing pages | 图片 / 形状 / 文本框 / 表格 placed on writing pages reuse the plot items; 公式 / 图表 / 内容块 / 思维导图 stay deferred (P3 in the spec, rarely used) | inserted items plot in page order with the text |
| P5 | Laser mode | power/speed per layer, `M3`/`M4 S` dialect, fill hatching; safety interlock prompt | fake-link stream; BOSS checks on a laser head if available |
| P6 | Cloud 绘图 / 激光 templates | the M7 template library gains the 绘图模板 / 激光模板 categories; the plotting ribbon's 模板库 opens it filtered; upload own drawings to 我的模板 | a cloud drawing template opens as editable shapes and plots |

---

## 5. Phase 3: 百宝箱 tools (about 5–8 engineer-weeks)

| # | Item | Notes |
|---|---|---|
| T1 | Gcode预览执行器 | native previewer: parse, render pen-up/down, stats, step/run on the device. **Can be pulled into Phase 1** as a debugging aid (it is P1 in the spec). |
| T2 | 一笔画【简易】 | X,Y lines + pen_up/pen_down + loop count |
| T3 | 远程文件 | SD browser on top of `esp3d.rs` |
| T4 | 自动换纸控制 | job list, feed/eject, loop N, for supported feeders |
| T5 | 拓竹写字助手 | Bambu A1 export: offsets, pen positions, LAN upload (FTPS/MQTT, verify) |
| T6 | Filters instead of plugins | run an external program with the job JSON on stdin and read G-code or strokes back; no in-process code |

Account commerce (upgrade/pay/invitations) stays out of scope. Cloud templates and 参数同步 are
in Phase 1 (M7, M10) and Phase 2 (P6).

---

## 6. Phase 4: users' own fonts (research track, about 7–10 engineer-weeks)

Goal: users get handwriting fonts in their own style without the vendor cloud, generated from a
few hundred samples of their handwriting. The result is a normal font file saved into the font
library folder (M4), so everything downstream (layout, effects, G-code) works unchanged.
System-installed fonts are not a source (see M4: screen fonts write badly).

**The core problem:** chixie fonts are **single-line pen strokes** (polylines in drawing order).
Scanned samples and generative models produce **raster images**. Every path below therefore ends
in one shared raster → centre-line vectoriser (F1).

| # | Milestone | Scope | Done when |
|---|---|---|---|
| F1 | Raster → stroke vectoriser | Shared core in `chixie-rs-fontgen`: binarise → thinning (Zhang–Suen) → skeleton graph (endpoints, junctions) → spur pruning and junction repair → polylines → smoothing and simplification (Douglas–Peucker) → **stroke ordering** (CJK heuristics: top-to-bottom, left-to-right, horizontal before vertical, enclosures last) → pen-width estimate and the advance from the glyph box | round-trip test: render every glyph of a known stroke font at pen width → vectorise → Hausdorff distance < 0.3 mm at 10 mm character size on ≥ 95% of glyphs; stroke count within ±1 on ≥ 90% |
| F2 | Personal handwriting capture | (a) **Sample sheet:** a printable 作文格-style template listing ~200–300 characters chosen to cover common components, with 4 corner markers. The user fills it in, then scans or photographs it; perspective correction from the markers → cell crops → 256×256 glyph PNGs. (b) **Direct capture:** write the samples on screen with a pen tablet, which gives exact strokes and stroke order (the best training data and an instant usable partial font). | a phone photo of a filled sheet yields 95% clean crops; direct capture saves strokes and PNGs |
| F3 | Model-generated full font | Generate the remaining characters (GB2312 6,763, or the 6,900 set the cloud fonts use) in the user's style. Primary: **zi2zi-JiT** + LoRA fine-tune on the user's samples. Candidate alternative: **DA-Font** (few-shot GAN, ACM MM 2025), evaluated side by side. Output glyph PNGs → F1 → stroke font, saved as an SVG font in the font library folder. A review gallery lets the user regenerate or redraw bad characters before saving. | on a held-out set of 100 characters of the user's own handwriting: SSIM/LPIPS reported, and BOSS rates ≥ 80% as acceptable; a full font builds end to end on one consumer GPU |

**zi2zi-JiT facts** ([repo](https://github.com/kaonashi-tyc/zi2zi-JiT), read 2026-11-08):
- Conditional pixel-space diffusion transformer (JiT-B/16 and JiT-L/16) with content and style
  encoders.
- Input: 256×256 glyph PNGs named by character; a source font renders the content images.
- LoRA fine-tune on one font with about 200 characters: about 4 GB VRAM at batch 16, under an hour
  on one H100 (expect several hours on a consumer GPU).
- Sampling: `ab2`, 20 steps, cfg 2.6 (B/16).
- Code is MIT. Generated fonts carry an attribution clause when distributing a font product with
  more than 200 generated characters; that is not a concern for internal use.

**DA-Font** ([repo](https://github.com/wrchen2001/DA-Font), [paper](https://arxiv.org/abs/2509.16632)):
a few-shot GAN with component and relation attention (VQ-VAE stage + GAN training). Its licence and
pretrained weights still have to be checked before use.

**How it runs.** Training and generation need PyTorch and a GPU, so they live in a separate `uv`
project, `tools/fontgen/` (Python, pinned), not inside the Rust app. The GUI starts it as a
subprocess, reads JSON progress lines, and imports the resulting PNGs through F1. It can run on the
local GPU (CUDA or Apple MPS) or on a GPU machine over SSH. The app itself stays pure Rust.

**Make vectorising easy (key idea).** Our existing stroke fonts can be rendered at a fixed pen
width to make clean training/reference images. If the model learns to output uniform-width pen
strokes instead of brush-like shapes, F1's centre-line extraction becomes much more reliable.
Measure this in F3: fine-tune on pen-width renderings vs. on raw scans.

**Risks:**
- Stroke order cannot be recovered perfectly from images. Mitigations: the heuristics in F1, direct
  capture (F2b) for the most frequent characters, and per-character correction in the review
  gallery.
- Sample quality: phone photos need good lighting. The sheet's markers and an in-app check catch
  bad crops before training.
- Compute: users without a GPU need a shared GPU box (internal); direct capture (F2b) alone still gives a partial font of the captured characters.

---

## 7. How we work

- **Parity first, then better:** each feature gets a ChiXie3 capture or UI reference
  (`docs/chixie3-analysis/`) before coding. The acceptance check compares against it, then we
  apply the improvement listed in spec §4.
- **Tests:** unit + property tests in the domain crates; goldens for layout and G-code; the
  fake-device autotest in the GUI; BOSS's hardware checklist per milestone. CI runs on 3 OSes.
- **Definition of done (every milestone):**
  - CI green on Windows/macOS/Linux; clippy clean;
  - goldens updated on purpose only (`BLESS=1`);
  - `current-state.md` and the spec status column updated;
  - the hardware checklist signed off where the milestone touches devices.
- **Parallel tracks**, if more than one engineer or agent session works at once, with no file
  overlap:
  - A: doc/layout/fonts (M2 → M3 → M4 → M5);
  - B: device/output (M1 → M6 → M9);
  - C: shell/platform (M0 → M10 pieces).
  - M7 starts after M2 and M3; M8 after M2.
- **Performance budgets:** 100-page document relayout < 100 ms; 60 fps scrolling; streaming
  never blocks the UI thread.

---

## 8. Decisions

Resolved (BOSS, 2026-11-08):
1. No `.cdoc` or encrypted ChiXie3 table-template import (needs reverse-engineered keys).
2. Cloud = fonts, the template library and parameter sync; no commerce. Everything synced is
   also stored locally and can be exported / imported. Users' own fonts come from Phase 4.
3. Image/PDF → table uses a local detector (M7).
4. Internal tool: no signing, notarisation, installers or self-update.

Still open for Phase 1 (settled in M0 with the backend team):
- The template library and full-profile sync endpoints: reuse ChiXie3's (once captured) or add
  JSON endpoints.

Still open for Phase 4 (not blocking Phase 1):
1. Is there a GPU machine for F3 (local CUDA/MPS, or a shared box over SSH)?
2. May the cloud stroke fonts (downloaded with your account) be used as training/reference data
   for F1 tests and F3 fine-tuning? Internal use only.

---

## 9. Mapping from the spec's roadmap

| Spec | This plan |
|---|---|
| R1 Reliable job loop | M1 |
| R2 Resume | M2 |
| R3 Paragraph and character attributes | M2 (model) + M3 + M8 |
| R4 Font set | M4 |
| R5 Output options | M6 |
| R6 Kinsoku and grid polish | M3 |
| R7 Tables | M7 |
| R8 Effects fidelity | M5 |
| R9 G-code tools | M6 (SD) + T1–T3 |
| R10 Plotting editor | Phase 2 |
| R11 Platform | M0 + M10 |
| R12 Optional | M9 + Phase 3 (AI dropped) |
