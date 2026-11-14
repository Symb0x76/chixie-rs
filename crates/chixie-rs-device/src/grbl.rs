use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::transport::Link;
use chixie_rs_core::{Error, Result};

/// Grbl RX buffer. Board doesn't report it via `$I`; 128 is the safe lower bound for Grbl_ESP32.
pub const RX_BUFFER_SIZE: usize = 128;

pub mod realtime {
    pub const STATUS: u8 = b'?';
    pub const FEED_HOLD: u8 = b'!';
    pub const CYCLE_START: u8 = b'~';
    pub const SOFT_RESET: u8 = 0x18;
}

/// Shared job control, safe to poke from another thread while `stream` runs.
pub struct Control {
    pub cancel: AtomicBool,
    /// While set, `stream` sends no new lines (a feed hold was issued) but keeps reading.
    paused: AtomicBool,
    /// Realtime bytes (`!`, `~`, `0x18`, …) to inject ahead of the line queue.
    realtime: Mutex<Vec<u8>>,
    /// Give up when no line has been acknowledged for this long (status reports don't count).
    pub response_timeout: Duration,
    /// Lines outstanding, no ack for this long and the board reports `Idle` → resync.
    pub stall_after: Duration,
    /// Send `?` this often while streaming; `None` disables polling (and stall detection).
    pub status_interval: Option<Duration>,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            cancel: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            realtime: Mutex::new(Vec::new()),
            response_timeout: Duration::from_secs(60),
            stall_after: Duration::from_secs(5),
            status_interval: Some(Duration::from_millis(250)),
        }
    }
}

impl Control {
    /// Queue a realtime byte; `stream` writes it on its next loop iteration.
    pub fn send_realtime(&self, byte: u8) {
        self.realtime.lock().unwrap_or_else(|e| e.into_inner()).push(byte);
    }

    /// Feed hold now and stop feeding new lines until `resume`.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
        self.send_realtime(realtime::FEED_HOLD);
    }

    pub fn resume(&self) {
        self.send_realtime(realtime::CYCLE_START);
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Fresh state for a new job: not cancelled, not paused, no stale realtime bytes (a `!`
    /// queued while idle would otherwise hold the new job forever).
    pub fn reset(&self) {
        self.cancel.store(false, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        self.realtime.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    fn take_realtime(&self) -> Vec<u8> {
        std::mem::take(&mut *self.realtime.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// The machine's own specs from `$$` (Grbl settings), per axis (x, y).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MachineSettings {
    /// `$100/$101` steps per mm.
    pub steps_per_mm: Option<(f32, f32)>,
    /// `$110/$111` max rate, mm/min.
    pub max_rate: Option<(f32, f32)>,
    /// `$120/$121` acceleration, mm/s².
    pub accel: Option<(f32, f32)>,
    /// `$130/$131` max travel, mm.
    pub travel: Option<(f32, f32)>,
    /// Every `$n=value` line, in order.
    pub raw: Vec<(u16, String)>,
}

impl MachineSettings {
    /// Parse `$n=value` lines (other lines are ignored).
    pub fn parse<'a>(lines: impl IntoIterator<Item = &'a str>) -> Self {
        let mut raw = Vec::new();
        for l in lines {
            let Some((k, v)) = l.trim().strip_prefix('$').and_then(|r| r.split_once('=')) else { continue };
            if let Ok(n) = k.trim().parse::<u16>() {
                raw.push((n, v.trim().to_string()));
            }
        }
        let get = |n: u16| raw.iter().find(|(k, _)| *k == n).and_then(|(_, v)| v.parse::<f32>().ok());
        let pair = |a: u16, b: u16| Some((get(a)?, get(b)?));
        Self { steps_per_mm: pair(100, 101), max_rate: pair(110, 111), accel: pair(120, 121), travel: pair(130, 131), raw }
    }

    /// The slower axis' max rate (mm/min), if known.
    pub fn max_feed(&self) -> Option<f32> {
        self.max_rate.map(|(x, y)| x.min(y))
    }
}

/// Parsed `<State|MPos:x,y,z|WPos:..|FS:feed,speed|…>` status report.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GrblStatus {
    /// `Idle`, `Run`, `Hold:0`, `Alarm`, `Jog`, …
    pub state: String,
    pub mpos: Option<[f32; 3]>,
    pub wpos: Option<[f32; 3]>,
    /// Current feed (mm/min) from `FS:` or `F:`.
    pub feed: Option<f32>,
    /// `Bf:` (free planner blocks, free RX bytes); only when the board's `$10` mask enables it.
    pub buf: Option<(u16, u16)>,
}

impl GrblStatus {
    pub fn parse(line: &str) -> Option<Self> {
        let body = line.trim().strip_prefix('<')?.strip_suffix('>')?;
        let mut fields = body.split('|');
        let mut st = GrblStatus { state: fields.next()?.to_string(), ..Default::default() };
        let xyz = |v: &str| -> Option<[f32; 3]> {
            let mut it = v.split(',').map(|n| n.parse::<f32>().ok());
            Some([it.next()??, it.next()??, it.next().flatten().unwrap_or(0.0)])
        };
        for f in fields {
            let Some((k, v)) = f.split_once(':') else { continue };
            match k {
                "MPos" => st.mpos = xyz(v),
                "WPos" => st.wpos = xyz(v),
                "FS" | "F" => st.feed = v.split(',').next().and_then(|n| n.parse().ok()),
                "Bf" => st.buf = v.split_once(',').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))),
                _ => {}
            }
        }
        Some(st)
    }
}

