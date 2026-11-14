# ChiXie3 slice B: page templates, plotting editor, settings dialog

Source: `slice_B.txt` (offsets 0x94a425f to 0x9506313), plus identifier lists extracted from the same slice. Offsets are written as decimal-free hex strings, e.g. `@94a5d8e`.

Summary. Slice B holds three big areas. (1) The page-template manager (`core.PageTempletManagement`): three template families, namely 文档模板 (note, "笔记/信纸"), 表格模板 (table/form, Excel-like grid of "parent cells" with inner rows, data sources and same-type marking) and 作文格模板 (composition grid). Table templates can be generated from a PDF/image through a backend call (`GenerateTableTemplateByFileThread` uses `core.Util.NetworkUtil`), drawn, inserted with N x M, or captured from a screenshot. (2) A "绘图与激光" ribbon category and canvas (`core.PlottingAndLaserPart`): a vector plotting editor (point/line/rect/ellipse/polyline/curve/polygon/arc, DXF + SVG import, maze/seal generators, layers, grid snap, rulers, text boxes and tables typed with the writing engine). Despite the name, **no laser-specific option (power, speed, hatching, fill) exists in this slice**; the only laser trace is the config key `DrawOrLaser` and the ribbon title. G-code is only mentioned once (Bezier points "适配Gcode需求", @94e4315). (3) `SettingDialog` (`core.Setting.SettingDialog`): a singleton form whose Designer labels (`ui.UI_SettingDialog`, all `UI_*` forms) are NOT in the string table, so only widget names and config keys are recoverable; I map them to the keys of `config_sanitized.ini`.

Important caveats about the evidence:
- Nuitka stores per-module constants in the order: identifiers and strings, then imports, then qualnames, then the file path and the `<module ...>` marker. So strings printed after a marker belong to the NEXT module. I attributed strings accordingly.
- Qt Designer form text (labels, group titles, spin box ranges, placeholders) is absent for every dialog here. Spin box ranges/defaults are integers or floats inlined in code objects and are not recoverable. I therefore list parameters by widget/attribute name and Chinese docstring.
- Developer docstrings (Chinese) are the richest source and are quoted exactly.

## Index of features

Template manager
- 页面模板 / 模板管理 context ribbon (two tabs) and template list
- 创建模板 (文档模板 / 表格模板 / 作文格模板), 保存模板, 删除模板, export button
- 通用参数 pannel (name, A4/B5, page size)
- 文档(笔记)模板参数 pannel, 笔记分栏 dialog, 双线文档 dialog
- 作文格参数 pannel
- 表格图元 pannel and 表格单元格属性 pannel
- 页面缩略图 pannel (add/copy/delete/move pages, background preview, opacity)
- 表格模板: 插入表格 (grid-hover menu and custom dialog), 绘制表格, 截图生成, import from PDF/image file, Excel导入配置, 同型标记, 数据源配置, 合并/拆分单元格, 内边距识别 and 重置内边距, 占位符识别
- Template preview scene, view (zoom, Delete key, context menu), preview items
- Validation messages in SaveTemplateData

Plotting and laser editor
- Ribbon: 绘图与激光 category (画布数据, 模板库, 数据添加, AI板块)
- Paper type settings (standard/custom paper, orientation, custom presets)
- Side toolbar (select mode, drag canvas, grid, grid snap, layer settings)
- Basic graphics: point, line, rectangle, round/ellipse, polyline, curve, polygon, arc by centre
- Graphics attribute dialog and line types (CAD linetypes)
- Layers (图层属性设置)
- Data import: DXF (CAD) and SVG
- Idea functions: 印章生成器, 迷宫生成器, 分形艺术生成器 and 立体字 (stubs)
- Text fields: 文本, 文本框, 表格 in plotting, typing/typesetting, alignment
- Group item resize with inline dimension editing
- Ruler, zoom, selection marks
- Geometry utilities: Bezier fitter, circle/ellipse fitter, image binarizer, image scan lines
- Plotting input box and shortcuts
- Adsorption (snap) points and node points

Settings
- General (默认功能, 同步参数, 语言, 设备管理器状态), CAD import mode
- Writing preferences (char spacing, space width, vertical offset, first line indent, path planning, first-text-not-align)
- Multicolor drawing (pen change mode)
- Character deformation
- Odd/even page direction mode
- Restore recommended deformation, help link

---

## A. Page template management (`core.PageTempletManagement`)

### 模板管理 context ribbon (页面模板 / 模板管理)
- Where: a ribbon context category with two tab names `u  页面模板  ` @94aea47 and `u  模板管理  ` @94aea99. Shown by `InitTemplateManagementUI.show_context_category_page`, closed by `close_context_category_page` (the "返回" panel, icon `close_all.svg` @94af786, "添加返回pannel" @94af797).
- What it does: switching to the template editor puts the ribbon in a context category and shows `PageTemplatePart` (list on the left via `TemplateListViewRegion`, attribute panels, and a scene/view preview). Ribbon panel visibility depends on the template type (`__set_widget_visible_by_template_type`: "根据模板类型设置ribbon菜单栏的控件可见性" @94aee21); note panels only for notes, table panels only for tables.
- Internals: `InitTemplateManagementUI` (@94b08d2) builds `note_extra_function_pannel`, `table_base_function_pannel` (generate-table frame), `table_extra_function_pannel`, `identification_region_pannel`, a save panel and a create/delete panel. It loads the currently chosen template from config keys `NowWritingTempType` / `NowWritingTempName` (cross-ref: `[WriteConfig]` in config, e.g. `NowWritingTempType=3`). Context signals: `TemplateContextSignal.merge_cell_signal`, `select_cell_signal`.
- Access: not stated; saving cloud-synced templates is outside this slice.
- Confidence: high for structure, medium for access.

### 创建模板 (文档模板 / 表格模板 / 作文格模板)
- Where: panel "创建模板" (`new_model.svg`, @94af89f) with a drop-down of three entries: `u文档模板` (icon `Word.svg`, @94af8d2), `u表格模板` (`Excel2.svg`, @94af943), `u作文格模板` (`OneNote.svg`, @94af95e). Also `delete.svg` (delete) and `export.svg` (export) actions (`deleteTemplateAction`, `exportTemplateAction`, `createTemplateAction`). The list model uses icon names `doc.svg`, `xls.svg`, `pptx.svg` @94b3831 for the three kinds (note/table/composition) in the saved-template list.
- What it does: creates a new unsaved template. New-mode names shown in the list: `u新建笔记` @94afea2, `u新建表格` @94afee8, `u新建作文` @94afef6. "取消选中（一般是新建模式下需要取消选中" @94bc284 (list selection is cleared in new mode).
- Internals: handlers `on_create_action_triggered`, `on_delete_action_triggered`; `PageTemplatePart.delete_template`, `save_template_data`. There is a leftover debug button `u保存图片测试` writing `test/test.png` @94af7e2 (`InitTemplateManagementUI.__test`).
- Access: unknown.
- Confidence: high.

### 保存模板 (SaveTemplateData)
- Where: panel with `save.svg`, action `save_action` (@94af823).
- What it does: validates, then persists. Error messages (InfoBar warnings), exact strings:
  - `模板名称不可为空` @94b3797
  - `笔记模板中已经存在该模板名，请修改模板名称` @94b3979
  - `最小行数不能大于最大行数` @94b39f3
  - `页面宽度高度必须大于0` @94b3a24
  - `数据输入异常` @94b3a5a
  - `表格模板中已经存在该模板名，请修改模板名称` @94b3aa0
  - `存在空白页面，无法新建表格模板` @94b3b49 (a table template page with no table is "blank")
  - `行数+行间距+上边距的数值超出页面高度，请检查` @94b3be6 and `列数+列间距+左边距的数值超出页面宽度，请检查` @94b3c6c (作文格 grid overflow)
  - `作文模板中已经存在该模板名，请修改模板名称` @94b3cb5
  - status string `未保存` @94b3cf7
