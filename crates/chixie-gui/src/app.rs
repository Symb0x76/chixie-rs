//! Main window, laid out like a word processor: login start screen, then a ribbon
//! (文件 / 开始 / 页面布局 / 手写效果 / 设备 / 视图), the page editor, and a status bar.
//! Session worker events are pumped here and fanned out to views + toasts.

use std::path::PathBuf;
use std::time::Duration;

use chixie_core::config::import_chixie3;
use chixie_core::editor::EditDoc;
use chixie_core::device::{DeviceHandle, Event as DevEvent, Job};
use chixie_core::gcode;
use chixie_core::session::{Command, Event};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, Theme, ThemeMode, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use crate::device::{DevicePanel, Notice};
use crate::doc_view::{self, DocView};
use crate::file_ops;
use crate::fonts::FontsView;
use crate::login::LoginView;
use crate::state::{ActiveFont, AppState, User};

const A4: (f32, f32) = (210.0, 297.0);
const A5: (f32, f32) = (148.0, 210.0);
/// Word's margin presets (mm): 普通 / 窄 / 宽, as (left, top, right, bottom).
type Margins = (f32, f32, f32, f32);
const MARGINS: [(&str, Margins); 3] =
    [("普通", (21.0, 20.0, 21.0, 20.0)), ("窄", (12.7, 12.7, 12.7, 12.7)), ("宽", (31.8, 25.4, 31.8, 25.4))];
const TABS: [&str; 6] = ["文件", "开始", "页面布局", "手写效果", "设备", "视图"];
const DRAFT_EVERY: Duration = Duration::from_secs(2);
/// Plotter travel on the verified board ($130/$131 = 300 mm).
const TRAVEL_MM: f32 = 300.0;

pub struct ChiXieApp {
    pub tab: usize,
    pub login: Entity<LoginView>,
    pub fonts: Entity<FontsView>,
    pub doc: Entity<DocView>,
    pub device: Entity<DevicePanel>,
    /// Right-hand device task pane (opens on connect / job start).
    pub show_pane: bool,
    font_size: Entity<InputState>,
    line_height: Entity<InputState>,
    /// Where 保存 writes; `None` until 另存为 / 打开 of a `.chx`.
    file: Option<PathBuf>,
    _tasks: Vec<Task<()>>,
    _subs: Vec<Subscription>,
}

