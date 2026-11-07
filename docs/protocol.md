# ChiXie3 device protocol (recovered)

Source: Nuitka constant pools in `ChiXie3/start.exe` (v3.0.310). No bytecode exists;
everything below is inferred from string constants + names. Items marked **VERIFY**
were not recoverable as strings (numeric constants) and must be confirmed against hardware
or a serial capture of the old app.

## Hardware probe (2026-11-06, COM3, CH340)

- 115200 baud works. `$I` → `[VER:1.3a.20221201:]`, `[OPT:MPHB]`,
  `[MSG:Mode=BT:Name=BtWriter(90:38:0C:6C:9B:E6):Status=Not connected]` → Grbl_ESP32 1.3a, radio in BT mode.
- `chixie-rs stream` of a 10 mm relative XY square: all lines acked, head traced it and
  returned to start (confirmed by operator).
- `$I` does not report RX buffer size; keeping 128 (safe lower bound; Grbl_ESP32 is ≥128).
- `$$` key values: `$100-102=80` steps/mm, `$110/111=12000` `$112=10000` mm/min max rate,
  `$120/121=3000` `$122=8000` accel, `$130-132=300` travel mm, `$22=0` homing off, `$32=0` laser mode off.

## Pen lift (`core.GcodeGenerator.gcode_generator_base`)

`get_pen_up_gcode` / `get_pen_down_gcode` branch on `DeviceType`:
`WritingRobot_StepperPenUp`, `WritingRobot_SteeringEnginePenUp` (servo),
`WritingRobot_ElectroMagnetPenUp`, `LaserEngravingMachine`, `ThreeDPrinter`, `CustomZCommandDevice`.
Only Z literal in the pool is `G1G90 Z` → writing robots all lift via absolute Z
(Grbl_ESP32 maps Z to stepper / RC servo / solenoid in firmware). Laser/3D/custom branches
use strings not recovered — **VERIFY** if needed.

Other literals: `G92 X0 Y…` (set origin), `G90G0 X0Y0` (back to origin, `BackOrigin`),
jog `$J=G21G91 …`, manual pen jog `G21G91 Z` / `G21G91 Z-`.

User config `%USERPROFILE%\ChiXie3\conf\config.ini` `[DeviceConfig] DeviceParams`
(QVariantMap, device "guangku"):

| key | value |
|---|---|
| deviceType | 0 (= `WritingRobot_StepperPenUp`) |
| firmware | 3 (enum index, **VERIFY** mapping) |
| penUp | 0 |
| penDown | "2" |
| writeSpeed | "19000" (mm/min) |
| pauseTime | "0" |
| x_plus/x_reduce/y_plus/y_reduce | `X+` `X-` `Y+` `Y-` (axis direction mapping) |

⇒ on this machine: pen up `G1G90 Z0`, pen down `G1G90 Z2`. Verified on hardware 2026-11-06
with `F1000` (operator confirmed pen touched paper and lifted). Original literal has no `F`
(relies on modal feed); the rewrite should always emit an explicit feed.

## Firmware families (`core.ChiXie` / `firmware_type`)

| Value | Meaning |
|---|---|
| `ChiXie`, `ESP32_Grbl`, `ChiXie_ESP32_Grbl` | Grbl / Grbl_ESP32 → `__connect_grbl`, `__send_command_of_grbl` |
| `Marlin` | Marlin → `__connect_marlin`, `__send_command_of_marlin` |

## Transports (`SerialMode`: `OnlineMode` / `TelnetMode`)

- **USB serial** via pyserial. Port auto-pick matches description `USB-SERIAL CH340`
  or `GD32 Virtual Com Port`. Baud **VERIFY** (assumed 115200, Grbl default).
- **Telnet** (`telnetlib`) to `wireless_ip`, port 23 (`[ESP131]23` sets it). Raw line protocol.
- **HTTP** to the ESP32 web server (Grbl_ESP32 / ESP3D API) for SD card + commands.

## Grbl streaming (`run_of_serial`)

