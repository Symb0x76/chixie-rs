# ChiXie3 slice C: writing (书写) editor and shared utilities

Source: `slice_C.txt` (0x9506313-0x957b2bf), Nuitka constants of ~170 app classes. Offsets quoted as `@hex`. Docstrings (Chinese, 3-space/4-space indented) survived compilation, so many behaviours are stated by the original developer; those are cited as "docstring". Numeric literals (floats like the TPS scale) are NOT recoverable (Nuitka stores them as raw bytes), so only names/keys are given. Cross-references: `docs/protocol.md` (G-code as captured), `docs/server-api.md`, `slice_A.txt` (enums in `core.ChiXie` @944...-9447...).

**Summary.** The writing part is a PyQt5 `QGraphicsScene` editor with three paper types: 笔记 (NoteTemplate: ruled rows, optional dual-line rows and columns), 表格 (TableTemplate V2: tables of "parent cells", each with 1..N internal input rows) and 作文格 (CompositionTemplate: one character per grid cell). Every character is a `TextItem` holding a font stroke path (fetched per character from the server, decrypted with Fernet, optionally deformed), laid out by an incremental, thread-based typesetter (`InputTypeSetting`, `SelectTypeSetting`, `ReTypeSettingContinuousItems`, `InputNesting*`). Handwriting naturalness comes from per-character random spacing, vertical offset, base deformation (scale/rotate/stroke jitter) and thin-plate-spline (TPS) "shape" deformation. Each page becomes G-code through `GcodeGenerator` (start point modes, path planning, parity-page rotation, pen-up/down per device type, optional external route-optimizer plugin, multicolour, break-point resume). Around that: import (txt/docx/pdf/xlsx/cdoc), table data sources, overflow handling, search/replace, character tuning, clipboard (internal doc-items, Excel XML Spreadsheet, WPS RTF), printing, plus login/account UI, anti-crack heartbeat and utility classes.

Not recoverable from constants: numeric defaults other than those in `config_sanitized.ini`, exact shortcut key sequences beyond the Qt standard keys named, enum integer values.

## Index of features
- Account and access: 登录 (LoginDialog); 用户信息/升级套餐 (UserInfoDialog); user roles; UserLimitUtil (macOS third-party device limit); AntiCrackingUtil (heartbeat/online detection); login gating of writing features; 系统托盘 (system tray)
- Papers and templates: 笔记纸 (single/dual-line/multi-column); 表格纸; 作文格纸; 单元格属性 (TableCellAttributeSettingDialog); 内部行数; 同型标记 (same-cell records); 书写原点 (OriginPointItem); 模板选择下拉; 模板存取 and encryption; 表格模板自动保存; 模板版本转换
- Character model: TextItem/SpaceItem/LineBreakItem/CursorItem; 字符格式 (标题/倾斜/下划线/删除线/上下标/颜色); font selection and mixing (FontMixer, CharAvailableFontList, 单元格字体加载模式); missing characters (MissingCharDisplayDialog); 字体下拉框 (FontDropdownBox); 字体加载 (FUtil)
- Typesetting: 输入排版; 换行/回车; Tab 首行缩进; 空格/半格空格; 作文格排版 + jieba 分词; 表格输入排版; 选中排版 (formatting of selection); 重排版; 对齐 (left/center/right/justify, vertical); 垂直偏移 (含智能垂直偏移); 字符间距; 字符位置合理性检测; 内容整体上下移; 合并内容 (作文格)
- Overflow: 单元格内容溢出 (3 plans); 批量溢出处理
- Handwriting effects: 基础变形; 字形变形 (TPS); 字符微调 (CharacterTuningDialog) incl. 变瘦/变胖/应用至同字符/同步至服务器/重复轨迹优化/异常宽度优化; 路径优化 (RouteImprove Simplify/CatmullRom/Uniform); 字符扩充
- Writing output: 书写管理 (WritingManagementWidget); 书写此页; 上传SD卡书写; 工作区巡边/绘制; 断点续写; 书写G-code生成器 (GcodeGenerator); 路径规划 (之/工); 起点模式; 奇偶页方向; 写完回位; pen up/down per device; 路径优化插件; 多色绘图; 自动进纸设备; 绘制表格 (table line drawing)
- Import/export: 导入文件 (txt/docx/pdf/xlsx/cdoc); 覆盖/追加; xlsx 表格导入 (merged cells, number formats, custom data source); 批量导入 (批量建页); 表格数据源 (default/random/custom); 另存为/打开 .cdoc; 打印文字/打印背景图 and print preview
- Editing: 复制/剪切/粘贴 (internal, Excel XML, WPS RTF, plain text); 全选; 撤销 (not implemented); 删除/退格 (+ table confirm); 方向键/Shift 选择; 鼠标选择; 查找和替换; 右键菜单; 插入菜单
- View: 缩放; 单页/双页预览; 显示框线/行号/页码; 大文本模式 (visible-item culling); 页码检测; 光标样式
- AI input widget hosted in WritingPart
- Shared utilities: ArcFitter/EllipticArcFitter; FontSimplifyUtil; FontCharExpandUtil; FullAndHalfWidthUtil; GenerateGcodeUtil; Network; Download/DownloadPreviewPicUtil; ToolUtils; ThemeUtil; TranslationUtil; LogUtil; SecurityUtil; ConfigurationUtil; CustomMessageBox; FilePathUtil; ExceptionUtil; global `Variable` state

---

## Account, roles and limits

### 登录 (LoginDialog)
- Where: login dialog (singleton); fields account/password, checkboxes 记住密码 / 自动登录, links 注册 and 忘记密码.
- What it does: reads `[LoginConfig] Account/Password/RememberPassword/AutomaticLogin` (@9508681 ids) and fills the form; register / forgot-password links are `WebsiteUrl` + `page/register` and `page/forget-password` (@9508858, @9508870). Button flow: `_checkInput` -> InfoBar "信息填写不完整" (@9508d9d); `_login` -> POST `LoginUrl` (`user/login`, from slice A) with `account`, `password`, `platform`; button text becomes "登录中..." and operations are disabled (`_enableUserOperation`, @9508e50). Failure: InfoBar "登录失败" / "登录失败，请检查网络连接" (@95089e2). Success: `_saveData` stores account/password/checkbox state back to config.ini; `save_gv_data` copies the response into globals (comment "1给本窗口的登录用，2给软件初始化时候的自动登录用", @9508c7a). Response fields used: `token`, `backup_token`, `username`, `role`, `email`, `phone`, `created_at`, `role_code`, `is_vip`, `upgrade_date`, `period_of_validity`, `expiry_date`.
- Internals: `core.User.LoginDialog`; `check_user_role` (@9508ac5 docstring "如果是普通用户，需要查询今日免费字体信息"): for `ordinary_user` it emits `show_daily_free_font_signal` so the font dropdown shows the daily free font area. Signals `login_success_signal`, `err_signal`, `msg_signal`. `Variable` keys: Logged, UserName, UserRole, UserRoleCode, UserPhone, UserEmail, UserCreatedAt, Token, BackupToken, IsVip, UpgradeDate, PeriodOfValidity, ExpiryDate, InvitationCode, UserPermission.
- Access: free (account needed).
- Confidence: high

### 用户角色 (role codes)
- Where: `Variable.UserRoleCode` compared to the strings below (@9508a2c-9508aac).
- What it does: roles are `ordinary_user`, `vip_user`, `super_user`, `writing_robot_seller`, `super_vip_user`. `UserRole` holds the display label (the tuning dialog compares it with the string "超级管理员", @951d9be).
- Internals: `ordinary_user` gets only the daily free font and the paid-upgrade page; `vip_user`/`super_vip_user` skip the license-code upgrade ("会员身份无需绑定注册码升级，您已经是会员了！" @9509804). `super_user` is the admin (see 字符微调 sync). `writing_robot_seller` is a reseller role (no direct use in slice C).
- Confidence: high for names, medium for permissions

### 用户信息 / 升级套餐 (UserInfoDialog)
- Where: user-info dialog (singleton): info page, 升级 page, buttons 退出登录, 使用注册码, 绑定邀请码（升级有折扣）, radio buttons "注册码选项", "我是炽写设备选项框", "直接支付 单选框（跳转支付宝支付）" (@950a1f9-950a2b1 docstrings).
- What it does:
  - Shows name, role, phone, email, created date; for members also 升级日期, 有效期 (period_of_validity), 到期日期 (`_initData`). Package durations dictionary `one_week`, `one_month`, `one_year`, `forever` (labels like "一个月").
  - 退出登录: confirm "您确定要退出登录吗？" then clears login/token globals and emits a restart/relogin path.
  - 使用注册码 (license code): empty -> "您未填写注册码，请填写后再绑定"; POST `pay/bind_license` (`BindLicenseCodeUrl`) with `license_code`; if the server says the code is bound to an exclusive device config, asks "该注册码已绑定专属设备：“…”，是否加载该设备的配置文件并重启？" (@9509a7a); on yes downloads the exclusive config (`config/query_exclusive_config_file`, `config/download_exclusive_config_file`), writes it as `config.ini` in the user dir, saves the login, and emits `restart_signal`.
  - 邀请码 (invitation code): `get_input_dialog` "请输入邀请码", POST `invitation/bind_invitation_code`, `invitation/query_binding_of_invitation_code` (shows 已绑邀请码, disables the button), `invitation/get_invitation_code` (own code). Bound codes give "折后" prices.
  - 升级 (package purchase): `pay/get_packages` fills a table (package_name, package_price, available_duration); select a row ("请先选中需要升级的套餐"), quantity spin box, price with/without invitation discount; confirm text "您选择的套餐是：…，数量…，总价…，绑定邀请码后折扣价…确认前往支付页面升级吗？" (title "下单确认"); POST `pay/upgrade_payment`, then `webbrowser.open(pay_url)` (Alipay web). Follow-up dialog "支付询问": "请在打开的支付宝页面进行支付" with button "我已支付"; `_check_paid_status` / `_check_if_upgrade` call `pay/check_upgrade` and refresh role/expiry ("当前用户类型：…，会员有效期至：…").
  - Network errors: "网络状态异常".
- Internals: `core.User.UserInfoDialog` (@950a673), `Variable.InvitationCode`.
- Access: login-only
- Confidence: high

### UserLimitUtil: macOS third-party-device limit
- Where: `core.User.user_limit_util` (added 2025-12-31 per header @950af62), single static method `limit_for_macos` (@950b225).
- What it does (docstring @950b11e, @950b147): "限制第三方设备使用macos" (limit use of non-ChiXie devices on macOS). TODO text: "暂时从2026年开始限制，后续要进一步细化谁有权限（超级会员除外，普通会员要看从macos会员上线后，是否有人购买过macos的版本的会员）". So: the limit is meant to apply from 2026; super VIP users are exempt; ordinary members are undecided. Constants show the method uses `sys` (platform check), `ChiXie` enum, `Variable` and returns a `can_use` flag; the year 2026 is an int constant that is not visible.
- This is the only dedicated "limit" util in slice C. Other practical limits seen elsewhere: not logged in -> cannot type/import ("请登录后再输入内容！" @95541d1, "请登录后使用！" @95362cf, "用户未登录，请先登录" @9562e05); ordinary users only get 今日免费试用字体; non-admin cannot sync character tuning; Export G-code is super_user only (protocol.md, other slice). Heartbeat failure revokes the session (see AntiCracking).
- Access: applies to non-super-VIP users on macOS with third-party devices
- Confidence: medium (logic body not recoverable, intent stated in docstring)

