# ChiXie3 feature inventory, slice A: device layer, G-code generators, main window shell

Scope: offsets 0x9441b78 to 0x94a425f of `slice_A.txt`. This covers the auto paper device, auto upgrade, the AI assistant, the document/template data models, serial/Telnet communication, the whole device management area (list, add device, firmware, control card, page writing/plotting views, SD card, wireless), the three G-code generator families, the main window and its helper dialogs (treasure box), the home page and font card, and the paper selection widget. Everything here was inferred from Nuitka constants only; no behaviour is observed. UI strings are quoted exactly. Offsets are hex, as in the slice (`@9479554`). Class docstrings and Chinese comments survived compilation and are the best evidence (author tag "沈刘兴", dates 2022-2025; the last module in the slice is dated 2025-12-15, so this build is very recent).

The headline findings: there are two device families with two very different control UIs. The first is the proprietary 炽写 N1 / 炽写 ESP32 (`ChiXie_N1`, `ChiXie_ESP32`, firmware `ChiXie_ESP32_Grbl`) with a "device card" control (speed mode, direction mode, pen pressure, write-done position, auto origin, beep). The second is any "other device" (`Grbl`, `Marlin`, `ESP32_Grbl` firmware; five device types) with a plain parameter form (writeSpeed, penUp, penDown, pauseTime, x/y direction mapping). Writing G-code is generated per page, with path planning (之/弓), odd/even-page rotation, multicolour pen change, optional stroke simplification or Catmull-Rom/uniform expansion ("writing improve"), and a pen-tip pressure test (`route_tip`).

## Index of features

1. Core enums and configuration vocabulary (Constant, `core.ChiXie`)
2. Device types, firmware types, connection modes
3. Device management panel for "other" devices (设备管理)
4. Add device dialog (添加设备)
5. Device parameters: keys, import/export (`.cset`)
6. Custom command dialog (自定义命令)
7. Manual jog / origin / pen controls on the other-device panel
8. Grbl error translation table
9. Serial communication layer (ChiXieSerial, `chixie_serial_thread`)
10. Auto connect thread and CH340/GD32 port discovery
11. CH340 driver check and install
12. Wired USB connect flow and Grbl alarm unlock
13. Wireless (Telnet / ESP32) connection and board discovery
14. Add network device manually
15. Wireless ESP32 settings dialog (WiFi/BT)
16. Firmware parameter dialog for Grbl (simple/expert modes, `$RST=*`)
17. ChiXie ESP32 firmware settings dialog (3 pages, PWM, wireless)
18. ChiXie N1 advanced device settings (TMC driver, one-click defaults)
19. ChiXie device control drawer (炽写设备管理)
20. ChiXie device control card (jog, origin, speed/direction/pressure parameters, menu)
21. ChiXie firmware update dialog
22. ChiXie device test dialog (admin only)
23. Writing management view (page list, start/stop, break-point resume)
24. Plotting management view
25. Writing improve dialog (书写优化)
26. Writing G-code generator (ChiXiePageToGcode)
27. Plotting G-code generator (ChiXiePlottingPageToGcode, per-item)
28. Generic G-code generator base and other-device generators
29. G-code generator for 3D printers / Bambu Lab
30. Bambu Lab assistant (拓竹助手, FTPS upload)
31. SD card file management (remote TF card)
32. Upload page to SD card and run
33. Auto paper device (自动换纸设备)
34. Multicolour drawing and pen-change prompt
35. Parity page direction (odd/even rotation)
36. Path planning, row offset, start point, data source and font priority enums
37. Workspace patrol and frame drawing (巡边 / 绘制框架线)
38. One-stroke drawing (一笔画)
39. G-code tool (web bridge)
40. Treasure box (百宝箱)
41. Plugin system (plugin edit dialog)
42. Font data optimize (字体数据优化)
43. Export G-code (page to `.gcode`)
44. Save/open ChiXie document (`.cdoc`)
45. Document and template data models
46. Main window: ribbon, modes, status bar, signals
47. Main window: tray, heartbeat, screen-resolution restart, close
48. Software init, login, auto login, login success
49. Daily free font, update log, record font guide, user guide dialogs
50. Home page (主页): fonts, templates, feedback, settings, account
51. Font card and font widget
52. Font library web dialog
53. Missing characters and auto font replacement
54. AI assistant (小炽, iFlytek Spark)
55. Auto upgrade
56. Paper selection widget (drawing/laser)
57. Backend endpoint constants
58. Cross-reference with existing docs

## Core vocabulary

### Core enums and config vocabulary (`core.ChiXie`, `核心枚举`)
- Where: not a UI feature; it names every mode the UI exposes. Module doc: "炽写的枚举类 / 用于存放一些配置数据，对应界面上或者程序中某些变量" @9446d7b.
- What it does: defines the closed sets below (values are ints or strings; only names survive).
  - Functions: `WritingFunction`, `PlottingFunction`, `TemplateFunction` @9446e1e..9446e4b.
  - Languages: `SimplifiedChinese`, `TraditionalChinese`, `English`, `Japanese`, `Korean`, `Russian` @9446e5d..9446eae.
  - Paper: `StandardPaper`, `CustomSizePaper`; `Horizontal`, `Vertical` @9446ebc..9446ef7.
  - Plotting graphics: `GraphicPoint/Line/Polyline/Polygon/Rectangle/Round/Arc/Curve`, `PlottingTextBox`, `PlottingTable`, `PlottingIdeaSeal`, `PlottingIdeaMaze` @9446f06..9446fe5. (Seal and maze are "idea function" items.)
  - Insert modes: `NoInserting`, `InsertingBasicGraphic`, `InsertingTextField`, `InsertingIdeaFunc`, `CAD_Group_Import`, `CAD_Single_Import` @9446ff7..9447054.
  - Writing templates: `NoteTemplate`, `TableTemplate`, `CompositionTemplate` with icons `note.svg`, `table.svg`, `composition.svg` and `WritingTempIconDict` @9447067..94470d3. Preview: `SinglePagePreview`, `DoublePagePreview`.
  - Alignment: `AlignLeft/HCenter/Right/None/Justify/Top/VCenter/Bottom` @944710e..944716e.
  - Input type setting: `InputTypeSetting`, `KeyPressTypeSetting`. Import modes: `DirectImport`, `OverwriteImport`, `AppendImport` @944717b..94471c1.
  - Firmware: `Grbl`, `Marlin`, `ESP32_Grbl`, `ChiXie_ESP32_Grbl` @94471cf..94471e9.
  - Device types: `WritingRobot_StepperPenUp`, `WritingRobot_SteeringEnginePenUp`, `WritingRobot_ElectroMagnetPenUp`, `LaserEngravingMachine`, `ThreeDPrinter`, `CustomZCommandDevice` @94471fc..9447280.
  - Row offset: `RowOffsetUpperRight`, `RowOffsetLowerRight`, `RowOffsetNone` @9447296.
  - Path planning: `PathPlanningZhi`, `PathPlanningGong` @94472cf.
  - Pen change: `ManualPenChange`, `AutomaticPenChange` @94472f2.
  - Origin: `StartPointOfPaper`, `StartPointOfFrame`, `StartPointOfCustom` @944731c.
  - Default hide/show side: `DefaultHide`, `DefaultShowOnLeft`, `DefaultShowOnRight` @9447360 (probably the device drawer position).
  - Parity page: `SameDirectionForParity`, `OddPagesRotate180`, `EvenPagesRotate180`, `SameDirection180` @9447394..94473d3.
  - Item count: `LargeNumberOfItems`, `SmallNumberOfItems`.
  - Table data source: `NoDataSource`, `DefaultValueDataSource`, `RandomValueDataSource`, `CustomCellDataSource` @944740d..944744a.
  - Font priority in cells: `CellSpecifiedFontFirst`, `CellFontLibraryConfigFirst`, `CellRandomFontFirst`; `RandomWithinValue`, `RandomWithinContent` @9447460..94474bc.
  - Device model/panel: `ChiXie_N1`, `ChiXie_ESP32`, `AppFirmware`, `BootFirmware`, `ChiXieDevice`, `OtherDevice`, `Lock`, `UnLock` @94474d6..944754e.
  - Speed modes: `SlowSpeedMode`, `CommonSpeedMode`, `HighSpeedMode`, `BerserkMode`, `SlowSpeedMode1` @9447556..9447592 (English display names are not in the slice; "BerserkMode" suggests a top speed tier, "SlowSpeedMode1" an extra slow tier; the visible combobox item is "常规（推荐）" for the common mode @946826f).
  - Direction modes: `Vertical_1`, `Horizontal_1`, `Vertical_2`, `Horizontal_2` @94475a2..94475c8 (UI: "竖向（默认）", "竖向(翻转)", "横向(翻转)" plus one more not captured).
  - Write-done position: `BackOrigin`, `BackOriginLeft2`, `BackNone` @94475d6..94475f3 (UI: "原点", "原点左2cm处" / "原点左侧2cm", "任务结束点").
  - Font mode: `FontDefaultMode`, `FontMixMode` @94475fd, 944760e (probably the "font mixer" option).
  - Route improve: `RouteImproveNone`, `RouteImproveSimplify`, `RouteImproveCatmullRom`, `RouteImproveUniform` @944761b..944765b.
- Internals: Python `enum` class `ChiXie` (Nuitka module `core.ChiXie`, `core\ChiXie.py` @9447680). Companion globals `Variable`, `Constant` (`core.Constant` @944d500).
- Access: n/a.
- Confidence: high for names, medium for meanings of the speed/direction tiers.

### Device types, firmware types, connection modes
- Where: Device management panel combos `firmwareComboBox` and `deviceTypeComboBox` @9454caa, 9454cf8; "添加设备" dialog.
- What it does:
  - Firmware combo items: `Grbl`, `Marlin`, `ESP32_Grbl`, `ChiXie_ESP32_Grbl` @9454ccb..9454ce5.
  - Device type combo items (UI): "步进抬笔写字机", "舵机抬笔写字机", "电磁铁抬笔写字机", "激光雕刻机", "3D打印机", "自定义抬落笔指令设备" @9454d11..9454d77.
  - Connection modes: USB serial (`SerialMode`, `serial_port`, `connect_mode`) and Wi-Fi/Telnet (`telnet_port`, `wireless_ip`) @9451ba8..9451bdc. Bluetooth is also supported, but only as a serial port: a Bluetooth SPP link shows up as a COM port described as "蓝牙链接上的标准串行" which the app labels "（蓝牙）" @9451c4e, 9451c97. CH340 ports are labelled "（CH340）". Bluetooth mode on the ESP32 board is configured in the wireless dialogs and then used "在电脑中连接该蓝牙，刷新软件内串口，连接相应新增的端口即可" @947d56f.
  - Wireless is limited to ESP32: "目前仅支持ESP32类型的控制板与相关固件进行无线连接" @9458302.
  - Marlin is a real serial path (own connect with baud probing, "【marlin】正在尝试波特率：" @945204d); firmware parameter view is not supported for it: "暂时不支持该固件类型的固件参数查看" @945579c.
  - Unknown boards: "不支持的主板或者固件" @944266c (auto paper device) and "不支持的设备" @945686b.
