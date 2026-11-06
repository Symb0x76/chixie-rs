use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::transport::Link;
use crate::{Error, Result};

/// Grbl RX buffer. Board doesn't report it via `$I`; 128 is the safe lower bound for Grbl_ESP32.
pub const RX_BUFFER_SIZE: usize = 128;

pub mod realtime {
    pub const STATUS: u8 = b'?';
    pub const FEED_HOLD: u8 = b'!';
    pub const CYCLE_START: u8 = b'~';
    pub const SOFT_RESET: u8 = 0x18;
}

pub struct Control {
    pub cancel: AtomicBool,
    pub response_timeout: Duration,
}

impl Default for Control {
    fn default() -> Self {
        Self { cancel: AtomicBool::new(false), response_timeout: Duration::from_secs(60) }
    }
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
/// `progress(done, total)` is called after each acknowledged line.
pub fn stream<'a>(
    link: &mut dyn Link,
    lines: impl IntoIterator<Item = &'a str>,
    ctl: &Control,
    mut progress: impl FnMut(usize, usize),
) -> Result<()> {
    let lines: Vec<String> = lines.into_iter().filter_map(clean_line).collect();
    let total = lines.len();
    let mut in_flight: VecDeque<usize> = VecDeque::new(); // byte length of each unacked line
    let mut sent = 0;
    let mut acked = 0;
    let mut last_rx = Instant::now();

    while acked < total {
        if ctl.cancel.load(Ordering::Relaxed) {
            link.write_all(&[realtime::FEED_HOLD])?;
            return Err(Error::Cancelled);
        }
        // Fill buffer.
        while sent < total {
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
                last_rx = Instant::now();
                progress(acked, total);
            }
            Some(l) => {
                check_line(&l, acked + 1)?;
                log::debug!("grbl: {l}");
            }
            None if last_rx.elapsed() > ctl.response_timeout => return Err(Error::Timeout),
            None => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fake Grbl: answers `ok` per received line, tracks peak buffered bytes.
    struct Fake {
        pending: VecDeque<usize>,
        peak: usize,
        replies: VecDeque<String>,
        fail_on: Option<usize>,
        received: usize,
    }

    impl Link for Fake {
        fn write_all(&mut self, d: &[u8]) -> Result<()> {
            if d.len() > 1 {
                self.pending.push_back(d.len());
                self.peak = self.peak.max(self.pending.iter().sum());
            }
            Ok(())
        }
        fn read_line(&mut self) -> Result<Option<String>> {
            if let Some(r) = self.replies.pop_front() {
                return Ok(Some(r));
            }
            Ok(self.pending.pop_front().map(|_| {
                self.received += 1;
                if Some(self.received) == self.fail_on { "error:20".into() } else { "ok".into() }
            }))
        }
    }

    fn fake() -> Fake {
        Fake { pending: VecDeque::new(), peak: 0, replies: VecDeque::new(), fail_on: None, received: 0 }
    }

    #[test]
    fn clean_strips_comments() {
        assert_eq!(clean_line("G1 X1 (move) Y2 ; tail").as_deref(), Some("G1 X1  Y2"));
        assert_eq!(clean_line("  ; only comment"), None);
    }

    #[test]
    fn stream_respects_buffer_and_finishes() {
        let gcode: Vec<String> = (0..500).map(|i| format!("G1 X{i}.123 Y{i}.456 F3000")).collect();
        let mut f = fake();
        let mut last = (0, 0);
        stream(&mut f, gcode.iter().map(String::as_str), &Control::default(), |d, t| last = (d, t)).unwrap();
        assert_eq!(last, (500, 500));
        assert!(f.peak <= RX_BUFFER_SIZE, "peak {}", f.peak);
    }

    #[test]
    fn stream_reports_error_line() {
        let mut f = fake();
        f.fail_on = Some(3);
        let err = stream(&mut f, ["G0 X0", "G0 X1", "G0 X2", "G0 X3"], &Control::default(), |_, _| {});
        assert!(matches!(err, Err(Error::Grbl { line: 3, .. })));
    }

    #[test]
    fn stream_stops_on_alarm() {
        let mut f = fake();
        f.replies.push_back("ALARM:1".into());
        let err = stream(&mut f, ["G0 X0"], &Control::default(), |_, _| {});
        assert!(matches!(err, Err(Error::Alarm(_))));
    }
}
