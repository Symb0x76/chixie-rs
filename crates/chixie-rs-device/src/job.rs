//! G-code for the page tools that are not text: 工作区巡边 (trace the work area) and 绘制框架线
//! (draw the template's ruling). Pure functions; the result runs as a one-page [`Job`].
//!
//! [`Job`]: crate::device::Job

use std::fmt::Write as _;

use chixie_rs_core::gcode::{self, DeviceProfile};
use chixie_rs_core::layout::{Guide, Page, Stroke};

/// 巡边模式.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatrolMode {
    /// 抬笔巡边: trace the rectangle with the pen up (checks paper placement, leaves no mark).
    Up,
    /// 落笔巡边: draw the rectangle.
    Down,
    /// 抬笔巡边（顶点落笔）: travel pen up, touch down only at the four corners.
    UpCornersDown,
}

impl PatrolMode {
    pub const ALL: [PatrolMode; 3] = [PatrolMode::Up, PatrolMode::Down, PatrolMode::UpCornersDown];
    pub fn label(self) -> &'static str {
        match self {
            PatrolMode::Up => "抬笔巡边",
            PatrolMode::Down => "落笔巡边",
            PatrolMode::UpCornersDown => "抬笔巡边（顶点落笔）",
        }
    }
}

/// Axis-aligned rectangle `(x0, y0, x1, y1)` in page (logical) mm.
pub type Rect = (f32, f32, f32, f32);

/// Bounding box of a page's written strokes (实际书写区域).
pub fn written_area(page: &Page) -> Option<Rect> {
    bbox(page.strokes.iter().flatten().copied())
}

/// Bounding box of a page's template ruling (模板框线区域).
pub fn template_area(page: &Page) -> Option<Rect> {
    bbox(page.guides.iter().flat_map(|g| [g.a, g.b]))
}

fn bbox(mut pts: impl Iterator<Item = (f32, f32)>) -> Option<Rect> {
    let (x, y) = pts.next()?;
    Some(pts.fold((x, y, x, y), |(a, b, c, d), (x, y)| (a.min(x), b.min(y), c.max(x), d.max(y))))
}

/// Trace `r` and come back to the origin.
pub fn patrol(r: Rect, mode: PatrolMode, d: &DeviceProfile) -> String {
    let (x0, y0, x1, y1) = r;
    let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    let mut g = match mode {
        PatrolMode::Down => gcode::emit(&page_of(vec![corners.iter().chain(&corners[..1]).copied().collect()]), d),
        PatrolMode::UpCornersDown => gcode::emit(&page_of(corners.iter().map(|&c| vec![c]).collect()), d),
        PatrolMode::Up => {
            let mut g = format!("{} F{:.0}\n", d.pen_up_cmd(), d.travel_speed);
            for &p in corners.iter().chain(&corners[..1]) {
                let (x, y) = d.axis_map.apply(p);
                let _ = writeln!(g, "G1 X{x:.3} Y{y:.3} F{:.0}", d.travel_speed);
            }
            g
        }
    };
    g.push_str("G90G0 X0Y0\n");
    g
}

/// Dash and gap length (mm) for dashed guides (田字格 / 米字格 helper lines).
const DASH: f32 = 2.0;
const GAP: f32 = 2.0;
/// Coordinates closer than this are the same line / touching ends.
const EPS: f32 = 0.01;

/// Draw the ruling: collinear touching segments merged into one stroke; horizontal lines top to
/// bottom, then vertical lines left to right, each row reversing direction (boustrophedon) so
/// travel between lines stays short; dashed guides drawn as dashes.
pub fn ruling(guides: &[Guide], d: &DeviceProfile) -> String {
    let horizontal = |g: &Guide| (g.a.1 - g.b.1).abs() < EPS;
    let vertical = |g: &Guide| (g.a.0 - g.b.0).abs() < EPS && !horizontal(g);
    // (fixed coordinate, from, to, dashed), from <= to.
    let lines = |pick: &dyn Fn(&Guide) -> bool, fixed: fn(&Guide) -> f32, along: fn((f32, f32)) -> f32| {
        let mut v: Vec<(f32, f32, f32, bool)> = guides
            .iter()
            .filter(|g| pick(g))
            .map(|g| {
                let (a, b) = (along(g.a), along(g.b));
                (fixed(g), a.min(b), a.max(b), g.dashed)
            })
            .collect();
        v.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.3.cmp(&q.3)).then(p.1.total_cmp(&q.1)));
        merge(v)
    };
    let rows = lines(&horizontal, |g| g.a.1, |p| p.0);
    let cols = lines(&vertical, |g| g.a.0, |p| p.1);

    let mut strokes: Vec<Stroke> = Vec::new();
    // Rows: top (largest y) first.
    for (k, &(y, a, b, dashed)) in rows.iter().rev().enumerate() {
        let (from, to) = if k % 2 == 0 { (a, b) } else { (b, a) };
        strokes.extend(pieces(from, to, dashed).into_iter().map(|(s, e)| vec![(s, y), (e, y)]));
    }
    for (k, &(x, a, b, dashed)) in cols.iter().enumerate() {
        let (from, to) = if k % 2 == 0 { (b, a) } else { (a, b) };
        strokes.extend(pieces(from, to, dashed).into_iter().map(|(s, e)| vec![(x, s), (x, e)]));
    }
    for g in guides.iter().filter(|g| !horizontal(g) && !vertical(g)) {
        strokes.push(vec![g.a, g.b]);
    }
    gcode::emit(&page_of(strokes), d)
}