- Internals: note templates are pickled (`NewNoteTempFilePath`, `pickle.dump/load`), composition templates to `CompositionV2TempFilePath`, table templates through `TemplateAccessUtil` (`read_table_template`, `save_target_table_template`). "表格模板中参数过多，不合适实时预存到临时模板数据中，需要每次都从页面中生成来保存" @94b404a: table template data is regenerated from the scene on save, notes/compositions are kept in `temporary_template_data`. Saved table fields (identifier names): `Table_Template`, `page_numbers`, `show_background`, `background_opacity`, `same_cell_data`, `same_cell_record`, per page `PageItemInTemplate` {`page_index`, `bg_img_b64`/`background_img`, import region `left_top_column`, `left_top_row`, `right_bottom_row`, `right_bottom_column`}, per table `TableItemInTemplate` {`table_class`, `table_width`, `table_height`, `lock_pos`, `cell_info` from `generate_cell_info_for_template`}, `paper_mode` (`BlankMode`/`ContentMode`). Note fields: `page_width/page_height`, `min_row_num`, `max_row_num`. Composition: `cell_row_number`, `cell_column_number`, `cell_width`, `cell_height`, `row_spacing`, `column_spacing`, `left_margin`, `top_margin`. After saving, `save_success_signal` appends a `QStandardItem` (icon + UserRole data) to `model_of_writing_template`. Also `delete_local_template` ("删除本地保存的模板数据" @94b375d).
- Access: unknown (cloud sync not visible here).
- Confidence: high.

### 通用参数面板 (模板名称, 页面尺寸, A4/B5)
- Where: `TemplateGeneralParameterPannel` (group box `Ui_generalParametersGroup`).
- What it does: template name (`lineEdit`, "模板名称变动" @94aae9f), page width and height double spin boxes ("页面宽度输入框 - 值改变", "页面高度输入框 - 值改变"), two tool buttons for A4 ("A4 按钮点击" @94aae55) and B5 ("B5 按钮点击" @94aae7c) that fill `StandardPaperSize`. `load_default_data` "加载默认数据.和上面一样。只是模板名称需要选中" (name field gets focus and selectAll, for new templates). `get_general_parameter_of_template` returns template name and page size. Every change triggers `preview_template_when_param_changed` ("预览模板，一般用于参数改变时候（模板管理器初始化时候不在这里做预览触发，是在外层整体预览）").
- Internals: `parameter_changed_signal`; `is_quick_edit`, `in_param_init` guards.
- Access: free (as far as shown).
- Confidence: high.

### 文档(笔记)模板参数 (NoteTemplateParameterPannel)
- Where: group `Ui_noteParameterGroup`.
- What it does: edits page margins, row count. Docstrings: "上边距值改变, 左边距值改变, 下边距值改变, 右边距值改变" (`margin_top/left/bottom/right`; doubleSpinBox_3..6, _5/_6 are top/bottom or left/right pairs), "默认行数值更改, 最小行数值更改, 最大行数值更改" (`spinBox`, `spinBox_2`, `spinBox_3`), "设置行数模式" with two radio modes `DefaultRowMode` (single default row count) and `RangeRowMode` (min..max; widgets label_2/label_3 hidden by mode). Internals: `min_row_num`, `max_row_num`. Preview refreshed on every change.
- Cross-ref: protocol.md note layout uses a row range (min/max) for variable row count.
- Confidence: high for names, low for numeric ranges (not in strings).

### 笔记分栏设置 (NoteTemplateColumnSettingDialog)
- Where: ribbon note panel button with `column-2.svg` (@94af14b); dialog `Ui_NoteTempColumnSettingDialog`, always on top.
- What it does: column count via a horizontal `QListWidget` (1..n, flow LeftToRight, items not selectable but current row shown) driven by `spinBox` (`column_num`, `column_number_changed_signal`). Three check boxes: `checkBox_2` 显示分割线 (`show_dividing_line`, "显示分割线复选框点击事件"), `checkBox` 栏宽相等 (`column_width_equal`, "栏宽相等复选框点击事件"), `checkBox_3` 异栏同型 (`column_same_type`, "异栏同型复选框点击事件", i.e. all columns share the same cell type). Per-column spin boxes for column width and spacing (`spinBox_2..7`, `column_width`, `column_space`, `default_column_width`), "栏宽1 spinbox值改变时触发", "间距1 spinbox值改变时触发", "重新计算某个之后的栏宽 / 均分剩余的宽度、间距为0" (`__reset_average_column_width_after_column`, remaining width split evenly, spacing 0 after the edited column). "根据栏数设置默认的栏宽和间距数值" (`__set_default_column_width_when_changed_column_number`); "设置栏宽参数控件状态" (controls disabled for unavailable columns). Loads "书写区域尺寸" = page width/height minus margins (`write_region_width/height`) and `column_num`. Close button: `closeBtn`.
- Internals: only valid for note templates ("肯定是笔记模板才有这个功能，所以不用判断" @94b1a50). Preview via `build_template_preview`.
- Confidence: high (names), low for ranges.

### 双线文档 (NoteTemplateDualLineSettingDialog)
- Where: ribbon button `two_line_note.svg` labelled `u双线文档` @94af0d4 in the note panel titled `u文档格式` @94af080.
- What it does: a `groupBox` checkbox "启用双线模式被勾选/取消勾选" (`enable_dual_line_mode`), "上侧行高值更改" (`up_row_height`, one `doubleSpinBox`), `checkBox` "启用虚线分割" (`enable_dashed_line_partition`). Used for 双线格 (English-style double line). Row types in the preview/cell items: `TopNoteRow`, `MiddleNoteRow`, `BottomNoteRow`, `IndependentRow` ("设置单元格行数位置类型，一般用于笔记，第一行、中间行、最后一行" @94bf578); preview line drawing for double-line notes (`__paint_for_dual_line_note`, "绘制预览框线 - 双线笔记"; `__draw_dividing_line_for_note`, `__place_cell_for_dual_line_note`).
- Confidence: high.

### 作文格模板参数 (CompositionTemplateParameterPannel)
- Where: group `Ui_compositionParameterGroup`.
- What it does: grid of boxes (作文纸). Parameters (exact attribute names): `cell_row_number` (spinBox), `cell_column_number` (spinBox_2), `left_margin`, `top_margin`, `cell_width`, `cell_height`, `row_spacing`, `column_spacing` (doubleSpinBox 1..6; docstrings "格子行数变化, 格子列数发生变化, 左边距变化, 上边距变化, 格子宽度变化, 格子高度变化, 行间距变化, 列间距变化"). Overflow warnings: `行数+行间距+上边距的数值超出页面高度，请检查` @94a5d8e and `列数+列间距+左边距的数值超出页面宽度，请检查` @94a5e1f, shown as InfoBar warnings. Config key `NowWritingTempName` in the sample config is `作文格1`.
- Internals: `TemplatePreviewCompositionPaperItem` / `...CompositionCellItem`; "绘制额外的线条" (`set_extra_line_path`).
- Confidence: high.

### 表格图元位置/尺寸 (TemplateBaseItemPositionPannel)
- Where: group `Ui_TemplateBaseItemPositionGroupBox`; docstring "基础图元位置参数设置面板，目前改为表格图元参数设置面板" @94a56bc.
- What it does: for the selected table item: X position (`doubleSpinBox`, "横向坐标发生修改" @94a521b), Y position (`doubleSpinBox_2`, "纵向坐标发生改变"), table width (`tableWidthSpinBox`, "表格宽度发生改变"), table height (`tableHeightSpinBox`), and a `SwitchButton` "锁定基础图元位置按钮" (`position_lock`; on/off text via `setOnText/setOffText`, off text likely `u自由移动` @94a4f82 = "free move"). Values are rounded with `ToolUtils.custom_round` and scaled by `PXScale` (mm to pixels).
- Confidence: high.

