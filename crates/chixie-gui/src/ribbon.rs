//! Word-style chrome building blocks: the window chrome palette, title bar controls, large
//! buttons (icon over label), small buttons (icon beside label, stacked up to three), captioned
//! groups with an optional dialog launcher ↘, and the tab strip items.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, Theme, ThemeMode, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// Ribbon card height (controls + group caption).
pub const RIBBON_H: f32 = 100.0;
/// Custom title bar height (Word draws its own caption with the quick access toolbar).
pub const TITLE_H: f32 = 36.0;

/// Switch light/dark and re-apply the Word-blue accent (the stock theme's primary is black).
pub fn set_theme(dark: bool, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(if dark { ThemeMode::Dark } else { ThemeMode::Light }, window, cx);
    let t = Theme::global_mut(cx);
    // Word's ribbon blue #2B579A (lighter on dark backgrounds).
    let (h, s) = (215.0 / 360.0, 0.56);
    if dark {
        (t.primary, t.primary_hover, t.primary_active) = (hsla(h, s, 0.52, 1.0), hsla(h, s, 0.58, 1.0), hsla(h, s, 0.46, 1.0));
    } else {
        (t.primary, t.primary_hover, t.primary_active) = (hsla(h, s, 0.39, 1.0), hsla(h, s, 0.45, 1.0), hsla(h, s, 0.33, 1.0));
    }
    t.primary_foreground = gpui_kit::white();
    // Buttons paint from the resolved token set, so update both the colours and the tokens.
    (t.button_primary, t.button_primary_hover, t.button_primary_active) = (t.primary, t.primary_hover, t.primary_active);
    t.button_primary_foreground = gpui_kit::white();
    (t.tokens.button_primary, t.tokens.button_primary_hover, t.tokens.button_primary_active) =
        (t.primary.into(), t.primary_hover.into(), t.primary_active.into());
    t.tokens.button_primary_foreground = gpui_kit::white().into();
    // Checkboxes and other controls paint from `tokens.primary` (black in the stock theme).
    (t.tokens.primary, t.tokens.primary_hover, t.tokens.primary_active, t.tokens.primary_foreground) =
        (t.primary.into(), t.primary_hover.into(), t.primary_active.into(), gpui_kit::white().into());
}

/// Word 365 "Colorful" chrome colours, derived from the light/dark mode and the accent.
pub struct Chrome {
    /// Title bar fill, its text/icon colour and the hover wash for its buttons.
    pub title_bg: Hsla,
    pub title_fg: Hsla,
    pub title_hover: Hsla,
    /// Tab strip, the gutter around the ribbon card and the status bar.
    pub strip_bg: Hsla,
    /// The ribbon card and task panes.
    pub card_bg: Hsla,
    /// The grey desk behind the pages.
    pub desk: Hsla,
}

pub fn chrome(cx: &App) -> Chrome {
    let t = cx.theme();
    if t.mode.is_dark() {
        Chrome {
            title_bg: hsla(0., 0., 0.12, 1.),
            title_fg: t.foreground,
            title_hover: hsla(0., 0., 1., 0.08),
            strip_bg: hsla(0., 0., 0.16, 1.),
            card_bg: hsla(0., 0., 0.21, 1.),
            desk: hsla(0., 0., 0.10, 1.),
        }
    } else if !cfg!(target_os = "windows") {
        // macOS / Linux: a light caption, as Word for Mac draws beside the system buttons.
        Chrome {
            title_bg: hsla(0., 0., 0.953, 1.),
            title_fg: t.foreground,
            title_hover: hsla(0., 0., 0., 0.06),
            strip_bg: hsla(0., 0., 0.953, 1.),
            card_bg: gpui_kit::white(),
            desk: hsla(0., 0., 0.902, 1.),
        }
    } else {
        // Windows: Word's blue caption.
        Chrome {
            title_bg: t.primary,
            title_fg: gpui_kit::white(),
            title_hover: hsla(0., 0., 1., 0.16),
            strip_bg: hsla(0., 0., 0.953, 1.),
            card_bg: gpui_kit::white(),
            desk: hsla(0., 0., 0.902, 1.),
        }
    }
}

/// Minimise / maximise / close in the title bar colours. On Windows the OS handles the clicks
/// through the hit-test areas (snap layouts on hover over maximise included).
pub fn window_controls(window: &Window, c: &Chrome) -> impl IntoElement {
    let fg = c.title_fg;
    let ctl = |id: &'static str, icon: IconName, area: WindowControlArea, hover: Hsla, hover_fg: Hsla| {
        div()
            .id(id)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(fg)
            .hover(move |s| s.bg(hover).text_color(hover_fg))
            .window_control_area(area)
            .child(Icon::new(icon).with_size(px(14.)))
    };
    let max_icon = if window.is_maximized() { IconName::WindowRestore } else { IconName::WindowMaximize };
    h_flex()
        .h_full()
        .flex_shrink_0()
        .child(ctl("win-min", IconName::WindowMinimize, WindowControlArea::Min, c.title_hover, fg))
        .child(ctl("win-max", max_icon, WindowControlArea::Max, c.title_hover, fg))
        .child(ctl("win-close", IconName::WindowClose, WindowControlArea::Close, hsla(355. / 360., 0.86, 0.47, 1.), gpui_kit::white()))
}