/// What `stream` reports while it runs.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// `done` of `total` lines acknowledged.
    Progress { done: usize, total: usize },
    Status(GrblStatus),
    /// Any other non-`ok` line from the controller (`[MSG:…]`, echoes, …).
    Message(String),
    /// A stall was detected and `lost` acknowledgements were written off (the board had
    /// already consumed those lines).
    Resynced { lost: usize },
}

/// Strip comments/whitespace; returns None for empty lines.
pub fn clean_line(raw: &str) -> Option<String> {
    let mut s = raw.split(';').next().unwrap_or("").to_string();
    while let (Some(a), Some(b)) = (s.find('('), s.find(')')) {
        if b < a {
            break;
        }
        s.replace_range(a..=b, "");
    }
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Send one command and wait for `ok` (or error/alarm). Returns non-`ok` lines seen.
pub fn command(link: &mut dyn Link, cmd: &str, timeout: Duration) -> Result<Vec<String>> {
    link.write_all(format!("{cmd}\n").as_bytes())?;
    let deadline = Instant::now() + timeout;
    let mut out = Vec::new();
    loop {
        match link.read_line()? {
            Some(l) if l == "ok" => return Ok(out),
            Some(l) => {
                check_line(&l, 0)?;
                out.push(l);
            }
            None if Instant::now() > deadline => return Err(Error::Timeout),
            None => {}
        }
    }
}

pub fn unlock(link: &mut dyn Link) -> Result<Vec<String>> {
    command(link, "$X", Duration::from_secs(5))
}

fn check_line(l: &str, line: usize) -> Result<()> {
    if l.to_ascii_lowercase().starts_with("error") {
        return Err(Error::Grbl { line, msg: l.to_string() });
    }
    if l.to_ascii_uppercase().starts_with("ALARM") {
        return Err(Error::Alarm(l.to_string()));
    }
    Ok(())
}

/// Stream G-code using Grbl character-counting flow control.
///
/// `on_event` gets `Progress` after each acknowledged line, `Status` for each `?` report
/// (polled every `ctl.status_interval`), and `Message` for other controller output.
/// Realtime bytes queued on `ctl` are written ahead of lines; while `ctl.is_paused()` no new
/// lines are sent.
///
/// Lost acknowledgements (seen on Telnet): when lines are outstanding, nothing was acked for
/// `ctl.stall_after` and a fresh status says `Idle`, the board has nothing left to run, so the
/// missing `ok`s were lost. If the status carries `Bf:` with an empty RX buffer that is proof
/// enough; otherwise a `$G` marker is sent and its reply (Grbl handles RX strictly in order)
/// proves every earlier line was consumed. The outstanding lines are then counted as acked
/// (`StreamEvent::Resynced`). `ctl.response_timeout` without any ack (not paused, not in a
/// hold) is an error; status reports don't reset it.
pub fn stream<'a>(
    link: &mut dyn Link,
    lines: impl IntoIterator<Item = &'a str>,
    ctl: &Control,
    mut on_event: impl FnMut(StreamEvent),
) -> Result<()> {
    let lines: Vec<String> = lines.into_iter().filter_map(clean_line).collect();
    let total = lines.len();
    let mut in_flight: VecDeque<usize> = VecDeque::new(); // byte length of each unacked line
    let mut sent = 0;
    let mut acked = 0;
    let mut last_ack = Instant::now();
    let mut last_poll: Option<Instant> = None;
    // Latest status state, and whether it arrived after the latest ack.
    let mut state = String::new();
    let mut fresh_status: Option<GrblStatus> = None;

    while acked < total {
        if ctl.cancel.load(Ordering::Relaxed) {
            link.write_all(&[realtime::FEED_HOLD])?;
            return Err(Error::Cancelled);
        }
        for b in ctl.take_realtime() {
            link.write_all(&[b])?;
        }
        if let Some(every) = ctl.status_interval
            && last_poll.is_none_or(|t| t.elapsed() >= every)
        {
            link.write_all(&[realtime::STATUS])?;
            last_poll = Some(Instant::now());
        }
        // Fill buffer.
        while sent < total && !ctl.is_paused() {
            let len = lines[sent].len() + 1;
            if in_flight.iter().sum::<usize>() + len > RX_BUFFER_SIZE && !in_flight.is_empty() {
                break;
            }
            link.write_all(format!("{}\n", lines[sent]).as_bytes())?;
            in_flight.push_back(len);
            sent += 1;
        }
        match link.read_line()? {
            Some(l) if l == "ok" || l.to_ascii_lowercase().starts_with("error") => {
                check_line(&l, acked + 1)?;
                in_flight.pop_front();
                acked += 1;
                last_ack = Instant::now();
                fresh_status = None;
                on_event(StreamEvent::Progress { done: acked, total });
            }
            Some(l) => {
                check_line(&l, acked + 1)?;
                match GrblStatus::parse(&l) {
                    Some(st) => {
                        state.clone_from(&st.state);
                        fresh_status = Some(st.clone());
                        on_event(StreamEvent::Status(st));
                    }
                    None => {
                        log::debug!("grbl: {l}");
                        on_event(StreamEvent::Message(l));
                    }
                }
            }
            None => {
                let quiet = last_ack.elapsed();
                let stalled = !in_flight.is_empty()
                    && !ctl.is_paused()
                    && quiet > ctl.stall_after
                    && fresh_status.as_ref().is_some_and(|s| s.state == "Idle");
                if stalled {
                    let rx_empty = fresh_status.as_ref().and_then(|s| s.buf).is_some_and(|(_, rx)| rx as usize >= RX_BUFFER_SIZE - 1);
                    let late = if rx_empty { 0 } else { probe_consumed(link, ctl, acked, &mut on_event)? };
                    // Acks that arrived during the probe were real; the rest were lost.
                    for _ in 0..late.min(in_flight.len()) {
                        in_flight.pop_front();
                        acked += 1;
                    }
                    let lost = in_flight.len();
                    log::warn!("grbl: stall after line {acked}, {lost} ack(s) lost; resynced");
                    in_flight.clear();
                    acked += lost;
                    last_ack = Instant::now();
                    fresh_status = None;
                    on_event(StreamEvent::Resynced { lost });
                    on_event(StreamEvent::Progress { done: acked, total });
                } else if !ctl.is_paused() && !state.starts_with("Hold") && quiet > ctl.response_timeout {
                    return Err(Error::Timeout);
                }
            }
        }
    }
    Ok(())
}

