mod fake_grbl;

use std::path::PathBuf;
use std::time::Duration;

use chixie_rs_core::{api, calibration, config, gcode, handwriting, layout};
use chixie_rs_device::{discovery, esp3d, grbl, transport};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "chixie-rs: fonts, layout and device control for pen-plotter handwriting machines")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

// Parsed once per process; boxing `Write`'s many flags would only add noise.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Cmd {
    /// List serial ports
    Ports,
    /// Find wireless boards on the LAN
    Discover,
    /// Send one command over serial (or telnet with --ip) and print replies
    Send {
        command: String,
        #[arg(long)]
        port: Option<String>,
        #[arg(long)]
        ip: Option<String>,
        #[arg(long, default_value_t = transport::DEFAULT_BAUD)]
        baud: u32,
    },
    /// Stream a G-code file (serial, or telnet with --ip)
    Stream {
        file: PathBuf,
        #[arg(long)]
        port: Option<String>,
        #[arg(long)]
        ip: Option<String>,
        #[arg(long, default_value_t = transport::DEFAULT_BAUD)]
        baud: u32,
    },
    /// SD card over HTTP
    Sd {
        ip: String,
        #[command(subcommand)]
        op: SdOp,
    },
    /// Download one font and report why decryption fails, without printing secrets. Logs in with
    /// the GUI's saved account and the password from the Windows credential store ("remember
    /// password" in the GUI), or CHIXIE_RS_ACCOUNT / CHIXIE_RS_PASSWORD.
    FontCheck {
        /// One font id; omit with --all
        #[arg(long)]
        font_id: Option<i64>,
        /// Check every collected font
        #[arg(long)]
        all: bool,
        /// Send a heartbeat before fetching (as the GUI does every 60 s)
        #[arg(long)]
        heartbeat: bool,
        /// Log in a second time (another client) before fetching with the first
        #[arg(long)]
        relogin: bool,
        /// Wait this many seconds after login before fetching
        #[arg(long, default_value_t = 0)]
        wait: u64,
    },
    /// List fonts (login via CHIXIE_RS_ACCOUNT / CHIXIE_RS_PASSWORD env)
    Fonts {
        /// Browse the full online catalog instead of just your collected fonts
        #[arg(long)]
        online: bool,
        /// Only this page. Default: all pages until empty (online is 1-based).
        #[arg(long)]
        page: Option<i64>,
        /// Filter online fonts by name substring
        #[arg(long, default_value = "")]
        search: String,
    },
    /// Generate writing G-code from text; print to stdout or --out, optionally stream
    Write {
        text: String,
        #[arg(long)]
        font_id: Option<i64>,
        /// Offline: decode a font from a saved encrypted blob + key file instead of login
        #[arg(long, requires = "key_file")]
        blob_file: Option<PathBuf>,
        /// Offline: an already-decrypted glyph JSON file (e.g. the test fixture)
        #[arg(long, conflicts_with_all = ["blob_file", "font_id"])]
        font_json: Option<PathBuf>,
        #[arg(long)]
        key_file: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        /// mm per glyph unit (character size). Prefer --font-size, which uses the calibrated rule.
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// ChiXie3 font size; sets mm per unit = size / 100 (verified by capture). Overrides --scale.
        #[arg(long)]
        font_size: Option<f32>,
        /// Handwriting/deformation profile (TOML). Write a template with `chixie style-template`.
        #[arg(long)]
        style: Option<PathBuf>,
        /// Override the style's random seed (same seed = same page)
        #[arg(long)]
        seed: Option<u64>,
        /// Force handwriting jitter off, even if the style enables it
        #[arg(long)]
        no_jitter: bool,
        /// Force base character deformation on (style ranges, or ChiXie3 defaults)
        #[arg(long)]
        deform: bool,
        /// Page origin in mm (machine coords). Y is the top; glyphs descend from it.
        #[arg(long, default_value_t = 0.0)]
        origin_x: f32,
        #[arg(long, default_value_t = 24.0)]
        origin_y: f32,
        /// Prepend `G92 X0 Y0` to zero work coords at the pen's current position.
        #[arg(long)]
        zero: bool,
        #[arg(long)]
        port: Option<String>,
        #[arg(long)]
        ip: Option<String>,
    },
    /// Fit mm-per-glyph-unit from G-code captured from ChiXie3 (see docs/protocol.md)
    Calibrate {
        /// Captured G-code (one line per command, as ChiXie3 streamed it)
        capture: PathBuf,
        /// The exact text ChiXie3 wrote (whitespace is ignored)
        #[arg(long)]
        text: String,
        #[arg(long)]
        font_id: i64,
        /// Z value that means pen down in the capture
        #[arg(long, default_value_t = 2.0)]
        pen_down_z: f32,
    },
    /// Print a style profile (ChiXie3 defaults, jitter on, deformation off) for `write --style`
    StyleTemplate,
    /// Convert ChiXie3's config.ini writing/deformation settings into a style profile
    /// (only [WriteConfig] and [CharacterDeformation] are read; login data is ignored)
    ImportStyle {
        /// Default: %USERPROFILE%\ChiXie3\conf\config.ini
        #[arg(long)]
        from: Option<PathBuf>,
    },
    /// Pretend to be a Grbl board on TCP and record every line streamed to it (captures ChiXie3's
    /// output without hardware; see tools/capture/README.md)
    FakeGrbl {
        /// Where the recorded G-code goes
        out: PathBuf,
        #[arg(long, default_value = "127.0.0.1:23")]
        bind: String,
        /// Stop after this line (ChiXie3 ends every job with it)
        #[arg(long, default_value = "G90G0 X0Y0")]
        end: String,
        /// Give up after this many seconds without traffic
        #[arg(long, default_value_t = 120)]
        idle: u64,
        /// Delay before each `ok` (ChiXie3 stalls when acks arrive back to back)
        #[arg(long, default_value_t = 5)]
        ack_delay_ms: u64,
    },
    /// Compare two G-code captures stroke by stroke: count, points per stroke and shape. Exit 1 if
    /// they differ more than --tol (absolute positions are not compared: spacing is random)
    CaptureDiff {
        a: PathBuf,
        b: PathBuf,
        /// Max per-point deviation in mm after aligning each stroke on its first point
        #[arg(long, default_value_t = 0.05)]
        tol: f32,
        #[arg(long, default_value_t = 2.0)]
        pen_down_z: f32,
    },
}