/// Icon-only button for the title bar (quick access toolbar).
pub fn title_btn(id: impl Into<ElementId>, icon: IconName, tip: impl Into<SharedString>, c: &Chrome) -> Stateful<Div> {
    let hover = c.title_hover;
    let tip: SharedString = tip.into();
    div()
        .id(id.into())
        // Inside the caption's drag area: without this, Windows treats the click as a title bar
        // drag, and on macOS/Linux the TitleBar starts a window move on mouse-down.
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .size(px(28.))
        .rounded_sm()
        .flex()
        .items_center()
        .justify_center()
        .text_color(c.title_fg)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        .child(Icon::new(icon).with_size(px(15.)))
}

/// Shortcut label for the platform's command key: "Ctrl+S" or "⌘S".
pub fn shortcut(key: &str) -> String {
    if cfg!(target_os = "macos") { format!("⌘{key}") } else { format!("Ctrl+{key}") }
}

/// Redo is Ctrl+Y on Windows/Linux, ⇧⌘Z on macOS.
pub fn redo_shortcut() -> String {
    if cfg!(target_os = "macos") { "⇧⌘Z".into() } else { "Ctrl+Y".into() }
}

/// A boxed click handler (group dialog launchers).
pub type ClickFn = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Large ribbon button: 28 px icon above a one-line label (▾ when it opens a menu).
pub fn big(id: impl Into<ElementId>, label: impl Into<SharedString>, icon: IconName) -> Button {
    big_inner(id, label.into(), icon, false)
}

/// Large button that opens a menu (adds ▾ under the label).
pub fn big_menu(id: impl Into<ElementId>, label: impl Into<SharedString>, icon: IconName) -> Button {
    big_inner(id, label.into(), icon, true)
}

fn big_inner(id: impl Into<ElementId>, label: SharedString, icon: IconName, caret: bool) -> Button {
    Button::new(id).ghost().h(px(70.)).min_w(px(52.)).px_1p5().child(
        v_flex()
            .items_center()
            .gap_1()
            .child(Icon::new(icon).with_size(px(28.)))
            .child(
                h_flex()
                    .gap_0p5()
                    .text_xs()
                    .child(label)
                    .when(caret, |el| el.child(Icon::new(IconName::ChevronDown).with_size(px(10.)))),
            ),
    )
}

/// Small ribbon button: 16 px icon + label, for 3-row stacks.
pub fn small(id: impl Into<ElementId>, label: impl Into<SharedString>, icon: IconName) -> Button {
    Button::new(id).ghost().xsmall().h(px(22.)).justify_start().icon(Icon::new(icon).with_size(px(15.))).label(label)
}

/// Icon-only small button (ribbon, status bar).
pub fn icon_btn(id: impl Into<ElementId>, icon: IconName, tip: impl Into<SharedString>) -> Button {
    Button::new(id).ghost().xsmall().icon(Icon::new(icon).with_size(px(15.))).tooltip(tip)
}

/// Up to three small controls stacked vertically.
pub fn stack(items: impl IntoIterator<Item = AnyElement>) -> Div {
    v_flex().gap_0p5().justify_center().children(items)
}

/// A captioned ribbon group followed by a short inset divider, with an optional ↘ launcher.
pub fn group(
    cx: &App,
    id: &'static str,
    label: &'static str,
    body: impl IntoElement,
    launcher: Option<ClickFn>,
) -> Div {
    let theme = cx.theme();
    let (muted, border) = (theme.muted_foreground, theme.border);
    let column = v_flex()
        .h_full()
        .px_2()
        .child(h_flex().flex_1().items_center().gap_1().child(body))
        .child(
            h_flex()
                .h(px(18.))
                .justify_center()
                .text_xs()
                .text_color(muted)
                .child(label)
                .when_some(launcher, |el, f| {
                    el.child(
                        Button::new(SharedString::from(format!("{id}-launch")))
                            .ghost()
                            .xsmall()
                            .ml_1()
                            .icon(Icon::new(IconName::ArrowDownRight).with_size(px(10.)))
                            .tooltip(format!("{label}设置…"))
                            .on_click(f),
                    )
                }),
        );
    h_flex().h_full().child(column).child(div().w(px(1.)).h(px(RIBBON_H - 26.)).bg(border))
}

/// A ribbon tab title; the selected one is semibold in the accent colour over a short bar.
pub fn tab_item(id: usize, label: &'static str, selected: bool, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let (fg, accent, hover) = (theme.foreground, theme.primary, theme.secondary_hover);
    div()
        .id(("ribbon-tab", id))
        .h(px(32.))
        .px_3()
        .flex()
        .flex_col()
        .items_center()
        .justify_end()
        .rounded_t_md()
        .cursor_pointer()
        .text_sm()
        .text_color(if selected { accent } else { fg })
        .when(selected, |el| el.font_weight(FontWeight::SEMIBOLD))
        .hover(|s| s.bg(hover))
        .child(label)
        .child(div().w_full().mt_1().mb_0p5().h(px(3.)).rounded_full().when(selected, |el| el.bg(accent)))
}
