# Hardware checklist

Run on the real machine before a milestone that touches devices is called done (plan §7). Copy
the table into the milestone's notes, fill in the result and date, and file anything that fails.

**Setup:** Grbl_ESP32 ChiXie board (CH340), USB to the PC, 300 × 300 mm travel, pen holder with
a pen, A4 paper taped at the origin. Start `sast-writer`, log in only if fonts must download.
Keep a hand near the power switch for the first run after any device-code change.

| # | Check | How | Expected | Result |
|---|---|---|---|---|
| 1 | Auto-detect and connect | 设备 › 连接 with port 自动 | the CH340 port is chosen (★), state Idle, position shown | |
| 2 | Jog | jog pad: 1 mm and 10 mm in all four directions | head moves the stated distance and direction | |
| 3 | Pen up / down | 抬笔 / 落笔 | pen lifts clear of the paper / touches it | |
| 4 | Set origin | jog to a corner, 设为原点 | position reads X0 Y0 | |
| 5 | Short job | type `你好赤写 A1`, 开始书写 | written as on screen; pen parks at X0 Y0 at the end | |
| 6 | Multi-page job | 3 pages of text | sheet-change prompt between pages; resumes after 已换纸继续 | |
| 7 | Pause / resume | 暂停 mid-stroke, wait 5 s, 继续 | head stops at once; continues without a gap or duplicate stroke | |
| 8 | Stop | 停止 mid-job | head stops, pen lifts, parks at X0 Y0; no alarm left | |
| 9 | Alarm and unlock | trigger an alarm (e.g. soft-limit jog), then 解锁 | alarm text shown in Chinese; unlock clears it | |
| 10 | USB drop | pull the USB cable mid-job, plug it back in | toast with page and line; reconnects via 自动 (COM number may change) | |
| 11 | Resume after drop | after #10, resume (M2: 断点续写) | continues from the interrupted stroke; nothing duplicated or missing | |
| 12 | Wireless | same board over Wi-Fi/Telnet (M1) | connects; a short job writes as over USB | |
| 13 | Templates | one 信纸 and one 作文格 page | text sits on the printed lines / in the cells (± 0.5 mm) | |
| 14 | Long job | 10 pages, sheets changed promptly | completes without stalls; no drift between pages | |

Notes:
- The CH340 board can drop off USB under motor load and come back on another COM number; use
  自动, not a fixed port. Power the board separately if drops repeat.
- Record the board's `$$` output once per firmware change (设备 › 参数设置 shows it).