### AntiCrackingUtil: online detection / heartbeat
- Where: `core.Util.AntiCrackingUtil` (@950b2a6); `user_online_detection` is called from `WritingManagementWidget` (before start/resume writing, @95525d1) and from `WritingFileBtnMenu` (file-menu actions such as new/append page, @953642b).
- What it does: POST `user/heartbeat` (`HeartBeatUrl`) with token data; on failure increments `OnlineDetectionFailedCount`, logs, and when it exceeds `OnlineDetectionFailedMaxCount` shows InfoBar "在线状态异常，请检查网络后重新登录" (@950b4e5) and calls `clear_font_and_login_info` (docstring "清空字体数据和用户登录数据", @950b548): clears loaded font info and resets Logged, IsVip, Token, BackupToken, UserPermission, UserRole, UserName. The next detection is scheduled with a `QTimer.singleShot` at a `random.randint` interval (random heartbeat delay). `debug_check` ("调试检测") has no body constants (stub; imports `psutil`, `platform`, `sys`, `logging` suggest an intended process/debugger check).
- Access: logged-in users
- Confidence: high for flow, low for debug_check

### 系统托盘 (CxSystemTrayWidget / InitCxSystemTray)
- Where: Windows tray icon (`chixie_logo.png` @95080a0) and a small widget with button "退出炽写" (docstring "退出炽写按钮点击", @9507a81).
- What it does: singleton widget (`__new__`, "实现单例"). Tooltip text: "炽写：" + "版本号：" + major version (`Major_Version`); "暂未登录" when logged out (TODO says it should also refresh after manual login). Dev comment: future custom tray menu with writing-control buttons.
- Confidence: high

---

## Papers, templates and cells

### 笔记纸 (NotePaperItem) - single / dual-line / multi-column
- Where: writing area when template type is Note (`ChiXie.NoteTemplate`); file filter "笔记文件 (*.txt *.docx *.pdf *.cdoc)".
- What it does: loads the NoteTemplate (page size, margins, row count min/max, `enable_dual_line_mode` + `up_row_height`, dashed partition, columns; see protocol.md for template fields), places input cells per row (`_place_cell`), dual-line notes use `_place_cell_for_dual_line_note` where `cell_row_position_type` is `TopNoteRow` (upper band) or `IndependentRow` (@954c4fc, @9559343), `_add_extra_line_path` draws extra preview lines, `_init_cell_list_key` builds the key list for cells. Import into a dual-line note asks "当前为双线模式笔记，请问导入至上侧行还是下侧行？"; multi-column asks "当前为多栏笔记，请问导入第一列还是第二列？（或点击右上角叉号取消）" with buttons 第一列/第二列 (@9528e9e, @9528f21).
- Internals: `NotePaperItem` (@955a6a2), `first_cell` (used by the AI widget), `BasePaperItem` (@9557ebe) provides origin-point item placement, `words_number`, `set_page_size`, `toPixmap`, `calculate_first_text_offset` (first-char offset, see first_text_not_align).
- Confidence: high

### 表格纸 (TablePaperItem_V2 / TableItem / TableParentCellItem)
- Where: Table template type; filter "表格文件 (*.docx *.xls *.xlsx *.cdoc)".
- What it does: pages hold one or more `TableItem`s (resizable `BaseItem` with handles and lock-size/lock-position) generated from `TableItemInTemplate`; each table has `TableParentCellItem`s (mother cells) containing `InputCellItem` rows. Multi-page table templates map page numbers to template pages via `ToolUtils.get_template_index_by_page_number` (docstring "【表格模板】根据页码，获取该页码在模板中对应的页码").
- Parent-cell attributes (@955d6b3 methods): accept_import (does cell receive imported data; non-importing cells paint a hatch "不导入标记"), write_region_width/height, internal_margin_left/top/bottom/right (painted as a margin frame to show the user), cell_font_loading_mode, cell_char_width_ratio, cell_hint_label (prompt text drawn in the cell), cell_class (e.g. `cell_1_3`), real_cell_class, cell_font_size, cell_font_id, h_alignment, `adaptive_font_size` ("自适应计算字体大小", uses `setPointSize` to fit hint text).
- Confidence: high

### 作文格 (CompositionPaperItem_V2)
- Where: Composition template type; filter "作文格文件 (*.txt *.docx *.pdf *.cdoc)".
- What it does: grid of single-character cells (`CompositionRowNumber`, `CompositionColumnNumber`); each char gets one cell; line breaks reflow content (`_wrap_content_down_for_composition`), `move_character_in_composition` moves content by +1/-1 cell. Combined characters (two digits, etc.) can share a cell via `generate_combined_text_item` (docstring "生成组合字符图元对象，目前是给作文格用", "用于两个数字情况"). Append import is not supported ("暂时不支持作文格追加" @952a3c6).
- Confidence: high

### 单元格属性 (TableCellAttributeSettingDialog)
- Where: right-click a table cell -> 单元格属性 (@9525b85); dialog stays on top.
- What it does: edits attributes of the selected cell(s) (multi-select supported: the dialog collects lists per attribute): 提示标识 (hint label, InfoBar "提示标识设置成功"), 默认字体 (combo of collected fonts, "默认字体设置成功"), 默认字号, 字体加载模式 (combo items: **指定字体优先**, **字体库配置优先**, **随机字体模式**, @9575fef-957601a = slice A enums `CellSpecifiedFontFirst`, `CellFontLibraryConfigFirst`, `CellRandomFontFirst`), 字符宽度比 (cell_char_width_ratio, spin box), 行数 (internal rows, via `SetTableCellRowCount`), 内边距 上/下/左/右. OK button confirms: "确认修改选中单元格的行数、上下左右内边距吗？" then "保存成功". Controls are disabled for cells with accept_import false (`_refresh_widget_status_by_accept_import`).
- Internals: `SetTableCellRowCount.set` adds/removes `InputCellItem` rows and refreshes row positions/size (`_refresh_row_pos_and_size`, `_add_input_cell_for_table_cell`, `_reduce_input_cell_for_table_cell`); changing rows/margins triggers `ReNestingTableCell` to re-typeset content.
- Confidence: high

### 同型标记 (SetSameCell)
- Where: runs after adding pages of a multi-page table template (`AddPaperOfWrite.add_paper_of_table(add_count, need_async)`).
- What it does: docstring "为新增的页面配置同型标记单元格". Uses the template's `same_cell_record` (record name -> cell classes per template page): for every added page, cells of the same record are re-keyed (`page_N_cell_x_y` renames, `fake_cell_class`) so related cells across pages are tracked together. On inconsistency: "同型设置 e", "同型标记设置存在一些问题，请删除同型记录或者联系客服". Exact user meaning (probably linking equivalent cells across pages) is inferred.
- Confidence: medium

### 书写原点 (OriginPointItem)
- Where: a draggable marker on each page; hover tooltip "书写原点" + "请确保笔尖在此位置处" (@953856c, @953857a).
- What it does: shows where the pen tip must be placed. Movement is constrained to the paper and snaps/hints to page key points (`_detect_page_key_point` returns x/y key points near the cursor, docstring "需要返回x、y方向的关键点。如果没有接近的点，则返回None"). Modes `StartPointOfPaper` (纸张起点), `StartPointOfFrame` (框线起点), `StartPointOfCustom` (自定义原点位置); fixed modes need no stored coordinates, custom mode saves origin position (docstring @9538344). Saving is debounced (`WritingOriginSavingFunc.schedule_delayed_save`, "安排延迟保存"; note and composition templates store one position, table templates in frame/paper start mode need one per page @95156e3).
- Right-click on the marker gives: 移至纸张起点, 移至框线起点, 自定义原点位置 (@9525144-95251e8). The choice is stored in `[WriteConfig] start_point_mode` (user config shows 11; integer encoding of the enum, mapping not recoverable) and `Variable.StartPointMode`.
- Confidence: high

### 模板选择 (TemplateSelectionWidget) and template storage
- Where: template dropdown in the writing ribbon; icons Word.svg / Excel2.svg / OneNote.svg per type (mapping inferred).
- What it does: lists template names by type (`init_model_data` at startup, `add_one_item_to_model`), selecting loads the template (`NowWritingTempType/Name` stored in `[WriteConfig]`), refreshes after save, converts old-version template files ("检测老版本模板文件" -> `convert_old_template_to_new`, `WriteTemplateUtil.convert_old_template_to_new_template`).
- Internals: `TemplateAccessUtil` saves note/composition templates, queries all template names/data, reads/saves table templates; table templates are encrypted with `cryptography.fernet` using a key hard-coded in the binary (@9515de1; value intentionally not reproduced). Docstring: "目前主要是给表格模板加密存取数据使用; 未来如果模板数据上云的话，可能直接在这里变更即可". `SaveTableTemplateTimer` (singleton QTimer, single shot) debounces auto-save of the current table template through `TemplateAccessThreadUtil` (`_save_now_table_temp`, `_save_temp_origin_data`); a code comment notes the timer only works with the singleton pattern. Files: composition/note templates are `composition_template.cx` etc. (slice A paths).
- Confidence: high

---

## Characters, fonts and formatting

### 字符图元 (TextItem / SpaceItem / LineBreakItem / CursorItem)
- Where: everything in the editor is an item in `WritingScene`.
- What it does:
  - `TextItem` attributes: text, font_id, preview_font_id (hover preview), is_title, a_width / b_width / c_width, v_offset, alignment, font_size, raw_font_size, under_line, delete_line, text_color, inclination, is_subscript, is_superscript, text_path, draw_path, raw_text_path (@95398af ids). Selected background `#FFFFCC`, warning background and statistics-hover background colours (`#D24D57`, @953a876). Cached with `DeviceCoordinateCache`; `getWritingPath` produces the final stroke path scaled by `PXScale` (preview scaling) for the G-code and the on-screen preview; `toPixmap` renders it; `reset_text_path` re-derives the path (docstring @953a6ab: after tuning, never deform an already-deformed path again, "用一个path多次变形后，会越来越不像样").
  - `__setCWidth` (docstring @9539d26 "设置字符的c宽度，一般是针对于标点符号"): punctuation gets a separate width (`c_width`) using `LeadingPunctuation` / `PostPunctuation` sets, a special case for `.,:;` (@9539cc3) and a `random.uniform` factor.
  - Sub/superscript: `setSubScript` / `setSuperScript`; `finalFontSize` is not `font_size` (docstring @953a14c: preview and final path use a size adjusted for super/subscripts) and `vOffset` shifts the glyph.
  - `SpaceItem`: width, `isIndent`/`setIndent` (first-line indent flag). `LineBreakItem`: paragraph/return marker with a drawn path (↵ symbol, "创建回车符换行符的路径path"). Both support `setRegionHeight`, `setFontSize`, `setRelativeFontSize`, `get_predict_width_before_insert`.
  - `CursorItem`: blinking caret (`QTimer`), `setCursorHeight`, `set_out_of_page` ("设置光标离开纸张", used on select-all, template change, new template), `re_show`, per-cell x position.
  - `InputCellItem` is the row container: `add_item`/`remove_item`, `left_align_margin`, `calculate_first_text_offset`.
- Confidence: high

