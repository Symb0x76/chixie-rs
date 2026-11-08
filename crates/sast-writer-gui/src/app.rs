//! Main window, laid out like Microsoft Word: login start screen, then a tab strip with a quick
//! access toolbar and a filled 文件 tab (opens the Backstage), the ribbon (开始 / 布局 / 手写 /
//! 设备 / 视图), the page editor with the device task pane, and a status bar.
//! Session and device events are pumped here and fanned out to views + toasts.

mod backstage;
mod ribbon_tabs;

use std::path::PathBuf;
use std::time::Duration;

use sast_writer_core::config::import_chixie3;
use sast_writer_core::device::{DeviceHandle, Event as DevEvent, Job};
use sast_writer_core::editor::EditDoc;
use sast_writer_core::gcode;
use sast_writer_core::session::{Command, Event, FontListKind};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme as _, IndexPath, Sizable as _, TitleBar, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub use backstage::Stage;

use crate::device::{DevicePanel, Notice};
use crate::doc_view::{self, DocView};
use crate::file_ops;
use crate::font_preview::{FontItem, FontPreview};
use crate::fonts::{FontsEvent, FontsView};
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

pub struct SastWriterApp {
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
    font_select: Entity<SelectState<Vec<FontItem>>>,
    /// Font ids, parallel to the font select's items.
    font_ids: Vec<i64>,
    /// Local fonts whose preview is being built in the background.
    previews_pending: std::collections::HashSet<i64>,
    style_dialog: Entity<StyleDialog>,
    backstage_focus: FocusHandle,
    /// Where 保存 writes; `None` until 另存为 / 打开 of a `.chx`.
    file: Option<PathBuf>,
    zoom_slider: Entity<SliderState>,
    /// Last title pushed to the OS window (taskbar).
    win_title: String,
    _tasks: Vec<Task<()>>,
    _subs: Vec<Subscription>,
}