Classic Grbl `stream.py` character-counting: names `RX_BUFFER_SIZE`, `l_count`,
`c_line`, `g_count`, `grbl_out`, `inWaiting`. Buffer size **VERIFY** (Grbl default 128).
Responses: `ok`, `error:*` → `serial_error_signal`, `ALARM` → `alarm_signal`,
unlock with `$X` (reply contains `UNLOCKED`). Task = `initial_gcode` + per-page
`body_gcode` + `end_gcode`; `change_pen_signal` between colours (multi-colour mode).
Pause/resume/stop are Python-side (QWaitCondition), plus realtime `!` `~` `?` `0x18`.

## ESP32 config commands (serial, `wireless_esp32_setting_dialog`)

| Cmd | Use |
|---|---|
| `[ESP110]` | query radio mode (`$ESP110=`); `[ESP110]STA`, `[ESP110]BT` set mode |
| `[ESP100]` / `[ESP101]` | STA SSID / password |
| `[ESP131]23` | telnet port |
| `[ESP140]` | BT name (`$ESP140=`) |
| `[ESP115]ON` | radio on; replies `Connected with` / `BT Started with` |
| `[ESP410]` | Wi-Fi scan, JSON `{"AP_LIST":[{"SSID":..,"SIGNAL":..}]}` |
| `[ESP210]` | SD status |
| `[ESP220]<path>` | run file from SD |

## HTTP API (`sd_card_file_management_dialog`, `upload_file_to_sd`)

- `GET /command?commandText=<cmd>&PAGEID=0` — `?` status (`<Run|`, `<Hold`, `Alarm`), `!` hold,
  `~` resume, `%18` soft reset, `$X` unlock, `[ESP220]<path>` run.
- `GET /upload?path=<dir>` — list → JSON `files`, `total`, `used`.
- `GET /upload?path=<dir>&action=delete|deletedir|createdir&filename=<name>`.
- `POST /upload` multipart, field `myfile[]`, into `/chixie_task/` (files named `page_<n>.gcode`).
- Also a websocket (`websocket.create_connection`) — purpose **VERIFY**.

## Discovery (`wireless_board_discovery_thread`)

Runs `arp -a`, then for each IP `GET http://<ip>/description.xml` (UPnP,
`urn:schemas-upnp-org:device-1-0`, reads `upnp:device` children).

## Writing G-code as streamed (captured 2026-11-06)

**Capture method (no hardware, no export button needed):**
1. Run a fake Grbl that logs every line on `127.0.0.1:23`. It answers `ok` per line and an Idle
   status to `?`.
2. In ChiXie3, open 设备管理 → 连接与控制 → 无线 → **+** and add `127.0.0.1` port 23. Connect, then
   press ▶ (书写).

The Export G-code button (`MainWindow.set_export_gcode_btn_visible`) is gated to `super_user`. The
fake-Grbl path works for any account. ChiXie3 runs elevated, so UI automation must run elevated too.

**Format** (艾玛体, text `你好赤写 A1`, size 100):
```
G21
G90
F19000
G1G90 Z0F19000              (pen up, twice at start)
G1 X+4.205Y-2.376F19000     (travel: G1, not G0; signed coords, trailing zeros trimmed)
G1G90 Z2F19000              (pen down)
G1 X+3.887Y-3.275F19000     (write moves)
...
G1G90 Z0F19000              (pen up after each stroke; twice at the end)
G90G0 X0Y0                  (return to origin)
```
- Origin is the pen's position at start. X grows right; Y is **negative going down** the page.
- One captured stroke per font stroke, in font order. Consecutive repeated points are not streamed
  (`1` has 5 font points but streams 4).

**Scale (`PXScale`), verified:**
- `chixie calibrate` against font id 1 gives mm/unit **1.00000** at size 100, RMS 0.0000.
- A size-50 capture fits the size-100 capture at a ratio of exactly **0.50000**, with per-glyph RMS
  ≤ 0.0004 mm.
- ⇒ `mm_per_unit = font_size / 100` (`calibration::mm_per_unit_for_font_size`).

**Handwriting effects in the default config** (deformation off, so RMS 0):

