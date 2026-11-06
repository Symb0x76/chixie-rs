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