impl SastWriterApp {
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
        let initial = file_ops::open(&draft).map(|l| l.doc).unwrap_or_else(|_| EditDoc::from_text("你好，SAST Writer！\n春眠不觉晓，处处闻啼鸟。"));
        let doc = cx.new(|cx| DocView::new(initial, window, cx));
        let page = cx.global::<AppState>().settings.page.clone();
        let font_size = cx.new(|cx| InputState::new(window, cx).step(5.0).min(5.0).max(1000.0).default_value(fmt_num(page.font_size)));
        let line_height = cx.new(|cx| InputState::new(window, cx).step(0.25).min(1.0).max(200.0).default_value(fmt_num(page.line_height)));
        let font_select = cx.new(|cx| SelectState::new(Vec::<FontItem>::new(), None, window, cx));
        let templates = cx.new(|cx| TemplatesView::new(window, cx));
        let style_dialog = cx.new(|cx| StyleDialog::new(window, cx));
        let device = cx.new(|cx| DevicePanel::new(device, window, cx));
        let zoom_slider = cx.new(|_| SliderState::new().min(0.0).max(100.0).step(0.5).default_value(50.0));

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
            cx.observe_in(&doc, window, |this, _, window, cx| {
                this.sync_zoom_slider(window, cx);
                cx.notify();
            }),
            cx.subscribe_in(&zoom_slider, window, |this, _s, ev: &SliderEvent, _w, cx| {
                if let SliderEvent::Change(v) = ev {
                    let z = slider_to_zoom(v.start()) / 100.0 * doc_view::ZOOM_100;
                    this.doc.update(cx, |d, cx| d.set_zoom(z, cx));
                }
            }),
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
            cx.subscribe_in(&font_select, window, |this, _s, ev: &SelectEvent<Vec<FontItem>>, window, cx| {
                if let SelectEvent::Confirm(Some(id)) = ev {
                    this.choose_font(*id, window, cx);
                }
            }),
            cx.subscribe_in(&fonts, window, |this, _v, ev: &FontsEvent, window, cx| match ev {
                FontsEvent::UseLocal(id) => this.load_local_font(*id, window, cx),
                FontsEvent::RemoveLocal(id) => this.remove_local_font(*id, window, cx),
                FontsEvent::Login => {
                    window.close_dialog(cx);
                    this.open_backstage(Stage::Account, window, cx);
                }
            }),
            cx.subscribe_in(&templates, window, |this, _v, ev: &TemplateEvent, window, cx| match ev {
                TemplateEvent::Apply(name) => {
                    this.apply_template(Some(name.clone()), cx);
                    this.close_backstage(window, cx);
                }
            }),
        ];
        // No login needed: list downloaded fonts and reopen the last one from disk.
        cx.defer_in(window, |this, window, cx| {
            this.sync_font_select(window, cx);
            let st = cx.global::<AppState>();
            if let Some(id) = st.settings.page.font_id.filter(|id| st.fonts.contains(*id)) {
                this.load_local_font(id, window, cx);
            }
        });
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
            previews_pending: Default::default(),
            style_dialog,
            backstage_focus: cx.focus_handle(),
            file: None,
            zoom_slider,
            win_title: String::new(),
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
                // The last-used font is normally already loaded from the local store; fetch it only
                // if it was never downloaded.
                if st.active_font.is_none()
                    && let Some(id) = st.settings.page.font_id.filter(|id| !st.fonts.contains(*id))
                {
                    st.session.send(Command::LoadFont { id });
                }
                st.session.send(Command::CollectedFonts);
                window.push_notification(Notification::success(format!("欢迎，{username}")), cx);
                if self.backstage == Some(Stage::Account) {
                    self.close_backstage(window, cx);
                } else {
                    self.focus_doc(window, cx);
                }
            }
            Event::LoginFailed(msg) => self.login.update(cx, |l, cx| l.on_failed(msg, cx)),
            // Logging out only affects cloud access: the document and downloaded fonts stay.
            Event::LoggedOut => {
                cx.global_mut::<AppState>().user = None;
                self.fonts.update(cx, |f, cx| f.clear_cloud(cx));
                self.sync_font_select(window, cx);
                window.push_notification(Notification::info("已退出登录；本地字体仍可使用"), cx);
            }
            Event::SessionLost(msg) => {
                cx.global_mut::<AppState>().user = None;
                self.fonts.update(cx, |f, cx| f.clear_cloud(cx));
                self.sync_font_select(window, cx);
                window.push_notification(Notification::warning(format!("登录已失效（{msg}）；本地字体仍可使用")), cx);
            }
            Event::Fonts { kind, page, fonts } => {
                let collected = kind == FontListKind::Collected;
                // A font downloaded before the lists arrived was saved under a placeholder name;
                // the server's name wins.
                let renamed = self.repair_font_names(&fonts, cx);
                self.fonts.update(cx, |f, cx| f.set_list(kind, page, fonts, cx));
                if collected || renamed {
                    self.sync_font_select(window, cx);
                }
            }
            Event::FontLoaded { id, font } => {
                // The last font is fetched at login, often before the font lists arrive.
                let name = self
                    .fonts
                    .read(cx)
                    .name_of(id)
                    .or_else(|| cx.global::<AppState>().fonts.name_of(id).filter(|n| !is_placeholder_name(n)))
                    .unwrap_or_else(|| placeholder_name(id));
                if let Some(p) = FontPreview::new(&font, &name) {
                    cx.global_mut::<AppState>().previews.insert(id, std::sync::Arc::new(p));
                }
                self.fonts.update(cx, |f, cx| f.font_loaded(cx));
                let glyphs = font.glyphs.len();
                // Keep a copy on this computer so the font works without logging in next time.
                let store = cx.global::<AppState>().fonts.clone();
                let (to_save, save_name) = (font.clone(), name.clone());
                cx.background_executor()
                    .spawn(async move {
                        if let Err(e) = store.save(id, &save_name, &to_save) {
                            log::warn!("saving font {id} locally: {e}");
                        }
                    })
                    .detach();
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

    /// Font picker items = downloaded fonts, then collected fonts not yet downloaded (marked
    /// 云端, login needed); selection = the active font.
    fn sync_font_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.global::<AppState>();
        let local = st.fonts.list();
        let mut items: Vec<FontItem> = local
            .iter()
            .map(|f| FontItem { id: f.id, name: f.name.clone().into(), cloud: false })
            .collect();
        for f in self.fonts.read(cx).collected() {
            if !items.iter().any(|it| it.id == f.id) {
                items.push(FontItem { id: f.id, name: f.font_name.clone().into(), cloud: true });
            }
        }
        let missing: Vec<(i64, String)> =
            local.into_iter().filter(|f| !st.previews.contains_key(&f.id) && !self.previews_pending.contains(&f.id)).map(|f| (f.id, f.name)).collect();
        let active = st.active_font.as_ref().map(|f| f.id);
        self.font_ids = items.iter().map(|it| it.id).collect();
        let selected = active.and_then(|id| self.font_ids.iter().position(|&x| x == id)).map(IndexPath::new);
        self.font_select.update(cx, |s, cx| {
            s.set_items(items, window, cx);
            s.set_selected_index(selected, window, cx);
        });
        if !missing.is_empty() {
            self.build_previews(missing, window, cx);
        }
    }

    /// Read local fonts off the UI thread and keep only their small previews.
    fn build_previews(&mut self, fonts: Vec<(i64, String)>, window: &mut Window, cx: &mut Context<Self>) {
        self.previews_pending.extend(fonts.iter().map(|(id, _)| *id));
        let store = cx.global::<AppState>().fonts.clone();
        let task = cx.background_executor().spawn(async move {
            fonts
                .into_iter()
                .filter_map(|(id, name)| {
                    let font = store.load(id).inspect_err(|e| log::warn!("preview of font {id}: {e}")).ok()?;
                    FontPreview::new(&font, &name).map(|p| (id, std::sync::Arc::new(p)))
                })
                .collect::<Vec<_>>()
        });
        cx.spawn_in(window, async move |this, cx| {
            let built = task.await;
            let _ = this.update_in(cx, |app, window, cx| {
                for (id, p) in built {
                    app.previews_pending.remove(&id);
                    cx.global_mut::<AppState>().previews.insert(id, p);
                }
                app.sync_font_select(window, cx);
            });
        })
        .detach();
    }

    /// Rename locally saved fonts after the server's names; true if any changed.
    fn repair_font_names(&mut self, fonts: &[sast_writer_core::api::FontInfo], cx: &mut Context<Self>) -> bool {
        let st = cx.global_mut::<AppState>();
        let mut changed = false;
        for f in fonts {
            match st.fonts.rename(f.id, &f.font_name) {
                Ok(true) => {
                    changed = true;
                    // The preview may have been a sample for want of the name.
                    st.previews.remove(&f.id);
                    if let Some(a) = st.active_font.as_mut().filter(|a| a.id == f.id) {
                        a.name = f.font_name.clone();
                    }
                }
                Ok(false) => {}
                Err(e) => log::warn!("renaming local font {}: {e}", f.id),
            }
        }
        changed
    }

    fn choose_font(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if cx.global::<AppState>().active_font.as_ref().map(|f| f.id) != Some(id) {
            self.use_font_id(id, window, cx);
        }
    }

    /// Use font `id`: from the local store if downloaded, else download it (login needed).
    pub fn use_font_id(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if cx.global::<AppState>().fonts.contains(id) {
            self.load_local_font(id, window, cx);
        } else if cx.global::<AppState>().user.is_some() {
            self.fonts.update(cx, |f, cx| f.use_font(id, cx));
        } else {
            window.push_notification(Notification::info("这个字体还没下载，请先登录"), cx);
            self.open_backstage(Stage::Account, window, cx);
        }
    }

    /// Load a downloaded font from disk (no login, no network).
    pub fn load_local_font(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let store = cx.global::<AppState>().fonts.clone();
        let task = cx.background_executor().spawn(async move {
            let name = store.name_of(id).unwrap_or_else(|| placeholder_name(id));
            store.load(id).map(|f| (name, f))
        });
        cx.spawn_in(window, async move |this, cx| {
            let res = task.await;
            let _ = this.update_in(cx, |app, window, cx| match res {
                Ok((name, font)) => {
                    let st = cx.global_mut::<AppState>();
                    if !st.previews.contains_key(&id)
                        && let Some(p) = FontPreview::new(&font, &name)
                    {
                        st.previews.insert(id, std::sync::Arc::new(p));
                    }
                    st.active_font = Some(ActiveFont { id, name, font: std::sync::Arc::new(font) });
                    st.settings.page.font_id = Some(id);
                    st.save_settings();
                    if window.has_active_dialog(cx) {
                        window.close_dialog(cx);
                    }
                    app.sync_font_select(window, cx);
                    cx.notify();
                }
                Err(e) => window.push_notification(Notification::error(format!("读取本地字体失败：{e}")), cx),
            });
        })
        .detach();
    }

    fn remove_local_font(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = cx.global::<AppState>().fonts.remove(id) {
            window.push_notification(Notification::error(format!("删除失败：{e}")), cx);
        }
        self.sync_font_select(window, cx);
        self.fonts.update(cx, |_, cx| cx.notify());
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
    fn edit_page(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut sast_writer_core::config::PageSettings)) {
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
            dialog.title("字体库").w(px(960.)).child(div().h(px(640.)).child(fonts.clone()))
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
        // The machine's own limits ($$) when known, else the verified board's.
        let machine = self.device.read(cx).params.read(cx).machine.clone();
        let (tx, ty) = machine.as_ref().and_then(|m| m.travel).unwrap_or((TRAVEL_MM, TRAVEL_MM));
        let over = w > tx || h > ty;
        let mut warn = String::new();
        if over {
            warn.push_str(&format!("⚠ 超出机器行程 {tx:.0} × {ty:.0} mm！"));
        }
        if let Some(max) = machine.as_ref().and_then(|m| m.max_feed()).filter(|m| profile.write_speed > *m) {
            warn.push_str(&format!("写字速度 {:.0} 高于机器最大速度 {max:.0}，将按最大速度运行。", profile.write_speed));
        }
        if !profile.firmware.supported() {
            window.push_notification(Notification::error("当前固件设置为 Marlin，暂不支持书写（设备 › 参数设置）"), cx);
            return;
        }
        let desc = format!(
            "共 {} 页（{} 笔），书写范围 {w:.1} × {h:.1} mm。{warn}请确认纸张已放好、笔已装好，起点为当前原点；写完后笔会回到原点。",
            pages.len(),
            doc.stroke_count(),
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
    /// Document name shown in the title bar and the taskbar (Word: "文档1 - Word").
    fn doc_title(&self) -> String {
        let name = self.file.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "文档1".into());
        format!("{name} - SAST Writer")
    }

    /// Word's caption: app mark + quick access toolbar on the left, the document name centred,
    /// the account and the window controls on the right. The whole bar drags the window.
    #[inline(never)]
    fn title_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let c = ribbon::chrome(cx);
        let st = cx.global::<AppState>();
        let user = st.user.as_ref().map(|u| u.name.clone());
        let act = |a: Box<dyn Action>| ribbon_tabs::editor_action(self.doc.clone(), a);
        let in_backstage = self.backstage.is_some();
        let qat = h_flex()
            .gap_0p5()
            // Word for Mac shows no app mark next to the traffic lights.
            .when(!cfg!(target_os = "macos"), |el| el.child(
                div()
                    .size(px(20.))
                    .mr_2()
                    .rounded_sm()
                    .bg(c.title_fg)
                    .text_color(c.title_bg)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("S"),
            ))
            .when(!in_backstage, |el| {
                el.child(
                    ribbon::title_btn("qat-save", IconName::Save, format!("保存 ({})", ribbon::shortcut("S")), &c)
                        .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.save_doc(false, w, cx))),
                )
                .child(ribbon::title_btn("qat-undo", IconName::Undo2, format!("撤销 ({})", ribbon::shortcut("Z")), &c).on_click(act(Box::new(doc_view::Undo))))
                .child(ribbon::title_btn("qat-redo", IconName::Redo2, format!("重做 ({})", ribbon::redo_shortcut()), &c).on_click(act(Box::new(doc_view::Redo))))
            });
        let account = match user {
            Some(name) => h_flex()
                .gap_2()
                .px_2()
                .child(div().text_xs().child(name.clone()))
                .child(
                    div()
                        .size(px(24.))
                        .rounded_full()
                        .bg(c.title_hover)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .child(name.chars().next().map(String::from).unwrap_or_default()),
                )
                .into_any_element(),
            None => div()
                .id("login-btn")
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .h(px(26.))
                .px_3()
                .rounded_sm()
                .flex()
                .items_center()
                .text_xs()
                .cursor_pointer()
                .hover({
                    let h = c.title_hover;
                    move |s| s.bg(h)
                })
                .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new("登录后可下载云端字体").build(w, cx))
                .child("登录")
                .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.open_backstage(Stage::Account, w, cx)))
                .into_any_element(),
        };
        let content = h_flex()
            .id("title-content")
            .relative()
            .flex_1()
            .h_full()
            .pl_2()
            // Centred like Word, independent of the side widths.
            .child(div().absolute().inset_0().flex().items_center().justify_center().text_sm().child(self.doc_title()))
            .child(qat)
            .child(div().flex_1())
            .child(account);
        if cfg!(target_os = "windows") {
            // Our own buttons; the OS handles drag, snap and the buttons through hit-test areas.
            h_flex()
                .id("title-bar")
                .w_full()
                .h(px(ribbon::TITLE_H))
                .flex_shrink_0()
                .bg(c.title_bg)
                .text_color(c.title_fg)
                .child(content.window_control_area(WindowControlArea::Drag))
                .child(ribbon::window_controls(window, &c))
                .into_any_element()
        } else {
            // macOS: room for the system traffic lights on the left. Linux: gpui-component draws
            // the window buttons (only under client-side decorations). Both drag by mouse-move.
            TitleBar::new()
                .h(px(ribbon::TITLE_H))
                .bg(c.title_bg)
                .text_color(c.title_fg)
                .border_color(cx.theme().border.opacity(0.6))
                .child(content)
                .into_any_element()
        }
    }

    /// 文件 + tab titles + collapse chevron, on the light strip above the ribbon card.
    #[inline(never)]
    fn tab_strip(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(px(34.))
            .px_2()
            .gap_0p5()
            .items_end()
            .child(ribbon::tab_item(TABS.len(), "文件", false, cx).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.open_backstage(Stage::New, w, cx))))
            .children(TABS.iter().enumerate().map(|(i, label)| {
                ribbon::tab_item(i, label, i == self.tab, cx).on_click(cx.listener(move |t, _: &ClickEvent, _w, cx| {
                    t.tab = i;
                    t.ribbon_collapsed = false;
                    cx.notify();
                }))
            }))
            .child(div().flex_1())
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
        let (dev_summary, dev_connected, job) = (dev.summary(), dev.connected.is_some(), dev.job);
        let theme = cx.theme();
        let (muted, hover, primary, danger) = (theme.muted_foreground, theme.secondary_hover, theme.primary, theme.danger);
        let strip = ribbon::chrome(cx).strip_bg;
        // Word's status bar items are flat cells that highlight on hover.
        let cell = move |id: &'static str| {
            h_flex().id(id).h_full().px_2().gap_1().items_center().hover(move |s| s.bg(hover))
        };
        h_flex()
            .w_full()
            .h(px(24.))
            .px_1()
            .flex_shrink_0()
            .bg(strip)
            .text_color(muted)
            .text_xs()
            .child(cell("sb-page").child(format!("第 {caret_page} 页，共 {pages} 页")))
            .child(cell("sb-words").child(format!("{words} 个字")))
            .when(missing > 0, |el| {
                let chars: String = layout.missing.iter().take(12).collect();
                el.child(
                    cell("sb-missing")
                        .text_color(danger)
                        .child(gpui_kit::component::Icon::new(IconName::TriangleAlert).with_size(px(12.)))
                        .child(format!("缺字 {missing}：{chars}")),
                )
            })
            .child(
                cell("sb-font")
                    .cursor_pointer()
                    .child(format!("字体：{font}"))
                    .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.open_fonts(w, cx))),
            )
            .child(cell("sb-template").child(format!("模板：{template}")))
            .child(
                cell("sb-device")
                    .cursor_pointer()
                    .child(div().size(px(7.)).rounded_full().bg(if dev_connected { gpui_kit::green() } else { muted.opacity(0.6) }))
                    .child(dev_summary)
                    .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                        t.tab = TAB_DEVICE;
                        t.show_pane = true;
                        t.ribbon_collapsed = false;
                        cx.notify();
                    })),
            )
            .when_some(job, |el, j| {
                let s = match j.awaiting {
                    Some(n) => format!("等待换纸（第 {} 页）", n + 1),
                    None => format!("书写 第 {}/{} 页 {:.0}%{}", j.page + 1, j.pages, j.percent(), if j.paused { "（暂停）" } else { "" }),
                };
                el.child(
                    cell("sb-job")
                        .text_color(primary)
                        .child(div().w(px(60.)).h(px(4.)).rounded_full().bg(muted.opacity(0.3)).child(
                            div().h_full().rounded_full().bg(primary).w(relative((j.percent() / 100.0).clamp(0.0, 1.0))),
                        ))
                        .child(s),
                )
            })
            .child(div().flex_1())
            .child(ribbon::icon_btn("sb-zout", IconName::Minus, "缩小").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.zoom_by(1.0 / 1.1, cx))))
            .child(div().w(px(110.)).px_1().child(Slider::new(&self.zoom_slider)))
            .child(ribbon::icon_btn("sb-zin", IconName::Plus, "放大").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.zoom_by(1.1, cx))))
            .child(
                cell("sb-zoom")
                    .w(px(52.))
                    .justify_center()
                    .cursor_pointer()
                    .child(format!("{zoom_pct}%"))
                    .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                        t.doc.update(cx, |d, cx| d.set_zoom(doc_view::ZOOM_100, cx));
                    })),
            )
            .into_any_element()
    }

    /// Word-style task pane: titled header with a close button over the device panel.
    #[inline(never)]
    fn task_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let border = theme.border;
        let card = ribbon::chrome(cx).card_bg;
        v_flex()
            .w(px(320.))
            .h_full()
            .flex_shrink_0()
            .bg(card)
            .border_l_1()
            .border_color(border)
            .child(
                h_flex()
                    .h(px(40.))
                    .px_3()
                    .flex_shrink_0()
                    .child(div().flex_1().text_base().font_weight(FontWeight::SEMIBOLD).child("设备"))
                    .child(ribbon::icon_btn("pane-close", IconName::Close, "关闭").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                        t.show_pane = false;
                        cx.notify();
                    }))),
            )
            .child(div().flex_1().min_h_0().child(self.device.clone()))
            .into_any_element()
    }

    /// Keep the status bar slider on the editor's zoom (Ctrl+wheel, buttons, 页宽 change it too).
    fn sync_zoom_slider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pct = self.doc.read(cx).zoom() / doc_view::ZOOM_100 * 100.0;
        let v = zoom_to_slider(pct);
        if (self.zoom_slider.read(cx).value().start() - v).abs() > 0.01 {
            self.zoom_slider.update(cx, |s, cx| s.set_value(v, window, cx));
        }
    }
}