| quantity | size 100 | size 50 | conclusion |
|---|---|---|---|
| first glyph x | 2.03 | 2.03 | absolute mm; fits `first_text_not_align_range` 2.0 |
| glyph y offsets (你好赤写A1) | −0.276 −0.296 −0.326 −0.186 −0.206 −0.236 | same deltas | absolute mm; spread ≤ 0.3, fits `vertical_offset` 0.0–0.3 |
| extra spacing per glyph (你/好/赤) | — | — | ≈ 1.08 / 1.04 / 0.92 mm: absolute, within zh 0.6–1.1 |

- Extra spacing is solved from advance = a·(size/100) + spacing across both sizes.
- Random values are drawn **per character when the text is laid out**, not per write. Repeated
  writes of the same document are byte-identical. Changing font size keeps the same draws.
**Character deformation (设置 → 字符变形), captured 2026-11-06:**

*Method:* the text is re-pasted each time (an effect only applies at layout), each capture is
written twice (the two are always byte-identical), and a per-glyph affine fit is made against a
deform-off control pasted the same way. The fit is `q = R(θ)·[[sx,k],[0,sy]]·p + t`, plus the
residual after it. Single-stroke `1` is collinear, so its sx is unreliable.

- **基础变形 (base)** — defaults: 胖瘦 96–104%, 高矮 96–104%, rotation −2…+2°, 笔画变形幅度 1,
  随机删除小尾巴 on.

  | glyph | sx | sy | rot | shear | residual mm |
  |---|---|---|---|---|---|
  | 你 | 1.037 | 1.072 | −0.85° | +0.017 | 0.061 |
  | 好 | 1.051 | 0.981 | −1.48° | −0.019 | 0.046 |
  | 赤 | 1.018 | 0.959 | −0.17° | −0.003 | 0.044 |
  | 写 | 0.941 | 1.027 | +0.05° | +0.011 | 0.022 |
  | A | 0.979 | 0.978 | +0.77° | +0.004 | 0.043 |

  - A per-glyph random x/y scale (≈ the configured %; a few values land just outside ±4%,
    **VERIFY** whether the range is exclusive or composed with other terms) and a rotation within ±2°.
  - Plus a stroke-level perturbation of about 0.02–0.06 mm RMS at amplitude 1. Single-stroke glyphs
    show none, so it is most likely a per-stroke offset/warp.
  - "Remove small tails" removed no points in this sample.
- **字形变形 (shape)**, amplitude 7: a consistent italic-like slant on CJK glyphs (shear −0.12…−0.13;
  rotation −2…−3.4°), scale up to +8%, and a non-affine residual of 0.035–0.106 mm. Latin `A` is
  barely affected. The exact warp field is **VERIFY**; model it as slant + small smooth noise.
- **Paste vs typed (unexplained, VERIFY):** text pasted with deformation **off** still gets a
  per-glyph axis-aligned scale relative to typed text (sx 0.70–1.05, sy 0.86–1.01, no rotation, exact
  fit). Typed text matched the raw font at scale 1.

Captures: `captures/{a2_pasted_deform_off,d_base_deform,d_shape_deform7}.gcode` (local only).

**Multi-line layout (captured 2026-11-07, 艾玛体 100, effects off)**
- Text: `你好赤写`×10 + ` A1 A1` ⏎ `写赤好你 写赤 A1` ⏎ `你 好 赤 写`.
- ChiXie3's Telnet streaming stalls on long jobs: at ~44, ~752 and ~1682 lines, even with a 3 ms ack
  delay. After a stall, the only reliable reset is disconnect + restart. The first 2 lines (34 glyphs)
  were captured; every glyph fits the typed reference exactly (no per-glyph scale).
- **Line pitch ≈ 10.25 mm** (baseline means −0.31 → −10.56).
- **Wrap:** 23 glyphs on line 1. The last one starts at x = 161.5 and the next would start at ≈ 169.
  That fits a writable width of ≈ 168 mm (A4 210 − 2×21). Whether the rule tests the glyph origin or
  its right edge is **VERIFY**.
- **Per-line start x** is random in [0, `first_text_not_align_range`=2.0] on **every** line (1.81, 1.42).
- **Advance** = glyph advance + spacing, with spacing drawn per glyph from zh 0.6–1.1 mm (each
  glyph pair spreads ≈ 0.5 mm: 你→好 8.46–8.86, 好→赤 7.29–7.59, 赤→写 5.65–6.01, 写→你 6.42–6.81).
