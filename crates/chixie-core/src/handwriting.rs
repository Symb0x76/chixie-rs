//! Handwriting jitter: per-character spacing, space width, vertical offset and per-line start
//! offset, drawn from a seeded RNG so the same seed always produces the same `Document`.
//!
//! Semantics come from ChiXie3 captures (docs/protocol.md, "Handwriting effects" and
//! "Multi-line layout"):
//! - every value is absolute mm and does not scale with font size;
//! - values are drawn per character, in text order, when the text is laid out;
//! - the line start offset applies to every line, not only the first;
//! - no first-line indent was observed in the default mode, so it is off unless set.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::deformation::Deformation;
use crate::{Error, Result};

/// Inclusive `[min, max]` range in mm (or the field's documented unit). Named after ChiXie3's
/// `min_*` / `max_*` config keys.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub min: f32,
    pub max: f32,
}

impl Span {
    pub const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    pub fn contains(&self, v: f32) -> bool {
        v >= self.min && v <= self.max
    }

    pub fn sample(&self, rng: &mut Rng) -> f32 {
        self.min + (self.max - self.min) * rng.unit()
    }
}

/// SplitMix64: tiny, fast and stable across platforms and versions, which matters because the
/// seed is persisted and must reproduce the same page later.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1]` (both ends reachable within f32 precision).
    pub fn unit(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u64 << 24) - 1) as f32
    }
}

/// How the vertical offset evolves along a row. Variant meanings beyond `Independent` are
/// best guesses: ChiXie3's `row_offset_style` value 1 was not distinguishable in captures. VERIFY.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowOffsetStyle {
    /// Each glyph draws its own offset from `vertical_offset` (matches captures).
    #[default]
    Independent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HandwritingStyle {
    pub enabled: bool,
    pub seed: u64,
    /// Extra gap after each CJK / non-CJK glyph, mm.
    pub char_spacing_zh: Span,
    pub char_spacing_en: Span,
    /// Width of a space after CJK / non-CJK context, mm.
    pub space_width_zh: Span,
    pub space_width_en: Span,
    /// Downward offset of each glyph from its line, mm.
    pub vertical_offset: Span,
    pub row_offset_style: RowOffsetStyle,
    /// Smooth the vertical offset against the previous glyph (random walk clamped to
    /// `vertical_offset`) instead of drawing independently. Best guess, VERIFY.
    pub intelligent_vertical_offset: bool,
    /// Pull the walk back toward the middle of the range each glyph. Only meaningful with
    /// `intelligent_vertical_offset`. Best guess, VERIFY.
    pub random_correct_offset: bool,
    /// X shift of the first glyph of every line, mm (ChiXie3 `first_text_not_align_range`, which
    /// captures show applies per line).
    pub line_start_offset: Span,
    /// Indent of a paragraph's first line, mm. `None` = not applied (ChiXie3 default mode).
    pub first_line_indent: Option<Span>,
}

impl Default for HandwritingStyle {
    /// ChiXie3's shipped `[WriteConfig]` values, effects disabled.
    fn default() -> Self {
        Self {
            enabled: false,
            seed: 0,
            char_spacing_zh: Span::new(0.6, 1.1),
            char_spacing_en: Span::new(0.5, 1.0),
            space_width_zh: Span::new(5.0, 7.0),
            space_width_en: Span::new(2.0, 3.0),
            vertical_offset: Span::new(0.0, 0.3),
            row_offset_style: RowOffsetStyle::Independent,
            intelligent_vertical_offset: false,
            random_correct_offset: false,
            line_start_offset: Span::new(0.0, 2.0),
            first_line_indent: None,
        }
    }
}

/// CJK ideographs, kana, hangul and full-width forms use the `_zh` ranges; everything else `_en`.
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF | 0x20000..=0x3134F)
}

/// Per-line jitter state: draws values in a fixed order so layouts are reproducible.
pub struct Jitter<'a> {
    style: &'a HandwritingStyle,
    rng: Rng,
    walk: Option<f32>,
}

impl<'a> Jitter<'a> {
    pub fn new(style: &'a HandwritingStyle) -> Self {
        Self { style, rng: Rng::new(style.seed), walk: None }
    }

    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }

    pub fn line_start(&mut self) -> f32 {
        self.walk = None;
        self.style.line_start_offset.sample(&mut self.rng)
    }

    pub fn spacing(&mut self, c: char) -> f32 {
        let s = &self.style;
        if is_cjk(c) { s.char_spacing_zh } else { s.char_spacing_en }.sample(&mut self.rng)
    }

    /// Space width; `prev` is the character before the space (decides zh vs en).
    pub fn space(&mut self, prev: Option<char>) -> f32 {
        let s = &self.style;
        if prev.is_some_and(is_cjk) { s.space_width_zh } else { s.space_width_en }.sample(&mut self.rng)
    }

    pub fn vertical(&mut self) -> f32 {
        let r = self.style.vertical_offset;
        if !self.style.intelligent_vertical_offset {
            return r.sample(&mut self.rng);
        }
        let step = (r.max - r.min) / 3.0;
        let mid = (r.min + r.max) / 2.0;
        let prev = self.walk.unwrap_or(mid);
        let mut v = prev + (self.rng.unit() * 2.0 - 1.0) * step;
        if self.style.random_correct_offset {
            v += (mid - v) * 0.5;
        }
        let v = v.clamp(r.min, r.max);
        self.walk = Some(v);
        v
    }
}

