//! HTTP API of the Grbl_ESP32 web server on the wireless board.

use std::path::Path;
use std::time::Duration;

use reqwest::blocking::{Client, multipart};

use chixie_rs_core::Result;

/// SD folder the old app uses for tasks.
pub const TASK_DIR: &str = "/chixie_task";

pub struct Board {
    base: String,
    http: Client,
}

impl Board {
    pub fn new(ip: &str) -> Result<Self> {
        let http = Client::builder().timeout(Duration::from_secs(10)).build()?;
        Ok(Self { base: format!("http://{ip}"), http })
    }

    /// `GET /command?commandText=<cmd>&PAGEID=0`
    pub fn command(&self, cmd: &str) -> Result<String> {
        let url = format!("{}/command", self.base);
        Ok(self
            .http
            .get(url)
            .query(&[("commandText", cmd), ("PAGEID", "0")])
            .send()?
            .error_for_status()?
            .text()?)
    }

    pub fn status(&self) -> Result<String> { self.command("?") }
    pub fn hold(&self) -> Result<String> { self.command("!") }
    pub fn resume(&self) -> Result<String> { self.command("~") }
    pub fn unlock(&self) -> Result<String> { self.command("$X") }
    pub fn soft_reset(&self) -> Result<String> { self.command("\u{18}") }

    /// Run a G-code file stored on the SD card.
    pub fn run_sd_file(&self, path: &str) -> Result<String> {
        self.command(&format!("[ESP220]{path}"))
    }

    /// `GET /upload?path=<dir>[&action=..&filename=..]` → raw JSON text.
    fn upload_get(&self, dir: &str, action: Option<(&str, &str)>) -> Result<String> {
        let mut q = vec![("path", dir)];
        if let Some((a, f)) = action {
            q.push(("action", a));
            q.push(("filename", f));
        }
        Ok(self
            .http
            .get(format!("{}/upload", self.base))
            .query(&q)
            .send()?
            .error_for_status()?
            .text()?)
    }

    pub fn list(&self, dir: &str) -> Result<String> { self.upload_get(dir, None) }
    pub fn delete(&self, dir: &str, name: &str) -> Result<String> { self.upload_get(dir, Some(("delete", name))) }
    pub fn delete_dir(&self, dir: &str, name: &str) -> Result<String> { self.upload_get(dir, Some(("deletedir", name))) }
    pub fn create_dir(&self, dir: &str, name: &str) -> Result<String> { self.upload_get(dir, Some(("createdir", name))) }

    /// Multipart upload, field `myfile[]`, file stored as `<dir>/<name>`.
    pub fn upload(&self, dir: &str, name: &str, data: Vec<u8>) -> Result<String> {
        let target = format!("{}/{}", dir.trim_end_matches('/'), name);
        let part = multipart::Part::bytes(data).file_name(target);
        let form = multipart::Form::new().part("myfile[]", part);
        Ok(self
            .http
            .post(format!("{}/upload", self.base))
            .query(&[("path", dir)])
            .multipart(form)
            .send()?
            .error_for_status()?
            .text()?)
    }

    pub fn upload_file(&self, dir: &str, local: &Path) -> Result<String> {
        let name = local.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.upload(dir, &name, std::fs::read(local)?)
    }
}
