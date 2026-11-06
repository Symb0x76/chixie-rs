//! M0 spike: proves the gpui-kit stack is fit for the ChiXie client before real views are built.
//!
//! Checks (see the GUI plan, M0):
//! 1. Chinese text in labels and inputs (system font fallback).
//! 2. Multi-line text input (`Textarea`) + masked password input.
//! 3. `canvas()` drawing ~5k polylines with pan (drag) / zoom (wheel) and a frame-time readout.
//! 4. Blocking worker thread -> `async-channel` -> foreground task -> entity update.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use chixie_core::font::Font;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::{ActiveTheme, Theme, ThemeMode, h_flex, v_flex};
use gpui_kit::*;

/// Hand-built test font shared with chixie-core's golden test (no real font blobs in repo).
const FIXTURE_FONT: &[u8] = include_bytes!("../../chixie-core/tests/fixtures/font_fixture.json");

/// Glyph strokes in glyph units, tiled into a grid big enough to stress the renderer.
fn stress_strokes(font: &Font, target: usize) -> Vec<Vec<(f32, f32)>> {
    let glyphs: Vec<_> = ['你', '好', 'A', '.'].iter().filter_map(|c| font.get(*c)).collect();
    let mut out = Vec::with_capacity(target);
    let (mut col, mut row) = (0usize, 0usize);
    'fill: loop {
        for g in &glyphs {
            let (ox, oy) = (col as f32 * 8.0, row as f32 * 9.0);
            for s in &g.strokes {
                out.push(s.iter().map(|&(x, y)| (ox + x, oy + y)).collect());
                if out.len() >= target {
                    break 'fill;
                }
            }
            col += 1;
            if col == 40 {
                col = 0;
                row += 1;
            }
        }
    }
    out
}

#[derive(Default)]
struct FrameStats {
    last: Option<Instant>,
    /// Exponential moving average of frame interval, ms.
    avg_ms: f32,
    /// Worst frame interval seen while animating, ms.
    max_ms: f32,
    frames: u64,
}

/// `CHIXIE_SPIKE_AUTOTEST=1`: drive the spike without input injection — start the worker,
/// animate the preview, flip to dark theme, print a report line to stdout, then quit.
fn autotest_enabled() -> bool {
    std::env::var_os("CHIXIE_SPIKE_AUTOTEST").is_some()
}

struct Spike {
    password: Entity<InputState>,
    text: Entity<TextareaState>,
    strokes: Rc<Vec<Vec<(f32, f32)>>>,
    zoom: f32,
    pan: Point<Pixels>,
    drag_from: Option<Point<Pixels>>,
    animate: bool,
    stats: Rc<RefCell<FrameStats>>,
    job: String,
    job_updates: u32,
    _tasks: Vec<Task<()>>,
}