### 字符格式 (ribbon operations in SelectTypeSetting)
- Where: ribbon 书写 tab font/paragraph groups (handled by `DisplayWritingFormat` refresh + `SelectTypeSetting` modes, @9541d75 ids).
- What it does: modes `SetFont`, `EnlargeFontSize` (增大字号), `ReduceFontSize` (缩小字号), `SetTilt` (倾斜), `SetUnderLine`, `SetDeleteLine`, `SetTextColor` (设置选中的文字颜色), `SetSuperScript` (设置选中的文字为上标格式), `SetSubScript` (…下标格式), `SetTitle` (标题), `ClearFormat`, `FirstLineIndent`, `VerticalOffset`, `DecreaseIndent`, `IncreaseIndent`, `AlignLeft/AlignHCenter/ALignRight/AlignJustify`, `ALignTop/AlignVCenter/AlignBottom` (vertical alignment, table cells). Applying a format to selected text re-typesets the affected range. If the target table cell has unresolved overflow: "存在内容溢出，请先处理溢出" (@9542abc).
- Notes: tilt is applied only after the user clicks 倾斜 (TODO @953d5a8 "写字偏好中要配置左右倾斜度范围，同时醒目提醒用户，只有界面点击了倾斜才会倾斜", so a left/right tilt range preference is planned); the GenerateItem inclination getter `__get_inclination`.
- `DisplayWritingFormat` (@9506313): `load_writing_format` runs on a thread after selection changes and drives checked/enabled state of the ribbon buttons for: cell selection in table, text selection, click in note. Docstring @950672c: "检测所选内容的字体格式一致性（所选内容是否每一个格式都相等）": a toggle shows checked only if every selected character has that format. Also loads font name/size, paragraph alignment (`__load_paragraph_alignment`). `SignalBridgeOfSelectOrClick` carries select/click signals between the view and the ribbon.
- Confidence: high

### 字体设置 (SetFontAndSize) and 字号
- Where: font combo box and font size box in the ribbon.
- What it does: `set_font` / `set_font_size` for selected characters or selected table cells (`__set_font_of_selected_table_cell`, `__set_font_size_of_selected_table_cell`); selected text path re-typeset by `ReTypeSettingContinuousItems`. Main window font size box is kept in sync with the selection (`__setFontSizeOfMainWindow` finds the `font_size_combobox`). Quick-size mode: `Variable.InQuicklySetFontSize`, `ShowFontSize`.
- Confidence: high