/// Send a `$G` marker and wait for its `[GC:…]` reply and `ok`. Returns how many plain `ok`s
/// (late acks of earlier lines) arrived before the marker's reply.
fn probe_consumed(link: &mut dyn Link, ctl: &Control, acked: usize, on_event: &mut impl FnMut(StreamEvent)) -> Result<usize> {
    link.write_all(b"$G\n")?;
    let deadline = Instant::now() + ctl.stall_after;
    let (mut late, mut marker) = (0, false);
    while Instant::now() < deadline {
        match link.read_line()? {
            Some(l) if l == "ok" && marker => return Ok(late),
            Some(l) if l == "ok" => late += 1,
            Some(l) if l.starts_with("[GC:") => marker = true,
            Some(l) => {
                check_line(&l, acked + 1)?;
                match GrblStatus::parse(&l) {
                    Some(st) => on_event(StreamEvent::Status(st)),
                    None => on_event(StreamEvent::Message(l)),
                }
            }
            None => {}
        }
    }
    Err(Error::Timeout)
}

/// In-memory Grbl stand-in shared by this module's and `device`'s tests.
#[cfg(test)]
pub(crate) mod fake {
    use std::collections::VecDeque;

    use chixie_rs_core::Result;
    use crate::transport::Link;

    /// Answers `ok` per received line (or `error:20` on line `fail_on`), replies to `?` with
    /// `status`, records realtime bytes, and tracks peak buffered bytes.
    pub struct Fake {
        pub pending: VecDeque<usize>,
        pub peak: usize,
        pub replies: VecDeque<String>,
        pub fail_on: Option<usize>,
        pub received: usize,
        pub lines: Vec<String>,
        pub realtime: Vec<u8>,
        pub status: String,
        /// Once `lines.len()` reaches this, `read_line` fails like an unplugged USB port.
        pub drop_after: Option<usize>,
        /// `?` replies: served after queued `replies` and pending `ok`s, like a real Grbl that
        /// answers status between acks but never starves them.
        pub status_q: VecDeque<String>,
        /// Replies queued when a given line is received (before its `ok`), like `$$`.
        pub respond: Vec<(String, Vec<String>)>,
        /// 1-based received-line numbers whose `ok` is swallowed (lost on the wire).
        pub drop_ok: Vec<usize>,
    }

