//! Documents on disk: `.chx` (JSON: text + per-character ids + handwriting seed, so a reopened
//! document keeps exactly the same handwriting), plain `.txt` import, the autosaved draft, and
//! G-code export (one file per page).

use std::path::{Path, PathBuf};

use sast_writer_core::editor::EditDoc;
use sast_writer_core::gcode::{self, DeviceProfile};
use sast_writer_core::layout::Document;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ChxFile {
    version: u32,
    text: String,
    ids: Vec<u64>,
    seed: u64,
}

pub struct Loaded {
    pub doc: EditDoc,
    /// Handwriting seed stored with a `.chx`; `None` for `.txt`.
    pub seed: Option<u64>,
}

pub fn save_chx(path: &Path, doc: &EditDoc, seed: u64) -> std::io::Result<()> {
    let f = ChxFile { version: 1, text: doc.text(), ids: doc.ids().to_vec(), seed };
    let json = serde_json::to_vec_pretty(&f).map_err(std::io::Error::other)?;
    // Write-then-rename so a crash never leaves a half-written document.
    let tmp = path.with_extension("chx.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

/// Open `.chx` (ids + seed kept) or anything else as UTF-8 text (fresh ids).
pub fn open(path: &Path) -> std::io::Result<Loaded> {
    let bytes = std::fs::read(path)?;
    let is_chx = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("chx"));
    if is_chx {
        let f: ChxFile = serde_json::from_slice(&bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let chars: Vec<char> = f.text.chars().collect();
        let doc = if f.ids.len() == chars.len() { EditDoc::from_parts(chars, f.ids) } else { EditDoc::from_text(&f.text) };
        return Ok(Loaded { doc, seed: Some(f.seed) });
    }
    let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
    Ok(Loaded { doc: EditDoc::from_text(text.trim_start_matches('\u{feff}')), seed: None })
}

pub fn draft_path(settings_path: &Path) -> PathBuf {
    settings_path.parent().unwrap_or_else(|| Path::new(".")).join("draft.chx")
}

pub fn documents_dir() -> PathBuf {
    std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Documents")).unwrap_or_else(|| PathBuf::from("."))
}

/// Write one G-code file per page: `name.gcode` for a single page, `name_p1.gcode`, … otherwise
/// (each page is a separate sheet). Returns the written paths.
pub fn export_files(doc: &Document, profile: &DeviceProfile, path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("chixie").to_string();
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut out = Vec::new();
    for (i, page) in doc.pages.iter().enumerate() {
        let p = if doc.pages.len() == 1 { path.to_path_buf() } else { dir.join(format!("{stem}_p{}.gcode", i + 1)) };
        std::fs::write(&p, gcode::emit(page, profile))?;
        out.push(p);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chx_round_trip_keeps_ids_and_seed() {
        let dir = std::env::temp_dir().join("sast-writer-gui-test-chx");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.chx");
        let mut d = EditDoc::from_text("你好");
        d.move_to(0, false);
        d.insert("啊"); // id 2 at the front: ids are no longer 0..n
        save_chx(&p, &d, 42).unwrap();
        let l = open(&p).unwrap();
        assert_eq!(l.doc.text(), "啊你好");
        assert_eq!(l.doc.ids(), d.ids());
        assert_eq!(l.seed, Some(42));
    }

    #[test]
    fn txt_opens_with_fresh_ids() {
        let dir = std::env::temp_dir().join("sast-writer-gui-test-chx");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("b.txt");
        std::fs::write(&p, "\u{feff}a\r\nb").unwrap();
        let l = open(&p).unwrap();
        assert_eq!(l.doc.text(), "a\nb");
        assert_eq!(l.seed, None);
    }
}
