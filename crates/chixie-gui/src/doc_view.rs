//! The WYSIWYG page editor: pages stacked on a grey desk (Word "print layout"), handwriting drawn
//! live, caret and selection on the paper, keyboard/mouse/clipboard/undo, and IME through
//! `EntityInputHandler` (all platform ranges are UTF-16; `EditDoc` converts).
//!
//! Geometry: layout coordinates have the writable area's top-left at (0,0) with Y negative going
//! down. On paper that point is (margin_left, margin_top) mm. On screen, page k's top-left is at
//! `origin + (x0, GAP + k * (page_h + GAP))` px and 1 mm = `zoom` px.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use chixie_core::config::{PageSettings, Settings};
use chixie_core::editor::EditDoc;
use chixie_core::font::Font;
use chixie_core::handwriting::StyleFile;
use chixie_core::template::Template;
use chixie_core::layout::{self, Document};
use gpui_kit::*;

use crate::state::AppState;

/// Gap between pages and around them, px.
const GAP: f32 = 24.0;
/// px per mm that shows paper at roughly its physical size on a 96-dpi screen.
pub const ZOOM_100: f32 = 96.0 / 25.4;
pub const KEY_CONTEXT: &str = "ChiXieEditor";

actions!(
    chixie_editor,
    [
        MoveLeft, MoveRight, MoveUp, MoveDown, SelectLeft, SelectRight, SelectUp, SelectDown,
        WordLeft, WordRight, SelectWordLeft, SelectWordRight, LineStart, LineEnd, SelectLineStart,
        SelectLineEnd, DocStart, DocEnd, Backspace, Delete, BackspaceWord, DeleteWord, Newline,
        SelectAll, Copy, Cut, Paste, Undo, Redo,
    ]
);

/// Key bindings (Windows/Linux style) scoped to the editor.
pub fn bind_keys(cx: &mut App) {
    let c = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("left", MoveLeft, c),
        KeyBinding::new("right", MoveRight, c),
        KeyBinding::new("up", MoveUp, c),
        KeyBinding::new("down", MoveDown, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("ctrl-left", WordLeft, c),
        KeyBinding::new("ctrl-right", WordRight, c),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, c),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, c),
        KeyBinding::new("home", LineStart, c),
        KeyBinding::new("end", LineEnd, c),
        KeyBinding::new("shift-home", SelectLineStart, c),
        KeyBinding::new("shift-end", SelectLineEnd, c),
        KeyBinding::new("ctrl-home", DocStart, c),
        KeyBinding::new("ctrl-end", DocEnd, c),
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("ctrl-backspace", BackspaceWord, c),
        KeyBinding::new("ctrl-delete", DeleteWord, c),
        KeyBinding::new("enter", Newline, c),
        KeyBinding::new("ctrl-a", SelectAll, c),
        KeyBinding::new("ctrl-c", Copy, c),
        KeyBinding::new("ctrl-x", Cut, c),
        KeyBinding::new("ctrl-v", Paste, c),
        KeyBinding::new("ctrl-z", Undo, c),
        KeyBinding::new("ctrl-y", Redo, c),
        KeyBinding::new("ctrl-shift-z", Redo, c),
    ]);
}

/// Screen geometry of the last paint, for mouse hit-testing and IME candidate placement.
#[derive(Clone, Copy)]
struct Geom {
    origin: Point<Pixels>,
    x0: f32,
    page_w: f32,
    page_h: f32,
    zoom: f32,
    margin_l: f32,
    margin_t: f32,
}

