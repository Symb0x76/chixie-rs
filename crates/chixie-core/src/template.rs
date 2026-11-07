//! Page templates: pre-printed paper the text is laid out on (ChiXie3 模板管理).
//!
//! - `Blank`: free lines every `line_height` (the default page).
//! - `Ruled`: letter paper with N rule lines per page (fixed, or a random count per page),
//!   optional dual-line band and equal columns.
//! - `Grid`: composition paper (作文格), one character per cell.
//!
//! Templates only position text; their ruling is drawn in the editor but never plotted.
//! Stored as `templates.toml` next to `settings.toml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::Ruling;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    /// (width, height) mm.
    pub paper: (f32, f32),
    /// (left, top, right, bottom) mm: the writable area. For `Grid` the right/bottom margins
    /// follow from the grid size (`Template::grid_margins`).
    pub margins: (f32, f32, f32, f32),
    pub kind: TemplateKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemplateKind {
    Blank,
    Ruled {
        /// Rule lines per page; `rows_min < rows_max` draws a count per page (随机行数).
        rows_min: u32,
        rows_max: u32,
        /// Dual-line mode: upper band height (mm) above each writing line.
        dual_line: Option<f32>,
        /// Draw the dual-line partition dashed.
        dashed: bool,
        columns: u32,
        column_gap: f32,
        /// Vertical divider between columns.
        divider: bool,
    },
    Grid {
        rows: u32,
        cols: u32,
        cell_w: f32,
        cell_h: f32,
        row_gap: f32,
        col_gap: f32,
    },
}

impl Template {
    pub fn blank(name: &str, paper: (f32, f32)) -> Self {
        Self { name: name.into(), paper, margins: (21.0, 20.0, 21.0, 20.0), kind: TemplateKind::Blank }
    }

    /// Ruled letter paper with a fixed line count.
    pub fn ruled(name: &str, paper: (f32, f32), margins: (f32, f32, f32, f32), rows: u32) -> Self {
        let kind = TemplateKind::Ruled {
            rows_min: rows,
            rows_max: rows,
            dual_line: None,
            dashed: true,
            columns: 1,
            column_gap: 6.0,
            divider: true,
        };
        Self { name: name.into(), paper, margins, kind }
    }

    /// Composition grid placed `left`/`top` from the paper edge.
    #[allow(clippy::too_many_arguments)]
    pub fn grid(name: &str, paper: (f32, f32), left: f32, top: f32, rows: u32, cols: u32, cell: (f32, f32), gap: (f32, f32)) -> Self {
        let kind = TemplateKind::Grid { rows, cols, cell_w: cell.0, cell_h: cell.1, row_gap: gap.1, col_gap: gap.0 };
        let mut t = Self { name: name.into(), paper, margins: (left, top, 0.0, 0.0), kind };
        t.margins = t.grid_margins();
        t
    }

    /// Margins for a grid: left/top as set, right/bottom = what the grid leaves of the paper.
    pub fn grid_margins(&self) -> (f32, f32, f32, f32) {
        let (l, t, ..) = self.margins;
        match self.kind {
            TemplateKind::Grid { rows, cols, cell_w, cell_h, row_gap, col_gap } => {
                let w = cols as f32 * cell_w + cols.saturating_sub(1) as f32 * col_gap;
                let h = rows as f32 * cell_h + rows.saturating_sub(1) as f32 * row_gap;
                (l, t, (self.paper.0 - l - w).max(0.0), (self.paper.1 - t - h).max(0.0))
            }
            _ => self.margins,
        }
    }