struct CaptureDiff {
    point_mismatches: usize,
    max_dev: f32,
}

/// Compare strokes pairwise in order. Each stroke is aligned on its first point, so random
/// character spacing and line offsets don't count; glyph shape and stroke structure do.
fn capture_diff(a: &[Vec<(f32, f32)>], b: &[Vec<(f32, f32)>]) -> CaptureDiff {
    let mut d = CaptureDiff { point_mismatches: 0, max_dev: 0.0 };
    for (sa, sb) in a.iter().zip(b) {
        if sa.len() != sb.len() {
            d.point_mismatches += 1;
            continue;
        }
        let (oa, ob) = (sa[0], sb[0]);
        for (pa, pb) in sa.iter().zip(sb) {
            let dev = ((pa.0 - oa.0) - (pb.0 - ob.0)).hypot((pa.1 - oa.1) - (pb.1 - ob.1));
            d.max_dev = d.max_dev.max(dev);
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_diff_ignores_offsets_but_not_shape() {
        let a = vec![vec![(0.0, 0.0), (1.0, 0.0)], vec![(5.0, 5.0), (5.0, 6.0)]];
        let shifted = vec![vec![(2.0, 1.0), (3.0, 1.0)], vec![(9.0, 9.0), (9.0, 10.0)]];
        let d = capture_diff(&a, &shifted);
        assert_eq!((d.point_mismatches, d.max_dev), (0, 0.0));
        let bent = vec![vec![(0.0, 0.0), (1.0, 0.5)], vec![(5.0, 5.0)]];
        let d = capture_diff(&a, &bent);
        assert_eq!(d.point_mismatches, 1);
        assert!((d.max_dev - 0.5).abs() < 1e-6);
    }
}

fn api_login() -> chixie_rs_core::Result<api::Client> {
    let account = std::env::var("CHIXIE_RS_ACCOUNT")
        .map_err(|_| chixie_rs_core::Error::Api("set CHIXIE_RS_ACCOUNT".into()))?;
    let password = std::env::var("CHIXIE_RS_PASSWORD")
        .map_err(|_| chixie_rs_core::Error::Api("set CHIXIE_RS_PASSWORD".into()))?;
    let mut c = api::Client::new()?;
    let l = c.login(&account, &password)?;
    eprintln!("logged in as {} ({})", l.username, l.role_code);
    Ok(c)
}

#[derive(Subcommand)]
enum SdOp {
    Ls { #[arg(default_value = esp3d::TASK_DIR)] dir: String },
    Upload { file: PathBuf, #[arg(long, default_value = esp3d::TASK_DIR)] dir: String },
    Rm { name: String, #[arg(long, default_value = esp3d::TASK_DIR)] dir: String },
    Run { path: String },
    Status,
}

fn print_progress(e: grbl::StreamEvent) {
    match e {
        grbl::StreamEvent::Progress { done, total } => eprint!("\r{done}/{total}"),
        grbl::StreamEvent::Status(_) => {}
        grbl::StreamEvent::Message(m) => eprint!("\n{m}\n"),
    }
}

fn open(port: Option<String>, ip: Option<String>, baud: u32) -> chixie_rs_core::Result<Box<dyn transport::Link>> {
    Ok(match (ip, port) {
        (Some(ip), _) => Box::new(transport::open_telnet(&ip, transport::DEFAULT_TELNET_PORT)?),
        (None, Some(p)) => Box::new(transport::open_serial(&p, baud)?),
        (None, None) => {
            let p = transport::list_ports()?
                .into_iter()
                .find(|p| p.known_device)
                .ok_or_else(|| std::io::Error::other("no ChiXie serial port found; pass --port"))?;
            Box::new(transport::open_serial(&p.name, baud)?)
        }
    })
}

fn main() -> chixie_rs_core::Result<()> {
    env_logger::init();
    match Cli::parse().cmd {
        Cmd::Ports => {
            for p in transport::list_ports()? {
                println!("{}\t{}{}", p.name, p.description, if p.known_device { "\t[chixie]" } else { "" });
            }
        }
        Cmd::Discover => {
            for d in discovery::discover()? {
                println!("{}\t{}\t{}", d.ip, d.friendly_name.unwrap_or_default(), d.model_name.unwrap_or_default());
            }
        }
        Cmd::Send { command, port, ip, baud } => {
            let mut link = open(port, ip, baud)?;
            for l in grbl::command(link.as_mut(), &command, Duration::from_secs(10))? {
                println!("{l}");
            }
            println!("ok");
        }
        Cmd::Stream { file, port, ip, baud } => {
            let text = std::fs::read_to_string(file)?;
            let mut link = open(port, ip, baud)?;
            grbl::stream(link.as_mut(), text.lines(), &grbl::Control::default(), print_progress)?;
            eprintln!("\ndone");
        }
        Cmd::Sd { ip, op } => {
            let b = esp3d::Board::new(&ip)?;
            let out = match op {
                SdOp::Ls { dir } => b.list(&dir)?,
                SdOp::Upload { file, dir } => b.upload_file(&dir, &file)?,
                SdOp::Rm { name, dir } => b.delete(&dir, &name)?,
                SdOp::Run { path } => b.run_sd_file(&path)?,
                SdOp::Status => b.status()?,
            };
            println!("{out}");
        }
        Cmd::FontCheck { font_id, all, heartbeat, relogin, wait } => {
            let settings = chixie_rs_core::config::Settings::default_path()
                .and_then(|p| chixie_rs_core::config::Settings::load(&p))
                .unwrap_or_default();
            let account = std::env::var("CHIXIE_RS_ACCOUNT").ok().or(settings.account).ok_or_else(|| {
                chixie_rs_core::Error::Api("no saved account: log in once in the GUI, or set CHIXIE_RS_ACCOUNT".into())
            })?;
            let password = match std::env::var("CHIXIE_RS_PASSWORD") {
                Ok(p) => p,
                Err(_) => chixie_rs_core::credentials::load(&account)?.ok_or_else(|| {
                    chixie_rs_core::Error::Api("no saved password: tick 记住密码 in the GUI, or set CHIXIE_RS_PASSWORD".into())
                })?,
            };
            let mut c = api::Client::new()?;
            let l = c.login(&account, &password)?;
            eprintln!("logged in as {} ({})", l.username, l.role_code);
            if wait > 0 {
                std::thread::sleep(std::time::Duration::from_secs(wait));
            }
            if heartbeat {
                c.heartbeat()?;
                eprintln!("heartbeat ok");
            }
            if relogin {
                let mut other = api::Client::new()?;
                other.login(&account, &password)?;
                eprintln!("second login ok");
            }
            let ids: Vec<(i64, String)> = if all {
                c.collected_fonts(0)?.into_iter().map(|f| (f.id, f.font_name)).collect()
            } else {
                let id = font_id.ok_or_else(|| chixie_rs_core::Error::Api("give --font-id N or --all".into()))?;
                vec![(id, String::new())]
            };
            for (id, name) in ids {
                match c.font(id) {
                    Ok(f) => println!("font {id} {name}: OK, {} glyphs", f.glyphs.len()),
                    Err(e) => println!("font {id} {name}: {e}"),
                }
            }
        }
        Cmd::Fonts { online, page, search } => {
            let c = api_login()?;
            // Online catalog is 1-based; collected is one page (0).
            let fetch = |p: i64| if online { c.online_fonts(p, &search) } else { c.collected_fonts(p) };
            let mut printed = 0;
            let mut emit = |batch: Vec<chixie_rs_core::api::FontInfo>| {
                for f in &batch {
                    println!("{}\t{}\t{} chars", f.id, f.font_name, f.character_number);
                }
                printed += batch.len();
                batch.len()
            };
            match (page, online) {
                (Some(p), _) => {
                    emit(fetch(p)?);
                }
                (None, false) => {
                    emit(fetch(0)?);
                }
                (None, true) => {
                    for p in 1..500 {
                        if emit(fetch(p)?) == 0 {
                            break;
                        }
                    }
                }
            }
            eprintln!("{printed} fonts");
        }
        Cmd::ImportStyle { from } => {
            let path = match from {
                Some(p) => p,
                None => PathBuf::from(std::env::var("USERPROFILE").map_err(|_| {
                    chixie_rs_core::Error::Config("USERPROFILE not set; pass --from".into())
                })?)
                .join(r"ChiXie3\conf\config.ini"),
            };
            print!("{}", config::import_chixie3(&std::fs::read_to_string(&path)?).to_toml());
        }
        Cmd::FakeGrbl { out, bind, end, idle, ack_delay_ms } => {
            let listener = std::net::TcpListener::bind(&bind)?;
            eprintln!("fake-grbl: listening on {bind}, recording to {}", out.display());
            let (why, lines) = fake_grbl::run(&listener, &out, &end, Duration::from_secs(idle), Duration::from_millis(ack_delay_ms))?;
            eprintln!("fake-grbl: {lines} lines, stopped: {why:?}");
            if why != fake_grbl::End::Marker {
                std::process::exit(2);
            }
        }
        Cmd::CaptureDiff { a, b, tol, pen_down_z } => {
            let sa = calibration::parse_strokes(&std::fs::read_to_string(&a)?, pen_down_z);
            let sb = calibration::parse_strokes(&std::fs::read_to_string(&b)?, pen_down_z);
            let d = capture_diff(&sa, &sb);
            println!("strokes\t{}\t{}", sa.len(), sb.len());
            println!("point_count_mismatches\t{}", d.point_mismatches);
            println!("max_shape_dev_mm\t{:.4}", d.max_dev);
            if sa.len() != sb.len() || d.point_mismatches > 0 || d.max_dev > tol {
                std::process::exit(1);
            }
        }
        Cmd::StyleTemplate => {
            let mut s = handwriting::StyleFile::default();
            s.handwriting.enabled = true;
            s.handwriting.seed = 1;
            print!("{}", s.to_toml());
        }
        Cmd::Calibrate { capture, text, font_id, pen_down_z } => {
            let strokes = calibration::parse_strokes(&std::fs::read_to_string(capture)?, pen_down_z);
            let font = api_login()?.font(font_id)?;
            let f = calibration::fit(&strokes, &text, &font)?;
            println!("mm_per_unit\t{:.5}", f.mm_per_unit);
            println!("rms_mm\t{:.4}", f.rms);
            for (ch, si, cap, fnt) in &f.skipped {
                println!("skipped\t'{ch}' stroke {si}: capture {cap} points, font {fnt}");
            }
            println!("glyph\toffset_x\toffset_y\tadvance_to_next\trms");
            for (i, p) in f.placements.iter().enumerate() {
                let adv = f.placements.get(i + 1).map(|n| format!("{:.3}", n.offset.0 - p.offset.0)).unwrap_or_default();
                println!("{}\t{:.3}\t{:.3}\t{adv}\t{:.4}", p.ch, p.offset.0, p.offset.1, p.rms);
            }
        }
        Cmd::Write {
            text, font_id, blob_file, key_file, font_json, out, scale, font_size, style, seed, no_jitter, deform, origin_x, origin_y, zero, port, ip,
        } => {
            let font = match (blob_file, key_file) {
                _ if font_json.is_some() => {
                    chixie_rs_core::font::Font::from_json(&std::fs::read(font_json.as_deref().unwrap())?)?
                }
                (Some(bf), Some(kf)) => {
                    let blob = std::fs::read_to_string(bf)?;
                    let key = std::fs::read_to_string(kf)?;
                    chixie_rs_core::font::Font::from_encrypted(blob.trim(), key.trim())?
                }
                _ => {
                    let id = font_id.ok_or_else(|| chixie_rs_core::Error::Api("--font-id required".into()))?;
                    let c = api_login()?;
                    c.font(id)?
                }
            };
            eprintln!("font: {} glyphs", font.glyphs.len());
            let file = style.as_deref().map(handwriting::StyleFile::load).transpose()?;
            let resolved = handwriting::resolve(file, seed, no_jitter, deform);
            let params = layout::PageParams {
                mm_per_unit: font_size.map(calibration::mm_per_unit_for_font_size).unwrap_or(scale),
                origin: (origin_x, origin_y),
                style: resolved.handwriting,
                deformation: resolved.deformation,
                ..Default::default()
            };
            let doc = layout::layout(&text, &font, &params);
            if !doc.missing.is_empty() {
                eprintln!("missing glyphs (skipped): {}", doc.missing.iter().collect::<String>());
            }
            let mut gcode = gcode::emit_document(&doc, &gcode::DeviceProfile::default());
            if zero {
                // Zero work coords at the pen's current position so absolute moves are local.
                gcode.insert_str(0, "G92 X0 Y0\n");
            }
            match &out {
                Some(path) => std::fs::write(path, &gcode)?,
                None if port.is_none() && ip.is_none() => print!("{gcode}"),
                None => {}
            }
            if port.is_some() || ip.is_some() {
                let mut link = open(port, ip, transport::DEFAULT_BAUD)?;
                grbl::stream(link.as_mut(), gcode.lines(), &grbl::Control::default(), print_progress)?;
                eprintln!("\ndone");
            }
        }
    }
    Ok(())
}
