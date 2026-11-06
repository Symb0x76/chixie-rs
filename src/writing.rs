//! Lay out text with a stroke font and emit writing G-code.
//!
//! Mirrors `generate_gcode_of_writing`: glyph units are scaled to mm, Y is flipped
//! (glyph y-down → machine y-up), each stroke starts with a pen-down and ends pen-up,
//! moves between strokes/glyphs are travel moves. Page origin is top-left in machine mm.

use std::fmt::Write;

use crate::font::Font;

/// How glyph units map to the page and how the pen is driven.
#[derive(Debug, Clone)]
pub struct WritingParams {
    /// mm per glyph unit (character size). The old app derives this from font size × PXScale;
    /// PXScale is a compiled constant — calibrate against a captured .gcode. TODO: verify.
    pub mm_per_unit: f32,
    /// Extra mm added after each glyph's advance (character spacing).
    pub char_spacing: f32,
    /// Space width in mm for ' ' when the font has no space glyph.
    pub space_width: f32,
    /// Line height in mm (baseline to baseline) for '\n'.
    pub line_height: f32,
    /// Page origin (machine mm): where the first glyph's (0,0) lands.
    pub origin: (f32, f32),
    /// Writing feed (pen down) and travel feed (pen up), mm/min.
    pub write_speed: f32,
    pub travel_speed: f32,
    /// Device G-code to lift / lower the pen (e.g. "G1G90 Z0" / "G1G90 Z2").
    pub pen_up: String,
    pub pen_down: String,
}

impl Default for WritingParams {
    fn default() -> Self {
        Self {
            mm_per_unit: 1.0,
            char_spacing: 0.0,
            space_width: 3.5,
            line_height: 10.0,
            origin: (0.0, 0.0),
            write_speed: 3000.0,
            travel_speed: 8000.0,
            pen_up: "G1G90 Z0".into(),
            pen_down: "G1G90 Z2".into(),
        }
    }
}

/// Generate the body G-code for `text`. Pen starts and ends up.
/// Unknown characters are skipped (advance only, like the original).
pub fn text_to_gcode(text: &str, font: &Font, p: &WritingParams) -> String {
    let mut g = String::new();
    let _ = writeln!(g, "{} F{:.0}", p.pen_up, p.travel_speed);

    let (ox, oy) = p.origin;
    let mut cx = 0.0f32; // cursor x in mm, relative to origin
    let mut line_top = 0.0f32; // mm below origin for the current line

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
                let my = oy - (line_top + gy * p.mm_per_unit); // flip Y
                if i == 0 {
                    // travel to the stroke start, pen up, then drop the pen
                    let _ = writeln!(g, "G0 X{mx:.3} Y{my:.3}");
                    let _ = writeln!(g, "{} F{:.0}", p.pen_down, p.travel_speed);
                    let _ = writeln!(g, "G1 X{mx:.3} Y{my:.3} F{:.0}", p.write_speed);
                } else {
                    let _ = writeln!(g, "G1 X{mx:.3} Y{my:.3} F{:.0}", p.write_speed);
                }
            }
            let _ = writeln!(g, "{} F{:.0}", p.pen_up, p.travel_speed);
        }
        cx += glyph.advance * p.mm_per_unit + p.char_spacing;
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{Font, Glyph};
    use std::collections::HashMap;

    fn font_with(c: char, glyph: Glyph) -> Font {
        let mut glyphs = HashMap::new();
        glyphs.insert(c, glyph);
        Font { glyphs }
    }

    #[test]
    fn emits_pen_cycle_and_flips_y() {
        // One vertical stroke from top(y=0) to y=2, advance 3.
        let g = Glyph { advance: 3.0, strokes: vec![vec![(0.0, 0.0), (0.0, 2.0)]] };
        let font = font_with('I', g);
        let p = WritingParams { origin: (10.0, 50.0), mm_per_unit: 1.0, ..Default::default() };
        let out = text_to_gcode("I", &font, &p);

        assert!(out.starts_with("G1G90 Z0 F8000\n")); // pen up first
        assert!(out.contains("G0 X10.000 Y50.000\n")); // travel to start, y not flipped at 0
        assert!(out.contains("G1G90 Z2 F8000\n")); // pen down
        assert!(out.contains("G1 X10.000 Y48.000 F3000\n")); // y flipped: 50 - 2
        // pen lifts after the stroke
        let lifts = out.matches("G1G90 Z0").count();
        assert_eq!(lifts, 2); // initial + after stroke
    }

    #[test]
    fn unknown_char_advances_only() {
        let font = Font::default();
        let p = WritingParams::default();
        let out = text_to_gcode("x", &font, &p);
        assert_eq!(out.matches("G1 X").count(), 0); // nothing drawn
    }

    #[test]
    fn newline_resets_x_and_drops_line() {
        let g = Glyph { advance: 1.0, strokes: vec![vec![(0.0, 0.0)]] };
        let font = font_with('.', g);
        let p = WritingParams { origin: (0.0, 100.0), line_height: 10.0, ..Default::default() };
        let out = text_to_gcode(".\n.", &font, &p);
        // first dot at y=100, second after newline at y=100-10=90
        assert!(out.contains("G0 X0.000 Y100.000\n"));
        assert!(out.contains("G0 X0.000 Y90.000\n"));
    }
}