impl ChiXieApp {
    pub fn new(
        events: async_channel::Receiver<Event>,
        device: DeviceHandle,
        device_events: async_channel::Receiver<DevEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let login = cx.new(|cx| LoginView::new(window, cx));
        let fonts = cx.new(|cx| FontsView::new(window, cx));
        let draft = file_ops::draft_path(&cx.global::<AppState>().settings_path);
        let initial = file_ops::open(&draft).map(|l| l.doc).unwrap_or_else(|_| EditDoc::from_text("你好，赤写！\n春眠不觉晓，处处闻啼鸟。"));
        let doc = cx.new(|cx| DocView::new(initial, window, cx));
        let page = cx.global::<AppState>().settings.page.clone();
        let font_size = cx.new(|cx| InputState::new(window, cx).default_value(fmt_num(page.font_size)));
        let line_height = cx.new(|cx| InputState::new(window, cx).default_value(fmt_num(page.line_height)));

        let pump = cx.spawn_in(window, async move |this, cx| {
            while let Ok(ev) = events.recv().await {
                if this.update_in(cx, |app, window, cx| app.on_session_event(ev, window, cx)).is_err() {
                    break;
                }
            }
        });
        // Draft autosave: cheap poll of the editor's dirty flag.
        let autosave = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(DRAFT_EVERY).await;
                let Ok(()) = this.update(cx, |app, cx| app.save_draft(cx)) else { break };
            }
        });
        let subs = vec![
            cx.observe_global::<AppState>(|_, cx| cx.notify()),
            cx.observe(&doc, |_, _, cx| cx.notify()),
            cx.subscribe_in(&font_size, window, |this, s, ev: &InputEvent, _w, cx| {
                if let (InputEvent::Change, Some(v)) = (ev, parse_in(s, cx, 5.0, 1000.0)) {
                    this.edit_settings(cx, |st| st.settings.page.font_size = v);
                }
            }),
            cx.subscribe_in(&line_height, window, |this, s, ev: &InputEvent, _w, cx| {
                if let (InputEvent::Change, Some(v)) = (ev, parse_in(s, cx, 1.0, 200.0)) {
                    this.edit_settings(cx, |st| st.settings.page.line_height = v);
                }
            }),
        ];
        let device = cx.new(|cx| DevicePanel::new(device, window, cx));
        let dev_pump = cx.spawn_in(window, async move |this, cx| {
            while let Ok(ev) = device_events.recv().await {
                if this.update_in(cx, |app, window, cx| app.on_device_event(ev, window, cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            tab: 1,
            login,
            fonts,
            doc,
            device,
            show_pane: false,
            font_size,
            line_height,
            file: None,
            _tasks: vec![pump, dev_pump, autosave],
            _subs: subs,
        }
    }

    fn on_session_event(&mut self, ev: Event, window: &mut Window, cx: &mut Context<Self>) {
        match ev {
            Event::LoggedIn { username, role } => {
                self.login.update(cx, |l, cx| l.on_logged_in(cx));
                let st = cx.global_mut::<AppState>();
                st.user = Some(User { name: username.clone(), role });
                // Restore the last-used font (decrypted again; fonts are never cached on disk).
                if let Some(id) = st.settings.page.font_id {
                    st.session.send(Command::LoadFont { id });
                }
                st.session.send(Command::CollectedFonts);
                window.push_notification(Notification::success(format!("欢迎，{username}")), cx);
                self.focus_doc(window, cx);
            }
            Event::LoginFailed(msg) => self.login.update(cx, |l, cx| l.on_failed(msg, cx)),
            Event::LoggedOut => {
                let st = cx.global_mut::<AppState>();
                st.user = None;
                st.active_font = None;
            }
            Event::SessionLost(msg) => {
                let st = cx.global_mut::<AppState>();
                st.user = None;
                st.active_font = None;
                window.push_notification(Notification::warning(format!("登录已失效，请重新登录（{msg}）")), cx);
            }
            Event::Fonts { kind, page, fonts } => self.fonts.update(cx, |f, cx| f.set_list(kind, page, fonts, cx)),
            Event::FontLoaded { id, font } => {
                let name = self.fonts.read(cx).name_of(id).unwrap_or_else(|| format!("字体 #{id}"));
                self.fonts.update(cx, |f, cx| f.font_loaded(cx));
                let glyphs = font.glyphs.len();
                let st = cx.global_mut::<AppState>();
                st.active_font = Some(ActiveFont { id, name: name.clone(), font });
                st.settings.page.font_id = Some(id);
                st.save_settings();
                if window.has_active_dialog(cx) {
                    window.close_dialog(cx);
                }
                window.push_notification(Notification::success(format!("已加载 {name}（{glyphs} 字）")), cx);
                self.focus_doc(window, cx);
            }
            Event::Error(msg) => {
                self.fonts.update(cx, |f, cx| f.font_loaded(cx));
                window.push_notification(Notification::error(msg), cx);
            }
        }
        cx.notify();
    }

    fn focus_doc(&self, window: &mut Window, cx: &mut App) {
        let focus = self.doc.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    }

    /// Apply a settings change and persist it; the editor re-lays out via its global observer.
    fn edit_settings(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut AppState)) {
        let st = cx.global_mut::<AppState>();
        f(st);
        st.save_settings();
    }

    fn save_draft(&mut self, cx: &mut Context<Self>) {
        if !self.doc.update(cx, |d, _| d.take_dirty()) {
            return;
        }
        let st = cx.global::<AppState>();
        let path = file_ops::draft_path(&st.settings_path);
        if let Err(e) = file_ops::save_chx(&path, self.doc.read(cx).doc(), st.settings.style.handwriting.seed) {
            log::warn!("draft {}: {e}", path.display());
        }
    }

    // ---- 文件 ----

    fn new_doc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.file = None;
        self.doc.update(cx, |d, cx| d.replace_doc(EditDoc::from_text(""), cx));
        self.focus_doc(window, cx);
    }

    fn open_doc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("打开".into()) });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let _ = this.update_in(cx, |app, window, cx| match file_ops::open(&path) {
                Ok(l) => {
                    if let Some(seed) = l.seed {
                        app.edit_settings(cx, |st| st.settings.style.handwriting.seed = seed);
                    }
                    let is_chx = l.seed.is_some();
                    app.file = is_chx.then(|| path.clone());
                    app.doc.update(cx, |d, cx| d.replace_doc(l.doc, cx));
                    app.focus_doc(window, cx);
                }
                Err(e) => window.push_notification(Notification::error(format!("打开失败：{e}")), cx),
            });
        })
        .detach();
    }

    fn save_doc(&mut self, save_as: bool, window: &mut Window, cx: &mut Context<Self>) {
        let doc = self.doc.read(cx).doc().clone();
        let seed = cx.global::<AppState>().settings.style.handwriting.seed;
        if let (false, Some(path)) = (save_as, self.file.clone()) {
            let msg = match file_ops::save_chx(&path, &doc, seed) {
                Ok(()) => Notification::success(format!("已保存 {}", path.display())),
                Err(e) => Notification::error(format!("保存失败：{e}")),
            };
            window.push_notification(msg, cx);
            return;
        }
        let rx = cx.prompt_for_new_path(&file_ops::documents_dir(), Some("文档.chx"));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(mut path))) = rx.await else { return };
            if path.extension().is_none() {
                path.set_extension("chx");
            }
            let res = file_ops::save_chx(&path, &doc, seed);
            let _ = this.update_in(cx, |app, window, cx| {
                let msg = match res {
                    Ok(()) => {
                        app.file = Some(path.clone());
                        Notification::success(format!("已保存 {}", path.display()))
                    }
                    Err(e) => Notification::error(format!("保存失败：{e}")),
                };
                window.push_notification(msg, cx);
            });
        })
        .detach();
    }

    fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let doc = self.doc.read(cx).document();
        if doc.stroke_count() == 0 {
            window.push_notification(Notification::warning("没有可导出的笔画（请先选择字体并输入文字）"), cx);
            return;
        }
        let profile = cx.global::<AppState>().settings.device.profile.clone();
        let rx = cx.prompt_for_new_path(&file_ops::documents_dir(), Some("chixie.gcode"));
        cx.spawn_in(window, async move |_this, cx| {
            let Ok(Ok(Some(path))) = rx.await else { return };
            let msg = match file_ops::export_files(&doc, &profile, &path) {
                Ok(files) => Notification::success(format!("已导出 {} 个文件：{}", files.len(), files[0].display())),
                Err(e) => Notification::error(format!("导出失败：{e}")),
            };
            let _ = cx.update(|window, cx| window.push_notification(msg, cx));
        })
        .detach();
    }

    fn import_style(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择炽写 config.ini".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let res = std::fs::read(&path).map(|b| import_chixie3(&String::from_utf8_lossy(&b)));
            let _ = this.update_in(cx, |app, window, cx| match res {
                Ok(style) => {
                    app.edit_settings(cx, |st| st.settings.style = style);
                    window.push_notification(Notification::success("已导入炽写的手写效果设置"), cx);
                }
                Err(e) => window.push_notification(Notification::error(format!("导入失败：{e}")), cx),
            });
        })
        .detach();
    }

    fn open_fonts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fonts = self.fonts.clone();
        fonts.update(cx, |f, cx| f.refresh_collected(cx));
        window.open_dialog(cx, move |dialog, _w, _cx| {
            dialog.title("字体库").w(px(820.)).child(div().h(px(520.)).child(fonts.clone()))
        });
    }

    fn reshuffle(&mut self, cx: &mut Context<Self>) {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        self.edit_settings(cx, |st| st.settings.style.handwriting.seed = seed);
    }

    // ---- ribbon ----
    // One method per tab: gpui element builders are large values, and a single function holding
    // every tab's temporaries overflowed the 1 MB main-thread stack in debug builds.

    fn ribbon_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let g = Group { muted: theme.muted_foreground, border: theme.border };
        let body = match self.tab {
            0 => self.tab_file(&g, cx),
            1 => self.tab_home(&g, cx),
            2 => self.tab_layout(&g, cx),
            3 => self.tab_style(&g, cx),
            4 => self.tab_device(&g, cx),
            _ => self.tab_view(&g, cx),
        };
        div().h(px(84.)).py_1().child(body).into_any_element()
    }

    #[inline(never)]
    fn tab_file(&self, g: &Group, cx: &mut Context<Self>) -> AnyElement {
        let user = cx.global::<AppState>().user.as_ref().map(|u| format!("{}（{}）", u.name, u.role)).unwrap_or_default();
        h_flex()
            .h_full()
            .child(g.group(
                "文档",
                h_flex()
                    .child(big("new", "新建", IconName::Plus).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.new_doc(w, cx))))
                    .child(big("open", "打开…", IconName::FolderOpen).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.open_doc(w, cx))))
                    .child(big("save", "保存", IconName::Inbox).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.save_doc(false, w, cx))))
                    .child(big("saveas", "另存为…", IconName::Copy).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.save_doc(true, w, cx)))),
            ))
            .child(g.group(
                "输出",
                h_flex().child(big("export", "导出 G-code…", IconName::ArrowDown).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.export(w, cx)))),
            ))
            .child(g.group(
                "设置",
                h_flex()
                    .child(big("import", "从炽写导入样式…", IconName::Settings).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.import_style(w, cx))))
                    .child(big("fontlib", "字体库…", IconName::Palette).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.open_fonts(w, cx)))),
            ))
            .child(g.group(
                "账户",
                h_flex().child(div().text_sm().child(user)).child(Button::new("logout").small().danger().label("退出登录").on_click(
                    |_: &ClickEvent, _w, cx| {
                        cx.global::<AppState>().session.send(Command::Logout);
                    },
                )),
            ))
            .into_any_element()
    }

    #[inline(never)]
    fn tab_home(&self, g: &Group, cx: &mut Context<Self>) -> AnyElement {
        let font_name = cx.global::<AppState>().active_font.as_ref().map(|f| f.name.clone()).unwrap_or_else(|| "选择字体…".into());
        let act = |a: Box<dyn Action>| editor_action(self.doc.clone(), a);
        h_flex()
            .h_full()
            .child(g.group(
                "剪贴板",
                h_flex()
                    .child(big("paste", "粘贴", IconName::Copy).on_click(act(Box::new(doc_view::Paste))))
                    .child(Button::new("cut").ghost().small().label("剪切").on_click(act(Box::new(doc_view::Cut))))
                    .child(Button::new("copy").ghost().small().label("复制").on_click(act(Box::new(doc_view::Copy)))),
            ))
            .child(g.group(
                "编辑",
                h_flex()
                    .child(Button::new("undo").ghost().small().icon(IconName::Undo2).tooltip("撤销 Ctrl+Z").on_click(act(Box::new(doc_view::Undo))))
                    .child(Button::new("redo").ghost().small().icon(IconName::Redo2).tooltip("重做 Ctrl+Y").on_click(act(Box::new(doc_view::Redo))))
                    .child(Button::new("selall").ghost().small().label("全选").on_click(act(Box::new(doc_view::SelectAll)))),
            ))
            .child(g.group(
                "字体",
                h_flex()
                    .child(
                        Button::new("font")
                            .outline()
                            .w(px(180.))
                            .label(font_name)
                            .dropdown_caret(true)
                            .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.open_fonts(w, cx))),
                    )
                    .child(div().text_sm().text_color(g.muted).child("字号"))
                    .child(div().w(px(64.)).child(Input::new(&self.font_size).small())),
            ))
            .child(g.group(
                "段落",
                h_flex().child(div().text_sm().text_color(g.muted).child("行距 mm")).child(div().w(px(64.)).child(Input::new(&self.line_height).small())),
            ))
            .into_any_element()
    }

    #[inline(never)]
    fn tab_layout(&self, g: &Group, cx: &mut Context<Self>) -> AnyElement {
        let page = cx.global::<AppState>().settings.page.clone();
        let portrait = page.paper.0 <= page.paper.1;
        let is_a5 = (page.paper.0.min(page.paper.1) - A5.0).abs() < 0.5;
        let margin_ix = MARGINS.iter().position(|(_, m)| *m == page.margins);
        h_flex()
            .h_full()
            .child(g.group(
                "纸张",
                h_flex().child(
                    TabBar::new("paper")
                        .small()
                        .segmented()
                        .selected_index(if is_a5 { 1 } else { 0 })
                        .children([Tab::new().label("A4"), Tab::new().label("A5")])
                        .on_click(cx.listener(move |t, ix: &usize, _w, cx| {
                            let base = if *ix == 1 { A5 } else { A4 };
                            let paper = if portrait { base } else { (base.1, base.0) };
                            t.edit_settings(cx, |st| st.settings.page.paper = paper);
                        })),
                ),
            ))
            .child(g.group(
                "方向",
                h_flex().child(
                    TabBar::new("orient")
                        .small()
                        .segmented()
                        .selected_index(if portrait { 0 } else { 1 })
                        .children([Tab::new().label("纵向"), Tab::new().label("横向")])
                        .on_click(cx.listener(move |t, ix: &usize, _w, cx| {
                            if (*ix == 0) != portrait {
                                t.edit_settings(cx, |st| {
                                    let (w, h) = st.settings.page.paper;
                                    st.settings.page.paper = (h, w);
                                });
                            }
                        })),
                ),
            ))
            .child(g.group(
                "页边距",
                h_flex().child(
                    TabBar::new("margins")
                        .small()
                        .segmented()
                        .selected_index(margin_ix.unwrap_or(usize::MAX))
                        .children(MARGINS.iter().map(|(n, _)| Tab::new().label(*n)))
                        .on_click(cx.listener(|t, ix: &usize, _w, cx| {
                            let m = MARGINS[*ix].1;
                            t.edit_settings(cx, |st| st.settings.page.margins = m);
                        })),
                ),
            ))
            .child(g.group(
                "尺寸",
                h_flex().child(div().text_sm().text_color(g.muted).child(format!(
                    "{:.0} × {:.0} mm，边距 {:.1}/{:.1}/{:.1}/{:.1}",
                    page.paper.0, page.paper.1, page.margins.0, page.margins.1, page.margins.2, page.margins.3
                ))),
            ))
            .into_any_element()
    }

    #[inline(never)]
    fn tab_style(&self, g: &Group, cx: &mut Context<Self>) -> AnyElement {
        let st = &cx.global::<AppState>().settings.style;
        let (hw, df) = (st.handwriting.clone(), st.deformation.clone());
        h_flex()
            .h_full()
            .child(g.group(
                "随机排布",
                h_flex().child(Checkbox::new("jitter").label("字距 / 行首 / 上下浮动").checked(hw.enabled).on_click(cx.listener(
                    |t, v: &bool, _w, cx| {
                        let v = *v;
                        t.edit_settings(cx, |st| st.settings.style.handwriting.enabled = v);
                    },
                ))),
            ))
            .child(g.group(
                "字符变形",
                h_flex()
                    .gap_3()
                    .child(Checkbox::new("base").label("基础变形").checked(df.enabled && df.base).on_click(cx.listener(|t, v: &bool, _w, cx| {
                        let v = *v;
                        t.edit_settings(cx, |st| {
                            let d = &mut st.settings.style.deformation;
                            d.base = v;
                            d.enabled = d.base || d.shape;
                        });
                    })))
                    .child(Checkbox::new("shape").label("字形变形").checked(df.enabled && df.shape).on_click(cx.listener(|t, v: &bool, _w, cx| {
                        let v = *v;
                        t.edit_settings(cx, |st| {
                            let d = &mut st.settings.style.deformation;
                            d.shape = v;
                            d.enabled = d.base || d.shape;
                        });
                    }))),
            ))
            .child(g.group(
                "笔迹",
                h_flex()
                    .child(big("reshuffle", "换一组", IconName::Redo).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.reshuffle(cx))))
                    .child(div().text_xs().text_color(g.muted).child(format!("种子 {}", hw.seed % 100_000))),
            ))
            .into_any_element()
    }

    #[inline(never)]
    fn tab_device(&self, g: &Group, cx: &mut Context<Self>) -> AnyElement {
        let dev = self.device.read(cx);
        let (connected, connecting_label) = (dev.is_connected(), dev.summary());
        let job = dev.job;
        let running = job.is_some();
        let paused = job.is_some_and(|j| j.paused);
        let awaiting = job.and_then(|j| j.awaiting);
        h_flex()
            .h_full()
            .child(g.group(
                "连接",
                h_flex()
                    .child(if connected {
                        big("disconnect", "断开", IconName::CircleX)
                            .disabled(running)
                            .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.device.update(cx, |d, cx| d.disconnect(cx))))
                    } else {
                        big("connect", "连接设备", IconName::Globe).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                            t.show_pane = true;
                            t.device.update(cx, |d, cx| d.connect(cx));
                        }))
                    })
                    .child(div().text_xs().text_color(g.muted).child(connecting_label)),
            ))
            .child(g.group(
                "书写",
                h_flex()
                    .child(
                        big("start", "开始书写", IconName::ArrowRight)
                            .disabled(!connected || running)
                            .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.start_job(w, cx))),
                    )
                    .when(awaiting.is_some(), |el| {
                        el.child(Button::new("next").small().primary().label("已换纸，继续").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                            t.device.update(cx, |d, cx| d.next_sheet(cx))
                        })))
                    })
                    .child(
                        Button::new("pause")
                            .small()
                            .ghost()
                            .label(if paused { "继续" } else { "暂停" })
                            .disabled(!running || awaiting.is_some())
                            .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.device.update(cx, |d, cx| d.toggle_pause(cx)))),
                    )
                    .child(
                        Button::new("stop")
                            .small()
                            .danger()
                            .label("停止")
                            .disabled(!running)
                            .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.device.update(cx, |d, cx| d.stop(cx)))),
                    ),
            ))
            .child(g.group(
                "窗格",
                h_flex().child(
                    Button::new("pane")
                        .small()
                        .ghost()
                        .label(if self.show_pane { "隐藏设备面板" } else { "显示设备面板" })
                        .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                            t.show_pane = !t.show_pane;
                            cx.notify();
                        })),
                ),
            ))
            .into_any_element()
    }

    /// Confirm extents, then stream every page (sheet change prompts in between).
    fn start_job(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let doc = self.doc.read(cx).document();
        if doc.stroke_count() == 0 {
            window.push_notification(Notification::warning("没有可书写的笔画（请先选择字体并输入文字）"), cx);
            return;
        }
        let profile = cx.global::<AppState>().settings.device.profile.clone();
        let pages: Vec<String> = doc.pages.iter().map(|p| gcode::emit(p, &profile)).collect();
        let (w, h) = doc.bounds().map(|(x0, y0, x1, y1)| (x1 - x0, y1 - y0)).unwrap_or_default();
        let over = w > TRAVEL_MM || h > TRAVEL_MM;
        let desc = format!(
            "共 {} 页（{} 笔），书写范围 {w:.1} × {h:.1} mm。{}请确认纸张已放好、笔已装好，起点为当前原点。",
            pages.len(),
            doc.stroke_count(),
            if over { "⚠ 超出 300 mm 行程！" } else { "" }
        );
        self.show_pane = true;
        let device = self.device.clone();
        window.open_alert_dialog(cx, move |a, _w, _cx| {
            let device = device.clone();
            let pages = pages.clone();
            a.title("开始书写？").description(desc.clone()).confirm().ok_text("开始").on_ok(move |_, _w, cx| {
                device.update(cx, |d, cx| d.run(Job { pages: pages.clone() }, cx));
                true
            })
        });
    }

    fn on_device_event(&mut self, ev: DevEvent, window: &mut Window, cx: &mut Context<Self>) {
        let notice = self.device.update(cx, |d, cx| d.on_event(ev, cx));
        match notice {
            Notice::None => {}
            Notice::Info(m) => window.push_notification(Notification::info(m), cx),
            Notice::Warn(m) => window.push_notification(Notification::warning(m), cx),
            Notice::Error(m) => window.push_notification(Notification::error(m), cx),
            Notice::AwaitingSheet(next) => {
                let device = self.device.clone();
                window.open_alert_dialog(cx, move |a, _w, _cx| {
                    let (ok, cancel) = (device.clone(), device.clone());
                    a.title("请换纸")
                        .description(format!("第 {} 页已就绪。放好新纸、确认原点后点「继续」。", next + 1))
                        .confirm()
                        .ok_text("继续")
                        .cancel_text("停止任务")
                        .on_ok(move |_, _w, cx| {
                            ok.update(cx, |d, cx| d.next_sheet(cx));
                            true
                        })
                        .on_cancel(move |_, _w, cx| {
                            cancel.update(cx, |d, cx| d.stop(cx));
                            true
                        })
                });
            }
        }
        cx.notify();
    }

    #[inline(never)]
    fn tab_view(&self, g: &Group, cx: &mut Context<Self>) -> AnyElement {
        let dark = cx.theme().mode == ThemeMode::Dark;
        h_flex()
            .h_full()
            .child(g.group(
                "缩放",
                h_flex()
                    .child(Button::new("zout").ghost().small().icon(IconName::Minus).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.zoom_by(1.0 / 1.1, cx))))
                    .child(Button::new("zin").ghost().small().icon(IconName::Plus).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.zoom_by(1.1, cx))))
                    .child(Button::new("z100").ghost().small().label("100%").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                        t.doc.update(cx, |d, cx| d.set_zoom(doc_view::ZOOM_100, cx))
                    })))
                    .child(Button::new("zfit").ghost().small().label("页宽").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                        t.doc.update(cx, |d, cx| d.fit_width(cx))
                    }))),
            ))
            .child(g.group(
                "外观",
                h_flex().child(Button::new("theme").ghost().small().label(if dark { "浅色模式" } else { "深色模式" }).on_click(
                    move |_: &ClickEvent, window, cx| {
                        Theme::change(if dark { ThemeMode::Light } else { ThemeMode::Dark }, Some(window), cx);
                        let st = cx.global_mut::<AppState>();
                        st.settings.ui.dark = !dark;
                        st.save_settings();
                    },
                )),
            ))
            .into_any_element()
    }

    fn zoom_by(&mut self, f: f32, cx: &mut Context<Self>) {
        self.doc.update(cx, |d, cx| {
            let z = d.zoom() * f;
            d.set_zoom(z, cx)
        });
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let st = cx.global::<AppState>();
        let font = st.active_font.as_ref().map(|f| f.name.clone()).unwrap_or_else(|| "未选择字体".into());
        let d = self.doc.read(cx);
        let layout = d.document();
        let pages = layout.pages.len().max(1);
        let words = d.doc().chars().iter().filter(|c| !c.is_whitespace()).count();
        let missing = layout.missing.len();
        let zoom_pct = (d.zoom() / doc_view::ZOOM_100 * 100.0).round();
        let dev = self.device.read(cx);
        let (dev_summary, job) = (dev.summary(), dev.job);
        let theme = cx.theme();
        h_flex()
            .w_full()
            .h(px(26.))
            .px_3()
            .gap_4()
            .bg(theme.muted)
            .border_t_1()
            .border_color(theme.border)
            .text_color(theme.muted_foreground)
            .text_xs()
            .child(format!("第 {} 页，共 {pages} 页", d.caret_page() + 1))
            .child(format!("{words} 个字"))
            .when(missing > 0, |el| {
                let chars: String = layout.missing.iter().take(12).collect();
                el.child(format!("缺字 {missing}：{chars}"))
            })
            .child(format!("字体：{font}"))
            .child(dev_summary)
            .when_some(job, |el, j| {
                let s = match j.awaiting {
                    Some(n) => format!("等待换纸（第 {} 页）", n + 1),
                    None => format!("书写 第 {}/{} 页 {:.0}%{}", j.page + 1, j.pages, j.percent(), if j.paused { "（暂停）" } else { "" }),
                };
                el.child(s)
            })
            .child(div().flex_1())
            .child(Button::new("sb-zout").xsmall().ghost().icon(IconName::Minus).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.zoom_by(1.0 / 1.1, cx))))
            .child(format!("{zoom_pct}%"))
            .child(Button::new("sb-zin").xsmall().ghost().icon(IconName::Plus).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.zoom_by(1.1, cx))))
    }
}