### 表格单元格属性 (TemplateTableParentCellAttrPannel)
- Where: group `Ui_TemplateTableParentCellAttrGroupBox`.
- What it does: for the selected "母单元格" (parent cell): inner row count (`spinBox`, "内部行数发生变化" = number of writing rows inside the cell), cell width and height (`doubleSpinBox`, `doubleSpinBox_2`, "单元格宽度数值修改完毕", "单元格高度发生改变"), a 行数锁定 / 行数解锁 switch (`lock_rows`, strings `u行数锁定` @94a9c6a, `u行数解锁` @94a9c89: keeps the row count from being auto-redistributed when the cell height changes) and a 不导入 switch (`accept_import`, label `u不导入` @94a9d4c: cell excluded from content import, drawn with a "不导入标记", `cell_mouse.svg`), an OK button ("确认按钮点击事件"). Messages: `请重新选中单元格，未检测到选中单元格` @94a9f99, `未解析的单元格宽度情况` @94aa08f. The panel computes the cell's coordinate range in the table (`_get_position_range_of_parent_cell`) and resizes neighbouring cells (`change_width`, `change_height`, `target_table_item_width/height`).
- Confidence: high.

### 页面缩略图面板 (TemplateThumbnailPreviewPannel)
- Where: right-hand thumbnail strip with buttons and a context menu.
- What it does: buttons with tooltips (exact): `预览原始背景` @94a7a3b (icon `page_check.svg`; shows the imported background image, `get_bk_preview_parameter`), `上移选中页面` (`icon-arrow-top.svg`), `下移选中页面` (`icon-arrow-down.svg`), `删除选中页面` (`icon-minus.svg`), `增加空白页面` (`icon-plus.svg`), and an opacity slider labelled `透明度：` @94a7bff (background opacity). Prompts/messages: `请先选中页面缩略图再使用此功能` @94a7d53, `当前已经是第一页，无法上移` @94a7dbb, `当前已经是最后一页，无法下移` @94a7edf, `请至少保留一个页面` @94a7f31, confirm dialog title `删除确认` with text `确认要删除页面 <n> 吗？`, `待删除页面不存在，如有问题请联系客服` @94a8053, copy confirm `确认复制模板页面的第 <n> 页吗？（默认会复制为最后一页）`, add confirm `确认新增一个空白页面到最后吗？`. Thumbnail label right-click menu: `复制页面` @94a87ee, `删除页面` @94a8804 (also an `Excel导入配置` entry with `Excel.svg` @94a8812/@94a881d may be here or the ribbon, medium). Selected thumbnail style: `border-width: 2px;border-style: solid;border-color: #ABCDEF;`. Mouse release selects a page; `refresh_appoint_page_preview` refreshes one page.
- Confidence: high.

### 表格模板 (Table templates)
General: "表格模板" are forms. A page (`TemplatePreviewTablePaperItem`) holds table items (`TemplatePreviewTableItem`) made of "母单元格" (`TemplatePreviewTableParentCellItem`) which can contain several inner writing rows (`internal_rows`) and carry an internal margin (`internal_margin_left/top/bottom/right`, only left "暂时只用到左边", @94c5694). A page is in `BlankMode` (empty) or `ContentMode` ("设置纸张的模式，有内容模式和空白模式" @94c443b). Data layout versions: V2 (`core.DataModel.TableTemplateV2`, "加载V2版本表格内单元格数据（一般是打开本地模板数据）") and old V1 ("加载表格单元格cell（老版本表格模板数据）（一般是新建模板）", "v1老版本专用，主要是解析后端生成的数据").

#### 插入表格 (快速绘制 + 自定义行列)
- Where: ribbon panel "生成表格框架": `插入表格` (menu, `table_2.svg`, @94af2a9) with sub-actions `绘制表格` (`InsertTableMenu`, @94af2f7) and `截图生成` (`screen_shot.svg`, @94af348).
- What it does: the hover menu `InsertTableMenu` ("快速绘制表格部分的label", `GridLabel`) shows a grid of squares; moving the mouse highlights R x C ("接收label悬浮事件传输过来的信号:行数和列数"), tool tip `{}×{} 表格` @94ba73d, clicking inserts ("接收用户在label上单击事件，进行绘制表格"; "鼠标移开事件，用于没有创建表格时候的取消绘制"). The menu has a title bar styled `background-color: rgb(235,235,235);color: rgb(106,106,106);font:bold 15px` and a custom entry that opens `InsertTableTemplateDialog` ("自定义行数列数插入表格模板"): `spinBox` rows/cols, `radioButton` (fixed column width) vs `radioButton_2` (auto column width), error `列宽*列数超出页面宽度(还需保留最少2mm边距)，请重新设置` @94b9440. The common inserter `InsertTableToTemplatePaper.insert_table` requires a selected page (`请先在右侧缩略图中选中页面再添加` @94b9b0f), computes `fixed_column_width`/`auto_column_width`, `target_row_height`, start point and builds `cell_pos_info`, then `set_paper_mode(ContentMode)` + `load_cell`. The same insert menu is reused in the plotting ribbon (`PlottingAddTextFieldWidget.insert_table`).
- Confidence: high.

#### 截图生成 (ImportTemplateByScreenShotDialog + Screenshot)
- Where: ribbon `截图生成`; dialog `Ui_ImportTemplateByScreenShotDialog`.
- What it does: a full-screen frameless overlay (`QGuiApplication.primaryScreen().grabWindow`, cross cursor) lets the user drag a rectangle; internals strings: `初始化窗口`, `捕获全屏`, `鼠标按下事件/移动/释放/双击`, `绘制背景图`, `获取矩形选框`, `绘制选框`, `保存图片` (name template `%Y%m%d%H%M%S` + `.png`), Esc/Enter/Return key handling; right button cancels, double click confirms. Dialog buttons `生成并导入按钮点击` ("生成并导入"). Result image is sent to the same backend table-recognition path as file import.
- Confidence: high.

#### 导入 PDF/图片 生成表格框架 (import region + backend)
- Where: each blank table page shows an `ImportTableFrameRegionItem` drop zone: `将文件拖到此处，或点击导入本地文件` @94bd2a2 and `（PDF或图片表格）`, hover/drag text `检测到文件，松手即可导入` @94bd2ff, busy text `本地文件导入解析中` @94bd32f, count suffix `个文件`. Icon `open_file.svg`; animated marching-ants dashed border ("为了实现流动边框，增加dash偏移量"); handles drag enter/leave/drop. Clicking opens a file dialog `选择文件` with filter `PDF或图片(*.pdf *.jpg *.png);;PDF文件(*.pdf);;图片文件(*.jpg *.png)` @94ac086.
- What it does: `BuildTemplatePreview._start_generate_table_template_thread` starts `GenerateTableTemplateByFileThread` ("开始转换", params `file_list`, `start_page_index` = page where the import region is). It converts each file to `QImage` (PDF pages rendered via a PDF library with zoom matrix `zoom_x/zoom_y`, `pdf_doc`; images via `QImageReader`, extensions `.pdf .PDF .png .jpg`), base64-encodes (`_QImage_to_base64`, utf-8), and posts to the backend through `core.Util.NetworkUtil` to get `template_data` (table cell geometry); empty dict means failure. Messages: `不支持的文件格式(` + ext, `<n> 页表格解析异常，请手动创建该页面表格` @94add9e, `<n> 页表格解析失败~，请手动创建该页面表格` @94addf8, `处理失败` @94ac635. Orientation fix: `检测到导入的模板与页面方向不同，已为您切换` @94ac293 (page switches landscape/portrait automatically; `_revoke_switch` "撤销自动切换的表格方向模式"). Result per page: `_generate_table_template_page_done(page_index, template_data, img_b64)` builds the preview and stores the image as the page background; `append_blank_table_page` adds a blank page, `copy_table_page` duplicates one.
- Internals: position info keys `position_info`, `point_1`, `point_3`, `new_point_1/3`, `scale_value` (backend gives two corner points per cell which are scaled to the page). Endpoint not visible in this slice.
- Access: backend call likely needs login (not stated here).
- Confidence: high for flow, low for endpoint.

