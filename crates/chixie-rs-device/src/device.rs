//! Device actor: one thread owns the controller link, and the UI talks to it by message.
//!
//! - Commands go in over `std::sync::mpsc` via [`DeviceHandle`].
//! - Events come out over `async-channel`, so a gpui foreground task can `.await` them (CLI
//!   code can use `recv_blocking`).
//! - Pause/Resume/Stop act on a shared [`grbl::Control`] directly. They work while a job is
//!   blocking inside `grbl::stream`.
//!
//! A link I/O error mid-job (the CH340 board drops off USB under motor load and comes back on a
//! new COM number) emits `Disconnected` with the last acknowledged line, then the actor rescans
//! (`Reconnecting`) and reports `Recovered` with the resume line and whether the board rebooted.
//! The job continues on `ResumeJob`, after the user re-zeroed if needed. Outside a job, an I/O
//! error just ends the session.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chixie_rs_core::gcode::DeviceProfile;
use crate::grbl::{self, Control, GrblStatus, MachineSettings, StreamEvent, realtime};
use crate::transport::{self, Link};
use chixie_rs_core::{Error, Result};

/// How long the idle loop waits for a command before polling the link again.
const IDLE_TICK: Duration = Duration::from_millis(50);
const STATUS_EVERY: Duration = Duration::from_millis(250);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
/// After a link drop: rescan this often, for this long.
const RECONNECT_EVERY: Duration = Duration::from_secs(1);
const RECONNECT_FOR: Duration = Duration::from_secs(60);
/// Time for a rebooting Grbl_ESP32 to print its banner after the port reopens.
const BOOT_WAIT: Duration = Duration::from_millis(1500);
/// Lines a Grbl planner may hold beyond the last ack (Grbl_ESP32 `BLOCK_BUFFER_SIZE`).
pub const PLANNER_DEPTH: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// `None` port: auto-detect the first known ChiXie USB-serial chip.
    Serial { port: Option<String>, baud: u32 },
    Telnet { ip: String, port: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pen {
    Up,
    Down,
}

/// A writing job: one G-code body per sheet. The actor pauses for a sheet change between pages.
#[derive(Debug, Clone, Default)]
pub struct Job {
    pub pages: Vec<String>,
    /// Document page number of `pages[0]` (书写此页 / 从第 N 页), so events name real pages.
    pub first_page: usize,
}

#[derive(Debug)]
pub enum Command {
    Connect(Target),
    Disconnect,
    /// Raw line (console); replies come back as `Log`.
    Send(String),
    Jog { dx: f32, dy: f32, feed: f32 },
    Pen(Pen),
    Unlock,
    /// Pen up, then rapid to work origin.
    BackOrigin,
    /// `G92 X0 Y0`: make the current pen position the work origin.
    Zero,
    SetProfile(DeviceProfile),
    /// Read the machine's `$$` settings; answered with `Event::Machine`.
    QueryMachine,
    Run(Job),
    /// Next sheet is in place; continue after `AwaitingSheet`.
    NextSheet,
    /// After `Event::Recovered`: continue the interrupted job.
    ResumeJob,
    /// Wakes the actor if it is waiting for a sheet; the stop itself is flagged on `Control`.
    Stop,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dir {
    Tx,
    Rx,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JobOutcome {
    Done,
    Stopped,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// `info` holds the `$I` reply lines (firmware version etc.).
    Connected { port: String, info: Vec<String> },
    /// Link lost or closed. `last_line` is the last acknowledged line of the running page.
    Disconnected { reason: String, page: Option<usize>, last_line: Option<usize> },
    Status(GrblStatus),
    Log { dir: Dir, line: String },
    Progress { page: usize, pages: usize, done: usize, total: usize },
    /// Page `next` (0-based) is ready; swap the sheet, then send `NextSheet`.
    AwaitingSheet { next: usize },
    JobEnded { outcome: JobOutcome, page: usize, line: usize },
    Error(String),
    /// `$$` settings (reply to `Command::QueryMachine`).
    Machine(MachineSettings),
    /// The stream stalled on lost acknowledgements and recovered by itself.
    Resynced { page: usize, lost: usize },
    /// The link dropped mid-job; reconnect attempt `attempt` (1 s apart, up to 60 s).
    Reconnecting { attempt: u32 },
    /// Back after a drop. The job resumes at `line` of `page` on `Command::ResumeJob`. If
    /// `rebooted`, coordinates are lost: the user must jog to the paper origin and zero first.
    Recovered { page: usize, line: usize, rebooted: bool },
}

/// User-facing (Chinese) text for a device error; Grbl codes come with their meaning.
pub fn describe(e: &Error) -> String {
    match e {
        Error::Grbl { msg, .. } => format!("控制板报错 {msg}"),
        Error::Alarm(m) => format!("控制板报警 {m}"),
        Error::Timeout => "设备长时间没有应答".into(),
        Error::Api(m) if m == NOT_CONNECTED => "设备未连接".into(),
        other => other.to_string(),
    }
}

const NOT_CONNECTED: &str = "not connected";

/// Where to restart a page after a drop. `acked` lines were acknowledged, but up to
/// [`PLANNER_DEPTH`] of them may still have been queued, not drawn. Back up past those, then to
/// the pen-up that precedes the stroke there (`gcode::emit` writes one before every travel), so
/// the pen comes down only at a stroke start. Redraws at most a few strokes, never skips one.
pub fn resume_point(lines: &[String], acked: usize, pen_up: &str) -> usize {
    let surely_drawn = acked.saturating_sub(PLANNER_DEPTH).min(lines.len());
    lines[..surely_drawn].iter().rposition(|l| l.starts_with(pen_up)).unwrap_or(0)
}

/// `0.0` → "0", `-20.0` → "-20", `12.5` → "12.5" (keeps the historical `X0Y0` literal).
fn fmt_mm(v: f32) -> String {
    let s = format!("{:.3}", v + 0.0); // + 0.0 turns -0.0 into 0.0
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub type BoxLink = Box<dyn Link + Send>;
pub type Opener = Box<dyn FnMut(&Target) -> Result<(BoxLink, String)> + Send>;

/// Opens real serial / telnet links.
pub fn default_opener() -> Opener {
    Box::new(|t| match t {
        Target::Serial { port, baud } => {
            let name = match port {
                Some(p) => p.clone(),
                None => transport::list_ports()?
                    .into_iter()
                    .find(|p| p.known_device)
                    .map(|p| p.name)
                    .ok_or_else(|| Error::Io(std::io::Error::other("no ChiXie serial port found")))?,
            };
            Ok((Box::new(transport::open_serial(&name, *baud)?) as BoxLink, name))
        }
        Target::Telnet { ip, port } => {
            Ok((Box::new(transport::open_telnet(ip, *port)?) as BoxLink, format!("{ip}:{port}")))
        }
    })
}

/// Cheap to clone; dropping every handle does not stop the actor (send `Shutdown`).
#[derive(Clone)]
pub struct DeviceHandle {
    tx: Sender<Command>,
    ctl: Arc<Control>,
}

impl DeviceHandle {
    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }
    pub fn pause(&self) {
        self.ctl.pause();
    }
    pub fn resume(&self) {
        self.ctl.resume();
    }
    pub fn stop(&self) {
        self.ctl.cancel.store(true, Ordering::SeqCst);
        let _ = self.tx.send(Command::Stop);
    }
}

/// Spawn the actor thread.
pub fn spawn(opener: Opener) -> (DeviceHandle, async_channel::Receiver<Event>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let (ev_tx, ev_rx) = async_channel::unbounded();
    let ctl = Arc::new(Control::default());
    let actor = Actor { opener, rx, ev: ev_tx, ctl: ctl.clone(), link: None, profile: DeviceProfile::default(), target: None, last_mpos: None, quit: false };
    let join = std::thread::Builder::new()
        .name("chixie-device".into())
        .spawn(move || actor.run())
        .expect("spawn device thread");
    (DeviceHandle { tx, ctl }, ev_rx, join)
}

struct Actor {
    opener: Opener,
    rx: Receiver<Command>,
    ev: async_channel::Sender<Event>,
    ctl: Arc<Control>,
    link: Option<BoxLink>,
    profile: DeviceProfile,
    /// Last connect target, reused to reconnect after a drop.
    target: Option<Target>,
    /// Last machine position reported during a job (reboot detection).
    last_mpos: Option<[f32; 3]>,
    /// Shutdown arrived while a job was recovering.
    quit: bool,
}

impl Actor {
    fn emit(&self, e: Event) {
        let _ = self.ev.send_blocking(e);
    }

    fn run(mut self) {
        let mut last_poll = Instant::now();
        loop {
            match self.rx.recv_timeout(IDLE_TICK) {
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(c) => {
                    if let Err(e) = self.handle(c) {
                        self.fail(e, None, None);
                    }
                    if self.quit {
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            if self.link.is_some() {
                if last_poll.elapsed() >= STATUS_EVERY {
                    last_poll = Instant::now();
                    if let Err(e) = self.link_mut().write_all(&[realtime::STATUS]) {
                        self.fail(e, None, None);
                        continue;
                    }
                }
                if let Err(e) = self.drain() {
                    self.fail(e, None, None);
                }
            }
        }
    }

    fn link_mut(&mut self) -> &mut BoxLink {
        self.link.as_mut().expect("link checked by caller")
    }

    /// I/O errors drop the link; anything else is reported and the link is kept.
    fn fail(&mut self, e: Error, page: Option<usize>, last_line: Option<usize>) {
        match e {
            Error::Io(_) | Error::Serial(_) => {
                self.link = None;
                self.emit(Event::Disconnected { reason: e.to_string(), page, last_line });
            }
            other => self.emit(Event::Error(describe(&other))),
        }
    }

    /// Read whatever the controller has sent, without blocking past the link's read timeout.
    fn drain(&mut self) -> Result<()> {
        while let Some(l) = self.link_mut().read_line()? {
            self.report_line(l);
        }
        Ok(())
    }

    fn report_line(&self, l: String) {
        match GrblStatus::parse(&l) {
            Some(st) => self.emit(Event::Status(st)),
            None => self.emit(Event::Log { dir: Dir::Rx, line: l }),
        }
    }

    fn cmd(&mut self, line: &str) -> Result<()> {
        self.emit(Event::Log { dir: Dir::Tx, line: line.to_string() });
        let replies = grbl::command(self.link_mut().as_mut(), line, COMMAND_TIMEOUT)?;
        for l in replies {
            self.report_line(l);
        }
        Ok(())
    }

    fn require_link(&self) -> Result<()> {
        if self.link.is_none() {
            return Err(Error::Api(NOT_CONNECTED.into()));
        }
        Ok(())
    }

    fn handle(&mut self, c: Command) -> Result<()> {
        match c {
            Command::Connect(t) => {
                self.link = None;
                let (link, port) = (self.opener)(&t)?;
                self.target = Some(t);
                self.link = Some(link);
                let info = grbl::command(self.link_mut().as_mut(), "$I", COMMAND_TIMEOUT)?;
                self.emit(Event::Connected { port, info });
            }
            Command::Disconnect => {
                if self.link.take().is_some() {
                    self.emit(Event::Disconnected { reason: "closed".into(), page: None, last_line: None });
                }
            }
            Command::SetProfile(p) => self.profile = p,
            Command::NextSheet | Command::Stop | Command::ResumeJob => {} // only meaningful inside a job
            Command::Shutdown => unreachable!("handled in run"),
            other => {
                self.require_link()?;
                match other {
                    Command::Send(l) => self.cmd(&l)?,
                    Command::Jog { dx, dy, feed } => self.cmd(&format!("$J=G21G91X{dx:.3}Y{dy:.3}F{feed:.0}"))?,
                    Command::Pen(p) => self.cmd(&self.pen_line(p))?,
                    Command::Unlock => self.cmd("$X")?,
                    Command::QueryMachine => {
                        self.emit(Event::Log { dir: Dir::Tx, line: "$$".into() });
                        let lines = grbl::command(self.link_mut().as_mut(), "$$", COMMAND_TIMEOUT)?;
                        self.emit(Event::Machine(MachineSettings::parse(lines.iter().map(String::as_str))));
                    }
                    Command::BackOrigin => {
                        self.cmd(&self.pen_line(Pen::Up))?;
                        self.cmd("G90G0 X0Y0")?;
                    }
                    Command::Zero => self.cmd("G92 X0 Y0")?,
                    Command::Run(job) => self.run_job(job),
                    _ => unreachable!(),
                }
            }
        }
        Ok(())
    }

    fn pen_line(&self, p: Pen) -> String {
        let code = if p == Pen::Up { self.profile.pen_up_cmd() } else { self.profile.pen_down_cmd() };
        format!("{code} F{:.0}", self.profile.travel_speed)
    }

    fn run_job(&mut self, job: Job) {
        self.ctl.reset();
        // Drop sheet-change wakeups queued before this job started.
        while let Ok(Command::Stop | Command::NextSheet | Command::ResumeJob) = self.rx.try_recv() {}
        self.last_mpos = None;
        self.run_pages(&job, 0, None);
    }

    /// Stream `job.pages[from..]` with sheet changes in between. `resume = Some(line)` starts page
    /// `from` at that line (a pen-up, see [`resume_point`]) without a sheet prompt.
    fn run_pages(&mut self, job: &Job, from: usize, resume: Option<usize>) {
        let pages = job.first_page + job.pages.len();
        for i in from..job.pages.len() {
            let page = job.first_page + i;
            if i > 0 && !(i == from && resume.is_some()) {
                self.emit(Event::AwaitingSheet { next: page });
                match self.wait_for_sheet() {
                    Ok(true) => {}
                    Ok(false) => return self.end_stopped(page, 0),
                    Err(e) => return self.recover(job, i, 0, true, e),
                }
            }
            let lines: Vec<String> = job.pages[i].lines().filter_map(grbl::clean_line).collect();
            let total = lines.len();
            // A resumed page restates absolute mode (custom pen-up commands may not carry G90).
            let (skip, prelude): (usize, &[&str]) = match resume {
                Some(s) if i == from => (s.min(total), &["G90"]),
                _ => (0, &[]),
            };
            let mut acked = skip;
            let mut mpos = None;
            let ev = self.ev.clone();
            let feed = prelude.iter().copied().chain(lines[skip..].iter().map(String::as_str));
            let link = self.link.as_mut().expect("checked by require_link").as_mut();
            let r = grbl::stream(link, feed, &self.ctl, |e| {
                let out = match e {
                    StreamEvent::Progress { done, .. } => {
                        acked = (skip + done).saturating_sub(prelude.len()).max(skip);
                        Event::Progress { page, pages, done: acked, total }
                    }
                    StreamEvent::Status(st) => {
                        mpos = st.mpos.or(mpos);
                        Event::Status(st)
                    }
                    StreamEvent::Message(line) => Event::Log { dir: Dir::Rx, line },
                    StreamEvent::Resynced { lost } => Event::Resynced { page, lost },
                };
                let _ = ev.send_blocking(out);
            });
            self.last_mpos = mpos.or(self.last_mpos);
            match r {
                Ok(()) => {}
                Err(Error::Cancelled) => return self.end_stopped(page, acked),
                Err(e @ (Error::Io(_) | Error::Serial(_))) => return self.recover(job, i, acked, false, e),
                Err(e) => {
                    let line = acked + 1;
                    return self.emit(Event::JobEnded { outcome: JobOutcome::Failed(describe(&e)), page, line });
                }
            }
        }
        // Park: pen up, then to the 写完位置, so the next sheet can go in.
        let last = pages.saturating_sub(1);
        if let Err(e) = self.park() {
            return self.fail(e, Some(last), None);
        }
        self.emit(Event::JobEnded { outcome: JobOutcome::Done, page: last, line: 0 });
    }

    /// The link died mid-job (USB brown-out: the CH340 board drops and comes back, often on a
    /// new COM number). Rescan until it is back, tell the UI where the job will resume and
    /// whether the board rebooted (coordinates lost → the user re-zeros), keep serving jog / pen /
    /// 设为原点 commands meanwhile, and continue on `ResumeJob`. `sheet_pending`: the drop came
    /// while waiting for a sheet change, which is asked again after the resume.
    fn recover(&mut self, job: &Job, i: usize, acked: usize, sheet_pending: bool, e: Error) {
        let page = job.first_page + i;
        self.link = None;
        self.emit(Event::Disconnected { reason: e.to_string(), page: Some(page), last_line: Some(acked) });
        let stopped = |a: &Self| a.emit(Event::JobEnded { outcome: JobOutcome::Stopped, page, line: acked });
        let Some(target) = self.target.clone() else { return stopped(self) };

        let deadline = Instant::now() + RECONNECT_FOR;
        let mut attempt = 0;
        let rebooted = loop {
            if self.ctl.cancel.load(Ordering::SeqCst) {
                return stopped(self);
            }
            match self.rx.recv_timeout(RECONNECT_EVERY) {
                Ok(Command::Stop | Command::Disconnect) => return stopped(self),
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                    self.quit = true;
                    return stopped(self);
                }
                Ok(Command::SetProfile(p)) => self.profile = p,
                Ok(_) => self.emit(Event::Error("设备正在重新连接，请稍候".into())),
                Err(RecvTimeoutError::Timeout) => {}
            }
            attempt += 1;
            self.emit(Event::Reconnecting { attempt });
            match self.reopen(&target) {
                Ok(rebooted) => break rebooted,
                Err(_) if Instant::now() < deadline => {}
                Err(e) => {
                    let why = format!("设备断开后未能重新连接（{}）", describe(&e));
                    return self.emit(Event::JobEnded { outcome: JobOutcome::Failed(why), page, line: acked });
                }
            }
        };

        let lines: Vec<String> = job.pages[i].lines().filter_map(grbl::clean_line).collect();
        let start = resume_point(&lines, acked, &self.profile.pen_up_cmd());
        self.emit(Event::Recovered { page, line: start, rebooted });
        self.ctl.reset();
        loop {
            if self.ctl.cancel.load(Ordering::SeqCst) {
                return self.end_stopped(page, acked);
            }
            let r = match self.rx.recv_timeout(STATUS_EVERY) {
                Ok(Command::ResumeJob) => break,
                Ok(Command::Stop) => return self.end_stopped(page, acked),
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                    self.quit = true;
                    return stopped(self);
                }
                Ok(c @ (Command::Run(_) | Command::NextSheet | Command::Connect(_) | Command::Disconnect)) => {
                    self.emit(Event::Error(format!("请先继续或停止当前任务：{c:?} 已忽略")));
                    Ok(())
                }
                Ok(c) => self.handle(c),
                Err(RecvTimeoutError::Timeout) => self.link_mut().write_all(&[realtime::STATUS]).and_then(|()| self.drain()),
            };
            match r {
                Ok(()) => {}
                Err(e @ (Error::Io(_) | Error::Serial(_))) => return self.recover(job, i, acked, sheet_pending, e),
                Err(e) => self.emit(Event::Error(describe(&e))),
            }
        }
        self.ctl.reset();
        self.run_pages(job, i, (!sheet_pending).then_some(start));
    }

    /// Reopen `target` (a serial port that vanished falls back to auto-detect), let a rebooting
    /// board finish its banner, identify it, and report whether it rebooted: banner seen, or
    /// the head reads (0, 0) although the last report before the drop was elsewhere.
    fn reopen(&mut self, target: &Target) -> Result<bool> {
        let (link, port) = match (self.opener)(target) {
            Ok(l) => l,
            Err(e) => match target {
                Target::Serial { port: Some(_), baud } => (self.opener)(&Target::Serial { port: None, baud: *baud })?,
                _ => return Err(e),
            },
        };
        self.link = Some(link);
        let mut rebooted = false;
        let quiet_until = Instant::now() + BOOT_WAIT;
        while Instant::now() < quiet_until {
            if let Some(l) = self.link_mut().read_line()? {
                rebooted |= l.starts_with("Grbl ");
                self.report_line(l);
            }
        }
        let info = grbl::command(self.link_mut().as_mut(), "$I", COMMAND_TIMEOUT)?;
        rebooted |= info.iter().any(|l| l.starts_with("Grbl "));
        self.link_mut().write_all(&[realtime::STATUS])?;
        let until = Instant::now() + Duration::from_millis(500);
        let mut now_at = None;
        while now_at.is_none() && Instant::now() < until {
            if let Some(l) = self.link_mut().read_line()? {
                now_at = GrblStatus::parse(&l).and_then(|s| s.mpos);
            }
        }
        let near0 = |p: [f32; 3]| p[0].abs() < 0.01 && p[1].abs() < 0.01;
        if let (Some(now), Some(before)) = (now_at, self.last_mpos) {
            rebooted |= near0(now) && !near0(before);
        }
        self.emit(Event::Connected { port, info });
        Ok(rebooted)
    }

    /// `Ok(true)` on `NextSheet`, `Ok(false)` on stop. Keeps status polling alive meanwhile.
    fn wait_for_sheet(&mut self) -> Result<bool> {
        loop {
            if self.ctl.cancel.load(Ordering::SeqCst) {
                return Ok(false);
            }
            match self.rx.recv_timeout(STATUS_EVERY) {
                Ok(Command::NextSheet) => return Ok(true),
                Ok(Command::Stop | Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => return Ok(false),
                Ok(other) => self.emit(Event::Error(format!("busy (waiting for sheet): {other:?} ignored"))),
                Err(RecvTimeoutError::Timeout) => {
                    self.link_mut().write_all(&[realtime::STATUS])?;
                    self.drain()?;
                }
            }
        }
    }

    /// Pen up, then rapid to the profile's 写完位置 (no move for `Stay`).
    fn park(&mut self) -> Result<()> {
        self.cmd(&self.pen_line(Pen::Up))?;
        match self.profile.end_position.target(&self.profile.axis_map) {
            Some((x, y)) => self.cmd(&format!("G90G0 X{}Y{}", fmt_mm(x), fmt_mm(y))),
            None => Ok(()),
        }
    }

    /// Stop = feed hold (already sent by `stream`) → wait for the hold to settle → soft reset
    /// (flushes the planner without losing position) → unlock → pen up → back to origin.
    /// If the hold never settled, a reset mid-motion may have lost position, so the head stays put
    /// (pen up) rather than rapid to a wrong origin.
    fn end_stopped(&mut self, page: usize, line: usize) {
        let r = (|| -> Result<()> {
            let link = self.link_mut();
            link.write_all(&[realtime::FEED_HOLD])?;
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut settled = false;
            'settle: while Instant::now() < deadline {
                link.write_all(&[realtime::STATUS])?;
                let poll_until = Instant::now() + Duration::from_millis(200);
                while Instant::now() < poll_until {
                    match link.read_line()? {
                        Some(l) => {
                            if let Some(st) = GrblStatus::parse(&l)
                                && (st.state.starts_with("Hold:0") || st.state == "Idle")
                            {
                                settled = true;
                                break 'settle;
                            }
                        }
                        None => break,
                    }
                }
            }
            link.write_all(&[realtime::SOFT_RESET])?;
            // Swallow the reset banner and any stale replies.
            let quiet_until = Instant::now() + Duration::from_millis(300);
            while Instant::now() < quiet_until {
                if link.read_line()?.is_none() {
                    break;
                }
            }
            self.ctl.reset();
            let _ = self.cmd("$X"); // best effort; only needed if the reset raised an alarm
            if settled { self.park() } else { self.cmd(&self.pen_line(Pen::Up)) }
        })();
        if let Err(e) = r {
            return self.fail(e, Some(page), Some(line));
        }
        self.emit(Event::JobEnded { outcome: JobOutcome::Stopped, page, line });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::grbl::fake::Fake;

    /// Lets the test inspect the fake after the actor took it. Each read costs `delay`, like a
    /// slow controller, so a test can act mid-job.
    struct Shared(Arc<Mutex<Fake>>, Duration);

    impl Link for Shared {
        fn write_all(&mut self, d: &[u8]) -> Result<()> {
            self.0.lock().unwrap().write_all(d)
        }
        fn read_line(&mut self) -> Result<Option<String>> {
            std::thread::sleep(self.1);
            self.0.lock().unwrap().read_line()
        }
    }

    fn start(fake: Fake) -> (DeviceHandle, async_channel::Receiver<Event>, Arc<Mutex<Fake>>) {
        start_slow(fake, Duration::from_millis(1))
    }

    fn start_slow(fake: Fake, delay: Duration) -> (DeviceHandle, async_channel::Receiver<Event>, Arc<Mutex<Fake>>) {
        let shared = Arc::new(Mutex::new(fake));
        let s2 = shared.clone();
        let opener: Opener =
            Box::new(move |_| Ok((Box::new(Shared(s2.clone(), delay)) as BoxLink, "FAKE".into())));
        let (h, ev, _join) = spawn(opener);
        h.send(Command::Connect(Target::Serial { port: None, baud: 115_200 }));
        (h, ev, shared)
    }

    /// Next event matching `f`, skipping status/log noise. Panics after 5 s.
    fn wait_for(ev: &async_channel::Receiver<Event>, f: impl Fn(&Event) -> bool) -> Event {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match ev.try_recv() {
                Ok(e) if f(&e) => return e,
                Ok(_) => {}
                Err(_) => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        panic!("timed out waiting for event");
    }

    fn page(n: usize) -> String {
        (0..n).map(|i| format!("G1 X{i}.000 Y{i}.000 F3000\n")).collect()
    }

    #[test]
    fn connects_and_jogs() {
        let (h, ev, fake) = start(Fake::default());
        assert!(matches!(wait_for(&ev, |e| matches!(e, Event::Connected { .. })), Event::Connected { port, .. } if port == "FAKE"));
        h.send(Command::Jog { dx: 1.5, dy: -2.0, feed: 1000.0 });
        h.send(Command::Pen(Pen::Down));
        wait_for(&ev, |e| matches!(e, Event::Log { dir: Dir::Tx, line } if line.starts_with("G1G90 Z2")));
        let lines = fake.lock().unwrap().lines.clone();
        assert_eq!(lines, vec!["$I", "$J=G21G91X1.500Y-2.000F1000", "G1G90 Z2 F8000"]);
        h.send(Command::Shutdown);
    }

    #[test]
    fn query_machine_parses_dollar_dollar() {
        // Grbl answers `$$` with one line per setting, then `ok`.
        let dollars = ["$110=12000.000", "$111=11000.000", "$130=300.000", "$131=280.000"].map(String::from).to_vec();
        let (h, ev, _fake) = start(Fake { respond: vec![("$$".into(), dollars)], ..Default::default() });
        wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
        h.send(Command::QueryMachine);
        match wait_for(&ev, |e| matches!(e, Event::Machine(_))) {
            Event::Machine(m) => {
                assert_eq!(m.max_feed(), Some(11000.0));
                assert_eq!(m.travel, Some((300.0, 280.0)));
            }
            _ => unreachable!(),
        }
        h.send(Command::Shutdown);
    }

    #[test]
    fn two_page_job_waits_for_sheet() {
        let (h, ev, fake) = start(Fake::default());
        wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
        h.send(Command::Run(Job { pages: vec![page(5), page(3)], ..Default::default() }));
        assert_eq!(wait_for(&ev, |e| matches!(e, Event::AwaitingSheet { .. })), Event::AwaitingSheet { next: 1 });
        assert_eq!(fake.lock().unwrap().lines.len(), 1 + 5); // $I + page 0 only
        h.send(Command::NextSheet);
        let end = wait_for(&ev, |e| matches!(e, Event::JobEnded { .. }));
        assert_eq!(end, Event::JobEnded { outcome: JobOutcome::Done, page: 1, line: 0 });
        let f = fake.lock().unwrap();
        assert_eq!(f.lines.len(), 1 + 5 + 3 + 2); // + park: pen up, back to origin
        assert_eq!(&f.lines[f.lines.len() - 2..], ["G1G90 Z0 F8000", "G90G0 X0Y0"]);
        drop(f);
        h.send(Command::Shutdown);
    }

    #[test]
    fn stop_holds_resets_and_lifts_pen() {
        let (h, ev, fake) = start(Fake { status: "<Hold:0|MPos:0,0,0>".into(), ..Default::default() });
        wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
        h.send(Command::Run(Job { pages: vec![page(5), page(5)], ..Default::default() }));
        wait_for(&ev, |e| matches!(e, Event::AwaitingSheet { .. }));
        h.stop();
        let end = wait_for(&ev, |e| matches!(e, Event::JobEnded { .. }));
        assert_eq!(end, Event::JobEnded { outcome: JobOutcome::Stopped, page: 1, line: 0 });
        let f = fake.lock().unwrap();
        assert!(f.realtime.contains(&realtime::FEED_HOLD) && f.realtime.contains(&realtime::SOFT_RESET));
        // Hold settled (Hold:0), so after the reset the pen lifts and the head parks at the origin.
        assert_eq!(&f.lines[f.lines.len() - 2..], ["G1G90 Z0 F8000", "G90G0 X0Y0"]);
        drop(f);
        h.send(Command::Shutdown);
    }

    #[test]
    fn usb_drop_reports_last_line_and_disconnects() {
        // $I is line 1; the link dies once 1 + 40 lines have been written.
        let (h, ev, _fake) = start(Fake { drop_after: Some(41), ..Default::default() });
        wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
        h.send(Command::Run(Job { pages: vec![page(200)], ..Default::default() }));
        match wait_for(&ev, |e| matches!(e, Event::Disconnected { .. })) {
            Event::Disconnected { page, last_line, .. } => {
                assert_eq!(page, Some(0));
                assert!(last_line.is_some_and(|l| l < 40), "last_line {last_line:?}");
            }
            _ => unreachable!(),
        }
        // Actor is still alive, rescanning, and refuses motion until the link is back.
        h.send(Command::Zero);
        assert!(matches!(wait_for(&ev, |e| matches!(e, Event::Error(_))), Event::Error(m) if m == "设备正在重新连接，请稍候"));
        h.send(Command::Shutdown);
    }

    #[test]
    fn pause_mid_job_holds_until_resume() {
        let (h, ev, fake) = start_slow(Fake::default(), Duration::from_millis(3));
        wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
        h.send(Command::Run(Job { pages: vec![page(100)], ..Default::default() }));
        wait_for(&ev, |e| matches!(e, Event::Progress { done: 1, .. }));
        h.pause();
        std::thread::sleep(Duration::from_millis(200));
        let sent_paused = fake.lock().unwrap().lines.len();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(fake.lock().unwrap().lines.len(), sent_paused, "lines sent while paused");
        h.resume();
        wait_for(&ev, |e| matches!(e, Event::JobEnded { outcome: JobOutcome::Done, .. }));
        h.send(Command::Shutdown);
        let f = fake.lock().unwrap();
        assert_eq!(f.lines.len(), 1 + 100 + 2); // + park
        let rt: Vec<u8> = f.realtime.iter().copied().filter(|b| *b != realtime::STATUS).collect();
        assert_eq!(rt, vec![realtime::FEED_HOLD, realtime::CYCLE_START]);
    }

    /// `n` strokes the way `gcode::emit` writes them: pen-up, travel, pen-down, two moves.
    fn stroke_page(n: usize) -> String {
        let mut s = String::from("G1G90 Z0 F8000\n");
        for i in 0..n {
            s += &format!("G0 X{i}.000 Y0.000\nG1G90 Z2 F8000\nG1 X{i}.000 Y1.000 F3000\nG1 X{i}.000 Y2.000 F3000\nG1G90 Z0 F8000\n");
        }
        s
    }

    fn lines_of(s: &str) -> Vec<String> {
        s.lines().map(String::from).collect()
    }

    #[test]
    fn resume_point_backs_up_to_a_pen_up() {
        let l = lines_of(&stroke_page(20)); // 1 + 5 per stroke
        let up = "G1G90 Z0";
        assert_eq!(resume_point(&l, 0, up), 0);
        assert_eq!(resume_point(&l, 10, up), 0, "within the planner depth of the start");
        // acked 60 → surely drawn up to index 43; pen-ups sit at 0, 5, 10, …, 40, 45.
        assert_eq!(resume_point(&l, 60, up), 40);
        assert!(l[40].starts_with(up));
        assert_eq!(resume_point(&l, 10_000, up), l.len() - 1);
        // Custom pen commands are matched verbatim.
        let custom: Vec<String> = ["M3 S0", "G0 X1", "M3 S90", "G1 X2", "M3 S0", "G0 X3", "M3 S90", "G1 X4"].map(String::from).to_vec();
        assert_eq!(resume_point(&custom, 4 + PLANNER_DEPTH + 1, "M3 S0"), 4);
    }

    #[test]
    fn usb_drop_reconnects_and_resumes() {
        let body = stroke_page(40); // 201 lines
        // $I is line 1; the link dies once 1 + 80 lines have been written.
        let (h, ev, fake) = start(Fake { drop_after: Some(81), ..Default::default() });
        wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
        h.send(Command::Run(Job { pages: vec![body.clone()], ..Default::default() }));
        let Event::Disconnected { page: Some(0), last_line: Some(acked), .. } = wait_for(&ev, |e| matches!(e, Event::Disconnected { .. })) else {
            panic!("expected a mid-job drop")
        };
        // Replug: the board rebooted and prints its banner.
        {
            let mut f = fake.lock().unwrap();
            f.drop_after = None;
            f.replies.push_back("Grbl 1.3a ['$' for help]".into());
        }
        let Event::Recovered { page: 0, line, rebooted } = wait_for(&ev, |e| matches!(e, Event::Recovered { .. })) else { unreachable!() };
        assert!(rebooted);
        assert_eq!(line, resume_point(&lines_of(&body), acked, "G1G90 Z0"));
        // The user jogs back and zeros while the job waits; then continues.
        h.send(Command::Zero);
        wait_for(&ev, |e| matches!(e, Event::Log { dir: Dir::Tx, line } if line == "G92 X0 Y0"));
        let sent_before = fake.lock().unwrap().lines.len();
        h.send(Command::ResumeJob);
        let end = wait_for(&ev, |e| matches!(e, Event::JobEnded { .. }));
        assert!(matches!(end, Event::JobEnded { outcome: JobOutcome::Done, page: 0, .. }), "{end:?}");
        h.send(Command::Shutdown);
        let f = fake.lock().unwrap();
        let resumed = &f.lines[sent_before..];
        let want: Vec<String> = std::iter::once("G90".to_string()).chain(lines_of(&body)[line..].iter().cloned()).collect();
        assert_eq!(&resumed[..want.len()], &want[..]);
    }

    #[test]
    fn usb_glitch_without_reboot_keeps_coordinates() {
        let status = "<Run|MPos:5.000,7.000,0.000>";
        let (h, ev, fake) = start_slow(Fake { drop_after: Some(60), status: status.into(), ..Default::default() }, Duration::from_millis(2));
        wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
        h.send(Command::Run(Job { pages: vec![stroke_page(40)], ..Default::default() }));
        wait_for(&ev, |e| matches!(e, Event::Disconnected { .. }));
        fake.lock().unwrap().drop_after = None; // replugged; the board kept running (no banner)
        assert!(matches!(wait_for(&ev, |e| matches!(e, Event::Recovered { .. })), Event::Recovered { rebooted: false, .. }));
        // Same position after reconnecting, but a board that reads (0, 0) would count as rebooted.
        h.send(Command::ResumeJob);
        assert!(matches!(wait_for(&ev, |e| matches!(e, Event::JobEnded { .. })), Event::JobEnded { outcome: JobOutcome::Done, .. }));
        h.send(Command::Shutdown);
    }

    #[test]
    fn stop_while_reconnecting_ends_job() {
        let (h, ev, _fake) = start(Fake { drop_after: Some(20), ..Default::default() });
        wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
        h.send(Command::Run(Job { pages: vec![stroke_page(40)], ..Default::default() }));
        wait_for(&ev, |e| matches!(e, Event::Disconnected { .. }));
        h.stop();
        assert!(matches!(wait_for(&ev, |e| matches!(e, Event::JobEnded { .. })), Event::JobEnded { outcome: JobOutcome::Stopped, .. }));
        h.send(Command::Shutdown);
    }

    #[test]
    fn first_page_offsets_events() {
        let (h, ev, _fake) = start(Fake::default());
        wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
        h.send(Command::Run(Job { pages: vec![page(2), page(2)], first_page: 3 }));
        assert!(matches!(wait_for(&ev, |e| matches!(e, Event::Progress { .. })), Event::Progress { page: 3, pages: 5, .. }));
        assert_eq!(wait_for(&ev, |e| matches!(e, Event::AwaitingSheet { .. })), Event::AwaitingSheet { next: 4 });
        h.send(Command::NextSheet);
        let end = wait_for(&ev, |e| matches!(e, Event::JobEnded { .. }));
        assert_eq!(end, Event::JobEnded { outcome: JobOutcome::Done, page: 4, line: 0 });
        h.send(Command::Shutdown);
    }

    #[test]
    fn end_position_controls_park() {
        use chixie_rs_core::gcode::{AxisMap, DeviceProfile, EndPosition};
        let cases = [
            (EndPosition::OriginLeft { mm: 20.0 }, AxisMap::default(), Some("G90G0 X-20Y0")),
            (EndPosition::OriginLeft { mm: 20.0 }, AxisMap { swap_xy: true, invert_x: false, invert_y: true }, Some("G90G0 X0Y20")),
            (EndPosition::Custom { x: 12.5, y: 3.0 }, AxisMap::default(), Some("G90G0 X12.5Y3")),
            (EndPosition::Stay, AxisMap::default(), None),
        ];
        for (end, axis_map, want) in cases {
            let (h, ev, fake) = start(Fake::default());
            wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
            h.send(Command::SetProfile(DeviceProfile { end_position: end, axis_map, ..Default::default() }));
            h.send(Command::Run(Job { pages: vec![page(2)], ..Default::default() }));
            wait_for(&ev, |e| matches!(e, Event::JobEnded { outcome: JobOutcome::Done, .. }));
            h.send(Command::Shutdown);
            let f = fake.lock().unwrap();
            let last = f.lines.last().unwrap().as_str();
            match want {
                Some(w) => assert_eq!(last, w, "{end:?}"),
                None => assert_eq!(last, "G1G90 Z0 F8000", "{end:?}"),
            }
        }
    }

    #[test]
    fn stale_pause_does_not_hold_next_job() {
        let (h, ev, _fake) = start(Fake::default());
        wait_for(&ev, |e| matches!(e, Event::Connected { .. }));
        h.pause(); // while idle: `!` queued, never sent
        h.send(Command::Run(Job { pages: vec![page(3)], ..Default::default() }));
        wait_for(&ev, |e| matches!(e, Event::JobEnded { outcome: JobOutcome::Done, .. }));
        h.send(Command::Shutdown);
    }
}
