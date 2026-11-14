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
use crate::handwriting::{HandwritingStyle, Jitter, SALT_DEFORM, SALT_ROWS, is_cjk, rng_for};

/// One pen-down polyline in machine mm.
pub type Stroke = Vec<(f32, f32)>;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    pub strokes: Vec<Stroke>,
    /// Template ruling (row lines, cell boxes, column dividers) in machine mm. Shown in the
    /// editor only; never written: templates describe pre-printed paper.
    pub guides: Vec<Guide>,
}

/// One ruling segment (machine mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guide {
    pub a: (f32, f32),
    pub b: (f32, f32),
    pub dashed: bool,
}

/// Nominal glyph height in font units (CJK glyphs span about 0..7). Used to sit text on ruled
/// rows and centre it in grid cells; both placements match ChiXie3 captures (docs/protocol.md).
pub const EM_UNITS: f32 = 7.0;

/// How lines are placed on a page.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Ruling {
    /// Lines every `line_height` from the top (no guides).
    #[default]
    Free,
    /// Ruled paper: `rows` lines per page (a random count in the range, drawn per page),
    /// `columns` equal columns `column_gap` apart, filled top to bottom then left to right.
    /// `band`: dual-line mode, an upper band (mm) kept free above the writing line.
    Rows { rows: (u32, u32), columns: u32, column_gap: f32, band: Option<f32>, dashed: bool, divider: bool },
    /// Composition grid: one character per `cell` (w, h), `gap` (col, row) between cells.
    Grid { rows: u32, cols: u32, cell: (f32, f32), gap: (f32, f32) },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub pages: Vec<Page>,
    /// Characters with no glyph in the font (each skipped with a space-width advance), deduped,
    /// in first-seen order.
    pub missing: Vec<char>,
    /// One box per input character (glyphs, spaces, newlines, missing glyphs), in text order.
    pub boxes: Vec<CharBox>,
    /// Every line, in order; caret positions on a line are `start..=end` (char indices).
    pub lines: Vec<LineInfo>,
    /// Layout origin (machine mm): caret x on an empty line.
    pub origin: (f32, f32),
}

/// Where one input character sits (machine mm; y up, so `top > bottom`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharBox {
    pub page: usize,
    pub line: usize,
    pub x0: f32,
    pub x1: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineInfo {
    pub page: usize,
    pub top: f32,
    pub bottom: f32,
    /// First char index on the line.
    pub start: usize,
    /// Caret index at the line's end: the index of its '\n', or of the first char of the next
    /// (wrapped) line, or the text length on the last line.
    pub end: usize,
}

/// A caret position on the page (machine mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caret {
    pub page: usize,
    pub line: usize,
    pub x: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Document {
    /// Line holding caret index `i`. At a wrap boundary the caret belongs to the next line.
    pub fn line_of(&self, i: usize) -> usize {
        self.lines
            .iter()
            .rposition(|l| l.start <= i && i <= l.end)
            .unwrap_or(self.lines.len().saturating_sub(1))
    }

    /// Caret geometry for index `i` (0..=text length).
    pub fn caret(&self, i: usize) -> Caret {
        let li = self.line_of(i);
        let Some(l) = self.lines.get(li) else {
            return Caret { page: 0, line: 0, x: self.origin.0, top: self.origin.1, bottom: self.origin.1 - 10.0 };
        };
        let x = match self.boxes.get(i) {
            Some(b) if b.line == li => b.x0,
            _ if i > l.start => self.boxes[i - 1].x1,
            _ => self.origin.0,
        };
        Caret { page: l.page, line: li, x, top: l.top, bottom: l.bottom }
    }

    /// Nearest caret index on line `li` for x (mm).
    pub fn index_on_line(&self, li: usize, x: f32) -> usize {
        let l = self.lines[li];
        for k in l.start..l.end {
            let b = &self.boxes[k];
            if x < (b.x0 + b.x1) / 2.0 {
                return k;
            }
        }
        l.end
    }

    /// Caret index for a point on `page` (mm): the line under y (or the nearest), then nearest x.
    pub fn index_at(&self, page: usize, x: f32, y: f32) -> usize {
        let best = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.page == page)
            .min_by(|(_, a), (_, b)| {
                let d = |l: &LineInfo| if y <= l.top && y >= l.bottom { 0.0 } else { (y - (l.top + l.bottom) / 2.0).abs() };
                d(a).total_cmp(&d(b))
            })
            .map(|(i, _)| i);
        match best {
            Some(li) => self.index_on_line(li, x),
            None => self.boxes.len(),
        }
    }

    /// Caret index one line up (`dir < 0`) or down, keeping x; clamps to start / end of text.
    pub fn vertical_move(&self, i: usize, dir: i32) -> usize {
        let li = self.line_of(i) as i64 + dir as i64;
        if li < 0 {
            return 0;
        }
        if li as usize >= self.lines.len() {
            return self.boxes.len();
        }
        self.index_on_line(li as usize, self.caret(i).x)
    }

    /// (min_x, min_y, max_x, max_y) over every point on every page, in machine mm; `None` when
    /// nothing is drawn. Pages share one coordinate frame (each sheet is drawn from the origin).
    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        let mut it = self.pages.iter().flat_map(|p| p.strokes.iter().flatten());
        let &(x, y) = it.next()?;
        Some(it.fold((x, y, x, y), |(a, b, c, d), &(x, y)| (a.min(x), b.min(y), c.max(x), d.max(y))))
    }

    pub fn stroke_count(&self) -> usize {
        self.pages.iter().map(|p| p.strokes.len()).sum()
    }
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
    /// Line placement; anything but `Free` needs `page`.
    pub ruling: Ruling,
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
            ruling: Ruling::Free,
        }
    }
}