#### Excel导入配置 (TableTemplateImportConfigOfExcel)
- Where: ribbon `Excel导入配置` (`Excel.svg`, @94af481), form `Ui_TableTemplateImportConfigOfExcel`.
- What it does: per page, maps the table area to an Excel range: a page list (`ListView`, titles per page) and four edits: left-top column letter, left-top row number, right-bottom column, right-bottom row (`left_top_column/row`, `right_bottom_column/row`). Validation: `行号输入框中只能输入数字` @94b7e5e, `列号输入框中只能输入字母` @94b7e8d, `如需保存请填写所有所有输入框数据` @94b7df7, success `暂存成功` @94b7ed2. Used when later importing an Excel file into a table template (the region is stored per page).
- Confidence: high (names), medium (exact use of the range).

#### 同型标记 (TableTemplateMarkCellSameTypeDialog)
- Where: ribbon `同型标记` (`data_merge.svg`, @94b4ec1 region), dialog with left list "同型记录" and table "同型单元格".
- What it does: groups cells that must be filled with the same kind of content. Left list shows records named by a remark; right table lists the member cells (columns: page, table, cell). Buttons: new (`addBtn`, mode `AddMode`, "同型单元格[新建模式]"), delete (`确认要删除所选记录吗？`), up/down (`当前已经是第一行，无法上移`, `当前已经是最后一行，无法下移`), save. Rules/messages: `请使用Ctrl+鼠标左键，或者鼠标滑动多选单元格` @94b5364, `请输入备注名`, `请先选中单元格再保存`, `请选中至少两个单元格`, `选中的单元格存在于其他同型记录中，请检查`, `同型标记中，暂不支持前页单元格是后页单元格的同型` (a cell on an earlier page cannot be the "same as" a later one), `同型标记单元格配置异常`. Data: `same_cell_data` / `same_cell_record`, `cell_class` ids shaped `page_<n>_table_<m>_cell_<k>`; selected cells are highlighted with `set_select_in_set_same_cell` (orange `#FF9966`).
- Confidence: high.

#### 数据源配置 (TableTemplateDataSourceDialog)
- Where: ribbon `数据源配置` (`data_source_transform.svg`, @94af535), form `Ui_TableTemplateDataSourceDialog`.
- What it does: binds a data source to ONE selected cell (`暂时数据源设置不支持多选单元格` @94bb06f; `请先选中单元格`). Source types (a list widget, identifier names): `NoDataSource`, `DefaultValueDataSource` (`default_value`, error `默认值未填写，未保存`), `RandomValueDataSource` with `random_mode` of `RandomWithinValue` (random number between `min_value` and `max_value`, `decimal_places`; errors `小数点后位数不能小于0`, `最小值不能大于最大值`) or `RandomWithinContent` (random pick from `content_list`, entered in a `textEdit`, one per line), and `CustomCellDataSource` (`row_num`, `col_num`: an Excel-style cell reference, i.e. take the content from another cell). Messages: `信息输入不全`, `请填写完整的数据`, `保存成功`. Saving stores into the cell's `data_source_type`/`data_source`/`default_value`/`random_mode`/`decimal_places`/`min_value`/`max_value`/`content_list`.
- Cross-ref: the writing side that consumes these is outside this slice.
- Confidence: high for types, medium for CustomCellDataSource semantics.

#### 合并单元格 / 拆分单元格
- Where: ribbon `合并单元格` (`merge_cell.svg`), `拆分单元格` (`cancel_merge_cell.svg`) with tooltip `目前版本是完全拆分解组单元格，不是取消合并上一次合并的单元格` @94af606 ("this version fully splits cells into a group; it does not undo the last merge"); also in the table right-click menu (`merge_cell_signal`).
- Internals: `PageTemplatePart.merge_table_cell`, `cancel_merge_table_cell`. Table items also support dragging split handles between cells (`splitHandleAt`, `interactiveSplitTableCell`, `updateTableCellSplitHandlesData`: "对于单元格，只考虑某个单元格的下、右进行处理。如果是最底下、最右边的单元格，则不进行处理"), and corner/edge resize handles (`interactiveResize`, `handleAt`, `lock_item_position`). Item moves are clamped inside the canvas (`itemChange`; "注意：itemChange中严禁调用setPos和moveBy"). Selection: only paper children are selectable ("目前默认只有纸张中的子图元项才能被选中，选中的图元的父级图元一定是纸张"), Delete deletes ("按下Delete删除按钮，目前一般在表格模板功能中使用").
- Confidence: high.

#### 内边距识别 / 重置内边距 (TableCellPaddingIdentification)
- Where: ribbon `内边距识别` @94af702 (identification region panel) and table right-click menu items `内边距识别` @94b7213, `重置内边距` @94b722a, `合并单元格` (from `ContextMenuOfTableTemplate.get_context_menu`, round `RoundMenu`).
- What it does: auto-detects the left padding of cell content from the page's background image. Errors: `请先选中表格单元格再识别` @94b63cd, `所在页面没有背景图，无法识别` @94b6580. Algorithm (identifier-level): decode `bg_img_b64` with PIL, crop the cell rectangle (`cell_pos_x/y_in_page`, `cell_max_x/y_in_page` scaled to image pixels), build a numpy array, find the leftmost "ink" pixel per row (`target_value`, `max_pixel_x_of_char`, `target_max_x_of_char`, `np.array_equal`), then `set_internal_margin(margin_left)`; supports batch via `identify_padding_of_selected_cells`. "占位符识别" (`kongbai.svg`, `on_placeHolderIdentificationAction_triggered`, `_place_holder_recognition`) is a stub whose docstring is "占位符识别功能".
- Confidence: high (flow), medium (exact thresholds unknown).

#### 模板预览视图
- View/scene: `PageTemplatePreviewScene` (`add_item`, `drawBackground`, Delete handler) and `PageTemplatePreviewView` (wheel zoom with factor, `zoomIn/zoomOut/zoom`, `@u缩放`, right-click `contextMenuEvent`, mouse release). Paper items: `TemplatePreviewPaperItem` (notes, page number, extra line path), `TemplatePreviewCompositionPaperItem`, `TemplatePreviewTablePaperItem` (can render to pixmap for thumbnails, "转换为pixmap数据"). Cell colours: `#c0c0c0 #d0d0d0 #e0e0e0` (note rows), `#707070`, `#FF9966` (selected), `#D5E394` (parent-cell highlight), `#ABCDEF` (selected thumbnail). Hover tooltip text "重载hover进入".
- Template list: `TemplateListViewRegion` with model of items (icon + name), selection signal `template_selected_signal`, reading saved templates from disk.
- Confidence: high.

---

## B. Plotting and laser editor (`core.PlottingAndLaserPart`)

