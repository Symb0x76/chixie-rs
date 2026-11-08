//! Ribbon tab bodies. One `#[inline(never)]` fn per tab: element builders are large values, and
//! one function holding every tab's temporaries overflowed the 1 MB debug main-thread stack.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::NumberInput;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::select::Select;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, ThemeMode, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{SastWriterApp, Stage, fmt_num};
use crate::doc_view::{self, DocView};
use crate::ribbon::{self, big, big_menu, group, small, stack};
use crate::state::AppState;
use crate::templates_view::{kind_icon, preview};

const PAPERS: [(&str, (f32, f32)); 4] = [("A4", (210.0, 297.0)), ("A5", (148.0, 210.0)), ("B5", (176.0, 250.0)), ("16K", (184.0, 260.0))];
/// Word's margin presets (mm) as (left, top, right, bottom).
type Margins = (f32, f32, f32, f32);
const MARGINS: [(&str, Margins); 4] = [
    ("普通", (21.0, 20.0, 21.0, 20.0)),
    ("窄", (12.7, 12.7, 12.7, 12.7)),
    ("适中", (19.1, 25.4, 19.1, 25.4)),
    ("宽", (31.8, 25.4, 31.8, 25.4)),
];

/// Click handler that runs an editor action with focus handed back to the page (so ribbon
/// buttons behave exactly like their shortcuts).
pub fn editor_action(doc: Entity<DocView>, a: Box<dyn Action>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let focus = doc.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.dispatch_action(a.boxed_clone(), cx);
    }
}

