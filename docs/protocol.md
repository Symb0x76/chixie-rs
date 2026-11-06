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
- **VERIFY:** the deformation semantics. That needs a capture with `EnableCharacterDeformation` on;
  `rms` > 0 will quantify per-point distortion.