    impl Default for Fake {
        fn default() -> Self {
            Self {
                pending: VecDeque::new(),
                peak: 0,
                replies: VecDeque::new(),
                fail_on: None,
                received: 0,
                lines: Vec::new(),
                realtime: Vec::new(),
                status: "<Idle|MPos:0.000,0.000,0.000|FS:0,0>".into(),
                drop_after: None,
                status_q: VecDeque::new(),
                respond: vec![("$G".into(), vec!["[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]".into()])],
                drop_ok: Vec::new(),
            }
        }
    }

    impl Link for Fake {
        fn write_all(&mut self, d: &[u8]) -> Result<()> {
            if let [b] = d {
                self.realtime.push(*b);
                if *b == b'?' {
                    self.status_q.push_back(self.status.clone());
                }
                return Ok(());
            }
            let line = String::from_utf8_lossy(d).trim_end().to_string();
            if let Some((_, r)) = self.respond.iter().find(|(k, _)| *k == line) {
                self.replies.extend(r.iter().cloned());
            }
            self.lines.push(line);
            self.pending.push_back(d.len());
            self.peak = self.peak.max(self.pending.iter().sum());
            Ok(())
        }
        fn read_line(&mut self) -> Result<Option<String>> {
            if self.drop_after.is_some_and(|n| self.lines.len() >= n) {
                return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into());
            }
            if let Some(r) = self.replies.pop_front() {
                return Ok(Some(r));
            }
            while self.pending.pop_front().is_some() {
                self.received += 1;
                if self.drop_ok.contains(&self.received) {
                    continue;
                }
                let r = if Some(self.received) == self.fail_on { "error:20" } else { "ok" };
                return Ok(Some(r.into()));
            }
            Ok(self.status_q.pop_front())
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn parses_machine_settings() {
        let lines = ["$100=80.000", "$101=80.000", "$110=12000.000", "$111=12000.000", "$120=3000", "$121=3000", "$130=300.000", "$131=300.000", "$22=0", "[MSG:x]"];
        let m = MachineSettings::parse(lines);
        assert_eq!(m.steps_per_mm, Some((80.0, 80.0)));
        assert_eq!(m.max_feed(), Some(12000.0));
        assert_eq!(m.travel, Some((300.0, 300.0)));
        assert_eq!(m.accel, Some((3000.0, 3000.0)));
        assert_eq!(m.raw.len(), 9);
        assert_eq!(MachineSettings::parse(["ok"]), MachineSettings::default());
    }