use gpui_kit::prelude::FluentBuilder as _;

impl Render for ChiXieApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (bg, fg, border) = (theme.background, theme.foreground, theme.border);
        if cx.global::<AppState>().user.is_none() {
            return div().size_full().bg(bg).text_color(fg).child(self.login.clone()).into_any_element();
        }
        let tabs = TabBar::new("ribbon-tabs")
            .underline()
            .selected_index(self.tab)
            .children(TABS.iter().map(|t| Tab::new().label(*t)))
            .on_click(cx.listener(|this, ix: &usize, _w, cx| {
                this.tab = *ix;
                cx.notify();
            }));
        v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .child(div().px_2().child(tabs))
            .child(div().w_full().border_b_1().border_color(border).child(self.ribbon_body(cx)))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(div().flex_1().h_full().min_w_0().child(self.doc.clone()))
                    .when(self.show_pane, |el| el.child(self.device.clone())),
            )
            .child(self.status_bar(cx))
            .into_any_element()
    }
}

/// A labelled ribbon group (controls on top, caption below, divider on the right).
struct Group {
    muted: Hsla,
    border: Hsla,
}

impl Group {
    fn group(&self, label: &str, body: Div) -> Div {
        v_flex()
            .h_full()
            .px_3()
            .gap_1()
            .border_r_1()
            .border_color(self.border)
            .child(body.flex_1().items_center().gap_1())
            .child(div().w_full().text_center().text_xs().text_color(self.muted).child(label.to_string()))
    }
}

fn big(id: &'static str, label: &'static str, icon: IconName) -> Button {
    Button::new(id).ghost().icon(icon).label(label)
}

/// Click handler that runs an editor action with focus handed back to the page (so ribbon
/// buttons behave exactly like their shortcuts).
fn editor_action(doc: Entity<DocView>, a: Box<dyn Action>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let focus = doc.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.dispatch_action(a.boxed_clone(), cx);
    }
}

fn fmt_num(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn parse_in(s: &Entity<InputState>, cx: &App, min: f32, max: f32) -> Option<f32> {
    s.read(cx).value().trim().parse::<f32>().ok().filter(|v| (min..=max).contains(v))
}