impl Geom {
    fn new(bounds: Bounds<Pixels>, page: &PageSettings, zoom: f32) -> Self {
        let (page_w, page_h) = (page.paper.0 * zoom, page.paper.1 * zoom);
        let x0 = ((f32::from(bounds.size.width) - page_w) / 2.0).max(GAP);
        Self { origin: bounds.origin, x0, page_w, page_h, zoom, margin_l: page.margins.0, margin_t: page.margins.1 }
    }
    fn page_origin(&self, k: usize) -> Point<Pixels> {
        self.origin + point(px(self.x0), px(GAP + k as f32 * (self.page_h + GAP)))
    }
    /// Layout mm (y up, origin at writable top-left) → screen px on page `k`.
    fn screen(&self, k: usize, x: f32, y: f32) -> Point<Pixels> {
        self.page_origin(k) + point(px((self.margin_l + x) * self.zoom), px((self.margin_t - y) * self.zoom))
    }
    /// Screen px → (page, layout x, layout y).
    fn unproject(&self, p: Point<Pixels>, pages: usize) -> (usize, f32, f32) {
        let rel_y = f32::from(p.y - self.origin.y) - GAP / 2.0;
        let k = ((rel_y / (self.page_h + GAP)).floor().max(0.0) as usize).min(pages.saturating_sub(1));
        let o = self.page_origin(k);
        let x = f32::from(p.x - o.x) / self.zoom - self.margin_l;
        let y = self.margin_t - f32::from(p.y - o.y) / self.zoom;
        (k, x, y)
    }
}

pub struct DocView {
    doc: EditDoc,
    layout: Arc<Document>,
    has_font: bool,
    focus: FocusHandle,
    scroll: ScrollHandle,
    zoom: f32,
    geom: Rc<Cell<Option<Geom>>>,
    selecting: bool,
    caret_visible: bool,
    reveal_caret: bool,
    /// What the current layout was computed from, to skip needless relayouts.
    layout_key: Option<(Option<i64>, PageSettings, StyleFile, Option<Template>)>,
    dirty_draft: bool,
    _tasks: Vec<Task<()>>,
    _subs: Vec<Subscription>,
}

