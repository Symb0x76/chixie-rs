//! Simulated Grbl controller for `CHIXIE_RS_FAKE`: answers `ok` per line after a short delay,
//! tracks the pen position from G0/G1/`$J`, honours feed hold / resume / soft reset and replies
//! to `?` with a status report. Enough to exercise the device UI without hardware.

use std::collections::VecDeque;
use std::time::Duration;

use chixie_rs_core::Result;
use chixie_rs_device::device::{BoxLink, Opener};
use chixie_rs_device::grbl::realtime;
use chixie_rs_device::transport::Link;

pub struct SimGrbl {
    out: VecDeque<String>,
    /// Lines accepted but not yet acknowledged (executed one per `delay`).
    queue: VecDeque<String>,
    pos: [f32; 2],
    held: bool,
    alarm: bool,
    delay: Duration,
    /// Lines executed so far, for the fault knobs below.
    executed: usize,
    /// `CHIXIE_RS_SIM_DROP_AT=n`: the link dies after `n` lines (a USB brown-out).
    drop_at: Option<usize>,
    /// `CHIXIE_RS_SIM_DROP_OK=n`: the `ok` for line `n` is lost.
    drop_ok: Option<usize>,
}

impl SimGrbl {
    fn new(delay: Duration) -> Self {
        Self { out: VecDeque::new(), queue: VecDeque::new(), pos: [0.0; 2], held: false, alarm: false, delay, executed: 0, drop_at: None, drop_ok: None }
    }

    fn status(&self) -> String {
        let state = if self.alarm {
            "Alarm"
        } else if self.held {
            "Hold:0"
        } else if self.queue.is_empty() {
            "Idle"
        } else {
            "Run"
        };
        format!("<{state}|MPos:{:.3},{:.3},0.000|FS:0,0>", self.pos[0], self.pos[1])
    }

    fn execute(&mut self, line: &str) {
        let up = line.to_ascii_uppercase();
        if up == "$I" {
            self.out.push_back("[VER:1.1h.SIM:]".into());
        } else if up == "$$" {
            // The verified board's settings (docs/protocol.md hardware probe).
            for l in ["$100=80.000", "$101=80.000", "$110=12000.000", "$111=12000.000", "$120=3000.000", "$121=3000.000", "$130=300.000", "$131=300.000"] {
                self.out.push_back(l.into());
            }
        } else if up == "$G" {
            self.out.push_back("[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]".into());
        } else if up == "$X" {
            self.alarm = false;
        } else if up.starts_with("G92") {
            self.pos = [0.0; 2];
            return;
        }
        let relative = up.starts_with("$J=") && up.contains("G91");
        for (axis, i) in [('X', 0), ('Y', 1)] {
            if let Some(v) = word(&up, axis) {
                self.pos[i] = if relative { self.pos[i] + v } else { v };
            }
        }
    }
}

/// Value after `axis` in a G-code line (`X12.5` → 12.5).
fn word(line: &str, axis: char) -> Option<f32> {
    let i = line.find(axis)?;
    let rest = &line[i + 1..];
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

impl Link for SimGrbl {
    fn write_all(&mut self, d: &[u8]) -> Result<()> {
        if let [b] = d {
            match *b {
                realtime::STATUS => self.out.push_back(self.status()),
                realtime::FEED_HOLD => self.held = true,
                realtime::CYCLE_START => self.held = false,
                realtime::SOFT_RESET => {
                    self.queue.clear();
                    self.held = false;
                    self.out.push_back("Grbl 1.1h ['$' for help]".into());
                }
                _ => {}
            }
            return Ok(());
        }
        let line = String::from_utf8_lossy(d).trim().to_string();
        self.queue.push_back(line);
        Ok(())
    }

    fn read_line(&mut self) -> Result<Option<String>> {
        if self.drop_at.is_some_and(|n| self.executed >= n) {
            return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into());
        }
        if let Some(l) = self.out.pop_front() {
            return Ok(Some(l));
        }
        if !self.held
            && let Some(line) = self.queue.pop_front()
        {
            if !self.delay.is_zero() {
                std::thread::sleep(self.delay);
            }
            self.execute(&line);
            self.executed += 1;
            // `$I` info lines go out before its `ok`.
            if self.drop_ok != Some(self.executed) {
                self.out.push_back("ok".into());
            }
            return Ok(self.out.pop_front());
        }
        std::thread::sleep(Duration::from_millis(5)); // like a serial read timeout
        Ok(None)
    }
}

/// Opener that ignores the target and connects to a fresh simulator. The fault knobs
/// (`CHIXIE_RS_SIM_DROP_AT`, `CHIXIE_RS_SIM_DROP_OK`) apply to the first connection only; a
/// reconnect gets a healthy board that has just rebooted (prints its banner).
pub fn opener(delay: Duration) -> Opener {
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<usize>().ok());
    let (drop_at, drop_ok) = (env("CHIXIE_RS_SIM_DROP_AT"), env("CHIXIE_RS_SIM_DROP_OK"));
    let mut opens = 0;
    Box::new(move |_t| {
        opens += 1;
        let mut g = SimGrbl::new(delay);
        if opens == 1 {
            (g.drop_at, g.drop_ok) = (drop_at, drop_ok);
        } else if drop_at.is_some() {
            g.out.push_back("Grbl 1.1h ['$' for help]".into());
        }
        Ok((Box::new(g) as BoxLink, "模拟设备".into()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_position_and_hold() {
        let mut g = SimGrbl::new(Duration::ZERO);
        g.write_all(b"G90G0 X10Y-5\n").unwrap();
        assert_eq!(g.read_line().unwrap().as_deref(), Some("ok"));
        g.write_all(b"$J=G21G91X1.000Y1.000F1000\n").unwrap();
        g.write_all(b"!").unwrap();
        assert_eq!(g.read_line().unwrap(), None); // held: nothing executes
        g.write_all(b"~").unwrap();
        assert_eq!(g.read_line().unwrap().as_deref(), Some("ok"));
        g.write_all(b"?").unwrap();
        assert_eq!(g.read_line().unwrap().as_deref(), Some("<Idle|MPos:11.000,-4.000,0.000|FS:0,0>"));
    }
}
