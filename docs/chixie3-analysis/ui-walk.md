# ChiXie3 live UI notes (run un-elevated via __COMPAT_LAYER=RunAsInvoker, 2026-11-07)


## Main window (shot 01)
- Title bar QAT icons: app logo, open folder, save, export(?), settings gear, help ?
- Ribbon tabs: 主页 (filled blue = opens backstage) | 书写 | 绘图与激光
- Top-right: 炽写AI, 教程与售后, crown + username
- 书写 ribbon: [模板管理] [文件 ≡] | 作文格1 ▾ (page template picker) | 剪切 复制 格式刷 | 字体库 (A) | font combo (艾玛体) size combo (100) A+ A- | I U A(strike?) A(color ▾) X₂ X² H(?) eraser | list/indent/align buttons (2 rows of 4) | 插入 (+) | 查找 | 字符微调 | 合并内容 | 书写 (pen) ▶ ■ + job box (第 1 页 / 共 0 字 / 未书写) | 录制字体 | 百宝箱
- Status bar: 设备未连接 (button) | page "第 0 页，共 0 页" | 2 view-mode toggles | zoom 100% spinner
- Canvas: 作文格 grid paper (20 cols), Word-like page on grey desk

## 主页 = backstage (shots 02-09)
- Left rail: back ←, 字体, 模板, 教程 | bottom: 账户, 设置, 反馈
- 字体 > 在线字体: 筛选 checkboxes 中文字体/外文字体/手写字体/标准字体/公式符号/潦草字体; 排序 收藏最多/时间最新/字数最多; search box + 搜索; 3×3 cards (preview image of name, star = collect, 名称/类型(中文6900)/作者/描述 e.g. "来源于TTF转换，效果不佳"); pager 上一页 [1] /541 下一页
- 字体 > 我的收藏; 字体 > 我的录制 (user-recorded fonts). Header shows 主线字体 / 主英文字体 / 主符号字体 / 主数字字体 = 艾玛体, hint "(右键字体卡片可设置主字体)"
- 模板 = 模板库: 在线模板 / 我的模板 / 我的收藏; 分类: 绘图模板/激光模板/写字模板; 标签: 全部模板/人物角色/教育学习/艺术结构/建筑风景/动物植物/科技科学/交通玩具/其他模板; 排序 时间最新/收藏最多/点赞最多; search; 12 pages
- 账户 → dialog 用户信息: logo, 欢迎你 <name>, 用户类型 通用会员, 查看权益 (link), 手机号, 邮箱, 邀请码, 注册时间, 升级日期, 升级时长, 有效期至, [升级], [退出账号]
- 设置 → dialog 设置 (7 pages):
  - 常规设置: 默认功能 (写字功能), 参数同步 (开启), 语言 (简体中文), 设备管理 (默认收起)
  - 写字偏好: 字符间隔(毫米) 中文 0.6–1.1, 英文 0.5–1.0; 空格宽度(毫米) 中文 5.0–7.0, 英文 2.0–3.0; 首行缩进 宽度 15.5–17.0; 垂直偏移(毫米) 范围 0.0–0.3, 风格 ☐启用智能偏移(同向偏移); 路径规划 ○之字型(默认) ●弓字型; 首字 ☑首字不对齐(适用于笔记、表格) 不对齐幅度 2.0
  - 多色绘制: ☐多色绘制; 换笔模式 ○手动换笔(弹窗提示) ○自动换笔(需设备支持); 自动换笔配置 共1支笔 [设置]; preview box
  - 绘图与激光: CAD导入: 拆分导入(根据源文件属性) ▾
  - 字符变形: ☑基础变形: 字符随机胖瘦 96%–104%, 字符随机高矮 98%–102%, 字符随机旋转 -2.0°–2.0°, 笔画变形幅度 slider (1), ☑随机删除小尾巴; ☑字形变形: 字形变形幅度 slider (7); "(滑动条为0则该项无变形)"; [使用推荐参数]
  - 主题与样式: 界面风格 Office风格 ▾; 笔记线条 实线 ▾
  - 页面旋转: 页面旋转模式 不旋转 ▾; "(默认不旋转，默认起点为左上角) 页面预览不旋转，只旋转书写模式输出"
- 教程: (click did not change page in shot 07 — maybe opens web)

