use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub use chixie_rs_core::config::DEFAULT_BAUD;
use chixie_rs_core::{Error, Result};

pub const DEFAULT_TELNET_PORT: u16 = 23;

/// USB-serial chips the old app auto-selects.
const KNOWN_PORT_DESCRIPTIONS: [&str; 2] = ["USB-SERIAL CH340", "GD32 Virtual Com Port"];

/// Line-oriented byte link to a controller.
pub trait Link {
    fn write_all(&mut self, data: &[u8]) -> Result<()>;
    /// Read one line (without terminator). `Ok(None)` on timeout.
    fn read_line(&mut self) -> Result<Option<String>>;
}

pub struct LineLink<S: Read + Write> {
    reader: BufReader<S>,
    buf: Vec<u8>,
}

impl<S: Read + Write> LineLink<S> {
    pub fn new(stream: S) -> Self {
        Self { reader: BufReader::new(stream), buf: Vec::new() }
    }
}

impl<S: Read + Write> Link for LineLink<S> {
    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        let s = self.reader.get_mut();
        s.write_all(data)?;
        s.flush()?;
        Ok(())
    }

    fn read_line(&mut self) -> Result<Option<String>> {
        // Partial data stays in `buf` across timeouts.
        match self.reader.read_until(b'\n', &mut self.buf) {
            Ok(_) if self.buf.ends_with(b"\n") => {
                let line = String::from_utf8_lossy(&self.buf).trim().to_string();
                self.buf.clear();
                Ok(Some(line))
            }
            Ok(0) => Err(Error::Io(std::io::ErrorKind::UnexpectedEof.into())),
            Ok(_) => Ok(None),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) => {
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }
}

pub type SerialLink = LineLink<Box<dyn serialport::SerialPort>>;
pub type TelnetLink = LineLink<TcpStream>;

pub fn open_serial(port: &str, baud: u32) -> Result<SerialLink> {
    let p = serialport::new(port, baud).timeout(Duration::from_millis(100)).open()?;
    Ok(LineLink::new(p))
}

pub fn open_telnet(ip: &str, port: u16) -> Result<TelnetLink> {
    let s = TcpStream::connect((ip, port))?;
    s.set_read_timeout(Some(Duration::from_millis(100)))?;
    s.set_nodelay(true)?;
    Ok(LineLink::new(s))
}

#[derive(Debug, Clone)]
pub struct PortInfo {
    pub name: String,
    pub description: String,
    pub known_device: bool,
}

pub fn list_ports() -> Result<Vec<PortInfo>> {
    Ok(serialport::available_ports()?
        .into_iter()
        .map(|p| {
            let description = match &p.port_type {
                serialport::SerialPortType::UsbPort(u) => u.product.clone().unwrap_or_default(),
                _ => String::new(),
            };
            let known_device = KNOWN_PORT_DESCRIPTIONS.iter().any(|k| description.contains(k));
            PortInfo { name: p.port_name, description, known_device }
        })
        .collect())
}