### 绘图与激光 ribbon category
- Where: category title `u  绘图与激光  ` @950003a (`InitPlottingAndLaserUI`, 2024-04-02 file header). Panels: `画布数据` @95000fa (paper type drop-down, icon `blank_page.svg`; widget `PlottingPaperTypeSettingWidget` shown in a menu), `模板库` @95001ef (`TemplateManagement.svg`: opens the template manager, section A), `数据添加` @9500220 (three embedded regions: `PlottingDataImportWidget` = import, `PlottingInsertBasicGraphicsWidget` = basic graphics, `PlottingAddTextFieldWidget` = text fields), a creative-function button (`idea_1.svg`, `plotting_idea_action`; label not in slice) opening `IdeaFuncMenu`, and `AI板块` @9500417 with `AI绘图` @950046e (`ai_plotting.svg`, `ai_plotting_action`) and a `personification.svg` entry (`plotting_personification_action`, label elsewhere).
- Internals: main widget `PlottingAndLaserPart` = side toolbar + `PlottingGraphicsView` + `PlottingGraphicsScene`, centred on the paper (`_set_view_center_on_paper`), plus a hidden `PlottingInputBox` (QLineEdit capturing typing). Signals bridge the controllers (`PlottingInputNestingSignal`, `PlottingSceneSignal`, `PlottingIdeaFuncExtraSignal`, `PlottingSideToolbarSignal`, `PlottingPagePreviewSignal.load_all_preview_signal`).
- Config: `[DrawConfig] DrawOrLaser=Draw` (the other value is presumably `Laser`; no laser UI is in this slice), `PaperType=StandardPaper`, `PaperName=A4`, `PaperOrientation=Horizontal`, `CustomPaperSize` (dict name -> (w, h) pickled in QSettings), `CAD_Import_Mode=1`.
- Access: typing text into plotting text fields requires login (`用户未登录，请先登录` @94dcfc9, @94e0bdf, @94e1bdb).
- Confidence: high.

### 画布数据 / 纸型设置 (PlottingPaperTypeSettingWidget)
- Where: drop-down menu from the `画布数据` button.
- What it does: two group boxes toggle between standard paper (`groupBox`: `ComboBox` of `StandardPaperSize` names, plus orientation radio `HRadioButton` = Horizontal / a vertical radio = Vertical) and custom size (`groupBox_2`: `EditableComboBox` of saved custom names, `DoubleSpinBox` width and `DoubleSpinBox_2` height, `保存` and `删除` (`deleteBtn`)). Messages: `纸张名称不能为空` @94d5d51, `待删除的页面名称不可为空` @94d5ddc, `当前页面名称尚未保存，无法删除` @94d5e0e. Config keys written: `PaperType` (`StandardPaper`/`CustomSizePaper`), `PaperName`, `PaperOrientation` (`Horizontal`/`Vertical`), `CustomPaperSize`. Changing it emits `paper_size_changed_signal` -> `PlottingGraphicsScene.set_plotting_paper_size` -> `set_page_size_done_signal`. The menu stays open after clicks (`mouseReleaseEvent` trick "menu菜单中，通过这个方法来关闭menu，再次阻止点击后关闭行为"). Paper is `PlottingPaperItem`: border pen width 0.6 (`f333333` = 0.6), optional grid, cache mode DeviceCoordinateCache.
- Standard names/sizes come from `core.Constant.StandardPaperSize` (not in this slice).
- Confidence: high.

### 侧边工具栏 (PlottingSideToolbar)
- Where: vertical toolbar left of the canvas, five checkable tool buttons with pointer cursor.
- Tooltips (exact): `鼠标框选模式` @94d68af (`select.svg`; rubber-band selection, default), `鼠标拖动画布模式` @94d6908 (`drag.svg`; hand drag), `启用画布网格` @94d6960 (`grid.svg`; toggles the paper grid, default spacing 5 mm: "网格间距，默认为5毫米" @94f76cd), `启用网格点吸附功能` @94d69d5 (`grid_point_adsorption.svg`; toast `已启用网格吸附` @94d6b83 / `已关闭网格吸附` @94d6b9a), `打开图层属性设置` @94d6a38 (`layer_attr.svg`). Enabling grid snapping sets `PlottingEnableGridAdsorption` global; the controller then shows a snap marker (`PlottingAdsorptionPointItem`, orange `#FF9966` ring + `#20A2DD`) at the nearest grid intersection (`grid_x_value_list`, `grid_y_value_list`, `pos_in_paper`) while drawing/inserting.
- Confidence: high.

### 基本图形 (basic graphics) drawing
- Where: embedded region "基本图形" (`#PlottingBasicGraphicsWidget`, light-blue 1px border `#87CEFA` radius 5 when expanded; first button "图形按钮点击，展开基本图形绘制区域"). Buttons (names): `importBasicGraphicsBtn` (expand/collapse), `insertPointBtn`, `insertLineBtn`, `insertRectangleBtn`, `insertRoundBtn`, `insertPolylineBtn`, `insertCurveBtn`, `insertPolygonBtn`, `insertArcByCenterBtn` ("中心点圆弧绘制按钮点击"), `graphicsAttrSettingBtn` ("图形属性窗口点击").
- What it does (mouse model, from the controller handlers): all tools are click/drag on the paper with snap and a live preview; cursor is a cross while drawing, view drag is cancelled.
  - 点: one click.
  - 直线: press = start, move = preview, release/second click = end ("鼠标释放事件，目前一般只用于手动绘制图形时候，鼠标第一次按下就立即拖动的情况": press-drag-release also works). A helper "计算AB方向上距离A点长度为L的C点坐标".
  - 矩形: two corners (press-drag or two clicks).
  - 圆形/椭圆: centre then radius (`PlottingRoundItem`, ellipse params `ellipse_a`, `ellipse_b`; `圆` and `椭圆` share one item via `set_ellipse_ab`).
  - 三点/中心点圆弧: centre, start point, end point (`PlottingArcItem.set_arc_param`, `get_angle`; "处理绘制三点圆弧时候的鼠标按下事件").
  - 多边形 (regular polygon): centre then one edge midpoint/vertex defines size; edge count and "内切圆圆心距" are editable afterwards (`PlottingPolygonItem.set_polygon_param`, `generate_polygon_from_edge_midpoint`, `compute_new_b`, `reset_center_distance`); the attribute dialog opens in polygon mode (`init_polygon_plotting_mode`, "初始化进入多边形绘制模式").
  - 折线 (polyline, 多段线): successive clicks add vertices; Enter finishes ("回车按下，目前只对于多段线绘制生效"); Backspace-like undo of the last vertex via `_delete_last_point_of_ploy_line` ("删除折线段的最后一段（即最后一个点）"); Esc cancels.
  - 曲线 (cubic Bezier path): press fixes the new anchor 1 and the previous segment ("鼠标按下，确定新曲线锚点 1 位置，以及上一条曲线；"), drag sets control handles; a predicted segment/control is previewed (`append_predict_curve_segment`, `predict_ctrl_item_1/2`, `refresh_curve_preview`); data class `PlottingCurveSegment`/`PlottingCurveData` (`core.DataModel.PlottingCurve`: start, control1, control2, end).
- Item classes: `PlottingPointItem`, `PlottingLineItem`, `PlottingRectangleItem`, `PlottingRoundItem`, `PlottingArcItem`, `PlottingCurveItem`, `PlottingPolylineItem`, `PlottingPolygonItem`, `PlottingSVGPathItem`, `CustomGraphicsItemGroup`. All derive from `PlottingItemBase` (handles, hover, `setPos` override); selection shows orange `#FF9966` control points, blue `#20A2DD`, grey `#8a8a8a`, black `#000000`. Hit-test shape is enlarged for thin lines ("重构shape方法，用于模拟扩大直线图元的boundingRect，方便选中"). Item types in the enum `ChiXie`: `GraphicPoint, GraphicLine, GraphicRectangle, GraphicRound, GraphicPolyline, GraphicCurve, GraphicPolygon, GraphicArc`, plus `PlottingTable`, `PlottingTextBox`, `PlottingIdeaSeal`, `PlottingIdeaMaze`.
- Confidence: high for tool list, medium for exact click order of each tool.

