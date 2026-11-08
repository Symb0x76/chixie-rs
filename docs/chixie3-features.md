# ChiXie3 (炽写) feature specification and chixie-rs implementation guide

Status: 2026-11-07. Subject: ChiXie3 / 炽写 `alpha.3.0.310` (Windows build, Nuitka). Audience: the
chixie-rs team. Companion documents: [protocol.md](protocol.md) (device protocol and captured
G-code), [server-api.md](server-api.md) (backend), [HANDOFF.md](HANDOFF.md) (project state).

## 1. Overview

### 1.1 What ChiXie3 is

ChiXie3 is the desktop client of a pen-plotter "handwriting robot" (写字机). It turns typed text
into handwriting-like pen strokes using stroke fonts downloaded from the vendor's cloud, lays them
out on pre-printed paper (letter paper, composition grids, forms), and streams G-code to a
Grbl / Grbl_ESP32 / Marlin controller over USB serial, Bluetooth SPP (as a COM port) or Wi-Fi
(Telnet + the ESP32 web server). It also has a small vector drawing editor for plotters and
laser engravers and a "toolbox" of add-ons.

The window has three top-level modes, shown as ribbon tabs (UI, shot 01):

| Tab | Mode | Content |
|---|---|---|
| 主页 | backstage | full-window page: 字体 (font library), 模板 (template library), 教程, 账户, 设置, 反馈 |
| 书写 | writing | Word-like page editor on 笔记 / 表格 / 作文格 paper, font/format ribbon, job box, device drawer |
| 绘图与激光 | plotting | vector canvas with rulers/grid, shapes, DXF/SVG/image import, text fields, generators |

A fourth, contextual ribbon category (页面模板 › 模板管理) appears while editing page templates.

### 1.2 Tech stack (ChiXie3)

- Python + PyQt5, `SARibbonMainWindow` ribbon, `qfluentwidgets` / `qfluentwidgetspro` widgets
  (`RoundMenu`, `InfoBar`, `SegmentedWidget`, `StateToolTip`), compiled with Nuitka
  (A §Main window; C summary).
- Editor: `QGraphicsScene` / `QGraphicsView` with one `TextItem` per character (C).
- QtWebEngine pages for: font library, G-code tool, user guide (A).
- Serial: `QSerialPort` + pyserial; Wi-Fi: `telnetlib`; ESP32 HTTP: `requests`; websocket client
  for the board and for the iFlytek Spark AI (A).
- Parsers: python-docx, PyMuPDF (`fitz`), openpyxl, chardet, `ezdxf`, `svgelements`, jieba,
  numpy, PIL, pywt (Haar wavelets), a thin-plate-spline module (B, C).
- Persistence: QSettings INI (`%USERPROFILE%\ChiXie3\conf\config.ini`), Python pickles (`.cx`,
  `.cset`, QVariant `PyQt_PyObject`), Fernet encryption with keys hard-coded in the binary
  (table templates, `.cdoc`, font optimisation). The keys exist; this document never copies them.

### 1.3 How this specification was produced

1. **Static analysis** of the Nuitka constant pools, split into three slices:
   [A_device_shell.md](chixie3-analysis/A_device_shell.md) (device, G-code, toolbox, shell; cited "A @hex"),
   [B_templates_plotting.md](chixie3-analysis/B_templates_plotting.md) (templates, plotting, settings; "B @hex"),
   [C_writing.md](chixie3-analysis/C_writing.md) (writing editor, typesetting, fonts, output; "C @hex").
   Docstrings, UI strings, identifiers and config keys survive; numeric literals and Designer form
   labels mostly do not. How to regenerate: [chixie3-analysis/README.md](chixie3-analysis/README.md).
2. **Live UI walk** of the running app (un-elevated, 2026-11-07): exact labels, defaults, menus,
   dialogs ("UI"), in [ui-walk.md](chixie3-analysis/ui-walk.md). Screenshots are not kept in the repo
   (they show account details).
