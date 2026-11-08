//! Character deformation (ChiXie3 设置 → 字符变形), modelled from captures (docs/protocol.md):
//!
//! - **Base** (基础变形): per-glyph random width/height scale (percent) and rotation (degrees)
//!   about the glyph centre, plus a small per-stroke shift (笔画变形幅度). Captures: scales near
//!   96–104%, rotation within ±2°, 0.02–0.06 mm stroke-level residual at amplitude 1.
//! - **Shape** (字形变形): CJK glyphs get a consistent slant (shear ≈ −0.12…−0.13 at amplitude 7)
//!   plus smooth non-affine noise (0.035–0.106 mm residual at 7). Latin was barely affected.
//!
//! The stroke-shift and shape-noise *forms* are best guesses that match the measured magnitudes;
//! their exact formulas are VERIFY. "Remove small tails" is not modelled (no effect seen).

use serde::{Deserialize, Serialize};

use crate::handwriting::{Rng, Span};
use crate::layout::Stroke;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Deformation {
    pub enabled: bool,
    pub base: bool,
    pub shape: bool,
    /// Width scale, percent (ChiXie3 字符随机胖瘦).
    pub h_scale_pct: Span,
    /// Height scale, percent (ChiXie3 字符随机高矮).
    pub v_scale_pct: Span,
    /// Rotation, degrees; positive = counter-clockwise on paper.
    pub rotation_deg: Span,
    /// Stroke deformation amplitude (笔画变形幅度), slider 0–2; 0 disables the stroke shift.
    pub stroke_degree: f32,
    /// Shape deformation amplitude (字形变形幅度), slider 0–10; 0 disables shape deformation.
    pub shape_degree: f32,
}

impl Default for Deformation {
    /// ChiXie3's shipped defaults, disabled.
    fn default() -> Self {
        Self {
            enabled: false,
            base: false,
            shape: false,
            h_scale_pct: Span::new(96.0, 104.0),
            v_scale_pct: Span::new(96.0, 104.0),
            rotation_deg: Span::new(-2.0, 2.0),
            stroke_degree: 1.0,
            shape_degree: 7.0,
        }
    }
}

/// Max per-stroke shift in mm per unit of `stroke_degree` (captures: ~0.02–0.06 mm RMS at 1).
const STROKE_SHIFT_MM: f32 = 0.06;
/// Shear per unit of `shape_degree` (captures: −0.118…−0.134 at 7).
const SHAPE_SHEAR: Span = Span::new(0.016, 0.0195);
/// Smooth-noise amplitude in mm per unit of `shape_degree` (captures: up to ~0.1 mm at 7).
const SHAPE_NOISE_MM: f32 = 0.012;

impl Deformation {
    pub fn is_active(&self) -> bool {
        self.enabled && (self.base || (self.shape && self.shape_degree > 0.0))
    }

    /// Deform one glyph's strokes (glyph units, y down). `mm_per_unit` converts the mm-valued
    /// stroke shift and noise into glyph units. Draws from `rng` in a fixed order.
    pub fn apply(&self, strokes: &[Stroke], cjk: bool, mm_per_unit: f32, rng: &mut Rng) -> Vec<Stroke> {
        let mut out: Vec<Stroke> = strokes.to_vec();
        if !self.is_active() || out.iter().all(|s| s.is_empty()) {
            return out;
        }
        let (cx, cy, w, h) = bbox(&out);
        let unit = |mm: f32| if mm_per_unit > 0.0 { mm / mm_per_unit } else { 0.0 };

        if self.base {
            let sx = self.h_scale_pct.sample(rng) / 100.0;
            let sy = self.v_scale_pct.sample(rng) / 100.0;
            // Glyph space is y-down, so a counter-clockwise turn on paper is clockwise here.
            let th = -self.rotation_deg.sample(rng).to_radians();
            let (sin, cos) = th.sin_cos();
            for p in out.iter_mut().flatten() {
                let (dx, dy) = ((p.0 - cx) * sx, (p.1 - cy) * sy);
                *p = (cx + dx * cos - dy * sin, cy + dx * sin + dy * cos);
            }
            if self.stroke_degree > 0.0 {
                for s in &mut out {
                    let r = unit(STROKE_SHIFT_MM * self.stroke_degree) * rng.unit();
                    let a = rng.unit() * std::f32::consts::TAU;
                    let (ox, oy) = (r * a.cos(), r * a.sin());
                    for p in s.iter_mut() {
                        *p = (p.0 + ox, p.1 + oy);
                    }
                }
            }
        }

        if self.shape && self.shape_degree > 0.0 && cjk {
            // Slant: on paper x shifts right as y goes up; glyph y is down, so use (cy - y).
            let k = SHAPE_SHEAR.sample(rng) * self.shape_degree;
            let amp = unit(SHAPE_NOISE_MM * self.shape_degree);
            let (p1, p2) = (rng.unit() * std::f32::consts::TAU, rng.unit() * std::f32::consts::TAU);
            let (w, h) = (w.max(1e-3), h.max(1e-3));
            for p in out.iter_mut().flatten() {
                let (x, y) = *p;
                let nx = amp * (std::f32::consts::TAU * (y - cy) / h + p1).sin();
                let ny = amp * (std::f32::consts::TAU * (x - cx) / w + p2).sin();
                *p = (x + k * (cy - y) + nx, y + ny);
            }
        }
        out
    }
}