    /// Layout ruling for this template.
    pub fn ruling(&self) -> Ruling {
        match self.kind {
            TemplateKind::Blank => Ruling::Free,
            TemplateKind::Ruled { rows_min, rows_max, dual_line, dashed, columns, column_gap, divider } => Ruling::Rows {
                rows: (rows_min.max(1), rows_max.max(rows_min).max(1)),
                columns: columns.max(1),
                column_gap,
                band: dual_line,
                dashed,
                divider,
            },
            TemplateKind::Grid { rows, cols, cell_w, cell_h, row_gap, col_gap } => {
                Ruling::Grid { rows: rows.max(1), cols: cols.max(1), cell: (cell_w, cell_h), gap: (col_gap, row_gap) }
            }
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            TemplateKind::Blank => "空白",
            TemplateKind::Ruled { .. } => "信纸",
            TemplateKind::Grid { .. } => "作文格",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TemplateStore {
    #[serde(default, rename = "template")]
    pub templates: Vec<Template>,
}

impl TemplateStore {
    /// Built-in starting set (also what a missing file yields).
    pub fn builtins() -> Self {
        Self {
            templates: vec![
                Template::blank("空白 A4", (210.0, 297.0)),
                Template::ruled("信纸 22 行", (160.0, 219.8), (1.5, 1.0, 1.5, 1.0), 22),
                Template::grid("作文格 20×20", (190.0, 255.0), 15.0, 20.0, 20, 20, (8.0, 8.0), (0.0, 2.9)),
            ],
        }
    }

    pub fn path_next_to(settings_path: &Path) -> PathBuf {
        settings_path.parent().unwrap_or_else(|| Path::new(".")).join("templates.toml")
    }

    /// Load; a missing file yields the built-ins.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => toml::from_str(&s).map_err(|e| Error::Config(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::builtins()),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomic save (write a temp file, then rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| Error::Config(e.to_string()))?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Template> {
        self.templates.iter().find(|t| t.name == name)
    }

    /// `base`, or `base (2)`, `base (3)`… whichever is free.
    pub fn unique_name(&self, base: &str) -> String {
        if self.get(base).is_none() {
            return base.to_string();
        }
        (2..).map(|n| format!("{base} ({n})")).find(|n| self.get(n).is_none()).expect("unbounded")
    }

    /// Add or replace by name; returns the index.
    pub fn upsert(&mut self, t: Template) -> usize {
        match self.templates.iter().position(|x| x.name == t.name) {
            Some(i) => {
                self.templates[i] = t;
                i
            }
            None => {
                self.templates.push(t);
                self.templates.len() - 1
            }
        }
    }

    pub fn remove(&mut self, name: &str) -> Option<Template> {
        let i = self.templates.iter().position(|t| t.name == name)?;
        Some(self.templates.remove(i))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::font::{Font, Glyph};
    use crate::layout::{PageBox, PageParams, layout};

    /// 'x' is a 7×7 box glyph with advance 7.
    fn font() -> Font {
        let g = Glyph { advance: 7.0, strokes: vec![vec![(0.0, 0.0), (7.0, 0.0), (7.0, 7.0), (0.0, 7.0)]] };
        Font { glyphs: HashMap::from([('x', g)]) }
    }

    fn params(t: &Template) -> PageParams {
        let (l, top, r, b) = t.margins;
        PageParams {
            page: Some(PageBox { width: t.paper.0 - l - r, height: t.paper.1 - top - b }),
            ruling: t.ruling(),
            ..Default::default()
        }
    }

    #[test]
    fn ruled_rows_set_pitch_and_page_breaks() {
        // 100 mm writable height, 10 rows → 10 mm pitch; 12 lines → 2 pages.
        let t = Template::ruled("r", (120.0, 120.0), (10.0, 10.0, 10.0, 10.0), 10);
        let text = ["x"; 12].join("\n");
        let doc = layout(&text, &font(), &params(&t));
        assert_eq!(doc.pages.len(), 2);
        assert_eq!(doc.lines[0].top, 0.0);
        assert_eq!(doc.lines[1].top, -10.0);
        assert_eq!(doc.lines[10].page, 1);
        assert_eq!(doc.lines[10].top, 0.0);
        // Glyph bottom sits above the row's rule (y = -10) and inside the row.
        let ys: Vec<f32> = doc.pages[0].strokes[0].iter().map(|p| p.1).collect();
        let (lo, hi) = ys.iter().fold((f32::MAX, f32::MIN), |(a, b), &y| (a.min(y), b.max(y)));
        assert!(lo > -10.0 && hi < 0.0 && lo < -8.0, "glyph y range {lo}..{hi}");
        // Ruling: 11 rules on page 0 (top + each row bottom), none ever in strokes.
        assert_eq!(doc.pages[0].guides.len(), 11);
    }

    #[test]
    fn random_rows_are_per_page_and_deterministic() {
        let mut t = Template::ruled("r", (120.0, 220.0), (10.0, 10.0, 10.0, 10.0), 10);
        if let TemplateKind::Ruled { rows_min, rows_max, .. } = &mut t.kind {
            (*rows_min, *rows_max) = (18, 24);
        }
        let text = vec!["x"; 200].join("\n");
        let a = layout(&text, &font(), &params(&t));
        let b = layout(&text, &font(), &params(&t));
        assert_eq!(a, b);
        let counts: Vec<usize> = (0..a.pages.len()).map(|k| a.lines.iter().filter(|l| l.page == k).count()).collect();
        for c in &counts[..counts.len() - 1] {
            assert!((18..=24).contains(c), "rows {c}");
        }
        assert!(counts.windows(2).any(|w| w[0] != w[1]), "row counts should vary: {counts:?}");
    }

    #[test]
    fn columns_fill_top_to_bottom_then_right() {
        let mut t = Template::ruled("r", (120.0, 120.0), (10.0, 10.0, 10.0, 10.0), 5);
        if let TemplateKind::Ruled { columns, column_gap, .. } = &mut t.kind {
            (*columns, *column_gap) = (2, 10.0);
        }
        let text = ["x"; 7].join("\n");
        let doc = layout(&text, &font(), &params(&t));
        assert_eq!(doc.pages.len(), 1);
        // Column width (100 - 10) / 2 = 45; line 5 starts the second column.
        assert_eq!(doc.boxes[doc.lines[4].start].x0, 0.0);
        assert_eq!(doc.boxes[doc.lines[5].start].x0, 55.0);
        assert_eq!(doc.lines[5].top, 0.0);
    }

    #[test]
    fn grid_centres_one_char_per_cell_and_wraps() {
        let t = Template::grid("g", (100.0, 100.0), 10.0, 10.0, 3, 4, (10.0, 10.0), (0.0, 2.0));
        assert_eq!(t.margins, (10.0, 10.0, 50.0, 56.0)); // grid 40 × 34 mm
        let doc = layout("xxxxx x", &font(), &params(&t));
        // 7 chars, 4 per row → 2 rows; 5th char starts row 2.
        assert_eq!(doc.lines.len(), 2);
        assert_eq!(doc.boxes[4].x0, 0.0);
        assert_eq!(doc.lines[1].top, -12.0);
        // 'x' (7 mm) centred in a 10 mm cell: left edge 1.5 mm into the cell.
        let x0 = doc.pages[0].strokes[1].iter().map(|p| p.0).fold(f32::MAX, f32::min);
        assert!((x0 - 11.5).abs() < 1e-4, "x0 {x0}");
        // The space took a cell; 3×4 cells × 4 edges of guides.
        assert_eq!(doc.boxes[5].x0, 10.0);
        assert_eq!(doc.pages[0].guides.len(), 48);
    }

    fn fixture_font() -> Font {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/font_fixture.json");
        Font::from_json(&std::fs::read(p).unwrap()).unwrap()
    }

    /// Ink top/bottom (mm, y up) of the strokes drawn for one glyph.
    fn ink_y(strokes: &[Vec<(f32, f32)>]) -> (f32, f32) {
        strokes.iter().flatten().fold((f32::MIN, f32::MAX), |(t, b), p| (t.max(p.1), b.min(p.1)))
    }

    /// ChiXie3 capture 2026-11-07, 我的笔记1 (A4, margins 20/30/20/20, 24 rows), 艾玛体 100,
    /// deformation off: 你 ink tops per row were −2.565, −22.424 (rows 1 and 3), bottoms −8.836,
    /// −28.777. ChiXie3 adds ≤ 0.3 mm vertical jitter plus a per-row spread (row residuals vs our
    /// mean model: +0.31, +0.09, −0.40 mm), so allow 0.6 mm.
    #[test]
    fn ruled_rows_match_chixie3_capture() {
        let t = Template::ruled("我的笔记1", (210.0, 297.0), (20.0, 30.0, 20.0, 20.0), 24);
        let doc = layout("你\n\n你", &fixture_font(), &params(&t));
        let (t1, b1) = ink_y(&doc.pages[0].strokes[..6]); // 你 = 6 strokes
        let (t3, b3) = ink_y(&doc.pages[0].strokes[6..]);
        for (ours, theirs) in [(t1, -2.565), (b1, -8.836), (t3, -22.424), (b3, -28.777)] {
            assert!((ours - theirs).abs() < 0.6, "ours {ours} vs captured {theirs}");
        }
    }

    /// ChiXie3 capture 2026-11-07, 作文格1 (8 mm cells, row gap 2.9): 你 in the 4th cell spans
    /// x 24.97..32.29, y −1.49..−7.85 (incl. ≤ 0.3 mm jitter): centred in its cell.
    #[test]
    fn grid_cells_match_chixie3_capture() {
        let t = Template::grid("作文格1", (190.0, 255.0), 15.0, 20.0, 20, 20, (8.0, 8.0), (0.0, 2.9));
        let doc = layout("好好好你", &fixture_font(), &params(&t));
        let ni = &doc.pages[0].strokes[doc.pages[0].strokes.len() - 6..]; // 你 = 6 strokes
        let x0 = ni.iter().flatten().map(|p| p.0).fold(f32::MAX, f32::min);
        let (top, bottom) = ink_y(ni);
        for (ours, theirs) in [(x0, 24.97), (top, -1.485), (bottom, -7.851)] {
            assert!((ours - theirs).abs() < 0.7, "ours {ours} vs captured {theirs}");
        }
    }

    #[test]
    fn store_round_trips_and_names_are_unique() {
        let dir = std::env::temp_dir().join("chixie-core-test-templates");
        let path = dir.join("templates.toml");
        let _ = std::fs::remove_file(&path);
        assert_eq!(TemplateStore::load(&path).unwrap(), TemplateStore::builtins());
        let mut s = TemplateStore::builtins();
        let name = s.unique_name("信纸 22 行");
        assert_eq!(name, "信纸 22 行 (2)");
        s.upsert(Template::ruled(&name, (160.0, 220.0), (1.0, 1.0, 1.0, 1.0), 20));
        s.save(&path).unwrap();
        assert_eq!(TemplateStore::load(&path).unwrap(), s);
        assert!(s.remove(&name).is_some());
    }
}
