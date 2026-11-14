//! chixie-rs core: documents, layout, G-code, fonts, backend API and settings.
//! Protocol notes: `docs/protocol.md`.

pub mod api;
pub mod calibration;
pub mod chixie3_devices;
pub mod chixie3_templates;
pub mod config;
pub mod credentials;
pub mod deformation;
pub mod editor;
pub mod font;
pub mod font_store;
pub mod gcode;
pub mod handwriting;
pub mod layout;
pub mod session;
pub mod template;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serial: {0}")]
    Serial(#[from] serialport::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("grbl error on line {line}: {msg}")]
    Grbl { line: usize, msg: String },
    #[error("grbl alarm: {0}")]
    Alarm(String),
    #[error("font: {0}")]
    Font(String),
    #[error("api: {0}")]
    Api(String),
    #[error("credential store: {0}")]
    Credential(String),
    #[error("config: {0}")]
    Config(String),
    #[error("timeout waiting for device")]
    Timeout,
    #[error("cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;
