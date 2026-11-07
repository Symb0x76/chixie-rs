//! 字体库: fonts saved on this computer (usable offline, no login), the account's collected fonts
//! and the online catalog (1-based pages of 9, with a name filter; both need login).
//! Cloud "使用" fetches and decrypts the font in the session worker; the app saves it to the local
//! store and makes it active when `FontLoaded` arrives. Local "使用" is handled by the app.

use chixie_core::api::FontInfo;
use chixie_core::font_store::LocalFont;
use chixie_core::session::{Command, FontListKind};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::state::AppState;

pub enum FontsEvent {
    /// Use a font from the local store.
    UseLocal(i64),
    /// Delete a font from the local store.
    RemoveLocal(i64),
    /// The user wants to log in (cloud tabs while logged out).
    Login,
}

pub struct FontsView {
    /// 0 本地字体, 1 我的收藏, 2 在线字体库.
    tab: usize,
    collected: Vec<FontInfo>,
    online: Vec<FontInfo>,
    page: i64,
    online_requested: bool,
    search: Entity<InputState>,
    loading_font: Option<i64>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<FontsEvent> for FontsView {}

impl FontsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索字体名称"));
        let sub = cx.subscribe_in(&search, window, |this, _s, ev: &InputEvent, _w, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.request_online(1, cx);
            }
        });
        let obs = cx.observe_global::<AppState>(|_, cx| cx.notify());
        Self {
            tab: 0,
            collected: Vec::new(),
            online: Vec::new(),
            page: 1,
            online_requested: false,
            search,
            loading_font: None,
            _subs: vec![sub, obs],
        }
    }

    pub fn set_list(&mut self, kind: FontListKind, page: i64, fonts: Vec<FontInfo>, cx: &mut Context<Self>) {
        match kind {
            FontListKind::Collected => self.collected = fonts,
            FontListKind::Online => {
                self.page = page;
                self.online = fonts;
            }
        }
        cx.notify();
    }

    pub fn collected(&self) -> &[FontInfo] {
        &self.collected
    }

    /// Forget cloud lists (after logout).
    pub fn clear_cloud(&mut self, cx: &mut Context<Self>) {
        self.collected.clear();
        self.online.clear();
        self.online_requested = false;
        cx.notify();
    }

    pub fn name_of(&self, id: i64) -> Option<String> {
        self.collected.iter().chain(&self.online).find(|f| f.id == id).map(|f| f.font_name.clone())
    }

    pub fn font_loaded(&mut self, cx: &mut Context<Self>) {
        self.loading_font = None;
        cx.notify();
    }

    pub fn refresh_collected(&mut self, cx: &mut Context<Self>) {
        if cx.global::<AppState>().user.is_some() {
            cx.global::<AppState>().session.send(Command::CollectedFonts);
        }
    }

    fn request_online(&mut self, page: i64, cx: &mut Context<Self>) {
        self.online_requested = true;
        let search = self.search.read(cx).value().trim().to_string();
        cx.global::<AppState>().session.send(Command::OnlineFonts { page: page.max(1), search });
    }

    /// Download (login needed) and use a cloud font.
    pub fn use_font(&mut self, id: i64, cx: &mut Context<Self>) {
        self.loading_font = Some(id);
        cx.global::<AppState>().session.send(Command::LoadFont { id });
        cx.notify();
    }

    fn cloud_rows(&self, list: &[FontInfo], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let st = cx.global::<AppState>();
        let active = st.active_font.as_ref().map(|f| f.id);
        let local: Vec<i64> = st.fonts.list().iter().map(|f| f.id).collect();
        let theme = cx.theme();
        list.iter()
            .map(|f| {
                let id = f.id;
                let is_active = active == Some(id);
                let saved = local.contains(&id);
                h_flex()
                    .w_full()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .when(is_active, |d| d.bg(theme.accent))
                    .child(div().w(px(56.)).text_xs().text_color(theme.muted_foreground).child(format!("#{id}")))
                    .child(div().flex_1().child(f.font_name.clone()))
                    .when(saved, |d| d.child(div().text_xs().text_color(theme.muted_foreground).child("已下载")))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{} 字", f.character_number)))
                    .child(
                        Button::new(("use", id as usize))
                            .small()
                            .label(if is_active { "使用中" } else if saved { "使用" } else { "下载并使用" })
                            .when(!is_active, |b| b.primary())
                            .loading(self.loading_font == Some(id))
                            .disabled(is_active || self.loading_font.is_some())
                            .on_click(cx.listener(move |this, _ev: &ClickEvent, _w, cx| {
                                if saved {
                                    cx.emit(FontsEvent::UseLocal(id));
                                } else {
                                    this.use_font(id, cx);
                                }
                            })),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn local_rows(&self, list: &[LocalFont], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let active = cx.global::<AppState>().active_font.as_ref().map(|f| f.id);
        let theme = cx.theme();
        list.iter()
            .map(|f| {
                let id = f.id;
                let is_active = active == Some(id);
                h_flex()
                    .w_full()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .when(is_active, |d| d.bg(theme.accent))
                    .child(div().w(px(56.)).text_xs().text_color(theme.muted_foreground).child(format!("#{id}")))
                    .child(div().flex_1().child(f.name.clone()))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{} 字", f.glyphs)))
                    .child(
                        Button::new(("local-use", id as usize))
                            .small()
                            .label(if is_active { "使用中" } else { "使用" })
                            .when(!is_active, |b| b.primary())
                            .disabled(is_active)
                            .on_click(cx.listener(move |_this, _ev: &ClickEvent, _w, cx| cx.emit(FontsEvent::UseLocal(id)))),
                    )
                    .child(
                        Button::new(("local-del", id as usize))
                            .small()
                            .ghost()
                            .label("删除")
                            .disabled(is_active)
                            .on_click(cx.listener(move |_this, _ev: &ClickEvent, _w, cx| cx.emit(FontsEvent::RemoveLocal(id)))),
                    )
                    .into_any_element()
            })
            .collect()
    }
}

impl Render for FontsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let logged_in = cx.global::<AppState>().user.is_some();
        let local = cx.global::<AppState>().fonts.list();
        let dir = cx.global::<AppState>().fonts.dir().display().to_string();
        let rows = match self.tab {
            0 => self.local_rows(&local, cx),
            1 => self.cloud_rows(&self.collected.clone(), cx),
            _ => self.cloud_rows(&self.online.clone(), cx),
        };
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let empty = rows.is_empty();
        let cloud = self.tab > 0;

        let toolbar = match self.tab {
            0 => h_flex().gap_2().child(div().text_xs().text_color(muted).child(format!("已下载的字体保存在 {dir}，无需登录即可使用。"))).into_any_element(),
            1 if logged_in => h_flex()
                .gap_2()
                .child(Button::new("refresh").label("刷新").on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| this.refresh_collected(cx))))
                .into_any_element(),
            2 if logged_in => h_flex()
                .gap_2()
                .child(div().w(px(260.)).child(Input::new(&self.search).cleanable(true)))
                .child(Button::new("search").label("搜索").on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| this.request_online(1, cx))))
                .child(
                    Button::new("prev")
                        .label("上一页")
                        .disabled(self.page <= 1)
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| this.request_online(this.page - 1, cx))),
                )
                .child(div().text_sm().child(format!("第 {} 页", self.page)))
                .child(
                    Button::new("next")
                        .label("下一页")
                        .disabled(self.online.len() < 9)
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| this.request_online(this.page + 1, cx))),
                )
                .into_any_element(),
            _ => div().into_any_element(),
        };

        let body = if cloud && !logged_in {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .child(div().text_color(muted).child("云端字体需要登录。下载过的字体会保存在本机，之后无需登录。"))
                .child(Button::new("login").primary().label("登录").on_click(cx.listener(|_this, _e: &ClickEvent, _w, cx| cx.emit(FontsEvent::Login))))
                .into_any_element()
        } else {
            v_flex()
                .flex_1()
                .min_h_0()
                .gap_1()
                .when(empty, |d| {
                    d.child(div().p_4().text_color(muted).child(if self.tab == 0 { "还没有下载字体：在「我的收藏」或「在线字体库」中下载。" } else { "暂无字体" }))
                })
                .children(rows)
                .overflow_y_scrollbar()
                .into_any_element()
        };

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                TabBar::new("font-tabs")
                    .selected_index(self.tab)
                    .children([Tab::new().label("本地字体"), Tab::new().label("我的收藏"), Tab::new().label("在线字体库")])
                    .on_click(cx.listener(move |this, ix: &usize, _w, cx| {
                        this.tab = *ix;
                        if *ix == 2 && logged_in && !this.online_requested {
                            this.request_online(1, cx);
                        }
                        cx.notify();
                    })),
            )
            .child(toolbar)
            .child(body)
    }
}
