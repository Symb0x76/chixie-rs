//! Font browser: the account's collected fonts and the online catalog (1-based pages of 9,
//! with a name filter). "使用" fetches and decrypts the font in the session worker; the app makes
//! it the active font when `FontLoaded` arrives.

use chixie_core::api::FontInfo;
use chixie_core::session::{Command, FontListKind};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::state::AppState;

pub struct FontsView {
    tab: usize,
    collected: Vec<FontInfo>,
    online: Vec<FontInfo>,
    page: i64,
    online_requested: bool,
    search: Entity<InputState>,
    loading_font: Option<i64>,
    _subs: Vec<Subscription>,
}

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

    pub fn name_of(&self, id: i64) -> Option<String> {
        self.collected.iter().chain(&self.online).find(|f| f.id == id).map(|f| f.font_name.clone())
    }

    pub fn font_loaded(&mut self, cx: &mut Context<Self>) {
        self.loading_font = None;
        cx.notify();
    }

    pub fn refresh_collected(&mut self, cx: &mut Context<Self>) {
        cx.global::<AppState>().session.send(Command::CollectedFonts);
    }

    fn request_online(&mut self, page: i64, cx: &mut Context<Self>) {
        self.online_requested = true;
        let search = self.search.read(cx).value().trim().to_string();
        cx.global::<AppState>().session.send(Command::OnlineFonts { page: page.max(1), search });
    }

    pub fn use_font(&mut self, id: i64, cx: &mut Context<Self>) {
        self.loading_font = Some(id);
        cx.global::<AppState>().session.send(Command::LoadFont { id });
        cx.notify();
    }

    fn rows(&self, list: &[FontInfo], cx: &mut Context<Self>) -> Vec<AnyElement> {
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
                    .child(div().flex_1().child(f.font_name.clone()))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{} 字", f.character_number)))
                    .child(
                        Button::new(("use", id as usize))
                            .small()
                            .label(if is_active { "使用中" } else { "使用" })
                            .when(!is_active, |b| b.primary())
                            .loading(self.loading_font == Some(id))
                            .disabled(is_active || self.loading_font.is_some())
                            .on_click(cx.listener(move |this, _ev: &ClickEvent, _w, cx| this.use_font(id, cx))),
                    )
                    .into_any_element()
            })
            .collect()
    }
}

impl Render for FontsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let online = self.tab == 1;
        let list = if online { self.online.clone() } else { self.collected.clone() };
        let rows = self.rows(&list, cx);
        let theme = cx.theme();
        let empty = rows.is_empty();

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                TabBar::new("font-tabs")
                    .selected_index(self.tab)
                    .children([Tab::new().label("我的收藏"), Tab::new().label("在线字体库")])
                    .on_click(cx.listener(|this, ix: &usize, _w, cx| {
                        this.tab = *ix;
                        if *ix == 1 && !this.online_requested {
                            this.request_online(1, cx);
                        }
                        cx.notify();
                    })),
            )
            .child(if online {
                h_flex()
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
                    .into_any_element()
            } else {
                h_flex()
                    .gap_2()
                    .child(Button::new("refresh").label("刷新").on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| this.refresh_collected(cx))))
                    .into_any_element()
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_1()
                    .when(empty, |d| d.child(div().p_4().text_color(theme.muted_foreground).child("暂无字体")))
                    .children(rows)
                    .overflow_y_scrollbar(),
            )
    }
}
