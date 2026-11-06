use std::path::PathBuf;
use std::time::Duration;

use chixie_rs::{api, discovery, esp3d, grbl, transport, writing};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "ChiXie device control")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

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
    /// List your collected fonts (login via CHIXIE_ACCOUNT / CHIXIE_PASSWORD env)
    Fonts,
    /// Generate writing G-code from text; print to stdout or --out, optionally stream
    Write {
        text: String,
        #[arg(long)]
        font_id: Option<i64>,
        /// Offline: decode a font from a saved encrypted blob + key file instead of login
        #[arg(long, requires = "key_file")]
        blob_file: Option<PathBuf>,
        #[arg(long)]
        key_file: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        /// mm per glyph unit (character size); calibrate against a captured .gcode
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        #[arg(long)]
        port: Option<String>,
        #[arg(long)]
        ip: Option<String>,
    },
}

fn api_login() -> chixie_rs::Result<api::Client> {
    let account = std::env::var("CHIXIE_ACCOUNT")
        .map_err(|_| chixie_rs::Error::Api("set CHIXIE_ACCOUNT".into()))?;
    let password = std::env::var("CHIXIE_PASSWORD")
        .map_err(|_| chixie_rs::Error::Api("set CHIXIE_PASSWORD".into()))?;
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

fn open(port: Option<String>, ip: Option<String>, baud: u32) -> chixie_rs::Result<Box<dyn transport::Link>> {
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

fn main() -> chixie_rs::Result<()> {
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
            grbl::stream(link.as_mut(), text.lines(), &grbl::Control::default(), |d, t| {
                eprint!("\r{d}/{t}");
            })?;
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
        Cmd::Fonts => {
            let c = api_login()?;
            for f in c.collected_fonts(0)? {
                println!("{}\t{}\t{} chars", f.id, f.font_name, f.character_number);
            }
        }
        Cmd::Write { text, font_id, blob_file, key_file, out, scale, port, ip } => {
            let font = match (blob_file, key_file) {
                (Some(bf), Some(kf)) => {
                    let blob = std::fs::read_to_string(bf)?;
                    let key = std::fs::read_to_string(kf)?;
                    chixie_rs::font::Font::from_encrypted(blob.trim(), key.trim())?
                }
                _ => {
                    let id = font_id.ok_or_else(|| chixie_rs::Error::Api("--font-id required".into()))?;
                    let c = api_login()?;
                    c.font(id)?
                }
            };
            eprintln!("font: {} glyphs", font.glyphs.len());
            let params = writing::WritingParams { mm_per_unit: scale, ..Default::default() };
            let gcode = writing::text_to_gcode(&text, &font, &params);
            match &out {
                Some(path) => std::fs::write(path, &gcode)?,
                None if port.is_none() && ip.is_none() => print!("{gcode}"),
                None => {}
            }
            if port.is_some() || ip.is_some() {
                let mut link = open(port, ip, transport::DEFAULT_BAUD)?;
                grbl::stream(link.as_mut(), gcode.lines(), &grbl::Control::default(), |d, t| {
                    eprint!("\r{d}/{t}");
                })?;
                eprintln!("\ndone");
            }
        }
    }
    Ok(())
}
