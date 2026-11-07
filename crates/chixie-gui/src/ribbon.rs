//! Word-style ribbon building blocks: large buttons (icon over label), small buttons (icon
//! beside label, stacked up to three), captioned groups with an optional dialog launcher ↘,
//! and the tab strip items.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, Theme, ThemeMode, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub const RIBBON_H: f32 = 92.0;

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
    Button::new(id).ghost().h(px(68.)).min_w(px(52.)).px_1p5().child(
        v_flex()
            .items_center()
            .gap_1()
            .child(Icon::new(icon).with_size(px(26.)))
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

/// Icon-only small button (quick access toolbar, status bar).
pub fn icon_btn(id: impl Into<ElementId>, icon: IconName, tip: impl Into<SharedString>) -> Button {
    Button::new(id).ghost().xsmall().icon(Icon::new(icon).with_size(px(15.))).tooltip(tip)
}

/// Up to three small controls stacked vertically.
pub fn stack(items: impl IntoIterator<Item = AnyElement>) -> Div {
    v_flex().gap_0p5().justify_center().children(items)
}

/// A captioned ribbon group with a divider on the right and an optional ↘ dialog launcher.
pub fn group(
    cx: &App,
    id: &'static str,
    label: &'static str,
    body: impl IntoElement,
    launcher: Option<ClickFn>,
) -> Div {
    let theme = cx.theme();
    let (muted, border) = (theme.muted_foreground, theme.border);
    v_flex()
        .h_full()
        .px_2()
        .border_r_1()
        .border_color(border)
        .child(h_flex().flex_1().items_center().gap_1().child(body))
        .child(
            h_flex()
                .h(px(16.))
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
        )
}

/// A ribbon tab title; the selected one is drawn in the accent colour with an underline bar.
pub fn tab_item(id: usize, label: &'static str, selected: bool, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let (fg, accent, hover) = (theme.foreground, theme.primary, theme.secondary);
    div()
        .id(("ribbon-tab", id))
        .h(px(32.))
        .px_3()
        .flex()
        .flex_col()
        .justify_center()
        .cursor_pointer()
        .text_sm()
        .text_color(if selected { accent } else { fg })
        .hover(|s| s.bg(hover))
        .child(label)
        .child(div().mt_1().h(px(3.)).rounded_sm().when(selected, |el| el.bg(accent)))
}