/// Word's zoom slider is two-sided: the left half spans the minimum..100 %, the right half
/// 100 %..maximum, so 100 % always sits in the middle.
fn zoom_to_slider(pct: f32) -> f32 {
    let (lo, hi) = zoom_pct_range();
    if pct <= 100.0 { (pct - lo) / (100.0 - lo) * 50.0 } else { 50.0 + (pct - 100.0) / (hi - 100.0) * 50.0 }.clamp(0.0, 100.0)
}

fn slider_to_zoom(v: f32) -> f32 {
    let (lo, hi) = zoom_pct_range();
    if v <= 50.0 { lo + v / 50.0 * (100.0 - lo) } else { 100.0 + (v - 50.0) / 50.0 * (hi - 100.0) }
}

fn zoom_pct_range() -> (f32, f32) {
    (doc_view::ZOOM_MIN / doc_view::ZOOM_100 * 100.0, doc_view::ZOOM_MAX / doc_view::ZOOM_100 * 100.0)
}

impl Render for SastWriterApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (bg, fg, border) = (theme.background, theme.foreground, theme.border);
        let c = ribbon::chrome(cx);
        let title = self.doc_title();
        if title != self.win_title {
            window.set_window_title(&title);
            self.win_title = title;
        }
        let title_bar = self.title_bar(window, cx);
        let body = if let Some(stage) = self.backstage {
            self.render_backstage(stage, cx)
        } else {
            v_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(
                    v_flex()
                        .w_full()
                        .flex_shrink_0()
                        .bg(c.strip_bg)
                        .child(self.tab_strip(cx))
                        .when(!self.ribbon_collapsed, |el| {
                            el.child(
                                div().w_full().px_2().pb_1p5().child(
                                    div()
                                        .w_full()
                                        .h(px(RIBBON_H))
                                        .py_1()
                                        .rounded_lg()
                                        .bg(c.card_bg)
                                        .border_1()
                                        .border_color(border.opacity(0.7))
                                        .shadow_xs()
                                        .overflow_hidden()
                                        .child(self.ribbon_body(cx)),
                                ),
                            )
                        })
                        .when(self.ribbon_collapsed, |el| el.pb_1()),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        .border_t_1()
                        .border_color(border.opacity(0.6))
                        .child(div().flex_1().h_full().min_w_0().child(self.doc.clone()))
                        .when(self.show_pane, |el| el.child(self.task_pane(cx))),
                )
                .child(self.status_bar(cx))
                .into_any_element()
        };
        let root = v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .on_action(cx.listener(|t, _: &doc_view::Save, w, cx| t.save_doc(false, w, cx)))
            .child(title_bar)
            .child(body);
        if cfg!(target_os = "linux") {
            // Client-side decorations: shadow and resize edges (a no-op under a server frame).
            gpui_kit::component::window_border().child(root).into_any_element()
        } else {
            root.into_any_element()
        }
    }
}

/// Name for a font whose real name is not known yet (fixed by `repair_font_names`).
fn placeholder_name(id: i64) -> String {
    format!("字体 #{id}")
}

fn is_placeholder_name(name: &str) -> bool {
    name.starts_with("字体 #")
}

pub(crate) fn fmt_num(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn parse_in(s: &Entity<InputState>, cx: &App, min: f32, max: f32) -> Option<f32> {
    s.read(cx).value().trim().parse::<f32>().ok().filter(|v| (min..=max).contains(v))
}
