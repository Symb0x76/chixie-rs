//! Font previews, like Word's font list: each font's name drawn in its own strokes (or a short
//! sample when the font lacks a character of its name), plus the font picker's item type.

use std::sync::Arc;

use sast_writer_core::font::Font;
use sast_writer_core::layout::EM_UNITS;
use gpui_kit::assets::IconName;
use gpui_kit::component::select::SelectItem;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// Shown when the font cannot write its own name.
const SAMPLE: &str = "永和九年";
/// Longest preview, in characters.
const MAX_CHARS: usize = 8;

/// Strokes in glyph units (y down, about 0..EM_UNITS tall), laid out left to right.
pub struct FontPreview {
    strokes: Vec<Vec<(f32, f32)>>,
    width: f32,
}

impl FontPreview {
    pub fn new(font: &Font, name: &str) -> Option<Self> {
        let writable = |s: &str| s.chars().filter(|c| !c.is_whitespace()).all(|c| font.get(c).is_some());
        let text: String = if !name.trim().is_empty() && writable(name) {
            name.chars().take(MAX_CHARS).collect()
        } else {
            let mut s: Vec<char> = SAMPLE.chars().filter(|&c| font.get(c).is_some()).collect();
            if s.len() < 2 {
                // Small fonts: any of its own Chinese characters, then the rest.
                let mut own: Vec<char> = font.glyphs.keys().copied().filter(|c| !c.is_whitespace()).collect();
                own.sort_by_key(|&c| (!('\u{4e00}'..='\u{9fff}').contains(&c), c));
                own.retain(|c| !s.contains(c));
                let room = 4 - s.len();
                s.extend(own.into_iter().take(room));
            }
            s.into_iter().collect()
        };
        let (mut strokes, mut x) = (Vec::new(), 0.0);
        for c in text.chars() {
            match font.get(c) {
                Some(g) => {
                    strokes.extend(g.strokes.iter().map(|s| s.iter().map(|&(px, py)| (px + x, py)).collect()));
                    x += g.advance;
                }
                None => x += EM_UNITS * 0.5,
            }
        }
        (!strokes.is_empty()).then_some(Self { strokes, width: x })
    }
}

/// Draw a preview `height` px tall (one em) in `color`.
pub fn preview(p: Arc<FontPreview>, height: f32, color: Hsla) -> impl IntoElement {
    let s = height / EM_UNITS;
    let w = p.width * s;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let o = bounds.origin;
            let at = |(x, y): (f32, f32)| o + point(px(x * s), px(y * s));
            let mut path = PathBuilder::stroke(px((height / 18.0).clamp(1.0, 2.0)));
            for st in &p.strokes {
                let Some(&first) = st.first() else { continue };
                path.move_to(at(first));
                if st.len() == 1 {
                    path.line_to(at(first) + point(px(0.8), px(0.8)));
                }
                for &q in &st[1..] {
                    path.line_to(at(q));
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .w(px(w))
    .h(px(height))
    .flex_shrink_0()
}

/// The best preview of font `id` that fits `max_w`×`max_h` px: its own strokes when downloaded,
/// else the server's preview image; `None` while neither is available.
pub fn face(id: i64, max_h: f32, max_w: f32, cx: &App) -> Option<AnyElement> {
    let st = cx.global::<crate::state::AppState>();
    if let Some(p) = st.previews.get(&id) {
        let h = max_h.min(max_w * EM_UNITS / p.width.max(0.1));
        return Some(preview(p.clone(), h, cx.theme().foreground).into_any_element());
    }
    match st.cloud_previews.get(&id) {
        Some(crate::cloud_preview::CloudPreview::Ready(im)) => {
            // The server image has a little margin around the ink, so give it a bit more room.
            let h = (max_h * 1.15).min(max_w / im.aspect);
            Some(img(im.for_theme(cx)).h(px(h)).w(px(h * im.aspect)).flex_shrink_0().into_any_element())
        }
        _ => None,
    }
}

/// One entry of the font picker: downloaded fonts with their preview, cloud fonts by name.
#[derive(Clone)]
pub struct FontItem {
    pub id: i64,
    pub name: SharedString,
    /// Collected on the server but not downloaded yet (login needed to use it).
    pub cloud: bool,
}

impl SelectItem for FontItem {
    type Value = i64;

    fn title(&self) -> SharedString {
        if self.cloud { format!("{}（云端）", self.name).into() } else { self.name.clone() }
    }

    fn value(&self) -> &i64 {
        &self.id
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (fg, muted) = (theme.foreground, theme.muted_foreground);
        h_flex()
            .w_full()
            .h(px(30.))
            .gap_3()
            .map(|el| match face(self.id, 22.0, 150.0, cx) {
                // Word's list: the font's own face first, its plain name after it.
                Some(f) => el.child(f).child(div().flex_1()).child(div().text_xs().text_color(muted).child(self.name.clone())),
                None => el.child(div().flex_1().text_color(fg).child(self.name.clone())),
            })
            .when(self.cloud, |el| {
                el.child(
                    h_flex()
                        .gap_1()
                        .text_xs()
                        .text_color(muted)
                        .child(Icon::new(IconName::Download).with_size(px(12.)))
                        .child("云端"),
                )
            })
    }
}
