//! 设备: controller state mirrored from the device actor, plus the right-hand task pane
//! (port choice, jog pad, pen/unlock/zero, live position, console, job progress).
//! Job start / sheet change / stop prompts live in `app.rs` because they need the document.

use std::collections::VecDeque;

use chixie_rs_device::device::{Command, DeviceHandle, Dir, Event, Job, JobOutcome, Pen, Target};
use chixie_rs_device::grbl::GrblStatus;
use chixie_rs_device::transport::{self, PortInfo};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::device_params::DeviceParams;
use crate::state::AppState;

const LOG_KEEP: usize = 300;
const LOG_SHOW: usize = 14;
const STEPS: [f32; 3] = [0.1, 1.0, 10.0];
const JOG_FEED: f32 = 1500.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JobState {
    pub page: usize,
    pub pages: usize,
    pub done: usize,
    pub total: usize,
    pub paused: bool,
    /// Waiting for the user to put sheet `n` (0-based) in place.
    pub awaiting: Option<usize>,
}

impl JobState {
    pub fn percent(&self) -> f32 {
        if self.total == 0 { 0.0 } else { self.done as f32 * 100.0 / self.total as f32 }
    }
}

/// What the app should tell the user about a device event (toasts / dialogs).
pub enum Notice {
    None,
    Info(String),
    Warn(String),
    Error(String),
    AwaitingSheet(usize),
}

pub struct DevicePanel {
    handle: DeviceHandle,
    pub connected: Option<String>,
    connecting: bool,
    pub status: Option<GrblStatus>,
    log: VecDeque<(Dir, String)>,
    ports: Vec<PortInfo>,
    /// `None` = auto-detect the ChiXie board (survives the USB-drop COM renumbering).
    port: Option<String>,
    step: usize,
    pub job: Option<JobState>,
    /// Outcome of the last finished job (status bar / autotest).
    pub last_end: Option<JobOutcome>,
    console: Entity<InputState>,
    /// 0 = 连接与控制, 1 = 参数设置.
    pub tab: usize,
    pub params: Entity<DeviceParams>,
    _subs: Vec<Subscription>,
}

