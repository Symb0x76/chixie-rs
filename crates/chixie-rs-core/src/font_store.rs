//! Local font library: fonts downloaded once (login needed) are kept on disk so the program
//! works offline afterwards. Stored next to `settings.toml` as `fonts/<id>.json` (the decrypted
//! glyph map, same shape as the server's plaintext) plus `fonts/index.json` (id → name).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::font::Font;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalFont {
    pub id: i64,
    pub name: String,
    pub glyphs: usize,
}

pub struct FontStore {
    dir: PathBuf,
}

impl FontStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `<settings dir>/fonts`.
    pub fn next_to(settings_path: &Path) -> Self {
        Self::new(settings_path.parent().unwrap_or_else(|| Path::new(".")).join("fonts"))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("index.json")
    }

    fn font_path(&self, id: i64) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Saved fonts, in the order they were added (a missing or broken index lists nothing).
    pub fn list(&self) -> Vec<LocalFont> {
        std::fs::read(self.index_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn contains(&self, id: i64) -> bool {
        self.list().iter().any(|f| f.id == id) && self.font_path(id).exists()
    }

    pub fn name_of(&self, id: i64) -> Option<String> {
        self.list().into_iter().find(|f| f.id == id).map(|f| f.name)
    }

    fn write_index(&self, list: &[LocalFont]) -> Result<()> {
        let tmp = self.dir.join("index.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(list).map_err(|e| Error::Font(e.to_string()))?)?;
        std::fs::rename(&tmp, self.index_path())?;
        Ok(())
    }

    /// Save (or replace) a font.
    pub fn save(&self, id: i64, name: &str, font: &Font) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!("{id}.json.tmp"));
        std::fs::write(&tmp, font.to_json())?;
        std::fs::rename(&tmp, self.font_path(id))?;
        let mut list = self.list();
        let entry = LocalFont { id, name: name.to_string(), glyphs: font.glyphs.len() };
        match list.iter_mut().find(|f| f.id == id) {
            Some(f) => *f = entry,
            None => list.push(entry),
        }
        self.write_index(&list)
    }

    /// Correct a saved font's name (it may have been saved before its name was known).
    /// Returns whether anything changed.
    pub fn rename(&self, id: i64, name: &str) -> Result<bool> {
        let mut list = self.list();
        let Some(f) = list.iter_mut().find(|f| f.id == id && f.name != name) else { return Ok(false) };
        f.name = name.to_string();
        self.write_index(&list)?;
        Ok(true)
    }

    pub fn load(&self, id: i64) -> Result<Font> {
        let bytes = std::fs::read(self.font_path(id)).map_err(|e| Error::Font(format!("本地字体 {id}: {e}")))?;
        Font::from_json(&bytes)
    }

    pub fn remove(&self, id: i64) -> Result<()> {
        let list: Vec<LocalFont> = self.list().into_iter().filter(|f| f.id != id).collect();
        let _ = std::fs::remove_file(self.font_path(id));
        if self.dir.exists() {
            self.write_index(&list)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Font {
        Font::from_json(include_bytes!("../tests/fixtures/font_fixture.json")).unwrap()
    }

    #[test]
    fn json_round_trip_keeps_every_glyph() {
        let f = fixture();
        let back = Font::from_json(&f.to_json()).unwrap();
        assert_eq!(back.glyphs.len(), f.glyphs.len());
        for (c, g) in &f.glyphs {
            let b = &back.glyphs[c];
            assert_eq!((b.advance, &b.strokes), (g.advance, &g.strokes), "glyph {c}");
        }
    }

    #[test]
    fn save_list_load_remove() {
        let dir = std::env::temp_dir().join(format!("chixie-font-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = FontStore::new(&dir);
        assert!(s.list().is_empty());
        s.save(10, "艾玛体", &fixture()).unwrap();
        s.save(11, "其他", &fixture()).unwrap();
        s.save(10, "艾玛体 v2", &fixture()).unwrap();
        assert_eq!(s.list().iter().map(|f| (f.id, f.name.as_str())).collect::<Vec<_>>(), vec![(10, "艾玛体 v2"), (11, "其他")]);
        assert!(s.contains(10));
        assert!(s.rename(11, "田英章").unwrap());
        assert!(!s.rename(11, "田英章").unwrap(), "same name is a no-op");
        assert!(!s.rename(99, "无").unwrap(), "unknown id is a no-op");
        assert_eq!(s.name_of(11).as_deref(), Some("田英章"));
        assert_eq!(s.load(10).unwrap().glyphs.len(), 4);
        s.remove(10).unwrap();
        assert!(!s.contains(10) && s.load(10).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