impl DocView {
    pub fn new(doc: EditDoc, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let sub = cx.observe_global::<AppState>(|this, cx| this.relayout_if_changed(cx));
        let blink = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(530)).await;
                if this.update(cx, |v, cx| {
                    v.caret_visible = !v.caret_visible;
                    cx.notify();
                })
                .is_err()
                {
                    break;
                }
            }
        });
        let mut this = Self {
            doc,
            layout: Arc::new(Document::default()),
            has_font: false,
            focus,
            scroll: ScrollHandle::new(),
            zoom: ZOOM_100,
            geom: Rc::new(Cell::new(None)),
            selecting: false,
            caret_visible: true,
            reveal_caret: false,
            layout_key: None,
            dirty_draft: false,
            _tasks: vec![blink],
            _subs: vec![sub],
        };
        this.relayout_if_changed(cx);
        this
    }

    // ---- state accessors used by the ribbon / status bar / file ops ----

    pub fn doc(&self) -> &EditDoc {
        &self.doc
    }
    pub fn document(&self) -> Arc<Document> {
        self.layout.clone()
    }
    pub fn zoom(&self) -> f32 {
        self.zoom
    }
    pub fn set_zoom(&mut self, z: f32, cx: &mut Context<Self>) {
        self.zoom = z.clamp(0.4, 6.0);
        cx.notify();
    }
    pub fn caret_page(&self) -> usize {
        self.layout.caret(self.doc.caret()).page
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty_draft)
    }
    /// Fit the page width into the viewport (needs one paint).
    pub fn fit_width(&mut self, cx: &mut Context<Self>) {
        let vw = f32::from(self.scroll.bounds().size.width);
        let paper_w = cx.global::<AppState>().settings.page.paper.0;
        if vw > 100.0 {
            self.set_zoom((vw - 2.0 * GAP) / paper_w, cx);
        }
    }

    pub fn replace_doc(&mut self, doc: EditDoc, cx: &mut Context<Self>) {
        self.doc = doc;
        self.edited(cx);
    }

    /// Typed / pasted text at the selection (also used by the autotest).
    pub fn type_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.doc.insert(&text.replace("\r\n", "\n").replace('\r', "\n"));
        self.edited(cx);
    }

    // ---- layout ----

    fn relayout_if_changed(&mut self, cx: &mut Context<Self>) {
        let st = cx.global::<AppState>();
        let key = (st.active_font.as_ref().map(|f| f.id), st.settings.page.clone(), st.settings.style.clone(), st.active_template());
        if self.layout_key.as_ref() != Some(&key) {
            self.layout_key = Some(key);
            self.relayout(cx);
        }
    }

    fn relayout(&mut self, cx: &mut Context<Self>) {
        let st = cx.global::<AppState>();
        let params = st.settings.page_params_with(st.active_template().as_ref());
        let empty = Font::default();
        let (font, has_font) = match &st.active_font {
            Some(f) => (f.font.clone(), true),
            None => (Arc::new(empty), false),
        };
        self.has_font = has_font;
        self.layout = Arc::new(layout::layout_ids(self.doc.chars(), self.doc.ids(), &font, &params));
        cx.notify();
    }

    /// After any text change: relayout, keep the caret visible, mark the draft dirty.
    fn edited(&mut self, cx: &mut Context<Self>) {
        self.relayout(cx);
        self.after_move(cx);
        self.dirty_draft = true;
    }

    fn after_move(&mut self, cx: &mut Context<Self>) {
        self.caret_visible = true;
        self.reveal_caret = true;
        cx.notify();
    }

    /// Scroll so the caret is inside the viewport (run during paint, when geometry is fresh).
    fn reveal(&self, g: &Geom) {
        let c = self.layout.caret(self.doc.caret());
        let top = f32::from(g.screen(c.page, c.x, c.top).y - g.origin.y);
        let bottom = f32::from(g.screen(c.page, c.x, c.bottom).y - g.origin.y);
        let off = self.scroll.offset();
        let view_h = f32::from(self.scroll.bounds().size.height);
        let shown = -f32::from(off.y);
        if top < shown + 8.0 {
            self.scroll.set_offset(point(off.x, px(-(top - 8.0).max(0.0))));
        } else if bottom > shown + view_h - 8.0 {
            self.scroll.set_offset(point(off.x, px(-(bottom - view_h + 8.0))));
        }
    }

    // ---- actions ----

    fn move_by(&mut self, to: usize, extend: bool, cx: &mut Context<Self>) {
        self.doc.move_to(to, extend);
        self.after_move(cx);
    }
    fn line_edge(&self, end: bool) -> usize {
        let l = self.layout.lines[self.layout.line_of(self.doc.caret())];
        if end { l.end } else { l.start }
    }
    fn move_left(&mut self, _: &MoveLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.doc.left(false);
        self.after_move(cx);
    }
    fn move_right(&mut self, _: &MoveRight, _: &mut Window, cx: &mut Context<Self>) {
        self.doc.right(false);
        self.after_move(cx);
    }
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.doc.left(true);
        self.after_move(cx);
    }
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.doc.right(true);
        self.after_move(cx);
    }
    fn move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.layout.vertical_move(self.doc.caret(), -1);
        self.move_by(to, false, cx);
    }
    fn move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.layout.vertical_move(self.doc.caret(), 1);
        self.move_by(to, false, cx);
    }
    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.layout.vertical_move(self.doc.caret(), -1);
        self.move_by(to, true, cx);
    }
    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.layout.vertical_move(self.doc.caret(), 1);
        self.move_by(to, true, cx);
    }
    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.doc.word_left(self.doc.caret());
        self.move_by(to, false, cx);
    }
    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.doc.word_right(self.doc.caret());
        self.move_by(to, false, cx);
    }
    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.doc.word_left(self.doc.caret());
        self.move_by(to, true, cx);
    }
    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.doc.word_right(self.doc.caret());
        self.move_by(to, true, cx);
    }
    fn line_start(&mut self, _: &LineStart, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.line_edge(false);
        self.move_by(to, false, cx);
    }
    fn line_end(&mut self, _: &LineEnd, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.line_edge(true);
        self.move_by(to, false, cx);
    }
    fn select_line_start(&mut self, _: &SelectLineStart, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.line_edge(false);
        self.move_by(to, true, cx);
    }
    fn select_line_end(&mut self, _: &SelectLineEnd, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.line_edge(true);
        self.move_by(to, true, cx);
    }
    fn doc_start(&mut self, _: &DocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by(0, false, cx);
    }
    fn doc_end(&mut self, _: &DocEnd, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.doc.len();
        self.move_by(n, false, cx);
    }
    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.doc.backspace(false) {
            self.edited(cx);
        }
    }
    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.doc.delete(false) {
            self.edited(cx);
        }
    }
    fn backspace_word(&mut self, _: &BackspaceWord, _: &mut Window, cx: &mut Context<Self>) {
        if self.doc.backspace(true) {
            self.edited(cx);
        }
    }
    fn delete_word(&mut self, _: &DeleteWord, _: &mut Window, cx: &mut Context<Self>) {
        if self.doc.delete(true) {
            self.edited(cx);
        }
    }
    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        self.type_text("\n", cx);
    }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.doc.select_all();
        self.after_move(cx);
    }
    pub fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.doc.has_selection() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.doc.selected_text()));
        }
    }
    pub fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if self.doc.has_selection() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.doc.selected_text()));
            self.doc.insert("");
            self.edited(cx);
        }
    }
    pub fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
            self.type_text(&text, cx);
        }
    }
    pub fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.doc.undo() {
            self.edited(cx);
        }
    }
    pub fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.doc.redo() {
            self.edited(cx);
        }
    }

    // ---- mouse ----

    fn index_at_point(&self, p: Point<Pixels>) -> Option<usize> {
        let g = self.geom.get()?;
        let (k, x, y) = g.unproject(p, self.layout.pages.len());
        Some(self.layout.index_at(k, x, y))
    }

    fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let Some(i) = self.index_at_point(e.position) else { return };
        if e.click_count >= 2 {
            let r = self.doc.word_at(i);
            self.doc.select_range(r);
        } else {
            self.doc.move_to(i, e.modifiers.shift);
            self.selecting = true;
        }
        self.after_move(cx);
    }
    fn mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting
            && let Some(i) = self.index_at_point(e.position)
        {
            self.doc.move_to(i, true);
            self.after_move(cx);
        }
    }
    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }
    fn scroll_zoom(&mut self, e: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        if e.modifiers.control {
            let dy = f32::from(e.delta.pixel_delta(px(16.)).y);
            self.set_zoom(self.zoom * if dy > 0.0 { 1.1 } else { 1.0 / 1.1 }, cx);
            cx.stop_propagation();
        }
    }

    fn caret_px_rect(&self, g: &Geom, i: usize) -> Bounds<Pixels> {
        let c = self.layout.caret(i);
        Bounds::from_corners(g.screen(c.page, c.x, c.top), g.screen(c.page, c.x, c.bottom) + point(px(2.0), px(0.0)))
    }
}

