//! Recover ChiXie3's glyph scale (`PXScale` → mm per glyph unit) from G-code it streamed.
//!
//! Capture method (see docs/protocol.md): point ChiXie3's wireless device at a fake Grbl on
//! 127.0.0.1:23 and log every line. ChiXie3 output looks like
//! `G1 X+4.205Y-2.376F19000` with pen `G1G90 Z2F19000` (down) / `G1G90 Z0F19000` (up), travel as
//! `G1` (not `G0`), and Y negative going down the page.
//!
//! Fit model: every captured glyph is the font glyph scaled by one global `s` and moved by its
//! own translation `t_g`. Per-glyph translations absorb character spacing and baseline jitter,
//! so they can't bias the scale: `s = Σ (q−q̄_g)·(p−p̄_g) / Σ |p−p̄_g|²`.

use crate::font::Font;
use crate::{Error, Result};

/// ChiXie3's glyph scale, verified 2026-11-06 by capture (艾玛体 at sizes 100 and 50, fits
/// exact to 3-decimal rounding): glyph units are millimetres at font size 100, linear in size.
/// Spacing, jitter and first-glyph offsets are absolute mm and do NOT scale with size.
pub fn mm_per_unit_for_font_size(font_size: f32) -> f32 {
    font_size / 100.0
}

/// A pen-down polyline, in captured machine mm with Y as streamed (up positive).
pub type CapturedStroke = Vec<(f32, f32)>;

/// Extract pen-down polylines from captured G-code. A stroke starts at the position where the
/// pen goes down and collects every move until the pen goes up.
/// `pen_down_z`: the Z value meaning "down" (2 on the verified board).
pub fn parse_strokes(gcode: &str, pen_down_z: f32) -> Vec<CapturedStroke> {
    let mut strokes = Vec::new();
    let mut pos = (0.0f32, 0.0f32);
    let mut down = false;
    let mut cur: CapturedStroke = Vec::new();
    for line in gcode.lines() {
        let l = line.split(';').next().unwrap_or("").trim().to_ascii_uppercase();
        if l.is_empty() {
            continue;
        }
        let (x, y, z) = (word(&l, 'X'), word(&l, 'Y'), word(&l, 'Z'));
        if let Some(z) = z {
            let now_down = (z - pen_down_z).abs() < 1e-3;
            if now_down && !down {
                cur = vec![pos];
            } else if !now_down && down && !cur.is_empty() {
                strokes.push(std::mem::take(&mut cur));
            }
            down = now_down;
        }
        if x.is_some() || y.is_some() {
            pos = (x.unwrap_or(pos.0), y.unwrap_or(pos.1));
            if down {
                cur.push(pos);
            }
        }
    }
    if down && !cur.is_empty() {
        strokes.push(cur);
    }
    strokes
}