/// On-disk style profile (`--style <toml>`, and later the GUI settings).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleFile {
    pub handwriting: HandwritingStyle,
    pub deformation: Deformation,
}

impl StyleFile {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        toml::from_str(&text).map_err(|e| Error::Api(format!("style {}: {e}", path.display())))
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("style serializes")
    }
}

/// CLI/GUI overrides on top of a style file, lowest to highest precedence:
/// built-in defaults (all off) < style file < `seed` < `no_jitter` / `deform`.
/// `deform` turns base deformation on with the style file's ranges (or ChiXie3 defaults).
pub fn resolve(file: Option<StyleFile>, seed: Option<u64>, no_jitter: bool, deform: bool) -> StyleFile {
    let mut s = file.unwrap_or_default();
    if let Some(seed) = seed {
        s.handwriting.seed = seed;
    }
    if no_jitter {
        s.handwriting.enabled = false;
    }
    if deform {
        s.deformation.enabled = true;
        if !s.deformation.base && !s.deformation.shape {
            s.deformation.base = true;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_stable_and_in_range() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        let xs: Vec<f32> = (0..1000).map(|_| a.unit()).collect();
        assert!(xs.iter().all(|x| (0.0..=1.0).contains(x)));
        assert_eq!(xs[..5], (0..5).map(|_| b.unit()).collect::<Vec<_>>()[..]);
        // Pinned values: the seed is persisted, so the stream must never change.
        assert_eq!(Rng::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn samples_stay_in_spans() {
        let style = HandwritingStyle { enabled: true, seed: 7, ..Default::default() };
        let mut j = Jitter::new(&style);
        for _ in 0..500 {
            assert!(style.char_spacing_zh.contains(j.spacing('你')));
            assert!(style.char_spacing_en.contains(j.spacing('A')));
            assert!(style.space_width_zh.contains(j.space(Some('你'))));
            assert!(style.space_width_en.contains(j.space(Some('A'))));
            assert!(style.vertical_offset.contains(j.vertical()));
            assert!(style.line_start_offset.contains(j.line_start()));
        }
    }

    #[test]
    fn intelligent_walk_stays_in_range() {
        let style = HandwritingStyle {
            enabled: true,
            intelligent_vertical_offset: true,
            random_correct_offset: true,
            ..Default::default()
        };
        let mut j = Jitter::new(&style);
        let mut prev = None::<f32>;
        for _ in 0..500 {
            let v = j.vertical();
            assert!(style.vertical_offset.contains(v));
            if let Some(p) = prev {
                assert!((v - p).abs() <= 0.3 / 3.0 + 1e-6, "step {}", v - p);
            }
            prev = Some(v);
        }
    }

    #[test]
    fn precedence() {
        let file = StyleFile {
            handwriting: HandwritingStyle { enabled: true, seed: 1, ..Default::default() },
            ..Default::default()
        };
        // No file, no flags: everything off.
        let s = resolve(None, None, false, false);
        assert!(!s.handwriting.enabled && !s.deformation.enabled);
        // --seed overrides the file's seed; --no-jitter forces jitter off.
        let s = resolve(Some(file.clone()), Some(9), true, false);
        assert_eq!(s.handwriting.seed, 9);
        assert!(!s.handwriting.enabled);
        // --deform without a file turns base deformation on with default ranges.
        let s = resolve(None, None, false, true);
        assert!(s.deformation.enabled && s.deformation.base && !s.deformation.shape);
    }

    #[test]
    fn style_toml_roundtrips() {
        let s = StyleFile {
            handwriting: HandwritingStyle { enabled: true, seed: 3, ..Default::default() },
            ..Default::default()
        };
        let back: StyleFile = toml::from_str(&s.to_toml()).unwrap();
        assert_eq!(back, s);
        // Partial files fill the rest from defaults.
        let p: StyleFile = toml::from_str("[handwriting]\nenabled = true\n").unwrap();
        assert!(p.handwriting.enabled);
        assert_eq!(p.handwriting.char_spacing_zh, Span::new(0.6, 1.1));
    }
}