impl Focusable for DocView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EntityInputHandler for DocView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let r = self.doc.range_from_utf16(&range);
        *adjusted = Some(self.doc.range_to_utf16(&r));
        Some(self.doc.chars()[r].iter().collect())
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: self.doc.range_to_utf16(&self.doc.selection()), reversed: self.doc.selection_reversed() })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.doc.marked().map(|r| self.doc.range_to_utf16(&r))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.doc.unmark();
        cx.notify();
    }

    fn replace_text_in_range(&mut self, range: Option<Range<usize>>, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let r = range.map(|r| self.doc.range_from_utf16(&r));
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        self.doc.commit_text(r, &text);
        self.edited(cx);
        window.notify_text_input_changed(&self.focus);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let r = range.map(|r| self.doc.range_from_utf16(&r));
        // Selection inside the new text: UTF-16 → char offsets within `new_text`.
        let to_chars = |u: usize| {
            let mut acc = 0;
            for (i, c) in new_text.chars().enumerate() {
                if acc >= u {
                    return i;
                }
                acc += c.len_utf16();
            }
            new_text.chars().count()
        };
        let sel = new_selected.map(|s| to_chars(s.start)..to_chars(s.end));
        self.doc.set_marked(r, new_text, sel);
        self.edited(cx);
        window.notify_text_input_changed(&self.focus);
    }

    fn bounds_for_range(&mut self, range: Range<usize>, _: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let g = self.geom.get()?;
        Some(self.caret_px_rect(&g, self.doc.from_utf16(range.start)))
    }

    fn character_index_for_point(&mut self, point: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        self.index_at_point(point).map(|i| self.doc.to_utf16(i))
    }
}