- Internals: `ChiXieSerial.connect_device` / `connect_wireless_device(ip, port, firmware)` @9452e83, 9452ec3. Port auto-detection: description "USB-SERIAL CH340" and "GD32 Virtual Com Port" @9451c6e, 9451c80; Linux/macOS `/dev/` prefix @9452127.
- Access: free. Confidence: high.

## Device management for other devices

### Device management panel (设备管理, `DeviceManagement`)
- Where: left device panel when the main function is drawing/laser or when "切换炽写设备管理" is not active. Title bar label "设备管理" with a "添加设备" tool button and a "..." menu with "切换炽写设备管理", "命令窗口", "坐标系设置", "自定义命令", "固件参数", "参数导入/导出" (with "设备参数导入", "设备参数导出"), "更多设置" (with "抬笔高度设置", "写完位置设置") @9455088..9455650. A "定制标题栏" (custom title bar) docstring @9455709.
- What it does:
  - Two tabs (`tabWidget`, `tabWidget_2`): connection/control and parameter settings; sub-tabs for USB and wireless ("usb/无线 面板切换" @9458379).
  - Connection: port combo with refresh ("刷新端口号按钮点击" @9456605), connect button; "未选中端口，请选择端口后再连接" @945662f. Port priority defaults to CH340 first ("设置端口优先级，默认先搜索CH340端口" @9456509; a TODO says a later setting may choose CH340 vs Bluetooth). When the chosen port does not look like a device port: "此端口应该不是设备端口，连接可能会导致闪退，确认要尝试此端口吗？" with title "连接确认" @945667a.
  - Checkboxes: "自动连接机器" (`autoConnectCheckBox`, config `AutoConnect`) and "设为默认设备" (`setDefaultDeviceCheckBox`, config `DefaultDevice`) @9455cb0..9455e41.
  - Device name combo (`deviceNameComboBox`, `comboBox_DeviceName`), add/delete device buttons. Delete confirmation "设备参数删除确认" / "是否删除设备模板“" ... @9457067. Unsaved new device prompt: "未保存提示" / "您刚新建的设备参数信息未保存，是否保存？" @94570eb. New device name rules: "请输入设备名称", "设备名称和已有设备重复" @9456d32..9456d6c.
  - Device parameter form (see Device parameters feature) and "保存设备参数" -> "保存设备参数中" / "设备参数设置成功" @9457212..9457375.
  - Coordinate direction mapping validation: "坐标系中两个不同方向不能为同一个轴。（即不能同时为Y或者不能同时为X）" @945726d.
  - Pen-down label switches text by device type: "落笔高度" (stepper), "落笔角度" (servo), "落笔指令" (custom command) @9456de6..9456ef4.
  - Show/hide "命令窗口" (config `show_command_window`) and "坐标系设置" region (config `show_coordinate_region`) @9455ce9, 9455cfe.
  - Start/stop plotting buttons for drawing/laser: "启动绘制" / "结束绘制" with tooltips, plus "开始书写/继续书写/暂停书写" toggling for page writing @9454ea7, 9454ecb, 94589f0..9458a74; `switch_plotting_ctrl_pannel_status` swaps panel state while a job runs.
  - While writing, most widgets are disabled: "请先连接设备。如果已连接，则先停止书写任务" @94562ec.
- Internals: class `DeviceManagement` (`core\DeviceManagement\DeviceManagement.py` @945a413). Signals: `connect_success_signal`, `auto_connect_success`, `serial_error_signal`, `discover_new_device_signal`, `discover_done_signal`, `grbl_command_error_signal`. UI file `UI_DeviceManagement`. `on_ribbon_function_change` shows the plotting card only for `PlottingFunction`.
- Access: free (not tied to login). Confidence: high.

### Add device dialog (添加设备, `AddDeviceDialog`)
- Where: "添加设备" in the device panel or the ChiXie drawer.
- What it does: three buttons: `addChiXieN1Btn` (adds "我的炽写N1", the proprietary device with default params speed_mode=SlowSpeedMode, write_done_pos=BackOrigin, done_beep, direction_mode=Vertical_1), `addChiXieEsp32Btn` (adds `ChiXie_ESP32`), `addOtherDeviceBtn` (other/third-party device). Detail link buttons point at two Taobao shop items (`item.taobao.com ... id=841292135294` and `id=845440544113`) @945ed53, 945ed9b. If a ChiXie device already exists: "暂不支持添加多个炽写设备，需要请联系客服" @945ee73. On macOS `UserLimitUtil.limit_for_macos` hides the "other device" button (`addOtherDeviceBtn.setHidden`) @9456…/945eddc, so third-party devices are Windows-only.
- Internals: config `DeviceConfig/ChiXieDevices` (value list/dict of ChiXie devices), `DeviceControlPannelMode`, signal `AddDeviceSignal.add_device_signal` @945ee73..945f3d8.
- Access: free; macOS limited. Confidence: high.

### Device parameters: keys and `.cset` import/export
- Where: "参数导入/导出" menu; `ExportDeviceParametersDialog` ("设备参数导出").
- What it does: `DeviceParams` is a dict keyed by device name; each value is a dict with keys `firmware`, `deviceType`, `writeSpeed`, `pauseTime`, `penUp`, `penDown`, `x_plus`, `x_reduce`, `y_plus`, `y_reduce`, `WriteDonePos` @9455dd1..9455f22, matches the config sample. Axis mapping values are labels `X+`, `X-`, `Y+`, `Y-` (config shows `x_plus: "X+"`, etc.). Combos: `writeSpeedComboBox`, `pauseTimeComboBox` with items "0, 0.1, 0.2, 0.5, 1" @9454de3..9454def; move unit combo "0.01mm, 0.1mm, 1mm, 2mm, 5mm, 10mm, 20mm, 50mm, 100mm" @9454e4d..9454e7d. Import: file dialog "选择设备参数文件", filter "炽写设备参数文件 (*.cset)"; same-name devices get `-new` appended after a confirmation "本地存在同名设备：…继续导入会自动给新设备名添加`-new`字符标记。（点击取消则会取消导入）"; success: "设备参数导入成功，重启软件后即可加载" @9455863..9455a52. Export: choose devices (checkable list), pick folder ("选择保存目录"), file name "`<设备名> - 设备参数.cset`" or "`…的设备参数.cset`", filter "All Files (*);;炽写设备参数文件 (*.cset)"; errors "请先选择需要导出的设备", success "设备参数导出成功" @9477edd..947807d.
- Internals: the `.cset` file is a Python `pickle` (import uses `pickle.load` and `device_params_import` @9455907) of the `DeviceParams` dict; config store is QSettings INI under `DeviceConfig/DeviceParams` (QVariant map with PyQt_PyObject pickles, see config sample). `DefaultDevice` is the device name.
- Access: free. Confidence: high. Security note for the Rust port: importing `.cset` via pickle would execute code; in Rust implement as safe parser of this schema only.

### Pen height and write-done position quick settings
- Where: device panel "更多设置" menu. "抬笔高度设置" prompt "请输入抬笔高度/角度/指令" (stored as `penUp`, so for stepper it is height, servo angle, custom device a command) @9455c1a..9455c5a; "写完位置设置" prompt "请输入数字（0-原点、1-原点左侧2cm、2-任务结束点处）" stored as `WriteDonePos` @9455b79..9455bd7.
- Confidence: high.

### Custom command dialog (自定义命令)
- Where: device panel menu "自定义命令"; custom buttons appear in a "自定义区" grid (`gridLayout_CustomCommand`).
- What it does: left list of saved commands, right editor with button name and command text. Save validates "按钮名和按钮命令不能为空！" (title "操作失败") and "未选中命令条目，请先选中左侧某个条目"; delete asks "确认删除自定义命令”…“吗？" @94541b4..9454372. Pressing a custom button sends the command to the device (blocked while writing). Stored in config `DeviceConfig/CustomCommand` (a dict name->command).
- Internals: singleton `CustomCommandDialog` (@945466c); `DeviceManagement.add_custom_command_btn`, `delete_custom_command_btn`, `click_custom_command_btn`, `init_load_custom_command` @9455f58.
- Confidence: high.

### Manual controls on the other-device panel
- Where: connection tab: X+/X-/Y+/Y-, Z+/Z-, "设置原点", "回到原点", pen up/down, command line (`lineEdit`, send button).
- What it does: jog sends `G21G91 ` / `$J=G21G91 ` relative moves using `moveUnit` and the configured axis mapping @9457dfa..9457e10; tooltips such as "坐标位置处的Y+"; set origin sends `G92 X0 Y0 Z0` ("设置原点按钮点击" @9457ed0); go home sends `G90G0 X0 Y0` then `G0 X0 Y0 F<speed>` ("回到原点" tooltip "坐标处回原点"); pen up/down toggle ("点击将笔抬起至原点" / "点击将笔落下至设定值"); Z fine-tune uses `G0 Z<n>` and `G21G91 Z-` / `G0 Z-` ("Z轴微调+/- 按钮点击" @9457fd8). Command line: "请先输入内容后再点击发送按钮"; send echoes to the command window ("<<<< 指令执行异常" on error, "串口命令操作失败，命令：… ，报错：" @94573c7..9457400). Gcode sanity hint: "Gcode一般由字母和数字组成，您输入的指令可能有误" for `error: Expected command letter` @9457504.
- Confidence: high.

### Grbl error translation table
- Where: command window / error popups titled "设备报错提示"; unknown: "控制板返回了一些未知异常报错：" @9457d93.
- What it does: translates Grbl `error:N` codes to Chinese: expected command letter, bad number format, unsupported `$` command, negative value, homing not enabled, min step pulse > 3 us, EEPROM read failed (reset to default), `$` only when idle, locked (alarm/jog) with `$X` hint, soft limit requires homing, line too long, `$` exceeds max step rate, safety door, startup line exceeds EEPROM (Grbl-Mega), jog target exceeds travel, jog command has no `=` or forbidden G-code, unsupported G-code, multiple G-codes in one modal group, feed rate undefined, G-code needs integer, hard limit (re-home advised), target exceeds travel, reset while moving, homing failures (reset / door open / pulloff failed / limit not found within 1.5x max travel and 5x pulloff) @9457604..9457d06.
- Internals: `DeviceManagement.parse_grbl_error_info` (@9459bb1) fed by `grbl_command_error_signal`.
- Confidence: high.

## Connection and communication

