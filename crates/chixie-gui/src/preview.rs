//! Paper preview: draws one page of a laid-out `Document` exactly as it will be written.
//!
//! Coordinates: layout uses the writable area's top-left as origin with Y negative going down
//! (what the plotter streams). On paper that point sits at (margin_left, margin_top), so a stroke
//! point (x, y) is at paper mm (l + x, t - y). Screen px = pan + paper_mm * zoom.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use chixie_core::layout::Document;
use gpui_kit::*;

#[derive(Clone, Copy)]
pub struct View {
    /// Screen pixels per paper millimetre.
    pub zoom: f32,
    /// Screen offset of the paper's top-left corner, relative to the canvas.
    pub pan: Point<Pixels>,
}

pub struct Colors {
    pub desk: Hsla,
    pub paper: Hsla,
    pub margin: Hsla,
    pub ink: Hsla,
}

/// Zoom and pan that fit the whole paper into a canvas of `size`, centred, with a 16 px border.
pub fn fit(paper: (f32, f32), size: Size<Pixels>) -> View {
    let (w, h) = paper;
    let (cw, ch) = (f32::from(size.width), f32::from(size.height));
    let zoom = ((cw - 32.0) / w).min((ch - 32.0) / h).clamp(0.3, 30.0);
    let pan = point(px(((cw - w * zoom) / 2.0).max(16.0)), px(16.0));
    View { zoom, pan }
}

/// `paper` = (width, height) mm; `margins` = (left, top, right, bottom) mm.
/// `size_out` receives the canvas size each frame (for fit-to-view).
pub fn canvas_element(
    doc: Option<Arc<Document>>,
    page: usize,
    paper: (f32, f32),
    margins: (f32, f32, f32, f32),
    view: View,
    colors: Colors,
    size_out: Rc<Cell<Size<Pixels>>>,
) -> impl IntoElement {
    canvas(
        move |bounds, _window, _cx| {
            size_out.set(bounds.size);
            let z = view.zoom;
            let o = bounds.origin + view.pan;
            let at = |mx: f32, my: f32| o + point(px(mx * z), px(my * z));
            let (l, t, r, b) = margins;
            let (w, h) = paper;

            let paper_rect = Bounds::from_corners(at(0.0, 0.0), at(w, h));
            let margin_rect = Bounds::from_corners(at(l, t), at(w - r, h - b));

            let ink = doc.as_ref().and_then(|d| d.pages.get(page)).and_then(|p| {
                if p.strokes.is_empty() {
                    return None;
                }
                let mut path = PathBuilder::stroke(px((0.35 * z).clamp(0.6, 2.5)));
                for s in &p.strokes {
                    let mut pts = s.iter().map(|&(x, y)| at(l + x, t - y));
                    let Some(first) = pts.next() else { continue };
                    path.move_to(first);
                    let mut drew = false;
                    for q in pts {
                        path.line_to(q);
                        drew = true;
                    }
                    if !drew {
                        // A single-point stroke is a dot; give it a visible length.
                        path.line_to(first + point(px(0.6), px(0.6)));
                    }
                }
                path.build().ok()
            });
            (paper_rect, margin_rect, ink)
        },
        move |bounds, (paper_rect, margin_rect, ink), window, _cx| {
            window.paint_quad(fill(bounds, colors.desk));
            window.paint_quad(fill(paper_rect, colors.paper));
            window.paint_quad(outline(margin_rect, colors.margin, BorderStyle::Dashed));
            if let Some(path) = ink {
                window.paint_path(path, colors.ink);
            }
        },
    )
    .size_full()
}