impl Render for DocView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::ActiveTheme as _;
        let theme = cx.theme();
        let desk = theme.muted;
        let (sel_color, caret_color, guide, missing_color) = (theme.selection, theme.primary, theme.border, theme.danger);
        let settings: &Settings = &cx.global::<AppState>().settings;
        let page = settings.page.clone();
        let page_prepaint = page.clone();
        let pages = self.layout.pages.len().max(1);
        let total_h = GAP + pages as f32 * (page.paper.1 * self.zoom + GAP);
        let total_w = page.paper.0 * self.zoom + 2.0 * GAP;

        let entity = cx.entity();
        let focus = self.focus.clone();
        let focused = focus.is_focused(window);
        let layout_doc = self.layout.clone();
        let chars: Rc<[char]> = self.doc.chars().into();
        let sel = self.doc.selection();
        let caret = self.doc.caret();
        let marked = self.doc.marked();
        let show_caret = focused && self.caret_visible && sel.is_empty();
        let zoom = self.zoom;
        let show_guides = cx.global::<AppState>().settings.ui.show_guides;
        let geom_out = self.geom.clone();
        let reveal = std::mem::take(&mut self.reveal_caret);
        let scroll = self.scroll.clone();
        let this_reveal = entity.clone();

        div()
            .id("doc-desk")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .bg(desk)
            .overflow_scroll()
            .track_scroll(&scroll)
            .on_action(cx.listener(Self::move_left))
            .on_action(cx.listener(Self::move_right))
            .on_action(cx.listener(Self::move_up))
            .on_action(cx.listener(Self::move_down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::line_start))
            .on_action(cx.listener(Self::line_end))
            .on_action(cx.listener(Self::select_line_start))
            .on_action(cx.listener(Self::select_line_end))
            .on_action(cx.listener(Self::doc_start))
            .on_action(cx.listener(Self::doc_end))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::backspace_word))
            .on_action(cx.listener(Self::delete_word))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_scroll(cx.listener(Self::scroll_zoom))
            .cursor(CursorStyle::IBeam)
            .child(
                canvas(
                    move |bounds, _w, _cx| {
                        let g = Geom::new(bounds, &page_prepaint, zoom);
                        geom_out.set(Some(g));
                        g
                    },
                    move |bounds, g, window, cx| {
                        // Pages.
                        for k in 0..pages {
                            let o = g.page_origin(k);
                            let paper = Bounds::new(o, size(px(g.page_w), px(g.page_h)));
                            window.paint_quad(fill(paper, gpui_kit::white()));
                            let ruled = layout_doc.pages.get(k).is_some_and(|p| !p.guides.is_empty());
                            if !ruled {
                                let (l, t, r, b) = page.margins;
                                let guides = Bounds::from_corners(
                                    o + point(px(l * g.zoom), px(t * g.zoom)),
                                    o + point(px(g.page_w - r * g.zoom), px(g.page_h - b * g.zoom)),
                                );
                                window.paint_quad(outline(guides, guide.opacity(0.5), BorderStyle::Dashed));
                            }
                        }
                        // Template ruling (pre-printed on the paper; never plotted).
                        if show_guides {
                            for (k, p) in layout_doc.pages.iter().enumerate() {
                                paint_guides(window, &p.guides, |x, y| g.screen(k, x, y), g.zoom);
                            }
                        }
                        // Selection.
                        for i in sel.clone() {
                            let bx = layout_doc.boxes[i];
                            let line = layout_doc.lines[bx.line];
                            let x1 = if bx.x1 > bx.x0 { bx.x1 } else { bx.x0 + 2.0 };
                            let rect = Bounds::from_corners(g.screen(bx.page, bx.x0, line.top), g.screen(bx.page, x1, line.bottom));
                            window.paint_quad(fill(rect, sel_color.opacity(0.35)));
                        }
                        // Missing glyphs: dashed placeholder boxes.
                        let missing: std::collections::HashSet<char> = layout_doc.missing.iter().copied().collect();
                        for (bx, c) in layout_doc.boxes.iter().zip(chars.iter()) {
                            if missing.contains(c) {
                                let line = layout_doc.lines[bx.line];
                                let pad = (line.top - line.bottom) * 0.2;
                                let rect = Bounds::from_corners(
                                    g.screen(bx.page, bx.x0 + 0.3, line.top - pad),
                                    g.screen(bx.page, bx.x1 - 0.3, line.bottom + pad),
                                );
                                window.paint_quad(outline(rect, missing_color, BorderStyle::Dashed));
                            }
                        }
                        // Ink.
                        let stroke_w = px((0.35 * g.zoom).clamp(0.6, 2.5));
                        for (k, p) in layout_doc.pages.iter().enumerate() {
                            if p.strokes.is_empty() {
                                continue;
                            }
                            let mut path = PathBuilder::stroke(stroke_w);
                            for s in &p.strokes {
                                let mut pts = s.iter().map(|&(x, y)| g.screen(k, x, y));
                                let Some(first) = pts.next() else { continue };
                                path.move_to(first);
                                let mut drew = false;
                                for q in pts {
                                    path.line_to(q);
                                    drew = true;
                                }
                                if !drew {
                                    path.line_to(first + point(px(0.6), px(0.6)));
                                }
                            }
                            if let Ok(path) = path.build() {
                                window.paint_path(path, gpui_kit::black());
                            }
                        }
                        // IME composition underline.
                        if let Some(m) = marked.clone() {
                            for i in m {
                                let bx = layout_doc.boxes[i];
                                let line = layout_doc.lines[bx.line];
                                let y = line.bottom + (line.top - line.bottom) * 0.08;
                                let rect = Bounds::from_corners(g.screen(bx.page, bx.x0, y), g.screen(bx.page, bx.x1, y) + point(px(0.), px(1.5)));
                                window.paint_quad(fill(rect, caret_color));
                            }
                        }
                        // Caret.
                        if show_caret {
                            let c = layout_doc.caret(caret);
                            let rect = Bounds::from_corners(g.screen(c.page, c.x, c.top), g.screen(c.page, c.x, c.bottom) + point(px(2.0), px(0.0)));
                            window.paint_quad(fill(rect, caret_color));
                        }
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity.clone()), cx);
                        if reveal {
                            this_reveal.read(cx).reveal(&g);
                        }
                    },
                )
                .w_full()
                .min_w(px(total_w))
                .h(px(total_h)),
            )
    }
}