/// Lay out `text` with `font`, using character positions as ids (`0..n`). Unknown characters
/// advance like a space and are listed in `Document::missing`. See [`layout_ids`].
pub fn layout(text: &str, font: &Font, p: &PageParams) -> Document {
    let chars: Vec<char> = text.chars().collect();
    let ids: Vec<u64> = (0..chars.len() as u64).collect();
    layout_ids(&chars, &ids, font, p)
}

/// Pen state while laying out; owns the line/page bookkeeping.
struct Pen<'p> {
    p: &'p PageParams,
    doc: Document,
    page: Page,
    page_ix: usize,
    cx: f32,       // cursor x in mm, relative to the line's left edge
    line_top: f32, // Free ruling: mm below origin for the current line (accumulated)
    line_start: usize,
    /// Line slot on the current page (ruled / grid) and cell column (grid).
    slot: usize,
    col: u32,
    /// Row count of the current page (ruled / grid).
    rows: u32,
    /// Current line geometry, mm relative to origin (x right, y down).
    line_x: f32,
    line_w: f32,
    row_top: f32,
    row_h: f32,
    /// Where glyph tops (glyph y = 0) go on this line.
    text_top: f32,
    em: f32,
}

impl Pen<'_> {
    fn page_box(&self) -> PageBox {
        self.p.page.unwrap_or(PageBox { width: f32::INFINITY, height: f32::INFINITY })
    }

    /// Row count for page `k` (a random line count is drawn per page).
    fn rows_for(&self, k: usize) -> u32 {
        match self.p.ruling {
            Ruling::Rows { rows: (lo, hi), .. } => {
                let lo = lo.max(1);
                if hi <= lo {
                    lo
                } else {
                    let r = rng_for(self.p.style.seed, SALT_ROWS, k as u64).next_u64();
                    lo + (r % u64::from(hi - lo + 1)) as u32
                }
            }
            Ruling::Grid { rows, .. } => rows.max(1),
            Ruling::Free => 0,
        }
    }

    /// Set the current line's geometry from the ruling and slot.
    fn enter_slot(&mut self) {
        let b = self.page_box();
        match self.p.ruling {
            Ruling::Free => {
                (self.line_x, self.line_w) = (0.0, b.width);
                (self.row_top, self.row_h, self.text_top) = (self.line_top, self.p.line_height, self.line_top);
            }
            Ruling::Rows { columns, column_gap, band, .. } => {
                let n = self.rows as usize;
                let cols = columns.max(1) as f32;
                let pitch = b.height / n as f32;
                let col_w = (b.width - column_gap * (cols - 1.0)) / cols;
                let (c, r) = (self.slot / n, self.slot % n);
                self.line_x = c as f32 * (col_w + column_gap);
                self.line_w = col_w;
                self.row_top = r as f32 * pitch;
                self.row_h = pitch;
                // Sit on the rule: the glyph box (em) ends 0.2 em above the row's line. Captured
                // 2026-11-07 (我的笔记1, A4, 24 rows, size 100): text top 1.89 mm into a 10.29 mm row
                // on average; ChiXie3 adds a per-row spread of ≈ ±0.35 mm we do not model.
                let band = band.unwrap_or(0.0).min(pitch);
                let gap = 0.2 * self.em;
                self.text_top = (self.row_top + pitch - self.em - gap).max(self.row_top + band);
            }
            Ruling::Grid { cols, cell: (cw, ch), gap: (cg, rg), .. } => {
                self.line_x = 0.0;
                self.line_w = cols as f32 * (cw + cg);
                self.row_top = self.slot as f32 * (ch + rg);
                self.row_h = ch;
                self.text_top = self.row_top + (ch - self.em) / 2.0;
            }
        }
    }

    /// Ruling guides for the page being started.
    fn page_guides(&mut self) {
        let b = self.page_box();
        let (ox, oy) = self.p.origin;
        let seg = |x0: f32, y0: f32, x1: f32, y1: f32, dashed: bool| Guide { a: (ox + x0, oy - y0), b: (ox + x1, oy - y1), dashed };
        let mut g = Vec::new();
        match self.p.ruling {
            Ruling::Free => {}
            Ruling::Rows { columns, column_gap, band, dashed, divider, .. } => {
                let n = self.rows;
                let cols = columns.max(1);
                let pitch = b.height / n as f32;
                let col_w = (b.width - column_gap * (cols as f32 - 1.0)) / cols as f32;
                for c in 0..cols {
                    let x0 = c as f32 * (col_w + column_gap);
                    for r in 0..=n {
                        let y = r as f32 * pitch;
                        g.push(seg(x0, y, x0 + col_w, y, false));
                        if let Some(band) = band
                            && r < n
                        {
                            let yb = y + band.min(pitch);
                            g.push(seg(x0, yb, x0 + col_w, yb, dashed));
                        }
                    }
                    if divider && c + 1 < cols {
                        let x = x0 + col_w + column_gap / 2.0;
                        g.push(seg(x, 0.0, x, b.height, false));
                    }
                }
            }
            Ruling::Grid { rows, cols, cell: (cw, ch), gap: (cg, rg) } => {
                for r in 0..rows {
                    let y0 = r as f32 * (ch + rg);
                    for c in 0..cols {
                        let x0 = c as f32 * (cw + cg);
                        let (x1, y1) = (x0 + cw, y0 + ch);
                        g.push(seg(x0, y0, x1, y0, false));
                        g.push(seg(x0, y1, x1, y1, false));
                        g.push(seg(x0, y0, x0, y1, false));
                        g.push(seg(x1, y0, x1, y1, false));
                    }
                }
            }
        }
        self.page.guides = g;
    }

    fn new_page(&mut self) {
        self.doc.pages.push(std::mem::take(&mut self.page));
        self.page_ix += 1;
        self.line_top = 0.0;
        self.slot = 0;
        self.rows = self.rows_for(self.page_ix);
        self.page_guides();
    }

    fn close_line(&mut self, end: usize) {
        let (_, oy) = self.p.origin;
        self.doc.lines.push(LineInfo {
            page: self.page_ix,
            top: oy - self.row_top,
            bottom: oy - (self.row_top + self.row_h),
            start: self.line_start,
            end,
        });
    }

    /// Finish the line at caret index `end`; the next line starts at char `next`.
    fn newline(&mut self, end: usize, next: usize) {
        self.close_line(end);
        self.cx = 0.0;
        self.col = 0;
        match self.p.ruling {
            Ruling::Free => {
                self.line_top += self.p.line_height;
                if let Some(b) = self.p.page
                    && self.line_top + self.p.line_height > b.height
                {
                    self.new_page();
                }
            }
            Ruling::Rows { columns, .. } => {
                self.slot += 1;
                if self.slot >= self.rows as usize * columns.max(1) as usize {
                    self.new_page();
                }
            }
            Ruling::Grid { .. } => {
                self.slot += 1;
                if self.slot >= self.rows as usize {
                    self.new_page();
                }
            }
        }
        self.enter_slot();
        self.line_start = next;
    }

    fn push_box(&mut self, x0: f32) {
        let ox = self.p.origin.0 + self.line_x;
        let line = self.doc.lines.len();
        self.doc.boxes.push(CharBox { page: self.page_ix, line, x0: ox + x0, x1: ox + self.cx });
    }
}

