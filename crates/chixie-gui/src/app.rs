//! Main window, laid out like Microsoft Word: login start screen, then a tab strip with a quick
//! access toolbar and a filled 文件 tab (opens the Backstage), the ribbon (开始 / 布局 / 手写 /
//! 设备 / 视图), the page editor with the device task pane, and a status bar.
//! Session and device events are pumped here and fanned out to views + toasts.

mod backstage;
mod ribbon_tabs;

use std::path::PathBuf;
use std::time::Duration;

use chixie_core::config::import_chixie3;
use chixie_core::device::{DeviceHandle, Event as DevEvent, Job};
use chixie_core::editor::EditDoc;
use chixie_core::gcode;
use chixie_core::session::{Command, Event, FontListKind};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme as _, IndexPath, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub use backstage::Stage;

use crate::device::{DevicePanel, Notice};
use crate::doc_view::{self, DocView};
use crate::file_ops;
use crate::fonts::FontsView;
use crate::login::LoginView;
use crate::ribbon::{self, RIBBON_H};
use crate::state::{ActiveFont, AppState, User};
use crate::style_dialog::StyleDialog;
use crate::templates_view::{TemplateEvent, TemplatesView};

pub const TABS: [&str; 5] = ["开始", "布局", "手写", "设备", "视图"];
pub const TAB_DEVICE: usize = 3;
const DRAFT_EVERY: Duration = Duration::from_secs(2);
/// Plotter travel on the verified board ($130/$131 = 300 mm).
const TRAVEL_MM: f32 = 300.0;

pub struct ChiXieApp {
    pub tab: usize,
    pub login: Entity<LoginView>,
    pub fonts: Entity<FontsView>,
    pub doc: Entity<DocView>,
    pub device: Entity<DevicePanel>,
    pub templates: Entity<TemplatesView>,
    /// Right-hand device task pane (opens on connect / job start).
    pub show_pane: bool,
    /// Full-window 文件 view; `None` shows the document.
    pub backstage: Option<Stage>,
    ribbon_collapsed: bool,
    font_size: Entity<InputState>,
    line_height: Entity<InputState>,
    font_select: Entity<SelectState<Vec<String>>>,
    /// Collected font ids, parallel to the font select's items.
    font_ids: Vec<i64>,
    style_dialog: Entity<StyleDialog>,
    backstage_focus: FocusHandle,
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
        let font_size = cx.new(|cx| InputState::new(window, cx).step(5.0).min(5.0).max(1000.0).default_value(fmt_num(page.font_size)));
        let line_height = cx.new(|cx| InputState::new(window, cx).step(0.25).min(1.0).max(200.0).default_value(fmt_num(page.line_height)));
        let font_select = cx.new(|cx| SelectState::new(Vec::<String>::new(), None, window, cx));
        let templates = cx.new(|cx| TemplatesView::new(window, cx));
        let style_dialog = cx.new(|cx| StyleDialog::new(window, cx));
        let device = cx.new(|cx| DevicePanel::new(device, window, cx));

