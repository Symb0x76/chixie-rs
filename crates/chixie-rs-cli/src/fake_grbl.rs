//! A fake Grbl board on TCP (ChiXie3's Telnet mode) that records every streamed line. Used to
//! capture ChiXie3's G-code without hardware: point ChiXie3's wireless device at it, start a
//! job, and the file holds exactly what a real board would have received.
//!
//! Behaviour (as a Grbl_ESP32 Telnet port): a banner on connect, `ok` per line, an Idle status
//! report for the realtime `?`; `!` `~` are swallowed and Ctrl-X re-sends the banner. The run ends
//! when the end marker line arrives (ChiXie3 finishes every job with `G90G0 X0Y0`), when the client
//! disconnects, or after `idle` with no traffic.

use std::io::{BufWriter, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;

const BANNER: &str = "\r\nGrbl 1.3a ['$' for help]\r\n";
const STATUS: &str = "<Idle|MPos:0.000,0.000,0.000|FS:0,0>\r\n";

/// Why the capture stopped.
#[derive(Debug, PartialEq, Eq)]
pub enum End {
    Marker,
    Disconnected,
    Idle,
}

/// Accept one client on `listener` and record its lines into `out` until `end_marker`.
pub fn run(listener: &TcpListener, out: &Path, end_marker: &str, idle: Duration, ack_delay: Duration) -> std::io::Result<(End, usize)> {
    let (stream, peer) = listener.accept()?;
    eprintln!("fake-grbl: client {peer}");
    let mut file = BufWriter::new(std::fs::File::create(out)?);
    let result = serve(stream, &mut file, end_marker, idle, ack_delay);
    file.flush()?;
    result
}

fn serve(mut stream: TcpStream, file: &mut impl Write, end_marker: &str, idle: Duration, ack_delay: Duration) -> std::io::Result<(End, usize)> {
    stream.set_read_timeout(Some(idle))?;
    stream.write_all(BANNER.as_bytes())?;
    let (mut line, mut lines, mut buf) = (Vec::<u8>::new(), 0usize, [0u8; 4096]);
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) => return Ok((End::Disconnected, lines)),
            Ok(n) => n,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                return Ok((End::Idle, lines));
            }
            Err(e) => return Err(e),
        };
        for &b in &buf[..n] {
            match b {
                b'?' => stream.write_all(STATUS.as_bytes())?,
                b'!' | b'~' => {}
                0x18 => stream.write_all(BANNER.as_bytes())?,
                b'\r' => {}
                b'\n' => {
                    let text = String::from_utf8_lossy(&line).trim().to_string();
                    line.clear();
                    if text.is_empty() {
                        continue;
                    }
                    writeln!(file, "{text}")?;
                    file.flush()?; // so a stalled sender can be inspected live
                    lines += 1;
                    // ChiXie3 counts one `ok` per socket read: acks that arrive together stall its
                    // character counter, so space them out like a real board.
                    std::thread::sleep(ack_delay);
                    stream.write_all(b"ok\r\n")?;
                    if text == end_marker {
                        return Ok((End::Marker, lines));
                    }
                }
                _ => line.push(b),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    #[test]
    fn records_lines_answers_ok_and_status_and_stops_at_marker() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let out = std::env::temp_dir().join(format!("fake-grbl-{}.gcode", std::process::id()));
        let out2 = out.clone();
        let server = std::thread::spawn(move || run(&listener, &out2, "G90G0 X0Y0", Duration::from_secs(5), Duration::ZERO).unwrap());

        let mut c = TcpStream::connect(addr).unwrap();
        let mut r = BufReader::new(c.try_clone().unwrap());
        let mut got = String::new();
        r.read_line(&mut got).unwrap(); // blank line before the banner
        r.read_line(&mut got).unwrap();
        assert!(got.contains("Grbl 1.3a"));
        c.write_all(b"G21\r\n?G1G90 Z0F19000\n\nG90G0 X0Y0\n").unwrap();
        let mut replies = Vec::new();
        for _ in 0..4 {
            let mut l = String::new();
            r.read_line(&mut l).unwrap();
            replies.push(l.trim().to_string());
        }
        assert_eq!(replies.iter().filter(|l| *l == "ok").count(), 3);
        assert!(replies.iter().any(|l| l.starts_with("<Idle|")));

        let (end, lines) = server.join().unwrap();
        assert_eq!((end, lines), (End::Marker, 3));
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "G21\nG1G90 Z0F19000\nG90G0 X0Y0\n");
        let _ = std::fs::remove_file(&out);
    }
}