### 图形属性 (PlottingGraphicsAttributeDialog)
- Where: toolbar button "图形属性窗口点击"; a stay-on-top tool window docked to the screen's top-right ("将窗口移动到屏幕右上角偏移 (offset_x, offset_y) 处", `move_to_top_right`). `stackedWidget` page depends on the selected type; type names (`graphic_name_dict`): `点, 直线, 圆/椭圆, 多边形 @94d773f, 矩形, 折线(多段线) @94d7775, 曲线, 圆弧, 表格, 文本框 @94d77c6`.
- Fields (widget -> meaning): `posXSpinBox`/`posYSpinBox` position (mm), line `lineP1XSpinBox/lineP1YSpinBox/lineP2XSpinBox/lineP2YSpinBox`, ellipse axes `ellipseParamASpinBox`/`ellipseParamBSpinBox`, rectangle `rectWidthSpinBox`/`rectHeightSpinBox`, polygon `polygonCenterDistanceSpinBox` (inner-circle radius, "多边形-内切圆圆心距变化") and `polygonEdgesSpinBox` (edge count, "多边形-边数变化"), and `lineTypeComboBox` (see next). The header is a `StrongBodyLabel`. Values are in mm and converted with `PXScale`. Live-updating the dialog while moving the selected item: `load_graphic_param_signal`.
- Confidence: high.

### 线型 (CAD line types for lines)
- Where: `lineTypeComboBox` in the attribute dialog; defined in `cad_line_type_define.py` (`CADLineTypeDefineDict`, `get_line_type_segment`, error `未知线型：` @94e845f).
- Items (identifier order): CONTINUOUS, CENTER, CENTERX2, CENTER2, DASHED, DASHEDX2, DASHED2, PHANTOM, PHANTOMX2, PHANTOM2, DASHDOT, DASHDOTX2, DASHDOT2, DOT, DOTX2, DOT2, DIVIDE, DIVIDEX2, DIVIDE2. Chinese labels in the binary: `中心线, 中心线X2, 中心线2, 虚线X2, 虚线2, 双点划线, 双点划线X2, 双点划线2, 点划线, 点划线X2, 点划线2, 点线X2, 点线2, 双点划线（长）, 双点划线（长）X2, 双点划线（长）2` (plus `虚线`, `点线`, `连续线` strings shared). Exact name-to-CAD-name mapping for 双点划线/双点划线（长） is uncertain (PHANTOM vs DIVIDE), medium confidence. DXF names also seen: ACAD_ISO02W100, ACAD_ISO03W100, ACAD_ISO05W100, BORDER2. `PlottingLineItem.generate_lines_by_line_type_param` turns the dash pattern (lists of floats such as `[fffffff...]` pattern constants) into separate segments and `generate_center_lines` handles CENTER. Dashed lines are real strokes, i.e. the plotter pen lifts between dashes.
- Confidence: medium-high.

### 图层 (PlottingLayerAttrDialog)
- Where: toolbar button "打开图层属性设置" (opens `Ui_PlottingLayerManagementDialog`, window title lists column `图层名` @94d8755).
- What it does: a table with one row per layer: name, visibility eye (`FluentIcon.VIEW`/`HIDE` toggle `is_visible`), lock (`lock-1.svg` / `unlock-1.svg`, `is_locked`), and row-order controls. Buttons (icons): `addLayerBtn` (`plus.svg`), `reduceLayerBtn` (`delete.svg`), `mergeLayerBtn` (`merge_cell.svg`), `moveUpBtn` (`icon-arrow-top.svg`), `moveDownBtn` (`icon-arrow-down.svg`). Model: `core.DataModel.PlottingLayer.PlottingLayers` (`add_item_to_layer`, `layer_order`, `LayerAttr`), initial layer via `PlottingLayerOperation.init_plotting_layer` ("初始化绘图功能的图层"). Debug prints `Button clicked!` remain in code.
- Confidence: high (UI), medium (layer semantics when plotting).

### 导入 SVG / CAD (DXF)
- Where: embedded region `数据导入` (`importPlottingDataBtn` toggles, `importSvgBtn`, `importCadBtn`), `#PlottingDataImportWidget` border `#87CEFA`.
- Dialogs: `选择SVG文件` with filter `SVG文件 (*.svg)` @94d08ca; `选择CAD文件` with filter `CAD文件 (*.dxf)` @94d098c ("导入CAD文件，目前默认只支持dxf类型"); the file dialog starts at `[FileConfig] DefaultFolderPath` and remembers it (`getDefaultFolderPath`/`setDefaultFolderPath`). While parsing: indeterminate progress ring dialog `请耐心等待...` / `正在解析文件 ...` @94d09e4. Parsers run in a `QThreadPool` (`QRunnable`), errors go through `error_call_back` (`PlottingDataImportSignal.error_signal`).
- DXF (`ChiXieDxfParser` using `ezdxf`): reads modelspace + blocks (`处理布局中的实体`, `处理块引用`), handled entity types and converters: LINE, MLINE, CIRCLE, ARC, ELLIPSE, SPLINE (-> cubic curve via `set_points_from_dxf_spline`), LWPOLYLINE, POLYLINE, POINT, SOLID ("二维实心填充区域", vertex order v0,v1,v3,v2), HATCH (`hatch_lines`, `solid_fill`, `polyline_path`), TEXT/MTEXT/ATTDEF (turned into `PlottingTextBoxItem` with `attachment_point` MTEXT_TOP_LEFT.. MTEXT_BOTTOM_RIGHT, char height, rotation), MULTILEADER, DIMENSION, TOLERANCE (parser only), ACAD_TABLE, INSERT (block expansion with `insert_pos`, `xscale`, `yscale`, `rotation`, virtual entities); unknown entities go to `handle_unknown` (`未能解析的类型：`, `Unsupported entity type:`). Also resolves linetype, lineweight, colour per entity and layer name. Units: reads `$INSUNITS` and applies `unit_factor_dict`; unsupported unit -> `dxf 单位 ... 暂不支持此文件所用的单位，已使用“毫米”解析` @94cf066 (mm fallback). Errors: `dxf解析异常` with `错误：无法打开文件或文件无效`, `错误：DXF文件结构无效`, `错误：DXF文件属性异常`. Scaling: `get_model_space_limits` computes the drawing extent (`dxf_min/max`, `dxf_width/height`) and `calculate_scale_size` ("分析dxf数据是否需要缩放") shrinks it to fit the paper; Y is flipped (`flip_y`); the result is a `CustomGraphicsItemGroup` that can be moved/resized.
- SVG (`ChiXieSVGParser`, library `svgelements`): reads viewBox, applies the transform matrix to Path segments ("通过应用单位变换矩阵，将 Path 对象中的所有路径段转换为绝对坐标"), converts Move/Line/QuadraticBezier/CubicBezier/Arc (via cubic approximations `as_cubic_curves`)/Close, plus Circle, Ellipse, Rect, Polygon, Polyline, Line elements (some are "todo": `todo: Rect`, `todo: Ellipse`, `todo: Line`, `todo: arc` log lines mean not fully handled). Output `PlottingSVGPathItem` with stroke width and fill colour (`set_fill_color`: QColor, "#FF0000", or colour name), scaled into a group via viewBox offset (`offset_x/y`, `svg_width/height`).
- Config: `CAD_Import_Mode` (settings dialog `comboBox_4`; meaning of 0/1 not in this slice, config value is `1`).
- Confidence: high for entity list, medium for CAD_Import_Mode meaning.

### 创意功能 (酷玩) menu
- Where: `IdeaFuncMenu` (RoundMenu shown below the button "创意功能、酷玩按钮点击"). Entries: `印章生成器` (`seal.svg`, @94d2c7a), `分形艺术生成器` (`wolfram.svg`, @94d2cfa), `迷宫生成器` (`maze.svg`, @94d2d1b), `立体字` (`立体-K.svg`, @94d2d3a). Only the seal and maze actions have handlers (`on_sealGenerator_triggered`, `on_mazeGenerator_triggered`); 分形艺术生成器 and 立体字 have no handler here (planned/unlocked elsewhere), medium confidence.
- Insert flow (`PlottingIdeaFuncController`): choose generator -> click on the paper to place (snap supported; "插入...点击后直接完成") -> a config dialog opens (`show_seal_config_dialog_signal`/`show_maze_config_dialog_signal`) and edits update the item live (`load_seal_item`/`load_maze_item`). Cancel with Esc.