### 字体下拉框 (FontDropdownBox, FontTitlePreviewLabel, ScaleSizeLabel, LoadFontComboBoxThread)
- Where: font dropdown popup in the writing ribbon (button "字体库" `on_pushButton_clicked`).
- What it does: three regions: 今日免费试用字体 (daily free fonts, hidden for members: "设置用户每日试用字体区域显示或隐藏（对于非会员用户）"), 用户录制 (the user's own recorded fonts, loaded once), 用户收藏 (collected fonts, reloaded when the collection changed: "用户收藏的字体内容改变", "检测用户收藏的字体是否发生变化"). Each font is a `FontTitlePreviewLabel` (picture of the font name, tooltip "字体名：… 字体类型：…", 1px/2px blue `rgb(131,198,239)` selection border). Hover preview: mouse-over a font previews it on the selection (`previewFont`) and leaving restores it (`fontPreviewRestore`, "预览还原"). The last row is the `ScaleSizeLabel` (size scaler). Font labels are loaded in a `QThread` (`load_font_label_in_thread`) after startup. Preview pictures come from `DownloadPreviewPicUtil` (`font_preview` folder).
- Confidence: high

### 字体调用模式与混用 (FontMixer, CharAvailableFontList)
- Where: settings 字体 tab (config `[FontConfig] MainFont, MainSymbolFont, MainEnglishFont, MainNumberFont, FontsDefaultSize, FontsMixRatio`; `Variable.FontCallingMode`).
- What it does: `ChiXie.FontDefaultMode` vs `ChiXie.FontMixMode`.
  - **Classification (`CharAvailableFontList.get_char_font_list`, @9568626)**: each char is typed `is_chinese`, `is_english`, `is_number`, `is_symbol` (`LeadingPunctuation` + `PostPunctuation` sets from slice A: opening `([{·‘“〈《「『【〔〖（．［｛￡￥` and closing `!),.:;?]}¨·ˇˉ―‖’”…∶、。〃々〉》」』】〕〗！＂＇），．：；？］｀｜｝～￠%`), or space. Four lists are built from the main fonts plus collected fonts: `chinese_char_font_list`, `english_char_font_list`, `number_char_font_list`, `symbol_char_font_list` (de-duplicated in order with `remove_duplicates_with_order`, main font first, then `UserCollectedFonts`, then `NowFontID`). In default mode the list is the fallback order: the first font that has the glyph is used (`check_sequence`). Lists refresh after the collected fonts change or after startup (`refresh_all_types_char_font_list`, signal `font_load_done_signal`; "开始刷新字符字体列表").
  - **Mixing (`FontMixer`, added 2025-09-02)**: `configure_fonts({font_id: weight})` (docstring "配置字体混用比例", dict "{字体ID: 权重}") or `configure_fonts_from_collection` (uses collected font info `mixing_ratio`). `get_font_for_character` picks the font for a char via three strategies: weighted random (`_get_font_random`, "基于权重的随机选择"), deterministic by char hash (`_get_font_deterministic`, "基于字符哈希值的确定性选择（相同字符总是返回相同字体）"), by Unicode code point (`_get_font_by_unicode`, "基于字符Unicode码点的确定性选择"). A `seed` attribute exists. Which strategy the UI exposes is not recoverable; weights come from `FontsMixRatio`.
  - **Table cell override**: cell font loading mode 指定字体优先 / 字体库配置优先 / 随机字体模式 and the cell's `cell_font_id` reorder the list (docstring @9568ac3: "对于表格单元格，需要注意单元格是否有默认的字体id设置").
  - `TextItem` generation: `GenerateTextItem.generate_text_item` (docstring @953c20a "根据各种设置来创建（主线字体等、收藏的字体等）"), `generate_text_item_with_specified_font`, `__get_number_char_font_list`, `__get_symbol_char_font_list`.
- Config sync: `FUtil.sync_user_font_config` GET/POST `config/font_config` (QueryFontConfigUrl/SetFontConfigUrl) round-trips main font ids, `CharacterDeformation` settings, `enable_shape_deform`, `deform_range` so font/deformation config follows the account.
- Confidence: high for structure, medium for UI exposure

### 缺字处理 (MissingCharDisplayDialog)
- Where: dialog opened from the missing-character warning; plain-text box with button 复制 (InfoBar "复制成功", @952fa44).
- What it does: when the chosen fonts lack a glyph, `TextItem.getWritingPath` logs "字体缺字符：" and appends the char to `Variable.missing_characters`; the dialog lists the missing characters (`missing_text`) so they can be copied (and recorded). A server endpoint `user/collect_missing_characters` exists (slice A) for reporting. The missing char itself shows in the warning colour in the page. Added 2025-12-16.
- Confidence: medium (display behaviour inferred from names)

### 字体加载 (FUtil)
- Where: startup / after login / on collect-or-uncollect.
- What it does: `QThread` with modes `LoadMainFont`, `PreLoadFont`, `DownloadFont`, `TextDeformation`; `get_collected_fonts` (`font/get_collected_fonts`, paged, page 0 = all at init), `get_recorded_fonts` (`font/get_recorded_fonts`), `get_main_fonts` (`font/get_main_fonts_info`), `download_font_file` (progress via `StateToolTip`, "预解析中", "个字体加载完毕", "个字体解析完毕"), `load_unloaded_fonts`, `pre_load_main_font`, `pre_load_font` (`font/get_font_access` -> `link` + key, JSON blob decrypted with Fernet: per-character data uses a key derived from `BackupToken`, `pre_load_font` takes `font_key` from the access response; see server-api.md). `pre_load_main_font` branches on `UserRoleCode == ordinary_user` (ordinary users load the daily free font rather than collected fonts; inferred). Failure message: "字体（…）加载失败，建议重启或取消收藏该字体。如是您本人的字体，请确保该字符已有录制数据" (@9511728). `get_character_path` (per-character fetch via `font/get_character_access`) is deprecated in favour of preloading ("！！！ 这个方法暂时弃用…", @9510c8a).
- Confidence: high

---

## Typesetting (排版) engine

### 输入排版 (InputTypeSetting)
- Where: every keystroke from the hidden `UserInputBox` (a `QLineEdit` whose `textChanged` carries IME text, @954b728).
- What it does (docstrings @953ee8a-953f765): for note/table `_type_setting_of_note_and_table`: "step1 遍历需要插入的字符，创建textItem，从光标处挨个插入; step2 向原本光标处后方遍历，遍历到字符则往后排" (insert new items at the cursor then push the following items of the same paragraph back; recursion/iteration "涉及迭代"). Row choice: "根据光标位置、（设置了行高的item）item的宽度属性去判断item插入哪一行" - item width (+ predicted width via `get_predict_width_before_insert`) is compared with the remaining width of the row, wrapping to the next row when it does not fit; in a note a missing next row creates a new page, in a table the cell gets another internal row or the content scales ("表格模板，需要新增单元格或者缩放前面的内容") or an overflow record is made. Stop condition when scanning the rest of the paragraph: a return char or an empty row ("后向搜索停止条件：需要向后搜索直到出现回车符或者空行").
  - Runs in a thread (`QRunnable`), `input_without_thread` for synchronous callers (search/replace). While busy `Variable.InTimeConsumingNesting` is set.
  - No selection typing: "暂不支持选中单元格后输入，请双击放置光标后输入" (table cell selected but no caret).
  - **Return**: `__press_Key_Return` adds a `LineBreakItem` at the caret, moves the rest of the row to the next row ("回车换行"), and a Return on the last row can add a page; in a table the count of line breaks is checked against the cell's internal rows first (`__query_table_cell_row_count_before_input`).
  - **Space**: `__press_Key_Space` inserts a `SpaceItem` whose width is random in `[min_space_width_zh|en, max_space_width_zh|en]` (zh or en selected by the previous character, `__generate_SpaceItem`).
  - **Tab**: `__press_Key_Tab` ("按键盘Tab键") inserts a first-line-indent `SpaceItem` with width `random.uniform(min_first_line_indent, max_first_line_indent)` at the cursor then re-flows (`setIndent`). So the indent is applied by Tab or the 首行缩进 button, not automatically (this explains the protocol.md observation).
  - Backspace/Delete: `delete_single_item`, `_move_cursor_to_previous_cell`, `_adjust_after_delete(_multiple_item_of_note)`; deleting at row start pulls the previous row up.
  - After input: `_set_alignment_after_input` (`align_function.set_alignment_of_paragraph_at_cursor`), then `typesetting_complete_signal` triggers the position-rationality detection and refreshes the writing-management list.
- Confidence: high

### 作文格排版 (composition grid) and 分词 (SegmentationUtil / jieba)
- Where: composition templates.
- What it does: input is split into words so a word is not broken across grid rows: `SegmentationUtil.get_text_list_of_composition(text)` returns blocks (`need_input`, `need_segmentation` lists); `segment` is `jieba.cut` with the bundled `jieba_dict.txt` (`jieba.set_dictionary(Constant.jieba_dict_path)`, `jieba.initialize`; digits are kept together via `isdigit`). `text_block_to_item_list_of_composition` (docstring @9564e21 "作文格式输入时候获取字符对应的item; text_block: 字符段（词语）") builds items per block; `_wrap_content_down_for_composition` shifts following cells when a block does not fit; `move_character_in_composition` (move_number is +1 or -1, @953f5ee). There is also a server endpoint `text_processing/text_block` (slice A). `jieba` is the only NLP use in slice C; it feeds the composition grid and the position-rationality check named "分词位置合理性检测" (docstring @953a767).
- Inference: the grid does not hyphenate by word; words are kept whole when they fit, which is the 作文 convention.
- Confidence: medium-high

### 选中排版 / 重排版 (SelectTypeSetting, ReTypeSettingContinuousItems, ReNestingTableCell)
- Where: any format change on a selection, font/size change, first-line indent, search-replace.
- What it does: `ReTypeSettingContinuousItems` (docstring @95474fa): "重排版连续的多个item（指定范围连续的item）… 当前版本的重排版会将所有待重新排版的item都先从cell中提取出来，然后再挨个导入排进去". Items after the range are re-typeset too unless a stop condition (line break, end) is met; afterwards it checks whether following rows must move up (last item's row changed) or down. `re_nesting_selection` is the entry for selections. Tables use `ReNestingTableCell(cell, appoint_items=None)` to re-flow a whole cell (used by font/size change, cell attributes and tuning). `TakeOutItemsInTableCell.take_out_all_items_in_table_cell` / `take_out_range_items_in_table_cell` pulls items out first.
- Confidence: high

### 首行缩进 (first-line indent)
- Where: ribbon 首行缩进 (`FirstLineIndent`), 增加缩进 / 减少缩进 (`IncreaseIndent` / `DecreaseIndent`), Tab key.
- What it does: for note/table, `_add_first_line_indent_item` inserts a `SpaceItem` (indent flag) before the first character of the paragraph at the caret or of every paragraph in the selection (`_get_first_item_in_every_paragraph_when_selected`) then re-types the range. For composition `_switch_the_state_of_first_line_indent_for_composition` toggles indent cells (first two cells of the paragraph): `_add_first_line_indent_item_for_composition`. Width is random `[min_first_line_indent, max_first_line_indent]` mm (config 15.5-17.0 in the user's ini). Only usable with a caret or a selection: "当前版本首行缩进需在‘选中字符’或‘放置光标’后使用" (@954229f).
- Confidence: high

### 首字不对齐 (first_text_not_align)
- Where: `[WriteConfig] first_text_not_align=true`, `first_text_not_align_range=2.0`.
- What it does: `BasePaperItem.calculate_first_text_offset` / `InputCellItem.calculate_first_text_offset` ("计算首字偏移、首字不对齐的偏移量"): the first character of every row starts at a random x offset in `[0, range]` mm (captured in protocol.md: 1.81, 1.42).
- Confidence: high

### 字符间距 / 垂直偏移 / 智能垂直偏移
- Where: settings 书写偏好: `min/max_char_spacing_zh|en`, `min/max_space_width_zh|en`, `min/max_vertical_offset`, `intelligent_vertical_offset`, `row_offset_style`, `random_correct_offset` (config) and `Variable.Min/MaxCharSpacingZh|En`, `EnableIntelligentVOffset`.
- What it does: `GenerateTextItem.__get_h_offset` returns the per-character horizontal offset ("获取字符间距（横向偏移量）"; ranges are loaded from config at writing init, docstring @953cb91) as `random.uniform(min,max)` using the zh or en range of the character; `get_v_offset` returns the per-character vertical offset. `SetVerticalOffsetOfText.set_vertical_offset_in_note_temp` / `_in_table_temp` (re)apply offsets: with `intelligent_vertical_offset` on the offset depends on `position_proportion = target_pos_x / cell_width`, i.e. a smooth drift along the line instead of pure noise (otherwise uniform random in `[min_vertical_offset, max_vertical_offset]`, `width_of_write_area`). TODOs in `get_v_offset` (@953cc4f): follow the whole-row offset trajectory, optional offset wobble, and the user's global vertical offset (to stop small fonts sitting at the row top). Menu: ribbon `VerticalOffset` action.
- Confidence: high for names, medium for exact algorithm

### 对齐 (AlignmentFunction)
- Where: ribbon 左对齐/居中/右对齐/两端对齐 and table vertical alignment buttons.
- What it does: `set_alignment` at the paragraph at the caret (`set_alignment_of_paragraph_at_cursor`, empty alignment value = refresh after edits, docstring @951bffe), or for a selected cell (`set_alignment_of_target_single_cell`) or whole cell class (`set_alignment_of_target_whole_cell`); `_calculate_content_width_in_cell` + `_adjust_items_pos_in_cell_for_alignment` shift items within each row. `refresh_alignment_after_input` is called whenever a character position or width changes. `show_alignment_info` loads the current state into the ribbon. Errors: "请先放置光标或者选中字符再使用对齐功能" and "请先选中单元格". Docx import maps paragraph alignment (see 导入).
- Confidence: high

### 字符位置合理性检测 (CharacterPositionRationalityDetection)
- Where: runs after `typesetting_complete` (docstring "位置合理性检测，目前是全局检测" @951d124; TODO: two modes later: global for import, local for input/paste).
- What it does: a `QRunnable` scans every row (`_position_detection_of_note/table/composition`) and flags rows where the first char is a closing mark of `PostPunctuation` (e.g. `，。！？）》`) at the line start, or the last char is an opening mark of `LeadingPunctuation` (e.g. `（《“`) at the line end (variables `warning_leading_items`, `warning_post_items`). A distance `threshold` from the row's right edge is used (`last_item_x` + `width*PXScale`). Flagged `TextItem`s get `setWarningStatus` (warning background). It is a warning only; slice C shows no automatic 避头尾 correction in the typesetter itself (the word-split TODO says it was meant to detect "分词位置" problems).
- Confidence: medium (inferred from variable names and the punctuation sets)

### 全角/半角 (FullAndHalfWidthUtil)
- Where: `GenerateTextItem.generate_text_item` calls `FullAndHalfWidthUtil.auto_convert` before generating glyphs.
- What it does: `is_full_width`, `is_half_width`, `Q2B` (全角转半角, single char) and `B2Q` (半角转全角), `auto_convert` ("自动转换全角半角"). Two long tables at @9513587 (full-width punctuation `。，、＇：…`) and @95136f3 (half-width counterparts `｡,､':…`) define the punctuation mapping. Which direction `auto_convert` chooses per context is not recoverable; the intent is that digits/letters/punctuation take their natural width for the chosen font.
- Confidence: medium

### 内容整体上下移 (ContentsMoveAsWhole)
- Where: internal, used when inserting/deleting returns or when paragraphs change row count.
- What it does: `contents_move_up(target_cell_index, target_move_row_num, cell_class)` moves all following content up by N rows; `contents_move_down(cell)` moves content from a row downward (default one row; "目标cell不存在，无法移动"). `GetNextCell.get_next_cell_for_same_class` finds the next row cell; for tables it returns None if the content overflowed; notes/composition auto-create the next page when missing.
- Confidence: high

### 合并内容 (CombineCompositionContent)
- Where: composition mode, selection -> merge into one cell.
- What it does: `combine_selected_content`: merges two or more selected chars into a single grid cell by building a combined glyph path (`generate_text_path_of_multi_char`, `_adjust_for_differences_width`, each char squeezed by `width_target_scale_size`). Errors: "合并内容只能合并两个以及以上的字符", "作文格模式合并内容只能选中连续文字内容". TODO @953c843: combined chars could come from different fonts.
- Confidence: high

---

## Table cell overflow

### 单元格内容溢出处理 (ContentOverflowHandling / ContentOverflowHandlingDialog)
- Where: when typing/pasting into a table cell exceeds its internal rows, the first sub-cell gets a warning button; click opens the dialog ("内容溢出处理窗"). Message: "该单元格存在内容溢出情况，请处理溢出后再编辑排版！" (@9554271) blocks further editing of that cell.
- What it does: `ContentOverflowHandling` stores per-cell `overflow_info` (content, predicted solutions) via `set/append/get/delete_overflow_info_of_cell`, shows the warning button (`__show_warning_btn_of_cell`). The dialog shows the overflow text (items converted via `items_to_content`; unknown item -> "未知类型Item字符") and computes solutions:
  1. 方案一: 缩放字符 (scale all text in the cell to fit current rows). Not available when line breaks exceed the rows: "(所输入内容中换行符超出单元格设定行数，方案1无法使用)"; "不推荐方案1，内容过多，预期缩放字符尺寸太小。" Uses the initial size = the size of the last typed character (`get_target_initial_font_size`) and `__get_width_of_item_block`.
  2. 方案二: 增加行数 (add internal rows and shrink the font). Computed in a thread (`ContentOverflowAnalysisThread.calculate_plan_2`, "方案2计算中，请稍等结果加载完毕", optimal row count via `predict_min_row_count_in_cells`, `__calculate_font_size_of_appoint_row_count`, "计算指定行数下什么字符尺寸可以塞进去"). Notices: "(当前行数已是最优行数，方案2不推荐)", "(方案2不推荐，内容相对过多，预期缩放尺寸太小。建议使用方案3…)", "(方案2无法使用，内容相对过多…)".
  3. 方案三: 忽略内容 (drop the overflowed text, "建议使用方案3删除字符后重新输入适量内容").
  Buttons call `on_pushButton_clicked/_2/_3` for plans 1/2/3; failures: "方案1处理失败，请联系开发者或客服", "方案2处理失败，". `ContentOverflowUtil.if_table_cell_content_overflow`, `compare_cell_count_and_line_break` (first check), `re_input_item`, `get_write_region_size_of_table_cell` (cell write region minus margins).
- Confidence: high

### 批量溢出处理 (BatchOverflowHandlingDialog)
- Where: dialog with a start button (`on_startHandleBtn_clicked`).
- What it does: on open shows the count of overflowed cells ("没有单元格内容溢出" when none) and enables mode buttons; modes 缩放字符 (`_handle_of_scale_size`, `_calculate_and_scale_text` then re-type), 增加行数 (`_handle_of_add_row_count`, `_calculate_optimal_count_and_size`), 忽略内容 (`_handle_of_ignore_content`). Message: "个单元格中行数不符合内容需要".
- Confidence: high

---

## Handwriting effects

### 基础变形 (CharacterBaseDeformation)
- Where: settings 字符变形: `[CharacterDeformation] EnableCharacterDeformation`, `enable_base_deform`, `min/max_horizontal_scale` (96-104), `min/max_vertical_scale` (98-102), `min/max_rotation_angle` (-2..2), `stroke_deform_degree` (1), `randomly_delete_trailing` (true).
- What it does: `CharacterDeformationUtil.character_deform(enable_deform, text_path, text, ...)` calls `CharacterBaseDeformation.base_deform` when `enable_base_deform`, then `CharacterTpsDeformation.tps_deform` when `enable_shape_deform`. Base deform draws a random horizontal scale, vertical scale and rotation within the configured ranges (`__get_random_value_of_range`, results rounded with a truncating `custom_round`: docstring "会将0.45999输出为0.45") and applies them to the whole glyph around the glyph centre (`math.cos/sin/radians`), then `re_calculate_width`. Stroke-level jitter: `set_stroke_deform_parameter_by_level(level)` derives `min/max_horizontal_scale_of_stroke`, `min/max_vertical_scale_of_stroke`, `min/max_rotation_angle_of_stroke` and `min/max_horizontal_offset_of_stroke`, `min/max_vertical_offset_of_stroke` from `stroke_deform_degree`; each stroke gets its own scale, rotation and offset. `randomly_delete_trailing` removes tail points ("随机删除小尾巴"). Reset/update: `reset_deform_parameter` ("重置默认变形参数", "恢复默认推荐的变形参数") is called at start-up and after config changes. Also `deform_for_thinner` / `deform_for_fatter` (horizontal squeeze/widen by `proportion` through `target_scale_size`) used by 变瘦/变胖. Errors are logged as "炽写字符变形异常1101：" and "变形错误：…，font_id:…,text:…".
- Slice C comment: "TODO 后续需要加速计算速度。考虑使用包括但不限于numpy…、concurrent.futures…、JIT编译、使用C重构调用" (deformation is slow).
- Confidence: high

### 字形变形 (CharacterTpsDeformation) - thin-plate spline
- Where: `[CharacterDeformation] enable_shape_deform` (true), `shape_deform_degree` (7); flags `Variable.CharacterDeformation`.
- What it does: warps the glyph with a thin-plate spline (`tps.ThinPlateSpline`, numpy). Algorithm (from names and docstrings): take the glyph's stroke points (`bihua_points`), build control points on a grid plus randomly perturbed copies; `random_ctrl_points(...)` ("Generate random ctrl points (In proportion of the desired shapes)"; docstring: the bigger `tps_scale` the bigger the base distortion; perturbation split into `coner_wave`, `center_wave`, `edge_wave`, with `increase_absolute_value` pushing a too-small perturbation outwards); fit the spline `fit(input_ctrl, output_ctrl)`, `transform` the points; check the result against the original with a similarity measure and retry (`start_similarity`, `tps_times`): `haar_wavelet_similarity` (2-D Haar wavelet `pywt.dwt2`, correlation of the cA/cH/cV/cD sub-bands, `overall_similarity`), `euclidean_distance` and `dtw_sim` (dynamic-time-warping similarity) as alternatives. The level (`tps_degree` = `shape_deform_degree`) controls `tps_scale`. `reset_deform_parameter_to_deform` ("恢复默认推荐的变形参数") sets tps_degree/start_similarity. Result is written back via `tps_font` per stroke; consecutive points are re-split by stroke start index (`start_index`).
- Per protocol.md capture: degree 7 gave an italic-like shear and a smooth non-affine residual, consistent with a low-frequency random warp.
- Confidence: medium-high (parameters' numeric values not recoverable)

### 字符微调 (CharacterTuningDialog)
- Where: dialog over the page (always on top), opened from the ribbon after selecting a character; needs a selected char ("请选中字符后使用", @951dec8) or a selected table cell.
- What it does: previews the selected char (`previewLabel`, `_refresh_preview` renders `text_path` with chosen font/size) and edits it:
  - font combo (collected fonts) -> `set_font_and_size.set_font` ("字体切换失败", "字符微调中字体切换失败：")
  - size combo, ➕/➖ buttons (font-sizeadd.svg / font-sizereduce.svg) -> `set_font_size`
  - ↑ ↓ ← → buttons (up.svg, down.svg, left.svg, right.svg) shift the glyph (`setVerticalOffset`, horizontal offset) and a centre button (yuandian.svg)
  - 变形 (`on_transformationBtn_clicked`: "请先在设置窗口中启用变形配置" if deformation off), 变瘦, 变胖 (`CharacterBaseDeformation.deform_for_thinner/fatter`)
  - 恢复默认 (`on_backDefaultBtn_clicked`): "当前版本恢复默认只可恢复变形、变胖、变瘦方面变动" (resets only deformation/thin/fat changes)
  - 应用至同字符 flyout (`RoundMenu` with actions "应用至同字符" (ApplyWithoutTransform) and "应用至同字符(同时变形)" (ApplyWithTransform)): applies the current tuning to all identical characters in the page (`_apply_to_other_text`, `Counter` of characters).
  - 字符重复轨迹优化 (`on_charSameRouteOptimizeBtn_clicked`, `find_and_clear_duplicates`): removes duplicated overlapping stroke segments in a glyph path (shorter pen route).
  - 异常宽度字符优化 (`on_abnormalTextWidthBtn_clicked`, `char_abnormal_width_optimize`): clamps/normalizes strokes whose x extent is abnormal (uses stroke min/max x).
  - 同步至服务器 (`asyncToServerBtn`, **visible only if `UserRole` == "超级管理员"**): posts the tuned path to `font/admin_tune_fonts` (`AdminTuneFontUrl`) -> `add_text_path_to_font`. Rules: "同步功能只支持选中单个字符进行同步", "同步功能只支持同步文字或者字符", "为防出错，微调同步操作仅支持收藏一种字体", "微调请先关闭字符变形并重启", result "保存失败". The preview shows the font author name (`author_name`).
  After any edit the affected cell is re-nested (`ReNestingTableCell`) or the range re-typeset.
- Confidence: high

### 路径优化 (RouteImprove: Simplify / CatmullRom / Uniform)
- Where: `Variable.RouteImproveMode` (`RouteImproveNone`, `RouteImproveSimplify`, `RouteImproveCatmullRom`, `RouteImproveUniform` from slice A) and `RouteImproveValue` (device settings 路径优化). Applied when a glyph path is made.
- What it does: `FontSimplifyUtil` (Douglas-Peucker with `epsilon`; `perpendicular_distance`, `simplify_stroke`, `simplify_all_strokes`) removes points to speed writing; `FontCharExpandUtil` (`expand_all_strokes_no_scipy(text_path, method, num_points, flip_y)`) adds points with `catmull_rom` interpolation (`catmull_rom_spline(num_points_per_segment)`, "不支持的插值方法" for other names) or `uniform` arc-length resampling (`uniform_resampling_no_scipy` using cumulative distance and `numpy.interp`) for smoother output, both without scipy.
- Confidence: medium (mode-to-function mapping inferred from names)

### 弧线拟合 (ArcFitter, EllipticArcFitter)
- Where: shared utilities (no caller inside slice C; presumably plotting/CAD import).
- What it does: `ArcFitter(radius, start_angle, offset_angle)` and `EllipticArcFitter(center_x, center_y, a, b, start_angle, offset_angle)` produce polyline points for an arc ("根据圆弧特性计算最优拟合点数量", `generate_points`, `get_fitting_info`). The elliptical one has `generate_points_with_adaptive_sampling(max_error)` (recursive subdivision until chord-height error <= `max_error`). Positive offset = counter-clockwise, negative = clockwise. Errors: "半径必须为正数", "椭圆长轴a和短轴b必须为正数".
- Confidence: high

---

## Writing output: tasks, G-code, resume

### 书写管理 (WritingManagementWidget)
- Where: the writing task panel (table view of pages) with buttons 开始书写 / 暂停书写 / 继续书写 / 停止书写 (`start_btn`, `stop_btn`; icons start.svg / pause.svg; strings "开始书写", "暂停书写", "继续书写", "停止书写").
- What it does: one row per page in `WritingTaskModel` (page index, character count per page, status `UnWritten` / `Writing` / `InPause` / `Written`); counts are kept in sync after every typesetting (`sync_char_count`, `_sync_of_page_count`: adds/removes rows when the number of pages changes). Click a row to select a page (`now_selected_page_index`). Start: runs `AntiCrackingUtil.user_online_detection` first, then builds a `Task` per page with `GcodeGenerator.get_gcode(paper, device_name, device_type)` -> `initial_gcode`, `body_gcode`, `end_gcode`, calls `ChiXieSerial.setEnableMultiColor(Variable.MulticolorDrawing)` and `startTask` (pause -> `pauseTask`, stop -> `stopTask`). Status changes switch the button icon/text (`_switch_status_to_writing/pause/stop`). Buttons are enabled by combination of character count, serial connection, and status (`_set_btn_group_status`, docstring @9552180); the start button tooltip is translated (`translation_util.translate_text`). Errors: serial error -> `on_serial_error_stop`; "书写任务页面获取失败，请联系客服"; InfoBar warning when the device is not connected; `writing_done` resets rows to 已书写. Log "开始书写序号为：". State flags also mirrored on `AutomaticPaperDeviceDialog` (`Writing`, `InPause`, `Written`) so the auto-paper-feed dialog stays in sync (inferred).
- Internals: slots for `write_specified_page_signal` / `write_from_break_point_signal` from `WritingFunction` (`write_specified_page`, `write_from_break_point`, `write_from_break_point_of_chixie_device` @950782c), `get_gcode_when_continue_write` for resume.
- Confidence: high

### 书写此页 / 直接书写 / 上传SD卡书写 / 工作区巡边/绘制 / 添加页面 (page context menu)
- Where: right-click on a paper (page) in the editor (`ContextMenuOfWrite._create_action_for_page`, @9525545).
- What it does:
  - 书写此页 (submenu icon PENCIL_INK): 直接书写 (writes that page through the serial/Telnet task, confirmation "书写确认": "是否立即书写第 N 页…", errors: "页没有书写内容", "设备未连接", "炽写设备未连接", "当前有书写任务正在进行中，请先结束任务") and 上传SD卡书写 (SDCard.svg: uploads that page's G-code to the board's SD card and runs it; emits `upload_to_sd_run_signal`; the connection check depends on `DeviceControlPannelMode`: for `OtherDevice` it tests `Serial.is_connected`, for ChiXie hardware `chixie_serial_process.hasTask`). 
  - 工作区巡边/绘制: opens `WorkspacePatrolAndDrawingDialog(paper_index, words_number)` (module `core.MainWindowFunction.workspace_patrol_and_drawing`, other slice) to trace the page border / draw on the workspace.
  - 断点续写: opens the break-point dialog (see below).
  - 添加页面 (ADD icon): `AddPaperOfWrite` adds a page of the current template type.
  - Start-point submenu on the origin marker (see 书写原点).
- Confidence: high

### 断点续写 (WriteFromBreakPointDialog)
- Where: context menu 断点续写; dialog stays on top.
- What it does: continue a page that stopped mid-way (pen ran dry, paper moved, serial error). The user selects the **first character of the not-yet-written text** in the editor (exactly one non-space char: "请选中待续写区文本中第一个字符，不要选中多个", "请选中有内容的字符，不要选中空格"); the dialog label shows that character and 继续书写 is enabled only then (`has_selected_item_signal`, `_enable_write_btn`). Preconditions: serial connected and no running task ("请先连接串口或先等待无书写任务时再续写", @95356a2); status text "正在续写". It then calls `write_from_break_point` (or `write_from_break_point_of_chixie_device` through `chixie_serial_process` for ChiXie hardware) and closes. G-code: `GcodeGenerator.get_gcode_when_continue_write(paper, item_to_continue_write)` regenerates the page but only emits strokes from that item on (`can_continue_write` flips true when the item is reached); initial/end G-code are re-emitted.
- Confidence: high

### 书写G-code生成器 (GcodeGenerator)
- Where: `core.WritingManagement.GenerateGcode` (@9519...); methods `get_gcode`, `get_gcode_of_automatic_paper_device`, `get_gcode_when_continue_write`.
- What it does (and see docs/protocol.md for captured text):
  - **Device params**: `_get_device_params(device_name)` returns `device_type`, `write_speed`, `pause_time`, `pen_up`, `pen_down`, `x_plus/x_minus/y_plus/y_minus` (axis sign mapping) from the active template's `Variable` values, or from the config for another named device (docstring @95198d7). Not cached on the object.
  - **Initial G-code** (`get_initial_gcode`): `G21`, `G90`, feed `F<WriteSpeed>`, pen up; for 3D-printer type literals `G1 E`, `G92X0.0Y0.0Z0` and `G0 X0 Y0 F` exist (priming/zero). 
  - **Offsets** (`get_offset_value`, docstring "获取因为原点位置带来的偏移值"): by origin mode - `StartPointOfPaper` uses the origin item position on the paper, `StartPointOfFrame` the table frame start/end (`origin_point_of_frame`, `end_point_of_frame`), `StartPointOfCustom` the user's dragged origin. Item coordinates are in preview pixels and are converted with `PXScale` (px -> mm) relative to the origin; the device sign mapping `x_plus/x_minus/y_plus/y_minus` is applied last (the captured stream has Y negative going down the page).
  - **Path planning (路径规划)**: `Variable.PathPlanningType` enum `PathPlanningZhi` (之字形: alternate rows are walked right-to-left, `sort_type` left_to_right / right_to_left) vs `PathPlanningGong` (工字形: same direction every row) - exact numeric mapping unknown (user config `path_planning_type=2`). `_get_content_items_by_path_planning_type(cell, sort_type)` returns the row's items in the order to write (for right-to-left the order is presumably reversed).
  - **Parity page direction (奇偶页方向)**: `check_if_need_to_flip_180(page_number)` by `ParityPageDirectionMode` (config `[ParityPageConfig]`): `SameDirectionForParity`, `OddPagesRotate180`, `EvenPagesRotate180`, `SameDirection180`. If flipped, `get_target_xy_value` mirrors the point around the paper/frame (`frame_width/height`, `ToolUtils.calculate_symmetric_point`) so a sheet fed upside-down (duplex / auto-feeder) is written rotated 180 degrees. The origin marker position is mirrored for those pages (`BasePaperItem.set_origin_point_item_pos_by_template` reads `ParityPageDirectionMode`).
  - **Body** (`_get_body_gcode`): for each cell (`cell_list`, table pages iterate parent cells with `relative_offset_x/y` = table position, composition/note per row cell) and each `TextItem` (`can_continue_write` honoured), take `getWritingPath()` strokes (`x_list`, `y_list`), add item offsets (`cell_pos_x/y`, `item_pox_x`), transform, then emit: travel `G1 X..Y..F..`, pen down, stroke points, pen up (`_add_single_text_gcode_to_body_gcode`). `G0 ` and `G4 P` literals: pen-down pause `G4 P<pause_time>` (`PauseTime`; the capture in protocol.md saw no dwell over Telnet). Axis words are formatted with an explicit sign and `handle_consecutive_negative` replaces `+-` -> `-` and `-+`/`++` fixes (docstring: "导致N1鬼叫的问题、导致输出Gcode中包含Y--的问题"), matching the signed coordinates in the capture.
  - **Multicolour**: if `Variable.MulticolorDrawing` the item's `QColor.getRgb()` (default `(0,0,0)`, `item_color`) is attached to each item so the sender can issue pen changes; `[MulticolorDrawing] EnableMulticolorDrawing, PenChangingMode` (slice A: `ManualPenChange` / `AutomaticPenChange`).
  - **3D printer type**: `ThreeDPrinter` adds `x_offset_3d`, `y_offset_3d` and a `pen_down_point_gcode` (extrusion move).
  - **End G-code** (`get_end_gcode`, docstring "获取Gcode的最后几行，一般是写完后抬笔回原定"; todo "后续设备管理中需要支持配置是否写完回原点"): by `WriteDonePos`: `BackOrigin` -> `G90G0 X0Y0`, `BackOriginLeft2` -> `G90G0 X…20Y0` (park about 20 mm beside the origin; sign/format not recoverable), `BackNone` -> nothing; preceded by pen up.
  - **Pen up/down** (`get_pen_up_gcode` / `get_pen_down_gcode`, also `GenerateGcodeUtil.gcode_pen_up/down`): by device type `WritingRobot_StepperPenUp` -> `G1G90 Z<penUp|penDown>`, `WritingRobot_SteeringEnginePenUp` / `WritingRobot_ElectroMagnetPenUp` / `LaserEngravingMachine` (`G1 Z`?) / `ThreeDPrinter` / `CustomZCommandDevice`; literal fragments present: `G1G90 Z`, `M3S`, `G1 Z` (@951a088, @951a0b3, @951a0f4, @9513b91, @9513bc5). The servo branch may use `M3S<angle>`, but protocol.md's capture showed servo still streams `G1G90 Z<value>`; trust the capture (VERIFY).
  - **Back to origin** (`GenerateGcodeUtil.gcode_back_to_origin`, @9513f0a): by firmware `ChiXie.Grbl`, `ESP32_Grbl`, `ChiXie_ESP32_Grbl` -> `G90G0 X0 Y0`; `Marlin` -> `G90` + `G0 X0 Y0 F<WriteSpeed>`.
  - **自动进纸设备** (auto paper feeder): `get_gcode_of_automatic_paper_device(paper, item)` returns the three G-code parts for one paper/item.
  - **路径优化插件**: if `Variable.enable_plugin`, `_get_body_gcode_with_plugin` / `_get_page_route_optimizer_data` build `page_route_data` (per text: `this_text_data`, per stroke `this_stroke_data` points, black default colour) and load `<ChixieUserDirPath>/plugin/test.py` with `importlib.util.spec_from_file_location`, instantiate its `RouteOptimizer` and call `start_optimizing(...)`. Errors "插件加载失败：", "插件不存在，请先添加插件", "测试插件返回值为None". Comment: pause-after-pen-down is ignored in plugin mode because it targets only the vendor's devices (@951a2b4).
  - `Variable.RouteImproveMode/RouteImproveValue` (stroke simplification/resampling, see 路径优化).
- Confidence: high for structure and literals, medium for branch mapping

### 绘制表格 (CalculateTableDrawingPath / TableDrawingLineGenerator)
- Where: table context menu 绘制表格 (docstring "选中并绘制表格功能"); a string "尚未开发" exists in the same menu builder (@9525d42).
- What it does: draws the table's grid lines on the paper with the pen ("用于纸面绘制表格等"). `CalculateTableDrawingPath.get_drawing_path(table_item, offset_x, offset_y)`: converts cells to rectangles (`_table_item_to_cell_region_data`), collects all horizontal and vertical lines, merges collinear segments (`merge_lines`, "合并共线的线段"), subtracts the border (`_reduce_border_lines`) and computes the outer frame path `[p1,p2,p3,p4,p1]` (`calculate_max_frame_border_path`); absolute position and the offsets are included. Class docstring: "计算表格框线绘制轨迹 - 遗传算法" (the earlier version ordered the lines with a genetic algorithm; `_raw` variants remain). The newer `TableDrawingLineGenerator(cells)` takes `{cell_class: (top_left, bottom_right)}` and builds: outer border as one continuous clockwise path ("最优路径：顺时针") and inner grid lines ordered by column/row to cut pen lifts ("优化路径：按列/行顺序，减少抬笔"); `generate_trajectory` returns a list of strokes (outer list = pen-lift separated, inner list = points). Error: "单元格数据不能为空".
- Confidence: medium-high

---

## Import / export / print

### 导入文件 (ImportFileFunctionOfWrite and parsers)
- Where: 文件菜单 -> 打开文件 (`on_open_file`, dialog title "打开文件") and the ribbon import button; filters by template: 笔记 "笔记文件 (*.txt *.docx *.pdf *.cdoc)", 表格 "表格文件 (*.docx *.xls *.xlsx *.cdoc)", 作文格 "作文格文件 (*.txt *.docx *.pdf *.cdoc)" (@9529086-95290db).
- What it does: `start_import` runs `ImportFileFunctionOfWrite` as a thread with a progress callback ("开始解析文件", "文件解析完毕"); the page must be empty or the user picks: "检测到页面已有内容，请问是覆盖还是追加？" (title "导入确认"; `ChiXie.DirectImport`, `AppendImport`, `OverwriteImport`). `_check_if_has_content_of_note/composition/table/in_target_cell` decide. Unsupported formats: ".doc" -> "暂不支持doc格式文档导入，请转换为docx格式，或直接粘贴" (tables: "暂不支持doc格式表格导入，请先转换为docx格式"); ".xls" -> "暂不支持xls格式表格导入，请先转换为xlsx格式"; PDF table -> "暂不支持pdf格式表格数据导入，请将数据发给客服适配"; other template -> "文件导入模块暂未支持该模板类型"; "文件解析异常"; excel not configured -> "存在未配置Excel导入参数的模板页，请咨询客服"; table parse error -> "表格数据解析异常，请检查配置数据或者联系客服".
- Parsers:
  - TXT (`ParseTxtFile`): reads raw bytes (size capped by `min(getsize, …)`), `chardet.detect` encoding, `splitlines` -> paragraphs; each paragraph dict `{style_name:'Normal', alignment:AlignLeft, runs:[{text, italic, underline, font_color, subscript, superscript}]}`. Errors "导入TXT报错, 请联系客服".
  - DOCX notes (`parse_docx_for_note`, python-docx): paragraphs -> runs with text, italic, underline, font colour (`font.color.rgb`), subscript, superscript, paragraph alignment (`transform_docx_paragraph_alignment`: CENTER -> AlignHCenter, RIGHT -> AlignRight, else left; no justify mapping seen). Errors: "docx文件版本过旧，请将文件重新另存为“Word文档 docx格式”，请勿选择“Strict Open XML文档 docx”", "docx文件中可能存在异常图片，或者未能成功解析的数据。请手动粘贴到炽写或者联系开发者" (zip/CRC errors), "docx文件解析失败，请联系客服".
  - DOCX tables (`parse_docx_for_table`): rows/columns/cells -> `table_data` per table (repeated merged cells removed) mapped to template cells named `cell_*`; "解析docx表格文件失败".
  - PDF (`parse_pdf_file_for_note`): PyMuPDF `fitz.open`, `page.get_text()`, lines -> paragraphs (no styling). "PDF文件解析失败，请联系开发者".
  - XLSX (`ParseXlsxFile`, openpyxl): `parse_xlsx_file_for_table(template, …)` -> pages ↔ sheets. If a multi-page template has a different page count than the workbook: "多页模板页数和所导入的表格sheet页数不一致"; a multi-page template supports only one complete data set: "当前多页模板只支持单份完整数据导入，请联系客服". Per sheet: template cell rectangles come from the table template's `cell_class`; merged cells are expanded (`merged_cells.ranges`, `get_need_exclude`, `is_subset`/`reduce_subset` remove cells covered by merges); cells flagged not-importable (`accept_import=False`) or with a custom data source (`CustomCellDataSource`) are skipped (`not_import_cell_class`, `custom_data_source_cell`; warning "导入xlsx时，自定义数据源设置或者导入异常:"); "视觉行列" mapping (`_analyze_the_actual_number_of_rows_and_columns`, visual rows/columns of merged cells); value formatting by `cell.number_format`: General, `0` -> `{:.0f}`, `0.0` -> `{:.1f}`, `0.00` -> `{:.2f}`, `0.000` -> `{:.3f}`, accounting-style `0_ `, dates (`m"月"d"日";@` -> "M月D日", datetime `00:00:00` suffix stripped). Formula cells: TODO ("后续需要支持对于公式的解析"). Warnings: "部分单元格解析异常，请检查相关参数或文件", "部分单元格可能未能正确解析格式，请联系开发者", "导入异常".
  - CDOC (`.cdoc` ChiXie document, `core.DataModel.ChiXieDocument`/`ChiXieDocItem`): lossless import of items (text, font id, size, colour, formats). Conversion `ChiXieDocItemToItem` (DocItem_to_TextItem / SpaceItem) and the reverse `ItemToChiXieDocItem` (Text/Space/LineBreak).
- Loading into pages:
  - Notes (`LoadContentToNote`, @952ac41): per paragraph builds `TextItem`s from runs (`_get_paragraph_text_item_list_of_note`) and places them **directly into cells** via `InputItemToCell.append_item_to_cell` (fast path, with its own row-width check `predict_max_x`), adds a `LineBreakItem` per paragraph, sets paragraph alignment at cell level (`set_alignment_of_target_single_cell`), adds pages with `AddPaperOfWrite.add_paper_of_note` when the page runs out. `target_cell_class` picks the column / upper-lower line. Modes overwrite (`clear_target_cell_class_content` first), append (after the last content or a new page), direct. Progress via `progress_call_back` ("数据导入中", "导入完毕", "开始导入页面").
  - Composition (`LoadContentToComposition`): paragraph runs -> `input_typesetting.input` (full typesetter, jieba segmentation) ("正在导入作文格数据...", "段落导入中..."); overwrite clears all pages (`DeleteFunction.clear_all_page_content`); append unsupported.
  - Tables (`LoadContentToTable`): `load_table_data_to_target_page(paper, table_data, progress_cb, start_progress, end_progress)` per page ("页面 N 单元格数据导入中", "表格页面数据导入完毕"); overwrite replaces the current table page, append creates new table pages.
- Confidence: high

### 批量导入 (BatchImportDataDialog) - 批量建页
- Where: 文件菜单 -> 批量导入 (@9535fb8); a dialog with a SegmentedWidget (two segments, "Item"/"Item2" tabs) and a multi-line "名单" text box (`nameListPlainTextEdit`).
- What it does: for table templates, creates one page per line of the list and writes that line into the **selected target cell** of each page (cell font/size from the cell's attributes via `generate_text_item_with_specified_font`; cells of type DefaultValue/RandomValue data source keep their data). Preconditions: "请先选中目标单元格", "请先输入名单数据", "暂时只支持单页模板批量建页" (multi-page templates not supported). Button `批量创建页面` (`on_batchCreatePageBtn_clicked`) uses `AddPaperOfWrite.add_paper_of_table(page_count)`, `CheckPageIfHasContent`, then `load_all_preview_signal`. Added 2025-11-02.
- Confidence: medium-high (second tab content not recoverable)

### 表格数据源 (LoadTableDataSource)
- Where: table templates' cell data source types (`ChiXie`: `NoDataSource`, `DefaultValueDataSource`, `RandomValueDataSource`, `CustomCellDataSource`); context menu 数据源 -> 刷新数据源数据 -> 刷新数据(本页) / 刷新数据(所有页) (`SETTING`, `SYNC` icons; submenu only when the template has `data_source_type`).
- What it does: generates cell contents as `TextItem`s and inserts them into cells (`generate_cell_data`, `generate_whole_page_data`) in a thread (`QThread`, `page_data_load_done_signal`): DefaultValue -> fixed text (`default_value`); RandomValue -> `random_setting.random_mode`: `RandomWithinValue` (number between `min_value` and `max_value` rounded to `decimal_places`: `random.uniform`/`randint`) or `RandomWithinContent` (`random.choice` from `content_list`); Custom -> filled by file import. Existing data cleared first (`_clear_cell_data_before`). Loading is offered when pages are added or after font loading (`query_if_load_page_data_source`: "询问用户是否加载页面数据源数据源", only after `FontInitLoaded`).
- Confidence: high

### 另存为 / 打开 / 保存 .cdoc
- Where: file menu 保存 / 另存为 (`on_saveFile`, `on_saveAsFile`: title "保存为炽写文档", filter "炽写文档 (*.cdoc)"), 打开文件 (accepts `.cdoc`, "u.cdoc" @953650b), 退出软件 (`on_closeSoft`).
- What it does: serializes the editor content as `ChiXieDocument` items (`ItemToChiXieDocItem`) and restores it (`NowChiXieDocument`, `NowChiXieDocumentPath` globals; default folder remembered via `FilePathUtil`, `[FileConfig] DefaultFolderPath`).
- Confidence: high

### 文件菜单 (WritingFileBtnMenu)
- Where: the 文件 button of the 书写 ribbon (`on_writing_file_btn_clicked`).
- Items (exact labels): 模板管理, 从当前模板新建, 追加当前模板页, 从炽写模板库新建, 另存为, 批量导入, 打印文字, 打印背景图, plus 打开文件/保存/退出软件 handlers. Guards: "请登录后使用！", "请等待字体加载完成后使用！" (title "操作提示"), online detection before new/append. "从当前模板新建" = new pages from the current template (`new_now_page_of_writing`), "追加当前模板页" = append another page (`append_now_page_of_writing`).
- Confidence: high

### 打印 (WritingPart.print_view / printPreview / handlePrint)
- Where: menu items 打印文字 and 打印背景图 (mode choice inferred) and a print-preview dialog.
- What it does: renders the scene's papers to a `QPrinter` (native format, high-quality antialiasing, `QPrintDialog` + `QPrintPreviewDialog` with `paintRequested`), page by page at paper size (double-page layout offsets `offset_x_1/offset_x_2` for two papers per row). Whether "打印文字" prints only glyphs and "打印背景图" only the template background is inferred.
- Confidence: medium

---

## Editing commands

### 复制 / 剪切 / 粘贴 (CopyFunctionOfWrite, CutFunctionOfWrite, PasteFunctionOfWrite, WritingClipboard)
- Where: Ctrl+C / Ctrl+X / Ctrl+V (`QKeySequence.Copy/Cut/Paste` in `UserInputBox.keyReleaseEvent`, handled on key release) and the context menu 复制/剪切/粘贴.
- What it does: copy fills the system clipboard with plain text plus the MIME `application/chixie-doc-items` (items with format, `CHIXIE_DOC_ITEM_MIME_TYPE`); `WritingClipboard` caches the data internally because of an observed hang ("出现过未响应问题：炽写内复制数据到剪贴板后，按住粘贴快捷键就会未响应"). `clipboard_data_type` classifies the clipboard ("XML Spreadsheet" for Excel cells, ChiXie doc items, RTF, text; else "未能解析的剪切板类型"). **WPS/Word RTF**: `parse_wps_rtf` decodes `\'xx` byte escapes and maps `\super N` / `\sub N` to `<sup>`/`<sub>` so super/subscripts survive (@9579de8-9579eeb). Excel: `get_selected_cell_matrix` / `_parse_spread_sheet_data` parse the `urn:schemas-microsoft-com:office:spreadsheet` table XML into a row/column matrix.
  - Note/composition paste: inserts items at the caret (`_paste_for_note`, `_paste_for_composition`) via the input typesetter; selection is deleted first (`pre_delete_selection`).
  - Table paste (`_paste_for_table`): single text pasted to every selected cell (`_paste_plain_text_to_cell_selection`); a matrix from Excel must match the selected cell rectangle (`_verify_consistency_of_two_matrix`, `_paste_matched_cell_to_selection`) else "您所复制的表格结构与炽写内选中单元格不匹配"; no cell selected -> "未选中表格单元格，无法粘贴"; "表格模式，粘贴失败".
  - Copy/cut in tables: single cell only ("暂不支持复制多个选中单元格的数据", "暂不支持剪切多个选中单元格的数据"); cell content copied as text and as doc items.
  - Cut = copy + `delete_selection`.
- Confidence: high

### 全选 (SelectAllFunctionOfWrite)
- Where: Ctrl+A / context menu 全选 (`QKeySequence.SelectAll`).
- What it does: note/table/composition variants; select-all state kept in globals (`InTableSelectAll`).
- Confidence: high

### 撤销 (UndoFunctionOfWrite)
- Where: Ctrl+Z (`QKeySequence.Undo`).
- What it does: only shows "撤销功能尚未开发完毕" (undo not implemented).
- Confidence: high

### 删除与退格 (KeyPressDelete / KeyPressBackSpace / DeleteFunction)
- Where: Delete / Backspace in the input box.
- What it does: removes the item before (Backspace) or after (Delete) the caret or the selection; `DeleteFunction.clear_all_page_content`, `clear_target_cell_class_content`, `clear_table_cell_content`, `delete_selected_item_of_note/composition`, `delete_item_before_cursor_of_note/composition`, with row pull-up logic (`_move_row_content_to_previous_cell`, `_calculate_move_value_for_composition_delete`). When whole table cells are selected, a confirm box asks "是否清空所选单元格的内容？(可回车确认）" (Backspace) / "是否删除所选单元格的内容？(可回车确认）" (Delete); Enter confirms.
- Confidence: high

### 方向键与 Shift 选择 (KeyPress*)
- Where: arrow keys and Shift+arrows in the hidden `UserInputBox` (`Key_Left/Right/Up/Down`, `SelectNextChar` = Shift+Right, `SelectPreviousChar` = Shift+Left, `SelectNextLine` = Shift+Down, `SelectPreviousLine` = Shift+Up).
- What it does: caret moves across characters and rows in notes and cell-editing table cells; Up/Down pick the closer row position by x (`_calculate_closer_point`); messages "当前已经是最后一行" / "当前已经是第一行". Table cell navigation by arrows is mostly undone: "暂不支持多选单元格时使用方向按钮", "请在选中单元格或放置光标后使用方向按钮", "方向按钮切换单元格选中尚未开发", "暂未开发单元格向下选中", "暂未开发单元格向上选中" (Right arrow can select the cell rightwards: `select_cell_rightwards_by_cell`). Composition: "作文格下方向按钮暂未开发", "上方向按钮暂未开发". Shift selection not developed for: table next/previous cell ("表格功能选中下一个单元格尚未开发", "表格功能向前选中单元格尚未开发"), table next/previous line ("表格功能想下一行选中单元格尚未开发", "表格向上一行选中单元格尚未开发"), composition lines ("作文格向下一行选中尚未开发", "作文格向上一行选中尚未开发", "作文格功能向前选中尚未开发").
- Confidence: high for strings, medium for exact key assignments

### 半格空格 (HalfSpaceFunction)
- Where: Space with a modifier key (`Key_Space` together with `ControlModifier` / `ShiftModifier` in `keyPressEvent`; probably Ctrl+Space or Shift+Space, the exact assignment is not recoverable).
- What it does: inserts a half-width space item at the caret (`input_half_space_at_cursor`, "输入半个空格").
- Confidence: high

### 查找和替换 (SearchAndReplaceDialog)
- Where: dialog with a 查找 page and a 替换 page; stays on top. Buttons: 查找下一处 (find), 替换 (replace), 替换全部, and the third line edit holds the replacement text.
- What it does: plain-substring search over the page's cell content: `generate_str_content` builds each paragraph's string (`TextItem` text, space, `LineBreakItem` -> newline; unsupported items -> "未支持的搜索内容："), `find_all_indexes` returns all occurrence positions (docstring "查询字符串a在字符串b中出现的序号，可能出现多次"), `generate_item_findings_list` maps matches to item lists. `find_next` selects the next match (`setSelectedStatus`, `ensure_visible_signal` scrolls to it). Replace deletes the match and types the new text through `input_typesetting.input_without_thread`. Replace All loops. Messages: "未搜索到结果", "请先进行查找定位，再点击替换", "未查询到结果，无法替换", "目标替换内容未找到，请先进行查找定位", "替换失败：". Buttons are enabled by whether the line edits have text (`on_LineEdit_textChanged`). No case/regex/whole-word options exist. TODO: "需要放入多线程中" (runs on the UI thread).
- Confidence: high

### 鼠标选择与光标 (SelectItemFunction, WritingView)
- Where: mouse in the page view.
- What it does: press sets the caret / starts a selection; drag selects characters across cells in notes (`_mouse_select_in_multi_cell_of_note`, `_single_cell`), selects table cells (rectangle) and characters inside a cell, and characters in composition grids; release finalizes. Double-click places the caret in a table cell (cell text editing: `table_cell_in_editing`, custom cursor `cell_mouse.svg`). Cursor style: I-beam in editable regions, arrow elsewhere; caret height/position per cell (`_set_cursor_pos_in_cell`, empty cell uses alignment to decide x: `_get_target_cursor_pos_x_when_no_content`). Right button shows the context menu unless `ShowContextMenu` is false or an AI input is active (`in_ai_input`, `in_ai_generate`). Clicking a cell's overflow warning (a proxy widget with data 'ContentOverflow'/'TableCellName') opens the overflow dialog. Selecting emits `load_writing_format` to refresh the ribbon.
- Confidence: high

### 右键菜单 (ContextMenuOfWrite) - complete list
- Where: `qfluentwidgets.RoundMenu` shown at the global click position with a drop-down animation.
- Items by target:
  - Origin marker: 移至纸张起点, 移至框线起点, 自定义原点位置.
  - Page: 书写此页 (直接书写 / 上传SD卡书写), 工作区巡边/绘制, 断点续写, 添加页面.
  - Table cell/table: 绘制表格, 单元格属性, 数据源 (刷新数据源数据 -> 刷新数据(本页), 刷新数据(所有页)).
  - Base (always): 复制, 剪切, 粘贴, 全选 (enabled state depends on selection / caret / template type via `setEnabled`).
- Confidence: high

### 插入菜单 (InsertBtnMenu)
- Where: ribbon 插入 button, `RoundMenu` under the button (added 2025-10-27).
- Items (ids `table_action` with LAYOUT icon, `new_sub_menu` with ADD_TO icon, `formula_action`; the labels 表格 and 新建 are not in this module's constants, probably shared strings): a table entry; a new-object submenu with 文本框, 内容块, 思维导图 (text_box.svg / base_shape.svg); a formula submenu (formula.svg) with 我的收藏, 数学公式, 化学公式, 物理公式. These feed the template editor shapes; in the writing editor only the menu is built here, the actions are placeholders (no handler constants in slice C).
- Confidence: medium

---

## View

### 视图缩放与预览 (WritingView / WritingScene / WritingPart)
- Where: view area; main-window zoom spinbox `writing_spinBox` at the bottom right; config `PreviewMode` (1).
- What it does: Ctrl+wheel zoom (`wheelEvent` with `ControlModifier`, `zoomIn`/`zoomOut`/`zoom(factor, out_control)`, `page_scale_signal`; transformation anchor under the mouse); `set_page_scale`; preview modes `SinglePagePreview` and `DoublePagePreview` with `WritingScenePadding` (`calculate_scene_size`, `set_every_page_position`); `ensure_item_visible_in_view` / `_set_view_center_on_paper` scroll to a page (used by search and import). Toggles for 模板框线 (`CellLineVisible`, `FormLineVisible`), 行号 (`RowNumberVisible`), 页码 (`PageNumberVisible`) via `show_temp_cell_frame_line`, `show_temp_form_line`, `show_row_number`, `show_page_number`. `pp_detection_timer` runs `page_num_detection_func` to show the current page number while scrolling (`set_page_num_signal`). **Large-text mode**: `WritingScene.set_item_quantity_mode_of(ChiXie.LargeNumberOfItems | SmallNumberOfItems)` plus `update_visible_items` (docstring: only items intersecting the visible rect extended by a margin are shown, others hidden/unloaded, "用于优化大文本模式下的交互速度") and a `QPixmapCache` limit.
- Confidence: high

### 复合辅助: 字符统计悬浮 / 警告 / 选中高亮
- `TextItem.setStatisticsHoverStatus` highlights characters when hovering in the character statistics panel (other slice); `setWarningStatus` is used by the rationality detection; `setSelectedStatus` paints the selection background.
- Confidence: medium

---

## AI input widget (hosted by WritingPart)
- Where: a floating `AIInputWidget` + `AIFunctionMenuWidget` anchored at the caret (module `core.ChiXieAi`, other slice).
- What it does in this slice: `__add_ai_input_widget` creates both widgets, `set_ai_input_widget_pos` places them next to the page's first cell (`first_cell` exists "目前主要是给AI控件使用"); signals `show_func_menu_signal`, `close_signal`, `set_page_focus_signal`, `start_generate_signal`, `function_selected_signal`; globals `in_ai_input`, `in_ai_generate` suppress the context menu and key handling while active. `TypeSettingUtilThread` ("排版编辑工具线程… 目前主要给ai工具使用") reads the selected text (`set_for_get_selected_text`), checks page content and adds composition paper for AI output. The AI function prompts live in slice A's `ai_function_enum` (续写, 润色, 简写, 扩写, style changes, 总结, 翻译 中/英/日, 释义, 行动列表, 教学大纲/计划).
- Confidence: medium

---

## Shared utilities

### 网络 (Network / NetworkUtil)
- `Network` wraps `requests`: `post(url, data, chixie_link=True…)` / `get(...)`; URL = `ServerUrl` + `ApiPrefix` (`client_api/`) + endpoint; adds the token from `Variable.Token`. If no token and the endpoint is not `LoginUrl`, `update/…` or `font/get_online_fonts` -> emits `err_signal` "操作失败X01， 请登录后操作" / "操作失败-x02" / "请登录后操作". `post_for_chixie_esp32(url, data, files, headers)` is for the ESP32 board's HTTP (file upload). Response JSON has `status`, `msg` ("请求失败" otherwise). Supports `proxies`, `timeout`, `stream` (downloads). `check_domain_existence` pre-checks a domain. Dev TODOs: unify status-code handling and force re-login on session expiry.
- Endpoints referenced from slice C: `user/login`, `user/heartbeat`, `pay/get_packages`, `pay/upgrade_payment`, `pay/check_upgrade`, `pay/bind_license`, `invitation/*`, `config/query_exclusive_config_file`, `config/download_exclusive_config_file`, `config/font_config`, `font/get_collected_fonts`, `font/get_recorded_fonts`, `font/get_main_fonts_info`, `font/get_font_access`, `font/get_character_access`, `font/admin_tune_fonts`, `font/get_daily_free_font` (via daily free font), `user/collect_missing_characters` (slice A name).

### 下载 (Download / DownloadPreviewPicUtil)
- `Download.download_file(url, save_path, call_back)` multipart range download (uses `content-length`; "文件分片下载失败，"), callback `(progress_percentage, downloaded_size, file_size, download_speed, rest_time)`; old `download_to_local` ("资源已经存在无需重复下载"). `DownloadPreviewPicUtil` is a singleton download thread for font preview images and template-library preview images (`add_card`, `add_image_url`, `set_saving_path`).

### 其他工具
- `ToolUtils`: random string, `rotate_point`, `calculate_symmetric_point`, `get_template_index_by_page_number` (table templates), `excel_column_to_number` (A=1), order-preserving de-dup, truncating `custom_round`, `stringToBool`, `hexToRGB`, `getScreenSize`, 4-bit `binary_to_decimal` / `decimal_to_binary` ("输入必须在0到15之间", "输入的不是有效的二进制字符串").
- `ThemeUtil.setWindowTheme(window, theme)` loads `:/theme/resource/qss/default_style.qss` for pop-up windows (main window already loaded).
- `TranslationUtil`: loads `:/translator/resource/i18n/appLang_CN.qm` / `appLang_EN.qm` (config `Language`), `translate_ui(widget)` per window ("在start入口处初始化加载不生效，需要在每个窗口中单独加载"), `translate_text`.
- `LogUtil`: rotating log `ChiXie-Log.log` with format `%(asctime)s - %(filename)s[line:%(lineno)d] - %(levelname)s: %(message)s`.
- `SecurityUtil.get_md5_of_str`; `ConfigurationUtil` (QSettings-style group write/read of config.ini); `CustomMessageBox.get_msg_box`/`get_input_dialog` (Chinese button text, `min-width:110px`); `ExceptionUtil.exception_handler` decorator; `FilePathUtil` (remember last open folder in globals and `[FileConfig]`).
- `WriteTemplateUtil.convert_old_template_to_new_template(template_type, old_template)`.
- `Variable` (global state class) keys of interest: Logged, UserRole, Token, IsVip, NowFunction, NowWritingTempType/Name/Data, IsMultiPageTemplate, PreviewMode, FormLineVisible, CellLineVisible, RowNumberVisible, PageNumberVisible, ShowCursor, MainFont..MainNumberFont, UserCollectedFonts, UserRecordedFonts, FontsDefaultSize, FontsMixRatio, FontCallingMode, TableDataIsInImport, ShowContextMenu, InTimeConsumingNesting, InTableSelectAll, NowSelectedText, NowSelectedTableCell, ctrl_key_is_pressed, ConnectMode, DeviceControlPannelMode, IsChiXieDevice, BaudRate, WriteSpeed, PauseTime, PenUp/PenDown, WriteDonePos, StartPointMode, X/Y plus/reduce, Min/Max CharSpacing/SpaceWidth/VerticalOffset (Zh/En), EnableIntelligentVOffset, PathPlanningType, Min/MaxFirstLineIndent, FirstTextNotAlign(+Range), MulticolorDrawing, PenChangingMode, CharacterDeformation, ParityPageDirectionMode, RouteImproveMode/Value, enable_plugin, missing_characters, overflow_info, OSS_URL.

---

## Appendix A: features present as strings but flagged "not developed" or stubbed
- Undo ("撤销功能尚未开发完毕" @9575b8d).
- Shift/arrow selection and navigation for table cells and composition rows (see 方向键与 Shift 选择).
- Composition append import ("暂时不支持作文格追加").
- Composition and multi-page table: batch page creation only supports single-page table templates; multi-page table templates import only one full data set.
- `AntiCrackingUtil.debug_check` (empty body), `WritingView._test` (debug; prints index, key_points, content_items, "关键点个数：", "测试用"), a "尚未开发" string in the table context-menu builder (@9525d42).
- doc/xls/pdf-table imports (convert first), Excel formulas.
- Search/replace has no options (case, regex) and runs on the UI thread (TODO).
- First-line indent availability is limited to caret/selection.

## Appendix B: rules worth carrying into chixie-rs
1. Randomness is applied at layout time per character (spacing zh/en, vertical offset, first-char offset, space width, indent width) and baked into the `TextItem`; G-code generation is deterministic from items (consistent with the captures).
2. Rows are filled by predicted item width vs remaining row width; Return adds a `LineBreakItem`; Tab/indent button inserts an indent `SpaceItem`; there is no automatic 避头尾, only a post-hoc warning pass (leading/post punctuation sets are in slice A @944dfb9 and @944e001).
3. Character font choice: per char type (zh/en/number/symbol) ordered list main font -> collected fonts, first font with the glyph wins; cell overrides: 指定字体优先 / 字体库配置优先 / 随机字体模式; mix mode picks by weights (random / hash / Unicode).
4. Deformation pipeline order: base deform (scale, rotate, per-stroke jitter, optional tail removal) then TPS warp; path simplification/Catmull-Rom/uniform resampling selected by `RouteImproveMode`.
5. G-code order: initial (`G21`, `G90`, feed, pen up) -> per cell/row, items in path-planning order (之/工), each stroke: travel, pen down (+`G4 P` if pause), draw, pen up -> end (pen up, `BackOrigin`/`BackOriginLeft2`/`BackNone`). Pages on odd/even rotated sheets are mirrored 180 degrees about the paper/frame.
6. Break-point resume is "skip items until the selected one, then emit".
7. Table overflow is a first-class state: record overflow info per cell, block editing until resolved by scale / add rows / drop.
8. Account gating in the original: typing, import and file-menu actions need login; macOS third-party devices limited from 2026 (super VIP exempt); admin-only tuning sync; repeated heartbeat failures revoke the session. These do not need to be replicated in an offline rewrite.

## Appendix C: config keys touched by slice C
`[WriteConfig]` min/max_char_spacing_zh|en, min/max_space_width_zh|en, min/max_vertical_offset, row_offset_style, random_correct_offset, intelligent_vertical_offset, path_planning_type, start_point_mode, min/max_first_line_indent, first_text_not_align, first_text_not_align_range, NowWritingTempName/Type, PreviewMode; `[FontConfig]` Tag_Filter, Sort_Rule, MainFont, MainSymbolFont, MainEnglishFont, MainNumberFont, FontsDefaultSize, FontsMixRatio; `[CharacterDeformation]` EnableCharacterDeformation, enable_base_deform, enable_shape_deform, min/max_horizontal_scale, min/max_vertical_scale, min/max_rotation_angle, stroke_deform_degree, randomly_delete_trailing, shape_deform_degree; `[MulticolorDrawing]` EnableMulticolorDrawing, PenChangingMode; `[ParityPageConfig]` ParityPageDirectionMode; `[LoginConfig]` Account, Password, RememberPassword, AutomaticLogin; `[FileConfig]` DefaultFolderPath; `[GeneralConfig]` Language.