### Serial communication layer (`ChiXieSerial`, `ChiXieSerialThread`)
- Where: invisible; used by every device feature. Two implementations: `core.DeviceCommunication.serial.serial_communication.ChiXieSerial` for the generic path and `chixie_serial_thread.ChiXieSerialThread` for the ChiXie device card (log prefixes `【chixie-serial-process】`).
- What it does:
  - Connect Grbl: probes baud rates, log "【grbl】正在尝试波特率:" then "【grbl】端口打开成功"; on failure "端口连接超时", "连接失败，建议更换usb口尝试", "Grbl连接失败，", "串口异常", "串口连接失败" @9451e80..9452027. Marlin: "【marlin】正在尝试波特率：". Telnet for wireless: "连接无线主板（使用telnet）" with ip, port, firmware @9452298; failure "无线设备连接失败" @945227c.
  - Send: single command (`send_command`), hex command (`send_hex_command`, "发送十六进制指令，目前只给炽写控制板用") @9452533, command send errors "GRBL串口指令发送失败：", "Marlin串口指令发送失败：", "无线设备指令发送失败：" @9452510, 945262f, 94525b4. `$Errors/Verbose` is used for verbose error output @9452379. Read timeout message "串口读取超时，超时时间：", "串口返回错误：", "串口不能发送None值" @9452338..9452823.
  - Task runner thread (`run_of_serial`, `run_of_telnet`): `setTask`, `startTask` (start/resume), `pauseTask`, `stopTask`, `hasTask`, `setEnableMultiColor` (pause for pen change when multicolour is on) using a `QWaitCondition`/`QMutex` pair with `isPause`/`isStop` flags @9451bf0..9451c1f. Empty task message: "书写任务为空，请手动选择任务条目后点击书写，或在页面右键书写尝试" @945288f.
  - ChiXie serial thread additionally exposes `write_target_page`, `task_page_index`, `task_text_count`, runs `_run_initial_gcode` / `_run_end_gcode` and logs "initial_gcode done" / "end_gcode done" @946c84e..946c8b3. Errors "指令执行超时", "设备端口未连接", "工作任务异常1-3" @946c381..946c79c.
  - `SimpleSendSerialCommandList` thread: "简单发送串口命令集（不带暂停）" (used by patrol/frame drawing) @9453581.
- Internals: Qt `QSerialPort`/`QSerialPortInfo` (vendorIdentifier, productIdentifier, `isBusy`); Python telnet for wireless. Signals in `chixie_serial_signal` (`serial_connected_signal`, `task_done_signal`, `task_error_signal`).
- Access: free. Confidence: high.

### Auto connect (`AutoConnectThread`)
- Where: checkbox "自动连接机器"; also at startup if config `AutoConnect=true`.
- What it does: if not connected and a CH340 port appears, connect automatically. Messages: "自动连接设备，自动获取端口号中", "检测到设备接入,连接中", "自动连接成功" @9453949..94539ab. Thread started at (1) software init (device management init with saved AutoConnect) and (2) when the user ticks the checkbox @9453c98..9453d1c.
- Confidence: high.

### CH340 driver check and install (`EventAfterMainWindowShowEvent`)
- Where: after the main window first shows.
- What it does: checks whether the CH340 driver is installed; prompt "检测到未安装CH340驱动，是否下载并安装？" with buttons "立即安装" and "不再提示"; downloads `CH341SER.EXE` from `<server>/default_res/driver/windows/CH341SER.EXE` with a "下载进度" dialog, error "驱动文件保存异常，请重启或者联系开发者" @9492e5f..9493142. Also checks local device parameter data (`_check_local_device_data`) and loads the writing clipboard.
- Access: free; Windows only. Confidence: high.

### Wired connect flow, ChiXie card
- Where: ChiXie control card connect button (`connectBtn`, toggled).
- What it does: searches candidate ports for the ChiXie board (GD32 virtual COM port; "检测获取串口是否有未连接的gd32虚拟串口设备") including busy ports: "未查询到炽写设备在线", "检测到存在忙碌设备，建议重新插拔设备尝试"; failure InfoBar "设备连接失败或者无有效设备，建议重新插拔或更换USB口"; on success the card sends `$HZ` (home Z) and shows "已连接". Disconnect/abnormal detection: "目标设备端口未开启，请重连设备", "ClearCommError failed", "设备异常断开，请重连设备。如需续写，可右键页面使用断点续写", "页书写任务异常，请重新连接设备。如需续写，可右键页面使用断点续写" @9469424..94696e4.
- Confidence: high.

### Grbl alarm unlock
- Where: after connect and also on main window `serial_alarm_tip`.
- What it does: "开始进行Grbl状态检查"; if the board status is Alarm: "Grbl控制板当前为“警报”状态，是否发送解锁指令？（解锁后才可正常使用）" with button "立即解锁" (sends `$X`) @9456bd9..9456c4d, 948c449..948c4bb. Non-ChiXie boards are verified with `$WHO`; unverified boards trigger "未经认证的设备，请谨慎使用！！可能即将停止支持！！" / "未经认证的设备，请谨慎使用，后续可能不再提供支持" @9456845..9456934.
- Confidence: high.

### Wireless board discovery
- Where: wireless sub-tab, "刷新网络设备按钮".
- What it does: `WirelessBoardDiscoveryThread` runs `arp -a` (decoded as gbk), takes each LAN IP, requests `http://<ip>/description.xml` and parses UPnP XML (`urn:schemas-upnp-org:device-1-0`, `upnp:device`) to show items like "[局域网] 模块类型:… 制造商:… 制造商URL:… 设备IP:…" @9458276..94582f2, 947c72d..947ca3f. The IP `192.168.0.1` appears as a special case (AP-mode board default; AP-mode instructions say to open `192.168.0.1` in a browser @9475fb6). Failures: "无线设备搜索，存在部分异常情况：".
- Internals: signals `discover_new_device_signal`, `discover_done_signal`; status enum `DiscoveryDone`/`InDiscovery`.
- Confidence: high.

### Add network device manually (`AddNetworkDeviceManuallyDialog`)
- Where: wireless sub-tab "+" button.
- What it does: fields IP, port, name, checkbox "使用本地默认设备名"; validation regex `^(\d{1,3}\.){3}\d{1,3}$`; errors "信息输入不完整", "IP地址格式输入错误，如果判断错误请联系开发者", "该设备名已存在，请重新输入"; success "网络设备添加成功" @945f76f..945f98e. Stored under `DeviceConfig/CustomNetworkDevices` as `{name: {ip, port}}` (matches config sample). List entries show "[自定义]" or "[局域网]" prefixes @94560aa, 9458136. Delete only user-added ones: "只可以删除用户自定义添加的无线设备"; "确认删除自定义无线设备'…" @9458143..9458207. Connect error "无线连接失败，请检查设备是否已正确配网并联网", "请先选中无线设备，再点击连接按钮" @9458464..94584a8.
- Confidence: high.

### Wireless ESP32 settings (`WirelessEsp32SettingDialog`)
- Where: wireless panel "配网" button (`pushButton_4`) for non-ChiXie ESP32 boards.
- What it does: reads the current radio mode via `$ESP110=` and scans WiFi networks / Bluetooth info (`$ESP140=` appears for the Bluetooth path; the exact scan command is not in the constants; signal icons `WIFIxinhao-ji/_1/_2/_3/-wu.svg` chosen by signal strength; an SSID list plus password is inferred); modes: "WIFI客户端模式" (STA), "蓝牙模式", "禁用无线(暂未开发)" @947cfe6..947d009. Result strings: "Connected with"→"配网成功(…)，请查看无线面板中设备列表", "WIFI客户端模式(STA模式)配网失败", "BT Started with"→"蓝牙模式设置成功，请在电脑中连接该蓝牙，刷新软件内串口，连接相应新增的端口即可。", "蓝牙模式设置异常", "Wifi模式获取失败", "Wifi扫描异常，或者相关参数获取异常" @947d14e..947d753. Progress label "（WIFI扫描中）", "设置中".
- Confidence: high.

## Firmware and board settings

### Grbl firmware parameter dialog (固件参数, `FirmwareParameterDialog`)
- Where: device panel menu "固件参数" ("打开固件参数窗口"). Singleton.
- What it does: two modes. Easy mode: sliders for speed and steps; slider value snaps to multiples of 1000 on release ("用户释放滑块后，将滑块移动到1000的倍数位置") with labels "有点慢" ... "超级快" @945b21f, 945b25d. Easy-mode save writes `$110= $111= $112=` (max rates), `$120= $121= $122=` (accelerations), `$100= $101= $102=` (steps/mm), `$1=` (stepper idle delay) and `$3=` (direction invert) @945df72..945dfb6. Expert mode: table with columns "参数名", "参数值", "参数描述" listing every Grbl `$` setting and for ESP32 Grbl the named settings (`$Sta/SSID`, `$Radio/Mode`, `$Stepper/Pulse`, `$Limits/Soft`, `$Homing/*`, `$X/StepsPerMm`, `$X/Current/Run`, ...). Fetch via `$$` (classic) or the ESP32 named-settings listing (`get_firmware_parameter_of_grbl` / `..._of_esp32_grbl`); parse errors: "固件解析失败", "参数值异常，解析失败，参数值", "固件参数获取失败，请重试！（1、请不要书写中打开此窗口。2、请检查固件类型是否正确）". Save failures say "简易模式，固件参数设置失败,请联系开发者解决，异常：" / "专业模式，…".
- "恢复默认值": confirmation "是否恢复Grbl固件到默认值？(新手小白慎点！执行此操作后，Grbl固件的所有设置、参数和校准数据都将被重置为默认值)", sends `$RST=*`, then "固件参数已经恢复为默认数值" / "固件参数恢复默认失败，" @945dde4..945df0b.
- Internals: full catalogue of descriptions at @945b3d0..945d4b9: classic numeric ids `$10 $11 $12 $13 $20 $21 $22 $23 $24 $25 $26 $27 $30 $31 $32 $100-$103 $110-$113 $120-$123 $130-$133`, plus ESP32 names. ChiXie-specific keys: `$X/ChiXie/HomeOrder=1` (home order 0-4 per axis), `$ChiXie/AxisFollow=0` (A axis follow: 0-A/1-X/2-Y/3-Z), `$ChiXie/ForceSetOrign=1` (homing sets origin), `$ChiXie/MachineMode=0` (0 = XY, 1 = CoreXY) @945c521..945c5ac. TMC driver per-axis keys (`Current/Run`, `Current/Hold`, `Microsteps`, `StallGuard`, labelled "高级驱动模式") also appear. Parsing helper strips non-numeric chars from values like `5000.000 (x max rate,mm/min)` @945dc22.
- Access: free. Confidence: high.