#### 迷宫生成器 (PlottingIdeaMazeConfigDialog + MazeGenerator)
- Dialog (always on top): `shapeTypeComboBox` items `矩形迷宫` @94d430c / `圆形迷宫` @94d431a (enum RECTANGLE/CIRCLE), `complexComboBox` items simple/medium/complex (`简单/中等/复杂` labels), `widthSpinBox` `heightSpinBox` (rectangular size mm), `radiusSpinBox` (circular radius), OK `pushButton`. Item `PlottingMazeItem` stores `maze_width/height/radius/complexity`; pen black, `setWidthF`.
- Algorithm: randomized Prim's algorithm ("使用Prim算法生成迷宫", "圆形形迷宫的Prim算法"). Rectangular: grid of cells (cell size from complexity, "根据复杂度确定单元格大小"), `horizontal_walls`/`vertical_walls`, `start_point`, `end_point` (entrance and exit gaps, `entrance_rad`/`exit_angle` for circular). Circular: concentric `rings` x `sectors`, `radial_walls`/`circular_walls`, `ring_spacing`, outer circle; output is a list of polylines (arcs sampled with sin/cos), so the plotter draws it as strokes.

#### 印章生成器 (PlottingSealConfigDialog + PlottingSealItem)
- Dialog: `sealTypeComboBox` items `圆形印章` @94d4d11, `椭圆印章` @94d4d1f, `方形印章` @94d4d2d; `sealImageComboBox` centre symbol: `五角星` @94d4d58 (`five-pointed-star.svg`, drawn with `QSvgRenderer`) and possibly others (`center_symbol_dict`); `plainTextEdit` the seal text (default text `炽写酷玩印章` @94f625a); `textAngleSpinBox` (`text_offset_angle`), `textSpaceSpinBox` (`spacing_factor` letter spacing), `fontComboBox` (font family; default `SimSun`, bold). Item draws: border (circle/ellipse/rect, antialiased), text along the circle (`drawCircularText`: "文字紧密排列", per-char angle from `QFontMetrics`), along the ellipse (`drawEllipticalText`), along the four sides of a square (`drawSquareText`: `chars_per_side`, `remainder`), and the centre symbol (`drawCenterSymbol`: "支持图片和矢量图形"). Can be converted to a pixmap.
- Confidence: high.

### 文本域 (文本 / 文本框 / 表格 in plotting)
- Where: embedded region `PlottingAddTextFieldWidget` (`#PlottingTextFieldWidget`): buttons `insertTextFieldBtn` ("文本按钮点击"), `insertTextBoxBtn` ("文本框按钮点击"), `fillingTextFieldBtn` ("填充文本域按钮点击") and a table insert menu `insert_table` reusing the `InsertTableMenu` grid (section A).
- What it does: clicking the paper inserts directly (no second click: "插入文本域这里做的简单点，直接是插入，而不是两次点击确定尺寸。所以是点击后直接完成插入"). Types: text box `PlottingTextBoxItem` ("绘图模式下的文本框模式", "手写模式的文本框": a box of stacked inner rows = `PlottingCellItem` rows, `row_type_dict` IndependentRow/Top/Middle/BottomNoteRow, insertion point mode `attachment_point` with the nine `MTEXT_*` anchors, `refresh_pos_by_attachment_point` after typing, "删除末尾的空行", auto width `calculate_content_text_max_width`) and plotting table `PlottingTableItem` (rows x cols of `PlottingTableParentCellItem`, resizable with split handles; same behaviour as template tables, see A). Resize handles show a dimension line with arrows and inline editable width/height ("开始编辑宽度/高度", `InlineLineEdit`, `2f` format, text background; double click a dimension to type a value). `PlottingTextFieldSetting.add_child_cell_to_field` adds a cell/row.
- Typing/typesetting: the hidden `PlottingInputBox` receives keys (Delete, Backspace, Space/half-space `HalfSpaceFunction`, Tab/Backtab, Enter, arrows, Shift+arrows select char/line, Ctrl+C/X/V/A/Z via `Copy/Cut/Paste/SelectAll/UndoFunctionOfWrite`, Period key, context menu) and runs `PlottingInputNesting` in a `QThreadPool`: splits text by paragraph, converts to `TextItem`/`SpaceItem`/`LineBreakItem` via `core.WritingPart` (shared with the writing function: composition and note modes), inserts at the cursor, re-nests the following content "涉及迭代", chooses the row from cursor position and item width, and adds a new row/cell when the box is full. Plotting text is therefore handwriting-font strokes (login required). `ReNestingOfPlottingTextField` re-lays out a box; vertical offset via `set_vertical_offset_of_text`.
- Alignment (`PlottingInputAlignmentFunction`): `请先放置光标或者选中字符再使用对齐功能` @94db76a, `请先选中单元格`; sets paragraph-at-cursor, whole-cell or single-cell alignment (left/centre/right, `h_alignment`), recomputing content width (`_calculate_content_width_in_cell`) and item positions, refreshed after every input.
- Cursor handling: `PlottingCursorUtil` (item before/after cursor, paragraph at cursor, "取出光标后、本段落中剩下的图元", "后向搜索停止条件： 需要向后搜索直到出现回车符或者空行"), cursor placement in note/table cells even when empty depending on alignment (`_get_target_cursor_pos_x_when_no_content`).
- Confidence: high for classes, medium for UI labels.

### 画布: 视图, 缩放, 标尺, 快捷键
- View (`PlottingGraphicsView`): antialiased, `RubberBandDrag` default, `ScrollHandDrag` in drag mode, anchor under mouse, cache background; keys: Esc (cancel drawing/insert/drag, `__key_press_of_esc`), Enter/Return (`__key_press_of_enter`, finishes polyline), Delete (`__key_press_of_delete`, deletes selected). Wheel: `zoomIn_discrete`/`zoomOut_discrete` through fixed `zoom_levels` ("离散放大/离散缩小", `zoom_to_level` "直接缩放到指定级别"); the view keeps the layout centred on the paper (`_set_view_center_on_paper`).
- Rulers (`PlottingRulerWidget`, horizontal + vertical): units from `get_unit_size` (mm base, DPI aware), major/medium/minor ticks (`major_tick_length`, `minor_tick_length`, `medium_tick_length`), tick interval picked from a minimum pixel spacing ("根据最小间距选择合适的刻度间隔", `get_base_interval`), labels formatted by `format_value`, mouse position indicator (`show_mouse_indicator`), and shaded range marks for the selected items (`set_mark_items`, "更新标尺上的标记", `mark_color`/`mark_border_color`). Rulers update on scroll, resize, zoom, selection change.
- Selection feedback: `on_selection_changed` updates the ruler marks.
- Confidence: high.