/// Value of the first `letter` word in a G-code line, e.g. `X+4.205` → 4.205.
fn word(line: &str, letter: char) -> Option<f32> {
    let i = line.find(letter)?;
    let rest = &line[i + 1..];
    let end = rest.find(|c: char| !(c.is_ascii_digit() || matches!(c, '+' | '-' | '.'))).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Where one glyph landed: its translation in captured mm (glyph unit origin → machine).
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphPlacement {
    pub ch: char,
    /// Translation of the glyph's (0,0) in machine mm (Y up, as captured).
    pub offset: (f32, f32),
    /// RMS distance (mm) between captured points and the fitted glyph.
    pub rms: f32,
}

#[derive(Debug, Clone)]
pub struct Fit {
    pub mm_per_unit: f32,
    pub placements: Vec<GlyphPlacement>,
    /// Overall RMS residual in mm. Near zero means no per-point deformation in the capture.
    pub rms: f32,
    /// Strokes left out because their point counts still differ after dropping repeated
    /// points: `(char, stroke index, captured points, font points)`.
    pub skipped: Vec<(char, usize, usize, usize)>,
}

/// Drop consecutive repeated points (ChiXie3 does not stream a move to where the pen already is).
fn dedup(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out: Vec<(f32, f32)> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    out
}

/// Fit the scale between `captured` strokes and `text` drawn with `font`.
/// Captured strokes must follow the font's stroke order (one per font stroke). Point counts are
/// compared after dropping consecutive repeated points on both sides; strokes that still differ
/// are left out of the fit and listed in `Fit::skipped`.
pub fn fit(captured: &[CapturedStroke], text: &str, font: &Font) -> Result<Fit> {
    // Group captured strokes by glyph, in text order: (char, font points y-up, captured points).
    type Group = (char, Vec<(f32, f32)>, Vec<(f32, f32)>);
    let mut groups: Vec<Group> = Vec::new();
    let mut skipped = Vec::new();
    let mut it = captured.iter();
    for ch in text.chars().filter(|c| !c.is_whitespace()) {
        let Some(g) = font.get(ch) else { continue };
        let (mut p, mut q) = (Vec::new(), Vec::new());
        for (si, s) in g.strokes.iter().enumerate() {
            let c = it.next().ok_or_else(|| {
                Error::Font(format!("capture has fewer strokes than the text ('{ch}' stroke {si})"))
            })?;
            let (s, c) = (dedup(s), dedup(c));
            if c.len() != s.len() {
                skipped.push((ch, si, c.len(), s.len()));
                continue;
            }
            p.extend(s.iter().map(|&(x, y)| (x, -y))); // glyph y-down → machine y-up
            q.extend(c.iter().copied());
        }
        if !p.is_empty() {
            groups.push((ch, p, q));
        }
    }
    if it.next().is_some() {
        return Err(Error::Font(format!("capture has {} strokes, more than the text", captured.len())));
    }
    if groups.is_empty() {
        return Err(Error::Font("no drawable glyphs to fit".into()));
    }

    let mean = |v: &[(f32, f32)]| {
        let n = v.len() as f64;
        let (sx, sy) = v.iter().fold((0f64, 0f64), |a, &(x, y)| (a.0 + x as f64, a.1 + y as f64));
        (sx / n, sy / n)
    };
    let (mut num, mut den) = (0f64, 0f64);
    for (_, p, q) in &groups {
        let (pm, qm) = (mean(p), mean(q));
        for (&(px, py), &(qx, qy)) in p.iter().zip(q) {
            let (dpx, dpy) = (px as f64 - pm.0, py as f64 - pm.1);
            num += (qx as f64 - qm.0) * dpx + (qy as f64 - qm.1) * dpy;
            den += dpx * dpx + dpy * dpy;
        }
    }
    if den == 0.0 {
        return Err(Error::Font("degenerate glyphs (all points coincide)".into()));
    }
    let s = num / den;

    let (mut placements, mut sq, mut n) = (Vec::new(), 0f64, 0usize);
    for (ch, p, q) in &groups {
        let (pm, qm) = (mean(p), mean(q));
        let t = (qm.0 - s * pm.0, qm.1 - s * pm.1);
        let mut gsq = 0f64;
        for (&(px, py), &(qx, qy)) in p.iter().zip(q) {
            let (ex, ey) = (qx as f64 - (t.0 + s * px as f64), qy as f64 - (t.1 + s * py as f64));
            gsq += ex * ex + ey * ey;
        }
        sq += gsq;
        n += p.len();
        placements.push(GlyphPlacement {
            ch: *ch,
            offset: (t.0 as f32, t.1 as f32),
            rms: (gsq / p.len() as f64).sqrt() as f32,
        });
    }
    Ok(Fit { mm_per_unit: s as f32, placements, rms: (sq / n as f64).sqrt() as f32, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/font_fixture.json");

    /// Render `text` the way ChiXie3 streams it: per-glyph offsets, `+`-signed 3-decimal coords.
    fn chixie_gcode(text: &str, font: &Font, s: f32, offsets: &[(f32, f32)]) -> String {
        let mut g = String::from("G21\nG90\nF19000\nG1G90 Z0F19000\nG1G90 Z0F19000\n");
        let mut k = 0;
        for ch in text.chars().filter(|c| !c.is_whitespace()) {
            let Some(gl) = font.get(ch) else { continue };
            let (ox, oy) = offsets[k];
            k += 1;
            for st in &gl.strokes {
                for (i, &(x, y)) in st.iter().enumerate() {
                    let (mx, my) = (ox + s * x, oy - s * y);
                    g += &format!("G1 X{mx:+.3}Y{my:+.3}F19000\n");
                    if i == 0 {
                        g += "G1G90 Z2F19000\n";
                    }
                }
                g += "G1G90 Z0F19000\n";
            }
        }
        g + "G1G90 Z0F19000\nG90G0 X0Y0\n"
    }

    #[test]
    fn font_size_maps_linearly() {
        assert_eq!(mm_per_unit_for_font_size(100.0), 1.0);
        assert_eq!(mm_per_unit_for_font_size(50.0), 0.5);
    }

    #[test]
    fn parses_chixie_format() {
        let g = "G21\nG90\nG1G90 Z0F19000\nG1 X+4.205Y-2.376F19000\nG1G90 Z2F19000\n\
                 G1 X+3.887Y-3.275F19000\nG1G90 Z0F19000\nG1 X+1Y-1F19000\nG1G90 Z2F19000\n\
                 G1G90 Z0F19000\nG90G0 X0Y0\n";
        let s = parse_strokes(g, 2.0);
        assert_eq!(s, vec![vec![(4.205, -2.376), (3.887, -3.275)], vec![(1.0, -1.0)]]);
    }

    #[test]
    fn recovers_scale_despite_per_glyph_jitter() {
        let font = Font::from_json(FIXTURE).unwrap();
        let text = "你好 A.";
        // Uneven spacing and baseline offsets, like handwriting jitter.
        let offsets = [(4.2, -2.4), (25.9, -2.1), (50.3, -2.9), (61.0, -2.2)];
        let g = chixie_gcode(text, &font, 2.85, &offsets);
        let f = fit(&parse_strokes(&g, 2.0), text, &font).unwrap();
        assert!((f.mm_per_unit - 2.85).abs() / 2.85 < 0.005, "scale {}", f.mm_per_unit);
        assert!(f.rms < 0.002, "rms {}", f.rms); // only 3-decimal rounding left
        let got: Vec<char> = f.placements.iter().map(|p| p.ch).collect();
        assert_eq!(got, vec!['你', '好', 'A', '.']);
        assert!((f.placements[1].offset.0 - 25.9).abs() < 0.01);
    }

    #[test]
    fn repeated_points_are_ignored_and_real_mismatches_skipped() {
        let mut font = Font::from_json(FIXTURE).unwrap();
        let g = chixie_gcode("A", &font, 2.0, &[(5.0, -1.0)]);
        // Font stroke 0 gains a repeated point: still fits after dedup.
        let a = font.glyphs.get_mut(&'A').unwrap();
        let p = a.strokes[0][1];
        a.strokes[0].insert(1, p);
        let f = fit(&parse_strokes(&g, 2.0), "A", &font).unwrap();
        assert!(f.skipped.is_empty());
        assert!((f.mm_per_unit - 2.0).abs() < 0.01);
        // A genuinely different stroke is skipped, not fatal; the other stroke still fits.
        font.glyphs.get_mut(&'A').unwrap().strokes[0].push((9.0, 9.0));
        let f = fit(&parse_strokes(&g, 2.0), "A", &font).unwrap();
        assert_eq!(f.skipped, vec![('A', 0, 3, 4)]);
        assert!((f.mm_per_unit - 2.0).abs() < 0.01, "scale {}", f.mm_per_unit);
    }

    #[test]
    fn stroke_mismatch_is_an_error() {
        let font = Font::from_json(FIXTURE).unwrap();
        let g = chixie_gcode("A", &font, 1.0, &[(0.0, 0.0)]);
        let err = fit(&parse_strokes(&g, 2.0), "AA", &font).unwrap_err();
        assert!(err.to_string().contains("fewer strokes"), "{err}");
    }
}