impl DevicePanel {
    pub fn new(handle: DeviceHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let st = cx.global::<AppState>();
        handle.send(Command::SetProfile(st.settings.device.profile.clone()));
        let port = st.settings.device.last_port.clone();
        let console = cx.new(|cx| InputState::new(window, cx).placeholder("发送 G-code / $ 命令，回车发送"));
        let sub = cx.subscribe_in(&console, window, |this, s, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let line = s.read(cx).value().trim().to_string();
                if !line.is_empty() {
                    this.handle.send(Command::Send(line));
                    s.update(cx, |s, cx| s.set_value("", window, cx));
                }
            }
        });
        let params = cx.new(|cx| DeviceParams::new(handle.clone(), window, cx));
        let obs = cx.observe(&params, |_, _, cx| cx.notify());
        let mut this = Self {
            handle,
            connected: None,
            connecting: false,
            status: None,
            log: VecDeque::new(),
            ports: Vec::new(),
            port,
            step: 1,
            job: None,
            last_end: None,
            console,
            tab: 0,
            params,
            _subs: vec![sub, obs],
        };
        this.refresh_ports(cx);
        this
    }

    pub fn is_connected(&self) -> bool {
        self.connected.is_some()
    }
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn refresh_ports(&mut self, cx: &mut Context<Self>) {
        let task = cx.background_executor().spawn(async { transport::list_ports().unwrap_or_default() });
        cx.spawn(async move |this, cx| {
            let ports = task.await;
            let _ = this.update(cx, |p, cx| {
                p.ports = ports;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn connect(&mut self, cx: &mut Context<Self>) {
        let baud = cx.global::<AppState>().settings.device.baud;
        self.connecting = true;
        self.handle.send(Command::Connect(Target::Serial { port: self.port.clone(), baud }));
        cx.notify();
    }
    pub fn disconnect(&mut self, cx: &mut Context<Self>) {
        self.handle.send(Command::Disconnect);
        cx.notify();
    }
    pub fn send(&self, c: Command) {
        self.handle.send(c);
    }
    pub fn run(&mut self, job: Job, cx: &mut Context<Self>) {
        let pages = job.pages.len();
        self.job = Some(JobState { page: 0, pages, done: 0, total: 0, paused: false, awaiting: None });
        self.handle.send(Command::Run(job));
        cx.notify();
    }
    pub fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        if let Some(j) = &mut self.job {
            if j.paused {
                self.handle.resume();
            } else {
                self.handle.pause();
            }
            j.paused = !j.paused;
            cx.notify();
        }
    }
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.handle.stop();
        cx.notify();
    }
    pub fn next_sheet(&mut self, cx: &mut Context<Self>) {
        if let Some(j) = &mut self.job {
            j.awaiting = None;
        }
        self.handle.send(Command::NextSheet);
        cx.notify();
    }

    fn push_log(&mut self, dir: Dir, line: String) {
        if self.log.len() == LOG_KEEP {
            self.log.pop_front();
        }
        self.log.push_back((dir, line));
    }

    pub fn on_event(&mut self, ev: Event, cx: &mut Context<Self>) -> Notice {
        let notice = match ev {
            Event::Connected { port, info } => {
                self.connecting = false;
                let ver = info.iter().find(|l| l.starts_with("[VER")).cloned().unwrap_or_default();
                self.connected = Some(port.clone());
                // Read the machine's own specs ($$) for the 参数设置 tab and the warnings.
                self.handle.send(Command::QueryMachine);
                self.params.update(cx, |p, cx| {
                    p.connected = true;
                    cx.notify();
                });
                let st = cx.global_mut::<AppState>();
                if self.port.is_some() {
                    st.settings.device.last_port = self.port.clone();
                    st.save_settings();
                }
                Notice::Info(format!("已连接 {port} {ver}"))
            }
            Event::Disconnected { reason, page, last_line } => {
                self.connecting = false;
                self.connected = None;
                self.status = None;
                self.params.update(cx, |p, cx| {
                    p.connected = false;
                    cx.notify();
                });
                let was_job = self.job.take().is_some();
                match (was_job, page, last_line) {
                    (true, Some(p), Some(l)) => Notice::Warn(format!(
                        "设备断开（{reason}）：第 {} 页停在第 {l} 行。请检查 USB 后重新连接（端口号可能变化，选「自动」即可）",
                        p + 1
                    )),
                    _ if reason == "closed" => Notice::Info("已断开设备".into()),
                    _ => Notice::Warn(format!("设备断开：{reason}")),
                }
            }
            Event::Status(s) => {
                self.status = Some(s);
                Notice::None
            }
            Event::Log { dir, line } => {
                self.push_log(dir, line);
                Notice::None
            }
            Event::Progress { page, pages, done, total } => {
                if let Some(j) = &mut self.job {
                    (j.page, j.pages, j.done, j.total) = (page, pages, done, total);
                }
                Notice::None
            }
            Event::AwaitingSheet { next } => {
                if let Some(j) = &mut self.job {
                    j.awaiting = Some(next);
                }
                Notice::AwaitingSheet(next)
            }
            Event::JobEnded { outcome, page, line } => {
                self.job = None;
                self.last_end = Some(outcome.clone());
                match outcome {
                    JobOutcome::Done => Notice::Info("书写完成".into()),
                    JobOutcome::Stopped => Notice::Warn(format!("已停止（第 {} 页，第 {line} 行），笔已抬起并回到原点", page + 1)),
                    JobOutcome::Failed(e) => Notice::Error(format!("书写出错（第 {} 页，第 {line} 行）：{e}", page + 1)),
                }
            }
            Event::Machine(m) => {
                self.params.update(cx, |p, cx| {
                    p.machine = Some(m);
                    cx.notify();
                });
                Notice::None
            }
            Event::Error(e) => {
                self.connecting = false;
                Notice::Error(e)
            }
        };
        cx.notify();
        notice
    }

    /// One-line summary for the status bar.
    pub fn summary(&self) -> String {
        match (&self.connected, &self.status) {
            (None, _) if self.connecting => "设备：连接中…".into(),
            (None, _) => "设备：未连接".into(),
            (Some(p), Some(s)) => {
                let pos = s.wpos.or(s.mpos).map(|m| format!(" X{:.1} Y{:.1}", m[0], m[1])).unwrap_or_default();
                format!("设备：{p} · {}{pos}", state_zh(&s.state))
            }
            (Some(p), None) => format!("设备：{p}"),
        }
    }

    fn jog(&self, dx: f32, dy: f32) {
        let s = STEPS[self.step];
        self.handle.send(Command::Jog { dx: dx * s, dy: dy * s, feed: JOG_FEED });
    }

    #[inline(never)]
    fn connection_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let connected = self.is_connected();
        let mut choices: Vec<(Option<String>, String)> = vec![(None, "自动".into())];
        for p in &self.ports {
            let mark = if p.known_device { " ★" } else { "" };
            choices.push((Some(p.name.clone()), format!("{}{mark}", p.name)));
        }
        let selected = choices.iter().position(|(n, _)| *n == self.port).unwrap_or(0);
        let names: Vec<Option<String>> = choices.iter().map(|(n, _)| n.clone()).collect();
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("连接"))
                    .child(div().flex_1())
                    .child(Button::new("ports-refresh").xsmall().ghost().icon(IconName::Redo).tooltip("刷新端口").on_click(cx.listener(
                        |t, _: &ClickEvent, _w, cx| t.refresh_ports(cx),
                    ))),
            )
            .child(
                TabBar::new("ports")
                    .small()
                    .segmented()
                    .selected_index(selected)
                    .children(choices.iter().map(|(_, l)| Tab::new().label(l.clone())))
                    .on_click(cx.listener(move |t, ix: &usize, _w, cx| {
                        t.port = names[*ix].clone();
                        cx.notify();
                    })),
            )
            .child(div().text_xs().text_color(muted).child("★ = 识别为赤写主板；「自动」在 USB 重新插拔后也能找到"))
            .child(if connected {
                Button::new("disconnect").small().label("断开").disabled(self.busy()).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.disconnect(cx)))
            } else {
                Button::new("connect").small().primary().label("连接").loading(self.connecting).on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.connect(cx)))
            })
            .into_any_element()
    }

    #[inline(never)]
    fn control_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.is_connected() && !self.busy();
        let jog = |id: &'static str, icon: IconName, dx: f32, dy: f32| {
            Button::new(id).small().icon(icon).disabled(!enabled).on_click(cx.listener(move |t, _: &ClickEvent, _w, _cx| t.jog(dx, dy)))
        };
        let cell = || div().size(px(32.));
        let pad = v_flex()
            .gap_1()
            .child(h_flex().gap_1().child(cell()).child(jog("jy+", IconName::ArrowUp, 0.0, 1.0)).child(cell()))
            .child(
                h_flex()
                    .gap_1()
                    .child(jog("jx-", IconName::ArrowLeft, -1.0, 0.0))
                    .child(cell())
                    .child(jog("jx+", IconName::ArrowRight, 1.0, 0.0)),
            )
            .child(h_flex().gap_1().child(cell()).child(jog("jy-", IconName::ArrowDown, 0.0, -1.0)).child(cell()));
        let cmd = |id: &'static str, label: &'static str, c: fn() -> Command| {
            Button::new(id).small().label(label).disabled(!enabled).on_click(cx.listener(move |t, _: &ClickEvent, _w, _cx| t.send(c())))
        };
        v_flex()
            .gap_2()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("点动与笔（步长 mm）"))
            .child(
                h_flex()
                    .gap_3()
                    .items_start()
                    .child(pad)
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                TabBar::new("step")
                                    .small()
                                    .segmented()
                                    .selected_index(self.step)
                                    .children(STEPS.iter().map(|s| Tab::new().label(format!("{s}"))))
                                    .on_click(cx.listener(|t, ix: &usize, _w, cx| {
                                        t.step = *ix;
                                        cx.notify();
                                    })),
                            )
                            .child(h_flex().gap_1().child(cmd("pen-up", "抬笔", || Command::Pen(Pen::Up))).child(cmd("pen-down", "落笔", || Command::Pen(Pen::Down))))
                            .child(h_flex().gap_1().child(cmd("zero", "设为原点", || Command::Zero)).child(cmd("origin", "回原点", || Command::BackOrigin)))
                            .child(cmd("unlock", "解锁 $X", || Command::Unlock)),
                    ),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn job_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (muted, primary, track) = (theme.muted_foreground, theme.primary, theme.border);
        let Some(j) = self.job else {
            return div().text_sm().text_color(muted).child("在「设备」选项卡点「开始书写」").into_any_element();
        };
        let pct = j.percent();
        let label = match j.awaiting {
            Some(n) => format!("等待换纸：第 {} 页", n + 1),
            None => format!("第 {}/{} 页 · {}/{} 行 · {pct:.0}%{}", j.page + 1, j.pages, j.done, j.total, if j.paused { " · 已暂停" } else { "" }),
        };
        v_flex()
            .gap_2()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("书写任务"))
            .child(div().text_sm().child(label))
            .child(div().w_full().h(px(6.)).rounded_sm().bg(track).child(div().h_full().rounded_sm().bg(primary).w(relative(pct / 100.0))))
            .child(
                h_flex()
                    .gap_2()
                    .when(j.awaiting.is_some(), |el| {
                        el.child(Button::new("next-sheet").small().primary().label("已换纸，继续").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.next_sheet(cx))))
                    })
                    .when(j.awaiting.is_none(), |el| {
                        el.child(Button::new("pause").small().label(if j.paused { "继续" } else { "暂停" }).on_click(cx.listener(
                            |t, _: &ClickEvent, _w, cx| t.toggle_pause(cx),
                        )))
                    })
                    .child(Button::new("stop").small().danger().label("停止").on_click(cx.listener(|t, _: &ClickEvent, _w, cx| t.stop(cx)))),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn console_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (muted, fg, bg, border) = (theme.muted_foreground, theme.foreground, theme.muted, theme.border);
        let lines = self.log.iter().rev().take(LOG_SHOW).rev().map(|(d, l)| {
            let (arrow, color) = if *d == Dir::Tx { ("› ", muted) } else { ("‹ ", fg) };
            div().text_color(color).child(format!("{arrow}{l}"))
        });
        v_flex()
            .gap_2()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("控制台"))
            .child(
                v_flex()
                    .h(px(LOG_SHOW as f32 * 16.0 + 12.0))
                    .p_1p5()
                    .overflow_hidden()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(bg)
                    .font_family("Consolas")
                    .text_xs()
                    .children(lines),
            )
            .child(Input::new(&self.console).small().disabled(!self.is_connected()))
            .into_any_element()
    }
}