    use super::fake::Fake;
    use super::*;

    fn no_poll() -> Control {
        Control { status_interval: None, ..Default::default() }
    }

    #[test]
    fn clean_strips_comments() {
        assert_eq!(clean_line("G1 X1 (move) Y2 ; tail").as_deref(), Some("G1 X1  Y2"));
        assert_eq!(clean_line("  ; only comment"), None);
    }

    #[test]
    fn stream_respects_buffer_and_finishes() {
        let gcode: Vec<String> = (0..500).map(|i| format!("G1 X{i}.123 Y{i}.456 F3000")).collect();
        let mut f = Fake::default();
        let mut last = None;
        stream(&mut f, gcode.iter().map(String::as_str), &no_poll(), |e| {
            if let StreamEvent::Progress { done, total } = e {
                last = Some((done, total));
            }
        })
        .unwrap();
        assert_eq!(last, Some((500, 500)));
        assert!(f.peak <= RX_BUFFER_SIZE, "peak {}", f.peak);
    }

    #[test]
    fn stream_reports_error_line() {
        let mut f = Fake { fail_on: Some(3), ..Default::default() };
        let err = stream(&mut f, ["G0 X0", "G0 X1", "G0 X2", "G0 X3"], &no_poll(), |_| {});
        assert!(matches!(err, Err(Error::Grbl { line: 3, .. })));
    }

    #[test]
    fn stream_stops_on_alarm() {
        let mut f = Fake::default();
        f.replies.push_back("ALARM:1".into());
        let err = stream(&mut f, ["G0 X0"], &no_poll(), |_| {});
        assert!(matches!(err, Err(Error::Alarm(_))));
    }

    #[test]
    fn parses_status_reports() {
        let st = GrblStatus::parse("<Run|MPos:12.500,-3.000,2.000|FS:3000,0|WCO:0,0,0>").unwrap();
        assert_eq!(st.state, "Run");
        assert_eq!(st.mpos, Some([12.5, -3.0, 2.0]));
        assert_eq!(st.feed, Some(3000.0));
        assert_eq!(GrblStatus::parse("<Hold:0|WPos:1,2>").unwrap().wpos, Some([1.0, 2.0, 0.0]));
        assert_eq!(GrblStatus::parse("[MSG:hi]"), None);
    }

    #[test]
    fn polls_status_and_emits_it() {
        let mut f = Fake { status: "<Run|MPos:1.000,2.000,0.000>".into(), ..Default::default() };
        // Short job: acks would drain before any queued `?` reply, so deliver one report first.
        f.replies.push_back(f.status.clone());
        let ctl = Control { status_interval: Some(Duration::ZERO), ..Default::default() };
        let mut statuses = 0;
        stream(&mut f, ["G0 X0", "G0 X1"], &ctl, |e| {
            if let StreamEvent::Status(st) = e {
                assert_eq!(st.mpos, Some([1.0, 2.0, 0.0]));
                statuses += 1;
            }
        })
        .unwrap();
        assert!(statuses >= 1);
        assert!(f.realtime.contains(&realtime::STATUS));
    }

    #[test]
    fn pause_holds_line_feed_until_resume() {
        // RX buffer fits ~3 lines of this size, so pausing after the first ack leaves the rest
        // queued on our side.
        let gcode: Vec<String> = (0..20).map(|i| format!("G1 X{i:03}.000 Y{i:03}.000 F3000 ; pad")).collect();
        let mut f = Fake::default();
        let ctl = Control { status_interval: Some(Duration::ZERO), ..Default::default() };
        let (mut paused_at, mut held_polls) = (None, 0);
        stream(&mut f, gcode.iter().map(String::as_str), &ctl, |e| match e {
            StreamEvent::Progress { done: 1, .. } => ctl.pause(),
            StreamEvent::Status(_) if ctl.is_paused() => {
                held_polls += 1;
                if held_polls == 1 {
                    paused_at = Some(());
                }
                if held_polls == 5 {
                    ctl.resume();
                }
            }
            _ => {}
        })
        .unwrap();
        assert!(paused_at.is_some());
        assert_eq!(f.lines.len(), 20);
        // Hold then cycle-start were injected, in that order.
        let rt: Vec<u8> = f.realtime.iter().copied().filter(|b| *b != realtime::STATUS).collect();
        assert_eq!(rt, vec![realtime::FEED_HOLD, realtime::CYCLE_START]);
    }

