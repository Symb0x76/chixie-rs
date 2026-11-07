//! Main window: login gate, then a sidebar shell (书写 / 字体 / 设备 / 设置) with a status bar.
//! Session worker events are pumped here and fanned out to views + toasts.

use chixie_core::session::{Command, Event};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem};
use gpui_kit::component::{ActiveTheme as _, IconName, StyledExt as _, Theme, ThemeMode, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use crate::fonts::FontsView;
use crate::login::LoginView;
use crate::state::{ActiveFont, AppState, User};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Write,
    Fonts,
    Device,
    Settings,
}

const PAGES: [(Page, &str, IconName); 4] = [
    (Page::Write, "书写", IconName::FileText),
    (Page::Fonts, "字体", IconName::Palette),
    (Page::Device, "设备", IconName::LayoutDashboard),
    (Page::Settings, "设置", IconName::Settings),
];

pub struct ChiXieApp {
    page: Page,
    pub login: Entity<LoginView>,
    pub fonts: Entity<FontsView>,
    _tasks: Vec<Task<()>>,
    _subs: Vec<Subscription>,
}

impl ChiXieApp {
    pub fn new(events: async_channel::Receiver<Event>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let login = cx.new(|cx| LoginView::new(window, cx));
        let fonts = cx.new(|cx| FontsView::new(window, cx));
        let pump = cx.spawn_in(window, async move |this, cx| {
            while let Ok(ev) = events.recv().await {
                if this.update_in(cx, |app, window, cx| app.on_session_event(ev, window, cx)).is_err() {
                    break;
                }
            }
        });
        let obs = cx.observe_global::<AppState>(|_, cx| cx.notify());
        Self { page: Page::Write, login, fonts, _tasks: vec![pump], _subs: vec![obs] }
    }

    pub fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
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
            }
            Event::LoginFailed(msg) => {
                self.login.update(cx, |l, cx| l.on_failed(msg, cx));
            }
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
            Event::Fonts { kind, page, fonts } => {
                self.fonts.update(cx, |f, cx| f.set_list(kind, page, fonts, cx));
            }
            Event::FontLoaded { id, font } => {
                let name = self.fonts.read(cx).name_of(id).unwrap_or_else(|| format!("字体 #{id}"));
                self.fonts.update(cx, |f, cx| f.font_loaded(cx));
                let glyphs = font.glyphs.len();
                let st = cx.global_mut::<AppState>();
                st.active_font = Some(ActiveFont { id, name: name.clone(), font });
                st.settings.page.font_id = Some(id);
                st.save_settings();
                window.push_notification(Notification::success(format!("已加载 {name}（{glyphs} 字）")), cx);
            }
            Event::Error(msg) => {
                self.fonts.update(cx, |f, cx| f.font_loaded(cx));
                window.push_notification(Notification::error(msg), cx);
            }
        }
        cx.notify();
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        Sidebar::new("nav").collapsible(false).w(px(180.)).child(SidebarMenu::new().children(PAGES.iter().map(
            |(page, label, icon)| {
                let page = *page;
                SidebarMenuItem::new(*label)
                    .icon(icon.clone())
                    .active(self.page == page)
                    .on_click(cx.listener(move |this, _ev: &ClickEvent, _w, cx| this.set_page(page, cx)))
            },
        )))
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let st = cx.global::<AppState>();
        let user = st.user.as_ref().map(|u| format!("{}（{}）", u.name, u.role)).unwrap_or_default();
        let font = st.active_font.as_ref().map(|f| format!("字体：{}", f.name)).unwrap_or_else(|| "未选择字体".into());
        let theme = cx.theme();
        h_flex()
            .w_full()
            .h(px(28.))
            .px_3()
            .gap_4()
            .border_t_1()
            .border_color(theme.border)
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(user)
            .child(font)
            .child("设备：未连接")
    }

    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        match self.page {
            Page::Fonts => self.fonts.clone().into_any_element(),
            Page::Write => {
                let font = cx.global::<AppState>().active_font.as_ref().map(|f| f.name.clone());
                placeholder(
                    "书写",
                    font.map(|n| format!("当前字体：{n}。文本编辑与预览将在下一阶段（M4）加入。"))
                        .unwrap_or_else(|| "请先在「字体」中选择一个字体。".into()),
                    muted,
                )
            }
            Page::Device => placeholder("设备", "连接、点动、下笔测试与书写任务将在 M5 加入。".into(), muted),
            Page::Settings => {
                let dark = cx.theme().mode == ThemeMode::Dark;
                v_flex()
                    .p_6()
                    .gap_4()
                    .child(div().text_lg().font_semibold().child("设置"))
                    .child(
                        Button::new("theme")
                            .label(if dark { "切换到浅色" } else { "切换到深色" })
                            .on_click(cx.listener(move |_this, _e: &ClickEvent, window, cx| {
                                Theme::change(if dark { ThemeMode::Light } else { ThemeMode::Dark }, Some(window), cx);
                                let st = cx.global_mut::<AppState>();
                                st.settings.ui.dark = !dark;
                                st.save_settings();
                            })),
                    )
                    .child(
                        Button::new("logout")
                            .danger()
                            .label("退出登录")
                            .on_click(cx.listener(|_this, _e: &ClickEvent, _w, cx| {
                                cx.global::<AppState>().session.send(Command::Logout);
                            })),
                    )
                    .into_any_element()
            }
        }
    }
}

fn placeholder(title: &str, body: String, muted: Hsla) -> AnyElement {
    v_flex()
        .p_6()
        .gap_2()
        .child(div().text_lg().font_semibold().child(title.to_string()))
        .child(div().text_sm().text_color(muted).child(body))
        .into_any_element()
}

impl Render for ChiXieApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (bg, fg) = (theme.background, theme.foreground);
        if cx.global::<AppState>().user.is_none() {
            return div().size_full().bg(bg).text_color(fg).child(self.login.clone()).into_any_element();
        }
        v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .child(h_flex().flex_1().min_h_0().child(self.sidebar(cx)).child(div().flex_1().h_full().min_w_0().child(self.content(cx))))
            .child(self.status_bar(cx))
            .into_any_element()
    }
}
