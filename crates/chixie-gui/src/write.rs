//! 书写: text editor + layout parameters + live paper preview + G-code export.
//!
//! Any change (text, parameters, active font) bumps a generation counter and schedules a layout
//! 150 ms later on the background executor; stale results are dropped. The preview and the export
//! use the same `Document`, so what is shown is what gets written.

use std::path::{Path, PathBuf};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use chixie_core::gcode::{self, DeviceProfile};
use chixie_core::layout::{self, Document};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::preview::{self, Colors};
use crate::state::AppState;

/// Plotter travel on the verified board ($130/$131 = 300 mm).
const TRAVEL_MM: f32 = 300.0;
const DEBOUNCE: Duration = Duration::from_millis(150);
const A4: (f32, f32) = (210.0, 297.0);
const A5: (f32, f32) = (148.0, 210.0);

pub struct WriteView {
    text: Entity<TextareaState>,
    font_size: Entity<InputState>,
    line_height: Entity<InputState>,
    doc: Option<Arc<Document>>,
    page: usize,
    zoom: f32,
    pan: Point<Pixels>,
    drag_from: Option<Point<Pixels>>,
    /// Canvas size from the last paint, for fit-to-view; `fitted` = view already fitted.
    canvas_size: Rc<Cell<Size<Pixels>>>,
    fitted: bool,
    generation: u64,
    laying_out: bool,
    last_font: Option<i64>,
    _layout_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl WriteView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let page = cx.global::<AppState>().settings.page.clone();
        let text = cx.new(|cx| TextareaState::new(window, cx).default_value("你好，赤写！\n春眠不觉晓，处处闻啼鸟。"));
        let font_size = cx.new(|cx| InputState::new(window, cx).default_value(fmt_num(page.font_size)));
        let line_height = cx.new(|cx| InputState::new(window, cx).default_value(fmt_num(page.line_height)));