        let pump = cx.spawn_in(window, async move |this, cx| {
            while let Ok(ev) = events.recv().await {
                if this.update_in(cx, |app, window, cx| app.on_session_event(ev, window, cx)).is_err() {
                    break;
                }
            }
        });
        let dev_pump = cx.spawn_in(window, async move |this, cx| {
            while let Ok(ev) = device_events.recv().await {
                if this.update_in(cx, |app, window, cx| app.on_device_event(ev, window, cx)).is_err() {
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
            cx.observe(&device, |_, _, cx| cx.notify()),
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
            cx.subscribe_in(&font_select, window, |this, _s, ev: &SelectEvent<Vec<String>>, _w, cx| {
                if let SelectEvent::Confirm(Some(name)) = ev {
                    this.choose_font(name, cx);
                }
            }),
            cx.subscribe_in(&templates, window, |this, _v, ev: &TemplateEvent, window, cx| match ev {
                TemplateEvent::Apply(name) => {
                    this.apply_template(Some(name.clone()), cx);
                    this.close_backstage(window, cx);
                }
            }),
        ];
        Self {
            tab: 0,
            login,
            fonts,
            doc,
            device,
            templates,
            show_pane: false,
            backstage: None,
            ribbon_collapsed: false,
            font_size,
            line_height,
            font_select,
            font_ids: Vec::new(),
            style_dialog,
            backstage_focus: cx.focus_handle(),
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
                self.backstage = None;
            }
            Event::SessionLost(msg) => {
                let st = cx.global_mut::<AppState>();
                st.user = None;
                st.active_font = None;
                window.push_notification(Notification::warning(format!("登录已失效，请重新登录（{msg}）")), cx);
            }
            Event::Fonts { kind, page, fonts } => {
                let collected = kind == FontListKind::Collected;
                self.fonts.update(cx, |f, cx| f.set_list(kind, page, fonts, cx));
                if collected {
                    self.sync_font_select(window, cx);
                }
            }
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
                self.sync_font_select(window, cx);
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

    /// Font picker items = collected fonts; selection = the active font.
    fn sync_font_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let list: Vec<(i64, String)> = self.fonts.read(cx).collected().iter().map(|f| (f.id, f.font_name.clone())).collect();
        let active = cx.global::<AppState>().active_font.as_ref().map(|f| f.id);
        self.font_ids = list.iter().map(|(id, _)| *id).collect();
        let names: Vec<String> = list.into_iter().map(|(_, n)| n).collect();
        let selected = active.and_then(|id| self.font_ids.iter().position(|&x| x == id)).map(IndexPath::new);
        self.font_select.update(cx, |s, cx| {
            s.set_items(names, window, cx);
            s.set_selected_index(selected, window, cx);
        });
    }

    fn choose_font(&mut self, name: &str, cx: &mut Context<Self>) {
        let ids = self.font_ids.clone();
        let names: Vec<String> = self.fonts.read(cx).collected().iter().map(|f| f.font_name.clone()).collect();
        if let Some(id) = names.iter().position(|n| n == name).and_then(|i| ids.get(i).copied())
            && cx.global::<AppState>().active_font.as_ref().map(|f| f.id) != Some(id)
        {
            self.fonts.update(cx, |f, cx| f.use_font(id, cx));
        }
    }

    pub fn focus_doc(&self, window: &mut Window, cx: &mut App) {
        let focus = self.doc.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    }

    /// Apply a settings change and persist it; the editor re-lays out via its global observer.
    fn edit_settings(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut AppState)) {
        let st = cx.global_mut::<AppState>();
        f(st);
        st.save_settings();
    }

    /// Use template `name` for the page (`None`: blank page, current paper kept).
    pub fn apply_template(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        self.edit_settings(cx, |st| match name.as_deref().and_then(|n| st.templates.get(n)).cloned() {
            Some(t) => st.settings.page.apply_template(&t),
            None => st.settings.page.template = None,
        });
    }

    /// Paper size / orientation / margin changes make the page a blank (template-less) page.
    fn edit_page(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut chixie_core::config::PageSettings)) {
        self.edit_settings(cx, |st| {
            st.settings.page.template = None;
            f(&mut st.settings.page);
        });
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

    pub fn new_doc(&mut self, template: Option<Option<String>>, window: &mut Window, cx: &mut Context<Self>) {
        self.file = None;
        if let Some(t) = template {
            self.apply_template(t, cx);
        }
        self.doc.update(cx, |d, cx| d.replace_doc(EditDoc::from_text(""), cx));
        self.close_backstage(window, cx);
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
                    app.close_backstage(window, cx);
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
            self.close_backstage(window, cx);
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
                app.close_backstage(window, cx);
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

    fn open_style_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.style_dialog.clone();
        window.open_dialog(cx, move |dialog, _w, _cx| dialog.title("手写效果范围").w(px(460.)).child(content.clone()));
    }

    fn reshuffle(&mut self, cx: &mut Context<Self>) {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        self.edit_settings(cx, |st| st.settings.style.handwriting.seed = seed);
    }

    pub fn open_backstage(&mut self, stage: Stage, window: &mut Window, cx: &mut Context<Self>) {
        if stage == Stage::Templates && self.backstage != Some(Stage::Templates) {
            let st = cx.global::<AppState>();
            let active = st.settings.page.template.clone();
            let ix = active.and_then(|n| st.templates.templates.iter().position(|t| t.name == n)).unwrap_or(0);
            self.templates.update(cx, |v, cx| v.select(ix, window, cx));
        }
        self.backstage = Some(stage);
        window.focus(&self.backstage_focus, cx);
        cx.notify();
    }

    pub fn close_backstage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.backstage = None;
        self.focus_doc(window, cx);
        cx.notify();
    }

    fn zoom_by(&mut self, f: f32, cx: &mut Context<Self>) {
        self.doc.update(cx, |d, cx| {
            let z = d.zoom() * f;
            d.set_zoom(z, cx)
        });
    }

    // ---- device ----

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
            "共 {} 页（{} 笔），书写范围 {w:.1} × {h:.1} mm。{}请确认纸张已放好、笔已装好，起点为当前原点；写完后笔会回到原点。",
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

    // ---- chrome ----