3. **The real settings file** `config_sanitized.ini` (keys and the user's values; secrets removed).
4. **Wire captures** already in protocol.md / server-api.md (fake-Grbl Telnet capture of real
   output; mitmproxy captures of the API). These beat static guesses.

Rule of precedence when sources disagree: wire capture > live UI > static docstring > static
identifier order. Every conflict found is listed in §6.5.

### 1.4 Confidence legend

| Tag | Meaning |
|---|---|
| **[cap]** | confirmed by a wire capture or hardware test |
| **[UI]** | seen in the live UI |
| **[high]** | static: docstring or exact UI string |
| **[ui-form]** | exact Designer form label (`ui.UI_*` constants, `docs/chixie3-analysis/ui-labels.md`): user-visible text and control names in form order; no defaults, ranges or enum values |
| **[med]** | static: inferred from identifier names / order |
| **[low]** | guess from weak evidence |
| **(inferred)** | the sentence is our reading, not the source's |
| **VERIFY** | must be confirmed (capture, hardware, or the vendor) before relying on it |

### 1.5 What could not be determined

- Numeric defaults and ranges that only exist as code literals (spin-box ranges, speed tiers of
  `speed_dict`, pen-weight depths, TPS scale, deformation stroke amplitudes, heartbeat interval).
  The Designer forms give labels and control names only, never `setRange`/`setValue` literals or
  combo-box item lists filled in code [ui-form].
- Enum integer encodings (e.g. `start_point_mode=11`, `row_offset_style=1`, `PenChangingMode=1`),
  except where the live UI pins them (`path_planning_type=2` = 弓字型, see §3.7).
- Qt Designer labels are now recovered: the `ui.UI_*` form modules (a constant region around
  `0xa01…`–`0xa04…` in this build, outside the three slices) were mined into
  `docs/chixie3-analysis/ui-labels.md` and merged into the sections below as [ui-form]. What
  stays unknown is everything a form does not carry: spin-box ranges and defaults, combo-box items
  filled in code (e.g. the 运动模式 / 传动类型 / 驱动模式 / 设备类型 lists), which label belongs to which
  hint text where a form has several, and enable/disable logic.
- The ChiXie N1 firmware update transfer protocol and file source; meanings of the N1 numeric
  `$190…$253` registers.
- 任务编程, 固件烧录, 多设备群控 are placeholders (no handler; inert even as administrator), so there
  is nothing to port.
- Where the user plugin (`plugin/test.py`) is invoked besides the route optimiser; the
  `.cdoc` serialisation; server shapes of most non-font endpoints.
- 拟人 and AI绘图 (plotting ribbon): buttons only; no handler or dialog found.

## 2. Feature map

Status is chixie-rs as of 2026-11-07. Priority: **P0** core writing loop (type → lay out on
paper → write on the machine, reliably), **P1** daily-use features of real users, **P2** valuable
but secondary, **P3** niche / vendor-specific / not worth porting soon. Effort: **S** ≤ 2 days,
**M** ≤ 2 weeks, **L** more.

| Area | Feature | Description | chixie-rs | Pri | Effort |
|---|---|---|---|---|---|
| Editor | 页面编辑器 | WYSIWYG pages, caret, selection, mouse, keys | Done (`editor.rs`, `doc_view.rs`) | P0 | — |
| Editor | 剪切/复制/粘贴 | internal items + plain text; Excel XML; WPS RTF super/sub | Partial (plain text) | P1 | M |
| Editor | 全选 | Ctrl+A per template type | Done | P0 | — |
| Editor | 撤销 | ChiXie3: not implemented | Done (undo/redo, better) | P0 | — |
| Editor | 删除/退格 | incl. "清空所选单元格" confirm for tables | Partial (no tables) | P0 | S |
| Editor | 方向键/Shift 选择 | caret and selection movement | Done (text); tables missing | P0 | S |
| Editor | 半格空格 | modifier+Space inserts half space | Missing | P2 | S |
| Editor | 查找和替换 | plain substring find / replace / replace all | Missing | P1 | S |
| Editor | 右键菜单 | page / origin / table / base menus | Missing | P1 | M |
| Editor | 插入菜单 | 表格 公式 图表 图片 文本框 形状 内容块 思维导图 (mostly stubs) | Missing | P3 | L |
| Editor | 字符格式 | 倾斜 下划线 删除线 颜色 上/下标 标题 清除格式 | Missing | P2 | M |
| Editor | 格式刷 | copy format (UI only) | Missing | P3 | S |
| Editor | 字体/字号 per selection | font and size of selected characters | Partial (document-wide) | P1 | M |
| Editor | 对齐 | left / centre / right / justify; vertical in cells | Missing | P1 | M |
| Editor | 缩放/单双页 | Ctrl+wheel zoom, single/double page preview | Partial (zoom; no double page) | P2 | S |
| Editor | 显示框线/行号/页码 | toggles | Partial (template grid toggle) | P2 | S |
| Editor | 大文本模式 | visible-item culling for big docs | Not needed (native) | P3 | — |
| Templates | 笔记 (信纸) | ruled rows, fixed/random count, margins | Done (`template.rs` Ruled) | P0 | — |
| Templates | 双线文档 | dual-line rows, dashed partition | Partial (band; use VERIFY) | P2 | S |
| Templates | 笔记分栏 | columns, widths, dividers, 异栏同型 | Partial (equal columns) | P2 | S |
| Templates | 作文格 | composition grid | Done (`template.rs` Grid) | P0 | — |
| Templates | 表格模板 | parent cells, internal rows, margins, flags | Missing | P1 | L |
| Templates | 插入表格 / 绘制表格 / 截图生成 | build table frames | Missing | P2 | M |
| Templates | PDF/图片 → 表格框架 | cloud table recognition | Missing | P2 | M |
| Templates | Excel导入配置 | per-page Excel range | Missing | P2 | S |
| Templates | 同型标记 | linked cells across pages | Missing | P3 | M |
| Templates | 数据源配置 | default / random / custom cell data | Missing | P2 | M |
| Templates | 合并/拆分单元格 | merge, full split, split handles | Missing | P2 | M |
| Templates | 内边距识别 | detect left padding from background image | Missing | P3 | S |
| Templates | 页面缩略图面板 | page list, background preview, opacity | Missing | P2 | S |
| Templates | 模板管理 UI | create / save / delete / export, validation | Partial (`templates_view.rs`) | P1 | S |
| Templates | `.cx` import | note / composition pickles | Done (`chixie3_templates.rs`) | P1 | — |
| Templates | 表格模板 import | Fernet + pickle table templates | Missing | P2 | M |
| Templates | 书写原点 | origin marker, 纸张/框线/自定义 | Partial (writable top-left only) | P1 | M |
| Templates | 模板库 (cloud) | online templates: 绘图/激光/写字 | Missing | P3 | M |
| Fonts | 字体库 browse | 在线字体 / 我的收藏 / 我的录制, filter, sort, search, pager | Partial (no 录制, no filter/sort UI) | P1 | S |
| Fonts | 收藏/取消收藏 | `font/collect_a_font`, 30-font cap | Missing | P1 | S |
| Fonts | 主字体 slots | 主线/主英文/主符号/主数字 font | Missing | P1 | M |
| Fonts | 字体调用/混用 | per char-type fallback lists; weighted mix | Missing (one font) | P1 | M |
| Fonts | 缺字处理 | auto-substitute, 查看缺失字符 dialog | Partial (missing list in layout) | P1 | S |
| Fonts | 今日试用字体 | daily free font for ordinary users | Missing | P3 | S |
| Fonts | 字体下拉框 | regions, hover preview | Partial (`font_preview.rs` picker) | P1 | S |
| Fonts | 本地字体 (offline) | ChiXie3 has none | Done (`font_store.rs`, better) | P0 | — |
| Fonts | 录制字体 | WeChat mini-program guide only | Not planned (link only) | P3 | S |
| Effects | 字符间隔/空格/垂直偏移/首字不对齐 | layout-time jitter | Done (`handwriting.rs`) | P0 | — |
| Effects | 智能垂直偏移 | position-proportional drift | Partial (guess, VERIFY) | P2 | S |
| Effects | 首行缩进 | Tab / button inserts random indent | Partial (style field, not applied) | P1 | S |
| Effects | 基础变形 | scale, rotate, stroke jitter, tail removal | Partial (`deformation.rs`, tails missing) | P1 | S |
| Effects | 字形变形 (TPS) | thin-plate-spline warp with similarity check | Partial (slant + noise model) | P2 | M |
| Effects | 字符微调 | per-char font/size/offset/thin/fat/apply-to-same | Missing | P2 | M |
| Effects | 重复轨迹/异常宽度优化 | glyph clean-ups | Missing | P2 | S |
| Effects | 书写优化 (路径) | Douglas-Peucker / Catmull-Rom / uniform | Missing | P2 | S |
| Typeset | 输入排版 / 换行 | width-based row fill, pages | Done (`layout.rs`) | P0 | — |
| Typeset | 作文格 + 分词 | jieba word blocks kept on one row | Partial (grid; no segmentation) | P2 | M |
| Typeset | 合并内容 | several chars in one grid cell | Missing | P3 | S |
| Typeset | 全角/半角 | auto conversion | Missing | P2 | S |
| Typeset | 位置合理性检测 | warn on bad line-start/end punctuation | Missing | P1 | S |
| Typeset | 避头尾 | automatic kinsoku (ChiXie3 lacks it) | Missing | P1 | S |
| Typeset | 表格单元格排版 / 溢出 | rows in cells, 3 overflow plans, batch | Missing | P1 | L |
| Device | 设备列表 / 参数 | named profiles, firmware, type, speed, pen, axis map | Done (`device_params.rs`) | P0 | — |
| Device | `.cset` import/export | pickled device params | Partial (config.ini import) | P2 | S |
| Device | USB 连接 / 自动检测 | CH340 / GD32, baud probe | Done (`transport.rs`) | P0 | — |
| Device | 自动连接机器 | connect when a port appears | Missing | P1 | S |
| Device | CH340 驱动检查 | download + install prompt | Not planned | P3 | — |
| Device | Grbl 报警解锁 | Alarm → `$X` prompt | Partial (解锁 button) | P0 | S |
| Device | Grbl 错误翻译 | `error:N` → Chinese | Missing | P1 | S |
| Device | 无线 (Telnet) | ESP32 only | Done (core) / untested | P1 | S |
| Device | 无线发现 | arp + UPnP `description.xml` | Partial (core, untested) | P2 | S |
| Device | 手动添加网络设备 | name/IP/port | Partial | P1 | S |
| Device | ESP32 配网 | STA / AP / BT / none | Missing | P2 | M |
| Device | 固件参数 ($$) | easy/expert editor, `$RST=*` | Partial (read only) | P2 | M |
| Device | 炽写ESP主板设置 | PWM, axis follow, CoreXY, radio | Missing | P3 | M |
| Device | 炽写 N1 card | speed/direction/pressure modes, beep | Missing | P2 | M |
| Device | 炽写设备高级设置 | N1 TMC driver + per-axis parameters (current, microstep, stall, offsets) | Missing | P3 | M |
| Device | N1 固件更新 / 设备测试 | `$WHO`, bootloader, admin tests | Not planned | P3 | — |
| Device | 手动控制 | jog, pen, origin, home, Z trim | Done (`device.rs` GUI) | P0 | — |
| Device | 命令窗口 / 自定义命令 | console, custom buttons | Partial (console) | P2 | S |
| Device | 坐标位置显示 | status-bar position | Done | P1 | — |
| Jobs | 书写管理 | page list, start/pause/resume/stop | Done (`device.rs`) | P0 | — |
| Jobs | 书写此页 | write a chosen page | Partial | P1 | S |
| Jobs | 换纸 | sheet change between pages | Done | P0 | — |
| Jobs | 断点续写 | resume from a chosen character | Partial (page/line toast) | P0 | M |
| Jobs | 多色绘制 / 换笔 | manual / automatic pen change | Missing | P2 | M |
| Jobs | 页面旋转 (奇偶页) | 180° rotation of odd/even pages | Missing | P2 | S |
| Jobs | 自动换纸设备 | Marlin sheet feeder, loops | Missing | P3 | M |
| Jobs | 工作区巡边/绘制框架线 | trace bounds; draw template lines | Missing | P1 | S |
| Jobs | 上传SD卡书写 | upload page to TF and run | Partial (core `esp3d.rs`) | P2 | S |
| G-code | 书写 G-code | initial/body/end, signed coords | Done (`gcode.rs`) | P0 | — |
| G-code | 路径规划 之/弓 | row order | Missing | P1 | S |
| G-code | 起点模式 | 纸张/框线/自定义 origin | Missing | P1 | S |
| G-code | 写完位置 | 原点 / 原点左2cm / 任务结束点 | Partial (always X0 Y0) | P1 | S |
| G-code | 抬落笔 per device type | stepper / servo / magnet / laser / 3D / custom | Partial (Z + custom) | P1 | S |
| G-code | 导出 G-code | one file per page | Done (`file_ops.rs`) | P0 | — |
| G-code | 笔锋测试 | Z lift along stroke tail | Missing | P3 | S |
| G-code | 路径优化插件 | user Python RouteOptimizer | Not planned (replace) | P3 | M |
| Plot | 绘图画布 | rulers, grid, snap, zoom, select/pan | Missing | P2 | L |
| Plot | 纸型设置 | standard / custom paper, presets | Missing | P2 | S |
| Plot | 基本图形 | point line rect circle/ellipse polyline curve polygon arc | Missing | P2 | M |
| Plot | 图形属性 / 线型 | numeric edit, CAD linetypes | Missing | P2 | M |
| Plot | 图层 | add / delete / merge / order / visible / lock | Missing | P3 | M |
| Plot | 导入 CAD (DXF) | ezdxf entities, units, fit | Missing | P2 | M |
| Plot | 导入 SVG | svgelements paths | Missing | P2 | M |
| Plot | 导入图片 | binarise, scan-line hatch | Missing | P2 | M |
| Plot | 文本 / 文本框 / 表格 | handwriting text fields on canvas | Missing | P2 | L |
| Plot | 酷玩: 印章 / 迷宫 | seal and maze generators | Missing | P3 | M |
| Plot | 酷玩: 分形 / 立体字 | stubs | Not planned | P3 | — |
| Plot | 拟人 / AI绘图 | buttons only | Not planned | P3 | — |
| Plot | 模板库 (绘图) | online drawing templates | Missing | P3 | M |
| Plot | 激光模式 | `DrawOrLaser` switch, no laser options | Missing | P3 | M |
| Toolbox | 拓竹写字助手 | Bambu Lab FTPS upload | Missing | P3 | M |
| Toolbox | Gcode预览执行器 | preview / run G-code (web page) | Missing | P1 | M |
| Toolbox | 插件管理 | in-process Python | Not planned | P3 | — |
| Toolbox | 任务编程 | placeholder in ChiXie3 (no handler) | Not planned | P3 | — |
| Toolbox | 自动换纸控制 | see Jobs | Missing | P3 | M |
| Toolbox | 固件烧录 | placeholder in ChiXie3 (no handler) | Not planned | P3 | — |
| Toolbox | 一笔画【简易】 | coordinate list → one path | Missing | P3 | S |
| Toolbox | 多设备群控 | placeholder in ChiXie3 (no handler) | Not planned | P3 | — |
| Toolbox | 远程文件 | TF card browser | Partial (core) | P2 | M |
| Toolbox | 字体优化 | unify glyph sizes, clean routes | Missing | P2 | S |
| AI | 炽写AI (小炽) | Spark LLM edit/draft menus | Missing | P3 | M |
| Account | 登录 / 自动登录 | account + password | Done (optional, keyring) | P0 | — |
| Account | 心跳 / 在线检测 | heartbeat; revoke on failure | Done (`session.rs`) | P1 | — |
| Account | 用户信息 | role, VIP, expiry | Partial (account page) | P2 | S |
| Account | 升级/注册码/邀请码 | pay, license, invitation | Not planned | P3 | M |
| Cloud | 参数同步 | `config/font_config` | Missing | P3 | S |
| Cloud | 专属配置 | license-bound config.ini | Not planned | P3 | — |
| Cloud | 自动升级 | MD5 diff download | Missing (replace) | P2 | M |
| Cloud | 缺字上报 / 退出信息 | `user/collect_*` | Not planned | P3 | S |
| Settings | 设置 dialog (7 pages) | 常规 写字偏好 多色 绘图 变形 主题 页面旋转 | Partial (`style_dialog.rs`, options) | P1 | M |
| Files | `.cdoc` | ChiXie document | Missing (`.chx` instead) | P2 | M |
| Files | 导入 txt/docx/pdf/xlsx | parsers + overwrite/append | Partial (txt) | P1 | M |
| Files | 批量导入 (批量建页) | one page per list line | Missing | P2 | S |
| Files | 打印文字/打印背景图 | QPrinter | Missing | P3 | M |
| Files | 草稿自动保存 | ChiXie3 has none | Done (better) | P1 | — |
| Shell | ribbon / QAT / status bar | Word-like chrome | Done (`ribbon.rs`, `app.rs`) | P0 | — |
| Shell | 主页 backstage | fonts, templates, tutorial, account, settings, feedback | Partial (`backstage.rs`) | P1 | S |
| Shell | 托盘 / 关闭确认 / 窗口记忆 | tray, close confirm, geometry | Partial (M6 planned) | P2 | S |
| Shell | 屏幕分辨率检测 | restart prompt | Not needed | P3 | — |
| Shell | 教程与售后 / 反馈 / 在线客服 | web links | Missing | P3 | S |
| Shell | 更新日志 | after upgrade | Missing | P3 | S |
| Shell | 异常提示 | global exception dialog + log | Partial (logging) | P1 | S |
| Shell | 多语言 | CN / EN `.qm` | Missing (M6) | P2 | M |

## 3. Features by area

Conventions: "chixie-rs today" names modules under `crates/chixie-core/src` (core) and
`crates/chixie-gui/src` (GUI). New crates are named explicitly with the reason. The workspace
already depends on: `serde`, `serde_json`, `toml`, `reqwest` (blocking, rustls), `serialport`,
`aes`/`cbc`/`hmac`/`sha2`/`base64` (manual Fernet), `async-channel`, `directories`, `keyring`,
`image` (png), `gpui-kit` (gpui + gpui-component), `clap`, `thiserror`, `log`.

Cross-cutting design rule for chixie-rs, referred to below as **the document model**: one
`Document` model in core (`editor.rs` + `layout.rs`), from which both the preview and the G-code
are produced, so "what you see is what is drawn". Per-character randomness is keyed by stable
character ids and a persisted seed (`handwriting::rng_for(seed, salt, id)`), so editing never
reshuffles the rest of the page.

### 3.1 Writing editor (书写)

#### Page editor model and view (书写编辑器)
**ChiXie3:** A `QGraphicsScene` holds pages (`BasePaperItem` → `NotePaperItem`,
`TablePaperItem_V2`, `CompositionPaperItem_V2`); each page holds input rows (`InputCellItem`) and
each row holds `TextItem` / `SpaceItem` / `LineBreakItem` items plus a blinking `CursorItem`.
Typing goes through a hidden `QLineEdit` (`UserInputBox`) whose `textChanged` carries IME text.
Selected characters get a `#FFFFCC` background; warnings `#D24D57` (C @95398af, @953a876).
Ctrl+wheel zoom anchored under the mouse; status bar zoom spinner (default 100%) [UI]; preview
modes `SinglePagePreview` / `DoublePagePreview` (config `PreviewMode=1`) [high].
**How it works there:** every character is a scene item with its own deformed stroke path
(`TextItem.getWritingPath` scaled by `PXScale`); layout is incremental and threaded
(`InputTypeSetting` as `QRunnable`, `Variable.InTimeConsumingNesting` while busy). A "large text"
mode hides items outside the viewport (`set_item_quantity_mode_of`, `update_visible_items`)
because the per-item scene is slow (C §View).
**chixie-rs today:** `editor.rs` (`EditDoc`: chars + stable ids, caret, selection, IME marked
range, UTF-16 bridge, undo/redo), `layout.rs` (`layout_ids` → `Document` with `CharBox`,
`LineInfo`, caret hit-testing), `doc_view.rs` (gpui canvas: stacked pages, live handwriting,
mouse, keyboard, clipboard, IME through `EntityInputHandler`, zoom, fit width).
**Recommended implementation:** keep the text-model + full relayout design; it is simpler and
deterministic. Add what is missing in this order: (1) per-character attributes as a run list
parallel to `ids` (`Vec<CharAttrs>` with font id, size, colour, flags; `Arc` shared runs to
keep memory low); (2) a cell/region addressing layer for tables (§3.2); (3) double-page view
(two pages per desk row; pure view change in `doc_view.rs`). Re-layout cost: measure on a
20-page document; if > 8 ms, lay out incrementally from the first changed paragraph (paragraphs
are independent when the template has no flowing cells). No "large text mode" toggle; cull in
paint instead (only pages intersecting the viewport are painted).
**Open questions / VERIFY:** none.

#### Caret, mouse selection and keyboard navigation (光标 / 鼠标选择 / 方向键与 Shift 选择)
**ChiXie3:** press sets the caret / starts a selection; drag selects characters across rows,
rectangles of table cells, or grid cells; double-click places a caret in a table cell (custom
`cell_mouse.svg` cursor). Arrow keys move by character / row; Up/Down pick the nearest x
(`_calculate_closer_point`); Shift+arrows extend. Messages "当前已经是最后一行" / "当前已经是第一行".
Many table/composition navigation paths are unfinished: "方向按钮切换单元格选中尚未开发",
"作文格下方向按钮暂未开发", "表格功能选中下一个单元格尚未开发", etc. (C §方向键) [high].
**How it works there:** `KeyPress*` classes on `UserInputBox`; `SelectItemFunction` for mouse.
**chixie-rs today:** Done for flowing text in `doc_view.rs` (Left/Right/Up/Down, word moves,
line start/end, doc start/end, Shift variants, mouse drag, double-click word).
**Recommended implementation:** add cell-aware navigation once tables exist: Tab / Shift+Tab
move between cells, arrows cross cell borders when the caret is at an edge, Ctrl+click
multi-selects cells. Everything ChiXie3 left "尚未开发" should simply work. Verify with
`editor.rs` unit tests per motion plus AUTOTEST key scripts.
**Open questions / VERIFY:** none.

#### Clipboard (复制 / 剪切 / 粘贴)
**ChiXie3:** Ctrl+C/X/V (handled on key release) and the context menu. Copy writes plain text
and MIME `application/chixie-doc-items` (items with font id and formats); an internal
`WritingClipboard` cache exists because of a hang "出现过未响应问题…按住粘贴快捷键就会未响应".
Paste classifies the clipboard: Excel "XML Spreadsheet" (parsed to a cell matrix), ChiXie items,
WPS/Word RTF (`parse_wps_rtf`: decodes `\'xx`, maps `\super`/`\sub` to `<sup>`/`<sub>`), plain
text; else "未能解析的剪切板类型". Table paste: one text into every selected cell, or a matrix
that must match the selected rectangle ("您所复制的表格结构与炽写内选中单元格不匹配"); "未选中表格单元格，无法粘贴".
Copy/cut of several cells is refused ("暂不支持复制多个选中单元格的数据") (C §复制) [high].
**How it works there:** `CopyFunctionOfWrite`, `CutFunctionOfWrite`, `PasteFunctionOfWrite`,
pyperclip + pickle of items.
**chixie-rs today:** plain-text copy/cut/paste via gpui's clipboard in `doc_view.rs`.
**Recommended implementation:** keep plain text as the interchange format and add a private
JSON flavour (`application/x-chixie-rs-items`: chars, ids-free attrs) so pasted characters keep
font/size/colour. gpui's `ClipboardItem` supports metadata strings; store the JSON there
(VERIFY API in `docs/gpui-api.md`). Never pickle. For tables, accept TSV (what Excel/WPS put
on the clipboard as text) rather than parsing XML Spreadsheet; it carries the same matrix.
RTF super/subscript is P3. Tests: round-trip copy→paste of a styled run; TSV → 2×3 cell paste.
**Open questions / VERIFY:** gpui clipboard metadata API on Windows.

#### Select all, delete, backspace (全选 / 删除与退格)
**ChiXie3:** Ctrl+A per template type (`InTableSelectAll`). Backspace/Delete remove the item
before/after the caret or the selection; deleting at a row start pulls the next rows up. With
whole table cells selected: "是否清空所选单元格的内容？(可回车确认）" (Backspace) / "是否删除所选单元格的内容？(可回车确认）"
(Delete), Enter confirms (C) [high].
**chixie-rs today:** Done for text (`EditDoc::backspace/delete`, word variants).
**Recommended implementation:** add cell clearing with the same confirm (Enter = yes) when
tables land; it is a destructive multi-cell edit but undoable in chixie-rs, so the confirm can
be dropped if undo covers it (UX improvement).
**Open questions / VERIFY:** none.

#### Undo (撤销)
**ChiXie3:** Ctrl+Z only shows "撤销功能尚未开发完毕" (C @9575b8d) [high].
**chixie-rs today:** Done: `EditDoc::undo/redo`, Ctrl+Z / Ctrl+Shift+Z.
**Recommended implementation:** extend the undo stack to attribute edits and template/cell
edits (command pattern: each edit records its inverse). Clearly better than ChiXie3.

#### Half-width space (半格空格)
**ChiXie3:** Space with Ctrl or Shift inserts a half-width space (`HalfSpaceFunction`,
"输入半个空格"); which modifier is not recoverable (C) [high].
**chixie-rs today:** missing.
**Recommended implementation:** bind Shift+Space (Ctrl+Space is the Windows IME toggle) to
insert U+2009 THIN SPACE or a private `HalfSpace` char kind; layout gives it half the random
space width. Test: width = ½ of the drawn space width with the same seed.
**Open questions / VERIFY:** which modifier ChiXie3 uses (try both in the live app).

#### Find and replace (查找和替换)
**ChiXie3:** dialog 查找和替换 with tabs 查找 / 替换, field 查找内容, buttons 查找下一处, 替换,
替换全部, 取消 [UI ck-k2]. Form `UI_SearchAndReplaceDialog` [ui-form] (title 查找和替换, tab widget): labels 查找内容：,
查找下一处, 替换为：, 全部替换 (the replace tab's 替换 button caption is not a form string). Plain substring search over paragraph strings (spaces and returns
included); `find_next` selects and scrolls; replace deletes and re-types the new text. Messages
"未搜索到结果", "请先进行查找定位，再点击替换", "未查询到结果，无法替换", "替换失败：". No case / regex /
whole-word options; runs on the UI thread (C §查找和替换) [high].
**chixie-rs today:** missing.
**Recommended implementation:** core `editor::find_all(&EditDoc, &str, opts) -> Vec<Range>`
and `replace_range`; replaced characters get fresh ids (new handwriting), untouched ones keep
theirs. GUI: a small non-modal bar at the top of the page view (Word style), Enter = next,
Shift+Enter = previous, match count "第 n / N 处". Options: 区分大小写, 全角半角等同 (useful for
Chinese text). Tests: `find_all` on overlapping matches; replace-all keeps other ids.

#### Context menus (右键菜单)
**ChiXie3** (`ContextMenuOfWrite`, `RoundMenu`, C §右键菜单) [high]:
- origin marker: 移至纸张起点, 移至框线起点, 自定义原点位置;
- page: 书写此页 ▸ (直接书写 / 上传SD卡书写), 工作区巡边/绘制, 断点续写, 添加页面;
- table/cell: 绘制表格, 单元格属性, 数据源 ▸ 刷新数据源数据 ▸ 刷新数据(本页) / 刷新数据(所有页);
- always: 复制, 剪切, 粘贴, 全选 (enabled by state).
The menu is suppressed while the AI input is active.
**chixie-rs today:** missing (actions exist on the ribbon only).
**Recommended implementation:** gpui-component `ContextMenu` on `doc_view` with the same
groups; every entry dispatches an existing action so ribbon, menu and shortcuts share one code
path. 断点续写 here should open the resume flow pre-filled with the clicked character (§3.7).

#### Insert menu (插入)
**ChiXie3:** ribbon 插入 menu: 表格, 公式 ▸ (我的收藏, 数学公式, 化学公式, 物理公式), 图表, 图片,
文本框, 形状, 内容块, 思维导图 [UI shot 24; C §插入菜单]. Static analysis finds no handlers for most
entries in the writing editor (placeholders, added 2025-10-27) [med].
**chixie-rs today:** missing.
**Recommended implementation:** only 表格 has substance (inserting a table region into a note
page; depends on §3.2 tables). Do not port the placeholders. P3.

#### Character formats (字符格式)
**ChiXie3:** ribbon buttons 倾斜 (I; the form names two tilt tooltips 右倾斜 / 左倾斜 [ui-form]), 下划线 (U), 删除线
(form tooltip 删除线、墨团等: strike-through and ink blobs), 文字颜色 ▾ (tooltip 设置颜色，用于换笔书写), 下标 X₂, 上标 X², 标题 (H),
清除格式 (eraser), 增大/缩小字号 (A+ A-), 首行缩进 / 增加缩进 / 减少缩进, align buttons [UI shot 01].
Modes in `SelectTypeSetting`: `SetFont, EnlargeFontSize, ReduceFontSize, SetTilt, SetUnderLine,
SetDeleteLine, SetTextColor, SetSuperScript, SetSubScript, SetTitle, ClearFormat,
FirstLineIndent, VerticalOffset, DecreaseIndent, IncreaseIndent, Align*` (C @9541d75). Applying
re-typesets the range; a cell with unresolved overflow refuses: "存在内容溢出，请先处理溢出".
Toggle state shows checked only if all selected characters share the format
(`DisplayWritingFormat`). Tilt is applied only when the user presses 倾斜 (C @953d5a8) [high].
Underline / strike-through are pen strokes added to the output (inferred). Colour matters for
multicolour drawing (§3.7).
**chixie-rs today:** missing (document-wide font and size only).
**Recommended implementation:** `CharAttrs { font: Option<FontId>, size: Option<f32>,
color: Rgb, italic: bool, underline: bool, strike: bool, script: Script, title: bool }` stored
as runs in `EditDoc`. Layout effects: italic = shear of the glyph (angle in style, default
12°), underline/strike = extra strokes generated in `layout.rs` per line segment (with the same
jitter so they look hand-drawn), super/subscript = 0.6 size and ±0.35 em offset, title = 1.5×
size (VERIFY ChiXie3's factors by capture). The ribbon state comes from a core helper
`common_attrs(selection) -> Tri<..>`. Tests: golden for an underlined word.
**Open questions / VERIFY:** how ChiXie3 draws underline/strike and the sizes used for 标题 and
super/subscript (capture with the fake Grbl).

#### Format painter (格式刷)
**ChiXie3:** ribbon button 格式刷 in the clipboard group [UI]; no static detail.
**chixie-rs today:** Missing.
**Recommended implementation:** after `CharAttrs` exist: click copies the caret's attrs, next
selection receives them; double-click keeps it armed (Word behaviour). P3, S.

#### Font and size of the selection (字体设置 / 字号)
**ChiXie3:** font combo + size combo (default 100) in the ribbon; apply to the selection or
selected cells (`SetFontAndSize`); the size box follows the selection; mixed fonts show
"所选内容中字体不统一" (A, C) [high]. Size 100 = 1 mm per font unit (protocol.md [cap]).
**chixie-rs today:** document-wide font (`Select` picker) and size (A+/A−).
**Recommended implementation:** per-run font/size via `CharAttrs`; the picker shows the common
value or blank when mixed. Layout already scales by `mm_per_unit_for_font_size`.

#### Alignment (对齐)
**ChiXie3:** 左对齐 / 居中 / 右对齐 / 两端对齐 (the main-window form has labels for 左对齐, 右对齐 and 两端对齐 only; the
centre button's caption is not a form string [ui-form]) per paragraph at the caret or per selected
cell(s); vertical 顶端 / 垂直居中 / 底端 for table cells. Errors "请先放置光标或者选中字符再使用对齐功能",
"请先选中单元格". Re-applied after every input (`refresh_alignment_after_input`) (C §对齐) [high].
**chixie-rs today:** missing (always left).
**Recommended implementation:** paragraph attribute `Align` in `EditDoc` (paragraph = text
between `\n`); `layout.rs` shifts each closed line by `(avail − used)` or `/2`; justify spreads
the remaining width over inter-glyph gaps (not after the last line). The random line-start
offset (首字不对齐) applies only to left/justify. Tests: right-aligned line ends at the margin
± spacing jitter.

#### View toggles (显示框线 / 行号 / 页码 / 单双页)
**ChiXie3:** toggles for template frame lines (`CellLineVisible`, `FormLineVisible`), row
numbers, page numbers; a page-number detector while scrolling updates "第 N 页，共 M 页"; two
status-bar view-mode buttons (single / double page) (C §View; UI) [high].
**chixie-rs today:** 模板格线 toggle; page count in the status bar.
**Recommended implementation:** add row numbers (left gutter, preview only) and double-page
view. Template lines are never plotted unless the user asks (§3.7 绘制框架线).

### 3.2 Page templates (页面模板 / 模板管理)

#### Template manager shell (模板管理)
**ChiXie3:** 书写 ribbon [模板管理] opens a contextual ribbon category 页面模板 › 模板管理 with
创建模板 ▾ (文档模板 / 表格模板 / 作文格模板), 删除, 导出, 保存, 返回 [UI]. Left: 模板列表 with type icons
(Word-like = notes, Excel-like = tables, OneNote-like = 作文格) [UI]. Right: parameter panels by
type and a live preview scene with wheel zoom. New templates are listed as 新建笔记 / 新建表格 /
新建作文 until saved; status "未保存" (B §A) [high]. The template picker in the 书写 ribbon
(e.g. 作文格1 ▾) lists 我的模板 [UI shot 22] (the popup form `UI_WritingTemplateSelection` is a list headed 我的模板
[ui-form]); selection is stored as
`[WriteConfig] NowWritingTempType/NowWritingTempName` (config `NowWritingTempType=3`,
name 作文格1).
**Save validation** (B @94b3797..94b3cf7) [high]: "模板名称不可为空";
"笔记模板中已经存在该模板名，请修改模板名称" (and 表格/作文 variants); "最小行数不能大于最大行数";
"页面宽度高度必须大于0"; "数据输入异常"; "存在空白页面，无法新建表格模板";
"行数+行间距+上边距的数值超出页面高度，请检查"; "列数+列间距+左边距的数值超出页面宽度，请检查".
**How it works there:** notes pickled to `note_template.cx`, compositions to
`composition_template.cx`, table templates through `TemplateAccessUtil`, Fernet-encrypted with a
hard-coded key. Old formats are converted on load (`convert_old_template_to_new`). Table template
autosave is debounced by `SaveTableTemplateTimer` (C) [high].
**chixie-rs today:** `template.rs` (`Template`, `TemplateKind::{Blank, Ruled, Grid}`,
`TemplateStore` → `templates.toml`), `templates_view.rs` (backstage page: list, edit form, live
preview, draft until 保存, 应用), `chixie3_templates.rs` (safe pickle reader for `.cx`).
**Recommended implementation:** keep TOML (human-readable, diffable). Add ChiXie3's validation
messages verbatim to `Template::validate() -> Result<(), TemplateError>` with the Chinese
`Display`. Add 导出 / 导入 of a single template as a `.chxt` TOML file (share between users).
Add 删除 with confirm. Tests: one per validation error.
**Open questions / VERIFY:** none.

#### General parameters (通用参数: 模板名称, 纸张宽度/高度, A4/B5)
**ChiXie3:** 模板名称; 纸张宽度 / 纸张高度 (mm; live values 190.0 × 255.0 for 作文格1); 快捷尺寸 [A4]
[B5] buttons fill `StandardPaperSize`; every change re-renders the preview (B §通用参数) [UI][high]. Form
`UI_TemplateGeneralParameterGroupBox` [ui-form] (group 页面通用参数): 模板名称：(line edit), 纸张宽度：, 纸张高度：(double
spin boxes), 快捷尺寸：(a tool button; the live UI showed A4 and B5 entries). The list pane's title is 模板列表
(`UI_TemplateItemListGroupBox`).
**chixie-rs today:** Done in `templates_view.rs` (paper size + margins).
**Recommended implementation:** add the A4/B5 quick buttons and a portrait/landscape swap.

#### Note templates (文档模板 / 笔记 / 信纸)
**ChiXie3:** margins 上/左/下/右 (mm); row mode radio: `DefaultRowMode` (one row count) or
`RangeRowMode` (min..max, 随机行数); fields `min_row_num`, `max_row_num` (B §文档模板). Form
`UI_NoteTemplateParameterGroupBox` [ui-form] (group 文档页面参数): 上侧边距：, 右侧边距：, 下侧边距：, 左侧边距：; radio
buttons 固定行数 (with 默认行数：) and 随机行数 (with 最小行数： and 最大行数：), so the two modes are labelled 固定行数
(= `DefaultRowMode`) and 随机行数 (= `RangeRowMode`). Paper item
draws note rows (`#c0c0c0 #d0d0d0 #e0e0e0`). Live note line style setting: 设置 › 主题与样式 ›
笔记线条 实线 ▾ [UI]. Captured: rules every writable height ÷ rows; glyph top ≈ row bottom − em −
0.2 em (protocol.md [cap]).
**chixie-rs today:** Done (`TemplateKind::Ruled`, `layout::Ruling`, capture-matched tests
`template::tests::ruled_rows_match_chixie3_capture`).
**Recommended implementation:** keep. Add 笔记线条 style (solid / dashed / dotted) as a preview
option only. VERIFY random-row semantics (one draw per page) with a capture.
**Open questions / VERIFY:** random row count per page; per-row ±0.4 mm spread seen in the
capture (model or ignore).

#### Columns (笔记分栏)
**ChiXie3:** dialog: column count (spin + horizontal list), checkboxes 分隔线 (form label; was 显示分割线,
`show_dividing_line`), 栏宽相等 (`column_width_equal`), 异栏同型 (`column_same_type`: all columns
share the same cell type), per-column width and spacing spin boxes; exact form `UI_NoteTempColumnSettingDialog`
[ui-form]: 栏数预设 (list of presets), 栏数：, 分隔线, group 宽度和间距 with 宽度: and 间距:, 栏宽相等, 异栏同型 and a
help hint "(异栏同型？)" (a "what is this" hint next to 异栏同型; its text is only that label); editing one width splits the
remaining width evenly with spacing 0 after it; region = page minus margins (B §笔记分栏) [high].
Import into a multi-column note asks "当前为多栏笔记，请问导入第一列还是第二列？" (C) [high].
**chixie-rs today:** equal columns with one gap and a divider (`TemplateKind::Ruled.columns`).
**Recommended implementation:** generalise to `columns: Vec<Column { width, gap_after }>` with an
"equal" helper; text fills column 1 top-to-bottom then column 2 (VERIFY order by capture).
Import target column becomes a choice in the import dialog.
**Open questions / VERIFY:** fill order across columns; meaning of 异栏同型 in output.

#### Dual-line notes (双线文档)
**ChiXie3:** ribbon 文档格式 › 双线文档 dialog 双线文档设置: 启用双线模式, 上侧行高： (`up_row_height`; a
double spin box), 虚线分割双行 (`enable_dashed_line_partition`; the form label, not 启用虚线分割) [ui-form
`UI_NoteTemplateDualLineSettingDialog`]. Row position types `TopNoteRow`, `MiddleNoteRow`,
`BottomNoteRow`, `IndependentRow`. Import asks "当前为双线模式笔记，请问导入至上侧行还是下侧行？"
(B, C) [high].
**chixie-rs today:** `dual_line: Option<f32>` band; text stays below the band (VERIFY).
**Recommended implementation:** add a per-row "target band" choice (upper / lower) as a template
option and in import; capture one dual-line page to pin placement.
**Open questions / VERIFY:** where ChiXie3 puts text in dual-line rows by default.

#### Composition grids (作文格模板)
**ChiXie3:** 作文页面参数: 格子行数 20, 格子列数 20, 左侧边距 15.0 mm, 上侧边距 20.0 mm, 格子宽度 8.00 mm,
格子高度 8.00 mm, 行间距离 2.90 mm, 列间距离 0.00 mm [UI, 作文格1]. Overflow validation as above.
Form `UI_CompositionTemplateParameterGroupBox` [ui-form] confirms the eight labels and their order: 格子行数：,
格子列数：, 左侧边距：, 上侧边距：, 格子宽度：, 格子高度：, 行间距离：, 列间距离：(the form has both integer and double spin boxes;
presumably counts are integer, lengths double).
Captured: one character per cell, centred at font size; punctuation drawn low in its own cell;
a space takes a cell; pasted line breaks dropped, typed Enter starts a new row
(protocol.md [cap]).
**chixie-rs today:** Done (`TemplateKind::Grid`, capture test `grid_cells_match_chixie3_capture`).
**Recommended implementation:** keep. Do not copy the "pasted line breaks dropped" quirk:
honour line breaks in paste (UX improvement; document it).

#### Table templates (表格模板)
**ChiXie3:** forms made of pages (`TemplatePreviewTablePaperItem`, mode `BlankMode` /
`ContentMode`), each holding tables (`TableItemInTemplate`: `table_class`, size, `lock_pos`,
`cell_info`) of "母单元格" (parent cells). A parent cell has `internal_rows` (writing rows inside),
`internal_margin_left/top/bottom/right` (only left used by the generator [high]; the cell dialog nevertheless shows
all four as 左/上/下/右内边距 [ui-form]), `accept_import`, `cell_class`
(`page_<n>_table_<m>_cell_<k>`), hint label, font id/size, font loading mode, char width ratio,
alignment, data source. Pages can carry a background image (`bg_img_b64`) with opacity and an
Excel import range. Template-level: `page_numbers`, `show_background`, `background_opacity`,
`same_cell_data`, `same_cell_record` (B §保存模板; C §表格纸) [high]. Two data versions (V1
backend-generated, V2 local). Exact editor forms [ui-form]: 表格位置 (`UI_TemplateBaseItemPositionGroupBox`, group 表格位置与尺寸(mm)):
表格宽度：, 表格高度：, 位置锁定：, 横向坐标：, 纵向坐标：. 单元格属性 (`UI_TemplateTableParentCellAttrGroupBox`, for a selected
cell): 单元格宽：, 单元格高：, 接受导入：, 内部行数：. 表格单元格属性 dialog (`UI_TableCellAttributeDialog`, OK / close):
接受导入：(switch), 内部行数, 提示标识：(hint label text), 左内边距, 上内边距, 下内边距, 右内边距, 排版内边距 (heading for the four margins; so they are "typesetting padding"),
单元格字体属性 (heading) with 默认字体, 字体加载模式 (combo; 指定字体优先 is one entry, cf. §3.3), 字符宽度比, 默认字号. The main
window also has ribbon entries 表格功能, 批量溢出 and 作文格功能 (contextual groups).
**chixie-rs today:** missing.
**Recommended implementation:** core module `table.rs`:
```rust
pub struct TableTemplate { pages: Vec<TablePage>, same_type: Vec<SameTypeRecord> }
pub struct TablePage { size_mm: (f32, f32), background: Option<PngBytes>, bg_opacity: f32,
                       excel_range: Option<CellRange>, tables: Vec<Table> }
pub struct Table { origin_mm: (f32, f32), locked: bool, cells: Vec<ParentCell> }
pub struct ParentCell { id: CellId, rect_mm: Rect, rows: u32, rows_locked: bool,
                        padding: Insets, accept_import: bool, hint: String,
                        font: CellFont, size: Option<f32>, char_width_ratio: f32,
                        align: (HAlign, VAlign), data_source: DataSource }
```
Layout treats each parent cell as a small ruled region (rows = `rows`), using the same row
placement rule as notes. Store templates as TOML + background PNG side files, not base64 in the
TOML. Import ChiXie3 table templates later (they are Fernet + pickle; needs the client key at
runtime from the user's own install, never committed). Verify with a capture of a 2×2 table.
**Open questions / VERIFY:** cell text placement (row baseline rule inside cells).

#### Insert table / draw table / screenshot (插入表格 / 绘制表格 / 截图生成)
**ChiXie3:** ribbon 生成表格框架 › 插入表格: a hover grid menu ("{}×{} 表格"), a custom dialog
(rows, cols, fixed vs auto column width; error "列宽*列数超出页面宽度(还需保留最少2mm边距)，请重新设置"),
requires a selected page ("请先在右侧缩略图中选中页面再添加"). Form `UI_InsertTableDialog` [ui-form] (title 插入表格): group
表格尺寸 with 列数： and 行数：; group 列宽选择 with radio buttons 固定列宽 (with a width spin box) and 自动列宽; checkbox
为新表格记忆此尺寸 (remember the size for the next new table). 截图生成: full-screen overlay, drag a
rectangle (Esc/right-click cancel, double-click/Enter confirm), the image goes to the cloud
table recognition like file import (B §插入表格, §截图生成) [high]. A confirmation dialog 截图导入表格
(`UI_ImportTemplateByScreenShotDialog` [ui-form]) with the button 生成并导入 presumably shows the captured image before it
is sent (inferred; the form has only a title and that button).
**chixie-rs today:** missing.
**Recommended implementation:** hover grid as a gpui popover (8×10 squares); custom dialog with
the same validation. Skip screen capture (P3): users can save a screenshot and import the image.

#### Import PDF/image as table frame (导入 PDF/图片 生成表格框架)
**ChiXie3:** each blank table page shows a drop zone "将文件拖到此处，或点击导入本地文件（PDF或图片表格）",
filter "PDF或图片(*.pdf *.jpg *.png)". Pages are rendered (PyMuPDF), base64-PNG posted to the
backend (`file_parsing/image-to-table-template`, `-2`; A endpoints) which returns cell corner
points; the image becomes the page background. Orientation auto-switch
"检测到导入的模板与页面方向不同，已为您切换" (undoable). Errors "<n> 页表格解析异常，请手动创建该页面表格" (B) [high].
**chixie-rs today:** missing.
**Recommended implementation:** two paths. (a) Cloud: call the same endpoint (capture the
request/response shape with mitmproxy first). (b) Local fallback (better, offline): detect
ruling lines with a morphological filter on the binarised image (horizontal/vertical runs ≥ 40%
of width/height), intersect them into a grid, merge spans. Pure Rust on `image` (already a dep);
PDF rasterising needs a new crate — `pdfium-render` (needs the Pdfium DLL) or `hayro`/`mupdf`;
prefer asking the user to export the page as PNG for now. Verify on 3 real forms.
**Open questions / VERIFY:** endpoint request/response shapes.

#### Excel import range (Excel导入配置)
**ChiXie3:** per page (group 范围配置 with sub-groups 左上角 and 右下角 [ui-form `UI_TableTemplateImportConfigOfExcel`]):
左上列 (letters), 左上行 (digits), 右下列, 右下行; validation "行号输入框中只能输入数字",
"列号输入框中只能输入字母", "如需保存请填写所有所有输入框数据", "暂存成功" (B) [high].
**chixie-rs today:** Missing.
**Recommended implementation:** `excel_range: Option<CellRange>` on `TablePage`, parsed with an
`A1` parser (`excel_column_to_number`). Used by xlsx import (§3.15).

#### Same-type marking (同型标记)
**ChiXie3:** dialog with records (备注名) and member cells (page, table, cell); add / delete /
move up/down / save [ui-form `UI_TableTemplateMarkSameCellDialog`: title 同型标记, list 已有同型, table 同型单元格, 同型备注：,
buttons add / delete / up / down / save / close]; Ctrl+click or drag to multi-select; at least two cells; a cell can be in
one record only; "同型标记中，暂不支持前页单元格是后页单元格的同型". When pages are added to a multi-page
table document, cells of a record are re-keyed so they stay linked (B §同型标记; C §同型标记)
[high]; exact user-visible effect [med] (inferred: equivalent cells across repeated pages
receive the same content).
**chixie-rs today:** Missing.
**Recommended implementation:** P3. Model as `SameTypeRecord { name, cells: Vec<CellId> }`; when
the user types into one member, offer "同步到同型单元格". VERIFY intended behaviour with the vendor.

#### Cell data sources (数据源配置)
**ChiXie3:** one selected cell at a time ("暂时数据源设置不支持多选单元格"). Types:
`NoDataSource`; `DefaultValueDataSource` (`default_value`); `RandomValueDataSource` with
`RandomWithinValue` (`min_value`..`max_value`, `decimal_places`; "最小值不能大于最大值") or
`RandomWithinContent` (one choice per line of `content_list`); `CustomCellDataSource` (`row_num`,
`col_num`: filled from a file import). Form `UI_TableTemplateDataSource` [ui-form] (title 表格模板数据源): a list with
无数据源 / 默认值 / 随机值 / 指定单元格 and a stacked page per mode: 无数据源模式 ("无数据源模式，数据需要导入或者输入"); 默认值模式
with 默认值：; 随机值模式 with two sub-modes 数值范围内随机 (取值范围：, 小数点后位数：) and 给定内容内随机(使用换行分隔) (a text
box, one choice per line); 指定单元格模式 with 绑定单元格（仅限于导入文件使用）, （Excel模式）, 行号：, 列号：; buttons save and close.
Context menu 刷新数据(本页) / 刷新数据(所有页); offered when
pages are added ("询问用户是否加载页面数据源数据源") (B §数据源配置; C §表格数据源) [high].
**chixie-rs today:** Missing.
**Recommended implementation:** `enum DataSource { None, Fixed(String), RandomNumber { min, max,
decimals }, RandomChoice(Vec<String>), FromImport { row, col } }`; generation uses the document
seed + cell id + page index (deterministic, re-rollable with 换一组). Typical use: forms such as
temperature logs. Test: same seed → same values; different page → different values.

#### Merge / split cells (合并单元格 / 拆分单元格)
**ChiXie3:** merge selected cells; split is a full split ("目前版本是完全拆分解组单元格，不是取消合并上一次合并的单元格");
drag split handles between cells; corner/edge resize handles; position lock; Delete deletes
the selected table (B) [high].
**chixie-rs today:** Missing.
**Recommended implementation:** cells as a rectangle partition of the table; merge = union of an
axis-aligned rectangle of cells (reject non-rectangular selections); split = back to the
underlying grid lines. Handles in the template editor canvas only.

#### Padding detection (内边距识别 / 重置内边距 / 占位符识别)
**ChiXie3:** from the page background image, crop each selected cell, find the leftmost ink
pixel per row and set `margin_left`; errors "请先选中表格单元格再识别", "所在页面没有背景图，无法识别".
占位符识别 is a stub (B) [high].
**chixie-rs today:** Missing.
**Recommended implementation:** P3; implement with `image` (luma threshold, column histogram).

#### Page thumbnail panel (页面缩略图面板)
**ChiXie3:** buttons 预览原始背景, 上移选中页面, 下移选中页面, 删除选中页面, 增加空白页面, slider 透明度;
messages "请先选中页面缩略图再使用此功能", "当前已经是第一页，无法上移", "请至少保留一个页面", confirms
"确认要删除页面 <n> 吗？", "确认复制模板页面的第 <n> 页吗？（默认会复制为最后一页）" (B) [high].
**chixie-rs today:** Missing.
**Recommended implementation:** part of the table template editor; a vertical list of page
thumbnails rendered from the same layout code.

#### Writing origin marker (书写原点)
**ChiXie3:** a draggable blue circle on each page, tooltip "书写原点 / 请确保笔尖在此位置处"; snaps to
page key points; right-click: 移至纸张起点 (`StartPointOfPaper`), 移至框线起点 (`StartPointOfFrame`),
自定义原点位置 (`StartPointOfCustom`); stored as `[WriteConfig] start_point_mode` (11 in the
sample, encoding unknown); custom positions saved debounced, per page for table templates in
frame/paper mode (C §书写原点) [high]. The N1 card text: "炽写页面中蓝色小圆圈即书写起点" (A) [high].
Captured: origin = writable top-left for notes and the grid top-left for 作文格 (protocol.md [cap]).
**chixie-rs today:** origin fixed at the writable top-left (`layout.rs`).
**Recommended implementation:** `Origin { Paper, Frame, Custom((f32, f32)) }` in
`PageSettings`; G-code subtracts the origin point (§3.8). Draw the marker in `doc_view.rs`,
draggable with 1 mm snapping plus key-point snapping (margins, paper corners). Show the "put the
pen tip here" hint once per job start. Test: Paper origin shifts every coordinate by the margin.
**Open questions / VERIFY:** encoding of `start_point_mode`.

#### Template library (模板库, cloud)
**ChiXie3:** 主页 › 模板: tabs 在线模板 / 我的模板 / 我的收藏; 分类 绘图模板 / 激光模板 / 写字模板; 标签
全部模板 / 人物角色 / 教育学习 / 艺术结构 / 建筑风景 / 动物植物 / 科技科学 / 交通玩具 / 其他模板; 排序
时间最新 / 收藏最多 / 点赞最多; search; 12 pages [UI]. All of these strings are confirmed by the form
`UI_TemplateWidget` [ui-form] (header 主页-模板; 请输入关键字 placeholder; 上一页 / "/12" / 下一页 pager; note the
order 标签 → 排序 → 分类, 写字模板 last among the radio categories). File menu 从炽写模板库新建 (C) [high]. Static
slice A calls the home template widget a placeholder shell [high] — the live UI shows it
populated, so it is server-driven (conflict; live wins).
**chixie-rs today:** missing.
**Recommended implementation:** P3. Needs the listing endpoint (not in the static endpoint list;
capture it). Reuse the 3×3 card grid of `fonts.rs`.
**Open questions / VERIFY:** endpoint and template payload format.

### 3.3 Fonts and font library (字体 / 字体库)

#### Font library browser (字体库: 在线字体 / 我的收藏 / 我的录制)
**ChiXie3:** 主页 › 字体 with three directions. 在线字体: 筛选 checkboxes 中文字体 / 外文字体 / 手写字体 /
标准字体 / 公式符号 / 潦草字体 (config `[FontConfig] Tag_Filter=1, 2, 3, 4, 5, 6`); 排序 收藏最多 /
时间最新 / 字数最多 (`Sort_Rule=1`); search box + 搜索; 3×3 cards (preview image of the font
name, star = collect, 名称, 类型 e.g. 中文6900, 作者, 描述); pager 上一页 [n] /541 下一页 with a jump
box ("页码不能超过总页码！") [UI; A §Font widget]. 我的收藏 and 我的录制 need login
("请登录后查看”我的收藏“和”我的录制“！"). The header shows the four main fonts with the hint
"(右键字体卡片可设置主字体)" [UI]. Form `UI_FontWidget` [ui-form] (header 主页-字体): tabs 在线字体 / 我的收藏 /
我的录制; 排序: 收藏最多 / 时间最新 / 字数最多; a search box with placeholder 请输入关键字; 筛选: 中文字体, 外文字体,
手写字体, 标准字体, 公式符号, 潦草字体 and a seventh entry **智能笔录制** (a recorded-with-smart-pen category not seen in
the live tag filters; its request value is unknown); the main-font row in the order 主符号字体： 主线字体：
主英文字体： 主数字字体：; pager 上一页 / "/12" (placeholder total, replaced at run time) / 下一页. Card form
`UI_FontCard`: rows 名称：, 类型：, 作者：, 描述：. 字体库 ribbon page `UI_FontLibraryWebViewDialog` is a web view titled
炽写字体 (loaded from the server). A ribbon 字体库 action also opens a QtWebEngine page with the
login token in the URL (A @94813b0) [high]; on close, newly collected fonts are downloaded.
**How it works there:** `font/get_online_fonts` (1-based, 9/page, verified shape),
`font/get_collected_fonts`, `font/get_recorded_fonts`; previews are public PNGs (server-api.md
[cap]).
**chixie-rs today:** `fonts.rs`: tabs 本地字体 / collected / online, 3×3 cards, paging by 9,
name search, cloud previews (`cloud_preview.rs`, ink-mask cache), 使用 / 删除.
**Recommended implementation:** add (1) the six tag filters and three sort rules (request fields
`filter_tags`, `sort_rule` are already known: `chinese, english, Japanese, handwritten,
standard, formula_symbol, scribble`; map 外文字体 → `english`+`Japanese` VERIFY); (2) 我的录制 tab
(`font/get_recorded_fonts`, shape VERIFY); (3) page jump box. Never put the token in a URL; no
WebEngine page.
**Open questions / VERIFY:** sort-rule strings other than `collections`; recorded-fonts shape.

#### Collect / uncollect (收藏 / 取消收藏)
**ChiXie3:** star on the card (`font/collect_a_font`). Limits: "当前版本客户端软件最多只支持收藏30种字体";
"普通账号无法收藏字体喔！（只能使用今日试用字体）"; a default font cannot be uncollected
("默认字体不能取消收藏，如需取消收藏，请先更换默认字体") (A) [high].
**chixie-rs today:** missing (collected list is read-only).
**Recommended implementation:** `Backend::collect_font(id, on: bool)` in `session.rs`; capture
the request body first. Keep the 30 cap client-side only if the server enforces it (it is a
"client version" limit; check the server's error).
**Open questions / VERIFY:** request shape, server-side cap.

#### Main fonts (主线字体 / 主英文字体 / 主符号字体 / 主数字字体)
**ChiXie3:** card context menu 设为“默认字体” / “默认符号字体” / “默认英文字体” / “默认数字字体” (labels
also 设为主线字体 etc.); only VIP ("普通账号或者未登录无法指定默认字体喔！"), must be collected first
("请先收藏该字体后再设置"). Stored as `[FontConfig] MainFont, MainSymbolFont, MainEnglishFont,
MainNumberFont` and synced via `font/get_main_fonts_info` / `config/font_config` (A, C) [high].
**chixie-rs today:** missing (one active font).
**Recommended implementation:** `FontRoles { main, english, symbol, number }` in
`PageSettings`, each `Option<FontId>` falling back to `main`. Set from the 字体库 card menu and
from a 字体 group dialog in the ribbon. No VIP gating locally (offline-first; the server still
controls which fonts can be downloaded).

#### Per-character font selection and mixing (字体调用模式 / 字体混用)
**ChiXie3:** `FontDefaultMode` vs `FontMixMode`. Classification of each char: Chinese / English /
number / symbol (symbol = `LeadingPunctuation` ∪ `PostPunctuation`) / space. Per class an
ordered de-duplicated list: role font → collected fonts → current font; the first font that has
the glyph wins. Mix mode (`FontMixer`, 2025-09-02): `{font_id: weight}` from `FontsMixRatio` or
the collection's `mixing_ratio`; strategies weighted random, deterministic by char hash ("相同字符总是返回相同字体"),
or by code point; a `seed` attribute exists. Table cells override with 指定字体优先 /
字体库配置优先 / 随机字体模式 (C §字体调用模式) [high]; which mix strategy the UI exposes [low].
**chixie-rs today:** missing.
**Recommended implementation:** core `fontset.rs`:
`FontSet { roles: FontRoles, fallbacks: Vec<FontId>, mix: Option<Vec<(FontId, f32)>> }` and
`fn pick(&self, c: char, char_id: u64, seed: u64, fonts: &FontCache) -> Option<FontId>`.
Mixing uses `rng_for(seed, SALT_FONT, char_id)` (stable per character, re-rollable via 换一组),
which is better than both "random each layout" and "same font for the same character" (real
handwriting varies even for the same character). Layout takes `&FontSet` instead of `&Font`.
Tests: missing glyph falls through to fallback; mix weights 3:1 give ≈ 75% over 1,000 chars.
**Open questions / VERIFY:** none critical.

#### Missing characters (缺字处理)
**ChiXie3:** when a font lacks a glyph the app auto-picks another collected font that has it
(`find_text_in_collection`) and shows "…体，在(所选)内容中，有 N 个字符缺失。已用其他收藏字体替换" with
查看缺失字符 (dialog with the list and 复制; the dialog `UI_MissingCharDialog` is titled 缺失字符, a read-only
text area plus one button [ui-form]) (A, C) [high]. Missing chars are reported to the server
on exit (`user/collect_missing_characters`). The failure message for unloadable fonts:
"字体（…）加载失败，建议重启或取消收藏该字体。如是您本人的字体，请确保该字符已有录制数据" (C @9511728).
**chixie-rs today:** `layout::Document.missing` lists missing characters once.
**Recommended implementation:** fall through `FontSet` first; if still missing, draw a dashed
box placeholder in the preview (never plotted) and show a status-bar badge "缺字 N" that opens a
list with 复制. Do not report to the server.

#### Daily free font (今日试用字体)
**ChiXie3:** ordinary users get one daily trial font (`font/get_daily_free_font`), shown in
the font dropdown's 今日免费试用字体 region; members do not see that region (A, C) [high]. A daily dialog
`UI_DailyFreeFontDialog` [ui-form] (title 今日试用字体) greets "哈喽，亲爱的用户。", says "当前为普通账号，如需升级请前往个人中心或炽写店铺",
"今日炽写为您准备的全量试用字体是", then shows 标题预览：, 字体描述：, 字符数量：, 字体类型：, 作者 ：, 字体名： and closes with
"祝您有个愉快的一天！". The dropdown region label itself is 今日试用 (see below).
**chixie-rs today:** Missing.
**Recommended implementation:** P3; a single call shown as a card in 字体库 when the account is
`ordinary_user`.

#### Font dropdown and previews (字体下拉框)
**ChiXie3:** regions 今日免费试用字体 / 用户录制 / 用户收藏 (the dropdown form `UI_FontDropdownBox` labels them
字体库 (a header or link to the browser; inferred), 我的录制, 今日试用, 我的收藏 in that order [ui-form]); each entry is a picture of the font name
(tooltip "字体名：… 字体类型：…"); hovering previews the font on the selection, leaving restores
(`previewFont`, `fontPreviewRestore`) (C) [high]. Fonts are pre-parsed after login with
progress tooltips ("收藏字体预解析中", "个字体解析完毕"); "请等待字体加载完成后使用！" blocks actions
until done (A) [high].
**chixie-rs today:** `font_preview.rs` picker items (own-stroke name or sample, 云端 badge).
**Recommended implementation:** add hover preview on the selection (layout with a temporary
font override, no edit recorded). Load fonts lazily per use instead of blocking the UI.

#### Offline font store (本地字体)
**ChiXie3:** none; every session re-downloads and decrypts fonts (6–13 MB each) (server-api.md).
**chixie-rs today:** Done: `font_store.rs` keeps decrypted glyph maps on disk
(`<config>/fonts/<id>.json` + `index.json`), usable without login.
**Recommended implementation:** keep; consider a compact binary cache (`postcard` or a custom
f32 array file) if load time > 200 ms per font (measure first).

#### Record your own font (录制字体)
**ChiXie3:** ribbon 录制字体 opens only a guide dialog 录制字体指南 (入口1(推荐) / 入口2): follow the WeChat
official account 炽写智能, bind the account in its menu, open the 炽写字体 mini-program
[UI ck-k5; A] [high]. Recording happens outside the desktop app. Form `UI_FontMiniProgramEntranceGuide`
[ui-form] (title 录制字体指南, two tabs): 入口1（推荐）: "1、微信关注“炽写智能”公众号", "2、公众号菜单栏，进行账号绑定",
"3、绑定后进入炽写字体小程序"; 入口2: "1、微信搜索或扫码“炽写字体”小程序", "2、在小程序内进行炽写账号绑定",
"（推荐使用方式一）".
**chixie-rs today:** Not planned (link only).
**Recommended implementation:** a link/help card only. Not planned.

#### Font data optimisation (字体优化 / 字体数据优化)
**ChiXie3:** 百宝箱 › 字体优化: font combo, [统一字符尺寸] [重复轨迹优化] [异常宽度优化]
[统一所有字体尺寸], ☐启动自动调用 [UI]. Form `UI_FontDataOptimizeDialog` [ui-form] (title 字体数据优化): the
three per-font buttons are labelled 统一字符尺寸, 重复轨迹优化, 异常宽度优化; plus 统一所有字体尺寸 and the checkbox 启动自动调用
(the 百宝箱 tile subtitle is 优化部分字体问题). Unify: compute each glyph's bbox, fetch a standard bbox
set (`font/get_standard_br`), scale/move strokes into the target box (default size 10). Uses a
Fernet key embedded in the binary (A §Font data optimize) [med].
**chixie-rs today:** missing.
**Recommended implementation:** core `glyph_fix.rs` with three pure functions over `Glyph`:
`normalize_bbox(glyph, target)`, `dedupe_overlapping_segments(glyph, tol_mm)` (same as 字符重复轨迹优化,
§3.4), `clamp_abnormal_width(glyph, median_advance)`. Apply on load when enabled ("启动自动调用"),
never rewrite the server copy. Standard boxes: derive locally from the font's own median CJK
bbox instead of `get_standard_br` (offline). Tests on fixture glyphs.

### 3.4 Handwriting effects and deformation (手写效果 / 字符变形)

#### Spacing, space width, vertical offset, first-char offset (写字偏好)
**ChiXie3:** 设置 › 写字偏好 [UI][ui-form]: 字符间隔(毫米) 中文 0.6–1.1, 英文 0.5–1.0; 空格宽度(毫米) 中文 5.0–7.0,
英文 2.0–3.0; 首行缩进 宽度 15.5–17.0; 垂直偏移(毫米) 范围 0.0–0.3, 风格 ☐启用智能偏移(同向偏移); 首字
☑首字不对齐(适用于笔记、表格) 不对齐幅度 2.0. Config keys `min/max_char_spacing_zh|en`,
`min/max_space_width_zh|en`, `min/max_first_line_indent`, `min/max_vertical_offset`,
`intelligent_vertical_offset`, `first_text_not_align(_range)`; also `row_offset_style=1`,
`random_correct_offset=false` without UI (B §写字偏好; config). Captured: all values absolute mm,
drawn per character at layout time, stable across writes; first-char offset on every line
(protocol.md [cap]).
**How it works there:** `GenerateTextItem.__get_h_offset` (uniform in the zh/en range by the
character's class), `get_v_offset`; space width chosen by the previous character's class.
**chixie-rs today:** Done in `handwriting.rs` (`HandwritingStyle`, `Jitter`, SplitMix64 `Rng`,
per-id streams), `style_dialog.rs` (↘ ranges dialog), `config::import_chixie3`.
**Recommended implementation:** keep; add the live-UI labels verbatim in the dialog. Space width
VERIFY by capture (stream stalled before).

#### Intelligent vertical offset (智能垂直偏移 / 同向偏移)
**ChiXie3:** with `intelligent_vertical_offset` the offset depends on the position along the
row (`position_proportion = target_pos_x / cell_width`), i.e. a smooth drift instead of noise;
TODOs mention following a whole-row trajectory (C §字符间距) [med]. UI label "(同向偏移)".
**chixie-rs today:** `intelligent_vertical_offset` as a clamped random walk (guess, VERIFY).
**Recommended implementation:** change to a per-row linear drift: draw a slope `k ∈ [−r, r]`
per row (r = max offset / row width) and offset = k·x + small noise, clamped to the range; this
matches "position proportion" and "同向" (same direction). Capture to confirm.
**Open questions / VERIFY:** formula; `row_offset_style` values; `random_correct_offset`.

#### First-line indent (首行缩进)
**ChiXie3:** not automatic. Tab inserts an indent `SpaceItem` of random width
`[min_first_line_indent, max_first_line_indent]`; ribbon 首行缩进 / 增加缩进 / 减少缩进 apply to the
paragraph at the caret or every selected paragraph; needs a caret or selection
("当前版本首行缩进需在‘选中字符’或‘放置光标’后使用"). In 作文格 it toggles two indent cells (C §首行缩进)
[high]. This explains why the capture showed no indent (protocol.md).
**chixie-rs today:** `HandwritingStyle.first_line_indent: Option<Span>` (not applied).
**Recommended implementation:** paragraph attribute `indent: bool` toggled by Tab-at-paragraph-
start and the ribbon button; width drawn per paragraph id from the span; grid templates indent
by two cells. Better than ChiXie3: Tab at paragraph start indents, elsewhere inserts a tab-width
space. Test: indented paragraph's first glyph x ∈ [15.5, 17].

#### Base deformation (基础变形)
**ChiXie3:** 设置 › 字符变形 › ☑基础变形: 字符随机胖瘦 96%–104%, 字符随机高矮 98%–102%, 字符随机旋转
−2.0°–2.0°, 笔画变形幅度 slider (1), ☑随机删除小尾巴; "(滑动条为0则该项无变形)"; [使用推荐参数] with
confirm "确认将字符变形参数设置为推荐参数吗？" [UI; B §字符变形]. Master switch
`EnableCharacterDeformation` (false in the sample). Algorithm: random scale x/y and rotation
about the glyph centre, then per-stroke scale/rotation/offset derived from the stroke level;
`randomly_delete_trailing` drops tail points; truncating rounding ("会将0.45999输出为0.45")
(C §基础变形) [high]. Captured magnitudes in protocol.md [cap]. Note protocol.md lists 高矮 as
96–104 while the UI and config say 98–102 (conflict: the UI/config value is authoritative for
defaults; the capture's fits are consistent with either).
**chixie-rs today:** `deformation.rs` (per-glyph scale, rotation, per-stroke shift); tail
removal missing.
**Recommended implementation:** add tail removal: with probability p per stroke, drop the last
k points whose cumulative length < 0.3 mm and whose turn angle > 90° (a "hook"); VERIFY against
a capture with many hooked glyphs. Expose the master switch in the 手写 tab. Keep separate RNG
streams per effect (already done).

#### Shape deformation (字形变形, thin-plate spline)
**ChiXie3:** ☑字形变形, 字形变形幅度 slider (7). Thin-plate spline over grid control points with
random perturbations (corner/centre/edge waves); result accepted only when similar enough to the
original (Haar-wavelet similarity, alternatives Euclidean and DTW); retries (`tps_times`). Very
slow; a TODO says to rewrite in numpy/JIT/C (C §字形变形) [med-high]. Captured: italic-like
slant on CJK, small smooth residual (protocol.md [cap]).
**chixie-rs today:** `deformation.rs` models shape as CJK slant + smooth noise.
**Recommended implementation:** implement a real TPS: 3×3 or 4×4 control grid over the glyph
bbox, displacements drawn from the seed scaled by level, solve the (n+3)² linear system once per
glyph (small dense solve; write it by hand or add `nalgebra` — already common, pure Rust).
Replace the wavelet similarity check by a cheap bound: reject when any control displacement
exceeds `level × 1.5%` of the bbox or when stroke self-intersections increase. Deterministic per
character id. Benchmark target: < 50 µs per glyph. Keep the current slant model as level ≤ 3.
**Open questions / VERIFY:** amplitude mapping of the slider (0–10?) — capture levels 3 and 10.

#### Character fine-tuning (字符微调)
**ChiXie3:** dialog 字符微调 [UI ck-k3][ui-form `UI_CharacterTuningDialog`]: 字符:, 字体: (collected fonts), 字号: with A+/A−
(tooltips 增大字号5% / 缩小字号5%), 微调: arrow
pad ↑↓←→ and centre (tooltips 上移0.2 / 下移0.2 / 左移0.2 / 右移0.2, i.e. a step of 0.2 per click, unit not on the
form), buttons 变形 / 变瘦 / 变胖, 恢复默认 ("当前版本恢复默认只可恢复变形、变胖、变瘦方面变动"),
应用至同字符 ▾ (应用至同字符 / 应用至同字符(同时变形)), red links 字符重复轨迹优化 (tooltip 对选中字符所在的字体进行整个的重复轨迹优化, i.e. it processes the whole font, not only
the character) and 异常宽度字符处理 (static name 异常宽度字符优化); the buttons 应用至同字符 and 同步至服务器字体 are on the
form [ui-form]. Needs a selected character ("请选中字符后使用"). 变形 needs deformation on
("请先在设置窗口中启用变形配置"). 同步至服务器 only for 超级管理员 (`font/admin_tune_fonts`) (C §字符微调)
[UI][high].
**chixie-rs today:** missing.
**Recommended implementation:** per-character overrides stored in the document:
`CharTweak { dx, dy, scale_x, font: Option<FontId>, size: Option<f32>, reroll: u32 }` keyed by
char id (`reroll` bumps the RNG salt = "变形" again). "应用至同字符" copies the tweak to all
chars with the same `char`. Glyph clean-ups call `glyph_fix.rs` (§3.3). A side panel instead of a
modal dialog; live preview on the page. No server sync.

#### Route optimisation (书写优化 / 路径优化)
**ChiXie3:** N1 card menu 书写优化 dialog (2025-09-14): 无优化 / 简化路径 / 优化方案1 / 优化方案2 =
`RouteImproveNone / Simplify / CatmullRom / Uniform`, value `RouteImproveValue`, button
应用至选中字符; the form `UI_FontWritingImproveDialog` [ui-form] confirms 书写优化, 优化规则： (combo), 扩张数量：
(spin box; presumably `RouteImproveValue`, inferred), 恢复默认, 应用至选中字符; at write time `GenerateGcodeOfWritingWithImprove` applies it with progress
"书写路径优化中". Simplify = Douglas-Peucker (`epsilon`); 方案1 = Catmull-Rom densification
(`num_points`); 方案2 = uniform arc-length resampling (A §Writing improve; C §路径优化) [high].
**chixie-rs today:** missing (gcode drops consecutive duplicate points only, like ChiXie3 [cap]).
**Recommended implementation:** core `path.rs`: `simplify_dp(&[P], eps)`,
`catmull_rom(&[P], per_seg)`, `resample_uniform(&[P], step_mm)`; applied in `gcode::emit` from a
`RouteImprove` setting in `DeviceProfile` (it is a machine-smoothness choice, not a layout one).
Default: simplify with ε = 0.02 mm (fewer lines, faster streaming, invisible on paper). Tests:
DP keeps endpoints; resample spacing within 1%.

### 3.5 Typesetting (排版)

#### Input typesetting and line filling (输入排版 / 换行 / 回车)
**ChiXie3:** insert at the caret, push the rest of the paragraph back; the row is chosen by
comparing item width (+ predicted width) with the remaining row width; a missing next row in a
note creates a new page; in a table cell the cell gains a row, scales, or records overflow.
Return adds a `LineBreakItem` (and can add a page); Space inserts a random-width space by the
previous char's class; Backspace at row start pulls rows up (C §输入排版) [high].
Captured: wrap at ≈ 168 mm writable width on A4 (protocol.md [cap]).
**chixie-rs today:** `layout.rs` full relayout with wrap and paging per template.
**Recommended implementation:** keep full relayout; VERIFY whether the wrap tests glyph origin
or right edge (one more capture with a near-margin glyph).

#### Composition grid typesetting and word segmentation (作文格排版 / 分词)
**ChiXie3:** input is split into words (jieba with bundled `jieba_dict.txt`, digits kept
together) so a word is not broken across grid rows; blocks that do not fit wrap down; two digits
can share one cell (`generate_combined_text_item`) (C §作文格排版) [med-high]. A server endpoint
`text_processing/text_block` also exists (A).
**chixie-rs today:** grid places one char per cell, no word keeping.
**Recommended implementation:** do not keep words together by default in 作文格: Chinese school
composition paper breaks anywhere (inferred; check with users). Implement only the parts that
matter: punctuation rules (below) and "two digits / two Latin letters per cell" as an option.
If word keeping is wanted, use the `jieba-rs` crate (pure Rust port, bundles its dictionary).
**Open questions / VERIFY:** whether ChiXie3 actually moves whole words (capture a long word at
the row end).

#### Line-start / line-end punctuation (避头尾 and 字符位置合理性检测)
**ChiXie3:** no automatic 避头尾. After typesetting a background pass flags rows whose first char
is a closing mark (`PostPunctuation`) or whose last char is an opening mark
(`LeadingPunctuation`) and paints them with the warning colour; a badge "存在一些标点符号位置异常 /
点击处理" (A §Main window; C §字符位置合理性检测) [med]. Punctuation sets (A @944dfb9, @944e001):
opening `([{·‘“〈《「『【〔〖（．［｛￡￥`; closing `!),.:;?]}¨·ˇˉ―‖’”…∶、。〃々〉》」』】〕〗！＂＇），．：；？］｀｜｝～￠%`.
**chixie-rs today:** missing.
**Recommended implementation:** real kinsoku in `layout.rs` (better than ChiXie3): when a closing
mark would start a line, either (a) hang it in the margin (default for 作文格: draw it in the
last cell's lower right, as Chinese composition rules require) or (b) pull the previous char
down (notes). When an opening mark would end a line, push it down. Use ChiXie3's sets as the
default tables (configurable). Tests: "你好，" at a forced wrap never starts a line with "，".

#### Full-width / half-width conversion (全角/半角)
**ChiXie3:** `FullAndHalfWidthUtil.auto_convert` runs before glyph generation (`Q2B`, `B2Q`,
two mapping tables) (C) [med]; direction per context not recoverable.
**chixie-rs today:** Missing.
**Recommended implementation:** convert only when the chosen font lacks the glyph in the typed
width (e.g. font has `，` but not `,` → use `，`). Implement as a fallback step in
`FontSet::pick`; the text itself is never changed.

#### Content moves and overflow in table cells (内容整体上下移 / 单元格内容溢出处理 / 批量溢出处理)
**ChiXie3:** typing past a cell's rows shows a warning button; editing that cell is blocked
("该单元格存在内容溢出情况，请处理溢出后再编辑排版！"). Dialog plans: 方案一 缩放字符 (fit by shrinking;
unusable when line breaks exceed rows), 方案二 增加行数 (add rows and shrink; optimal row count
computed in a thread), 方案三 忽略内容 (drop overflow); with recommendations like
"(当前行数已是最优行数，方案2不推荐)". 批量溢出处理 applies one plan to all overflowing cells and shows
the count ("没有单元格内容溢出") (C §Table cell overflow) [high]. Exact forms [ui-form]:
`UI_ContentOverflowDialog` (title 内容溢出) shows the 溢出内容 text area and three plan groups: 方案1 (仅减小字符尺寸)
with 当前尺寸： and 预期缩至：; 方案2 (增加单元格行数) with 最优行数：, 当前行数： and 需缩放字符：; 方案3 (忽略溢出内容) with the
button 清空/忽略溢出内容. `UI_BatchOverflowHandlingDialog` (title 批量溢出处理) has two parts: 单个单元格溢出处理 with the note
"点击溢出单元格左上角的警告标志，即可处理单个单元格溢出" (the warning badge sits at the cell's top-left corner), and three radio
buttons 优先缩放字符 / 优先增加内部行数 / 优先忽略溢出内容, the counter 当前溢出单元格： ("0 个" placeholder) and the
button 开始处理.
**chixie-rs today:** missing.
**Recommended implementation:** make overflow a layout result, not an editor state:
`CellLayout { fitted: bool, overflow_from: Option<usize> }`. Cell policy per cell:
`Overflow::Shrink { min_size }` (default: binary-search the largest size ≤ the cell size that
fits, ≥ min), `Overflow::AddRows { max }`, `Overflow::Clip` (overflow drawn red in preview, never
plotted). No editing block; a status-bar badge lists overflowing cells with a "全部处理" action.
Tests: shrink fits a 40-char text into a 2-row cell; clip never emits overflow strokes.

#### Combine characters in a grid cell (合并内容)
**ChiXie3:** 作文格: select ≥ 2 consecutive chars → one cell holding a combined glyph (each squeezed
horizontally). Errors "合并内容只能合并两个以及以上的字符", "作文格模式合并内容只能选中连续文字内容" (C)
[high]. Ribbon button 合并内容 [UI].
**chixie-rs today:** Missing.
**Recommended implementation:** a `CharTweak`-like run flag `combine_with_next` honoured by grid
layout (scale_x = 1/n). P3.

#### Re-typesetting and threads (重排版)
**ChiXie3:** `ReTypeSettingContinuousItems` pulls items out and re-inserts them; tables use
`ReNestingTableCell`. Work runs on Qt threads with flags to block input (C) [high].
**chixie-rs today:** Missing (see §2).
**Recommended implementation:** not needed as a feature: chixie-rs relayouts the document from
the model (§3.1). Keep layout off the UI thread only if measurements require it.

### 3.6 Device and connection (设备管理)

Two device families with different control UIs (A summary) [high]:
- **炽写 devices** (`ChiXie_N1`, `ChiXie_ESP32`, firmware `ChiXie_ESP32_Grbl`): a "device card"
  in the 炽写设备管理 drawer with speed / direction / pressure modes.
- **Other devices** (firmware `Grbl`, `Marlin`, `ESP32_Grbl`, `ChiXie_ESP32_Grbl`; six device
  types): the plain 设备管理 panel with a parameter form. This is the panel seen in the live walk.

#### Device management panel (设备管理)
**ChiXie3** [UI]: status-bar 设备未连接 opens a docked left pane: header 设备管理, [+] 添加设备,
更多 ▾ (切换炽写设备管理, ✓命令窗口, ✓坐标系设置, 自定义命令, 固件参数, 参数导入/导出 ▸ (设备参数导入,
设备参数导出), 更多设置 ▸ (抬笔高度设置, 写完位置设置)), × close. Tab 连接与控制: 端口号 `COMn (CH340)` ▾ +
refresh, 设备名 ▾, ☐自动连接设备, [连接], sub-tabs USB | 无线, jog pad (↑↓←→ + centre = home),
[抬笔] toggle with ▲▼ (Z trim), 单位 5mm ▾, [设置原点]. Tab 参数设置: 设备名, 固件 ▾, 设备类型 ▾, 写字速度
19000 毫米/分, 落笔后暂停 0 秒, 落笔高度 2 毫米, ☐设为默认设备, [炽写ESP主板设置], 坐标系 Y+ ▾ / X+ ▾
direction pickers, [delete] [保存]. While writing most controls are disabled
("请先连接设备。如果已连接，则先停止书写任务"). Pen-down label follows the type: 落笔高度 (stepper),
落笔角度 (servo), 落笔指令 (custom) (A @9456de6). Form `UI_DeviceManagement` [ui-form]: tab widget
连接与控制 / 参数设置; connect tab labels 设备名：, 自动连接设备, 端口号：, 刷新端口号, 无线设置, 刷新局域网内网络设备,
手动添加网络设备, 删除添加的网络设备 (the 无线 sub-tab has its own connect button), 回到原点, "点击将笔落下至设定值"
(pen-down button tooltip), 抬笔Z轴微调 + / −, 设置原点, a 绘图控制： card with start and stop buttons
(`plottingStartBtn`/`plottingStopBtn`: run the plotting page; separate from 书写), a command line with a
send button (`sendCommandBtn`); a unit combo `moveUnitComboBox`. Parameter tab labels 设备名, 设为默认设备,
落笔高度, 落笔后暂停, 毫米/分 (unit of 写字速度), 写字速度, 设备类型, 炽写ESP主板设置, 坐标系, a delete button
(tooltip 删除这个设备模板), a save button, and the hint 修改设备参数后，需要保存设备模板; the add button
tooltip is 新增一个设备模板; 固件 is `firmwareComboBox`. The add-device dialog `UI_AddDeviceDialog` [ui-form]
(title 添加设备) has two groups: 炽写官方设备（搭载炽写专用控制板） with 炽写 N1 (plus a detail-link button
and an ESP32 button `addChiXieEsp32Btn` whose caption is not a form string) and 其他设备（使用炽写通用控制板）
with 其他控制板/设备. The main-window module adds status strings 设备未连接 / 已连接 / 未连接, "检测到CH340设备,连接中",
自动连接成功, "未选中端口，请选择端口后再连接", "设备名称和已有设备重复", "设备参数删除确认", "设备参数设置成功"
[ui-form]. The drawer start state comes from
设置 › 常规设置 › 设备管理 (默认收起) = `DeviceManagerStatus` (`DefaultHide` / `DefaultShowOnLeft` /
`DefaultShowOnRight`) [UI][med].
**How it works there:** `DeviceManagement` (A @945a413), config `[DeviceConfig] DeviceParams`
(QVariant map), `DefaultDevice`, `AutoConnect`, `show_command_window`,
`show_coordinate_region`, `CustomCommand`, `CustomNetworkDevices`.
**chixie-rs today:** 设备 ribbon tab and right task pane `device.rs` (port 自动/COMx ★, jog
0.1/1/10 mm, 抬笔/落笔, 设为原点, 回原点, 解锁, live state + position, console, job progress);
`device_params.rs` (参数设置: profile fields, ChiXie3 import, `$$` 机器规格); core `device.rs` actor,
`config::DeviceSettings`, `chixie3_devices.rs` (QVariant decoder).
**Recommended implementation:** keep the task-pane design. Add: multiple named profiles with a
default (`Vec<DeviceProfile>` + `default: String`), 自动连接 (below), a 单位 choice matching
ChiXie3's jog list (0.01–100 mm), Z trim ▲▼. Disable editing controls while a job runs (same
rule). Tests: profile switch updates the actor (`Command::SetProfile`).

#### Device types and pen commands (设备类型 / 抬落笔)
**ChiXie3:** types 步进抬笔写字机, 舵机抬笔写字机, 电磁铁抬笔写字机, 激光雕刻机, 3D打印机, 自定义抬落笔指令设备
(config `deviceType` 0..5 in this order, [cap] protocol.md). Pen literals: `G1G90 Z<v>`
(stepper; also servo in the capture [cap]), `M3S<angle>` and `G1 Z` exist for other branches
[med]; custom type sends the user's raw strings; 3D printer adds offsets and an extrusion move
(A, C) [med]. `pauseTime` (0 / 0.1 / 0.2 / 0.5 / 1 s) should emit `G4 P` but was not sent over
Telnet [cap].
**chixie-rs today:** `gcode::DeviceType::{StepperPenUp, ServoPenUp, ElectromagnetPenUp,
CustomZ}`; real `G4 P<s>` dwell.
**Recommended implementation:** add `Laser { power_s: u16 }` (pen down = `M3 S<power>` /
`M4`, pen up = `M5`, with `$32=1` laser mode check) and `PenPlotter3D { z_up, z_down,
offset }` only when a user needs them (P3). Keep explicit feeds on pen moves.
**Open questions / VERIFY:** ChiXie3's laser and 3D-printer pen literals (capture with
type = 激光雕刻机).

#### Device parameter import/export (参数导入/导出, `.cset`)
**ChiXie3:** import "选择设备参数文件" filter "炽写设备参数文件 (*.cset)"; same-name devices get
`-new` after a confirm; "设备参数导入成功，重启软件后即可加载". Export: dialog 导出设备参数 with a
selectable list of devices (`listWidget`) and one export button [ui-form]; then folder, file
"<设备名> - 设备参数.cset". The file is a Python **pickle** of the `DeviceParams` dict (A §Device
parameters) [high] — importing a foreign `.cset` in ChiXie3 can execute code.
**chixie-rs today:** import from `config.ini` (`chixie3_devices.rs`).
**Recommended implementation:** export/import chixie-rs profiles as TOML (`.chxdev`). Import
`.cset` through the existing safe pickle reader in `chixie3_templates.rs` (generalise it into
`pickle.rs`: opcodes only, never resolve classes). Tests: fixture `.cset` (build one with Python
locally; do not commit user data).

#### USB serial connection and port discovery (USB 连接)
**ChiXie3:** probes baud rates ("【grbl】正在尝试波特率:", Marlin has its own probe); ports labelled
"（CH340）" (description `USB-SERIAL CH340`) or "（蓝牙）" ("蓝牙链接上的标准串行"); GD32 virtual COM for
N1; Linux/macOS `/dev/`. Default port priority CH340 first. Non-device port warning
"此端口应该不是设备端口，连接可能会导致闪退，确认要尝试此端口吗？" (A) [high]. Errors "端口连接超时",
"连接失败，建议更换usb口尝试", "串口连接失败". Verified hardware: 115200 baud (protocol.md [cap]).
**chixie-rs today:** Done: `transport.rs` (serial + telnet `LineLink`, CH340/GD32 auto-detect),
`Target::Serial { port: None }` = auto.
**Recommended implementation:** label Bluetooth SPP ports ("（蓝牙）") from the port description
(`serialport::SerialPortType::BluetoothPort` / description text); try 115200 first, then
250000 / 57600 only if `$I` gets no reply within 1 s. Known USB-drop behaviour (CH340 returns
on a new COM number, memory `chixie-board-usb-drop`): auto mode re-scans by VID/PID.

#### Auto connect (自动连接机器)
**ChiXie3:** checkbox; at start (config `AutoConnect`) and when ticked, a thread waits for a CH340
port to appear and connects: "自动连接设备，自动获取端口号中", "检测到设备接入,连接中", "自动连接成功" (A) [high].
**chixie-rs today:** missing.
**Recommended implementation:** in the device actor, an `AutoConnect` mode polling
`list_ports()` every 1 s while disconnected and not user-disconnected; also used after a USB
drop during a job (connect only; never auto-resume a job). Test with `grbl::fake`.

#### CH340 driver check (CH340驱动检查)
**ChiXie3:** after first show, "检测到未安装CH340驱动，是否下载并安装？" [立即安装] [不再提示]; downloads
`CH341SER.EXE` from the vendor server (A) [high].
**chixie-rs today:** Not planned.
**Recommended implementation:** not planned (download + run an EXE is a security risk). Show a
help link when no port is found.

#### Alarm unlock and device verification (Grbl报警解锁 / `$WHO`)
**ChiXie3:** on connect "开始进行Grbl状态检查"; in Alarm: "Grbl控制板当前为“警报”状态，是否发送解锁指令？（解锁后才可正常使用）"
[立即解锁] → `$X`. Non-炽写 boards are checked with `$WHO`; unknown boards get
"未经认证的设备，请谨慎使用！！可能即将停止支持！！" (A) [high].
**chixie-rs today:** 解锁 button; Alarm shown in state.
**Recommended implementation:** on `Connected`, if status is `Alarm`, show a toast with 解锁.
Do not warn about "uncertified" boards (vendor lock-in; chixie-rs supports any Grbl).

#### Grbl error translation (Grbl错误翻译)
**ChiXie3:** `error:N` → Chinese texts for all classic Grbl codes (expected command letter, bad
number format, invalid `$`, negative value, homing disabled, step pulse < 3 µs, EEPROM read fail,
`$` not idle, locked (alarm/jog) with `$X` hint, soft limit needs homing, line too long, max step
rate, safety door, startup line too long, jog travel, jog syntax, unsupported G-code, modal
group, undefined feed, integer required, hard limit, target beyond travel, reset while moving,
homing failures); title "设备报错提示"; unknown "控制板返回了一些未知异常报错：" (A §Grbl error) [high].
**chixie-rs today:** raw error lines in the console.
**Recommended implementation:** `grbl::error_text(code) -> &'static str` table (codes 1–38 and
alarms 1–10, Chinese text written fresh from the Grbl wiki meanings, not copied); shown in the
toast and console. Test: every code maps.

#### Wireless connection (无线 / Telnet)
**ChiXie3:** only ESP32 boards ("目前仅支持ESP32类型的控制板与相关固件进行无线连接"); Telnet to
`wireless_ip:23`; failure "无线连接失败，请检查设备是否已正确配网并联网" (A) [high]. The fake-Grbl capture
used this path; ChiXie3's Telnet streaming stalls on long jobs (~44, ~752, ~1682 lines) and only
disconnect + restart recovers (protocol.md [cap]).
**chixie-rs today:** `Target::Telnet` in core; GUI 无线 entry; untested on hardware.
**Recommended implementation:** same char-counting streamer as serial; add TCP keepalive and a
read timeout with `?` probing; on a stall (no `ok` for 5 s while `?` answers `Run`/`Idle`), resync
by counting outstanding bytes from the status `Bf:` field (Grbl_ESP32 reports it) instead of
hanging. Test with a fake that drops one `ok`.

#### Wireless discovery and manual devices (无线发现 / 手动添加网络设备)
**ChiXie3:** 刷新网络设备: `arp -a` (gbk), `GET http://<ip>/description.xml` (UPnP), entries
"[局域网] 模块类型:… 制造商:… 设备IP:…"; manual add (IP, port, name, ☐使用本地默认设备名; IP regex;
"该设备名已存在，请重新输入") stored under `CustomNetworkDevices {name: {ip, port}}`, shown "[自定义]";
only custom ones deletable (A) [high]. Form `UI_AddNetworkDeviceDialog` [ui-form]: title 添加网络设备,
rows 设备名：(line edit) with checkbox 使用本地默认设备名, IP地址：(line edit), 端口号：(spin box), one
confirm button.
**chixie-rs today:** `discovery.rs` (untested), manual IP in settings.
**Recommended implementation:** keep arp + UPnP but add mDNS (`_telnet._tcp`/ESP3D advertises
`http`) via the pure-Rust `mdns-sd` crate (new; reason: works without arp parsing and on
macOS/Linux). Manual list in settings TOML.

#### ESP32 wireless setup (配网 / WirelessEsp32SettingDialog)
**ChiXie3:** reads the mode (`[ESP110]`), scans Wi-Fi (`[ESP410]`, signal icons), modes
WIFI客户端模式 (STA), 蓝牙模式, 禁用无线(暂未开发); results "配网成功(…)，请查看无线面板中设备列表",
"WIFI客户端模式(STA模式)配网失败", "蓝牙模式设置成功，请在电脑中连接该蓝牙，刷新软件内串口，连接相应新增的端口即可。"
(A) [high]; command table in protocol.md. Form `UI_WirelessESP32SettingDialog` [ui-form]: title 无线模式配置,
three fields 无线网络模式：(combo; its items are filled in code), WIFI密码：(with a Wi-Fi list view and a
stacked page per mode), 蓝牙名称：; one confirm button. The separate 炽写ESP主板设置 dialog (below) carries
the richer set of modes (None(有线), WIFI-STA模式, WIFI-AP模式, 蓝牙模式).
**chixie-rs today:** missing.
**Recommended implementation:** `esp3d::wifi_scan/set_sta/set_bt` over the serial link
(`[ESP410]`, `[ESP100]`/`[ESP101]`, `[ESP110]`, `[ESP115]ON`); dialog with an SSID list and a
password field (password never stored, never logged). P2.

#### Firmware parameters (固件参数)
**ChiXie3:** singleton dialog, two modes. Easy: sliders for speed (snap to 1000 on release,
labels 有点慢 … 超级快) and steps; writes `$110–$112`, `$120–$122`, `$100–$102`, `$1`, `$3`.
Expert: table 参数名 / 参数值 / 参数描述 for every `$` setting (classic `$$` or ESP32 named
settings like `$Sta/SSID`, `$X/StepsPerMm`); 炽写 keys `$X/ChiXie/HomeOrder`, `$ChiXie/AxisFollow`
(0-A/1-X/2-Y/3-Z), `$ChiXie/ForceSetOrign`, `$ChiXie/MachineMode` (0 XY, 1 CoreXY); TMC keys.
恢复默认值: "是否恢复Grbl固件到默认值？(新手小白慎点！…)" → `$RST=*`. Errors "固件参数获取失败，请重试！（1、请不要书写中打开此窗口。2、请检查固件类型是否正确）"
(A §Grbl firmware) [high]. Marlin: "暂时不支持该固件类型的固件参数查看". Form `UI_FirmwareParameterDialog`
[ui-form] (title 固件参数设置; stacked easy / expert pages): easy page groups 三轴最大速度, 三轴加速度,
三轴步进当量 with rows 三轴最大速度：, 三轴加速度：, 三轴步进当量：, 电机锁定： (checkbox 保持锁定, with the hint
"保持锁定更稳定，但可能电机会有声音（正常现象），驱动板与电机质量越好声音越小") and 方向： (combo: 反转X, 反转Y,
反转XY, 反转Z, 反转XZ, 反转YZ, 反转XYZ, plus an unreversed entry set in code; = `$3`). Three recommendation
hints sit beside the rows: "一般设置 8000 即可，比较稳定", "一般设置 80。（常见数值就是80、100、160）",
"一般设置 4000 即可，比较稳定，舵机抬笔可以设置更小点" (which hint belongs to which row is not recoverable from the
constants). The expert page is a table with row-group labels 三轴步进当量, 运动速度 / 移动速度 / 抬落笔速度, 运动加速度 /
移动加速度 / 抬落笔加速度 (i.e. each of speed and acceleration is split into drawing, travel and pen-lift
values); buttons: save, import and export of parameters (a parameter file) and 恢复默认. So the dialog also
has parameter import/export, which this section's earlier text did not list.
**chixie-rs today:** `$$` read and shown (`grbl::MachineSettings`, 机器规格).
**Recommended implementation:** an expert table only (sliders hide units): rows from `$$`, editable
value, description from a local table, "改动" highlight, 应用 sends only changed keys, each acked;
`$RST=*` behind a typed confirmation. Read back after writing. Block while a job runs.

#### 炽写 ESP32 board settings (炽写ESP主板设置)
**ChiXie3:** three pages: `$Stepper/IdleTime`, `$ChiXie/AxisFollow`, `$ChiXie/MachineMode`,
`$Spindle/PWM/Frequency` (presets + custom kHz, "自定义PWM频率不能小于等于0"), X/Y/Z/A
steps/max rate/accel; radio `$Radio/Mode` None/STA/AP/Bluetooth, `$AP/SSID`, `$Bluetooth/Name`;
AP success "AP模式配网成功，请用电脑连接控制板WIFI后，在浏览器打开192.168.0.1使用"; 恢复默认值 `$RST=*`
(A) [high]. Refused on other boards ("当前控制板不是炽写ESP32控制板，无法使用专属设置界面").
Form `UI_ChiXieESP32FirmwareSettingDialog` [ui-form], title 炽写ESP32固件设置, left pages 常规设置 / 轴参数设置 /
无线通信设置. 常规设置: A轴模式：(独立A轴, 跟随X轴（双X）, 跟随Y轴（双Y）, 跟随Z轴（双Z）; = `$ChiXie/AxisFollow`),
锁定电机：(switch), 传动类型：(combo, items in code; = `$ChiXie/MachineMode`), 主轴PWM频率：with radio buttons
1 kHz, 5 kHz, 10 kHz and 自定义 plus a line edit. 轴参数设置 (X/Y/Z/A each): 运动反向 (switch), 回原点顺序
(combo), 脉冲数/mm, 最大速度mm/min, 最大加速度, 回原点反向 (switch) — so axis direction and homing
direction are separate per-axis switches, which the text above did not list. 无线通信设置: 无线网络模式 (None(有线),
WIFI-STA模式, WIFI-AP模式, 蓝牙模式), "有线模式无需额外设置", a Wi-Fi list + password for STA, AP SSID + password
with the note that a default AP password exists (the form text states the value; not repeated here), 控制板蓝牙名称,
and the button 立即配置 with the caption （无线配置请点这里保存）. Footer: 帮助教程, 恢复默认, save, back.
**chixie-rs today:** Missing.
**Recommended implementation:** P3; a preset view over the expert table (filter to these keys).

#### 炽写 N1 device card and drawer (炽写设备管理 / 设备卡片)
**ChiXie3:** drawer header 炽写设备管理 (lock, hide, 添加设备, 切换通用控制, 设备购买). Card: image, 连接
toggle, menu 绘图/激光, 固件更新, 命令窗口, 用户指南, 删除设备, 设备测试 (super_user), 高级设置, 书写优化; jog
with units 0.1/1/10/20 mm and a slider 1–200 mm; 回机械原点 `$H`, 绝对抬笔 (to the limit switch,
probably `$HZ`), 抬笔 `G1G90 Z0 F5000` / pen down `G1G90 Z<v> F5000`, 设置原点 `G92 X0 Y0`, 回到原点
`G90G0 X0 Y0 F5000`. Parameters: 运动模式 (`SlowSpeedMode, CommonSpeedMode`=常规（推荐）,
`HighSpeedMode, BerserkMode, SlowSpeedMode1`), 写完位置 (原点 / 原点左2cm处 / 任务结束点), 写完提示音
(`$Beep 843 843 840 843 840 841 843 840 845`), 方向模式 (竖向（默认） / 竖向(翻转) / 横向(翻转) / 横向),
落笔轻重 (更轻Ⅱ / 更轻Ⅰ / 标准(默认) / 更重Ⅰ / 更重Ⅱ), 空驶抬笔, 自动原点; saved under
`DeviceConfig/ChiXieDevices`. Connect sends `$HZ`; one 炽写 device max ("暂不支持添加多个炽写设备") (A
§ChiXie device UI) [high]. macOS hides "other device" (A) [high].
**chixie-rs today:** missing (generic panel only).
**Recommended implementation:** model the N1 as presets over the generic profile instead of a
second UI: `speed_mode` → feed pair, `pressure` → pen-down Z, `direction_mode` → `AxisMap` +
180° flag, `done_position` → end move, `done_beep` → end lines. One panel for all devices.
Numeric tiers are unknown: capture each mode once with the fake Grbl (the N1 path uses the
same G-code family). P2.
**Open questions / VERIFY:** speed tiers, pen depths, which mode is 横向.

#### 炽写 N1 advanced settings (炽写设备高级设置)
**ChiXie3:** card menu 高级设置 opens dialog 炽写设备高级设置 with two pages [ui-form `UI_ChiXieDeviceFirmwareSettingDialog`].
常规设置: 归位强制设置原点 (switch; cf. `$ChiXie/ForceSetOrign`), A轴模式： (独立A轴, 跟随X轴（双X）, 跟随Y轴（双Y）,
跟随Z轴（双Z）), PWM阈值 (tpwmthrs), 失速电流阈值 (tcoolthrs), 禁用蜂鸣器：(switch), 锁定电机：(switch), 传动类型：
(combo, items in code). 轴参数设置, one column per axis X / Y / Z / A: 原点偏移, 回原点顺序, 脉冲数/mm, 驱动运行电流
(run current combo), 驱动保持电流 (hold current combo), 最大加速度, 驱动细分 (microstep combo), 驱动模式 (combo),
最大速度mm/min, 失速检测灵敏度SGTHRS (spin box). Footer buttons: 一键配置设备参数 and 一键配置设备参数为出厂设置,
保存, 返回. The names (tpwmthrs, tcoolthrs, SGTHRS, IRun/IHold, Mres) are TMC stepper-driver registers, so the N1
exposes TMC2209-style StealthChop/StallGuard tuning; the values are written as `$` settings (inferred, cf.
the `$ChiXie/…` and TMC keys in 固件参数).
**chixie-rs today:** Missing.
**Recommended implementation:** P3. A preset view over the expert table in 固件参数 (filter to the same
keys) with plain-language group names; the 出厂设置 button must show the factory values before sending.
Hide on non-炽写 boards (same `$WHO` gate as 炽写ESP主板设置).
**Open questions / VERIFY:** the real `$` key per field, value ranges, factory values, combo item lists
(capture the dialog once on an N1 with the fake Grbl).

#### N1 firmware update and device test (固件更新 / 设备测试)
**ChiXie3:** update dialog 固件更新 shows `$WHO` info (UUID, version, newest version); guide "请按住抬笔电机旁的解锁按钮，然后点击开始。"
(App and Boot images; transport unknown). Form `UI_ChiXieFirmwareUpdateDialog` [ui-form]: labels 型号：(device
type, e.g. ChiXie N1), 序列号：, 固件版本：, 最新固件： with the state text 当前已为最新, an 更新说明 area and the button
更新固件. 设备测试 (admin only; form `UI_ChiXieDeviceTestingDialog`, title 炽写设备测试) has a 速度： combo and nine test
buttons [ui-form]: 落笔测试, 回字方框, 蜂鸣器测试, 字符阵列, 自动抬笔测试, 交叉网格, 锯齿折线 (45° zigzag
50 mm, 10 mm pitch, multiple speeds, tests lost steps), 同心圆与螺旋线, 书写测试 — more than the "buzzer, zigzag,
基础图形" set earlier recovered from identifiers (A) [med].
**chixie-rs today:** Not planned.
**Recommended implementation:** not planned (vendor firmware; risk of bricking). A generic
"测试图形" job is useful for any machine: generate it in core (P2, S) with the same set as 设备测试 minus
the buzzer: 回字方框 (nested squares), 交叉网格, 锯齿折线, 同心圆与螺旋线, 字符阵列 (a glyph grid), and a pen-down
test.

#### Manual controls and console (手动控制 / 命令窗口 / 自定义命令)
**ChiXie3:** jog `$J=G21G91 X.. F..` with axis mapping; 设置原点 `G92 X0 Y0 Z0`; 回到原点
`G90G0 X0 Y0`; pen toggle; Z trim `G21G91 Z±`; command line with echo (`<<<< 指令执行异常` on error),
hint "Gcode一般由字母和数字组成，您输入的指令可能有误". 自定义命令: list + editor (按钮名, 命令), buttons in a
自定义区 grid, stored `CustomCommand {name: cmd}` (A) [high]. Form `UI_CustomCommandDialog` [ui-form]: title
自定义命令按钮, list 已有按钮, single-line 按钮名称, multi-line 按钮命令 (a command may span several lines), buttons
保存/添加 and delete.
**chixie-rs today:** jog pad, pen, zero, home, unlock, console in `device.rs`.
**Recommended implementation:** add custom command buttons (TOML list) and command history (↑/↓)
in the console. Block custom commands during a job (same as ChiXie3).

#### Status-bar position (坐标位置显示)
**ChiXie3:** a timer polls position; status bar shows 已连接/未连接 and coordinates (A) [high].
**chixie-rs today:** Done (`?` every 250 ms, live position).

### 3.7 Job execution (书写任务)

#### Writing management (书写管理 / 作业框)
**ChiXie3:** ribbon job box: 书写 (pen) ▶ ■ and "第 1 页 / 共 0 字 / 未书写" [UI]; drawer page list
with thumbnails, per-page progress, 开始书写 / 暂停书写 / 继续书写 / 停止书写; table model per page:
页码, 字数, 书写状态 `未书写 / 书写中 / 暂停中 / 已书写`
(`UI_WritingManagementWidget` is just a table view; the state texts, 暂停书写 / 继续书写 / 开始书写 / 停止书写
and 开始书写序号为： are main-window strings [ui-form]). Start runs the online check, builds a task per
page (`initial`, `body`, `end`), enables multicolour, starts the serial thread (A, C §书写管理)
[high]. Empty task: "书写任务为空，请手动选择任务条目后点击书写，或在页面右键书写尝试".
**chixie-rs today:** Done: 设备 tab 开始书写 / 暂停 / 继续 / 停止, `device::Job { pages }`,
`Event::Progress`, start confirmation with extents and a 300 mm travel warning, Stop and Done
park at X0 Y0.
**Recommended implementation:** add the per-page list (status + character count + thumbnail) to
the task pane and "write from page N". Pause = feed hold `!` (immediate) rather than ChiXie3's
"stop sending" (which lets the planner buffer drain). Keep the online check out (offline-first).

#### Write this page (书写此页 / 直接书写)
**ChiXie3:** page context menu 书写此页 ▸ 直接书写: confirm "书写确认: 是否立即书写第 N 页…"; errors
"页没有书写内容", "设备未连接", "当前有书写任务正在进行中，请先结束任务" (C) [high].
**chixie-rs today:** Partial.
**Recommended implementation:** `Job { pages, start_page }`; context menu + task-pane button.

#### Sheet change (换纸)
**ChiXie3:** manual: pages are written one at a time by user click; automatic only with the
paper-feeder device (A §Page writing management) [high].
**chixie-rs today:** Done: `Event::AwaitingSheet` → 换纸 dialog → `Command::NextSheet`
(ribbon 已换纸，继续).
**Recommended implementation:** keep. Optional "每页后自动暂停" vs "连续书写" for continuous paper.

#### Break-point resume (断点续写)
**ChiXie3:** after a stop, pen run-out, paper slip or disconnect ("设备异常断开，请重连设备。如需续写，可右键页面使用断点续写"),
the user opens 断点续写 from the page menu, selects **the first not-yet-written character**
(exactly one non-space char: "请选中待续写区文本中第一个字符，不要选中多个", "请选中有内容的字符，不要选中空格"),
and presses 继续书写; requires connection and no running task. The generator re-emits the page
skipping items before the chosen one (`can_continue_write`), with initial and end G-code (C
§断点续写) [high]. Failure "断点续写异常，请将日志发给客服". Form `UI_ContinueWriteFromBreakPoint` [ui-form]:
title 断点续写, 续写起始字符： showing the selected character (未选中 until one is selected), hint
"提示：请选中续写的起始字符，然后点击开始", warning "注意：请保证已经设置过原点", button 开始续写 (so the dialog is
presumably non-modal, letting the user click a character while it is open; inferred).
**chixie-rs today:** on disconnect, a toast with page and last acknowledged line
(`Event::Disconnected { page, last_line }`); no resume.
**Recommended implementation (better):** two resume modes. (1) **Automatic, by line**: the actor
knows the last acked line; on resume it re-emits from the start of the stroke containing that
line (pen up, travel, pen down), so no character is redrawn or lost. Each G-code line carries its
char id in a side table (`Vec<(line, char_id)>`) so the UI can also show "停在「X」". (2)
**Manual, by character** (ChiXie3's): click a character → resume from its first stroke. Because the
origin may have moved (paper slip), offer "重新设置原点后续写". Tests: fake link drops at line 120 →
resumed stream starts with pen up + travel to the stroke start of line ≤ 120 and ends identical
to the uninterrupted tail.

#### Multicolour drawing and pen change (多色绘制 / 换笔)
**ChiXie3:** 设置 › 多色绘制: ☐多色绘制; 换笔模式 ○手动换笔(弹窗提示) ○自动换笔(需设备支持); 自动换笔配置 共1支笔
[设置]; preview box [UI]. Exact form labels: 手动换笔（弹窗提示）, 自动换笔（需设备支持）, 自动换笔配置, 共1支笔
(a pen-count summary, i.e. the automatic mode has a configurable list of pens), 换笔模式 [ui-form]. Config
`EnableMulticolorDrawing=false`, `PenChangingMode=1`. Strokes are
grouped by the item's colour (`getRgb`, default black); at a colour change the sender pauses with
"换笔提示: 您已启用多色绘制，请更换笔颜色为：<colour>" and [换笔完成] (A §Pen change prompt) [high].
Automatic mode implementation not found [med].
**chixie-rs today:** missing.
**Recommended implementation:** `layout` keeps per-stroke colour from `CharAttrs.color`; `gcode`
orders a page by colour (all black strokes, then red, …; within a colour keep reading order) and
inserts a `PenChange { color }` marker between groups. The actor turns markers into
`Event::AwaitingPen { color }` (same flow as sheet change). Automatic changers: a per-profile
`pen_slots: Vec<PenSlot { color, pick_gcode, drop_gcode }>` macro list. Tests: two-colour page →
one pause; colour order stable.

#### Page rotation for odd/even pages (页面旋转 / 奇偶页方向)
**ChiXie3:** 设置 › 页面旋转 › 页面旋转模式 (不旋转 default) with note "(默认不旋转，默认起点为左上角) 页面预览不旋转，只旋转书写模式输出"
[UI]. Modes `SameDirectionForParity`, `OddPagesRotate180`, `EvenPagesRotate180`,
`SameDirection180` (`[ParityPageConfig] ParityPageDirectionMode=0`); the form labels are 不旋转,
奇数页旋转180°, 偶数页旋转180°, 旋转180° in that order [ui-form], which pins the mapping of the four identifiers
(order inferred, integer values still unconfirmed); the generator mirrors each
point about the paper/frame centre for rotated pages (`calculate_symmetric_point`) and the origin
marker moves accordingly (A, C) [high]; numeric mapping [med]. Purpose: writing both sides of a
sheet or a feeder that returns sheets upside down (inferred).
**chixie-rs today:** missing.
**Recommended implementation:** `PageRotation::{None, Odd180, Even180, All180}` in
`PageSettings`; applied in `gcode::emit` as `p → (W − x, −H − y)` relative to the writable area
(origin becomes the bottom-right corner, so the job start hint must say so). Test: All180 twice =
identity; extents unchanged.

#### Workspace patrol and frame drawing (工作区巡边 / 绘制框架线)
**ChiXie3:** page menu 工作区巡边/绘制: 开始巡边 ("是否开始巡边？（请确保已经设置过原点）") traces the
bounding rectangle of the frame or of the written area with the pen up ("没有书写内容，无法巡边");
开始绘制 ("是否开始绘制页面框架线？") draws the template lines (note rows incl. double lines and columns,
table borders, composition grid) with the pen down. Busy/disconnected refusals (A §Workspace
patrol) [high]. Form `UI_DrawingAndPatrolOfWorkspaceDialog` [ui-form] (title 工作区绘制与巡边): group 工作区巡边
with 巡边模式： 抬笔巡边 / 落笔巡边 / 抬笔巡边（顶点落笔）(pen up, pen down, or pen up with the pen touching only at
the corners), 巡边范围： 模板框线区域 / 实际书写区域 (template frame vs written area), button 开始巡边; group
框架线绘制 with 开始绘制 and the note "注：此处是绘制页面中横线/表格/作文格框架". So patrol has three pen modes
and two ranges, not just the pen-up trace described above. Table borders use `TableDrawingLineGenerator` (outer frame one clockwise path,
inner lines ordered to cut pen lifts) (C §绘制表格) [med-high].
**chixie-rs today:** missing.
**Recommended implementation:** `job::patrol(bounds, mode: Up | Down | UpCornersDown) -> String` (pen up
by default, rectangle at travel feed, back to origin; range = frame or written area) and `job::ruling(template, page) -> Vec<Stroke>` (uses `layout::Guide`s).
Patrol button next to 开始书写 ("先巡边" is the best way to check paper placement; make it one
click, no dialog). Merge collinear segments and order lines boustrophedon to minimise lifts.

#### Upload to SD and run (上传SD卡书写)
**ChiXie3:** page menu 上传SD卡书写: uploads the page's G-code to `/chixie_task/` on the board
(`POST /upload`, multipart `myfile[]`) and asks "是否立即开始书写页面：" then runs `[ESP220]<path>`
(A §Upload page to SD; protocol.md) [high]. Only for 炽写 ESP32 firmware.
**chixie-rs today:** core `esp3d.rs` (upload, run, hold/resume/reset) — untested.
**Recommended implementation:** GUI action in the 设备 tab when an IP is known; progress polling via
`?` over HTTP. Benefit: no streaming stalls. Test against a mock HTTP server (`tiny_http` dev-dep,
or a hand-rolled `TcpListener`).

#### Automatic paper feeder (自动换纸设备)
**ChiXie3:** 百宝箱 › 自动换纸控制 ("目前仅适配部分换纸机器"): ▶/■, 当前页码, 书写状态 停止中, 任务总量, 进纸状态
未进纸, [进纸] [退纸], table 序号 / 页码 / 字数 / 书写状态, [导入页面], 任务列表循环 N 次 [UI]. Form
`UI_AutomaticPaperDeviceDialog` [ui-form] confirms the labels 自动换纸控制, 进纸状态：(未进纸), 当前页码：, 书写状态：
(停止中), 任务总量：, 导入页面, 任务列表循环 (spin box N) and has start / stop buttons and give / spool paper
buttons. On connect
sends `M119` and checks `z_max: TRIGGERED` ("检测到有物体遮挡限位开关，请检查后操作"); feed/spool via
Marlin moves on Z/E (`G92 Z0`, `G1 Z500 F9000`, `G1 Z113.84615 E113.84615 F8000`, `G28 Z0`, …);
Marlin only ("不支持的主板或者固件"); close guard "设备非停止状态下，无法关闭自动换纸控制窗口！" (A §Auto
paper device) [high]; hardware identity [med].
**chixie-rs today:** missing.
**Recommended implementation:** P3. Generalise as "sheet change macros": a profile can define
`feed_out` / `feed_in` G-code macros run at `AwaitingSheet` instead of prompting, plus a loop
count. Requires a second link (the feeder is a separate Marlin board): `device.rs` gains a
secondary `Target`. Marlin streaming (ok-per-line, no char counting) needed first.

### 3.8 G-code generation, path planning, start points

#### Page G-code structure (书写G-code生成器)
**ChiXie3:** `get_gcode` → (initial, body, end). Captured over Telnet (protocol.md [cap]):
`G21`, `G90`, `F19000`, pen up twice, travel `G1 X+4.205Y-2.376F19000` (G1, signed, trimmed),
pen down `G1G90 Z2F19000`, writes, pen up after each stroke, end `G90G0 X0Y0`. Origin = pen
position at start; Y negative going down. Consecutive repeated points are not sent. Static:
`_handle_consecutive_negative` fixes `+-`/`--` ("导致N1鬼叫的问题") (A, C) [high].
**chixie-rs today:** `gcode.rs` (`emit`, `emit_document`; travel `G0`, explicit feeds, optional
dwell, axis map), golden tests.
**Recommended implementation:** keep our cleaner format (no signs, `G0` travel with a travel
feed, explicit `F`); add an optional "ChiXie3-compatible" emitter only if a user needs byte
compatibility (not planned). Add a header comment block (`; chixie-rs v…, page n, font, seed`)
behind a setting (some firmware echo comments; Grbl ignores `;`).

#### Path planning (路径规划: 之字型 / 弓字型)
**ChiXie3:** 设置 › 写字偏好 › 路径规划 ○之字型(默认) ●弓字型 [UI]; the sample config has
`path_planning_type=2` with 弓字型 selected, so 弓 = 2 and (inferred) 之 = 1. Rows are ordered by
`_get_content_items_by_path_planning_type(cell, sort_type)` with `left_to_right` /
`right_to_left` (A, C). **Sources conflict on semantics:** A says 之 = every row left-to-right,
弓 = alternate rows reversed (boustrophedon); C says the opposite and calls the second mode 工.
The character shapes (之 = Z-scan, 弓 = serpentine) and the CNC meaning of 弓字形 favour A
(inferred).
**chixie-rs today:** missing (always left-to-right).
**Recommended implementation:** `PathPlan::{Raster, Serpentine}` in `DeviceProfile` (output
order only; the layout is unchanged). Serpentine writes odd rows' characters right-to-left (each
character's own strokes keep their order). Default Raster until VERIFY. Also add a greedy
nearest-neighbour stroke order option within a character for fonts with bad stroke order
(measure travel saved). Test: serpentine reverses glyph order on row 2 only.
**Open questions / VERIFY:** capture two lines with each mode and compare glyph order.

#### Start-point modes (起点模式 / 书写原点)
**ChiXie3:** `StartPointOfPaper` (paper top-left), `StartPointOfFrame` (frame/margin start; uses
`origin_point_of_frame`, `end_point_of_frame` and cos/sin for flips), `StartPointOfCustom` (the
dragged marker); offsets by `get_offset_value`; units px → mm via `PXScale` (A, C) [high].
**chixie-rs today:** origin = writable top-left (matches ChiXie3's captured note default).
**Recommended implementation:** see 书写原点 (§3.2); emitter subtracts `origin_mm` from every
point. Test per mode.

#### Write-done position and end beep (写完位置 / 写完提示音)
**ChiXie3:** `WriteDonePos` 0 原点 `G90G0 X0Y0`, 1 原点左侧2cm `G90G0 X-20Y0` (sign depends on the
axis map), 2 任务结束点 (no move); prompt "请输入数字（0-原点、1-原点左侧2cm、2-任务结束点处）"; 炽写 devices may
append the `$Beep …` melody. Marlin back-to-origin `G90` + `G0 X0 Y0 F<speed>` (A, C) [high].
**chixie-rs today:** always parks at X0 Y0 (BOSS request).
**Recommended implementation:** `EndPosition::{Origin, OriginLeft(mm), Stay, Custom(x, y)}`,
default Origin; "OriginLeft 20 mm" keeps the pen off the written text so the sheet can be
removed. Beep only for boards that support `$Beep` (detected from `$I`).

#### Axis mapping and direction modes (坐标系设置 / 方向模式)
**ChiXie3:** `x_plus/x_reduce/y_plus/y_reduce` labels (`X+`, `X-`, `Y+`, `Y-`) mapped to signed
axes; validation "坐标系中两个不同方向不能为同一个轴"; 炽写 direction modes flip signs and rotate 180°
(A) [high].
**chixie-rs today:** `AxisMap { swap_xy, invert_x, invert_y }`.
**Recommended implementation:** keep; the UI shows a small diagram of the page on the bed with the
pen at the origin, updating live (ChiXie3 uses two drop-downs).

#### Pen-tip ("笔锋") test
**ChiXie3:** `enable_route_tip`: lifts Z gradually along the last segment of each stroke to imitate
a brush tip (`route_tip_test_gcode`, `route_tip_last_z_value`) (A) [high]; labelled 笔锋简单测试.
**chixie-rs today:** Missing.
**Recommended implementation:** P3; only meaningful with a Z stepper. `gcode` option
`tip_taper: Option<(len_mm, z_end)>` interpolating Z on the last `len_mm` of each stroke.

#### Route optimiser plugin (路径优化插件)
**ChiXie3:** if `enable_plugin`, page route data (per text, per stroke points) is passed to
`RouteOptimizer.start_optimizing(...)` loaded from `<user dir>/plugin/test.py` with `importlib`
— user Python code runs in-process (C §书写G-code生成器) [high].
**chixie-rs today:** Not planned (replace).
**Recommended implementation:** not planned as code loading. If extension is needed: an external
filter command (`stdin` = JSON strokes, `stdout` = JSON strokes) run as a child process with a
timeout, opt-in per profile. Safer and language-neutral.

#### Export G-code (导出 G-code)
**ChiXie3:** `exportAction` (visible only to super users [cap] protocol.md): pick a folder, writes
`<n>页.gcode` per page; "Gcode导出成功，共导出 N 页数据"; "页面没有数据，请输入内容或导入文件后再导出！" (A) [high].
**chixie-rs today:** Done (`file_ops.rs`: one file per page, backstage 导出).
**Recommended implementation:** keep; name files `<doc>-p<n>.gcode`; add "合并为一个文件（含换纸暂停 M0）"
option for SD use.

### 3.9 Plotting editor (绘图与激光)

Overall recommendation: build the plotting editor as a **second document kind** in chixie-rs
(`PlotDoc`), sharing paper, device, job and G-code code with writing. Core module `plot/`
(`model.rs`, `geom.rs`, `import_dxf.rs`, `import_svg.rs`, `raster.rs`, `gen/{maze,seal}.rs`),
GUI `plot_view.rs`. Geometry is kept as exact primitives (line, arc, ellipse, cubic) and only
flattened to polylines at emit time with a chord-error tolerance, so zooming the preview and
the G-code agree. New crate candidates: `kurbo` (2-D curves, flattening, arc ↔ cubic; small, pure
Rust, widely used) for geometry; `usvg` for SVG; `dxf` crate for DXF (or a minimal own parser
for the entities in use). Priority P2 overall.

#### Plotting ribbon and canvas (绘图与激光 ribbon / 画布)
**ChiXie3** [UI c-ribbon]: ribbon groups A4 ▾ | 模板库 | 导入 (CAD / SVG / 图片) | 图形 (gallery) |
文本 (文本 / 文本框 / 表格 ▾ / 填充文本域) | 酷玩 ▾ | 拟人 | AI绘图. Canvas: mm rulers (0–290 for A4
landscape) with red marks at the paper bounds, grid, left rail tools (select, hand pan, grid,
snap, layers), status bar X:/Y: cursor coordinates [UI]. Static: view centred on the paper,
discrete zoom levels, Esc cancels, Enter finishes a polyline, Delete deletes (B §画布) [high].
Typing into plotting text fields needs login ("用户未登录，请先登录") (B) [high]. Mode switch
绘图模式 / 激光模式 (`DrawConfig/DrawOrLaser=Draw|Laser`, tooltip "切换绘图模式或者切换激光模式") (A)
[high]; **no laser-specific options exist** (no power, speed, fill) (B) [high].
**chixie-rs today:** missing.
**Recommended implementation:** `plot_view.rs` with the same desk/page painter as `doc_view.rs`,
rulers as two thin canvases, discrete zoom steps (25–800%), select/pan tools, Space-drag to pan.
Laser mode becomes a device-profile type (§3.6), not an editor mode.

#### Paper type (画布数据 / 纸型设置)
**ChiXie3:** popup: ●标准纸张 纸型 A4(210×297) ▾, 方向 ○横向 ○纵向; ○自定义纸张 纸名 ▾ (editable, saved
presets), 宽度 180.0, 高度 150.0 (mm), [确定], 保存 / 删除 presets ("纸张名称不能为空",
"纸张名不能与标准纸张名相同", "当前页面名称尚未保存，无法删除"). Forms [ui-form]: `UI_PlottingPaperTypeSettingWidget` has group 标准纸张 (纸型：, 方向：)
and group 自定义纸张 (纸名：, 宽度：, 高度：) plus a delete button; `UI_DrawingPaperSelection` (the ribbon's paper popup) has
the same two groups with a 删除此纸张 button. Config `PaperType`, `PaperName=A4`,
`PaperOrientation=Horizontal`, `CustomPaperSize` (pickled dict) (A, B) [UI][high].
**chixie-rs today:** 布局 tab paper size / orientation / margins for writing.
**Recommended implementation:** share the paper presets list between writing and plotting
(`PaperPreset { name, w, h }` in settings TOML).

#### Side toolbar, grid and snap (侧边工具栏 / 网格 / 吸附)
**ChiXie3:** tools 鼠标框选模式 (default), 鼠标拖动画布模式, 启用画布网格 (5 mm default), 启用网格点吸附功能
(toasts 已启用网格吸附 / 已关闭网格吸附; marker at the nearest intersection), 打开图层属性设置 (B §侧边工具栏)
[high].
**chixie-rs today:** Missing (see §2).
**Recommended implementation:** same tools; snap targets: grid, endpoints, midpoints, centres
(object snap is a clear improvement); hold Alt to suspend snapping.

#### Basic shapes (基本图形)
**ChiXie3:** gallery: point, line, polyline, S-curve (cubic Bezier), polygon (hexagon icon),
rectangle, circle, 3-point arc, centre arc, ⚙ properties [UI]. Interaction: 点 one click; 直线
press-drag-release or two clicks; 矩形 two corners; 圆/椭圆 centre then radius (`ellipse_a/b`);
圆弧 centre, start, end; 多边形 centre + edge midpoint, edge count and 内切圆圆心距 editable; 折线
successive clicks, Enter finishes, Backspace removes the last point, Esc cancels; 曲线 press = anchor,
drag = handles with predicted segment preview (B §基本图形) [high]. Items share a base with handles,
hover, enlarged hit shape for thin lines.
**chixie-rs today:** Missing.
**Recommended implementation:** `enum Shape { Point, Line, Rect, Ellipse, Arc, Polyline, Polygon {
n, r_in }, Path(Vec<Seg>) }` with an affine `transform`; tool state machines per shape in the GUI;
Shift constrains (45°, square, circle). Tests: emit of a circle with tolerance 0.05 mm has max
radial error ≤ 0.05 mm.

#### Shape properties and line types (图形属性 / 线型)
**ChiXie3:** stay-on-top panel at the screen's top-right: 类型, 横坐标, 纵坐标, line endpoints, ellipse
a/b, rectangle w/h, polygon 内切圆圆心距 / 边数, 线型 (实线 default), 颜色 [UI; B]. Exact labels in `UI_PlottingGraphicsAttrDialog` [ui-form]
(title 图形属性; a stacked page per shape): 颜色：, 类型：, 横坐标：, 纵坐标：, 线型：; line: 点1X：, 点1Y：, 点2X：, 点2Y：, 长度：;
rectangle: 宽度：, 高度：; polygon: 边数：, 圆心距：; ellipse: 圆参数a：, 圆参数b：. The shape gallery form
`UI_PlottingBasicGraphicsWidget` [ui-form] names its buttons 插入点, 插入直线, 插入折线(多段线), 插入曲线, 插入多边形, 插入矩形,
插入圆形, 三点绘制圆弧, 根据圆心绘制圆弧 and 图形属性设置. CAD line types
CONTINUOUS, CENTER(X2,2), DASHED(X2,2), PHANTOM(X2,2), DASHDOT(X2,2), DOT(X2,2), DIVIDE(X2,2) with
Chinese labels (中心线, 虚线, 双点划线, 点划线, 点线, …); dashes are real pen lifts (B §线型) [med-high].
**chixie-rs today:** Missing.
**Recommended implementation:** a docked properties pane (not a floating window). Line types as
dash arrays in mm applied at flatten time (`kurbo::dash`). Colour drives multicolour pen
changes (§3.7).

#### Layers (图层)
**ChiXie3:** dialog table 图层名 / visible eye / lock; buttons add, delete, merge, up, down
(`PlottingLayers`, `LayerAttr`) (B) [high]; layer semantics when plotting [med]. Form
`UI_PlottingLayerManagementDialog` [ui-form] (title 图层管理): the button tooltips are 添加图层, 删除图层,
合并多个选中图层, 向上移动所选图层顺序, 向下移动所选图层顺序; the column headers (图层名, eye, lock) are not form strings.
**chixie-rs today:** Missing.
**Recommended implementation:** P3: layers = ordered groups with `visible`, `locked`, `plot`
flags; hidden or non-plot layers are not emitted; one layer per pen colour is a handy default.

#### DXF import (导入 CAD)
**ChiXie3:** "选择CAD文件" `*.dxf`; progress "请耐心等待... / 正在解析文件 ..."; entities LINE, MLINE,
CIRCLE, ARC, ELLIPSE, SPLINE (→ cubic), LWPOLYLINE, POLYLINE, POINT, SOLID, HATCH (lines / solid /
paths), TEXT/MTEXT/ATTDEF (→ handwriting text boxes with anchors, height, rotation), MULTILEADER,
DIMENSION, TOLERANCE, ACAD_TABLE, INSERT (block expansion with scale/rotation); per-entity
linetype, lineweight, colour, layer; `$INSUNITS` with mm fallback ("暂不支持此文件所用的单位，已使用“毫米”解析");
fit-to-paper scaling; Y flip; result is a movable group. Errors "dxf解析异常", "错误：DXF文件结构无效"
(B §导入 SVG / CAD) [high]. 设置 › 绘图与激光 › CAD导入: 拆分导入(根据源文件属性) ▾ (`CAD_Import_Mode=1`;
the form now names the pair: 作为整体导入 / 拆分导入(根据源文件属性)) [UI][ui-form]; "split by source-file
attributes" means layer / colour / linetype of the entities (inferred).
**chixie-rs today:** Missing.
**Recommended implementation:** `import_dxf.rs` using the `dxf` crate (reads R12–2018 ASCII/binary;
new dependency, reason: DXF parsing is large) covering LINE, ARC, CIRCLE, ELLIPSE, LWPOLYLINE,
POLYLINE, SPLINE (via de Boor → cubic), INSERT, POINT; TEXT/MTEXT → plain text items written with
the current handwriting font; HATCH boundary only. Units from `$INSUNITS`. Import mode: 整体 /
按图层拆分 / 按颜色拆分. Golden tests on small DXFs (exported from LibreCAD).
**Open questions / VERIFY:** the other `CAD_Import_Mode` options.

#### SVG import (导入 SVG)
**ChiXie3:** `svgelements`: viewBox, transforms applied, Move/Line/Quad/Cubic/Arc (as cubic)/Close
plus Circle, Ellipse, Rect, Polygon, Polyline, Line (some logged "todo"); stroke width and fill
colour kept; output path items in a group (B) [high]; fills are not hatched (inferred from
absence).
**chixie-rs today:** Missing.
**Recommended implementation:** `usvg` (new crate; reason: resolves CSS, transforms, units, use,
shapes → paths reliably) then convert `usvg::Path` segments to our `Seg`; optional hatch fill for
filled shapes (scan-line at pen width, angle configurable) — a clear improvement for laser/plotter
fills.

#### Image import and raster to strokes (导入图片 / 图像二值化 / 扫描线)
**ChiXie3:** ribbon 导入 › 图片 [UI] (form `UI_PlottingDataImportWidget` [ui-form]: the 导入 group holds three buttons
for CAD, SVG and image, matching `importCadBtn`, `importSvgBtn`, `importImageBtn`); utilities `ImageBinarizer` (global threshold 0–255, Otsu) and
`PlottingImageScanner.process_image(pixmap, scan_spacing, threshold)` producing horizontal scan
lines over dark runs (B §几何工具) [high]; no UI caller found statically [low].
**chixie-rs today:** Missing.
**Recommended implementation:** `raster.rs` on `image`: grayscale → Otsu or manual threshold →
modes (a) 扫描线 hatch at spacing s mm and angle θ, serpentine order; (b) 轮廓 (marching squares +
DP simplification) for line art. Dialog with a live preview and stroke count / time estimate.
Tests: a black square at 1 mm spacing gives N lines of the right length.

#### Geometry fitting (拟合)
**ChiXie3:** Bezier points sampled uniformly by arc length (`segment_length`; bisection on t),
circle/ellipse point counts from a max chord-height ratio, `include_start_twice` to close
shapes; elliptic arcs with adaptive subdivision to `max_error`; arcs emitted as `G2/G3` in the
plotting generator via `ArcFitterUtil` (A, B, C) [high].
**chixie-rs today:** Missing (see §2).
**Recommended implementation:** flatten everything with `kurbo` to a chord tolerance (default
0.02 mm); emit `G1` only (Grbl handles `G2/G3` but arcs from fitted ellipses are approximations
anyway, and pen plotters do not benefit). Optional `G2/G3` for true circles when the firmware
supports it (fewer lines).

#### Text fields on the canvas (文本 / 文本框 / 表格 / 填充文本域)
**ChiXie3:** 文本: click inserts a text field; 文本框: a box of stacked rows with nine anchor points
(`MTEXT_TOP_LEFT…`), auto width, trailing empty rows removed; 表格 ▾: the template grid-insert menu;
填充文本域: fill a field. The form `UI_PlottingTextFieldWidget` [ui-form] gives the button text: 填充文本域 =
"在选定闭合区域内填充可输入的文本域" (fill an inputtable text field into the selected closed region), next to 文本框 and
the 文本 / 表格 buttons (`insertTextFieldBtn`, `insertTextBoxBtn`, `insertTableFieldBtn`, `fillingTextFieldBtn`). Typing uses the writing typesetter (note/composition
modes), alignment, Shift-selection, clipboard; resize handles show dimension lines with inline
editable width/height (B §文本域) [high/med].
**chixie-rs today:** Missing.
**Recommended implementation:** a `TextBox { rect, anchor, EditDoc, template: Ruled|Blank }`
shape that runs the same `layout_ids` inside its rectangle — the plotting editor gets writing for
free. Double-click to edit text in place. Tables after §3.2 tables exist.
**Open questions / VERIFY:** how a text field is bound to a closed shape (the tooltip says it is filled
inside a selected closed region; the exact fit rule is unknown).

#### Generators: maze, seal, fractal, 3-D text (酷玩: 迷宫生成器 / 印章生成器 / 分形艺术生成器 / 立体字)
**ChiXie3:** 酷玩 ▾ menu: 印章生成器, 分形艺术生成器, 迷宫生成器, 立体字 [UI]; only seal and maze have
handlers (B) [med]. Flow: choose, click on the paper to place, a config dialog edits live.
- **迷宫生成器:** 矩形迷宫 / 圆形迷宫; 复杂度 简单 / 中等 / 复杂 (cell size); width/height or radius
  (mm); form `UI_PlottingMazeConfigDialog` [ui-form] labels: 迷宫生成器, 迷宫形状：, 复杂程度：, 宽度(mm)：,
  高度(mm)：, 半径(mm)：, button 刷新迷宫 (re-roll); randomized Prim's algorithm; circular uses rings × sectors; entrance/exit gaps; output
  polylines (B §迷宫生成器) [high].
- **印章生成器:** 圆形印章 / 椭圆印章 / 方形印章; centre symbol 五角星 (SVG); text (default 炽写酷玩印章)
  along the circle / ellipse / four sides; 文字角度, 字间距, font (default SimSun bold) (B §印章生成器)
  [high]. Form `UI_PlottingSealConfigDialog` [ui-form] (title 模拟印章配置): 印章类型：, 印章图案：(combo; the 五角星
  centre symbol is one pattern), 印章文字：(multi-line), 字体：, 文字间距：, 文字角度：. Note: the seal text uses a system TrueType font, i.e. outline fonts, not stroke fonts.
**chixie-rs today:** Not planned.
**Recommended implementation:** maze: own Prim/recursive-backtracker in `gen/maze.rs` seeded,
with the wall polylines merged into long runs (fewer pen lifts) — P3, S. Seal: use the
handwriting stroke font for the text (single-line strokes plot cleanly; outline fonts draw
double contours), circle/ellipse/square border, star centre — P3, M. Fractals (L-system: Koch,
Hilbert, dragon) are cheap to add in the same framework and fit a plotter well — P3, S.
立体字, 拟人 and AI绘图: not planned (no behaviour to port).

#### Plotting templates (模板库, 绘图)
**ChiXie3:** 模板库 button opens the template library (categories 绘图模板 / 激光模板) [UI]; static B
says it opens the template manager (section A) — conflict: the live ribbon button belongs to
plotting and the backstage library has drawing/laser categories, so it most likely opens the
library filtered to drawings (inferred).
**chixie-rs today:** Missing.
**Recommended implementation:** P3, after the cloud template endpoint is known.

#### Plotting G-code (绘图 G-code)
**ChiXie3:** `ChiXiePlottingPageToGcode` per item type (line, rect, round, arc with G2/G3, polyline,
polygon, SVG path, curve, maze, seal, text box, table cells, image scan); items outside the paper
skipped (`item_in_paper`); a third-party copy `PlottingPageToGcodeForOtherDevice` ("完全照搬")
(A §Plotting G-code) [high]. Only a single plotting page is handled ("暂时只考虑加载一页").
**chixie-rs today:** Missing.
**Recommended implementation:** flatten → `Page { strokes }` → existing `gcode::emit`. Add stroke
ordering (greedy nearest neighbour + 2-opt on long jobs, optionally reversing strokes) because
imported drawings are unordered; report pen-up travel saved. Clip strokes to the paper (not
just skip items).

### 3.10 Toolbox (百宝箱)

The live dialog shows ten tools, each with 查看 [UI c-treasure]; the static slice lists seven
buttons (A §Treasure box). The three extra live tools (任务编程, 固件烧录, 多设备群控) are placeholders:
their cards exist in the Designer form `ui.UI_TreasureBoxDialog` (@a049713 in the form mining; the earlier
note cited @a04a86d, a different constant) with controls `taskCodingBtn`, `firmwareBtn`, `multiDeviceBtn`
and the exact titles / subtitles of all ten cards [ui-form], but the controller
`TreasureBoxDialog` has only seven `on_*_clicked` handlers, and clicking them opens nothing, also
in an elevated session (UI, 2026-11-07).

| Tool (UI) | Subtitle (UI) | Static class |
|---|---|---|
| 拓竹写字助手 | 输出Gcode至拓竹设备 | `BambuLabAssistantDialog` |
| Gcode预览执行器 | 导入Gcode，预览/执行 | `GcodeToolDialog` (web page) |
| 插件管理 | 目前仅用于开发者使用 | `PluginEditDialog` |
| 任务编程 | 手动灵活编排任务 | — |
| 自动换纸控制 | 目前仅适配部分换纸机器 | `AutomaticPaperDeviceDialog` (§3.7) |
| 固件烧录 | 常用控制板烧录固件 | — |
| 一笔画【简易】 | 输入坐标集，一笔绘制 | `OneStrokeDrawingDialog` |
| 多设备群控 | 设备群控与任务分配 | — |
| 远程文件 | 管理远程TF卡文件 | `SDCardFileManagementDialog` |
| 字体优化 | 优化部分字体问题 | font data optimise (§3.3) |

#### Bambu Lab writing assistant (拓竹写字助手)
**ChiXie3:** dialog 拓竹助手 [UI][ui-form `UI_BambuLabAssistantDialog`; field order 打印机IP：(自动搜索), 访问码：(连接设备),
设备型号：(拓竹A1 is the form's default entry), 页面选择：(upload button, file list with refresh and delete), 抬笔位置：,
落笔位置：, X轴偏移：, Y轴偏移：, 原点模式：, 提示音：]: note "请先配置页面模板，页面尺寸不要超出设备行程", 详细使用教程; 设备型号 (拓竹A1,
拓竹A1-Mini, 拓竹P1S(输出可能有问题), 拓竹X1(输出可能有问题), 其他I3（如创想三维Ender-3）), 打印机IP + 自动搜索,
访问码 + 连接设备, 页面选择 + 上传, X轴偏移 0.0, Y轴偏移 0.0, 抬笔位置 8.0, 落笔位置 4.0 (help text recommends
抬笔7, 落笔2), 原点模式 None / 自动XY原点, 提示音 启用; file list with 刷新 / 删除 ("暂未开发"). Search scans
the /24 for port 990; implicit FTPS (TLS 1.2, user `bblp`, password = access code, no hostname
check, optional fingerprint pin); upload with `STOR`; or 导出Gcode to a folder. Only note
templates ("暂时只支持笔记模式下导出"). G-code: `G1 X.. Y.. F<speed>`, `G1 Z<up/down>`, `G4 P<pause>`,
end `G90G0 X0Y0` + `M18 X Y Z`; optional A1 tune via `M400 S1`/`M1006` lines. Config
`[BambuLabConfig]` stores the IP and the access code in plain text (A §Bambu Lab) [high].
**chixie-rs today:** missing.
**Recommended implementation:** P3. A device profile type `BambuPrinter { model, ip }` with an
`FtpsUpload` transport (`suppaftp` crate with rustls; new dependency, reason: implicit FTPS) and
a 3D-printer emitter (`G28` only if the user opts in; Z pen heights; `M18` at the end). Access
code in the OS credential store (`keyring`), never in settings or logs. Export-to-file works
without the FTPS part (do that first, S).

#### G-code previewer and executor (Gcode预览执行器)
**ChiXie3:** QtWebEngine page "Gcode预览与执行工具" from the vendor server (`client_api/gcode_page/
gcode_tool_page`) with a WebChannel bridge: drop/click `.gcode`/`.txt`, X轴方向 右(X+), Y轴方向
上(Y+), 应用坐标系设置, ☐路径简化, ☑渐进式渲染, 细节级别 slider (10), ▶执行 with progress 就绪, 重置;
canvas with stats (已解析行, 路径数, 解析时间, 内存使用, FPS), legend 移动轨迹(抬笔) blue / 绘制轨迹(落笔)
green / 当前位置 red, 适应视图, 性能面板, zoom [UI]. The Qt form `UI_GcodeToolDialog` is only a web view
(initial URL `about:blank`): every label above comes from the served page, not from Designer [ui-form]. 执行 builds a task and runs it on the 炽写 device;
refuses "设备未连接", "请先结束书写" (A §G-code tool) [high]. Online only.
**chixie-rs today:** missing (but the page preview renders strokes natively).
**Recommended implementation:** P1 (it is the natural "run an arbitrary G-code file" feature):
core `gcode_parse.rs` (G0/G1/G2/G3/G90/G91/G92/Z moves → pen-up/down polylines, extents),
GUI dialog reusing the page canvas painter (travel blue dashed, drawn green, live head red dot
from `?` status), axis direction toggles, 执行 = `Command::Run(Job { pages: vec![file] })`. Offline.
Tests: parse the protocol.md capture sample; extents match.

#### Plugin manager (插件管理 / 开发者调试)
**ChiXie3:** dialog 开发者调试: 语言 Python ▾, 代码 editor, 输出 pane, [保存] [启用插件] (form
`UI_PluginEditDialog` [ui-form]: 语言：, 代码：, 输出：, 启用插件, save); edits
`<user dir>/plugin/test.py`, imported with `importlib` and run in-process (A §Plugin system; UI)
[high/med].
**chixie-rs today:** Not planned.
**Recommended implementation:** not planned (arbitrary code execution inside the GUI, no
sandbox). See the external filter design in §3.8.

#### Task programming, firmware flashing, multi-device control (任务编程 / 固件烧录 / 多设备群控)
**ChiXie3:** placeholder cards; subtitles 手动灵活编排任务, 常用控制板烧录固件, 设备群控与任务分配 [UI].
Not implemented in this build: no click handler in `TreasureBoxDialog` (A @949f622), and nothing
opens even when run as administrator [UI] [high].
**chixie-rs today:** Missing (ChiXie3 has no behaviour to match).
**Recommended implementation:** design our own when demanded:
- 任务编程: a job list editor (sequence of pages / files / macros / waits / pen changes) saved as
  TOML; it generalises sheet change and pen change. P3, M.
- 固件烧录: shell out to `esptool` for ESP32 boards with bundled known-good images only; P3, risky.
- 多设备群控: N device actors (the actor design already supports several), one job queue, page
  assignment per idle device; status grid. P3, M.
**Open questions / VERIFY:** open these three in an elevated ChiXie3 session to record behaviour.

#### One-stroke drawing (一笔画【简易】)
**ChiXie3:** dialog 一笔画V1: "请逐行输入坐标: X,Y 【针对炽写设备，如需换行可输入如下指令】 pen_up, pen_down";
循环次数 1; [保存] (local `one_stroke_test.txt`) [执行] (form `UI_OneStrokeDrawingDialog` [ui-form]: title 一笔画V1,
the instruction text quoted above, 循环次数：spin box, run and save buttons); validation "请输入坐标点内容", "坐标点输入异常",
"设备未连接", "请先结束书写" [UI; A] [high]. 炽写 variant pen `G1G90 Z0 F7000` / `G1 Z-1.5 F7000`.
**chixie-rs today:** Missing.
**Recommended implementation:** P3, S: a text area parsed into strokes (`pen_up` / `pen_down`
keywords), repeat count, preview, run. Same G-code path as everything else.

#### Remote TF card files (远程文件 / 远程TF卡文件管理)
**ChiXie3:** 炽写 ESP32 firmware only ("当前固件不支持查看远程SD卡文件"); board IP via serial or the
network list; SD detect `[ESP200]` ("未检测到SD卡接入"); list via `[ESP210]` or `/upload`; folders
with "返回..."; delete file/dir ("确认删除所选文件(夹)吗？"), new folder ("文件夹创建成功"), upload local
files (filter `*.g *.gco *.gcode *.nc *.ngc *.ncg *.tap *.txt`), run `[ESP220]<path>`, pause `!`,
resume `~`, status `?`, unlock `$X`, stop 0x18; status timer "执行中(" / "暂停中(" / "任务执行完毕";
default folder `/chixie_task` auto-created; a websocket is also used (A §Remote TF) [high]. Form
`UI_SDCardFileManagementDialog` [ui-form] (title 远程SD卡): path label, file list, buttons for new folder / refresh /
upload / delete / stop, 开始执行 (run), 容量： with "x GB" placeholders and used / total labels plus a status label
(so the free-space display is used / total capacity).
**chixie-rs today:** `esp3d.rs` (list, delete, delete_dir, create_dir, upload, run, hold,
resume, unlock, soft_reset) — untested.
**Recommended implementation:** GUI file browser dialog over `esp3d::Board`, progress for uploads,
run/pause/stop buttons with `?` polling. P2. Test against a mock server; then hardware once the
board is in STA mode.
**Open questions / VERIFY:** websocket purpose (ESP3D uses it for live messages; probably status).

### 3.11 AI features (炽写AI / 小炽)

#### AI writing assistant
**ChiXie3:** top-right 炽写AI (login required: "请登录后使用！"). A floating input at the caret
"选择下方功能，或在此输入需求" with a menu [UI ck-k1] (form `UI_ChiXieAIWidget` [ui-form]: states AI编写中……, 所选功能,
buttons 发送给AI and 停止生成, feedback tooltips 回答的不错，点个赞 / 回答的不行，反馈上报 — so there is a thumbs up /
thumbs down feedback report, a flow this section did not list; the menu tree itself is code-built): 用AI起草 › 大纲, 教学材料 ▸ (教学大纲, 教学计划, 课堂讲稿,
练习题), 邮件信函 ▸ (邀请函, 感谢信, 道歉信, 辞职信, 保证书), 工作总结, 头脑风暴, 学习笔记, 课设报告 ▸ (全文 or
项目目标 / 需求分析 / 系统设计 / 实施过程 / 结果分析 / 总结), 实验报告 ▸ (全文 or 实验目的 / 原理 / 过程 / 数据记录 /
结果 / 分析), 实习报告. With a selection: 编辑/评审选区 › 内容润色, 内容简写, 内容扩写, 风格修改 (专业化 /
通俗化 / 直接化 / 间接化 / 亲切化); 从选区生成 › 内容续写, 提炼总结, 内容翻译 (中文 / 英文 / 日文), 内容释义,
行动列表 (A §AI) [high]. States: generating, stop, 重写, close; streaming result inserted through
the typesetter. Identity questions answered locally ("我是炽写研发的AI小助手，我的名字叫小炽…"); brand
words are rewritten in output.
**How it works there:** `CallXunFeiAI` websocket to iFlytek Spark v1.1
(`wss://spark-api.xf-yun.com/v1.1/chat`), HMAC-SHA256 signed URL; vendor API keys fetched from
`ai/get_ai_account` at runtime (A) [high]; the keys arrive in the client (server-api.md [cap]).
**chixie-rs today:** missing.
**Recommended implementation:** P3. Provider-neutral `ai.rs` trait with an OpenAI-compatible
chat-completions client (most Chinese LLM vendors offer that API) configured by the user's own
endpoint + key in the credential store; prompts as a TOML table (the same menu, editable).
Streaming via `reqwest` blocking + SSE line reader on a worker thread. Inserted text gets fresh
char ids. Do not reuse the vendor's Spark keys.

### 3.12 Account, VIP and limits (账户 / 会员 / 限制)

#### Login and auto login (登录 / 自动登录)
**ChiXie3:** login dialog (form `UI_LoginDialog` [ui-form]: placeholders 手机号或者邮箱 and 请输入密码, checkboxes
记住密码 and 自动登录, link 忘记密码; the 注册 link is not a form string): account, password, 记住密码, 自动登录, links 注册 / 忘记密码 (web pages
`page/register`, `page/forget-password`); "信息填写不完整", "登录中...", "登录失败，请检查网络连接". Saves
account **and password in plaintext** to `[LoginConfig]` (C §登录) [high]. Auto login at start
("自动登录成功" / "自动登陆失败，请检查网络连接"). After login: sync collected fonts, pre-parse them with
progress, show/hide 录制 and 今日试用 regions by role (A §Initialisation) [high]. Request/response
verified (server-api.md [cap]).
**chixie-rs today:** Done and optional: `login.rs`, `session.rs`, `credentials.rs` (OS credential
store), app opens straight to the editor; login only to download fonts.
**Recommended implementation:** keep. Add 注册 / 忘记密码 links (open the website in the browser).

#### Roles and gating (用户角色 / 权限)
**ChiXie3:** roles `ordinary_user`, `vip_user`, `super_vip_user`, `super_user` (title 超级管理员),
`writing_robot_seller`; login data has `permissions: [draw, laser, write, mobile_app]` [cap].
Gates seen: not logged in → cannot type, import or use file-menu actions ("请登录后再输入内容！",
"请登录后使用！"); ordinary → only the daily free font, cannot collect or set main fonts; admin →
设备测试, 同步至服务器 in 字符微调, G-code export button; macOS → third-party devices limited from 2026,
super VIP exempt (`UserLimitUtil.limit_for_macos`, docstring) (A, C) [high/med]. Live account
type shown: 通用会员 [UI].
**chixie-rs today:** no gating (offline-first decision, 2026-11-07).
**Recommended implementation:** do not replicate client-side gating; the server already decides
which fonts a user may download. Show the role and expiry on the account page only.

#### Online detection / heartbeat (在线检测 / 心跳)
**ChiXie3:** `AntiCrackingUtil.user_online_detection` before start/resume writing and file actions;
`user/heartbeat`; after `OnlineDetectionFailedMaxCount` failures: "在线状态异常，请检查网络后重新登录"
and **clears fonts and login** (`clear_font_and_login_info`); next check at a random interval; a
`debug_check` stub suggests anti-debugging (C §AntiCrackingUtil) [high].
**chixie-rs today:** heartbeat every 60 s in `session.rs` (`SessionLost` event); writing never
depends on it.
**Recommended implementation:** keep: a lost session only disables cloud actions; local fonts and
jobs keep working (never interrupt a running job for a network blip).

#### User info, upgrade, license, invitation (用户信息 / 升级 / 注册码 / 邀请码)
**ChiXie3:** dialog 用户信息 [UI]: 欢迎你 <name>, 用户类型, 查看权益, 手机号, 邮箱, 邀请码, 注册时间, 升级日期,
升级时长, 有效期至, [升级], [退出账号]. Form `UI_UserInfoDialog` [ui-form] (title 用户信息) adds: 欢迎你, 用户名, 邮箱：, 手机号：,
注册时间：, 用户类型：, 邀请码：, 升级日期：, 升级时长：, 有效期至：, 查看权益, 退出账号; a package page with the banner
"当前套餐为限时折扣价喔！！", table columns 套餐名 and 可用时长, purchase mode radios 直接支付 / 我是炽写设备 / 我有注册码, 购买数量,
绑邀请码打折, 请输入注册码 + 使用注册码, and for device buyers 订单号： with "输入购买时的订单号，快递签收则自动升级炽写超级会员"
(a 炽写设备 purchase upgrades to super VIP automatically after courier sign-off). 退出登录 confirm "您确定要退出登录吗？". 使用注册码 → `pay/bind_license`
(may offer to load a device-bound config and restart: "该注册码已绑定专属设备：“…”，是否加载该设备的配置文件并重启？").
邀请码: `invitation/bind_invitation_code`, `query_binding_of_invitation_code`,
`get_invitation_code`; discounts. 升级: `pay/get_packages` (name, price, duration: one_week /
one_month / one_year / forever), quantity, confirm "下单确认", `pay/upgrade_payment` → opens Alipay in
the browser, "支付询问 … 我已支付" → `pay/check_upgrade` (C §用户信息) [high].
**chixie-rs today:** account page in the backstage (name, logout).
**Recommended implementation:** show the fields (no phone number in logs). Payments: link to the
vendor's website only (not planned in-app). License-bound config: not planned.

### 3.13 Cloud services (云端)

#### Parameter sync (参数同步 / `config/font_config`)
**ChiXie3:** 设置 › 常规设置 › 参数同步 (开启) [UI] = `Synchronization_Parameters=0`; `FUtil.sync_user_font_config`
GET/POST `config/font_config` round-trips main font ids and deformation settings (C) [high].
**chixie-rs today:** Missing.
**Recommended implementation:** P3; optional "从云端导入设置" one-shot button (read only), to avoid
two-way sync conflicts.

#### Exclusive config (专属配置)
**ChiXie3:** `config/query_exclusive_config_file`, `config/query_config_download_url`,
`config/download_exclusive_config_file`: a license-bound `config.ini` replaces the user's; also a
TODO to fetch platform defaults when the local config fails (A, C) [high].
**chixie-rs today:** Not planned.
**Recommended implementation:** not planned. Our `config::import_chixie3` covers migration.

#### Auto upgrade (自动升级)
**ChiXie3:** `update/check_update` (`major/minor/fixed/alphabet_version`, `soft_type`) and
`update/compare_file_differences` (MD5 per file); chunked download with speed/time remaining;
restart via `taskkill /f /im` and relaunch; changelog saved to `update_log.txt` and shown once in
更新日志 (A §Auto upgrade) [high]. Error "更新出错，建议官网下载并安装最新版".
**chixie-rs today:** Missing (replace).
**Recommended implementation:** our own channel (GitHub releases or the team's server):
`self_update`-style check of a signed manifest (version, SHA-256, URL), download to a temp file,
verify signature (`minisign`/ed25519 via `ed25519-dalek`), swap on next start. Never patch files
of a running exe in place. P2, M.

#### Missing-character and logout reports (缺字上报 / 退出信息)
**ChiXie3:** on close: `user/collect_logout_info`, `user/collect_missing_characters`, then logout
(A §Tray, heartbeat) [high].
**chixie-rs today:** Not planned.
**Recommended implementation:** not planned (telemetry); logout on exit only if the user chose
"退出时注销".

#### Feedback, tutorial, support (反馈 / 教程与售后 / 在线客服)
**ChiXie3:** 反馈 opens a Tencent Docs sheet; 教程与售后 opens the user-guide page in a WebEngine
dialog (炽写用户指南与售后); 在线客服 action; shop links (Taobao) on 设备购买 and the add-device dialog; a
promo tip ("本周店铺所有配件8折！", "前往购买") likely server-driven (A, UI) [high].
**Recommended implementation:** links that open the system browser; no ads.

### 3.14 Settings (设置)

**ChiXie3:** singleton dialog 设置 with seven pages [UI m-settings, c-12…c-17]; saved on close
(`saveData` on `QEvent.Close`); "?" opens the website (B §C) [high].

| Page | Controls (UI, defaults) | Config keys |
|---|---|---|
| 常规设置 | 默认功能 写字功能; 参数同步 开启; 语言 简体中文; 设备管理 默认收起 | `[GeneralConfig] Default_Function=0, Synchronization_Parameters=0, Language=0, DeviceManagerStatus=0` |
| 写字偏好 | 字符间隔 中 0.6–1.1 / 英 0.5–1.0; 空格宽度 中 5.0–7.0 / 英 2.0–3.0; 首行缩进 15.5–17.0; 垂直偏移 0.0–0.3, ☐启用智能偏移(同向偏移); 路径规划 ○之字型(默认) ○弓字型; ☑首字不对齐 幅度 2.0 | `[WriteConfig]` (Appendix A) |
| 多色绘制 | ☐多色绘制; 换笔模式 手动(弹窗提示) / 自动(需设备支持); 自动换笔配置 共1支笔 [设置] | `[MulticolorDrawing] EnableMulticolorDrawing, PenChangingMode` |
| 绘图与激光 | CAD导入 拆分导入(根据源文件属性) ▾ | `[DrawConfig] CAD_Import_Mode=1` |
| 字符变形 | ☑基础变形 (胖瘦 96–104%, 高矮 98–102%, 旋转 −2–2°, 笔画变形幅度 1, ☑随机删除小尾巴); ☑字形变形 (幅度 7); [使用推荐参数] | `[CharacterDeformation]` |
| 主题与样式 | 界面风格 Office风格 ▾; 笔记线条 实线 ▾ | not in the sample ini (defaults) |
| 页面旋转 | 页面旋转模式 不旋转 ▾ | `[ParityPageConfig] ParityPageDirectionMode=0` |

Language files: `appLang_CN.qm` / `appLang_EN.qm` (C §其他工具) while `core.ChiXie` lists six
languages (zh-Hans, zh-Hant, en, ja, ko, ru) (A) — only two are shipped [med]. The settings form
`UI_SettingDialog` [ui-form] lists exactly four language entries: 简体中文, 繁体中文, 日本語, русский язык
(no English, no Korean), so the combo and the shipped `.qm` files disagree (see §6.5).

**Form details** (`UI_SettingDialog`, [ui-form]): the left list has the seven pages in the order 常规设置, 写字偏好,
多色绘制, 绘图与激光, 字符变形, 主题与样式, 页面旋转. 常规设置: 参数同步：, 语言：, 默认功能：(写字功能 / 绘图与激光功能),
设备管理：(默认收起 / 默认左侧展开 / 默认右侧展开). 写字偏好 group headings 字符间隔（毫米）, 空格宽度（毫米）, 首行缩进,
垂直偏移（毫米）, 启用智能偏移(同向偏移), 路径规划 (之字型（默认）, 弓字型), 首字不对齐(适用于笔记、表格) with 不对齐幅度：.
多色绘制: 换笔模式 (手动换笔（弹窗提示）, 自动换笔（需设备支持）), 自动换笔配置 with 共1支笔. 绘图与激光: CAD导入： 作为整体导入 /
拆分导入(根据源文件属性) (so the second option of `CAD_Import_Mode` is "import as one whole"). 字符变形: 基础变形
(字符随机高矮：, 字符随机胖瘦：, 字符随机旋转：, 笔画变形幅度：, 随机删除小尾巴), 字形变形 (字形变形幅度：,
"（滑动条为0则该项无变形）"), 使用推荐参数. 主题与样式: 界面风格：, 笔记线条：. 页面旋转: 页面旋转模式： with the four entries
不旋转, 奇数页旋转180°, 偶数页旋转180°, 旋转180° and the notes "（默认不旋转，默认起点为左上角）" and
"页面预览不旋转，只旋转书写模式输出". Each page is a plain group of line edits, spin boxes, sliders, check boxes and
radio buttons; there is no separate 保存 button (values are saved on close).

**chixie-rs today:** backstage 选项 (dark mode, template grid, imports), 手写效果 ↘
`style_dialog.rs` (ranges), `config.rs` TOML settings, `import_chixie3` for `[WriteConfig]` and
`[CharacterDeformation]`.
**Recommended implementation:** one 选项 page in the backstage with the same seven groups (Word-
style left list), all edits applied live (no "save on close"). Settings that belong to the
document (template, origin, page rotation) live in the document, not in global settings, so a
saved `.chx` reproduces the output exactly (better than ChiXie3, whose output depends on global
state). Defaults = ChiXie3's shipped values (already in `HandwritingStyle::default`).

### 3.15 Files: documents, import, export, print (文件)

#### File menu (文件 ≡)
**ChiXie3:** live menu: 模板管理 | 新建 ▸ | 打开 | 保存 | 另存为 | 批量导入 | 打印 ▸ | 选项 | 退出 [UI]. Static
items: 模板管理, 从当前模板新建, 追加当前模板页, 从炽写模板库新建, 另存为, 批量导入, 打印文字, 打印背景图 (C) [high];
login and font-loading guards; online detection before new/append.
**chixie-rs today:** backstage 新建 (blank / template), 打开 (.chx, .txt), 保存, 另存为, 导出, 模板管理,
账户, 选项; draft autosave.
**Recommended implementation:** add 追加当前模板页 (append a page) and 批量导入.

#### ChiXie document (`.cdoc`) vs chixie-rs document (`.chx`)
**ChiXie3:** 保存为炽写文档 / 打开炽写文档, filter "炽写文档 (*.cdoc)": items (text, space, line break)
with font ids and formats plus the template; loads with progress (准备加载炽写文档 … 加载完成); if the
template is not local it offers to save it ("当前炽写文档对应的模板未在本地保存，是否保存该模板？"). Encrypted
with Fernet (hard-coded key); serialisation probably pickle (A §Save/open) [high/low]. The
handwriting itself (random draws, deformation) is **not** guaranteed reproducible on reopen
(inferred: randomness is drawn at layout time and not stored).
**chixie-rs today:** `.chx` JSON: text + per-character ids + handwriting seed, so a reopened
document keeps exactly the same handwriting (`file_ops.rs`).
**Recommended implementation:** extend `.chx` (versioned: `"version": 2`) with attribute runs,
paragraph attributes, template (embedded copy), origin, page rotation, char tweaks, font roles
(ids + names, so a missing font can be reported by name). Keep JSON (diffable, small). Import
`.cdoc` only if a user has many (needs the client key at runtime from their own install; never
commit it).

#### Import text documents (导入 txt / docx / pdf)
**ChiXie3:** filters by template: 笔记 `*.txt *.docx *.pdf *.cdoc`; 表格 `*.docx *.xls *.xlsx *.cdoc`;
作文格 `*.txt *.docx *.pdf *.cdoc`. If the page has content: "检测到页面已有内容，请问是覆盖还是追加？"
(`DirectImport`, `AppendImport`, `OverwriteImport`); 作文格 append unsupported. TXT: chardet
encoding, one paragraph per line. DOCX (python-docx): runs with italic, underline, colour, sub/
superscript, paragraph alignment (centre/right; no justify); errors for Strict OOXML and broken
images. PDF (PyMuPDF): plain text lines. `.doc` / `.xls` refused with conversion hints (C
§导入文件) [high]. Notes import is a fast path placing items directly into rows.
**chixie-rs today:** `.txt` import (`file_ops.rs`).
**Recommended implementation:** TXT: detect UTF-8/UTF-16 BOM, else try GB18030 (`encoding_rs`,
new dependency, small and standard). DOCX: unzip (`zip` crate) + read `word/document.xml` with
`quick-xml` (both new; reason: a full docx crate is unnecessary for runs/paragraphs) mapping
`w:i`, `w:u`, `w:strike`, `w:color`, `w:vertAlign`, `w:jc`. PDF: P3 (text extraction quality is
poor; ask users to copy-paste). Overwrite/append/insert-at-caret choice in one dialog.

#### Import spreadsheets into tables (导入 xlsx)
**ChiXie3:** openpyxl; sheets ↔ template pages ("多页模板页数和所导入的表格sheet页数不一致"); merged cells
expanded; cells with `accept_import=False` or a custom data source skipped; visual row/column
mapping; number formats `General`, `0`, `0.0`, `0.00`, `0.000`, `0_ `, `m"月"d"日";@` (→ M月D日),
datetime suffix stripped; formulas not supported (C §XLSX) [high].
**chixie-rs today:** Missing.
**Recommended implementation:** after tables: `calamine` (new crate; reason: reads xlsx/xls/ods,
pure Rust, gives cached formula results — supports `.xls` too, better than ChiXie3) mapped through
the page's `excel_range`. Tests on a fixture workbook with merges and formats.

#### Batch import (批量导入 / 批量建页)
**ChiXie3:** dialog with a 名单 text box: for a single-page table template, creates one page per
line and writes the line into the selected target cell; other cells keep data-source values;
"请先选中目标单元格", "暂时只支持单页模板批量建页" (C §批量导入, 2025-11-02) [high]. Form `UI_BatchImportForWritingDialog`
[ui-form] (title 批量导入) has a segmented switch with two pages, so the "second tab" is known: 批量导入数据
(card text "批量导入数据文件，或导入多页表格实现批量数据导入") and 批量名单建页 (card text "根据名单数据，自动新建对应数量表格页面并填充",
button 批量建页, label 目标单元格 showing 未选中 until a cell is chosen, text box 名单数据：). The first page has only the
hint card on the form, so it is probably a pointer to normal data-file import (inferred).
**chixie-rs today:** Missing.
**Recommended implementation:** generalise to mail merge: a CSV/TSV with a header row; each column
binds to a cell (tables) or to a `{{name}}` placeholder in note text; one page (or one document
copy) per row. Works for notes too (e.g. 30 personalised letters). P2, S after tables / with
placeholders.

#### Print (打印文字 / 打印背景图)
**ChiXie3:** `QPrinter` with print dialog and preview, page by page; 打印 ▸ submenu with 打印文字 and
打印背景图 (C §打印) [med]; meaning inferred (text only vs template background only).
**chixie-rs today:** Missing.
**Recommended implementation:** P3: export a PDF of the preview (vector strokes) with `printpdf`
(new crate) or SVG per page (no dependency) and let the OS print it. "打印背景图" = print the template
ruling on blank paper (useful: make your own 作文纸), "打印文字" = preview of the handwriting.

#### Template and device files
See §3.2 (`.cx`, table templates) and §3.6 (`.cset`). Appendix C lists all formats.

### 3.16 Main shell and UX (主窗口)

#### Ribbon, quick access toolbar, title bar (功能区 / 快速访问工具栏)
**ChiXie3:** SARibbon window titled "炽写(alpha.3.0.310)"; QAT: logo, open, save, export, settings, help
[UI]; tabs 主页 (blue, backstage) | 书写 | 绘图与激光; top right 炽写AI, 教程与售后, crown + user name. 书写
ribbon groups [UI shot 01]: 模板管理 / 文件 ≡ | template picker (作文格1 ▾) | 剪切 复制 格式刷 | 字体库 | font
combo, size combo (100), A+ A− | I U 删除线 颜色 ▾ X₂ X² H 清除 | list/indent/align (2 rows of 4) | 插入 |
查找 | 字符微调 | 合并内容 | 书写 ▶ ■ + job box | 录制字体 | 百宝箱. `UI_MainWindow` [ui-form] confirms these names
(tooltips 剪切（Ctrl+V）[sic: the form says V for cut], 复制（Ctrl+C）, 增大字号(5%), 减小字号(5%), 设为标题字号(放大15%) — i.e.
标题 = 115% of the size —, 清除格式, 设置颜色，用于换笔书写, 查找与替换, 打开炽写文档, 保存为炽写文档, 导出Gcode, 使用帮助), contextual ribbon groups
表格功能, 批量溢出, 作文格功能, 合并内容, and a formula contextual category 公式 (创建上下文标签页 - 公式 - 公式模板) with
formula template groups: 代数代数, 不等式, 常用符号, 希腊字母, 根式角标, 极限对数, 三角函数,
积分运算, 大型运算, 括号取整, 数组矩阵 (a formula template ribbon that has no handler, cf. the 插入 › 公式 placeholders), 左倾斜 /
右倾斜 (tooltips; two tilt buttons rather than one 倾斜), 下划线, 删除线、墨团等 (strike-through *and ink blobs*, a
variant not mentioned before), 设置上标 / 设置下标, 首行缩进, 垂直偏移, 增加/减少缩进量, 左对齐, 右对齐, 两端对齐 (no 居中 string in
the form), a 单页视图 / 双页视图，适用于写本子预览 pair, status 第 0 页，共 0 页 and "X:123 Y:245" with the tooltip 鼠标当前位置,
新建模板 / 新建文档 / 新建表格 / 新建作文, 炽写AI, 插件平台 / 插件广场 (plugin platform ribbon text, no handler known).
Contextual categories: table and
composition panels appear by selection; the template manager category (A §Main window) [high].
**chixie-rs today:** Word 365-style chrome: custom blue title bar with QAT, centred "文档名 - 赤写",
登录/account, tab strip (文件 backstage, 开始 / 布局 / 手写 / 设备 / 视图), ribbon card with large/small
buttons and ↘ launchers, status bar (`ribbon.rs`, `app.rs`, `app/ribbon_tabs.rs`).
**Recommended implementation:** keep the Word mapping (users know Word better than ChiXie3's
hybrid). Add a 插入 tab later (表格, 文本框) and a 工具 tab hosting the toolbox tools instead of a modal
百宝箱.

#### Status bar (状态栏)
**ChiXie3:** 设备未连接 button (opens the device pane) | "第 N 页，共 M 页" | word count | two view-mode
toggles | zoom spinner 100% | font name (A, UI) [high].
**chixie-rs today:** Done: hover cells (字体 opens 字体库, 设备 opens the pane), job progress bar,
two-sided zoom slider.

#### Backstage home page (主页)
**ChiXie3:** left rail ← back, 字体, 模板, 教程 | bottom 账户, 设置, 反馈 [UI] (form `UI_HomePage` has only a rail of
tool buttons and a stacked widget [ui-form]; the page titles come from the child forms 主页-字体 / 主页-模板); back saves each widget's
config ("返回时候通知各个组件保存配置"); 账户 requires login (A §Home page) [high].
**chixie-rs today:** `backstage.rs`: ← 返回, 新建, 打开, 保存, 另存为, 导出, 模板管理, 账户, 选项; 字体库 is a
dialog from the ribbon.
**Recommended implementation:** add 字体库 as a backstage page too (it is a full-window browser
in ChiXie3) and a 帮助 page with links.

#### Shortcuts (快捷键)
**ChiXie3:** Qt standard keys for copy/cut/paste/select-all/undo (undo empty); arrows,
Shift+arrows, Tab (indent), Enter, Backspace/Delete (+ Enter confirms cell clearing), Esc (cancel
drawing), Ctrl+wheel zoom; others bound in `ChiXieInit.__init_shortcut_signal` (keys not
recoverable) (A, C) [high/med].
**chixie-rs today:** Word set in `doc_view.rs::bind_keys` (secondary = Ctrl/⌘): Save, word/line/
document moves, select-all, clipboard, undo/redo.
**Recommended implementation:** add F5 = 开始书写, Shift+F5 = 从光标处书写 (resume by character), Esc =
暂停 while a job runs (with confirm to stop), Ctrl+F = 查找, Ctrl+H = 替换, Ctrl+P = 导出 PDF.

#### Tray, close, window geometry, resolution change (托盘 / 关闭 / 窗口记忆)
**ChiXie3:** tray icon with tooltip "炽写：版本号：…" / "暂未登录" and a widget button 退出炽写 (form `UI_SystemTray`
[ui-form]: a pop-up card with the logo, the user name and type line (普通用户), a 交流反馈 button and 退出炽写); close-to-tray
drafted but unfinished; close confirm "确认要退出炽写吗？"; remembers `Window_Size`,
`Window_IsMaximized`; detects a resolution change and offers a restart (A, C) [high].
**chixie-rs today:** M6 planned (window geometry, icon).
**Recommended implementation:** save geometry on exit; confirm close **only** while a job runs or
the document has unsaved changes (autosave makes the latter rare). No tray (P3). No restart on DPI
change (gpui handles scale changes).

#### Notifications and errors (提示 / 异常提示)
**ChiXie3:** InfoBar success/warning/error tips, file-saved tip with 打开目录, tray balloons; a
global exception hook shows "异常提示" and logs file/function/line; rotating log `ChiXie-Log.log`
(A, C) [high]; progress dialog "后台任务(请勿操作)".
**chixie-rs today:** gpui-component `Notification` toasts; `env_logger`.
**Recommended implementation:** add a rotating file log in the config dir (`log` + a tiny file
writer, or `flexi_logger`), a panic hook that writes the backtrace there and shows a dialog with
"打开日志文件夹". Redact tokens/phone numbers in log lines (helper `redact()`).

#### Update log, guides, daily dialogs (更新日志 / 用户指南)
**ChiXie3:** 更新日志 dialog after an upgrade (reads `update_log.txt`, opens `page/update-log`); 用户指南与售后
web dialog; daily free font dialog (A) [high]. Form `UI_UpdateLogDialog` [ui-form]: title 更新日志, 版本号： with the
version, 更新内容：, and links 查看所有日志 (command link) plus one close button.
**chixie-rs today:** Missing.
**Recommended implementation:** show release notes once after an update (bundled markdown).

#### Internationalisation (语言)
**ChiXie3:** 简体中文 / English via `.qm` files; enum lists six languages (A, C); the settings form offers
简体中文, 繁体中文, 日本語, русский язык (four, no English) [ui-form] (§6.5 conflict 11).
**chixie-rs today:** Chinese strings in code; M6 planned.
**Recommended implementation:** `rust-i18n` or `fluent` (new crate; pick one in M6) with zh-CN as
the source language and en as the first translation.

## 4. Where chixie-rs can be clearly better

1. **Streaming robustness.** ChiXie3's Telnet path stalls on long jobs (~44, ~752, ~1682 lines)
   and only disconnect + restart recovers (protocol.md [cap]); CH340 boards drop under motor load
   and come back on a new COM number (memory `chixie-board-usb-drop`). chixie-rs: one device actor,
   Grbl character counting with `?` polling, stall detection (no ack for N s while the board
   reports Idle/Run → resync from `Bf:`), auto re-scan by VID/PID, and **line-exact resume**
   (re-emit from the start of the interrupted stroke). Pause uses feed hold `!` so the head stops
   at once; stop = `!` → `0x18` → `$X` → pen up (already in `device.rs`).
2. **Determinism.** ChiXie3 draws randomness at layout time and does not store it; reopening a
   `.cdoc` or re-pasting text changes the handwriting (protocol.md saw a paste-only scale anomaly).
   chixie-rs keys every random draw by (document seed, effect salt, character id): byte-identical
   output for the same document, edits never reshuffle other characters, 换一组 re-rolls on purpose.
   Golden tests pin it.
3. **No code execution from data.** ChiXie3 loads `.cset` and `.cx` with Python `pickle`
   (executes code by design), runs user Python plugins in-process, downloads and runs a driver
   EXE. chixie-rs: a reader for the pickle opcodes only (never resolves classes), TOML/JSON for
   its own files, external-process filters instead of plugins, no driver installer.
4. **Secrets.** ChiXie3 stores the password and the Bambu access code in plaintext INI, puts the
   login token in a WebEngine URL, ships hard-coded Fernet keys for templates/documents/fonts and
   receives the AI vendor's keys from the server. chixie-rs: password and access codes only in the
   OS credential store, nothing secret in URLs or logs, no vendor AI keys, decrypted fonts kept
   only on the user's own disk (BOSS decision).
5. **Offline-first.** ChiXie3 requires login to type, import, or use the file menu, revokes fonts
   when heartbeats fail, and loads tools from web pages. chixie-rs works fully offline after fonts
   are downloaded; the cloud is used only for fonts (and later templates).
6. **Typesetting quality.** ChiXie3 has no automatic 避头尾 — only a warning pass — and no justify
   in docx import; overflow in table cells blocks editing. chixie-rs: real kinsoku (hanging
   punctuation in 作文格), overflow as a layout policy (shrink / add rows / clip) instead of a modal
   state, real undo, find/replace with options, Word-standard navigation everywhere (ChiXie3 has
   many "尚未开发" paths).
7. **Native performance.** ChiXie3 uses one QGraphicsItem per character (needs a "large text mode")
   and a slow Python TPS warp (its own TODO asks for numpy/JIT/C). chixie-rs relayouts from a flat
   model and paints only visible pages; a TPS solve per glyph is microseconds in Rust.
8. **Documents that reproduce output.** Template, origin, page rotation and per-character tweaks
   belong to the document, not to global settings, so a `.chx` file plus a font id is a complete
   job description.
9. **Cross-platform without vendor locks.** ChiXie3 hides third-party devices on macOS and warns
   on "uncertified" boards. chixie-rs: gpui runs on Windows, macOS and Linux; any Grbl board works;
   Bluetooth SPP shows as a serial port everywhere.
10. **Plotting correctness.** Exact primitives flattened at emit time with a chord tolerance, stroke
    ordering to cut pen-up travel, clipping to the paper, hatch fills for SVG/images — none exist in
    ChiXie3.

## 5. Roadmap

Each milestone ends with `cargo test`, clippy clean, AUTOTEST green, and the listed acceptance
checks. Hardware checks are run by BOSS.

| # | Milestone | Scope | Acceptance |
|---|---|---|---|
| R1 | Reliable job loop | auto connect; Alarm toast + 解锁; Grbl error texts; per-page list + "write from page N"; patrol (巡边); write-done position; stall detection | fake link: drop an `ok` → resync, no hang; unplug mid-job → reconnect by auto-scan; patrol traces the written bounds; 20-page job on hardware without intervention except sheet changes |
| R2 | Resume | line-exact resume after stop/disconnect; resume from a clicked character; "re-zero then resume" | fake link killed at line k: resumed stream = pen up + travel + identical tail; hardware: pull USB mid-page, resume, no duplicated or missing stroke |
| R3 | Paragraph and character attributes | `CharAttrs` runs, alignment, first-line indent (Tab/button), font/size per selection, colour, half space, find/replace, context menu | goldens for right-aligned and indented paragraphs; replace keeps untouched ids; `.chx` v2 round-trip |
| R4 | Font set | font roles (主线/英文/符号/数字), fallback lists, weighted mixing per char id, missing-glyph badge, collect/uncollect, tag filters/sort, 我的录制 | 1,000-char mix ratio within ±3%; missing char falls through; collect request captured and matched |
| R5 | Output options | path planning 之/弓, origin modes (纸张/框线/自定义 marker), page rotation, route simplify, multicolour with pen-change pauses | captures from ChiXie3 for 弓 and rotation match ours (glyph order, mirrored coordinates); two-colour page pauses once |
| R6 | Kinsoku and grid polish | 避头尾 with hanging punctuation, two-digit cells, full/half-width fallback, rationality badge | no line starts with a closing mark in a 1,000-char fuzz; 作文格 capture still matches |
| R7 | Tables | `table.rs` model, template editor (insert table, merge/split, cell attrs, rows, padding), cell layout with overflow policy, data sources, xlsx import (`calamine`), batch import / mail merge | capture of a 2×2 form matches within 0.3 mm; overflow shrink fits; xlsx with merges maps correctly |
| R8 | Effects fidelity | TPS shape deformation, tail removal, smart vertical offset, 字符微调 panel, glyph clean-ups (`glyph_fix.rs`) | deformation captures (levels 3/7/10) reproduce magnitudes; < 50 µs per glyph |
| R9 | G-code tools | G-code previewer/executor, SD card browser + upload-and-run, one-stroke tool, test pattern job | parse + preview of the protocol.md capture; SD upload against a mock server; hardware run from SD |
| R10 | Plotting editor | `PlotDoc`, shapes, properties, grid/snap, SVG (`usvg`), DXF (`dxf`), image hatch, text boxes, stroke ordering | goldens for circle tolerance, DXF fixture, SVG fixture; travel reduction ≥ 30% on an imported drawing |
| R11 | Platform | i18n (en), window geometry, icon, release build, self-update with signed manifest, rotating logs + panic dialog | clean install on a fresh Windows VM; update from N to N+1 verified by signature |
| R12 | Optional | Bambu export/FTPS, ESP32 配网, firmware `$` editor, auto paper feeder macros, multi-device queue, AI assistant (user's own endpoint), cloud template library, PDF export | per feature, on demand |

Ordering rationale: R1–R2 make the existing loop trustworthy (the most painful ChiXie3 problems
are stalls and drops), R3–R6 cover what daily users of notes and 作文格 touch, R7 opens the form use
case, R8 polishes realism, R9–R10 widen the device use, R11–R12 are platform and niche.

## 6. Appendices

### 6.1 Appendix A: settings keys (`config.ini`)

Defaults are the values of the sample file unless the live UI shows otherwise. "→" is the
chixie-rs equivalent (`config.rs` `Settings`, `HandwritingStyle`, `DeviceProfile`, `Template`).

| Section / key | Meaning | Default (sample / UI) | chixie-rs |
|---|---|---|---|
| GeneralConfig/Default_Function | start mode (0 = 写字功能) | 0 | not needed (one mode) |
| GeneralConfig/Synchronization_Parameters | 参数同步 (0 = 开启 per UI) | 0 | not planned |
| GeneralConfig/Language | 0 = 简体中文 | 0 | `UiSettings` locale (M6) |
| GeneralConfig/Window_Size, Window_IsMaximized | window geometry | 1920×1044, true | M6 |
| GeneralConfig/DeviceManagerStatus | device pane start state (0 = 默认收起, `DefaultHide`) | 0 | `UiSettings` pane visibility |
| DrawConfig/DrawOrLaser | 绘图 / 激光 mode | Draw | device type (R10/R12) |
| DrawConfig/PaperType, PaperName, PaperOrientation | plotting paper | StandardPaper, A4, Horizontal | `PageSettings` paper |
| DrawConfig/CustomPaperSize | named custom sizes (pickled dict) | 4 entries | `PaperPreset` list |
| DrawConfig/CAD_Import_Mode | 拆分导入(根据源文件属性) | 1 | DXF import mode |
| FileConfig/DefaultFolderPath | last folder | user folder | last folder in `UiSettings` |
| FontConfig/Tag_Filter | 字体库 filters (6 tags) | 1, 2, 3, 4, 5, 6 | 字体库 UI state |
| FontConfig/Sort_Rule | 字体库 sort | 1 | 字体库 UI state |
| FontConfig/MainFont, MainSymbolFont, MainEnglishFont, MainNumberFont | font roles (ids) | 1 each | `FontRoles` (R4) |
| FontConfig/FontsDefaultSize, FontsMixRatio | default size, mix weights (not in sample) | — | `PageSettings.font_size`, `FontSet.mix` |
| WriteConfig/NowWritingTempName, NowWritingTempType | active template (3 = composition, inferred) | 作文格1, 3 | document template |
| WriteConfig/PreviewMode | single/double page | 1 | view setting |
| WriteConfig/min/max_char_spacing_zh | CJK spacing (mm) | 0.6 / 1.1 | `char_spacing_zh` |
| WriteConfig/min/max_char_spacing_en | Latin spacing (mm) | 0.5 / 1.0 | `char_spacing_en` |
| WriteConfig/min/max_space_width_zh | CJK space (mm) | 5.0 / 7.0 | `space_width_zh` |
| WriteConfig/min/max_space_width_en | Latin space (mm) | 2.0 / 3.0 | `space_width_en` |
| WriteConfig/min/max_vertical_offset | glyph offset (mm) | 0.0 / 0.3 | `vertical_offset` |
| WriteConfig/intelligent_vertical_offset | 智能偏移(同向偏移) | false | `intelligent_vertical_offset` |
| WriteConfig/row_offset_style | unknown variants | 1 | `RowOffsetStyle` (VERIFY) |
| WriteConfig/random_correct_offset | unknown | false | `random_correct_offset` (guess) |
| WriteConfig/path_planning_type | 1 = 之字型 (inferred), 2 = 弓字型 [UI] | 2 | `PathPlan` (R5) |
| WriteConfig/start_point_mode | origin mode (encoding unknown) | 11 | `Origin` (R5) |
| WriteConfig/min/max_first_line_indent | indent (mm) | 15.5 / 17.0 | `first_line_indent` |
| WriteConfig/first_text_not_align, _range | per-line start offset (mm) | true, 2.0 | `line_start_offset` |
| DeviceConfig/AutoConnect | 自动连接机器 | false | `DeviceSettings.auto_connect` (R1) |
| DeviceConfig/DeviceParams | `{name: {firmware, deviceType, writeSpeed, pauseTime, penUp, penDown, x_plus, x_reduce, y_plus, y_reduce, WriteDonePos}}` | one device: firmware 3, type 0, 19000, 0, 0, 2, X+/X-/Y+/Y- | `DeviceProfile` (imported by `chixie3_devices.rs`) |
| DeviceConfig/DefaultDevice | default device name | 默认设备 | profile default |
| DeviceConfig/CustomCommand | custom buttons `{name: cmd}` | empty | custom commands list |
| DeviceConfig/show_coordinate_region, show_command_window | pane sections | true, true | UI state |
| DeviceConfig/CustomNetworkDevices | `{name: {ip, port}}` | one entry | network device list |
| DeviceConfig/ChiXieDevices, DeviceControlPannelMode | 炽写 card devices and panel mode (not in sample) | — | presets (R12) |
| MulticolorDrawing/EnableMulticolorDrawing | 多色绘制 | false | multicolour flag (R5) |
| MulticolorDrawing/PenChangingMode | 手动 / 自动换笔 (1 = manual, inferred) | 1 | pen-change mode |
| CharacterDeformation/EnableCharacterDeformation | master switch | false | `deform` flag |
| CharacterDeformation/enable_base_deform, enable_shape_deform | sub-switches | true, true | deformation config |
| CharacterDeformation/min/max_horizontal_scale | 胖瘦 % | 96 / 104 | base scale x |
| CharacterDeformation/min/max_vertical_scale | 高矮 % | 98 / 102 | base scale y |
| CharacterDeformation/min/max_rotation_angle | 旋转 ° | −2 / 2 | base rotation |
| CharacterDeformation/stroke_deform_degree | 笔画变形幅度 | 1 | stroke amplitude |
| CharacterDeformation/randomly_delete_trailing | 随机删除小尾巴 | true | tail removal (R8) |
| CharacterDeformation/shape_deform_degree | 字形变形幅度 | 7 | shape level |
| ParityPageConfig/ParityPageDirectionMode | 页面旋转模式 (0 = 不旋转) | 0 | `PageRotation` (R5) |
| BambuLabConfig/DeviceIP, DevicePassword | printer IP, access code (plaintext!) | empty | profile + keyring (R12) |
| BambuLabConfig/PenUp, PenDown | Z heights | 8, 4 | profile |
| BambuLabConfig/EnableSound, XOffset, YOffset, OriginMode, DeviceType | options | 1, 0, 0, 0, 0 | profile |
| LoginConfig/Account, Password, RememberPassword, AutomaticLogin | login (password plaintext) | (removed) | account in settings, password in keyring; never imported |

### 6.2 Appendix B: server endpoints

Base `https://chixie.com.cn/client_api/`. All POST with JSON and header `token` where verified.
"Used" = used by chixie-rs today.

| Endpoint | Purpose | Known shape | chixie-rs |
|---|---|---|---|
| `user/login` | login | req `{account, password, platform:"windows"}` → `data {token, access_token, refresh_token, expires_in, backup_token, username, phone, role_code, role, permissions[], expiry_date, period_of_validity, async_operation}` [cap] | used |
| `user/logout` | logout | body `{}` guessed | used (VERIFY) |
| `user/heartbeat` | online check | req `{}` → `data:"ok"` [cap] | used |
| `user/collect_logout_info` | telemetry on exit | unknown | not planned |
| `user/collect_missing_characters` | report missing glyphs | unknown | not planned |
| `update/check_update` | version check | req `{major_version, minor_version, fixed_version, alphabet_version, soft_type}` [cap] | not used |
| `update/compare_file_differences` | MD5 file diff | unknown | not planned |
| `font/get_online_fonts` | catalog | req `{filter_tags[], sort_rule, page_num (1-based), search_name}` → `FontInfo[]` (9/page) [cap] | used |
| `font/get_collected_fonts` | collection | req `{page_num}` (0 = all) → `FontInfo[]` [cap] | used |
| `font/get_recorded_fonts` | user's recorded fonts | unknown (paged) | R4 |
| `font/get_main_fonts_info` | role font ids | unknown | R4 |
| `font/collect_a_font` | collect/uncollect | unknown | R4 |
| `font/get_daily_free_font` | daily trial font | unknown | P3 |
| `font/get_font_access` | whole font (Fernet token) | req `{font_id, token}` → `data` token string [cap] | used |
| `font/get_character_access` | per-character strokes | HTTP 404 now [cap] | dead |
| `font/download_font` | whole-font download | unknown | not used |
| `font/get_standard_br` | standard glyph bboxes | unknown | not needed (local) |
| `font/admin_tune_fonts` | admin glyph upload | unknown | not planned |
| `config/font_config` | get/set font + deformation config | unknown | P3 |
| `config/query_exclusive_config_file`, `config/query_config_download_url`, `config/download_exclusive_config_file` | license-bound config | unknown | not planned |
| `pay/get_packages`, `pay/upgrade_payment`, `pay/check_upgrade`, `pay/bind_license` | purchase / license | fields `package_name, package_price, available_duration`; `license_code`; `pay_url` [high] | not planned |
| `invitation/get_invitation_code`, `invitation/bind_invitation_code`, `invitation/query_binding_of_invitation_code` | invitation codes | unknown | not planned |
| `file_parsing/dxf` | server DXF parse | unknown | not needed (local) |
| `file_parsing/image-to-table-template`, `-2` | table frame recognition | base64 image → cell corner points [med] | R7 (optional) |
| `text_processing/text_block` | word blocks for 作文格 | unknown | not needed (local) |
| `ai/get_ai_account` | Spark API keys | req `{ai_type}` → keys [cap] | not planned |
| `gcode_page/gcode_tool_page` | G-code tool web page | HTML | replaced natively |
| `font_page/font_library_home_page?token=` | font library web page | HTML, token in URL | replaced natively |
| `user_guide_service/user-guide-and-service-page` | help page | HTML | link only |
| `page/register`, `page/forget-password`, `page/update-log` | website pages | HTML | links |
| OSS `font_preview/font_<id>.png` | public preview image | PNG, no auth [cap] | used |
| `default_res/driver/windows/CH341SER.EXE` | driver download | EXE | not planned |
| (unlisted) template library listing | 模板库 online templates | unknown | capture first |

Device-side HTTP (ESP3D / Grbl_ESP32): `/command?commandText=…&PAGEID=0`, `/upload` (list,
delete, deletedir, createdir, multipart upload), `/description.xml` (UPnP), websocket (purpose
VERIFY); see protocol.md.

### 6.3 Appendix C: file formats

| File | Owner | Format | Notes | chixie-rs handling |
|---|---|---|---|---|
| `config.ini` | ChiXie3 | QSettings INI, `@Variant(...)` QDataStream values, some `PyQt_PyObject` pickles | password in plaintext under `[LoginConfig]` | `config::import_chixie3` (WriteConfig, CharacterDeformation), `chixie3_devices.rs` (DeviceParams); LoginConfig never read |
| `note_template.cx`, `composition_template.cx` | ChiXie3 | pickle protocol 4 of `{name: NoteTemplate/CompositionTemplate}` | fields in protocol.md | `chixie3_templates.rs` (safe reader) |
| `table_template.cx`, `composition_temp.cx` | ChiXie3 | table templates: pickle + Fernet (hard-coded key); V1/V2 layouts | background images as base64 | not yet (R7, optional) |
| `*.cset` | ChiXie3 | pickle of `DeviceParams` dict | executes code in ChiXie3 | planned via safe reader |
| `*.cdoc` | ChiXie3 | ChiXie document items + template, Fernet-encrypted (pickle likely) | MIME `application/chixie-doc-items` for clipboard | not planned unless requested |
| `jieba_dict.txt` | ChiXie3 | jieba dictionary | 作文格 segmentation | not needed |
| `update_log.txt` | ChiXie3 | text | shown once after update | — |
| `plugin/test.py` | ChiXie3 | Python | run in-process | not supported |
| `one_stroke_test.txt` | ChiXie3 | `X,Y` lines + `pen_up`/`pen_down` | one-stroke tool | readable by our one-stroke tool |
| `<n>页.gcode`, `page_<n>.gcode` | ChiXie3 | G-code per page | export / SD upload | our export: one file per page |
| `ChiXie-Log.log` | ChiXie3 | rotating text log | `%(asctime)s - %(filename)s[line:%(lineno)d] - %(levelname)s: %(message)s` | — |
| `settings.toml` | chixie-rs | TOML | `%APPDATA%\chixie-rs\config` | `config.rs` |
| `templates.toml` | chixie-rs | TOML | user templates | `template.rs` |
| `*.chx` | chixie-rs | JSON: text, char ids, seed (v2 adds runs, template, origin, tweaks) | reproducible handwriting | `file_ops.rs` |
| `draft.chx` | chixie-rs | as `.chx` | autosave, restored at start | `file_ops.rs` |
| `fonts/<id>.json`, `fonts/index.json`, `fonts/previews/<id>.png` | chixie-rs | decrypted glyph map; id → name; ink mask | offline fonts | `font_store.rs`, `cloud_preview.rs` |
| font payload | server | Fernet token (+2 junk chars) → JSON `{char: [advance, X, Y]}` | key = `backup_token` with the last two chars swapped | `font.rs` |

### 6.4 Appendix D: glossary

| Term | Meaning |
|---|---|
| 炽写 / 赤写 | ChiXie3's UI name / chixie-rs's UI name |
| 写字机 | handwriting robot: a pen plotter with a pen-lift (Z stepper, servo or solenoid) |
| 书写 / 绘图与激光 / 主页 | writing mode / plotting mode / backstage |
| 笔记 / 信纸 / 文档模板 | ruled letter paper template |
| 双线 | dual-line rows (an upper band above each writing line) |
| 分栏 | columns on a note page |
| 作文格 | composition grid paper: one character per square cell |
| 表格模板 / 母单元格 / 内部行数 | form template / parent cell / writing rows inside a cell |
| 同型标记 | linking equivalent cells across pages |
| 数据源 | generated cell content (fixed, random number, random choice, imported) |
| 溢出 | text that does not fit a table cell |
| 字体库 / 收藏 / 录制 | font library / collected fonts / user-recorded fonts |
| 主线字体 / 主英文字体 / 主符号字体 / 主数字字体 | font roles by character class |
| 混用 | mixing several fonts with weights |
| 缺字 | a glyph missing from the chosen font |
| 字符间隔 / 空格宽度 / 垂直偏移 / 首字不对齐 / 首行缩进 | spacing jitter / space width / vertical jitter / per-line start offset / paragraph indent |
| 基础变形 / 字形变形 | per-glyph affine jitter (+ stroke jitter) / thin-plate-spline warp |
| 随机删除小尾巴 | randomly removing small stroke tails |
| 字符微调 | per-character manual adjustment |
| 书写优化 / 路径优化 | path simplification or resampling |
| 避头尾 | kinsoku: no closing punctuation at line start, no opening at line end |
| 抬笔 / 落笔 | pen up / pen down |
| 落笔高度 / 落笔角度 / 落笔指令 | pen-down Z (stepper) / servo angle / raw command (custom device) |
| 落笔后暂停 | dwell after pen down |
| 写字速度 | feed rate (mm/min) |
| 书写原点 / 设置原点 / 回到原点 | job origin marker / `G92 X0 Y0` / move to X0 Y0 |
| 纸张起点 / 框线起点 / 自定义原点 | origin modes |
| 之字型 / 弓字型 | path planning: raster vs serpentine row order (semantics VERIFY) |
| 写完位置 | where the pen parks after a job (原点 / 原点左2cm / 任务结束点) |
| 换纸 | sheet change between pages |
| 断点续写 | resume an interrupted page from a chosen character |
| 换笔 / 多色绘制 | pen change / multicolour drawing |
| 页面旋转 / 奇偶页方向 | 180° output rotation for odd/even pages |
| 巡边 / 绘制框架线 | trace the bounds pen-up / draw the template lines |
| 百宝箱 | toolbox dialog |
| 拓竹 | Bambu Lab (3D printers used as plotters) |
| 远程TF卡 / SD卡 | the ESP32 board's SD card |
| 配网 | Wi-Fi provisioning of the board |
| 固件参数 | Grbl `$` settings |
| 报警 / 解锁 | Grbl Alarm state / `$X` |
| 酷玩 | "fun" generators (seal, maze, fractal, 3-D text) |
| 小炽 | the AI assistant's persona |

### 6.5 Appendix E: source conflicts and unresolved questions

**Conflicts found**

1. **Path planning semantics.** A: 之 = every row left-to-right, 弓 = alternate rows reversed. C:
   之 = alternate rows reversed, the other mode "工" = same direction. Live UI names the modes
   之字型(默认) / 弓字型 and the sample config (`path_planning_type=2`) has 弓 selected. Resolution:
   trust names + A (inferred), VERIFY by capture.
2. **Base deformation 高矮 default.** UI and config: 98–102%. protocol.md capture notes: 96–104%.
   UI/config win for defaults.
3. **Toolbox content.** Static slice: 7 buttons. Live UI: 10 tools (adds 任务编程, 固件烧录, 多设备群控).
   Resolved: the three are Designer-only placeholders with no handler (also inert when elevated).
4. **Template library.** A calls the home template widget a placeholder; the live backstage shows a
   populated online library with categories and paging. Live wins (server-driven).
5. **Plotting 模板库 button.** B: opens the template manager; live: the template library has 绘图/激光
   categories. Inferred: opens the library.
6. **Servo pen command.** Static literals include `M3S` for the servo branch; the Telnet capture shows
   `G1G90 Z<value>` for servo too. Capture wins.
7. **落笔后暂停.** Static generator emits `G4 P<pause>`; the Telnet capture sent no dwell with 0.5 s.
   Capture wins for ChiXie3's behaviour; chixie-rs emits a real dwell deliberately.
8. **First-line indent.** Config holds an indent range; captures showed none. Static C explains it:
   indent is only inserted by Tab or the 首行缩进 button. No conflict after all.
9. **字符微调 label.** Static "异常宽度字符优化", live "异常宽度字符处理". Live label used.
10. **Export G-code access.** C says export is super_user only (from protocol.md); A shows the export
    action exists with a visibility setter. Consistent: visible only to admins.
11. **Languages.** Static `core.ChiXie` enum: six (zh-Hans, zh-Hant, en, ja, ko, ru); shipped `.qm`:
    two (CN, EN). The settings form lists four: 简体中文, 繁体中文, 日本語, русский язык [ui-form]. No English
    entry although `appLang_EN.qm` exists. Live UI showed only 简体中文 selected; whether the other
    three work is unverified (VERIFY by switching in the live app).
12. **Cell padding.** C/B: only `internal_margin_left` is used by the generator; the cell attribute form
    has four spin boxes 左/上/下/右内边距 under 排版内边距 [ui-form]. Both can be true (the UI edits all four, the
    generator reads one); treat only the left margin as functional until a capture shows otherwise.
13. **Label wording.** Static / earlier text → form [ui-form]: 显示分割线 → 分隔线 (note columns); 启用虚线分割 →
    虚线分割双行 (dual-line); 左上列/左上行… → groups 左上角 / 右下角 (Excel range); 异常宽度字符优化 vs 处理 (see 9).
    Live UI `[UI]` labels and defaults still win where they exist; the form string is used where the live
    walk did not reach the dialog.
14. **Alignment centre.** The ribbon documentation lists 居中, but `UI_MainWindow` has no 居中 string
    (only 左对齐, 右对齐, 两端对齐) [ui-form]. The live ribbon has a centre button (UI), so its caption is set elsewhere
    (an icon-only button or a code-set tooltip). No behavioural conflict.
15. **Treasure-box form offset.** Earlier note cited `UI_TreasureBoxDialog` @a04a86d; the form mining shows the
    module constants at @a049713. Probably two views of the same module (not checked); use the latter for lookups.
16. **N1 device test scope.** Earlier text (from identifiers) named a buzzer melody, 锯齿折线 and 基础图形测试.
    The form lists nine buttons (落笔测试, 回字方框, 蜂鸣器测试, 字符阵列, 自动抬笔测试, 交叉网格, 锯齿折线,
    同心圆与螺旋线, 书写测试) [ui-form]; the form wins, 基础图形测试 is probably the umbrella for the shape tests.

**Unresolved (VERIFY list)**

- Encodings: `start_point_mode` (11), `row_offset_style` (1), `PenChangingMode` (1), the integer
  values of `CAD_Import_Mode` (options now named: 作为整体导入 / 拆分导入(根据源文件属性)) and of
  `ParityPageDirectionMode` (the four labels are now known: 不旋转, 奇数页旋转180°, 偶数页旋转180°, 旋转180°
  [ui-form]; their order suggests 0–3).
- Algorithms: intelligent vertical offset formula; TPS amplitude per slider level; tail-removal rule;
  stroke-level jitter form; wrap test (glyph origin vs right edge); space width capture.
- Templates: random row count per page; dual-line text placement; column fill order; table cell text
  placement; 异栏同型 meaning; 同型标记 user effect; 填充文本域 fit rule (the tooltip now says it fills an
  inputtable field into a selected closed region, [ui-form]).
- Forms: combo-box item lists filled in code (传动类型, 驱动模式, 运动模式, 优化规则, 字体加载模式, 设备类型, 印章类型/图案,
  迷宫形状/复杂程度), spin-box ranges, which recommendation hint in 固件参数 belongs to which row, 智能笔录制
  (an extra font filter / category) and the meaning of the AI feedback report, 排版内边距 effect.
- Formats: underline/strike/title/super/subscript rendering and sizes; `.cdoc` serialisation; table
  template layout.
- Device: N1 speed tiers, pen depths, direction modes, `$190–$253`; ChiXie3 laser / 3D-printer pen
  literals; websocket use on the board; Telnet `?` handling.
- Server: shapes of `collect_a_font`, `get_recorded_fonts`, `get_main_fonts_info`,
  `get_daily_free_font`, template-library listing, table recognition.
- UI: behaviour of 拟人, AI绘图 (inert un-elevated; 任务编程/固件烧录/多设备群控 resolved as placeholders); 半格空格
  modifier; 打印文字 vs 打印背景图.

**Resolved by the Designer labels ([ui-form])**: the batch-import "second tab" (批量名单建页); the exact option
names of 页面旋转 and CAD导入; the full N1 device-test list; the 炽写设备高级设置 and 炽写ESP32固件设置 field sets;
fonts' seventh filter 智能笔录制 is *new* (see Unresolved); the 工作区巡边 modes (抬笔 / 落笔 / 抬笔（顶点落笔）) and ranges
(模板框线区域 / 实际书写区域).