### ChiXie ESP32 firmware settings (`ChiXieESP32FirmwareSettingDialog`)
- Where: device panel "炽写ESP32主板专用设置" (`ChiXieESP32BoardSetting`); denied with "当前控制板不是炽写ESP32控制板，无法使用专属设置界面" or "控制板连接异常，请检查" @94586ba..9458729.
- What it does: three pages (segmented widget). Page 1: `$Stepper/IdleTime`, `$ChiXie/AxisFollow`, `$ChiXie/MachineMode`, `$Spindle/PWM/Frequency` (PWM button group plus a custom kHz radio; validation "自定义PWM频率不能小于等于0", "自定义PWM的值异常，请重新检查") and X/Y/Z/A steps-per-mm, max rate, acceleration (`$X/StepsPerMm`, `$X/MaxRate`, `$X/Acceleration`) @94750fa..947546e. Page 2/3: radio/WiFi: `$Radio/Mode`, `$AP/SSID`, `$Bluetooth/Name`. Radio mode combo: None / STA / AP / Bluetooth. Handlers: `set_for_station_mode` (STA: "配网成功(…)，请查看无线面板中设备列表" / "STA模式配网失败"), `set_for_ap_mode` ("AP模式配网成功，请用电脑连接控制板WIFI后，在浏览器打开192.168.0.1使用"), `set_for_bluetooth_mode` ("蓝牙模式设置成功，请在电脑中连接该蓝牙，刷新炽写内串口，连接相应新增的端口即可。"), `set_for_radio_none` ("无线模式关闭成功"). Validation "STA模式未选中无线网，或者未输入密码，请检查。（如果已经配置过sta模式，则无需重复配置）", "AP模式下网络名称和密码未输入完全…", "设备蓝牙名称未输入…", "查询WIFI信息失败，如需继续设置STA模式，建议先设置模式为第一项None，然后再切换至STA模式进行设置尝试" @947612c..947649e. "恢复默认值" asks "是否恢复ChiXie-ESP32控制板的固件到默认值？(新手小白慎点！…)" and sends `$RST=*`. Help button: "教程或者帮助请先加入炽写群联系群主或者群友，图文教程即将上线！" @9475c45. A TODO notes a cloud check "这个控制板id是否由本人持有使用".
- Confidence: high.

### ChiXie N1 advanced device settings (`ChiXieDeviceFirmwareSettingDialog`)
- Where: control card menu "高级设置" (`on_chixie_device_firmware_setting_triggered`); two pages (segmented widget).
- What it does: widget names reveal per-axis fields for X/Y/Z/A: steps per mm, max rate, acceleration, origin offset, home order, TMC driver IRun/IHold/Mres/driver mode, StallGuard threshold (`*SgthrsSpinBox`), plus `tcoolthrs`, `tpwmthrs`, `AAxisModeComboBox`, `deviceTypeCombobox`, `motionTypeComboBox`, `lockMotorSwitchButton`, `beepDisableSwitchButton`, `homingForceSettingOriginSwitchButton`. Firmware keys read/written: `$63 $61 $62 $65 $71 $72`, `$100-$103`, `$110-$113`, `$120-$123`, `$190-$193`, `$200-$203`, `$210-$213`, `$220-$223`, `$231-$233`, `$240-$243`, `$250-$253` (numeric TMC-style ids; meanings not in constants) @94603fa..9460873. "一键配置设备参数" (`quicklySettingParametersBtn`): confirmation "确认导入(<device>)设备的默认参数吗？" writes defaults `$1=255, $3=12, $24=1200, $25=1400, $27=1, $100..$103=100, $120..$123=3000, $63=1, $71=37`; "默认参数(…)写入异常", "默认参数写入执行完毕" @9460992..9460b07. Float display simplified (8000.000 -> 8000). Save: "保存成功".
- Access: probably free (no limit string). Confidence: high for flow, low for the meaning of the numeric ids.

### ChiXie firmware update (`ChiXieFirmwareUpdateDialog`)
- Where: control card top menu "固件更新".
- What it does: shows "设备信息:" parsed from `$WHO` ("UUID:" serial number label, current firmware version label, newest-version label with hyperlink). If not connected: "设备未连接" (update disabled). Start shows a guide dialog "更新指导": "请按住抬笔电机旁的解锁按钮，然后点击开始。(进度条开始后可以松手)" @946376f..94637be. Firmware kinds `AppFirmware` / `BootFirmware` exist in enum @94474f9, so there are separate application and bootloader images. The transfer protocol and file source are not in this slice (hex-command channel `send_hex_command` is the likely transport; inferred).
- Confidence: medium.

### ChiXie device test dialog (设备测试)
- Where: control card menu "设备测试"; gated: `UserRoleCode.super_user` only, otherwise "当前仅支持管理员使用此功能"; not connected: "设备未连接" @946a12d..946a1b6.
- What it does: three tests. Buzzer: sends `$Beep 843 843 840 843 840 841 843 840 845` @9462d15 (a melody). Polyline "锯齿折线": 45 degree zigzag, 50 mm long, 10 mm spacing, fast reversal, "测试重点：电机急停/启动时的丢步情况" with multi-speed list ("多速度测试"; `fontSpeedComboBox`), pen `G1 Z-1.5 F8000` @9462d90..9462e7e. "基础图形测试": draws basic shapes (G2 circle arcs, pen down/up) after `G21 G90 G92 X0 Y0 Z0`.
- Confidence: high.

## ChiXie device UI

### ChiXie device control drawer (炽写设备管理)
- Where: collapsible drawer panel (`ChiXieDeviceControlDrawerView`), shown when the main function is writing; header "炽写设备管理" with tool buttons "切换管理面板锁定状态", "隐藏设备管理", "添加设备", "切换通用控制" (switch to other-device panel), "设备购买" (opens the Taobao item) @9461ec3..9462146. A device-card combo picks among ChiXie devices (qfluentwidgetspro combo box). Drawer side comes from `DefaultHide` / `DefaultShowOnLeft` / `DefaultShowOnRight` and lock/unlock state (`Lock`/`UnLock`).
- Internals: `init_chixie_device_window`, `init_other_device_window`, `toggle_chixie_device_pannel_show` in `InitDeviceControlWindow` @9478a64..9479025; config `DeviceControlPannelMode`, `DeviceManagerStatus` (config sample). Writing management widget is the area under it.
- Confidence: high.

### ChiXie device control card (`ChiXieDeviceControlCard`, `ChiXieDeviceSettingView`)
- Where: card with device image "炽写N1透明背景1.png", title, outlined "连接" button (toggle), "..." menu, jog pad and a settings grid.
- What it does:
  - Menu items: "绘图/激光", "固件更新", "命令窗口", "用户指南", "删除设备", "设备测试" (super_user), "高级设置", "书写优化" @9468e22..9468f84.
  - Jog pad: up/down/left/right arrows (`G21G91 X/Y` relative with sign set by direction mode) and unit buttons 0.1, 1, 10, 20 mm (`unit01Btn`..`unit20Btn`; slider `SportUnitSlider` with tooltip, values "1mm 2mm 5mm 10mm 20mm 50mm 100mm 200mm" @9468072..9468095). Buttons: "回机械原点 $H" (home), "绝对抬笔" ("控制抬笔直至触发限位开关"; probably the `$HZ` home-Z command, inferred), "抬笔" (pen up `G1G90 Z0 F5000`, pen down `G1G90 Z<pen_down_value> F5000`), "设置原点" ("设置当前XY坐标为原点（书写起点）"; sends `G92 X0 Y0`; success "设置原点成功"; note "炽写页面中蓝色小圆圈即书写起点"), "回到原点（XY方向）" (`G90G0 X0 Y0 F5000`) @9468bab..9469b58.
  - Parameters (labels): "运动模式：" speed combo ("常规（推荐）" and more), "写完位置：" ("原点", "原点左2cm处", "任务结束点"), "写完提示音：" switch (beep `$Beep ...` at end), "方向模式：" ("竖向（默认）", "竖向(翻转)", "横向(翻转)", "横向"), "落笔轻重：" ("更轻Ⅱ", "更轻Ⅰ", "标准(默认)", "更重Ⅰ", "更重Ⅱ" -> five pen-down depths), "空驶抬笔：" switch (lift pen during travel moves), "自动原点：" switch @9468156..94683ec. Each change calls `set_param_*` then `_save_param_to_local_config` (config `DeviceConfig/ChiXieDevices`, keys `speed_mode`, `direction_mode`, `write_done_pos`, `pen_down_mode`, `auto_origin`, `done_beep`, `pen_up_move`).
  - Direction-mode changes jog axis mapping: `Vertical_1`/`Vertical_2`/`Horizontal_1`/`Horizontal_2` flip signs of X and Y moves (code swaps `G21G91 X-`/`Y-` @9469a48..9469a90) and the generators flip 180 degrees (`_need_to_flip_180`).
  - Connection errors/handling as in "Wired connect flow".
- Confidence: high.

## Page job management

### Page writing management view (`ChiXiePageWritingManagementView`)
- Where: area under the device card in the writing function ("炽写设备 - 写字区域视图"). A scrolling list of "page control groups": thumbnail (`WritingPagePreviewLabel`, selected border `2px solid #ABCDEF`), page number, progress bar, start and stop buttons ("绘制此页(", "停止书写此页(" prefixed texts) @9466155..9466217.
- What it does:
  - One group per page of the current document; selecting a thumbnail deselects the others and centres the real page view ("1、取消其他页面的预览状态 2、实际页面设置焦点（CenterOn）") @9466c04..9466c94.
  - Start/pause toggle: "开始书写" / "暂停书写" / "继续书写"; start sets a task (page items) and starts the serial thread; "停止书写此页" stops. Progress comes from `show_writing_progress` (page index, text count). On finish: `on_writing_done` and `set_writing_done_status` (also on abnormal disconnect).
  - Path optimisation before writing (the "writing improve" mode): "书写路径优化中" progress; slot `slot_func_of_start_improve_writing` starts the improve generator thread and `show_progress_of_improve` shows its progress @9466720..9466783.
  - Break-point resume: `continue_write_from_break_point` ("炽写设备断点续写"), failure text "断点续写异常，请将日志发给客服" @9466822..94668b4. The page right-click menu offers "断点续写" (mentioned in disconnect warnings: "如需续写，可右键页面使用断点续写"). Generator side: `get_gcode_when_continue_write`, `item_to_continue_write`, `can_continue_write` (resume from a chosen character; earlier items skipped).
  - Per-page "upload to remote SD and write" is also driven via the page context menu (`upload_to_sd_run_signal`, see SD feature).
  - Multi-page jobs: pages are written one at a time by user click; sheet change is manual unless the auto paper device is used. `change_pen_signal` supports pen change between colours (see multicolour).
- Internals: signals in `chixie_serial_signal` and `writing_function`; `ChiXiePageWritingManagementView.on_device_connected` flips the view when the device connects.
- Confidence: high.

