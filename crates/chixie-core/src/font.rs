//! ChiXie stroke fonts: decrypt the `get_font_access` payload and parse glyph strokes.
//!
//! Payload (see docs/server-api.md): `Fernet(key).decrypt(blob)` → JSON
//! `{ "<char>": [advance_width, stroke, stroke, ...] }` where
//! `stroke = [[x0,x1,...],[y0,y1,...]]`. Coordinates are a small float grid
//! (cell ~0..7 units, y increasing downward); pen is down along a stroke, up between strokes.

use std::collections::HashMap;

use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::{Error, Result};

/// Decode urlsafe base64 the way Python's `urlsafe_b64decode` does for these payloads:
/// the server appends junk after the `=` padding, so cut at the first `=` (a base64 data
/// terminator) and decode up to it. Without a `=`, decode unpadded.
fn b64_url(s: &str) -> Option<Vec<u8>> {
    let t = s.trim();
    match t.find('=') {
        Some(i) => {
            // Keep the whole padding run (one or two '='); junk follows it.
            let b = t.as_bytes();
            let mut j = i;
            while b.get(j) == Some(&b'=') {
                j += 1;
            }
            base64::engine::general_purpose::URL_SAFE.decode(&t[..j]).ok()
        }
        None => base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(t).ok(),
    }
}

/// Decrypt a Fernet token manually: version → HMAC-SHA256 verify → AES-128-CBC/PKCS7.
/// Timestamp is intentionally ignored (the server clock may run ahead of this machine).
/// Stage-specific errors make failures diagnosable.
pub fn fernet_decrypt(token: &str, key: &str) -> Result<Vec<u8>> {
    let kb = b64_url(key).ok_or_else(|| Error::Font("key: bad base64".into()))?;
    if kb.len() != 32 {
        return Err(Error::Font(format!("key: {} bytes, need 32", kb.len())));
    }
    let tb = b64_url(token).ok_or_else(|| Error::Font("token: bad base64".into()))?;
    if tb.len() < 1 + 8 + 16 + 32 {
        return Err(Error::Font(format!("token: too short ({} bytes)", tb.len())));
    }
    if tb[0] != 0x80 {
        return Err(Error::Font(format!("token: version {:#x}, need 0x80", tb[0])));
    }
    let (sign_key, enc_key) = kb.split_at(16);
    let hmac_at = tb.len() - 32;

    let mut mac = Hmac::<Sha256>::new_from_slice(sign_key).expect("hmac any key len");
    mac.update(&tb[..hmac_at]);
    mac.verify_slice(&tb[hmac_at..])
        .map_err(|_| Error::Font("hmac mismatch (wrong key)".into()))?;

    let iv: [u8; 16] = tb[9..25].try_into().unwrap();
    let mut buf = tb[25..hmac_at].to_vec();
    if buf.is_empty() || buf.len() % 16 != 0 {
        return Err(Error::Font(format!("ciphertext not block-aligned ({})", buf.len())));
    }
    type Dec = cbc::Decryptor<aes::Aes128>;
    let pt = Dec::new(enc_key.into(), (&iv).into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| Error::Font("aes/pkcs7 unpad failed".into()))?;
    Ok(pt.to_vec())
}