pub fn state_zh(s: &str) -> &'static str {
    match s.split(':').next().unwrap_or(s) {
        "Idle" => "空闲",
        "Run" => "运行",
        "Hold" => "暂停",
        "Jog" => "点动",
        "Alarm" => "报警",
        "Door" => "门开",
        "Home" => "回零",
        "Check" => "检查",
        "Sleep" => "休眠",
        _ => "未知",
    }
}

impl Render for DevicePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let border = theme.border;
        let pos = self.status.as_ref().and_then(|s| s.wpos.or(s.mpos));
        let state = self.status.as_ref().map(|s| state_zh(&s.state)).unwrap_or("—");
        let pos_text = pos.map(|m| format!("X {:.2}   Y {:.2}", m[0], m[1])).unwrap_or_else(|| "X —   Y —".into());
        let sep = || div().h(px(1.)).w_full().bg(border);
        v_flex()
            .id("device-pane")
            .size_full()
            .px_3()
            .pb_3()
            .gap_3()
            .overflow_y_scroll()
            .child(
                TabBar::new("dev-tabs")
                    .small()
                    .underline()
                    .selected_index(self.tab)
                    .children([Tab::new().label("连接与控制"), Tab::new().label("参数设置")])
                    .on_click(cx.listener(|t, ix: &usize, _w, cx| {
                        t.tab = *ix;
                        cx.notify();
                    })),
            )
            .when(self.tab == 1, |el| el.child(self.params.clone()))
            .when(self.tab == 0, |el| {
                el.child(self.connection_section(cx))
                    .child(sep())
                    .child(h_flex().gap_3().child(div().text_sm().child(format!("状态：{state}"))).child(div().text_sm().font_family("Consolas").child(pos_text)))
                    .child(self.control_section(cx))
                    .child(sep())
                    .child(self.job_section(cx))
                    .child(sep())
                    .child(self.console_section(cx))
            })
    }
}
