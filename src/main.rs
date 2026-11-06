use std::path::PathBuf;
use std::time::Duration;

use chixie_rs::{discovery, esp3d, grbl, transport};
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
    }
    Ok(())
}