impl SastWriterApp {
    pub(super) fn ribbon_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let body = match self.tab {
            0 => self.tab_home(cx),
            1 => self.tab_layout(cx),
            2 => self.tab_style(cx),
            3 => self.tab_device(cx),
            _ => self.tab_view(cx),
        };
        h_flex().h_full().px_1().child(body).into_any_element()
    }

    #[inline(never)]
    fn tab_home(&self, cx: &mut Context<Self>) -> AnyElement {
        let act = |a: Box<dyn Action>| editor_action(self.doc.clone(), a);
        let has_fonts = !self.font_ids.is_empty();
        let font_size = cx.global::<AppState>().settings.page.font_size;
        h_flex()
            .h_full()
            .child(group(
                cx,
                "clip",
                "剪贴板",
                h_flex().gap_1().child(big("paste", "粘贴", IconName::ClipboardPaste).on_click(act(Box::new(doc_view::Paste)))).child(stack([
                    small("cut", "剪切", IconName::Scissors).on_click(act(Box::new(doc_view::Cut))).into_any_element(),
                    small("copy", "复制", IconName::Copy).on_click(act(Box::new(doc_view::Copy))).into_any_element(),
                ])),
                None,
            ))
            .child(group(
                cx,
                "font",
                "字体",
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                div().w(px(190.)).child(
                                    Select::new(&self.font_select)
                                        .small()
                                        .placeholder(if has_fonts { "选择字体" } else { "（无收藏字体）" })
                                        .menu_width(px(260.)),
                                ),
                            )
                            .child(small("fontlib", "字体库…", IconName::Type).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.open_fonts(w, cx)))),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(div().text_xs().child("字号"))
                            .child(div().w(px(96.)).child(NumberInput::new(&self.font_size).small()))
                            .child(ribbon::icon_btn("grow", IconName::AArrowUp, "增大字号").on_click(cx.listener(move |t, _: &ClickEvent, w, cx| {
                                t.set_font_size(font_size + 10.0, w, cx)
                            })))
                            .child(ribbon::icon_btn("shrink", IconName::AArrowDown, "减小字号").on_click(cx.listener(move |t, _: &ClickEvent, w, cx| {
                                t.set_font_size(font_size - 10.0, w, cx)
                            }))),
                    ),
                None,
            ))
            .child(group(
                cx,
                "para",
                "段落",
                v_flex()
                    .gap_1()
                    .child(h_flex().gap_1().child(div().text_xs().child("行距")).child(div().w(px(104.)).child(NumberInput::new(&self.line_height).small().suffix("mm"))))
                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child("信纸 / 作文格按模板排行")),
                None,
            ))
            .child(group(
                cx,
                "edit",
                "编辑",
                stack([
                    small("selall", "全选", IconName::TextCursorInput).on_click(act(Box::new(doc_view::SelectAll))).into_any_element(),
                    small("undo", "撤销", IconName::Undo2).on_click(act(Box::new(doc_view::Undo))).into_any_element(),
                    small("redo", "重做", IconName::Redo2).on_click(act(Box::new(doc_view::Redo))).into_any_element(),
                ]),
                None,
            ))
            .into_any_element()
    }

    fn set_font_size(&mut self, v: f32, window: &mut Window, cx: &mut Context<Self>) {
        let v = v.clamp(5.0, 1000.0);
        self.font_size.update(cx, |s, cx| s.set_value(fmt_num(v), window, cx));
        self.edit_settings(cx, |st| st.settings.page.font_size = v);
    }

    #[inline(never)]
    fn tab_layout(&self, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let st = cx.global::<AppState>();
        let page = st.settings.page.clone();
        let active = page.template.clone();
        let templates = st.templates.templates.clone();
        let portrait = page.paper.0 <= page.paper.1;
        let theme = cx.theme();
        let (accent, border, hover, muted) = (theme.primary, theme.border, theme.secondary, theme.muted_foreground);

        let w1 = this.clone();
        let paper_menu = big_menu("paper", "纸张大小", IconName::File).dropdown_menu(move |m, _w, _cx| {
            PAPERS.iter().fold(m, |m, (name, (w, h))| {
                let this = w1.clone();
                let (w, h) = (*w, *h);
                m.item(PopupMenuItem::new(format!("{name}  {w:.0} × {h:.0} mm")).on_click(move |_, _w, cx| {
                    let _ = this.update(cx, |t, cx| t.edit_page(cx, |p| p.paper = if p.paper.0 <= p.paper.1 { (w, h) } else { (h, w) }));
                }))
            })
        });
        let w2 = this.clone();
        let orient_menu = big_menu("orient", "纸张方向", if portrait { IconName::RectangleVertical } else { IconName::RectangleHorizontal })
            .dropdown_menu(move |m, _w, _cx| {
                let (a, b) = (w2.clone(), w2.clone());
                m.item(PopupMenuItem::new("纵向").icon(IconName::RectangleVertical).checked(portrait).on_click(move |_, _w, cx| {
                    let _ = a.update(cx, |t, cx| t.edit_page(cx, |p| p.paper = (p.paper.0.min(p.paper.1), p.paper.0.max(p.paper.1))));
                }))
                .item(PopupMenuItem::new("横向").icon(IconName::RectangleHorizontal).checked(!portrait).on_click(move |_, _w, cx| {
                    let _ = b.update(cx, |t, cx| t.edit_page(cx, |p| p.paper = (p.paper.0.max(p.paper.1), p.paper.0.min(p.paper.1))));
                }))
            });
        let w3 = this.clone();
        let current_margins = page.margins;
        let margin_menu = big_menu("margins", "页边距", IconName::Ruler).dropdown_menu(move |m, _w, _cx| {
            let m = MARGINS.iter().fold(m, |m, (name, mg)| {
                let this = w3.clone();
                let mg = *mg;
                m.item(
                    PopupMenuItem::new(format!("{name}  上 {:.1} 下 {:.1} 左 {:.1} 右 {:.1}", mg.1, mg.3, mg.0, mg.2))
                        .checked(mg == current_margins)
                        .on_click(move |_, _w, cx| {
                            let _ = this.update(cx, |t, cx| t.edit_page(cx, |p| p.margins = mg));
                        }),
                )
            });
            let this = w3.clone();
            m.separator().item(PopupMenuItem::new("自定义页面（模板管理）…").on_click(move |_, w, cx| {
                let _ = this.update(cx, |t, cx| t.open_backstage(Stage::Templates, w, cx));
            }))
        });

        // Template gallery: 空白 + every stored template, the active one outlined.
        let card = |id: usize, name: String, icon: IconName, t: Option<sast_writer_core::template::Template>, selected: bool| {
            v_flex()
                .id(("gallery", id))
                .w(px(64.))
                .h(px(72.))
                .p_0p5()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(if selected { accent } else { gpui_kit::transparent_black() })
                .hover(|s| s.bg(hover))
                .cursor_pointer()
                .child(div().w(px(42.)).h(px(50.)).child(match t {
                    Some(t) => preview(Some(t), gpui_kit::white(), border).into_any_element(),
                    None => div().size_full().flex().items_center().justify_center().child(Icon::new(icon).with_size(px(22.))).into_any_element(),
                }))
                .child(div().w_full().text_center().text_xs().truncate().child(name))
        };
        let mut gallery = h_flex().id("gallery").gap_1().max_w(px(540.)).overflow_x_scroll().child(
            card(0, "空白页".into(), IconName::File, None, active.is_none())
                .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.apply_template(None, cx))),
        );
        for (i, t) in templates.into_iter().enumerate() {
            let name = t.name.clone();
            let selected = active.as_deref() == Some(name.as_str());
            let icon = kind_icon(&t.kind);
            gallery = gallery.child(
                card(i + 1, name.clone(), icon, Some(t), selected).on_click(cx.listener(move |a, _: &ClickEvent, _w, cx| a.apply_template(Some(name.clone()), cx))),
            );
        }

        h_flex()
            .h_full()
            .child(group(cx, "page", "页面设置", h_flex().gap_0p5().child(paper_menu).child(orient_menu).child(margin_menu), None))
            .child(group(
                cx,
                "templates",
                "模板",
                h_flex().gap_1().child(gallery).child(
                    big("manage", "管理模板", IconName::LayoutGrid).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.open_backstage(Stage::Templates, w, cx))),
                ),
                None,
            ))
            .child(div().px_2().text_xs().text_color(muted).child(format!("{:.0} × {:.0} mm", page.paper.0, page.paper.1)))
            .into_any_element()
    }

    #[inline(never)]
    fn tab_style(&self, cx: &mut Context<Self>) -> AnyElement {
        let st = &cx.global::<AppState>().settings.style;
        let (hw, df) = (st.handwriting.clone(), st.deformation.clone());
        let this = cx.entity().downgrade();
        let launcher: ribbon::ClickFn = Box::new(move |_, w, cx| {
            let _ = this.update(cx, |t, cx| t.open_style_dialog(w, cx));
        });
        h_flex()
            .h_full()
            .child(group(
                cx,
                "effects",
                "效果",
                stack([
                    Checkbox::new("jitter")
                        .small()
                        .label("随机排布（字距 / 行首 / 上下浮动）")
                        .checked(hw.enabled)
                        .on_click(cx.listener(|t, v: &bool, _w, cx| {
                            let v = *v;
                            t.edit_settings(cx, |st| st.settings.style.handwriting.enabled = v);
                        }))
                        .into_any_element(),
                    Checkbox::new("base")
                        .small()
                        .label("基础变形（胖瘦 / 高矮 / 旋转）")
                        .checked(df.enabled && df.base)
                        .on_click(cx.listener(|t, v: &bool, _w, cx| {
                            let v = *v;
                            t.edit_settings(cx, |st| {
                                let d = &mut st.settings.style.deformation;
                                d.base = v;
                                d.enabled = d.base || d.shape;
                            });
                        }))
                        .into_any_element(),
                    Checkbox::new("shape")
                        .small()
                        .label("字形变形（倾斜 / 笔画扰动）")
                        .checked(df.enabled && df.shape)
                        .on_click(cx.listener(|t, v: &bool, _w, cx| {
                            let v = *v;
                            t.edit_settings(cx, |st| {
                                let d = &mut st.settings.style.deformation;
                                d.shape = v;
                                d.enabled = d.base || d.shape;
                            });
                        }))
                        .into_any_element(),
                ]),
                Some(launcher),
            ))
            .child(group(
                cx,
                "seed",
                "笔迹",
                h_flex()
                    .gap_1()
                    .child(big("reshuffle", "换一组", IconName::Shuffle).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.reshuffle(cx))))
                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!("种子\n{}", hw.seed % 100_000))),
                None,
            ))
            .child(group(
                cx,
                "import",
                "导入",
                h_flex().child(big("import-style", "从炽写导入", IconName::Import).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.import_style(w, cx)))),
                None,
            ))
            .into_any_element()
    }

    #[inline(never)]
    fn tab_device(&self, cx: &mut Context<Self>) -> AnyElement {
        let dev = self.device.read(cx);
        let (connected, summary) = (dev.is_connected(), dev.summary());
        let job = dev.job;
        let running = job.is_some();
        let paused = job.is_some_and(|j| j.paused);
        let awaiting = job.and_then(|j| j.awaiting);
        h_flex()
            .h_full()
            .child(group(
                cx,
                "conn",
                "连接",
                h_flex()
                    .gap_1()
                    .child(if connected {
                        big("disconnect", "断开", IconName::Unplug)
                            .disabled(running)
                            .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.device.update(cx, |d, cx| d.disconnect(cx))))
                    } else {
                        big("connect", "连接设备", IconName::Plug).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                            t.show_pane = true;
                            t.device.update(cx, |d, cx| d.connect(cx));
                        }))
                    })
                    .child(div().w(px(150.)).text_xs().text_color(cx.theme().muted_foreground).child(summary)),
                None,
            ))
            .child(group(
                cx,
                "write",
                "书写",
                h_flex()
                    .gap_0p5()
                    .child(big("start", "开始书写", IconName::PencilLine).disabled(!connected || running).on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.start_job(w, cx))))
                    .child(
                        big(if paused { "resume" } else { "pause" }, if paused { "继续" } else { "暂停" }, if paused { IconName::Play } else { IconName::Pause })
                            .disabled(!running || awaiting.is_some())
                            .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.device.update(cx, |d, cx| d.toggle_pause(cx)))),
                    )
                    .child(big("stop", "停止", IconName::Square).disabled(!running).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.device.update(cx, |d, cx| d.stop(cx)))))
                    .when(awaiting.is_some(), |el| {
                        el.child(Button::new("next").small().primary().label("已换纸，继续").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                            t.device.update(cx, |d, cx| d.next_sheet(cx))
                        })))
                    }),
                None,
            ))
            .child(group(
                cx,
                "pane",
                "窗格",
                h_flex().child(big("pane", if self.show_pane { "隐藏面板" } else { "设备面板" }, IconName::PanelRight).on_click(cx.listener(
                    |t, _: &ClickEvent, _w, cx| {
                        t.show_pane = !t.show_pane;
                        cx.notify();
                    },
                ))),
                None,
            ))
            .into_any_element()
    }

    #[inline(never)]
    fn tab_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let dark = cx.theme().mode == ThemeMode::Dark;
        let show_guides = cx.global::<AppState>().settings.ui.show_guides;
        h_flex()
            .h_full()
            .child(group(
                cx,
                "zoom",
                "缩放",
                h_flex()
                    .gap_1()
                    .child(big("z100", "100%", IconName::ZoomIn).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| {
                        t.doc.update(cx, |d, cx| d.set_zoom(doc_view::ZOOM_100, cx))
                    })))
                    .child(stack([
                        small("zfit", "页宽", IconName::StretchHorizontal)
                            .on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.doc.update(cx, |d, cx| d.fit_width(cx))))
                            .into_any_element(),
                        small("zin", "放大", IconName::ZoomIn).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.zoom_by(1.1, cx))).into_any_element(),
                        small("zout", "缩小", IconName::ZoomOut).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.zoom_by(1.0 / 1.1, cx))).into_any_element(),
                    ])),
                None,
            ))
            .child(group(
                cx,
                "show",
                "显示",
                stack([Checkbox::new("guides")
                    .small()
                    .label("模板格线")
                    .checked(show_guides)
                    .on_click(cx.listener(|t, v: &bool, _w, cx| {
                        let v = *v;
                        t.edit_settings(cx, |st| st.settings.ui.show_guides = v);
                    }))
                    .into_any_element()]),
                None,
            ))
            .child(group(
                cx,
                "look",
                "外观",
                stack([Checkbox::new("dark")
                    .small()
                    .label("深色模式")
                    .checked(dark)
                    .on_click(|v: &bool, window, cx| {
                        let v = *v;
                        ribbon::set_theme(v, Some(window), cx);
                        let st = cx.global_mut::<AppState>();
                        st.settings.ui.dark = v;
                        st.save_settings();
                    })
                    .into_any_element()]),
                None,
            ))
            .into_any_element()
    }
}
