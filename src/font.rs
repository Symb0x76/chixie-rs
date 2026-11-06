//! ChiXie stroke fonts: decrypt the `get_font_access` payload and parse glyph strokes.
//!
//! Payload (see docs/server-api.md): `Fernet(key).decrypt(blob)` → JSON
//! `{ "<char>": [advance_width, stroke, stroke, ...] }` where
//! `stroke = [[x0,x1,...],[y0,y1,...]]`. Coordinates are a small float grid
//! (cell ~0..7 units, y increasing downward); pen is down along a stroke, up between strokes.

use std::collections::HashMap;

use crate::{Error, Result};

/// One point in glyph units (y downward).
pub type Point = (f32, f32);

#[derive(Debug, Clone)]
pub struct Glyph {
    /// Horizontal advance after this character, in glyph units.
    pub advance: f32,
    /// Each stroke is one pen-down polyline.
    pub strokes: Vec<Vec<Point>>,
}

#[derive(Debug, Default)]
pub struct Font {
    pub glyphs: HashMap<char, Glyph>,
}

impl Font {
    pub fn get(&self, c: char) -> Option<&Glyph> {
        self.glyphs.get(&c)
    }

    /// Build the Fernet key from the login `backup_token`: the client swaps the last two chars.
    pub fn key_from_backup_token(backup_token: &str) -> String {
        let mut c: Vec<char> = backup_token.chars().collect();
        let n = c.len();
        if n >= 2 {
            c.swap(n - 1, n - 2);
        }
        c.into_iter().collect()
    }

    /// Decrypt a raw `get_font_access` payload with the derived key, then parse.
    pub fn from_encrypted(blob: &str, key: &str) -> Result<Self> {
        let f = fernet::Fernet::new(key).ok_or_else(|| Error::Font("bad fernet key".into()))?;
        let plain = f
            .decrypt(blob)
            .map_err(|_| Error::Font("fernet decrypt failed".into()))?;
        Self::from_json(&plain)
    }

    /// Parse the decrypted UTF-8 JSON glyph map.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        // Each value is [advance, stroke, stroke, ...]; a stroke is [xs, ys].
        // Use serde_json::Value so a malformed glyph can be skipped, not fail the whole font.
        let raw: HashMap<String, Vec<serde_json::Value>> =
            serde_json::from_slice(bytes).map_err(|e| Error::Font(format!("json: {e}")))?;
        let mut glyphs = HashMap::with_capacity(raw.len());
        for (k, parts) in raw {
            let Some(c) = one_char(&k) else { continue };
            let Some(glyph) = parse_glyph(&parts) else { continue };
            glyphs.insert(c, glyph);
        }
        Ok(Font { glyphs })
    }
}

fn one_char(s: &str) -> Option<char> {
    let mut it = s.chars();
    let c = it.next()?;
    it.next().is_none().then_some(c)
}

fn parse_glyph(parts: &[serde_json::Value]) -> Option<Glyph> {
    let advance = parts.first()?.as_f64()? as f32;
    let mut strokes = Vec::with_capacity(parts.len().saturating_sub(1));
    for s in &parts[1..] {
        if let Some(pts) = parse_stroke(s).filter(|p| !p.is_empty()) {
            strokes.push(pts);
        }
    }
    Some(Glyph { advance, strokes })
}

/// `[xs, ys]` → points; `None` if the shape is wrong so the caller can skip just this stroke.
fn parse_stroke(s: &serde_json::Value) -> Option<Vec<Point>> {
    let arr = s.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    let xs = arr[0].as_array()?;
    let ys = arr[1].as_array()?;
    let n = xs.len().min(ys.len());
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        pts.push((xs[i].as_f64()? as f32, ys[i].as_f64()? as f32));
    }
    Some(pts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_swaps_last_two() {
        assert_eq!(Font::key_from_backup_token("abcd=i"), "abcdi=");
    }

    #[test]
    fn parses_glyph_map_and_skips_garbage() {
        let json = br#"{
            "A": [2.5, [[0.0,1.0],[7.0,0.0]], [[1.0],[3.0]]],
            "multi": [1.0, [[0.0],[0.0]]],
            "B": [1.0, "oops"]
        }"#;
        let f = Font::from_json(json).unwrap();
        assert_eq!(f.glyphs.len(), 2); // "multi" key skipped, "A" and "B" kept
        let a = f.get('A').unwrap();
        assert_eq!(a.advance, 2.5);
        assert_eq!(a.strokes.len(), 2);
        assert_eq!(a.strokes[0], vec![(0.0, 7.0), (1.0, 0.0)]);
        assert!(f.get('B').unwrap().strokes.is_empty()); // bad stroke dropped, glyph kept
    }
}