### 几何工具 (拟合, 二值化, 扫描线)
- 二次/三次 Bezier 拟合 (`PlottingQuadraticBezierFitterUtil`, `PlottingBezierFitterUtil`): points sampled uniformly by arc length, "适配Gcode需求": `bezier_point(t)`, `bezier_derivative`, `arc_length_segment` (numeric integration, chord approximation), `total_arc_length`, `find_t_for_s` (bisection, `eps`, `max_iter`, "保证点集按弧长均匀分布"), `generate_bezier_points(segment_item, x_scale, y_scale, segment_length)` where `segment_length` is the target segment length (smaller = more points). Input dict keys: start, control (quadratic), control1/control2 (cubic), end.
- 圆/椭圆拟合 (`PlottingCircleFitter`, `PlottingEllipseFitter`): `center`, `radius` (or a/b semi-axes), `start_angle` (default 0 deg = 3 o'clock), `rotation_angle` (ellipse), `include_start_twice` (default True so closed shapes close), `generate_points`, `generate_points_with_angle_range` (arcs), `get_fitting_info`, and optimal point count from chord-height ratio (`max_chord_height_ratio`, `max_segment_angle`, `max_points`, uses `acos`). Errors: `半径必须为正数`, `长半轴和短半轴必须为正数`.
- 图像二值化 (`ImageBinarizer`): loads from file/array/QPixmap, grayscale, `global_threshold` (0-255, `>= threshold -> 255 white else 0 black`; error `阈值必须在0-255范围内，当前值: `), `otsu_threshold` (between-class variance), `save_result`; also `PlottingImageScanner.process_image(pixmap, scan_spacing, threshold=None)`: gray via luminance formula, Otsu when threshold is None, then `generate_scanlines` produces x-direction scan lines (start/end segments of dark runs, row spacing `scan_spacing` px, error `扫描间距必须大于0`). This is an image-to-strokes (raster hatching) primitive; no caller UI found in this slice (probably used by the AI/image functions elsewhere).
- Confidence: high for algorithms, low for where they are surfaced.

### 点/节点图元
- `PlottingAdsorptionPointItem` ("绘图中的网格吸附点图元") and `PlottingNodePointItem` ("节点图元，一般是各个图形线上的节点"; `NodeType`/`ControlType`, `inner_radius`, cross cursor on hover) are the small handle markers shown on selected shapes (anchor vs control points).

---

## C. Settings dialog (`core.Setting.SettingDialog`, singleton `__new__`)

Caveat: the dialog is a Qt Designer form (`ui.UI_SettingDialog`), whose labels are NOT in the constants. Below, widget names are mapped to config keys and to `[section]` in `config_sanitized.ini`. Reading is in `initData` ("初始化时候读取配置文件"), writing in `saveData` ("保存设置到配置文件") on close (`event` handles `QEvent.Close` -> `saveData`). A "?" help button (WhatsThis event) opens `https://chixie.com.cn` @95046f5.

### 通用 (GeneralConfig, DrawConfig.CAD_Import_Mode)
- `comboBox` -> `Default_Function` (default function at start; `WritingFunction` is the value-0 meaning; config `0`).
- `comboBox_2` -> `Synchronization_Parameters` (cloud parameter sync mode, config `0`).
- `comboBox_3` -> `Language` (config `0`).
- `comboBox_7` -> `DeviceManagerStatus` (device manager drawer start state; one value name is `DefaultHide`, config `0`).
- `comboBox_4` -> `[DrawConfig] CAD_Import_Mode` (`CADImportMode`, config `1`).
- Confidence: medium (the option text is unknown).

### 写字偏好 (WriteConfig)
Spin boxes are plain line edits (`lineEdit_*`), global variables are in `core.Variable` (names below). Defaults from the sample config:
- Character spacing, Chinese: `min_char_spacing_zh`=0.6, `max_char_spacing_zh`=1.1 (`lineEdit_8`/`lineEdit_9`; globals `MinCharSpacingZh`/`MaxCharSpacingZh`; code default literal 1.x, `f333333 ?` = 0.6).
- Character spacing, English: `min_char_spacing_en`=0.5, `max_char_spacing_en`=1.0 (`lineEdit_10`/`lineEdit_7`).
- Space width, Chinese: `min_space_width_zh`=5.0, `max_space_width_zh`=7.0 (`lineEdit`/`lineEdit_5`); English: `min_space_width_en`=2.0, `max_space_width_en`=3.0 (`lineEdit_2`/`lineEdit_6`) (mm, random in range per space).
- Vertical offset: `min_vertical_offset`=0.0, `max_vertical_offset`=0.3 (`lineEdit_12`/`lineEdit_13`); check box `intelligent_vertical_offset`=false (global `EnableIntelligentVOffset`; `checkBox`).
- First-line indent: `min_first_line_indent`=15.5, `max_first_line_indent`=17.0 (`lineEdit_14`/`lineEdit_15`; globals `MinFirstLineIndent`/`MaxFirstLineIndent`).
- 首字不对齐: `first_text_not_align`=true (`firstTextNotAlignCheckBox`, string-to-bool) and `first_text_not_align_range`=2.0 (`firstTextNotAlignRangeLineEdit`; globals `FirstTextNotAlign`, `FirstTextNotAlignRange`).
- 路径规划: `path_planning_type` (radio `radioButton_3`/`radioButton_4`: `PathPlanningZhi` 之字型 vs `PathPlanningGong` 弓字型; docstring "获取路径规划方式，之字型还是弓字型", config value `2`; the mapping of 1/2 is not stated), global `PathPlanningType`.
- Route improvement: globals `RouteImproveMode`/`RouteImproveValue` are saved by `save_write_config_to_gv` but have no config key listed here.
- Other keys in the sample config not handled in this dialog: `row_offset_style`, `random_correct_offset`, `start_point_mode`, `PreviewMode` (set elsewhere).
- Side effect on save: `writing_paper_items ... set_origin_point_item_pos` is called (the origin point item on writing papers is repositioned when settings change).
- Cross-ref: protocol.md layout rules use these as random ranges.
- Confidence: high for key mapping, low for UI widget order for en/zh pairs (assignment inferred from the order the code reads them).

### 多色绘制 (MulticolorDrawing)
- `groupBox_4` check -> `EnableMulticolorDrawing` (false); radios `radioButton` = `ManualPenChange` (手动换笔), `radioButton_2` = `AutomaticPenChange` (自动换笔) -> `PenChangingMode` (config `1`). Saved to globals by `save_multi_color_config_to_gv`.
- Confidence: high.

### 字符变形 (CharacterDeformation)
- Keys: `enable_base_deform` (group `groupBox_7`, basic affine deformation) and `enable_shape_deform` (`groupBox_8`, shape deformation, TPS thin-plate spline: `CharacterBaseDeformation`, `CharacterTpsDeformation`, `enable_tps_deform`, `tps_degree`); horizontal scale range `min/max_horizontal_scale` (96..104 in config, percent; `spinBox`..`spinBox_4` cover the h/v scale min/max), vertical scale `min/max_vertical_scale` (98..102), rotation `min/max_rotation_angle` (-2..2 deg, `doubleSpinBox`, `doubleSpinBox_2`), `stroke_deform_degree` (slider `horizontalSlider_2`, config 1; applied via `set_stroke_deform_parameter_by_level`), `shape_deform_degree` (slider `horizontalSlider`, config 7), `randomly_delete_trailing` (`checkBox_3`, true: randomly drop stroke tails). Master switch `EnableCharacterDeformation` (false in config) lives elsewhere (writing function panel).
- Button "字符变形板块 - 使用默认推荐参数按钮点击": confirm dialog `操作确认` / `确认将字符变形参数设置为推荐参数吗？` @95044b1, then resets all the above to recommended values (the sample config shows what is probably the recommended or last-used set; true defaults not stated).
- Config is pushed into class attributes of `CharacterDeformationUtil` (`save_character_deform_config_to_gv`: "这里目前是存储在变形那边的类属性中"). Other strings (outside slice B): `微调请先关闭字符变形并重启`.
- Confidence: high for keys, medium for widgets.

### 奇偶页方向 (ParityPageConfig)
- `comboBox_8` -> `ParityPageDirectionMode` (config `0`; one choice name is `SameDirectionForParity`: same writing direction for odd and even pages).
- Confidence: medium.

---

## D. Notes for the Rust port
- The template manager's table templates are the biggest unknown format: persisted through `TemplateAccessUtil` (slice A/C), per-page background image as base64 PNG, tables as parent cells with `internal_rows`, inner margin, `accept_import` flag, `cell_class` ids, same-type records and data sources. Backend extraction of table frames from images (cloud call) has no local fallback in this slice; a Rust port needs either the endpoint (check `docs/server-api.md`) or a local detector.
- Plotting editor features are all local geometry. Items that matter for G-code export: Bezier arc-length sampling with `segment_length`, circle/ellipse sampling by chord height, line types as pen-up dashes, maze/seal as polylines, DXF (units, scale to paper, Y flip) and SVG (transform, viewBox).
- No laser power/speed/hatching/fill-pattern options were found in B; look in slices A/C for `DrawOrLaser`, firmware/GRBL params and device-side `S`/`M3`.
