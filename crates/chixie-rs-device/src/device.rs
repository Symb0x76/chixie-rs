//! Device actor: one thread owns the controller link, and the UI talks to it by message.
//!
//! - Commands go in over `std::sync::mpsc` via [`DeviceHandle`].
//! - Events come out over `async-channel`, so a gpui foreground task can `.await` them (CLI
//!   code can use `recv_blocking`).
//! - Pause/Resume/Stop act on a shared [`grbl::Control`] directly. They work while a job is
//!   blocking inside `grbl::stream`.
//!
//! A link I/O error (the CH340 board drops off USB under motor load and comes back on a new COM
//! number; see memory `chixie-board-usb-drop`) ends the session with `Disconnected` and the last
//! acknowledged line. There is no auto-resume: reconnecting is the user's call.

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
    let actor = Actor { opener, rx, ev: ev_tx, ctl: ctl.clone(), link: None, profile: DeviceProfile::default() };
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
            Command::NextSheet | Command::Stop => {} // only meaningful inside a job
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
        while let Ok(Command::Stop | Command::NextSheet) = self.rx.try_recv() {}

        let pages = job.first_page + job.pages.len();
        for (i, body) in job.pages.iter().enumerate() {
            let page = job.first_page + i;
            if i > 0 {
                self.emit(Event::AwaitingSheet { next: page });
                match self.wait_for_sheet() {
                    Ok(true) => {}
                    Ok(false) => return self.end_stopped(page, 0),
                    Err(e) => return self.fail(e, Some(page), Some(0)),
                }
            }
            let mut acked = 0;
            let ev = self.ev.clone();
            let link = self.link.as_mut().expect("checked by require_link").as_mut();
            let r = grbl::stream(link, body.lines(), &self.ctl, |e| {
                let out = match e {
                    StreamEvent::Progress { done, total } => {
                        acked = done;
                        Event::Progress { page, pages, done, total }
                    }
                    StreamEvent::Status(st) => Event::Status(st),
                    StreamEvent::Message(line) => Event::Log { dir: Dir::Rx, line },
                    StreamEvent::Resynced { lost } => Event::Resynced { page, lost },
                };
                let _ = ev.send_blocking(out);
            });
            match r {
                Ok(()) => {}
                Err(Error::Cancelled) => return self.end_stopped(page, acked),
                Err(e @ (Error::Io(_) | Error::Serial(_))) => return self.fail(e, Some(page), Some(acked)),
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
        // Actor is still alive and reports the missing link instead of panicking.
        h.send(Command::Zero);
        assert!(matches!(wait_for(&ev, |e| matches!(e, Event::Error(_))), Event::Error(m) if m == "设备未连接"));
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