## 书写 ribbon menus (shots 22-25)
- 作文格1 ▾ = template picker: "我的模板" list (我的笔记1, 南部信纸22行no edge…, 作文格1)
- 文件 ≡ menu: 模板管理 | 新建 ▸ | 打开 | 保存 | 另存为 | 批量导入 | 打印 ▸ | 选项 | 退出
- 插入 menu: 表格 | 公式 ▸ | 图表 | 图片 | 文本框 | 形状 | 内容块 | 思维导图
- 百宝箱 dialog (10 tools, each "查看"):
  - 拓竹写字助手 "输出Gcode至拓竹设备": dialog 拓竹助手 — note "请先配置页面模板，页面尺寸不要超出设备行程", 详细使用教程; 设备型号 (拓竹A1), 打印机IP + 自动搜索, 访问码 + 连接设备, 页面选择 (第1页) + 上传; X轴偏移 0.0, Y轴偏移 0.0, 抬笔位置 8.0, 落笔位置 4.0, 原点模式 None, 提示音 启用; file list with 刷新 / 删除
  - Gcode预览执行器 "导入Gcode，预览/执行": web page (chixie.com.cn/client_api/gcode_page/gcode_tool_page) in a dialog "Gcode预览与执行工具": 炽写GCode工具, drop/click .gcode/.txt, X轴方向 右(X+), Y轴方向 上(Y+), 应用坐标系设置, ☐路径简化 ☑渐进式渲染, 细节级别 slider (10), ▶执行 + progress "就绪", 重置; preview canvas with stats overlay (已解析行, 路径数, 解析时间, 内存使用, FPS), legend 移动轨迹(抬笔) blue / 绘制轨迹(落笔) green / 当前位置 red, 适应视图, 性能面板, zoom +/−/reset
  - 插件管理 "目前仅用于开发者使用": dialog 开发者调试 — 语言 Python ▾, 代码 editor, 输出 pane, [保存] [启用插件]
  - 任务编程 "手动灵活编排任务": (no dialog appeared)
  - 自动换纸控制 "目前仅适配部分换纸机器": dialog — ▶/■, 当前页码, 书写状态 停止中, 任务总量, 进纸状态 未进纸, [进纸] [退纸]; table 序号/页码/字数/书写状态; [导入页面]; 任务列表循环 N 次
  - 固件烧录 "常用控制板烧录固件": (no dialog appeared un-elevated)
  - 一笔画【简易】 "输入坐标集，一笔绘制": dialog 一笔画V1 — "请逐行输入坐标: X,Y 【针对炽写设备，如需换行可输入如下指令】 pen_up, pen_down"; 循环次数 1; [保存] [执行]
  - 多设备群控 "设备群控与任务分配": (no dialog appeared)
  - 远程文件 "管理远程TF卡文件" (needs device)
  - 字体优化 "优化部分字体问题": dialog 字体数据优化 — 字体 (田英章 ▾), [统一字符尺寸] [重复轨迹优化] [异常宽度优化] [统一所有字体尺寸], ☐启动自动调用
- Web links: 教程 → chixie.com.cn/client_api/user_guide_service/user-guide-and-service-page ; 反馈 → docs.qq.com sheet (炽写3.0bug记录表); taobao items (shop)

## 绘图与激光 tab (shots 30, p*, r*, s*)
- Ribbon (groups expand in place): A4 ▾ | 模板库 | 导入 → CAD / SVG / 图片 | 图形 → gallery: point, line, polyline, S-curve, polygon(hexagon), rectangle, circle, 3-point arc, center arc, ⚙ (opens 图形属性 side panel: 类型, 横坐标, 纵坐标, 线型 实线, 颜色) | 文本 → 文本框 / 表格 ▾ / 填充文本域 | 酷玩 ▾ (印章生成器, 分形艺术生成器, 迷宫生成器, 立体字) | 拟人 | AI绘图 (no visible dialog un-elevated / account-gated?)
- A4 ▾ paper popup: ●标准纸张 纸型 A4(210×297) ▾, 方向 ○横向 ○纵向; ○自定义纸张 纸名 ▾, 宽度 180.0, 高度 150.0 (spin, mm), [确定]
- Canvas: mm rulers (0–290 for A4 landscape), red marks at paper bounds, grid; left rail: select arrow, hand pan, grid toggle, snap toggle, layers; status bar X:/Y: cursor coordinates
- CAD / SVG import open file dialogs (CAD文件 *.dxf, SVG文件 *.svg)

## Device manager (status bar 设备未连接 → docked left pane)
- Header 设备管理, [+] add device, 更多 ▾ menu: 切换炽写设备管理 | ✓命令窗口 | ✓坐标系设置 | 自定义命令 | 固件参数 | 参数导入/导出 ▸ | 更多设置 ▸ ; × close
- Tab 连接与控制: 端口号 COM4 (CH340) ▾ + refresh; 设备名 guangku ▾; ☐自动连接设备; [连接]; sub-tabs USB | 无线; jog pad (↑↓←→ + centre home), [抬笔] (toggle) with ▲▼, 单位 5mm ▾, [设置原点]
- Tab 参数设置: 设备名, 固件 ChiXie_ESP32 ▾, 设备类型 步进抬笔写字 ▾, 写字速度 19000 毫米/分, 落笔后暂停 0 秒, 落笔高度 2 毫米, ☐设为默认设备, [炽写ESP主板设置]; 坐标系 Y+ ▾ / X+ ▾ direction pickers around a pad; [delete] [保存]

## 模板管理 (contextual ribbon tab 页面模板 > 模板管理)
- Ribbon: 创建模板 ▾ (文档模板 / 表格模板 / 作文格模板) | 删除 | 导出 | 保存 | 返回
- Left: 模板列表 (icons: Word-like for notes, OneNote-like for 作文格, Excel-like for tables)
- 页面通用参数: 模板名称, 纸张宽度 190.0mm, 纸张高度 255.0mm, 快捷尺寸 [A4] [B5]
- 作文页面参数: 格子行数 20, 格子列数 20, 左侧边距 15.0mm, 上侧边距 20.0mm, 格子宽度 8.00mm, 格子高度 8.00mm, 行间距离 2.90mm, 列间距离 0.00mm