/// (centre x, centre y, width, height) of all points.
fn bbox(strokes: &[Stroke]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in strokes.iter().flatten() {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    ((x0 + x1) / 2.0, (y0 + y1) / 2.0, x1 - x0, y1 - y0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<Stroke> {
        vec![vec![(0.0, 0.0), (4.0, 0.0), (4.0, 6.0), (0.0, 6.0), (0.0, 0.0)]]
    }

    #[test]
    fn disabled_is_identity() {
        let d = Deformation { base: true, ..Default::default() }; // enabled = false
        assert_eq!(d.apply(&square(), true, 1.0, &mut Rng::new(1)), square());
    }

    #[test]
    fn base_scale_stays_in_range_without_rotation_or_shift() {
        let d = Deformation {
            enabled: true,
            base: true,
            rotation_deg: Span::new(0.0, 0.0),
            stroke_degree: 0.0,
            ..Default::default()
        };
        let mut rng = Rng::new(3);
        for _ in 0..200 {
            let out = d.apply(&square(), true, 1.0, &mut rng);
            let (_, _, w, h) = bbox(&out);
            assert!((4.0 * 0.96 - 1e-4..=4.0 * 1.04 + 1e-4).contains(&w), "w {w}");
            assert!((6.0 * 0.96 - 1e-4..=6.0 * 1.04 + 1e-4).contains(&h), "h {h}");
        }
    }

    #[test]
    fn rotation_is_bounded() {
        let d = Deformation {
            enabled: true,
            base: true,
            h_scale_pct: Span::new(100.0, 100.0),
            v_scale_pct: Span::new(100.0, 100.0),
            stroke_degree: 0.0,
            ..Default::default()
        };
        let mut rng = Rng::new(5);
        for _ in 0..200 {
            let out = d.apply(&[vec![(0.0, 0.0), (10.0, 0.0)]], false, 1.0, &mut rng);
            let (a, b) = (out[0][0], out[0][1]);
            let deg = (b.1 - a.1).atan2(b.0 - a.0).to_degrees();
            assert!(deg.abs() <= 2.0 + 1e-3, "deg {deg}");
        }
    }

    #[test]
    fn shape_slants_cjk_only() {
        let d = Deformation { enabled: true, shape: true, ..Default::default() };
        let vertical = vec![vec![(0.0, 0.0), (0.0, 10.0)]]; // top then bottom (y down)
        let cjk = d.apply(&vertical, true, 1.0, &mut Rng::new(9));
        // Top point moves right relative to the bottom point: shear ~0.11–0.14 * 10 units.
        let lean = (cjk[0][0].0 - cjk[0][1].0) / 10.0;
        assert!((0.10..=0.15).contains(&lean), "lean {lean}");
        assert_eq!(d.apply(&vertical, false, 1.0, &mut Rng::new(9)), vertical);
    }

    #[test]
    fn same_seed_same_result() {
        let d = Deformation { enabled: true, base: true, shape: true, ..Default::default() };
        let a = d.apply(&square(), true, 1.0, &mut Rng::new(11));
        let b = d.apply(&square(), true, 1.0, &mut Rng::new(11));
        assert_eq!(a, b);
        assert_ne!(a, d.apply(&square(), true, 1.0, &mut Rng::new(12)));
    }
}
