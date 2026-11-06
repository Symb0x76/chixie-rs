//! `Document` → Grbl G-code for a pen plotter.
//!
//! Each stroke: travel (`G0`) to its start, pen down, write moves (`G1`), pen up. The page starts
//! with a pen-up. Feeds are always explicit: the ChiXie3 pen literals carry no `F` and rely on the
//! modal feed (see docs/protocol.md).

use std::fmt::Write;

use crate::layout::{Document, Page};

/// How the device moves the pen. Defaults match the verified Grbl_ESP32 board.
#[derive(Debug, Clone)]
pub struct DeviceProfile {
    /// Device G-code to lift / lower the pen (e.g. "G1G90 Z0" / "G1G90 Z2").
    pub pen_up: String,
    pub pen_down: String,
    /// Writing feed (pen down) and travel feed (pen up), mm/min.
    pub write_speed: f32,
    pub travel_speed: f32,
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self {
            pen_up: "G1G90 Z0".into(),
            pen_down: "G1G90 Z2".into(),
            write_speed: 3000.0,
            travel_speed: 8000.0,
        }
    }
}

/// G-code body for one page. The pen starts and ends up.
pub fn emit(page: &Page, d: &DeviceProfile) -> String {
    let mut g = String::new();
    let _ = writeln!(g, "{} F{:.0}", d.pen_up, d.travel_speed);
    for stroke in &page.strokes {
        for (i, &(mx, my)) in stroke.iter().enumerate() {
            if i == 0 {
                let _ = writeln!(g, "G0 X{mx:.3} Y{my:.3}");
                let _ = writeln!(g, "{} F{:.0}", d.pen_down, d.travel_speed);
            }
            let _ = writeln!(g, "G1 X{mx:.3} Y{my:.3} F{:.0}", d.write_speed);
        }
        // Unconditional (even for an empty stroke) to stay byte-identical with the original.
        let _ = writeln!(g, "{} F{:.0}", d.pen_up, d.travel_speed);
    }
    g
}

/// All pages back to back, for single-sheet output (CLI). Multi-page jobs should stream
/// `emit` per page with a sheet change in between.
pub fn emit_document(doc: &Document, d: &DeviceProfile) -> String {
    let mut out = String::new();
    for (i, page) in doc.pages.iter().enumerate() {
        let body = emit(page, d);
        // Pages after the first skip their leading pen-up: the previous page ended pen-up.
        out.push_str(if i == 0 { &body } else { body.split_once('\n').map_or("", |(_, rest)| rest) });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_pen_cycle() {
        let page = Page { strokes: vec![vec![(10.0, 50.0), (10.0, 48.0)]] };
        let out = emit(&page, &DeviceProfile::default());
        assert_eq!(
            out,
            "G1G90 Z0 F8000\nG0 X10.000 Y50.000\nG1G90 Z2 F8000\nG1 X10.000 Y50.000 F3000\n\
             G1 X10.000 Y48.000 F3000\nG1G90 Z0 F8000\n"
        );
    }

    #[test]
    fn empty_page_is_just_pen_up() {
        assert_eq!(emit(&Page::default(), &DeviceProfile::default()), "G1G90 Z0 F8000\n");
    }
}