### Plotting management view (`ChiXiePagePlottingManagementView`)
- Where: same layout used for the drawing/laser function: preview "写字功能缩略预览图" (`PlottingPagePreviewLabel`), controls "书写此页(", "停止书写此页(", pause/resume @94644ca..9464792.
- What it does: loads the thumbnail of the (single) plotting page ("暂时只考虑加载一页"), refreshes preview, `start_plotting` builds G-code with `ChiXiePlottingPageToGcode` and runs it. Paper settings come from `paper_type_setting_widget`. Finished/abort sets status.
- Confidence: high.

### Writing improve dialog (书写优化, `ChiXieWritingImproveDialog`)
- Where: control card menu "书写优化" (new, dated 2025-09-14 in the slice).
- What it does: combo `comboBox` with four modes: "无优化", "简化路径", "优化方案1", "优化方案2" mapped to `RouteImproveNone / Simplify / CatmullRom / Uniform`; a spin box value `RouteImproveValue` (strength / point count; stored under config `WriteConfig` keys `RouteImproveMode`, `RouteImproveValue`). Button applies the improvement to the currently selected text items (`NowSelectedText`): "应用至选中字符". Comments: "简化路径", "方案1的扩张" (Catmull-Rom with `num_points`), "方案2的扩张" (uniform spacing). Implementation: `FontSimplifyUtil.simplify_all_strokes`, `FontCharExpandUtil.expand_all_strokes_no_scipy(method='catmull_rom'|'uniform', num_points)` and `reset_text_path` @946d27f..946daa8.
- Internals: at write time, `GenerateGcodeOfWritingWithImprove` (QThread) wraps `ChiXiePageToGcode`: `get_body_gcode_with_improve`, `generate_of_simplify_route`, `generate_of_catmull_rom_expand`, `generate_of_uniform_expand`, `cal_improve_progress`, `calculate_page_char_count`; emits `improve_done_signal` and progress ("生成优化路径的Gcode，同时发送进度信号") @9474352..9474acc.
- Confidence: high. The meaning "denser path points make the motors run more smoothly" is inferred.

## G-code generation

### Writing G-code generator (`ChiXiePageToGcode`, ChiXie device)
- Where: invoked per page by the writing management view, SD upload, export and the auto paper device (`gcode_generator.get_gcode_of_automatic_paper_device`).
- What it does:
  - `get_gcode` returns `initial_gcode + body_gcode + end_gcode` as a triple. Initial: `G21`, `G90`, `G92 X0 Y0` style origin setup, pen up. End depends on `param_write_done_pos`: `BackOrigin` -> `G90G0 X0Y0`; `BackOriginLeft2` -> `G90G0 X-20Y0` (code builds `G90G0 ` + x_minus_value + `20Y0`); `BackNone` -> nothing; and if `param_done_beep`, appends `$Beep 843 843 840 843 840 841 843 840 845` @9472c19..9472c9a.
  - Speeds: `write_speed` / `travel_speed` come from `speed_dict` indexed by speed mode (`SlowSpeedMode, CommonSpeedMode, HighSpeedMode, BerserkMode, SlowSpeedMode1`). Pen up: `G1G90 Z0 F7000` (also `G0 Z0.75` / `G0 Z0` forms in the base class). Pen down: `G1 Z-<depth>` with `F...` from `param_pen_down_value` (five pen-weight levels).
  - Axis mapping by `param_direction_mode` (`Vertical_1/Horizontal_1/Vertical_2/Horizontal_2`): `x_plus_value`, `x_minus_value`, `y_minus_value`; `check_if_need_to_flip_180` and `_get_target_xy_value` apply 180-degree flip.
  - Origin: `StartPointOfPaper` (paper top-left), `StartPointOfFrame` (frame/margin start, uses `origin_point_of_frame`, `end_point_of_frame` with cos/sin rotation for flips), `StartPointOfCustom` (user-set origin item `origin_point_item`) @947308b..947397e; offsets by `get_offset_value`.
  - Content order via `_get_content_items_by_path_planning_type` using `PathPlanningType`: `PathPlanningZhi` (之, zig-zag: all rows left-to-right) vs `PathPlanningGong` (弓, boustrophedon: alternate rows reversed; `right_to_left` branch inserts reversed order). Config default `path_planning_type=2` and `start_point_mode=11` exist in `[WriteConfig]` (value mapping inferred).
  - Parity: reads `ParityPageDirectionMode` and `page_number`; `SameDirectionForParity` (all pages same orientation), `OddPagesRotate180`, `EvenPagesRotate180`, `SameDirection180` (every page rotated 180) so that when the same sheet is fed back to a bed-fed plotter the page orientation can alternate.
  - Pen-tip test ("笔锋简单测试", `enable_route_tip`): `route_tip_test_gcode` and `route_tip_last_z_value` modify the Z of the last points of a stroke (stroke_index, stroke_length) to simulate a brush tip by lifting the pen along the final segment.
  - Multicolour: when `MulticolorDrawing` is on, groups by item colour (`getRgb`, default `(0,0,0)`) and emits `change_pen` pauses between colours.
  - Cell/table layout: for note/table/composition templates walks `cell_list`/`TableParentCellItem` with `relative_offset`, `PXScale` and item positions to compute absolute mm coordinates.
  - Consecutive-sign fix: replaces `--` with `+` and `+-` with `-` (`_handle_consecutive_negative`, comment: "处理连续负号问题（导致N1鬼叫的问题、导致输出Gcode中包含Y--的问题）": a double minus used to make the N1 board beep/"ghost-cry").
  - Progress callbacks (`progress_call_back`, `text_route_handle_func`).
- Internals: `core.DeviceManagement.chixie_device_management.generate_gcode.generate_gcode_of_writing` (@94722e2), base `ChiXieGenerateGcodeBase` @946e1c8 (get_gcode, get_body_gcode, get_end_gcode, x_plus_value, ..., pen_up_gcode, pen_down_gcode, get_initial_gcode). `docs/protocol.md` already holds layout rules captured from real output; this inventory is consistent with it.
- Confidence: high for structure, medium for the zig-zag/boustrophedon semantics.

### Plotting (drawing) G-code generator (`ChiXiePlottingPageToGcode`)
- What it does: per-item generators for the drawing/laser canvas: line, rectangle, round (circle/ellipse), arc (circular and elliptic: `generate_arc_gcode` using G2/G3 with `ArcFitterUtil` / `ArcEllipticFitterUtil` and a feed in mm/min, precision digits), polyline, polygon, SVG path (segment types including "Line", Bezier via `plotting_bezier_fitter_util`), curve item (`PlottingCurveData`, bezier/circle fitting utilities), maze item, seal item, text box (text drawn through the writing font path), table cell item, and image scan (`plotting_image_scan_util`). Unknown items: "未知item的绘图Gcode生成：", unknown SVG path "未知svg path——222222". Each item checks `item_in_paper`. Output `G0`/`G1`. A try/except logs "generate_arc_gcode e:" .
- Internals: module `generate_gcode_of_plotting` (@946ec5f..9471062). The third-party variant `PlottingPageToGcodeForOtherDevice` is a "完全照搬" copy (@948588e: "下面的代码目前完全照搬炽写设备绘图Gcode生成代码") built on `GcodeGeneratorBase`.
- Confidence: high.

### Generic G-code base and other-device writing (`GcodeGeneratorBase`)
- What it does: for non-ChiXie devices builds base G-code: `G21`, `G90`, `G92 X0 Y<height>`-style origin (`aG92 X0 Y`), pen-up/pen-down per device type: stepper `G1G90 Z<penUp>`; servo `M3S<angle>`; electromagnet / laser / 3D printer / custom-Z-command use `G1 Z<value>` or the user's raw command string ("自定义抬落笔指令设备"); `write_speed` and `travel_speed` from `DeviceParams.writeSpeed`; `x_plus`/`x_reduce`/`y_plus`/`y_reduce` label mapping (`XPlus`, `XReduce`, `YReduce`) to signed coordinates; `pauseTime` as `G4 P<n>` after pen moves; end: `WriteDonePos` -> `G90G0 X0Y0` / `G90G0 X-20Y0` / none. `StartPointOfFrame`/`Custom` origin handled in `get_offset_value`.
- Confidence: medium (device-type-to-pen-code mapping order inferred from constant order).

### Task model in the Rust port: note
- `get_gcode` for all families returns three parts (initial, body, end); the serial threads send initial and end separately (`initial_gcode done`, `end_gcode done`), which supports pause/resume between lines.

### G-code for 3D printers (`GcodeGeneratorFor3DPrinter`) and Bambu A1 sound
- Where: used only by the Bambu Lab assistant.
- What it does: `get_page_gcode(page_index, offset_x, offset_y, auto_origin_xy, pen_up, pen_down, enable_sound, device_type)` produces one gcode text per page ("page_<n>.gcode"). Body: moves `G1 X.. Y.. F<writeSpeed>`, pen down `G1 Z<pen_down>`, pen up `G1 Z<pen_up>`, `G4 P<pauseTime>` after pen moves; end: `G90G0 X0Y0` and `M18 X Y Z` (motors off). Optional "提示音" for Bambu A1 series: `M400 S1`, `M1006 S1` ... `M1006 W` sequence (about 18 `M1006` tone lines forming a short melody) @9483a4d..9483d5c. Supported types in the UI: "拓竹A1", "拓竹A1-Mini", "拓竹P1S(输出可能有问题)", "拓竹X1(输出可能有问题)", "其他I3（如创想三维Ender-3）" (TODO: per-model parameters). Origin mode "自动XY原点" (auto XY origin) or `None`.
- Confidence: high.

## Treasure box (百宝箱) tools

### Treasure box dialog (`TreasureBoxDialog`)
- Where: ribbon action `treasureBoxAction` (主窗口 on_treasureBoxAction_triggered). Buttons: `bambulabWritingBtn` (拓竹写字助手), `pluginManagementBtn` (插件管理), `autoPaperDeviceBtn` (自动换纸设备控制), `oneStrokeDrawingBtn` (一笔画绘制), `gcodeToolBtn` (Gcode工具), `remoteTFManagementBtn` (远程TF卡文件管理), `optimizeFontDataBtn` (字体数据优化) @949efc5..949f87f.
- Confidence: high.

