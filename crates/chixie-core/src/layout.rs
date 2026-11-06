//! Text → positioned pen strokes in machine mm (`Document`).
//!
//! The preview and the G-code emitter (`gcode.rs`) both consume the same `Document`, so what is
//! shown is what is drawn. Glyph units are scaled to mm and Y is flipped (glyph y-down → machine
//! y-up). The page origin is the top-left corner, in machine mm.
//!
//! With every effect off, the arithmetic here keeps the exact float expressions and operation
//! order of the original `writing::text_to_gcode`, so its output stays byte-identical. The golden
//! test and the `legacy` oracle below check this; don't reorder the math.

use crate::deformation::Deformation;
use crate::font::Font;
use crate::handwriting::{HandwritingStyle, Jitter, Rng, is_cjk};

/// One pen-down polyline in machine mm.
pub type Stroke = Vec<(f32, f32)>;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    pub strokes: Vec<Stroke>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub pages: Vec<Page>,
    /// Characters with no glyph in the font (each skipped with a space-width advance), deduped,
    /// in first-seen order.
    pub missing: Vec<char>,
}

/// Writable area measured from `origin`: x grows right, y grows *down* the page (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBox {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct PageParams {
    /// mm per glyph unit (character size). ChiXie3 derives it from font size × PXScale; the
    /// calibrated value comes from M1 (see docs/protocol.md). TODO: verify against capture A.
    pub mm_per_unit: f32,
    /// Extra mm added after each glyph's advance (character spacing).
    pub char_spacing: f32,
    /// Advance in mm for ' ' when the font has no space glyph, and for missing characters.
    pub space_width: f32,
    /// Baseline-to-baseline distance in mm for '\n' and wrapping.
    pub line_height: f32,
    /// Machine mm where the first glyph's (0,0) lands.
    pub origin: (f32, f32),
    /// `None`: one unbounded page (CLI default). `Some`: wrap at `width`, new page past `height`.
    pub page: Option<PageBox>,
    /// Handwriting jitter. When enabled it replaces `char_spacing` and `space_width`.
    pub style: HandwritingStyle,
    pub deformation: Deformation,
}

impl Default for PageParams {
    fn default() -> Self {
        Self {
            mm_per_unit: 1.0,
            char_spacing: 0.0,
            space_width: 3.5,
            line_height: 10.0,
            origin: (0.0, 0.0),
            page: None,
            style: HandwritingStyle::default(),
            deformation: Deformation::default(),
        }
    }
}

/// Lay out `text` with `font`. Unknown characters advance like a space and are listed in
/// `Document::missing`.
///
/// With jitter and deformation off, every coordinate uses the original expressions (see module
/// docs). With them on, values are drawn in text order from the style's seed: line start offset,
/// then per glyph spacing / space width, vertical offset, and deformation.
pub fn layout(text: &str, font: &Font, p: &PageParams) -> Document {
    let s = p.mm_per_unit;
    let (ox, oy) = p.origin;
    let jitter_on = p.style.enabled;
    let deform_on = p.deformation.is_active();
    let mut jit = Jitter::new(&p.style);
    // Separate stream so turning deformation on doesn't reshuffle the jitter (and vice versa).
    let mut drng = Rng::new(p.style.seed ^ 0xD3F0_12A7_5EED_0001);

    let mut doc = Document::default();
    let mut page = Page::default();
    let mut cx = 0.0f32; // cursor x in mm, relative to origin
    let mut line_top = 0.0f32; // mm below origin for the current line
    let mut prev: Option<char> = None;

    let newline = |cx: &mut f32, line_top: &mut f32, page: &mut Page, doc: &mut Document| {
        *cx = 0.0;
        *line_top += p.line_height;
        if let Some(b) = p.page
            && *line_top + p.line_height > b.height
        {
            doc.pages.push(std::mem::take(page));
            *line_top = 0.0;
        }
    };
    // Start-of-line jitter (every line, per captures) and paragraph indent (first line only).
    let start_line = |cx: &mut f32, jit: &mut Jitter, paragraph: bool| {
        if jitter_on {
            *cx = jit.line_start();
            if paragraph && let Some(indent) = p.style.first_line_indent {
                *cx += indent.sample(jit.rng());
            }
        }
    };
    start_line(&mut cx, &mut jit, true);

    for ch in text.chars() {
        match ch {
            '\n' => {
                newline(&mut cx, &mut line_top, &mut page, &mut doc);
                start_line(&mut cx, &mut jit, true);
                prev = None;
                continue;
            }
            ' ' => {
                if jitter_on {
                    cx += jit.space(prev);
                } else {
                    cx += font.get(' ').map(|gl| gl.advance * s).unwrap_or(p.space_width);
                    cx += p.char_spacing;
                }
                continue;
            }
            _ => {}
        }
        let Some(glyph) = font.get(ch) else {
            if !doc.missing.contains(&ch) {
                doc.missing.push(ch);
            }
            if jitter_on {
                cx += jit.space(Some(ch));
            } else {
                cx += p.space_width + p.char_spacing;
            }
            prev = Some(ch);
            continue;
        };
        // Wrap before a glyph that would cross the right edge (never on an empty line).
        if let Some(b) = p.page
            && cx > 0.0
            && cx + glyph.advance * s > b.width
        {
            newline(&mut cx, &mut line_top, &mut page, &mut doc);
            start_line(&mut cx, &mut jit, false);
        }
        if !jitter_on && !deform_on {
            for stroke in &glyph.strokes {
                page.strokes.push(
                    stroke
                        .iter()
                        .map(|&(gx, gy)| (ox + cx + gx * s, oy - (line_top + gy * s)))
                        .collect(),
                );
            }
            cx += glyph.advance * s + p.char_spacing;
        } else {
            let voff = if jitter_on { jit.vertical() } else { 0.0 };
            let strokes = p.deformation.apply(&glyph.strokes, is_cjk(ch), s, &mut drng);
            for stroke in &strokes {
                page.strokes.push(
                    stroke
                        .iter()
                        .map(|&(gx, gy)| (ox + cx + gx * s, oy - (line_top + voff + gy * s)))
                        .collect(),
                );
            }
            cx += glyph.advance * s + if jitter_on { jit.spacing(ch) } else { p.char_spacing };
        }
        prev = Some(ch);
    }
    doc.pages.push(page);
    doc
}

