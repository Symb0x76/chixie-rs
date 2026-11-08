//! 文件 Backstage (Word's File view): a full-window page with a coloured navigation column
//! (← 返回, 新建, 打开, 保存, 另存为, 导出, 模板管理, 账户, 选项) and the chosen page beside it.
//! Esc or ← returns to the document.

use chixie_core::session::Command;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, ThemeMode, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ChiXieApp;
use crate::state::AppState;
use crate::templates_view::preview;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    New,
    Open,
    Export,
    Templates,
    Account,
    Options,
}

/// Navigation entries; `None` stage = an action (保存 / 另存为) that runs at once.
const NAV: [(&str, IconName, Option<Stage>); 8] = [
    ("新建", IconName::FilePlus, Some(Stage::New)),
    ("打开", IconName::FolderOpen, Some(Stage::Open)),
    ("保存", IconName::Save, None),
    ("另存为", IconName::Copy, None),
    ("导出", IconName::FileDown, Some(Stage::Export)),
    ("模板管理", IconName::LayoutGrid, Some(Stage::Templates)),
    ("账户", IconName::User, Some(Stage::Account)),
    ("选项", IconName::Settings, Some(Stage::Options)),
];

impl ChiXieApp {
    pub(super) fn render_backstage(&mut self, stage: Stage, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (bg, fg, nav_bg, nav_fg) = (theme.background, theme.foreground, theme.primary, theme.primary_foreground);
        let nav_hover = theme.primary_hover;
        let nav_sel = nav_fg.opacity(0.18);
        let items = NAV.iter().enumerate().map(|(i, (label, icon, target))| {
            let (target, label) = (*target, *label);
            h_flex()
                .id(("bs-nav", i))
                .h(px(40.))
                .px_4()
                .gap_3()
                .cursor_pointer()
                .text_color(nav_fg)
                .when(target == Some(stage), |el| el.bg(nav_sel))
                .hover(|s| s.bg(nav_hover))
                .child(Icon::new(*icon).with_size(px(18.)))
                .child(label)
                .on_click(cx.listener(move |t, _: &ClickEvent, w, cx| match target {
                    Some(s) => t.open_backstage(s, w, cx),
                    None => t.save_doc(label == "另存为", w, cx),
                }))
        });
        let nav = v_flex()
            .w(px(200.))
            .h_full()
            .bg(nav_bg)
            .py_3()
            .child(
                h_flex()
                    .id("bs-back")
                    .h(px(48.))
                    .px_4()
                    .cursor_pointer()
                    .text_color(nav_fg)
                    .hover(|s| s.bg(nav_hover))
                    .child(Icon::new(IconName::ArrowLeft).with_size(px(22.)))
                    .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.close_backstage(w, cx))),
            )
            .children(items);
        let (title, body) = match stage {
            Stage::New => ("新建", self.bs_new(cx)),
            Stage::Open => ("打开", self.bs_open(cx)),
            Stage::Export => ("导出", self.bs_export(cx)),
            Stage::Templates => ("模板管理", self.templates.clone().into_any_element()),
            Stage::Account => ("账户", self.bs_account(cx)),
            Stage::Options => ("选项", self.bs_options(cx)),
        };
        h_flex()
            .id("backstage")
            .track_focus(&self.backstage_focus)
            .on_key_down(cx.listener(|t, e: &KeyDownEvent, w, cx| {
                if e.keystroke.key == "escape" {
                    t.close_backstage(w, cx);
                }
            }))
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(bg)
            .text_color(fg)
            .child(nav)
            .child(
                v_flex()
                    .id("bs-content")
                    .flex_1()
                    .h_full()
                    .px_10()
                    .py_8()
                    .gap_6()
                    .overflow_y_scroll()
                    .child(div().text_3xl().child(title))
                    .child(body),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn bs_new(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (border, hover, muted) = (theme.border, theme.secondary, theme.muted_foreground);
        let templates = cx.global::<AppState>().templates.templates.clone();
        let card = |id: usize, name: String, t: Option<chixie_core::template::Template>| {
            v_flex()
                .id(("bs-new", id))
                .w(px(150.))
                .gap_2()
                .p_2()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(hover))
                .child(div().w(px(134.)).h(px(176.)).border_1().border_color(border).bg(gpui_kit::white()).child(preview(t, gpui_kit::white(), border)))
                .child(div().text_sm().text_center().child(name))
        };
        let mut row = h_flex().gap_4().flex_wrap().child(card(0, "空白文档".into(), None).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.new_doc(Some(None), w, cx))));
        for (i, t) in templates.into_iter().enumerate() {
            let name = t.name.clone();
            row = row.child(card(i + 1, name.clone(), Some(t)).on_click(cx.listener(move |a, _: &ClickEvent, w, cx| a.new_doc(Some(Some(name.clone())), w, cx))));
        }
        v_flex()
            .gap_3()
            .child(div().text_sm().text_color(muted).child("新建空白文档，或用模板（信纸 / 作文格）开始。当前文档会被清空，草稿已自动保存。"))
            .child(row)
            .into_any_element()
    }

    #[inline(never)]
    fn bs_open(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        v_flex()
            .gap_3()
            .child(div().text_sm().text_color(muted).child("打开赤写文档（.chx，保留每个字的笔迹）或纯文本（.txt）。"))
            .child(
                Button::new("bs-browse")
                    .large()
                    .icon(Icon::new(IconName::FolderOpen))
                    .label("浏览…")
                    .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.open_doc(w, cx))),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn bs_export(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let doc = self.doc.read(cx).document();
        let extent = doc.bounds().map(|(x0, y0, x1, y1)| format!("书写范围 {:.1} × {:.1} mm", x1 - x0, y1 - y0)).unwrap_or_else(|| "无笔画".into());
        v_flex()
            .gap_3()
            .child(div().text_sm().text_color(muted).child("把文档导出为 G-code，每页一个文件（可用于 SD 卡或其他上位机）。"))
            .child(div().text_sm().child(format!("共 {} 页 · {} 笔 · {extent}", doc.pages.len(), doc.stroke_count())))
            .child(
                Button::new("bs-export")
                    .large()
                    .primary()
                    .icon(Icon::new(IconName::FileDown))
                    .label("导出 G-code…")
                    .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.export(w, cx))),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn bs_account(&self, cx: &mut Context<Self>) -> AnyElement {
        let st = cx.global::<AppState>();
        let font = st.active_font.as_ref().map(|f| f.name.clone()).unwrap_or_else(|| "未选择".into());
        let Some(user) = st.user.as_ref().map(|u| format!("{}（{}）", u.name, u.role)) else {
            // Logged out: the login form. Only needed to download fonts.
            return v_flex()
                .gap_3()
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(format!(
                    "当前字体：{font}。无需登录也能使用全部功能；登录只用于下载云端字体，下载后的字体保存在本机。"
                )))
                .child(div().h(px(420.)).w(px(480.)).child(self.login.clone()))
                .into_any_element();
        };
        v_flex()
            .gap_3()
            .child(h_flex().gap_3().child(Icon::new(IconName::User).with_size(px(40.))).child(div().text_lg().child(user)))
            .child(div().text_sm().child(format!("当前字体：{font}")))
            .child(
                Button::new("bs-logout")
                    .danger()
                    .icon(Icon::new(IconName::LogOut))
                    .label("退出登录")
                    .on_click(|_: &ClickEvent, _w, cx| cx.global::<AppState>().session.send(Command::Logout)),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn bs_options(&self, cx: &mut Context<Self>) -> AnyElement {
        let dark = cx.theme().mode == ThemeMode::Dark;
        let st = cx.global::<AppState>();
        let show_guides = st.settings.ui.show_guides;
        let muted = cx.theme().muted_foreground;
        let path = st.settings_path.display().to_string();
        v_flex()
            .gap_4()
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("常规"))
            .child(Checkbox::new("opt-dark").label("深色模式").checked(dark).on_click(|v: &bool, window, cx| {
                let v = *v;
                crate::ribbon::set_theme(v, Some(window), cx);
                let st = cx.global_mut::<AppState>();
                st.settings.ui.dark = v;
                st.save_settings();
            }))
            .child(Checkbox::new("opt-guides").label("在页面上显示模板格线").checked(show_guides).on_click(cx.listener(|t, v: &bool, _w, cx| {
                let v = *v;
                t.edit_settings(cx, |st| st.settings.ui.show_guides = v);
            })))
            .child(div().pt_2().text_base().font_weight(FontWeight::SEMIBOLD).child("从炽写迁移"))
            .child(
                h_flex()
                    .gap_2()
                    .child(Button::new("opt-style").icon(Icon::new(IconName::Import)).label("导入手写效果（config.ini）…").on_click(cx.listener(
                        |t, _: &ClickEvent, w, cx| t.import_style(w, cx),
                    )))
                    .child(Button::new("opt-tpl").icon(Icon::new(IconName::LayoutGrid)).label("导入模板（.cx）…").on_click(cx.listener(
                        |t, _: &ClickEvent, w, cx| t.open_backstage(super::Stage::Templates, w, cx),
                    ))),
            )
            .child(div().text_xs().text_color(muted).child(format!("设置文件：{path}")))
            .into_any_element()
    }
}