### Bambu Lab assistant (拓竹助手, `BambuLabAssistantDialog`)
- Where: treasure box "拓竹写字助手". Help text "使用方法：1、搜索或输入局域网内设备IP，输入访问码进行连接设备 2、配置抬笔落笔位置（推荐抬笔7，落笔2）。推荐使用自动XY原点模式，开启后则自动设置笔尖位置为XY书写原点 3、选择页面，点击上传。前往设备屏幕上操作回中，（如果是自动XY原点，则先控制笔尖移动到目标原点）然后启动文件" @949b29d..949b38f.
- What it does: device combo (IP, password/access code), "自动搜索" scans the local /24 subnet for hosts with port 990 open (thread pool; "开始扫描 x.x.x.x 的端口...", "发现开放990端口的IP:", "未发现拓竹设备，请检查", "共发现N台设备"). Connect: implicit FTPS (`FTP_TLS`) with TLS 1.2, cipher `ECDHE-RSA-AES256-GCM-SHA384`, no hostname/cert verification except an optional fingerprint pinning check ("证书指纹不匹配！预期: …，实际: …" via sha256 of the peer cert), user `bblp`, `PASS <access code>`; log lines "服务器欢迎消息" (220), USER 331, PASS 230, "登录成功！", "设备连接成功" @949b612..949b8c6. Folder list via `LIST`, file view with icons; "上传": uploads the generated page G-code with `STOR` (`custom_storbinary`, expects 226) -> "页数据上传成功" / "上传异常，". "导出Gcode-请选择文件夹" saves the `.gcode` instead. Refresh button works; delete: "暂未开发". Limitation: "暂时只支持笔记模式下导出" (note template only; TODO for table and composition). Config section `BambuLabConfig`: `DeviceIP, DevicePassword, PenUp (8), PenDown (4), EnableSound (1), XOffset, YOffset, OriginMode, DeviceType` (config sample). The access code is stored in plain config; the Rust port should avoid logging it.
- Access: free. Confidence: high.

### Auto paper device control (自动换纸设备, `AutomaticPaperDeviceDialog`)
- Where: treasure box "自动换纸设备控制" (dated 2023-10, an older add-on). Singleton dialog.
- What it does: a table of page tasks ("书写状态" column with "未书写", "书写中", "已书写", "暂停中") built from the current document pages via "导入页面任务" (page number, text count; `WritingTaskModel`/`TaskModelOfAutomaticPaperDevice`), buttons "进纸" / "退纸" (feed paper in/out), start / pause / continue / stop, spin box for loop count. On serial connect it sends `M119` and reads `z_max` = `TRIGGERED`; if the limit switch is blocked: "检测到有物体遮挡限位开关，请检查后操作" and "纸张限位开关状态检测异常". The firmware check compares against `Marlin` (meant for Marlin boards; otherwise error "不支持的主板或者固件"; inferred). Feed/spool run in `GivingAndSpoolingPaperThread` with timers: sequences such as `G92 Z0`, `G1 Z500 F9000`, `G92 Z0 E0`, `G1 Z113.84615 E113.84615 F8000`, `G1 Z100 F8000`, `G28 Z0`, `G1 Z-5 F5000`, `G1 Z600 F8000` @9443bf0..9443d2f. Statuses: "进纸中", "退纸中", "进纸完成", "退纸完成". It then runs the writing task for each page: loop "任务循环"; after a page finishes it spools, feeds the next. Close guard: "确认关闭自动换纸设备控制窗口嘛？" and "设备非停止状态下，无法关闭自动换纸控制窗口！". The generator used is `gcode_generator.get_gcode_of_automatic_paper_device` (old `core.WritingManagement.GenerateGcode`) with multicolour flag `setEnableMultiColor`.
- Access: free. Confidence: high for flow, medium for hardware identity (sheet feeder on the Z/E axes of a Marlin board).

### One-stroke drawing (一笔画, `OneStrokeDrawingDialog`)
- Where: treasure box "一笔画绘制" (dated 2024-12-21).
- What it does: text box of coordinate points (default saved locally as `one_stroke_test.txt`), "保存" ("保存成功"), "执行" runs one continuous pen-down path through the points. Validation: "请输入坐标点内容", "坐标点输入异常", "设备未连接", "请先结束书写", "炽写设备未连接". ChiXie variant uses `G1G90 Z0 F7000`, `G1 Z-1.5 F7000`; generic variant uses `G90G0 X0Y0`, `G0`/`G1`.
- Confidence: high.

### G-code tool (Gcode工具, `GcodeToolDialog` + `WebBridge`)
- Where: treasure box. A QWebEngine page `https://chixie.com.cn/client_api/gcode_page/gcode_tool_page` with a Qt WebChannel (`WebBridge`: `receiveFromJs`, `processData`, `receiveGcodeTaskStart`, `send_message_to_js`). The page's "start" action calls `start_gcode_task_from_js`, which builds a `Task` (initial/body/end G-code) and runs it on the ChiXie writing management; refuses with "设备未连接", "请先结束书写", "炽写设备未连接"; window follows the main window (`adjust_to_mainwindow`) @949e0ca..949ed27.
- Access: online (server page). Confidence: high.

### Remote TF card (SD) file management (`SDCardFileManagementDialog`)
- Where: treasure box "远程TF卡文件管理" and the page right-click "上传到远程SD卡并书写".
- What it does: for ChiXie ESP32 boards only: "当前固件不支持查看远程SD卡文件". Detects connection (wired or wireless), board identity and SD presence via HTTP `GET http://<board-ip>/command?commandText=[ESP200]&PAGEID=0` (`SD card detected` or "未检测到SD卡接入", "通信异常，建议控制板重新通电、重新连接后尝试"). Board IP via serial or from the network device list ("未成功获取控制板IP，部分功能无法使用"). File list via `[ESP210]` or the `/upload` endpoint; folder navigation with a "返回..." entry (`higher_level.svg`), double-click descends. Operations: delete file/dir (`/upload?path=..&action=delete|deletedir&filename=..`, confirm "确认删除所选文件(夹)吗？"), create folder (`action=createdir&filename=`, "新建文件夹", "请输入文件夹名称", "文件夹名未输入", "文件夹创建成功"), upload local file (filter "自定义文件(*.g *.gco *.gcode *.nc *.ngc *.ncg *.tap *.txt)", "开始上传"), run file via `[ESP220]` (`/command?commandText=[ESP220]/<path>`), pause `!`, resume `~`, status `?`, unlock `$X`, stop `%18` (0x18 soft reset) sent through HTTP `command` endpoint or serial; a timer monitors status: "执行中(", "暂停中(", "任务执行完毕". Default task folder `/chixie_task` is auto-created. WebSocket (`ws://`) is used too (`ws_url`, `base_path`).
- Confidence: high.

### Upload page to SD (`UploadFileToSD`)
- What it does: QThread posting the page's G-code (`<path>/chixie_task/…`) or a local file to `http://<ip>/upload` (multipart with `path`), results "上传成功"/"上传失败"; after upload asks "是否立即开始书写页面：" @947c05c..947c1b4. Prompts "开始上传页面".
- Confidence: high.

### Plugin system (`PluginEditDialog`)
- Where: treasure box "插件管理".
- What it does: an in-app Python code editor (`CodeEdit`, language combo `HighlightLanguage`, PYTHON by default) editing `<ChixieUserDirPath>/plugin/test.py` (path joined with `plugin_folder`), "启用插件" toggle (`enable_plugin`, labels "已启用"/"已关闭"), "保存" ("保存成功"), and `test_plugin_load` imports the file with `importlib.util.spec_from_file_location` and calls a sample class (`Dancer.dance`) @9496a0e..94971f1. This means the real app runs user Python code in-process; a Rust replacement would need a different extension mechanism (or drop it). Where the enabled plugin is invoked is not in this slice.
- Access: free. Confidence: medium.

### Font data optimize (字体数据优化)
- Where: treasure box. Dated 2025-12-15.
- What it does: font combobox filled from the user's fonts (`init_font_data_combobox`, "请检查网络连接"), buttons "统一所有字体字符尺寸" (`unifyAllFontSizeBtn`) and "统一字体字符尺寸" (`unifyFontCharSizeBtn`); finish "字体字符尺寸统一完毕". Per char: computes bbox (`cal_char_bbox` xMin yMin xMax yMax), parses a standard bbox set from the server (`parse_standard_bbox_data`), then scales and moves glyph strokes into the target box, default size 10 (`bbox_adjust_to_target_size`). Uses Fernet to decrypt font data (a hard-coded Fernet key is present @949d5bb; not reproduced here).
- Confidence: medium.

### Workspace patrol and frame drawing (工作区巡边 / 绘制框架线)
- Where: dialog `WorkspacePatrolAndDrawingDialog` ("DrawingAndPatrolOfWorkspaceDialog"), launched from the origin/alignment features (not directly in the treasure box list).
- What it does: "开始巡边" ("是否开始巡边？（请确保已经设置过原点）") traces the bounding rectangle of either the frame region or the actual writing area with the pen up so the user can check placement (error "没有书写内容，无法巡边"); "开始绘制" ("是否开始绘制页面框架线？（请确保已经设置过原点）") draws the template frame: note lines (normal, double-line, multi-column notes), table cell borders (`table_drawing_line_generator`), composition grid. Not available when busy: "当前有书写任务，请先结束", "炽写设备未连接", "[第三方设备]请先结束书写任务", "第三方设备未连接". Sends the route via `SimpleSendSerialCommandList` (ChiXie variant uses a limit-switch pen-up TODO). Patrol range ignores 180-degree flip and origin for the frame, but includes them for the writing area @94a0242..94a1b20.
- Confidence: high.

### Export G-code (导出)
- Where: ribbon `exportAction` (`on_exportAction_triggered`), button visibility `set_export_gcode_btn_visible` (hidden for ChiXie-only flows).
- What it does: asks folder ("导出Gcode-请选择文件夹"), writes one `<n>页.gcode` per page (`initial_gcode + body + end`), reports "Gcode导出成功，共导出 N 页数据" ("导出成功"), errors "页面没有数据，请输入内容或导入文件后再导出！", "导出Gcode出了一些问题", "导出Gcode失败:", "导出Gcode失败，有问题请联系开发者" @948c9a7..948cc2c.
- Confidence: high.

## Document and templates

### Save/open ChiXie document (`.cdoc`, `SaveAndOpenContent`)
- Where: ribbon actions `openChiXieFileAction` ("打开炽写文档", filter "炽写文档 (*.cdoc)") and `saveChiXieFileAction` ("保存为炽写文档").
- What it does: saves page content so it can be edited again: items (text, space, line break) with font ids plus the template used. Loading steps with progress texts: "准备加载炽写文档", "移除原有页面", "加载文档模板", "加载文档页面", "加载文档内容", "加载字符数据", "内容加载完成", "加载完成"; failures "炽写文档加载失败:" and "炽写文档加载失败，请联系客服", save failure "炽写文档保存失败：". If the document's template is not local: "当前炽写文档对应的模板未在本地保存，是否保存该模板？" (with "不保存"); same-name template: "本地存在同名模板，是否进行覆盖？", "模板保存成功". Loaders per template type: note / composition / table. Unknown item: "未知炽写文档中doc_item类型：". 
- Internals: encryption with `cryptography.fernet` (key literal @9498dbd; not copied); `ChiXieDocument`/`ChiXieDocItem` data model, conversion `item_to_ChiXieDocItem` and `ChiXieDocItemToItem` (TextItem/SpaceItem/LineBreakItem); MIME `application/chixie-doc-items` @944e086 for clipboard copy/paste of items; background `QThread`.
- Confidence: high for the flow; format details low (serialisation unknown: likely pickle then Fernet).