/// Non-secret description of a key token and font blob for decrypt-failure reports: lengths,
/// where `=` sits, and which characters fall outside the urlsafe-base64 alphabet. Never the
/// characters themselves.
pub fn shape(backup_token: &str, blob: &str) -> String {
    let eq = |s: &str| s.char_indices().filter(|(_, c)| *c == '=').map(|(i, _)| i).collect::<Vec<_>>();
    let odd = |s: &str| {
        let mut v: Vec<char> = s.chars().filter(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '='))).collect();
        v.sort_unstable();
        v.dedup();
        v.into_iter().map(|c| format!("{:?}", c)).collect::<Vec<_>>().join(" ")
    };
    let tail_eq: Vec<usize> = eq(blob).into_iter().filter(|i| *i + 8 >= blob.len()).map(|i| blob.len() - i).collect();
    format!(
        "backup_token: {} chars, '=' at {:?}, non-b64 [{}]; blob: {} chars (mod 4 = {}), starts {:?}, '=' from end {:?}, non-b64 [{}]",
        backup_token.chars().count(),
        eq(backup_token),
        odd(backup_token),
        blob.len(),
        blob.len() % 4,
        blob.get(..4).unwrap_or(""),
        tail_eq,
        odd(blob),
    )
}

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

    /// Candidate Fernet keys derived from the login `backup_token`. The server ships the
    /// 44-char urlsafe-b64 key with its `=` padding shifted off the end (obfuscation); the
    /// client restores it. Observed fix is swapping the last two chars, but the exact
    /// position can vary, so try a few normalizations and let HMAC pick the right one.
    pub fn key_candidates(backup_token: &str) -> Vec<String> {
        let mut out = Vec::new();
        let c: Vec<char> = backup_token.chars().collect();
        let n = c.len();
        if n >= 2 {
            let mut swapped = c.clone();
            swapped.swap(n - 1, n - 2);
            out.push(swapped.into_iter().collect());
        }
        // Move a misplaced '=' to the end (handles padding anywhere near the tail).
        if backup_token.contains('=') {
            out.push(format!("{}=", backup_token.replace('=', "")));
        }
        out.push(backup_token.to_string()); // raw, in case no obfuscation
        out
    }

    /// Back-compat: the first (most likely) key candidate.
    pub fn key_from_backup_token(backup_token: &str) -> String {
        Self::key_candidates(backup_token).into_iter().next().unwrap_or_default()
    }

    /// Decrypt a raw `get_font_access` payload with the derived key, then parse.
    pub fn from_encrypted(blob: &str, key: &str) -> Result<Self> {
        let plain = fernet_decrypt(blob, key)?;
        Self::from_json(&plain)
    }

    /// Decrypt by trying each key candidate derived from the backup_token.
    pub fn from_encrypted_with_backup_token(blob: &str, backup_token: &str) -> Result<Self> {
        let mut why = Vec::new();
        for (i, key) in Self::key_candidates(backup_token).into_iter().enumerate() {
            match fernet_decrypt(blob, &key) {
                Ok(plain) => {
                    log::debug!("font decrypt ok with key candidate {}", i + 1);
                    return Self::from_json(&plain);
                }
                Err(e) => why.push(format!("#{}: {e}", i + 1)),
            }
        }
        Err(Error::Font(format!("fernet decrypt failed [{}]; {}", why.join("; "), shape(backup_token, blob))))
    }

    /// The glyph map in the same JSON shape `from_json` reads (`{"字": [advance, [xs…], [ys…]]}`,
    /// one inner array per stroke), for the local font store.
    pub fn to_json(&self) -> Vec<u8> {
        let mut map = serde_json::Map::with_capacity(self.glyphs.len());
        for (c, g) in &self.glyphs {
            let xs: Vec<Vec<f32>> = g.strokes.iter().map(|s| s.iter().map(|p| p.0).collect()).collect();
            let ys: Vec<Vec<f32>> = g.strokes.iter().map(|s| s.iter().map(|p| p.1).collect()).collect();
            map.insert(c.to_string(), serde_json::json!([g.advance, xs, ys]));
        }
        serde_json::to_vec(&map).expect("glyph map serialises")
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

/// Glyph JSON is `[advance, X, Y]`. X and Y are parallel coordinate containers:
/// either flat number arrays (one stroke) or arrays-of-arrays (one entry per stroke).
fn parse_glyph(parts: &[serde_json::Value]) -> Option<Glyph> {
    let advance = parts.first()?.as_f64()? as f32;
    let (Some(x), Some(y)) = (parts.get(1), parts.get(2)) else {
        return Some(Glyph { advance, strokes: vec![] });
    };
    let (xa, ya) = (x.as_array()?, y.as_array()?);
    let mut strokes = Vec::new();
    match xa.first() {
        // Flat: X = [x0, x1, ...]  -> a single polyline.
        Some(v) if v.is_number() => {
            if let Some(s) = zip_points(xa, ya) {
                strokes.push(s);
            }
        }
        // Nested: X = [[..],[..]], one inner array per stroke.
        Some(_) => {
            for (xi, yi) in xa.iter().zip(ya.iter()) {
                if let (Some(xs), Some(ys)) = (xi.as_array(), yi.as_array())
                    && let Some(s) = zip_points(xs, ys).filter(|p| !p.is_empty())
                {
                    strokes.push(s);
                }
            }
        }
        None => {}
    }
    Some(Glyph { advance, strokes })
}

/// Pair two coordinate arrays into points, truncating to the shorter.
fn zip_points(xs: &[serde_json::Value], ys: &[serde_json::Value]) -> Option<Vec<Point>> {
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
    fn decrypts_python_fernet_token() {
        // Generated by pyca/cryptography Fernet; proves the Rust crate is cross-compatible.
        let key = "eYATO4_NrVMGe-hK54eMCMBPlyqXwQEOBAoQNQE7rY0=";
        let token = "gAAAAABq7Z1-Iyw8Zr0Cwd-oMu7e7s3t8acrzEafFSdLadD4XwXcKbiObWfMwzLOOSMMu8XxprTcoJzi8XcOIv_Wq_E3otmU2g==";
        let plain = fernet_decrypt(token, key).expect("decrypt python token");
        assert_eq!(String::from_utf8(plain).unwrap(), "héllo-stroke");
    }

    #[test]
    fn parses_flat_and_nested_glyphs() {
        // I: flat single stroke (X,Y are number arrays). A: nested, two strokes.
        let json = br#"{
            "I": [1.0, [0.0, 0.0], [0.0, 7.0]],
            "A": [2.5, [[0.0,1.0],[2.0]], [[7.0,0.0],[3.0]]],
            "two": [1.0, [0.0], [0.0]]
        }"#;
        let f = Font::from_json(json).unwrap();
        assert_eq!(f.glyphs.len(), 2); // multi-char key "two" skipped
        let i = f.get('I').unwrap();
        assert_eq!(i.strokes.len(), 1);
        assert_eq!(i.strokes[0], vec![(0.0, 0.0), (0.0, 7.0)]);
        let a = f.get('A').unwrap();
        assert_eq!(a.advance, 2.5);
        assert_eq!(a.strokes.len(), 2);
        assert_eq!(a.strokes[0], vec![(0.0, 7.0), (1.0, 0.0)]);
        assert_eq!(a.strokes[1], vec![(2.0, 3.0)]);
    }
}