impl Spike {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let font = Font::from_json(FIXTURE_FONT).expect("fixture font");
        let password = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("密码 password"));
        let text = cx.new(|cx| {
            TextareaState::new(window, cx).default_value("你好，世界！\n赤写机器人 ChiXie writing robot\n第三行：中英混排 123")
        });
        let mut tasks = Vec::new();
        if autotest_enabled() {
            tasks.push(cx.spawn(async move |this, cx| {
                let ms = Duration::from_millis;
                cx.background_executor().timer(ms(500)).await;
                let _ = this.update(cx, |s, cx| {
                    s.run_background_job(cx);
                    s.animate = true;
                    *s.stats.borrow_mut() = FrameStats::default();
                    cx.notify();
                });
                cx.background_executor().timer(ms(1500)).await;
                let _ = this.update(cx, |_, cx| Theme::change(ThemeMode::Dark, None, cx));
                cx.background_executor().timer(ms(2000)).await;
                let _ = this.update(cx, |s, cx| {
                    let st = s.stats.borrow();
                    println!(
                        "AUTOTEST frames={} avg_ms={:.2} max_ms={:.2} job=\"{}\" job_updates={} dark={}",
                        st.frames,
                        st.avg_ms,
                        st.max_ms,
                        s.job,
                        s.job_updates,
                        cx.theme().mode == ThemeMode::Dark
                    );
                });
                cx.background_executor().timer(ms(300)).await;
                let _ = this.update(cx, |_, cx| cx.quit());
            }));
        }
        Self {
            password,
            text,
            strokes: Rc::new(stress_strokes(&font, 5000)),
            zoom: 3.0,
            pan: point(px(10.), px(10.)),
            drag_from: None,
            animate: false,
            stats: Rc::default(),
            job: "idle".into(),
            job_updates: 0,
            _tasks: tasks,
        }
    }

    /// Blocking work on a plain thread; results cross back via a channel awaited on the UI thread.
    fn run_background_job(&mut self, cx: &mut Context<Self>) {
        let (tx, rx) = async_channel::unbounded::<String>();
        std::thread::spawn(move || {
            for i in 1..=20 {
                std::thread::sleep(Duration::from_millis(100));
                if tx.send_blocking(format!("worker step {i}/20")).is_err() {
                    return;
                }
            }
            let _ = tx.send_blocking("worker done ✓ 完成".into());
        });
        let task = cx.spawn(async move |this, cx| {
            while let Ok(msg) = rx.recv().await {
                if this.update(cx, |s, cx| {
                    s.job = msg;
                    s.job_updates += 1;
                    cx.notify();
                }).is_err() {
                    break;
                }
            }
        });
        self._tasks.push(task);
    }

    fn preview(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let strokes = self.strokes.clone();
        let (zoom, pan) = (self.zoom, self.pan);
        let stats = self.stats.clone();
        let color = cx.theme().primary;
        let paper = cx.theme().background;
        div()
            .id("preview")
            .flex_1()
            .size_full()
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .on_scroll(cx.listener(|s, e: &ScrollEvent, _, cx| {
                let dy = f32::from(e.delta.pixel_delta(px(16.)).y);
                s.zoom = (s.zoom * if dy > 0.0 { 1.1 } else { 1.0 / 1.1 }).clamp(0.2, 50.0);
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(|s, e: &MouseDownEvent, _, _| {
                s.drag_from = Some(e.position);
            }))
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                if let Some(from) = s.drag_from {
                    s.pan += e.position - from;
                    s.drag_from = Some(e.position);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|s, _: &MouseUpEvent, _, _| {
                s.drag_from = None;
            }))
            .child(
                canvas(
                    move |bounds, _, _| {
                        let mut b = PathBuilder::stroke(px(1.));
                        for s in strokes.iter() {
                            let mut it = s.iter().map(|&(x, y)| {
                                bounds.origin + pan + point(px(x * zoom), px(y * zoom))
                            });
                            if let Some(p0) = it.next() {
                                b.move_to(p0);
                                // A single-point stroke is a dot; nudge so it stays visible.
                                let mut any = false;
                                for p in it {
                                    b.line_to(p);
                                    any = true;
                                }
                                if !any {
                                    b.line_to(p0 + point(px(0.5), px(0.5)));
                                }
                            }
                        }
                        b.build().ok()
                    },
                    move |bounds, path, window, _| {
                        window.paint_quad(fill(bounds, paper));
                        if let Some(path) = path {
                            window.paint_path(path, color);
                        }
                        let mut st = stats.borrow_mut();
                        let now = Instant::now();
                        if let Some(last) = st.last {
                            let ms = (now - last).as_secs_f32() * 1000.0;
                            st.avg_ms = if st.avg_ms == 0.0 { ms } else { st.avg_ms * 0.9 + ms * 0.1 };
                            st.max_ms = st.max_ms.max(ms);
                            st.frames += 1;
                        }
                        st.last = Some(now);
                    },
                )
                .size_full(),
            )
    }
}

impl Render for Spike {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.animate {
            window.request_animation_frame();
        }
        let avg = self.stats.borrow().avg_ms;
        let fps = if avg > 0.0 { 1000.0 / avg } else { 0.0 };
        let chars = self.text.read(cx).value().chars().count();
        let dark = cx.theme().mode == ThemeMode::Dark;

        v_flex()
            .size_full()
            .p_3()
            .gap_3()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_lg().child("赤写 ChiXie — gpui 验证 spike"))
                    .child(
                        Button::new("theme")
                            .label(if dark { "浅色 Light" } else { "深色 Dark" })
                            .on_click(move |_, window, cx| {
                                Theme::change(if dark { ThemeMode::Light } else { ThemeMode::Dark }, Some(window), cx);
                            }),
                    )
                    .child(
                        Button::new("job")
                            .primary()
                            .label("后台任务 Run job")
                            .on_click(cx.listener(|s, _, _, cx| s.run_background_job(cx))),
                    )
                    .child(
                        Button::new("anim")
                            .label(if self.animate { "停止重绘 Stop" } else { "连续重绘 Animate" })
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.animate = !s.animate;
                                *s.stats.borrow_mut() = FrameStats::default();
                                cx.notify();
                            })),
                    )
                    .child(div().id("job-status").child(format!("job: {}", self.job))),
            )
            .child(
                // h_flex centers cross-axis by default; stretch so the preview column gets full height.
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .gap_3()
                    .child(
                        v_flex()
                            .w(px(360.))
                            .gap_2()
                            .child("密码输入 Password")
                            .child(Input::new(&self.password).mask_toggle())
                            .child(format!("多行文本 Textarea ({chars} 字)"))
                            .child(Textarea::new(&self.text).h(px(240.))),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .gap_1()
                            .child(format!(
                                "预览 {} polylines · zoom {:.2} · frame {:.1} ms ({:.0} fps)",
                                self.strokes.len(),
                                self.zoom,
                                avg,
                                fps
                            ))
                            .child(self.preview(cx)),
                    ),
            )
    }
}

fn main() {
    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(|cx| {
        gpui_kit::init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1280.), px(800.)), cx)),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Spike::new(window, cx)))
            .expect("open window");
        cx.activate(true);
    });
}