/// Light-blue ruling lines; dashed ones are cut into 1.2 mm dashes.
pub fn paint_guides(window: &mut Window, guides: &[chixie_core::layout::Guide], to_px: impl Fn(f32, f32) -> Point<Pixels>, zoom: f32) {
    if guides.is_empty() {
        return;
    }
    let color = hsla(210.0 / 360.0, 0.65, 0.55, 0.55);
    let mut path = PathBuilder::stroke(px(1.0));
    for gd in guides {
        if gd.dashed {
            let (dx, dy) = (gd.b.0 - gd.a.0, gd.b.1 - gd.a.1);
            let len = (dx * dx + dy * dy).sqrt();
            let n = (len / 1.2).floor().max(1.0) as usize;
            for k in (0..n).step_by(2) {
                let t0 = k as f32 / n as f32;
                let t1 = ((k + 1) as f32 / n as f32).min(1.0);
                path.move_to(to_px(gd.a.0 + dx * t0, gd.a.1 + dy * t0));
                path.line_to(to_px(gd.a.0 + dx * t1, gd.a.1 + dy * t1));
            }
        } else {
            path.move_to(to_px(gd.a.0, gd.a.1));
            path.line_to(to_px(gd.b.0, gd.b.1));
        }
    }
    let _ = zoom;
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
