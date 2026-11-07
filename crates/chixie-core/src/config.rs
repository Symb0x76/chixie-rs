//! Persistent settings (`%APPDATA%\chixie-rs\config\settings.toml` on Windows) and a one-shot
//! importer for ChiXie3's `config.ini`.
//!
//! Secrets never go here: the account is stored, the password lives in the OS credential store
//! (`credentials.rs`). The importer deliberately ignores ChiXie3's `[LoginConfig]`, which holds the
//! password in plaintext.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::gcode::DeviceProfile;
use crate::handwriting::{Span, StyleFile};
use crate::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Last login account (phone/email). Password: see `credentials`.
    pub account: Option<String>,
    pub remember_password: bool,
    pub device: DeviceSettings,
    pub page: PageSettings,
    pub style: StyleFile,
    pub ui: UiSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceSettings {
    pub profile: DeviceProfile,
    pub baud: u32,
    pub last_port: Option<String>,
    pub last_ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PageSettings {
    pub font_id: Option<i64>,
    /// ChiXie3 font size; mm per glyph unit = size / 100.
    pub font_size: f32,
    /// Baseline-to-baseline distance, mm (≈10.25 measured in ChiXie3's default template).
    pub line_height: f32,
    /// Paper size in mm (portrait width × height).
    pub paper: (f32, f32),
    /// Margins in mm: left, top, right, bottom.
    pub margins: (f32, f32, f32, f32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub dark: bool,
    pub locale: String,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self { profile: DeviceProfile::default(), baud: crate::transport::DEFAULT_BAUD, last_port: None, last_ip: None }
    }
}

impl Default for PageSettings {
    fn default() -> Self {
        // A4 with 21 mm side margins matches the ≈168 mm writable width seen in captures.
        Self { font_id: None, font_size: 100.0, line_height: 10.25, paper: (210.0, 297.0), margins: (21.0, 20.0, 21.0, 20.0) }
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self { dark: false, locale: "zh-CN".into() }
    }
}

impl PageSettings {
    /// Writable area (paper minus margins), mm.
    pub fn writable(&self) -> (f32, f32) {
        let (w, h) = self.paper;
        let (l, t, r, b) = self.margins;
        ((w - l - r).max(1.0), (h - t - b).max(1.0))
    }
}

impl Settings {
    /// Layout parameters for the current page and style. Origin (0,0) is the top-left of the
    /// writable area = the pen's start position, Y negative going down the page, matching what
    /// ChiXie3 streams (docs/protocol.md).
    pub fn page_params(&self) -> crate::layout::PageParams {
        let (width, height) = self.page.writable();
        crate::layout::PageParams {
            mm_per_unit: crate::calibration::mm_per_unit_for_font_size(self.page.font_size),
            line_height: self.page.line_height,
            origin: (0.0, 0.0),
            page: Some(crate::layout::PageBox { width, height }),
            style: self.style.handwriting.clone(),
            deformation: self.style.deformation.clone(),
            ..Default::default()
        }
    }

    /// Default location: `<config dir>/chixie-rs/.../settings.toml`.
    pub fn default_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "", "chixie-rs")
            .ok_or_else(|| Error::Config("no home directory".into()))?;
        Ok(dirs.config_dir().join("settings.toml"))
    }

    /// Load settings; a missing file yields defaults. Unknown keys are ignored and missing keys
    /// take defaults, so old files keep working as fields are added.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| Error::Config(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Write atomically (temp file + rename) so a crash never leaves a half-written file.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| Error::Config(e.to_string()))?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// Import handwriting and deformation settings from ChiXie3's `config.ini`