### Data models (DataModel)
- `ChiXieDocument`, `ChiXieDocItem`: document and one element. `CompositionTemplate` "作文格模板 数据模型, 数据单位： 毫米" @944e702. `NoteTemplate` "笔记模板 数据模型, 数据单位：毫米" with `set_column_space`, `get_width_of_column`, `get_horizontal_pos_of_column` (multi-column notes) @944f6de..944fa77. `TableTemplateV2`: `TableCellInTemplate`, `TableItemInTemplate`, `PageItemInTemplate`, `Table_Template`. `FontData` (module `FontInfo`): font info cache with `get_text_route(font_id, text, deformation)`, `get_randomly_font_id`, `get_font_name_by_id`, `has_font_info`, `has_font_route`, `add_font_info`, `add_font_route`, `add_font_route_after_deformation`, `add_text_path_to_font`, `font_has_text`, `find_text_in_collection` (search the user's collected fonts for a character, with `max_search_count`), `get_font_default_size`, `set_font_default_size`, `clear_font_info`. `PlottingCurveSegment`/`PlottingCurveData` (curve segments), `PlottingLayers` + `LayerAttr` (`add_item_to_layer`, layers feature of the drawing board, dated 2025-04-12). `Task` model (initial/body/end G-code, page index, status), `WritingTaskModel` (Qt table model: status "未书写/书写中/已书写/暂停中", `get_page_number_info`, `get_total_words_info`, `get_task_status_info`).
- Confidence: high.

## Main window shell

### Main window structure (`MainWindow`, SARibbonMainWindow)
- Where: the whole app frame, Qt `SARibbonMainWindow` with qfluentwidgetspro widgets. Functions (ribbon categories): writing (`WritingFunction`), drawing/laser (`PlottingFunction`), template management (`TemplateFunction`); `NowFunction`, `Default_Function` config.
- What it does:
  - Mode switch between "绘图模式" (`DrawingBoard.svg`) and "激光模式" (`Laser.svg`), tooltip "切换绘图模式或者切换激光模式", stored as `DrawConfig/DrawOrLaser` (`Draw`/`Laser`); `onSwitchDrawingOrLaser`. "显示主页" shows the home page.
  - Writing function actions found: insert menu (`insertBtn` -> `InsertBtnMenu`), writing file menu (`WritingFileBtnMenu`), writing template management ("书写模板按钮点击", "书写模板管理按钮点击"), batch overflow handling ("批量溢出处理"), combine composition content ("作文格模式的合并内容", `CombineCompositionContent`), search and replace ("写字功能-搜索与替换"), global refresh ("写字功能-全局刷新"), character fine-tune ("字符微调", `CharacterTuningDialog`), export G-code, open/save ChiXie file, font library, record font, treasure box, connect button, single/double page view (`single_page_btn`, `two_page_btn`, config `PreviewMode`), "在线客服" (online support), login button, AI button (`chixie_ai_btn`, tooltip "智能助手 - chatGpt" though it is the Spark model).
  - Template/context categories: table function panel and composition function panel appear contextually (`SARibbonContextCategory`, `table_func_pannel`, `composition_func_pannel`); alignment region enable/disable (`set_enable_of_writing_align_region`; align left/centre/right/justify/top/vcenter/bottom).
  - Status bar widgets per mode ("设置切换状态的时候，状态栏显示的部件"): page-number label "第 N 页，共 M 页", word count, scale label at lower right (`setPageScaleNum`), device position label (`device_pos_label`, refreshed by `device_pos_query_timer`, shows "已连接"/"未连接"), font name label.
  - Font selector for the writing function: `fontSelectionBox`, `font_size_combobox`; shows "所选内容中字体不统一" when mixed; `set_font_name_signal`.
  - Keyboard: key press/release overrides; shortcuts bound in `ChiXieInit.__init_shortcut_signal`; keypad navigation signals Left/Right/Up/Down, "SelectPreviousChar/NextChar/PreviousLine/NextLine", copy/cut/paste/undo via custom internal clipboard (`writing_clipboard`) and system clipboard (`pyperclip`, pickle of items) @948f6de..9490256. (Exact shortcut keys are not in constants.)
  - Cursor: text cursor blink timer (`CursorBlinkInterval`, `reset_cursor_blink`) and `CXCursorItem`.
  - Template change signals and loading (`load_writing_template`: dropdown text, icon, value, then load into canvas; `NowWritingTempName/Type`), `before_add_paper`/`after_add_paper` (controls are disabled while pages are added), `smart_nesting.svg` icon.
  - Warning marks: overflowing cells get a `warning_red.svg` badge (`set_warning_btn_of_cell`, "存在一些标点符号位置异常" `warning_red1.svg` with "点击处理": char position rationality detection).
  - Progress dialog "后台任务(请勿操作)" used for CAD parse, document loading etc.
  - Tips: `success_tip`, `warning_tip`, `error_tip`, `custom_tip`, `file_save_success_tip` ("打开目录" button), `systemPrompt` (tray balloon). Shop ad strings exist: "派曦科技", "本周店铺所有配件8折！", "前往购买" @948cee0..948cf19 (a promo tip, likely server-driven).
  - Global exception hook `my_excepthook` shows "异常提示" and logs file/function/line from the traceback (matches `log_sanitized.txt` file:line style).
- Confidence: high for named functions, low for exact ribbon layout.

### Pen change prompt and alarm slots
- Where: main window slots `change_pen_slot_for_chixie_device`, `change_pen_slot`, `serial_error_slot`, `serial_alarm_tip`.
- What it does: with multicolour on, when the task reaches a colour change: dialog titled "换笔提示" with text "您已启用多色绘制，请更换笔颜色为：<span style="color: rgb(r,g,b)">…" and buttons including "换笔完成" (resume) @948c295..948c34f; includes the page index. Serial errors: "检测到异常": "与设备通信发生异常，建议断开设备后重新连接 (如有需要可使用右键的“断点续写”功能)" @948c1b3..948c205.
- Confidence: high.

### Multicolour drawing
- Config: `[MulticolorDrawing] EnableMulticolorDrawing=false`, `PenChangingMode=1` (1 probably `ManualPenChange`; `AutomaticPenChange` exists in the enum, no UI in this slice). Serial thread `setEnableMultiColor(bool)` @9452aa3; page generator groups strokes/items by `item_color` and pauses at colour changes. Confidence: medium (automatic mode implementation unseen).

### Parity page direction
- Config `[ParityPageConfig] ParityPageDirectionMode=0`; enum values 0..3 map to `SameDirectionForParity`, `OddPagesRotate180`, `EvenPagesRotate180`, `SameDirection180` (order inferred from enum declaration order @9447394..94473d3). Generator rotates the page 180 degrees via `check_if_need_to_flip_180(paper)` using `page_number`.
- Confidence: medium for numeric mapping.

### Tray, heartbeat, screen change, close
- Tray: `InitCxSystemTray` / `ucore.SystemTray.init_system_tray` (tray icon with balloon prompt); close-to-tray was drafted but left as a commented TODO ("TODO 后续这里要改成自定义窗口，用于提示是直接关闭还是最小化到系统托盘") @948eaa6.
- Heartbeat: `_init_heartbeat_timer`, `_heartbeat` posts to `user/heartbeat` (`HeartbeatInterval`, `HeartBeatUrl`) while logged in; `OnlineDetectionFailedCount` tracks failures; `Logged` flag.
- Device position: `_init_device_pos_query_timer` (`DevicePosQueryInterval`) queries coordinates; `refresh_device_pos_value` updates the status bar; laser mode special-cased (`LaserEngravingMachine`).
- Screen change: "添加屏幕分辨率检测": "检测到屏幕分辨率发生改变，是否重启软件以获得最佳使用体验" with `restart_software`; failure "重启失败 / 自动重启失败，请手动重启软件".
- Close (`CloseChiXie`): confirm "关闭确认" / "确认要退出炽写吗？"; accepts: destroys tray icon, `collect_logout_info` (`user/collect_logout_info`), `collect_missing_characters` (`user/collect_missing_characters`: reports the missing characters to the server), logs out of the server ("退出服务器登录失败，原因：", "实际上是同步文档"), remembers window position and size (config `Window_Size`, `Window_IsMaximized`) @9491df2..94926ff.
- Confidence: high.

### Initialisation (`ChiXieInit`) and login flow
- Steps: tray icon, shortcut signals, read config, restore window pos/size, user font info ("初始化用户字体信息：收藏的字体、录制的字体", used for the font dropdown), auto login ("自动登录", "自动登录成功", "自动登录失败", "登陆失败", "自动登陆失败，请检查网络连接", "自动登录账号"), writing and drawing function init, cursor timer @9493a52..9495335. A TODO says: when local config fails to load, fetch platform defaults from the cloud (and the user's own config if any) (`config/query_exclusive_config_file`, `config/download_exclusive_config_file`).
- Login success (`LoginSuccess` thread): "同步账号收藏字体信息", "字体下拉框准备中", "收藏字体预解析中" (fonts are downloaded and parsed with progress tooltips; "加载字体中，请等待加载完毕"); super user title "超级管理员"; `IsVip`, `UserRole`, `UserName` fields; hides/shows recorded-font and daily-free-font regions per login/VIP state (`set_recorded_font_region_hidden`, `set_daily_free_font_region_hidden`); loads table data sources (`load_table_data_source`).
- Gate messages: "请登录后使用！" (AI button), "请等待字体加载完成后使用！" when fonts still loading.
- Confidence: high.

### Daily free font, update log, record font guide, user guide
- Daily free font (`DailyFreeFontDialog`, "每日试用字体 - 单例"): shows "今日试用字体" fetched from `font/get_daily_free_font` (`GetDailyFreeFontUrl`), preview images downloaded. Non-VIP accounts can only use this font.
- Update log (`UpdateLogDialog`, "更新日志窗口 - 单例"): reads `update_log.txt` (written by auto-upgrade; `JustUpdated`, `Major_Version`), shown after an update and deleted ("删除更新日志失败，"); button `commandLinkButton` opens the web page `page/update-log` ("打开更新日志按钮").
- Record font guide (`RecordFontGuideDialog`, UI "FontMiniProgramEntranceGuide"): the main window's "录制字体" action only opens a WeChat mini-program entry guide (not in-app recording).
- User guide and service (`UserGuideAndServiceDialog`, title "炽写用户指南与售后"): QWebEngine at `https://chixie.com.cn/client_api/user_guide_service/user-guide-and-service-page`.
- Confidence: high.

## Home page

### Home page (主页, `HomePageWidget`)
- Where: full-window overlay (stacked) opened by "显示主页"; left rail buttons: "字体库" (`font_region_btn`), "模板库" (`template_region_btn`), 反馈 (`feedback_region_btn`, opens a Tencent Docs form `https://docs.qq.com/sheet/DSHBwbGFuSnlHWmtS?tab=BB08J2`), 设置 (`setting_region_btn`, `SettingDialog`), 账号 (`account_region_btn`; if not logged in: "请先回到主窗口，点击右上角的未登录按钮进行登录"), and a back button (saves each widget's config on return: "返回时候通知各个组件保存配置") @9489d11..948a5f0.
- What it does: `FontWidget` hosts the online font browser; `TemplateWidget` is a placeholder ("主页 - 模板部分") shell. Singleton, resized to the parent.
- Confidence: high.

### Font widget and font card (`FontWidget`, `FontCard`)
- Where: home page font area: tag filter checkboxes (config `FontConfig/Tag_Filter=1, 2, ...`), sort rule radio (config `Sort_Rule`), left list (`listWidget`) with main directions "在线字体", "我的收藏", "我的录制" ("主方向变化需要重新获取数据：在线字体、我的收藏、我的录制"), search button, paginator (previous/next, jump box: "页码不能超过总页码！"), cards.
- What it does:
  - Fetches cards via `font/get_online_fonts`, `font/get_collected_fonts`, `font/get_recorded_fonts`, previews downloaded as images ("未查询到相关数据", "请检查网络连接"). "我的收藏" and "我的录制" need login: "请登录后查看”我的收藏“和”我的录制“！".
  - Card actions: collect/uncollect toggle (`font/collect_a_font`); "取消收藏" refuses for a default font: "默认字体不能取消收藏，如需取消收藏，请先更换默认字体". Limits: "当前版本客户端软件最多只支持收藏30种字体"; "普通账号无法收藏字体喔！（只能使用今日试用字体）"; "请登陆后再使用".
  - Card context menu: "设为“默认字体”", "设为“默认符号字体”", "设为“默认英文字体”", "设为“默认数字字体”" (labels differ: "设为主线字体", "设为主符号字体", "设为主英文字体", "设为主数字字体"). Only VIP can set defaults: "普通账号或者未登录无法指定默认字体喔！", "验证用户身份，只有会员用户可以设置主线字体"; must be collected first: "请先收藏该字体后再设置". Config keys: `MainFont`, `MainSymbolFont`, `MainEnglishFont`, `MainNumberFont` (config sample; the "main" slots show in labels on the page: `set_main_font_label` etc.).
  - Font permission check is a TODO (`check_font_permission`).
- Confidence: high.

### Font library web dialog (`FontLibraryViewDialog`)
- Where: ribbon `fontLibraryAction`. QWebEngine loading `https://chixie.com.cn/client_api/font_page/font_library_home_page?token=<login token>` (token in URL; do not copy) with injected CSS (Font Awesome fixes, custom checkboxes, filter panel) "注入CSS修复QWebEngine布局问题" @94813b0..9481417. On close, `EventAfterCloseFontLibrary` (thread) downloads newly collected fonts with progress (`download_font_progress`).
- Confidence: high.

### Missing characters and auto font replacement
- When a chosen font lacks characters, the app picks another collected font containing the character automatically (`find_text_in_collection`, `auto_choice_font_signal`) and shows: "…体，在(所选)内容中，有 N 个字符缺失。已用其他收藏字体替换" with a "查看缺失字符" button (`MissingCharDisplayDialog`, `show_missing_of_text`) @948bb4a..948bc5b. Missing characters are also reported at close.
- Confidence: high.

## AI, upgrade, paper

### AI assistant (炽写 AI "小炽", iFlytek Spark)
- Where: ribbon AI button (requires login: "请登录后使用！"), `AIInputWidget` floating input and `AIFunctionMenu` popup when text is selected.
- What it does:
  - Menu "编辑/评审选区": "内容润色", "内容简写", "内容扩写", "风格修改" (专业化/通俗化/直接化/间接化/亲切化); "从选区生成": "内容续写", "提炼总结", "内容翻译" (中文/英文/日文), "内容释义", "行动列表". Menu "用AI起草": "教学材料" (教学大纲, 教学计划, 课堂讲稿, 练习题), "邮件信函" (邀请函, 感谢信, 道歉信, 辞职信, 保证书), "工作总结", "头脑风暴", "学习笔记", "课设报告" (full text or 项目目标/需求分析/系统设计/实施过程/结果分析/总结), "实验报告" (full, 实验目的/原理/过程/数据记录/结果/分析), "实习报告" @9449055..9449798. Prompt templates in Chinese, e.g. "帮我润色如下内容：", "把如下内容翻译为英文：", "生成一份教学大纲，" @94478c8..9447fc5.
  - Input widget states: initial, custom question generating, function generating, generated; buttons send, stop, rewrite ("重写"), close; streaming output via `ai_streaming_response`; result inserted as typeset items (`InputTypeSetting`, `type_setting_util`).
  - Identity guard: questions like "你是谁" / "你是chatgpt吗" are answered locally: "我是炽写研发的AI小助手，我的名字叫小炽，我可以帮您完成多种文档的处理与内容生成等。有需要可以随时呼唤我哦！" @944c5fd..944c73c. A `check_content` filter rejects some content ("检查内容是否符合要求") and replaces brand words (科大讯飞 / 讯飞星火认知大模型 -> 炽写星火认知大模型, 炽写炽写, 炽写讯飞).
- Internals: `CallXunFeiAI(QThread)` over websocket to `wss://spark-api.xf-yun.com/v1.1/chat` (Spark v1.1) with an HMAC-SHA256 signed URL built by `Ws_Param.create_url` (`authorization` string `algorithm="hmac-sha256", headers="host date request-line", signature=`); credentials from `ai/get_ai_account` (`_init_xunfei_account`) @944c3a5..944cf8. Callbacks on_open/on_message/on_error/on_close, `stop_generate`. No local model.
- Access: login required; no VIP check visible in this slice. Confidence: high.

### Auto upgrade (`AutomaticUpgrade`, `DownloadUpgradeFileThread`)
- Where: after login/start; backend `update/check_update`, `update/compare_file_differences`.
- What it does: `check_upgrade` ("检测是否有文件更新"), compare file lists by MD5 (`_get_md5_of_file`, `compare_file_differences` and `_for_mac`) giving files to download and folders to create ("文件夹检测或创建失败"); progress strings "开始分析待更新文件", "待更新文件分析完毕", "开始下载更新文件，即将重启", "更新文件下载完毕，等待重新启动", "下载更新文件 … 中，文件 … MB，已下载 … %，还剩 …", "下载失败，请更换网络尝试", error "更新出错，建议官网下载并安装最新版：https://chixie.com.cn". Chunked download with a progress callback (percentage, size, speed, remaining time). Windows restart uses `taskkill /f /im` then relaunch; macOS uses `open -n -a <app>` ("重启软件中", "升级完毕后重启炽写"). Saves the changelog to `update_log.txt` for the update log dialog @9444d6b..9446ad3.
- Access: free. Confidence: high.

### Paper selection widget (`PaperSelectionWidget`, drawing/laser)
- Where: drawing/laser function menu; two group boxes: standard paper vs custom size paper (config `DrawConfig/PaperType=StandardPaper|CustomSizePaper`, `PaperName=A4`, `PaperOrientation=Horizontal|Vertical`, `CustomPaperSize` dict of named custom sizes).
- What it does: loads standard sizes into a combo; custom paper list with delete ("自定义纸张 - 删除模板"), name change ("自定义纸张-纸名发生变动时候"), validation "纸张名称为空", "纸张名不能与标准纸张名相同"; "保存数据到全局变量，1初始化时候，2点确定时候"; inside a menu, clicking is absorbed so the menu does not close ("menu菜单中，是通过这个方法关闭menu的，在此组织点击时候发生关闭") @947e2b4..947e94e.
- Confidence: high.

## Backend endpoints seen in this slice (`core.Constant`, @944d849..944dfb9)
Base `client_api/`. `update/check_update`, `update/compare_file_differences`, `user/login`, `user/logout`, `user/collect_missing_characters`, `user/collect_logout_info`, `user/heartbeat`, `pay/get_packages`, `pay/upgrade_payment`, `pay/check_upgrade`, `pay/bind_license`, `file_parsing/dxf`, `file_parsing/image-to-table-template`, `file_parsing/image-to-table-template-2`, `text_processing/text_block`, `font/get_online_fonts`, `font/get_collected_fonts`, `font/get_recorded_fonts`, `font/get_main_fonts_info`, `font/collect_a_font`, `font/get_character_access`, `font/get_daily_free_font`, `font/download_font`, `font/get_font_access`, `font/get_standard_br` (standard bounding boxes), `font/admin_tune_fonts`, `invitation/get_invitation_code`, `invitation/bind_invitation_code`, `invitation/query_binding_of_invitation_code`, `config/query_exclusive_config_file`, `config/query_config_download_url`, `config/font_config`, `config/download_exclusive_config_file`, `ai/get_ai_account`. Also local files: `note_template.cx`, `table_template.cx`, `composition_template.cx`, `composition_temp.cx`, `jieba_dict.txt` (segmentation for line breaking), custom punctuation sets used for typesetting (prohibited line-start/line-end punctuation lists at @944dfb9, 944e001: `([{·‘“〈《「『【〔〖（．［｛￡￥` and `!),.:;?]}¨·ˇˉ―‖’”…∶、。〃々〉》」』】〕〗！＂＇），．：；？］｀｜｝～￠%`). Existing `docs/server-api.md` should be cross-checked for `font/get_standard_br`, `config/font_config`, `pay/*`, `invitation/*`, `file_parsing/*`, `text_processing/text_block`, `font/admin_tune_fonts` (candidates not yet implemented).
- Account/VIP signals in this slice: `IsVip`, `UserRole`, `UserRoleCode.super_user`, "超级管理员" title, `pay/*` (packages, upgrade, license binding), invitation codes. VIP-gated: default (main) fonts, font collection (and the 30-font cap), only daily-free font for ordinary accounts, device test (admin).

## Cross-reference with existing docs
- `docs/protocol.md` (G-code layout): confirms `G21`, `G90`, `G92 X0 Y0`, pen up `G1G90 Z0 F7000`, pen down `G1 Z-<depth>`, and the `$Beep` end melody. This slice adds: the exact beep string, `G90G0 X0Y0` / `G90G0 X-20Y0` end positions, `$HZ` (home Z), `$H` (machine home), `$WHO` (identity/UUID/firmware version), `$X` unlock, `$RST=*`, speed-mode tiers, direction-mode flips and the pen weight levels.
- `docs/server-api.md`: this slice names many additional endpoints (see above).