        let subs = vec![
            cx.subscribe_in(&text, window, |this, _s, ev: &InputEvent, _w, cx| {
                if let InputEvent::Change = ev {
                    this.schedule_layout(cx);
                }
            }),
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
            // A different active font means a new layout.
            cx.observe_global::<AppState>(|this, cx| {
                let id = cx.global::<AppState>().active_font.as_ref().map(|f| f.id);
                if id != this.last_font {
                    this.last_font = id;
                    this.schedule_layout(cx);
                }
                cx.notify();
            }),
        ];
        let mut this = Self {
            text,
            font_size,
            line_height,
            doc: None,
            page: 0,
            zoom: 2.2,
            pan: point(px(16.), px(16.)),
            drag_from: None,
            canvas_size: Rc::new(Cell::new(Size::default())),
            fitted: false,
            generation: 0,
            laying_out: false,
            last_font: None,
            _layout_task: None,
            _subs: subs,
        };
        this.last_font = cx.global::<AppState>().active_font.as_ref().map(|f| f.id);
        this.schedule_layout(cx);
        this
    }

    /// Apply a settings change, persist it and re-layout. Paper changes refit the view.
    fn edit_settings(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut AppState)) {
        let st = cx.global_mut::<AppState>();
        let paper = st.settings.page.paper;
        f(st);
        st.save_settings();
        if st.settings.page.paper != paper {
            self.fitted = false;
        }
        self.schedule_layout(cx);
    }

    /// Fit the whole sheet into the preview (needs one paint to know the canvas size).
    fn fit_view(&mut self, cx: &mut Context<Self>) {
        let size = self.canvas_size.get();
        if size.width < px(10.) {
            return; // not painted yet; the next layout result retries
        }
        let v = preview::fit(cx.global::<AppState>().settings.page.paper, size);
        (self.zoom, self.pan) = (v.zoom, v.pan);
        self.fitted = true;
        cx.notify();
    }

    pub fn schedule_layout(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        let st = cx.global::<AppState>();
        let Some(font) = st.active_font.as_ref().map(|f| f.font.clone()) else {
            self.doc = None;
            self.laying_out = false;
            cx.notify();
            return;
        };
        let params = st.settings.page_params();
        let text = self.text.read(cx).value().to_string();
        self.laying_out = true;
        self._layout_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            if this.read_with(cx, |v, _| v.generation != generation).unwrap_or(true) {
                return; // superseded by a newer edit
            }
            let doc = cx.background_executor().spawn(async move { layout::layout(&text, &font, &params) }).await;
            let _ = this.update(cx, |v, cx| {
                if v.generation == generation {
                    v.page = v.page.min(doc.pages.len().saturating_sub(1));
                    v.doc = Some(Arc::new(doc));
                    if !v.fitted {
                        v.fit_view(cx);
                    }
                    v.laying_out = false;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    /// Set the editor text (autotest / future "open file").
    pub fn set_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.text.update(cx, |s, cx| s.set_value(text.to_string(), window, cx));
        self.schedule_layout(cx); // set_value does not emit Change
    }

    pub fn document(&self) -> Option<Arc<Document>> {
        self.doc.clone()
    }

    fn reshuffle(&mut self, cx: &mut Context<Self>) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        self.edit_settings(cx, |st| st.settings.style.handwriting.seed = seed);
    }

    fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = self.doc.clone() else { return };
        let profile = cx.global::<AppState>().settings.device.profile.clone();
        let dir = dirs_documents();
        let rx = cx.prompt_for_new_path(&dir, Some("chixie.gcode"));
        cx.spawn_in(window, async move |_this, cx| {
            let Ok(Ok(Some(path))) = rx.await else { return };
            let msg = match export_files(&doc, &profile, &path) {
                Ok(files) => Notification::success(format!("已导出 {} 个文件：{}", files.len(), files[0].display())),
                Err(e) => Notification::error(format!("导出失败：{e}")),
            };
            let _ = cx.update(|window, cx| window.push_notification(msg, cx));
        })
        .detach();
    }

    fn params_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let st = cx.global::<AppState>();
        let page = st.settings.page.clone();
        let hw = st.settings.style.handwriting.clone();
        let df = st.settings.style.deformation.clone();
        let portrait = page.paper.0 <= page.paper.1;
        let is_a5 = (page.paper.0.min(page.paper.1) - A5.0).abs() < 0.5;
        let theme = cx.theme();
        let label = |s: &str| div().w(px(64.)).text_sm().text_color(theme.muted_foreground).child(s.to_string());

        v_flex()
            .w(px(260.))
            .gap_3()
            .child(h_flex().gap_2().child(label("字号")).child(div().w(px(90.)).child(Input::new(&self.font_size).small())))
            .child(h_flex().gap_2().child(label("行距 mm")).child(div().w(px(90.)).child(Input::new(&self.line_height).small())))
            .child(
                h_flex()
                    .gap_2()
                    .child(label("纸张"))
                    .child(
                        TabBar::new("paper")
                            .small()
                            .segmented()
                            .selected_index(if is_a5 { 1 } else { 0 })
                            .children([Tab::new().label("A4"), Tab::new().label("A5")])
                            .on_click(cx.listener(move |this, ix: &usize, _w, cx| {
                                let base = if *ix == 1 { A5 } else { A4 };
                                let paper = if portrait { base } else { (base.1, base.0) };
                                this.edit_settings(cx, |st| st.settings.page.paper = paper);
                            })),
                    )
                    .child(
                        TabBar::new("orient")
                            .small()
                            .segmented()
                            .selected_index(if portrait { 0 } else { 1 })
                            .children([Tab::new().label("竖"), Tab::new().label("横")])
                            .on_click(cx.listener(move |this, ix: &usize, _w, cx| {
                                if (*ix == 0) != portrait {
                                    this.edit_settings(cx, |st| {
                                        let (w, h) = st.settings.page.paper;
                                        st.settings.page.paper = (h, w);
                                    });
                                }
                            })),
                    ),
            )
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("手写效果"))
            .child(
                Checkbox::new("jitter")
                    .label("字距 / 行首 / 上下浮动")
                    .checked(hw.enabled)
                    .on_click(cx.listener(|this, v: &bool, _w, cx| {
                        let v = *v;
                        this.edit_settings(cx, |st| st.settings.style.handwriting.enabled = v);
                    })),
            )
            .child(
                Checkbox::new("base")
                    .label("基础变形（胖瘦 / 高矮 / 旋转）")
                    .checked(df.enabled && df.base)
                    .on_click(cx.listener(|this, v: &bool, _w, cx| {
                        let v = *v;
                        this.edit_settings(cx, |st| {
                            let d = &mut st.settings.style.deformation;
                            d.base = v;
                            d.enabled = d.base || d.shape;
                        });
                    })),
            )
            .child(
                Checkbox::new("shape")
                    .label("字形变形（倾斜 / 扰动）")
                    .checked(df.enabled && df.shape)
                    .on_click(cx.listener(|this, v: &bool, _w, cx| {
                        let v = *v;
                        this.edit_settings(cx, |st| {
                            let d = &mut st.settings.style.deformation;
                            d.shape = v;
                            d.enabled = d.base || d.shape;
                        });
                    })),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(div().text_xs().text_color(theme.muted_foreground).child(format!("随机种子 {}", hw.seed % 100_000)))
                    .child(Button::new("reshuffle").small().label("换一组").on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| this.reshuffle(cx)))),
            )
    }

    fn summary(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, warn) = (theme.muted_foreground, theme.danger);
        let st = cx.global::<AppState>();
        let Some(doc) = &self.doc else {
            let msg = if st.active_font.is_none() { "请先在「字体」中选择字体" } else { "排版中…" };
            return div().text_sm().text_color(muted).child(msg).into_any_element();
        };
        let mut lines: Vec<AnyElement> = Vec::new();
        lines.push(
            div()
                .text_sm()
                .text_color(muted)
                .child(format!("{} 页 · {} 笔{}", doc.pages.len(), doc.stroke_count(), if self.laying_out { " · 排版中…" } else { "" }))
                .into_any_element(),
        );
        if let Some((x0, y0, x1, y1)) = doc.bounds() {
            let (w, h) = (x1 - x0, y1 - y0);
            let over = w > TRAVEL_MM || h > TRAVEL_MM;
            lines.push(
                div()
                    .text_sm()
                    .text_color(if over { warn } else { muted })
                    .child(format!("范围 {w:.1} × {h:.1} mm{}", if over { "（超出 300 mm 行程！）" } else { "" }))
                    .into_any_element(),
            );
        }
        if !doc.missing.is_empty() {
            let chars: String = doc.missing.iter().take(20).collect();
            lines.push(
                div()
                    .text_sm()
                    .text_color(warn)
                    .child(format!("字体缺字 {} 个：{chars}", doc.missing.len()))
                    .into_any_element(),
            );
        }
        v_flex().gap_1().children(lines).into_any_element()
    }
}