/// The original `writing::text_to_gcode`, kept verbatim as a test oracle for the split.
/// Delete once the split has been verified on hardware (plan M2: oracle goes, golden stays).
#[cfg(test)]
pub(crate) mod legacy {
    use std::fmt::Write;

    use super::PageParams;
    use crate::font::Font;
    use crate::gcode::DeviceProfile;

    pub fn text_to_gcode(text: &str, font: &Font, p: &PageParams, d: &DeviceProfile) -> String {
        let mut g = String::new();
        let _ = writeln!(g, "{} F{:.0}", d.pen_up, d.travel_speed);
        let (ox, oy) = p.origin;
        let mut cx = 0.0f32;
        let mut line_top = 0.0f32;
        for ch in text.chars() {
            match ch {
                '\n' => {
                    cx = 0.0;
                    line_top += p.line_height;
                    continue;
                }
                ' ' => {
                    cx += font.get(' ').map(|gl| gl.advance * p.mm_per_unit).unwrap_or(p.space_width);
                    cx += p.char_spacing;
                    continue;
                }
                _ => {}
            }
            let Some(glyph) = font.get(ch) else {
                cx += p.space_width + p.char_spacing;
                continue;
            };
            for stroke in &glyph.strokes {
                for (i, &(gx, gy)) in stroke.iter().enumerate() {
                    let mx = ox + cx + gx * p.mm_per_unit;
                    let my = oy - (line_top + gy * p.mm_per_unit);
                    if i == 0 {
                        let _ = writeln!(g, "G0 X{mx:.3} Y{my:.3}");
                        let _ = writeln!(g, "{} F{:.0}", d.pen_down, d.travel_speed);
                        let _ = writeln!(g, "G1 X{mx:.3} Y{my:.3} F{:.0}", d.write_speed);
                    } else {
                        let _ = writeln!(g, "G1 X{mx:.3} Y{my:.3} F{:.0}", d.write_speed);
                    }
                }
                let _ = writeln!(g, "{} F{:.0}", d.pen_up, d.travel_speed);
            }
            cx += glyph.advance * p.mm_per_unit + p.char_spacing;
        }
        g
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::font::Glyph;
    use crate::gcode::{DeviceProfile, emit_document};

    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/font_fixture.json");

    fn font_with(c: char, glyph: Glyph) -> Font {
        Font { glyphs: HashMap::from([(c, glyph)]) }
    }

    #[test]
    fn matches_legacy_oracle_across_params() {
        let font = Font::from_json(FIXTURE).unwrap();
        let d = DeviceProfile::default();
        let texts = ["你好 A.\n好你", "", "  ", "A\n\n.你", "x你y好z", "好好好好好好好好好好好好"];
        let params = [
            PageParams::default(),
            PageParams { mm_per_unit: 3.0, origin: (0.0, 24.0), ..Default::default() },
            PageParams { mm_per_unit: 0.37, char_spacing: 0.13, origin: (12.5, 280.1), ..Default::default() },
            PageParams { mm_per_unit: 7.1, space_width: 2.2, line_height: 13.7, ..Default::default() },
        ];
        for t in texts {
            for p in &params {
                let new = emit_document(&layout(t, &font, p), &d);
                assert_eq!(new, legacy::text_to_gcode(t, &font, p, &d), "text {t:?} params {p:?}");
            }
        }
    }

    #[test]
    fn flips_y_and_scales() {
        let g = Glyph { advance: 3.0, strokes: vec![vec![(0.0, 0.0), (0.0, 2.0)]] };
        let p = PageParams { origin: (10.0, 50.0), ..Default::default() };
        let doc = layout("I", &font_with('I', g), &p);
        assert_eq!(doc.pages, vec![Page { strokes: vec![vec![(10.0, 50.0), (10.0, 48.0)]] }]);
    }

    #[test]
    fn missing_chars_are_listed_once_and_advance() {
        let g = Glyph { advance: 1.0, strokes: vec![vec![(0.0, 0.0)]] };
        let p = PageParams { space_width: 4.0, ..Default::default() };
        let doc = layout("x.x", &font_with('.', g), &p);
        assert_eq!(doc.missing, vec!['x']);
        assert_eq!(doc.pages[0].strokes, vec![vec![(4.0, 0.0)]]);
    }

    #[test]
    fn newline_resets_x_and_drops_line() {
        let g = Glyph { advance: 1.0, strokes: vec![vec![(0.0, 0.0)]] };
        let p = PageParams { origin: (0.0, 100.0), line_height: 10.0, ..Default::default() };
        let doc = layout(".\n.", &font_with('.', g), &p);
        assert_eq!(doc.pages[0].strokes, vec![vec![(0.0, 100.0)], vec![(0.0, 90.0)]]);
    }

    fn jitter_params(seed: u64) -> PageParams {
        PageParams {
            style: HandwritingStyle { enabled: true, seed, ..Default::default() },
            ..Default::default()
        }
    }

    #[test]
    fn jitter_is_seeded_and_bounded() {
        // '.' is one point at (0.6, 6.9), advance 1.25 (fixture font); en spacing 0.5–1.0.
        let font = Font::from_json(FIXTURE).unwrap();
        let text = "....\n....";
        let a = layout(text, &font, &jitter_params(1));
        assert_eq!(a, layout(text, &font, &jitter_params(1)));
        assert_ne!(a, layout(text, &font, &jitter_params(2)));

        let pts: Vec<(f32, f32)> = a.pages[0].strokes.iter().map(|s| s[0]).collect();
        for line in pts.chunks(4) {
            // Line start offset 0–2 mm on every line.
            assert!((0.6..=2.6 + 1e-4).contains(&line[0].0), "start {}", line[0].0);
            for w in line.windows(2) {
                let adv = w[1].0 - w[0].0;
                assert!((1.75 - 1e-4..=2.25 + 1e-4).contains(&adv), "advance {adv}");
            }
        }
        // Vertical offset 0–0.3 below each line's nominal y (6.9 on line 0, 16.9 on line 1).
        for (i, p) in pts.iter().enumerate() {
            let nominal = if i < 4 { -6.9 } else { -16.9 };
            let off = nominal - p.1;
            assert!((-1e-4..=0.3 + 1e-4).contains(&off), "offset {off}");
        }
    }

    #[test]
    fn deformation_alone_is_seeded_and_changes_strokes() {
        let font = Font::from_json(FIXTURE).unwrap();
        let p = PageParams {
            deformation: Deformation { enabled: true, base: true, ..Default::default() },
            ..Default::default()
        };
        let plain = layout("你好", &font, &PageParams::default());
        let d1 = layout("你好", &font, &p);
        assert_ne!(plain, d1);
        assert_eq!(d1, layout("你好", &font, &p));
        // Jitter stays off: same stroke count, no extra spacing.
        assert_eq!(plain.pages[0].strokes.len(), d1.pages[0].strokes.len());
    }

    #[test]
    fn first_line_indent_only_on_paragraph_starts() {
        let font = Font::from_json(FIXTURE).unwrap();
        let mut p = jitter_params(4);
        p.style.line_start_offset = crate::handwriting::Span::new(0.0, 0.0);
        p.style.first_line_indent = Some(crate::handwriting::Span::new(10.0, 10.0));
        // 12.8 mm wide: after the 10 mm indent one dot fits (10 + 1.25), the second wraps
        // (cursor ≥ 11.75 + 1.25 > 12.8).
        p.page = Some(PageBox { width: 12.8, height: 100.0 });
        let doc = layout("..\n.", &font, &p);
        let xs: Vec<f32> = doc.pages[0].strokes.iter().map(|s| s[0].0).collect();
        // Paragraph start indented, wrapped line not, next paragraph indented again.
        let want = [10.6, 0.6, 10.6];
        assert!(xs.iter().zip(want).all(|(x, w)| (x - w).abs() < 1e-4), "{xs:?}");
    }

    #[test]
    fn wraps_at_width_and_breaks_pages() {
        // advance 4 mm, box 10 x 20 mm, line height 10 → 2 glyphs/line (third would end at 12),
        // 2 lines/page (a line fits while line_top + line_height <= height).
        let g = Glyph { advance: 4.0, strokes: vec![vec![(0.0, 0.0)]] };
        let p = PageParams { page: Some(PageBox { width: 10.0, height: 20.0 }), ..Default::default() };
        let doc = layout(".....", &font_with('.', g), &p);
        let starts: Vec<Vec<(f32, f32)>> =
            doc.pages.iter().map(|pg| pg.strokes.iter().map(|s| s[0]).collect()).collect();
        assert_eq!(
            starts,
            vec![vec![(0.0, 0.0), (4.0, 0.0), (0.0, -10.0), (4.0, -10.0)], vec![(0.0, 0.0)]]
        );
    }
}