/// (`%USERPROFILE%\ChiXie3\conf\config.ini`). Only plain keys in `[WriteConfig]` and
/// `[CharacterDeformation]` are read; `[LoginConfig]` and QVariant blobs are ignored.
pub fn import_chixie3(ini_text: &str) -> StyleFile {
    let ini = parse_ini(ini_text);
    let get = |sec: &str, key: &str| ini.get(&(sec.to_string(), key.to_string())).map(String::as_str);
    let num = |sec: &str, key: &str| get(sec, key).and_then(|v| v.parse::<f32>().ok());
    let flag = |sec: &str, key: &str| get(sec, key).map(|v| v.eq_ignore_ascii_case("true"));
    let span = |sec: &str, min: &str, max: &str, default: Span| match (num(sec, min), num(sec, max)) {
        (Some(a), Some(b)) => Span::new(a, b),
        _ => default,
    };

    let mut s = StyleFile::default();
    let (w, h) = ("WriteConfig", &mut s.handwriting);
    // ChiXie3 always applies its jitter in the default writing mode (captures).
    h.enabled = true;
    h.char_spacing_zh = span(w, "min_char_spacing_zh", "max_char_spacing_zh", h.char_spacing_zh);
    h.char_spacing_en = span(w, "min_char_spacing_en", "max_char_spacing_en", h.char_spacing_en);
    h.space_width_zh = span(w, "min_space_width_zh", "max_space_width_zh", h.space_width_zh);
    h.space_width_en = span(w, "min_space_width_en", "max_space_width_en", h.space_width_en);
    h.vertical_offset = span(w, "min_vertical_offset", "max_vertical_offset", h.vertical_offset);
    if let Some(v) = flag(w, "intelligent_vertical_offset") {
        h.intelligent_vertical_offset = v;
    }
    if let Some(v) = flag(w, "random_correct_offset") {
        h.random_correct_offset = v;
    }
    match (flag(w, "first_text_not_align"), num(w, "first_text_not_align_range")) {
        (Some(false), _) => h.line_start_offset = Span::new(0.0, 0.0),
        (_, Some(r)) => h.line_start_offset = Span::new(0.0, r),
        _ => {}
    }
    // min/max_first_line_indent are present in ChiXie3 but were not applied in captures of the
    // default mode, so they are not imported (first_line_indent stays None). VERIFY.

    let (c, d) = ("CharacterDeformation", &mut s.deformation);
    if let Some(v) = flag(c, "EnableCharacterDeformation") {
        d.enabled = v;
    }
    if let Some(v) = flag(c, "enable_base_deform") {
        d.base = v;
    }
    if let Some(v) = flag(c, "enable_shape_deform") {
        d.shape = v;
    }
    d.h_scale_pct = span(c, "min_horizontal_scale", "max_horizontal_scale", d.h_scale_pct);
    d.v_scale_pct = span(c, "min_vertical_scale", "max_vertical_scale", d.v_scale_pct);
    d.rotation_deg = span(c, "min_rotation_angle", "max_rotation_angle", d.rotation_deg);
    if let Some(v) = num(c, "stroke_deform_degree") {
        d.stroke_degree = v;
    }
    s
}