impl Render for WriteView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = cx.global::<AppState>();
        let (paper, margins) = (st.settings.page.paper, st.settings.page.margins);
        let theme = cx.theme();
        let colors = Colors { desk: theme.muted, paper: gpui_kit::white(), margin: theme.border, ink: gpui_kit::black() };
        let pages = self.doc.as_ref().map(|d| d.pages.len()).unwrap_or(0);
        let has_doc = self.doc.as_ref().is_some_and(|d| d.stroke_count() > 0);

        let left = v_flex()
            .w(px(340.))
            .h_full()
            .gap_3()
            .p_4()
            .border_r_1()
            .border_color(theme.border)
            .child(Textarea::new(&self.text).h(px(260.)))
            .child(self.params_panel(cx))
            .child(self.summary(cx))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("export")
                            .primary()
                            .label("导出 G-code")
                            .disabled(!has_doc)
                            .on_click(cx.listener(|this, _e: &ClickEvent, window, cx| this.export(window, cx))),
                    )
                    .child(Button::new("send").label("发送到设备").disabled(true).tooltip("设备页（M5）完成后可用")),
            );

        let preview = v_flex()
            .flex_1()
            .h_full()
            .min_w_0()
            .child(
                h_flex()
                    .gap_2()
                    .p_2()
                    .when(pages > 1, |d| {
                        d.child(
                            TabBar::new("pages")
                                .small()
                                .selected_index(self.page)
                                .children((1..=pages).map(|i| Tab::new().label(format!("第 {i} 页"))))
                                .on_click(cx.listener(|this, ix: &usize, _w, cx| {
                                    this.page = *ix;
                                    cx.notify();
                                })),
                        )
                    })
                    .child(div().flex_1())
                    .child(Button::new("fit").small().label("适应页面").on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.fit_view(cx);
                    }))),
            )
            .child(
                div()
                    .id("preview")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .on_scroll(cx.listener(|this, e: &ScrollEvent, _w, cx| {
                        let dy = f32::from(e.delta.pixel_delta(px(16.)).y);
                        this.zoom = (this.zoom * if dy > 0.0 { 1.1 } else { 1.0 / 1.1 }).clamp(0.3, 30.0);
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _w, _cx| this.drag_from = Some(e.position)))
                    .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _w, cx| {
                        if let Some(from) = this.drag_from {
                            this.pan += e.position - from;
                            this.drag_from = Some(e.position);
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _e: &MouseUpEvent, _w, _cx| this.drag_from = None))
                    .child(preview::canvas_element(
                        self.doc.clone(),
                        self.page,
                        paper,
                        margins,
                        preview::View { zoom: self.zoom, pan: self.pan },
                        colors,
                        self.canvas_size.clone(),
                    )),
            );

        h_flex().size_full().child(left).child(preview)
    }
}

fn fmt_num(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn parse_in(s: &Entity<InputState>, cx: &App, min: f32, max: f32) -> Option<f32> {
    s.read(cx).value().trim().parse::<f32>().ok().filter(|v| (min..=max).contains(v))
}

fn dirs_documents() -> PathBuf {
    std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Documents")).unwrap_or_else(|| PathBuf::from("."))
}

/// Write one G-code file per page: `name.gcode` for a single page, `name_p1.gcode`, … otherwise
/// (each page is a separate sheet). Returns the written paths.
pub fn export_files(doc: &Document, profile: &DeviceProfile, path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("chixie").to_string();
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut out = Vec::new();
    for (i, page) in doc.pages.iter().enumerate() {
        let p = if doc.pages.len() == 1 { path.to_path_buf() } else { dir.join(format!("{stem}_p{}.gcode", i + 1)) };
        std::fs::write(&p, gcode::emit(page, profile))?;
        out.push(p);
    }
    Ok(out)
}