- **Vertical offset** per glyph spans ≤ 0.3 mm on each line (0.28, 0.27).
- **First-line indent** (`min/max_first_line_indent` 15.5–17) is **not** applied here: paragraphs
  start at the same x as wrapped lines (also visible on screen). It probably belongs to another
  template or mode, **VERIFY**.
- **Space width** was not captured (the stream stalled first). On screen, zh spaces look ≈ 5–7 mm,
  consistent with `space_width_zh`; **VERIFY**.
- The paste-vs-typed scale anomaly above did not reproduce here, where all sx = sy = 1.000. Treat it
  as an artifact of that one paste.

## Page templates (`%USERPROFILE%\ChiXie3\conf\*.cx`, decoded 2026-11-07)
Python pickles (protocol 4) of `{name: object}`; the objects come from `core.DataModel.*` and carry
their state as a plain `__dict__`. They are read by `chixie_core::chixie3_templates`, a small pickle
reader that never executes class references.
- `note_template.cx` holds `NoteTemplate` objects, which become ruled templates:
  - `page_width`, `page_height` and `margin_top/bottom/left/right`, all in mm (whole mm stored as int);
  - `min_row_num` / `max_row_num`: a fixed row count when equal, otherwise 随机行数;
  - `enable_dual_line_mode` + `up_row_height` (mm);
  - `enable_dashed_line_partition`;
  - `column_num`, `column_width[]`, `column_space[]`, `column_width_equal`, `show_dividing_line`,
    `column_same_type`. Only equal columns are mapped, with the gap taken from `column_space[0]`.
- `composition_template.cx` holds `CompositionTemplate` objects, which become grid templates:
  - `page_width`, `page_height`;
  - `cell_row_number`, `cell_column_number`;
  - `left_margin`, `top_margin`;
  - `cell_width`, `cell_height`;
  - `row_spacing`, `column_spacing` (mm).
- **Template captures (2026-11-07, 艾玛体 100, deformation off; ChiXie3's spacing and vertical
  jitter stay on):**
  - **我的笔记1** (A4, margins L20 T30 R20 B20, 24 rows), text `你好你好⏎好你⏎你`:
    - The origin is the writable top-left; the rules sit every 247/24 = 10.29 mm (also measured on
      screen).
    - Line tops (glyph y = 0) were at 2.20, 12.27 and 22.07 mm below the origin, i.e. 2.20, 1.98 and
      1.49 mm into their rows (mean 1.89). That puts the em box's bottom ≈ 1.4 mm = 0.2 em above the
      row's line.
    - There is a per-row spread of ≈ ±0.4 mm beyond the ≤ 0.3 mm per-glyph vertical jitter. We
      model the mean only.
  - **作文格1** (8 × 8 mm cells, column gap 0, row gap 2.9), text `你好，你好 你。`:
    - The origin is the grid's top-left.
    - Each character is centred in its cell at the font size (not scaled), within ±0.3 mm.
    - Punctuation is drawn low in its own cell, and a space takes a cell.
    - Pasted line breaks are dropped in grid mode (a ChiXie3 quirk). A typed Enter starts the next
      row.
  - Captures are local only: scratchpad `ruled.log` / `grid.log`. Regression tests:
    `template::tests::{ruled_rows,grid_cells}_match_chixie3_capture`.
- How our layout uses them:
  - The row pitch is the writable height ÷ rows.
  - Ruled rows: the glyph top is the row bottom − em − 0.2 em (never into the dual-line band), with
    the nominal glyph height `EM_UNITS` = 7 font units. **Matches the capture.**
  - Grid templates place one character per cell, centred at the font size. Spaces and punctuation
    take a cell. **Matches the capture.**
  - Still **VERIFY** (not captured):
    - A random row count is drawn once per page.
    - Columns fill top to bottom, then left to right.
    - Dual-line mode keeps the text below the upper band.
  - Ruling is never plotted, because the paper is pre-printed.