/// Merge sorted same-line segments whose ends touch or overlap (same dash style only).
fn merge(v: Vec<(f32, f32, f32, bool)>) -> Vec<(f32, f32, f32, bool)> {
    let mut out: Vec<(f32, f32, f32, bool)> = Vec::new();
    for s in v {
        match out.last_mut() {
            Some(l) if (l.0 - s.0).abs() < EPS && l.3 == s.3 && s.1 <= l.2 + EPS => l.2 = l.2.max(s.2),
            _ => out.push(s),
        }
    }
    out
}

/// `from → to` as one piece, or as dashes when `dashed`.
fn pieces(from: f32, to: f32, dashed: bool) -> Vec<(f32, f32)> {
    if !dashed {
        return vec![(from, to)];
    }
    let (len, dir) = ((to - from).abs(), (to - from).signum());
    let mut out = Vec::new();
    let mut t = 0.0;
    while t < len - EPS {
        out.push((from + dir * t, from + dir * (t + DASH).min(len)));
        t += DASH + GAP;
    }
    out
}

fn page_of(strokes: Vec<Stroke>) -> Page {
    Page { strokes, ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d() -> DeviceProfile {
        DeviceProfile::default()
    }

    #[test]
    fn patrol_up_never_lowers_the_pen() {
        let g = patrol((0.0, -50.0, 100.0, 0.0), PatrolMode::Up, &d());
        assert!(!g.contains("Z2"), "{g}");
        let lines: Vec<&str> = g.lines().collect();
        assert_eq!(lines[0], "G1G90 Z0 F8000");
        assert_eq!(lines[1], "G1 X0.000 Y-50.000 F8000");
        assert_eq!(lines[5], "G1 X0.000 Y-50.000 F8000"); // closed
        assert_eq!(*lines.last().unwrap(), "G90G0 X0Y0");
    }

    #[test]
    fn patrol_down_draws_one_closed_stroke() {
        let g = patrol((0.0, 0.0, 10.0, 10.0), PatrolMode::Down, &d());
        assert_eq!(g.matches("G1G90 Z2").count(), 1);
        assert_eq!(g.matches("G1 X").count(), 5);
    }

    #[test]
    fn patrol_corners_touch_four_times() {
        let g = patrol((0.0, 0.0, 10.0, 10.0), PatrolMode::UpCornersDown, &d());
        assert_eq!(g.matches("G1G90 Z2").count(), 4);
        assert_eq!(g.lines().filter(|l| l.starts_with("G0 X")).count(), 4);
    }

    #[test]
    fn patrol_follows_axis_map() {
        let mut p = d();
        p.axis_map.invert_x = true;
        let g = patrol((0.0, 0.0, 10.0, 5.0), PatrolMode::Up, &p);
        assert!(g.contains("G1 X-10.000 Y0.000"), "{g}");
    }

    fn seg(a: (f32, f32), b: (f32, f32)) -> Guide {
        Guide { a, b, dashed: false }
    }

    #[test]
    fn ruling_merges_and_alternates() {
        // Two halves of the top row, one lower row, one column.
        let guides = [seg((0.0, -10.0), (50.0, -10.0)), seg((50.0, -10.0), (100.0, -10.0)), seg((0.0, -20.0), (100.0, -20.0)), seg((30.0, -10.0), (30.0, -20.0))];
        let g = ruling(&guides, &d());
        assert_eq!(g.matches("G1G90 Z2").count(), 3, "{g}");
        let travels: Vec<&str> = g.lines().filter(|l| l.starts_with("G0")).collect();
        // Row 1 left→right, row 2 right→left, then the column top→bottom.
        assert_eq!(travels, ["G0 X0.000 Y-10.000", "G0 X100.000 Y-20.000", "G0 X30.000 Y-10.000"]);
    }

    #[test]
    fn dashed_guides_become_dashes() {
        let g = ruling(&[Guide { a: (0.0, 0.0), b: (10.0, 0.0), dashed: true }], &d());
        assert_eq!(g.matches("G1G90 Z2").count(), 3); // 0–2, 4–6, 8–10
    }

    #[test]
    fn areas() {
        let page = Page { strokes: vec![vec![(1.0, -2.0), (5.0, -1.0)]], guides: vec![seg((0.0, 0.0), (100.0, -80.0))] };
        assert_eq!(written_area(&page), Some((1.0, -2.0, 5.0, -1.0)));
        assert_eq!(template_area(&page), Some((0.0, -80.0, 100.0, 0.0)));
        assert_eq!(written_area(&Page::default()), None);
    }
}