/// Minimal INI reader for Qt `QSettings` files: `[Section]` headers and `key=value` lines.
/// Values are trimmed; surrounding double quotes are removed. Sections other than the two the
/// importer needs are skipped entirely (so `[LoginConfig]` values are never even stored).
fn parse_ini(text: &str) -> HashMap<(String, String), String> {
    const WANTED: [&str; 2] = ["WriteConfig", "CharacterDeformation"];
    let mut out = HashMap::new();
    let mut section: Option<&str> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = WANTED.into_iter().find(|w| *w == name);
            continue;
        }
        let (Some(sec), Some((k, v))) = (section, line.split_once('=')) else { continue };
        let v = v.trim();
        let v = v.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(v);
        out.insert((sec.to_string(), k.trim().to_string()), v.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape of a real ChiXie3 config.ini (values from the verified install; login is fake).
    const INI: &str = r#"[LoginConfig]
Account=10000000000
Password=not-a-real-password
RememberPassword=true

[WriteConfig]
min_char_spacing_zh=0.6
max_char_spacing_zh=1.1
min_char_spacing_en=0.5
max_char_spacing_en=1.0
min_space_width_zh=5.0
max_space_width_zh=7.0
min_space_width_en=2.0
max_space_width_en=3.0
min_vertical_offset=0.0
max_vertical_offset=0.3
row_offset_style=1
random_correct_offset=false
min_first_line_indent=15.5
max_first_line_indent=17.0
intelligent_vertical_offset=false
first_text_not_align=true
first_text_not_align_range=2.0

[DeviceConfig]
DeviceParams=@Variant(\0\0\0\b\0\0\0\x1)

[CharacterDeformation]
EnableCharacterDeformation=true
enable_base_deform=true
enable_shape_deform=false
min_horizontal_scale=95
max_horizontal_scale=105
min_vertical_scale=96
max_vertical_scale=104
min_rotation_angle=-3
max_rotation_angle=3
stroke_deform_degree=1
"#;

    #[test]
    fn imports_write_and_deformation_keys() {
        let s = import_chixie3(INI);
        let h = &s.handwriting;
        assert!(h.enabled);
        assert_eq!(h.char_spacing_zh, Span::new(0.6, 1.1));
        assert_eq!(h.space_width_en, Span::new(2.0, 3.0));
        assert_eq!(h.vertical_offset, Span::new(0.0, 0.3));
        assert_eq!(h.line_start_offset, Span::new(0.0, 2.0));
        assert_eq!(h.first_line_indent, None);
        let d = &s.deformation;
        assert!(d.enabled && d.base && !d.shape);
        assert_eq!(d.h_scale_pct, Span::new(95.0, 105.0));
        assert_eq!(d.rotation_deg, Span::new(-3.0, 3.0));
        assert_eq!(d.stroke_degree, 1.0);
    }

    #[test]
    fn login_section_is_never_read() {
        let ini = parse_ini(INI);
        assert!(ini.keys().all(|(sec, _)| sec != "LoginConfig"));
        assert!(!ini.values().any(|v| v.contains("not-a-real-password")));
    }

    #[test]
    fn first_text_not_align_off_disables_line_start_offset() {
        let s = import_chixie3("[WriteConfig]\nfirst_text_not_align=false\nfirst_text_not_align_range=2.0\n");
        assert_eq!(s.handwriting.line_start_offset, Span::new(0.0, 0.0));
    }

    #[test]
    fn settings_roundtrip_and_missing_file() {
        let dir = std::env::temp_dir().join(format!("chixie-cfg-{}", std::process::id()));
        let path = dir.join("settings.toml");
        assert_eq!(Settings::load(&path).unwrap(), Settings::default());
        let mut s = Settings { account: Some("13800000000".into()), remember_password: true, ..Default::default() };
        s.device.last_port = Some("COM4".into());
        s.style = import_chixie3(INI);
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), s);
        let text = std::fs::read_to_string(&path).unwrap();
        let has_password_key = text.lines().any(|l| l.trim_start().to_lowercase().starts_with("password"));
        assert!(!has_password_key, "no password field may be persisted:\n{text}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn page_params_follow_settings() {
        let mut s = Settings::default();
        s.page.font_size = 50.0;
        s.style.handwriting.enabled = true;
        let p = s.page_params();
        assert_eq!(p.mm_per_unit, 0.5);
        assert_eq!(p.page, Some(crate::layout::PageBox { width: 168.0, height: 257.0 }));
        assert!(p.style.enabled);
        // Landscape = swapped paper.
        s.page.paper = (297.0, 210.0);
        assert_eq!(s.page_params().page.unwrap().width, 255.0);
    }

    #[test]
    fn partial_files_fill_defaults() {
        let s: Settings = toml::from_str("account = \"x\"\n[page]\nfont_size = 80.0\n").unwrap();
        assert_eq!(s.account.as_deref(), Some("x"));
        assert_eq!(s.page.font_size, 80.0);
        assert_eq!(s.page.line_height, 10.25);
    }
}