    /// Quick access toolbar + 文件 + tab titles + collapse chevron.
    #[inline(never)]
    fn tab_strip(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);
        let user = cx.global::<AppState>().user.as_ref().map(|u| u.name.clone()).unwrap_or_default();
        let act = |a: Box<dyn Action>| ribbon_tabs::editor_action(self.doc.clone(), a);
        h_flex()
            .h(px(36.))
            .px_2()
            .gap_1()
            .child(ribbon::icon_btn("qat-save", IconName::Save, "保存 (Ctrl+S)").on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.save_doc(false, w, cx))))
            .child(ribbon::icon_btn("qat-undo", IconName::Undo2, "撤销 (Ctrl+Z)").on_click(act(Box::new(doc_view::Undo))))
            .child(ribbon::icon_btn("qat-redo", IconName::Redo2, "重做 (Ctrl+Y)").on_click(act(Box::new(doc_view::Redo))))
            .child(div().w(px(1.)).h(px(18.)).mx_1().bg(border))
            .child(
                Button::new("file-tab")
                    .primary()
                    .small()
                    .px_4()
                    .label("文件")
                    .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.open_backstage(Stage::New, w, cx))),
            )
            .children(TABS.iter().enumerate().map(|(i, label)| {
                ribbon::tab_item(i, label, i == self.tab, cx).on_click(cx.listener(move |t, _: &ClickEvent, _w, cx| {
                    t.tab = i;
                    t.ribbon_collapsed = false;
                    cx.notify();
                }))
            }))
            .child(div().flex_1())
            .child(div().text_xs().text_color(muted).child(user))
            .child(
                ribbon::icon_btn(
                    "collapse",
                    if self.ribbon_collapsed { IconName::ChevronDown } else { IconName::ChevronUp },
                    if self.ribbon_collapsed { "展开功能区" } else { "折叠功能区" },
                )
                .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                    t.ribbon_collapsed = !t.ribbon_collapsed;
                    cx.notify();
                })),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn status_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let st = cx.global::<AppState>();
        let font = st.active_font.as_ref().map(|f| f.name.clone()).unwrap_or_else(|| "未选择字体".into());
        let template = st.settings.page.template.clone().unwrap_or_else(|| "空白页".into());
        let d = self.doc.read(cx);
        let layout = d.document();
        let pages = layout.pages.len().max(1);
        let words = d.doc().chars().iter().filter(|c| !c.is_whitespace()).count();
        let missing = layout.missing.len();
        let zoom_pct = (d.zoom() / doc_view::ZOOM_100 * 100.0).round();
        let caret_page = d.caret_page() + 1;
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
            .child(format!("第 {caret_page} 页，共 {pages} 页"))
            .child(format!("{words} 个字"))
            .when(missing > 0, |el| {
                let chars: String = layout.missing.iter().take(12).collect();
                el.child(format!("缺字 {missing}：{chars}"))
            })
            .child(format!("字体：{font}"))
            .child(format!("模板：{template}"))
            .child(dev_summary)
            .when_some(job, |el, j| {
                let s = match j.awaiting {
                    Some(n) => format!("等待换纸（第 {} 页）", n + 1),
                    None => format!("书写 第 {}/{} 页 {:.0}%{}", j.page + 1, j.pages, j.percent(), if j.paused { "（暂停）" } else { "" }),
                };
                el.child(s)
            })
            .child(div().flex_1())
            .child(ribbon::icon_btn("sb-zout", IconName::Minus, "缩小").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.zoom_by(1.0 / 1.1, cx))))
            .child(format!("{zoom_pct}%"))
            .child(ribbon::icon_btn("sb-zin", IconName::Plus, "放大").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.zoom_by(1.1, cx))))
            .into_any_element()
    }
}

impl Render for ChiXieApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (bg, fg, border, ribbon_bg) = (theme.background, theme.foreground, theme.border, theme.secondary);
        if cx.global::<AppState>().user.is_none() {
            return div().size_full().bg(bg).text_color(fg).child(self.login.clone()).into_any_element();
        }
        if let Some(stage) = self.backstage {
            return self.render_backstage(stage, cx);
        }
        v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .child(div().bg(ribbon_bg).child(self.tab_strip(cx)))
            .when(!self.ribbon_collapsed, |el| {
                el.child(div().w_full().h(px(RIBBON_H)).bg(ribbon_bg).border_b_1().border_color(border).py_1().child(self.ribbon_body(cx)))
            })
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

pub(crate) fn fmt_num(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn parse_in(s: &Entity<InputState>, cx: &App, min: f32, max: f32) -> Option<f32> {
    s.read(cx).value().trim().parse::<f32>().ok().filter(|v| (min..=max).contains(v))
}