/// Place one glyph's strokes with its left edge at `x` (mm from origin) on the current line.
fn draw_glyph(pen: &mut Pen, strokes: &[Vec<(f32, f32)>], x: f32, voff: f32, s: f32) {
    let (ox, oy) = pen.p.origin;
    let top = pen.text_top;
    for stroke in strokes {
        pen.page.strokes.push(stroke.iter().map(|&(gx, gy)| (ox + x + gx * s, oy - (top + voff + gy * s))).collect());
    }
}

/// Lay out `chars` with stable per-character `ids` (same length). Each character's jitter and
/// deformation come from its own id-keyed random stream, so editing one character never moves the
/// others; a line's start offset is keyed by the id of its first character.
///
/// With jitter and deformation off, every coordinate uses the original expressions (see module
/// docs), so the output stays byte-identical to the pre-refactor pipeline.
pub fn layout_ids(chars: &[char], ids: &[u64], font: &Font, p: &PageParams) -> Document {
    assert_eq!(chars.len(), ids.len(), "one id per character");
    let s = p.mm_per_unit;
    let (ox, oy) = p.origin;
    let jitter_on = p.style.enabled;
    let deform_on = p.deformation.is_active();
    let mut jit = Jitter::new(&p.style);

    let mut pen = Pen {
        p,
        doc: Document { origin: p.origin, ..Default::default() },
        page: Page::default(),
        page_ix: 0,
        cx: 0.0,
        line_top: 0.0,
        line_start: 0,
        slot: 0,
        col: 0,
        rows: 0,
        line_x: 0.0,
        line_w: f32::INFINITY,
        row_top: 0.0,
        row_h: p.line_height,
        text_top: 0.0,
        em: EM_UNITS * s,
    };
    pen.rows = pen.rows_for(0);
    pen.enter_slot();
    pen.page_guides();
    let grid = match p.ruling {
        Ruling::Grid { cols, cell, gap, .. } if p.page.is_some() => Some((cols.max(1), cell.0, gap.0)),
        _ => None,
    };
    let mut prev: Option<char> = None;
    // Jitter: the next placed character starts a line (and maybe a paragraph).
    let (mut fresh_line, mut fresh_paragraph) = (true, true);

    for (i, (&ch, &id)) in chars.iter().zip(ids).enumerate() {
        if jitter_on && fresh_line && ch != '\n' {
            pen.cx = jit.line_start(id);
            if fresh_paragraph && let Some(indent) = jit.indent(id) {
                pen.cx += indent;
            }
            fresh_line = false;
            fresh_paragraph = false;
        }
        let x0 = pen.cx;
        if let (Some((cols, cw, cg)), false) = (grid, ch == '\n') {
            // One character per cell, centred; spaces and missing glyphs take a cell too.
            if pen.col >= cols {
                pen.newline(i, i);
            }
            let cell_x = pen.col as f32 * (cw + cg);
            match (ch, font.get(ch)) {
                (' ', _) => {}
                (_, None) => {
                    if !pen.doc.missing.contains(&ch) {
                        pen.doc.missing.push(ch);
                    }
                }
                (_, Some(glyph)) => {
                    let voff = if jitter_on { jit.vertical(id) } else { 0.0 };
                    let x = cell_x + (cw - glyph.advance * s) / 2.0;
                    if deform_on {
                        let mut drng = rng_for(p.style.seed, SALT_DEFORM, id);
                        let strokes = p.deformation.apply(&glyph.strokes, is_cjk(ch), s, &mut drng);
                        draw_glyph(&mut pen, &strokes, x, voff, s);
                    } else {
                        draw_glyph(&mut pen, &glyph.strokes, x, voff, s);
                    }
                }
            }
            pen.cx = cell_x + cw;
            pen.col += 1;
            pen.push_box(cell_x);
            prev = Some(ch);
            continue;
        }
        match ch {
            '\n' => {
                pen.push_box(x0);
                pen.newline(i, i + 1);
                (fresh_line, fresh_paragraph) = (true, true);
                prev = None;
                continue;
            }
            ' ' => {
                if jitter_on {
                    pen.cx += jit.space(prev, id);
                } else {
                    pen.cx += font.get(' ').map(|gl| gl.advance * s).unwrap_or(p.space_width);
                    pen.cx += p.char_spacing;
                }
                pen.push_box(x0);
                continue;
            }
            _ => {}
        }
        let Some(glyph) = font.get(ch) else {
            if !pen.doc.missing.contains(&ch) {
                pen.doc.missing.push(ch);
            }
            if jitter_on {
                pen.cx += jit.space(Some(ch), id);
            } else {
                pen.cx += p.space_width + p.char_spacing;
            }
            pen.push_box(x0);
            prev = Some(ch);
            continue;
        };
        // Wrap before a glyph that would cross the right edge (never on an empty line).
        if p.page.is_some() && pen.cx > 0.0 && pen.cx + glyph.advance * s > pen.line_w {
            pen.newline(i, i);
            if jitter_on {
                pen.cx = jit.line_start(id);
            }
        }
        let x0 = pen.cx;
        // `line_x + cx` is exactly `cx` on Free ruling (line_x = 0), keeping the golden bytes.
        let (cx, line_top) = (pen.line_x + pen.cx, pen.text_top);
        if !jitter_on && !deform_on {
            for stroke in &glyph.strokes {
                pen.page.strokes.push(
                    stroke
                        .iter()
                        .map(|&(gx, gy)| (ox + cx + gx * s, oy - (line_top + gy * s)))
                        .collect(),
                );
            }
            pen.cx += glyph.advance * s + p.char_spacing;
        } else {
            let voff = if jitter_on { jit.vertical(id) } else { 0.0 };
            let mut drng = rng_for(p.style.seed, SALT_DEFORM, id);
            let strokes = p.deformation.apply(&glyph.strokes, is_cjk(ch), s, &mut drng);
            for stroke in &strokes {
                pen.page.strokes.push(
                    stroke
                        .iter()
                        .map(|&(gx, gy)| (ox + cx + gx * s, oy - (line_top + voff + gy * s)))
                        .collect(),
                );
            }
            pen.cx += glyph.advance * s + if jitter_on { jit.spacing(ch, id) } else { p.char_spacing };
        }
        pen.push_box(x0);
        prev = Some(ch);
    }
    pen.close_line(chars.len());
    let mut doc = pen.doc;
    doc.pages.push(pen.page);
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

    fn dot_font() -> Font {
        // '.' : one point, advance 2; 'A' : advance 3. Simple numbers for box checks.
        Font {
            glyphs: HashMap::from([
                ('.', Glyph { advance: 2.0, strokes: vec![vec![(0.0, 0.0)]] }),
                ('A', Glyph { advance: 3.0, strokes: vec![vec![(0.0, 0.0), (1.0, 1.0)]] }),
            ]),
        }
    }

    #[test]
    fn boxes_and_lines_cover_every_char() {
        let p = PageParams { space_width: 1.0, ..Default::default() };
        let doc = layout(".A \n.x", &dot_font(), &p);
        // . A ' ' \n . x  -> 6 boxes; 'x' is missing (space width).
        assert_eq!(doc.boxes.len(), 6);
        let xs: Vec<(f32, f32)> = doc.boxes.iter().map(|b| (b.x0, b.x1)).collect();
        assert_eq!(xs, vec![(0.0, 2.0), (2.0, 5.0), (5.0, 6.0), (6.0, 6.0), (0.0, 2.0), (2.0, 3.0)]);
        assert_eq!(doc.lines.len(), 2);
        assert_eq!((doc.lines[0].start, doc.lines[0].end), (0, 3)); // ends at the '\n'
        assert_eq!((doc.lines[1].start, doc.lines[1].end), (4, 6));
        assert_eq!(doc.lines[1].top, -10.0);
    }

    #[test]
    fn caret_and_hit_test_round_trip() {
        let doc = layout("..\n.", &dot_font(), &PageParams::default());
        // Caret x for every index; index 2 is end of line 0 (before '\n').
        let carets: Vec<(usize, f32)> = (0..=4).map(|i| (doc.caret(i).line, doc.caret(i).x)).collect();
        assert_eq!(carets, vec![(0, 0.0), (0, 2.0), (0, 4.0), (1, 0.0), (1, 2.0)]);
        // Clicking left/right halves of a glyph picks the nearer boundary.
        assert_eq!(doc.index_at(0, 0.9, -5.0), 0);
        assert_eq!(doc.index_at(0, 1.1, -5.0), 1);
        assert_eq!(doc.index_at(0, 99.0, -5.0), 2);
        assert_eq!(doc.index_at(0, 99.0, -15.0), 4);
        // Up/down keep x.
        assert_eq!(doc.vertical_move(1, 1), 4);
        assert_eq!(doc.vertical_move(4, -1), 1);
        assert_eq!(doc.vertical_move(0, -1), 0);
    }

    #[test]
    fn wrap_boundary_caret_belongs_to_next_line() {
        let p = PageParams { page: Some(PageBox { width: 5.0, height: 100.0 }), ..Default::default() };
        let doc = layout("...", &dot_font(), &p); // 2 dots per line
        assert_eq!(doc.lines.len(), 2);
        assert_eq!((doc.lines[0].start, doc.lines[0].end), (0, 2));
        assert_eq!(doc.caret(2).line, 1);
        assert_eq!(doc.caret(2).x, 0.0);
    }

    #[test]
    fn empty_text_has_one_line_and_origin_caret() {
        let doc = layout("", &dot_font(), &PageParams { origin: (3.0, 7.0), ..Default::default() });
        assert_eq!(doc.lines.len(), 1);
        assert_eq!(doc.caret(0).x, 3.0);
        assert_eq!(doc.caret(0).top, 7.0);
    }

    #[test]
    fn editing_does_not_reshuffle_other_characters() {
        let font = Font::from_json(FIXTURE).unwrap();
        let p = PageParams {
            style: HandwritingStyle { enabled: true, seed: 5, ..Default::default() },
            deformation: Deformation { enabled: true, base: true, ..Default::default() },
            ..Default::default()
        };
        // Same characters with the same ids, one inserted at the front with a new id.
        let base: Vec<char> = "你好你好".chars().collect();
        let ids: Vec<u64> = vec![10, 11, 12, 13];
        let a = layout_ids(&base, &ids, &font, &p);
        let mut chars2 = vec!['A'];
        chars2.extend(&base);
        let mut ids2 = vec![99];
        ids2.extend(&ids);
        let b = layout_ids(&chars2, &ids2, &font, &p);
        // Each original glyph keeps its shape (strokes relative to the glyph's own first point);
        // only its position shifts. 'A' adds 2 strokes in front.
        let rel = |d: &Document, skip: usize| -> Vec<Vec<(i32, i32)>> {
            d.pages[0].strokes[skip..]
                .iter()
                .map(|s| s.iter().map(|&(x, y)| (((x - s[0].0) * 1000.0) as i32, ((y - s[0].1) * 1000.0) as i32)).collect())
                .collect()
        };
        assert_eq!(rel(&a, 0), rel(&b, 2));
    }

    #[test]
    fn bounds_cover_all_pages() {
        let doc = Document {
            pages: vec![Page { strokes: vec![vec![(1.0, -2.0), (5.0, -1.0)]], ..Default::default() }, Page { strokes: vec![vec![(0.5, -9.0)]], ..Default::default() }],
            ..Default::default()
        };
        assert_eq!(doc.bounds(), Some((0.5, -9.0, 5.0, -1.0)));
        assert_eq!(doc.stroke_count(), 2);
        assert_eq!(Document::default().bounds(), None);
    }

    #[test]
    fn flips_y_and_scales() {
        let g = Glyph { advance: 3.0, strokes: vec![vec![(0.0, 0.0), (0.0, 2.0)]] };
        let p = PageParams { origin: (10.0, 50.0), ..Default::default() };
        let doc = layout("I", &font_with('I', g), &p);
        assert_eq!(doc.pages, vec![Page { strokes: vec![vec![(10.0, 50.0), (10.0, 48.0)]], ..Default::default() }]);
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