    fn fast_stall() -> Control {
        Control {
            status_interval: Some(Duration::from_millis(5)),
            stall_after: Duration::from_millis(50),
            response_timeout: Duration::from_millis(400),
            ..Default::default()
        }
    }

    fn run_collect(f: &mut Fake, n: usize, ctl: &Control) -> (Result<()>, Vec<StreamEvent>) {
        let gcode: Vec<String> = (0..n).map(|i| format!("G1 X{i}.000 Y{i}.000 F3000")).collect();
        let mut evs = Vec::new();
        let r = stream(f, gcode.iter().map(String::as_str), ctl, |e| {
            if !matches!(e, StreamEvent::Status(_)) {
                evs.push(e)
            }
        });
        (r, evs)
    }

    #[test]
    fn lost_ok_resyncs_with_marker() {
        let mut f = Fake { drop_ok: vec![7], ..Default::default() };
        let (r, evs) = run_collect(&mut f, 10, &fast_stall());
        r.unwrap();
        assert!(evs.contains(&StreamEvent::Resynced { lost: 1 }), "{evs:?}");
        assert_eq!(evs.last(), Some(&StreamEvent::Progress { done: 10, total: 10 }));
        assert!(f.lines.contains(&"$G".to_string()));
    }

    #[test]
    fn lost_ok_resyncs_from_bf_without_marker() {
        let mut f = Fake { drop_ok: vec![3], status: "<Idle|MPos:0,0,0|Bf:15,128>".into(), ..Default::default() };
        let (r, evs) = run_collect(&mut f, 5, &fast_stall());
        r.unwrap();
        assert!(evs.contains(&StreamEvent::Resynced { lost: 1 }));
        assert!(!f.lines.contains(&"$G".to_string()), "marker sent although Bf proved an empty buffer");
    }

    #[test]
    fn running_board_is_not_a_stall() {
        // Board keeps reporting Run and never acks: no resync, the hard timeout fires instead.
        let mut f = Fake { drop_ok: vec![2], status: "<Run|MPos:0,0,0>".into(), ..Default::default() };
        let (r, evs) = run_collect(&mut f, 4, &fast_stall());
        assert!(matches!(r, Err(Error::Timeout)), "{r:?}");
        assert!(!evs.iter().any(|e| matches!(e, StreamEvent::Resynced { .. })));
    }

    #[test]
    fn silent_board_times_out() {
        // Status replies alone must not keep a job without acks alive.
        let mut f = Fake { drop_ok: vec![1, 2, 3, 4], respond: Vec::new(), status: "<Idle|MPos:0,0,0>".into(), ..Default::default() };
        let (r, _) = run_collect(&mut f, 4, &fast_stall());
        assert!(matches!(r, Err(Error::Timeout)), "{r:?}");
    }

    #[test]
    fn parses_buffer_field() {
        assert_eq!(GrblStatus::parse("<Idle|MPos:0,0,0|Bf:15,127>").unwrap().buf, Some((15, 127)));
    }

    #[test]
    fn paused_stream_sends_no_new_lines() {
        let gcode: Vec<String> = (0..20).map(|i| format!("G1 X{i:03}.000 Y{i:03}.000 F3000 ; pad")).collect();
        let mut f = Fake::default();
        let ctl = Control { status_interval: Some(Duration::ZERO), ..Default::default() };
        ctl.pause();
        let mut polls = 0;
        let mut sent_while_paused = Vec::new();
        let r = stream(&mut f, gcode.iter().map(String::as_str), &ctl, |e| {
            if let StreamEvent::Status(_) = e {
                polls += 1;
                sent_while_paused.push(polls);
                if polls == 10 {
                    ctl.cancel.store(true, Ordering::SeqCst);
                }
            }
        });
        assert!(matches!(r, Err(Error::Cancelled)));
        assert!(f.lines.is_empty(), "sent {:?}", f.lines);
    }
}
