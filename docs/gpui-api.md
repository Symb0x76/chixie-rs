# gpui-kit 0.9.0 API cheat sheet (verified from source, nothing built)

Roots (all under `~/.cargo/registry/src/mirrors.ustc.edu.cn-38d0e5eb5da2abae/`):
GC = `gpui-component-0.9.0/src`, GB = `gpui-base-0.9.0/src`, GP = `gpui-pre-0.3.11/src`, GK = `gpui-kit-0.9.0`.
Line cites are `file:line` relative to those roots. "gpui" below = gpui-pre (renamed `gpui`), re-exported at `gpui_kit::*`.

## 0. Bootstrap / re-exports
- `gpui_kit::*` = all of gpui (GK/src/lib.rs `pub use ::gpui::*`), `gpui_kit::component` = gpui_component (GK lib.rs), `gpui_kit::assets` = gpui_kit_assets, `gpui_kit::base` = gpui_base.
- `component::init(cx)` is called by `gpui_kit::init(cx)` (GK lib.rs `pub fn init(cx: &mut App)`). Call once before opening windows.
- `pub fn open_window<V: Render>(options: WindowOptions, cx: &mut App, build: impl FnOnce(&mut Window, &mut App) -> Entity<V>) -> Result<(AnyWindowHandle, Entity<V>)>` (GK lib.rs). It wraps your view in `base::Root` (== `component::Root`, GC/lib.rs:101) itself; builder returns your view, NOT a Root.
- `gpui_kit::application()` (desktop) -> `Application`; `.with_assets(impl AssetSource) -> Self` (GP/app.rs:266); `.run(|cx: &mut App| ..)` (GP/app.rs, near :314).
```rust
use gpui_kit::*;
gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(|cx| {
    gpui_kit::init(cx);
    gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| cx.new(|cx| MyView::new(window, cx))).unwrap();
});   // GK/lib.rs doc + gpui-kit-assets-0.9.0/README.md:57-62
```
- ICONS NEED ASSETS: `with_assets(gpui_kit::assets::Assets)` (default 101 icons) or `AllAssets` (all 1830); without it icons render blank. Selective: `icon_assets!` macro (assets lib.rs:62-90).
- Traits you must import for builder methods: `use gpui_kit::component::{ActiveTheme as _, Sizable as _, Disableable as _, Selectable as _, StyledExt as _, WindowExt as _, button::ButtonVariants as _, scroll::ScrollableElement as _}; use gpui_kit::prelude::*` (re-export `gpui::prelude::*` gives FluentBuilder `.when`, ParentElement, Styled, InteractiveElement, IntoElement, Render, RenderOnce...). `gpui_kit::component::{h_flex, v_flex}` (GC/styled.rs:7) are `-> Div` helpers.
  (`Sizable, Size, Disableable, Selectable, Collapsible` live at `component::` root via styled.rs:1-2; `Icon, IconName` via `pub use icon::*`; `Theme, ThemeMode, ThemeColor` via `pub use theme::*`; `IndexPath`; `WindowExt` lib.rs:123.)

## 1. Sidebar  (`gpui_kit::component::sidebar::*`; GC/sidebar/{mod,menu,group,header,footer}.rs)
Types: `Sidebar<E: SidebarItem>`, `SidebarMenu`, `SidebarMenuItem`, `SidebarGroup<E>`, `SidebarHeader`, `SidebarFooter`, `SidebarToggleButton`, `SidebarCollapsible {Icon(default), Offcanvas, None}`, trait `SidebarItem`.
- `Sidebar::new(id: impl Into<ElementId>) -> Self` (mod.rs:238). It is `#[derive(IntoElement)]` + `Styled` (so `.w(px(..))`, `.h_full()`; default width 255px, mod.rs:27).
  Builders: `.side(Side)` :254 (default Left; `component::Side`), `.collapsible(impl Into<SidebarCollapsible>)` :264 (bool ok: true=Icon,false=None), `.collapsed(bool)` :270, `.header(impl IntoElement)` :276, `.footer(impl IntoElement)` :282, `.child(E)` :288, `.children(impl IntoIterator<Item=E>)` :294. Sidebar itself paints `bg(tokens.sidebar)`, `sidebar_border` right border (mod.rs:416-432). It is `h_full` -> put it in an `h_flex().size_full()` parent.
- `SidebarMenu::new() -> Self` (menu.rs:28); `.child(impl Into<SidebarMenuItem>)` :39; `.children(impl IntoIterator<Item = impl Into<SidebarMenuItem>>)` :45 (REPLACES items). `SidebarMenu: SidebarItem`, so `Sidebar<SidebarMenu>` works.
- `SidebarMenuItem::new(label: impl Into<SharedString>) -> Self` (menu.rs:114). Builders: `.icon(impl Into<Icon>)` :135 (IconName works), `.active(bool)` :155, `.on_click(impl Fn(&ClickEvent, &mut Window, &mut App) + 'static)` :161, `.disable(bool)` :221 (NOTE name: `disable`, not `disabled`), `.suffix(Fn(&mut Window,&mut App)->impl IntoElement)` :209, `.children(..)` submenu :203, `.default_open/.click_to_open/.click_to_toggle(bool)`, `.label_style(StyleRefinement)`, `.accessibility_label(..)`, `.context_menu(..)`. Also `FluentBuilder` (`.when`).
- No event type emitted; navigation is the `on_click` closure. Use `cx.listener` (returns exactly `Fn(&E,&mut Window,&mut App)`, GP/app/context.rs:241).
- Real usage: GC/setting/settings.rs:164-190 (Sidebar::new + `.collapsible(false)` + `SidebarMenu::new().children(.. SidebarMenuItem::new(..).active(..).on_click(..))`).
```rust
use gpui_kit::component::{h_flex, IconName, sidebar::{Sidebar, SidebarMenu, SidebarMenuItem}};
let nav = ["Dashboard","Fonts","Settings","About"]; // self.page: usize
let icons = [IconName::LayoutDashboard, IconName::Palette, IconName::Settings, IconName::Info];
h_flex().size_full().child(
    Sidebar::new("nav").collapsible(false).w(px(200.))
        .child(SidebarMenu::new().children(nav.iter().enumerate().map(|(ix, name)|
            SidebarMenuItem::new(*name).icon(icons[ix].clone()).active(self.page == ix)
                .on_click(cx.listener(move |this, _ev, _w, cx| { this.page = ix; cx.notify(); }))))))
    .child(div().flex_1().child(/* page content */))
```
(IconName variants `LayoutDashboard, Palette, Settings, Info` exist: gpui-kit-assets-0.9.0/default-icons.txt; IconName is `Clone`, GC/icon.rs:19.)
Optional: `SidebarToggleButton::new().collapsed(bool).side(Side).on_click(Fn(&ClickEvent,&mut Window,&mut App))` (mod.rs:309-350).

## 2. Tabs  (`gpui_kit::component::tab::{Tab, TabBar}`; GC/tab/tab_bar.rs, tab.rs)
- `TabBar::new(id: impl Into<ElementId>) -> Self` (tab_bar.rs:68). Builders: `.selected_index(usize)` :173, `.children(impl IntoIterator<Item = impl Into<Tab>>)` :161, `.child(impl Into<Tab>)` :167, `.on_click(Fn(&usize, &mut Window, &mut App) + 'static)` :187 (overrides per-tab on_click), styles `.pill() .outline() .segmented() .underline() .folder()` :95-119, `.with_variant(TabVariant)`, `.prefix/.suffix(impl IntoElement)`, `.menu(bool)`, `.max_width`, `.track_scroll(&ScrollHandle)`. `Sizable` too (`.small()`).
- `Tab::new() -> Self` (tab.rs:624), `.label(impl Into<SharedString>)` :629, `.icon(impl Into<Icon>)`, `.disabled(bool)` :699, `.on_click(Fn(&ClickEvent,&mut Window,&mut App))` :705. `From<&'static str>/String/SharedString/Icon/IconName for Tab` (tab.rs:565-589) so `.children(["A","B"])` works.
- No event type; state is yours (controlled).
```rust
use gpui_kit::component::tab::{Tab, TabBar};
TabBar::new("tabs").selected_index(self.tab)
    .children([Tab::new().label("General"), Tab::new().label("Advanced")])
    .on_click(cx.listener(|this, ix: &usize, _w, cx| { this.tab = *ix; cx.notify(); }))
// GK/tests/integration/controls.rs:55-61 (same, `index` inferred)
```

## 3. Checkbox  (`gpui_kit::component::checkbox::Checkbox`; GC/checkbox.rs)
- `Checkbox::new(id: impl Into<ElementId>) -> Self` :39. `.label(impl Into<Text>)` :80 (&str/String/SharedString ok, text/compat.rs:451-466), `.checked(bool)` :96, `.on_click(impl Fn(&bool,&mut Window,&mut App)+'static)` :102 == alias of `.on_change(same sig)` :112 (last one wins; ONE callback). Controlled: handler gets the *requested* value, you must store it and `cx.notify()`. Also `.disabled(bool)` (Disableable), `.accessibility_label`, `.tooltip`, `.tab_stop`, Sizable, Styled.
```rust
Checkbox::new("agree").label("Agree").checked(self.agreed)
    .on_click(cx.listener(|this, checked: &bool, _w, cx| { this.agreed = *checked; cx.notify(); }))
// GK/tests/integration/controls.rs:33-41
```
(`Switch::new(id).checked(bool).on_change(Fn(&bool,..))` in `component::switch` has the same shape.)

## 4. Notifications  (`component::notification::Notification`, `component::WindowExt`)
- `WindowExt::push_notification(&mut self, note: impl Into<Notification>, cx: &mut App)` (GC/window_ext.rs:118) implemented for `Window`. `Into<Notification>` from `&str/String/SharedString/Cow<str>/(NotificationType, impl Into<SharedString>)` (notification.rs:135-166). Also `remove_notification::<T>(cx)`, `remove_notification1::<T>(key, cx)`, `clear_notifications(cx)`, `notifications(cx)`.
- Constructors: `Notification::new()` :174 (random id, autohide true), `::info/::success/::warning/::error(impl Into<SharedString>)` :203-224. Builders: `.message(..)`, `.title(..)`, `.icon(impl Into<Icon>)`, `.with_type(NotificationType::{Info,Success,Warning,Error})`, `.id::<T>()` / `.id1::<T>(key)` (same id replaces previous toast), `.autohide(bool)` :321, `.placement(Anchor)`, `.delivery(NotificationDelivery::{InApp(default),System,InAppAndSystem})` / `.system()` / `.in_app_and_system()`, `.on_click(Fn(&ClickEvent,&mut Window,&mut App))`, `.on_close(Fn(&mut Window,&mut App))`, `.action(..)`.
- ROOT: nothing extra to render. `gpui_kit::open_window` uses `base::Root`; `component::init` registers `WindowState` as a Root plugin (GC/root.rs:22) whose Render mounts sheet/dialog/notification layers (root.rs:739-773 `WindowStateLayers`). Test `GK/tests/integration/root.rs:1-24` ("a view that never mentions the layers still gets its dialogs"). There is NO `Root::render_notification_layer` public API in 0.9.0 (grep: none). Requirement: `gpui_kit::init(cx)` first, and window root must be created via `gpui_kit::open_window` (else panics "call gpui_component::init before gpui_kit::open_window", root.rs:752).
```rust
use gpui_kit::component::{WindowExt as _, notification::Notification};
// in an on_click(|_, window, cx| ..) or cx.listener(|this, _, window, cx| ..)
window.push_notification(Notification::success("Saved").title("Done"), cx);
window.push_notification("plain message", cx);
// GK/tests/integration/overlays.rs:53-58,77-80
```
- From async code use `cx.spawn_in(window, async move |this, cx| { this.update_in(cx, |_, window, cx| window.push_notification(Notification::error("x"), cx)).ok(); })` (see section 9).

## 5. Dialog / modal  (`component::dialog::*`, `WindowExt::open_dialog / open_alert_dialog`)
- `fn open_dialog<F: Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static>(&mut self, cx: &mut App, build: F)` (window_ext.rs:75). Builder is re-run on rebuild: capture clones of entities, do not `cx.new` inside it (doc :40-56). `close_dialog(cx)` :112, `close_all_dialogs(cx)`, `has_active_dialog(cx)`.
- `fn open_alert_dialog<F: Fn(AlertDialog, &mut Window, &mut App) -> AlertDialog + 'static>(&mut self, cx, build)` :101.
- `Dialog` (dialog/dialog.rs:314; `Dialog::new(cx: &mut App)` :364, normally given to you by open_dialog): `.title(impl IntoElement)` :402, `.footer(impl IntoElement)` :421, `.content(Fn(DialogContent,&mut Window,&mut App)->DialogContent)` :393, `.on_ok(Fn(&ClickEvent,&mut Window,&mut App)->bool)` :462 (return true = close, false = stay open), `.on_cancel(same)` :473, `.on_close(Fn(&ClickEvent,&mut Window,&mut App))` :451, `.button_props(DialogButtonProps)`, `.close_button(bool)`, `.width/.w(impl Into<Pixels>)`, `.max_w`, `.margin_top`, `.centered()`, `.overlay(bool)`, `.overlay_closable(bool)`, `.keyboard(bool)`, `.entrance(DialogEntrance)`; implements `ParentElement` (`.child(..)`) and `Styled`. A plain Dialog renders NO buttons itself (dialog.rs:413-420): add `DialogFooter::new().child(DialogClose::new().trigger(|b| b.label("Cancel"))).child(DialogAction::new().child(Button::new("ok").label("Save")))` (footer.rs:23,71,104; `DialogAction` click dispatches Confirm -> on_ok).
- `AlertDialog` (alert_dialog.rs:66; `AlertDialog::new(cx)` :80): `.title(impl IntoElement)` :169, `.description(impl IntoElement)` :177, `.confirm()` :96 (adds Cancel button; default has only OK), `.show_cancel(bool)` :240, `.ok_text/.cancel_text(impl Into<SharedString>)` :210/:222, `.ok_variant/.cancel_variant(ButtonVariant)`, `.icon(impl IntoElement)`, `.width(impl Into<Pixels>)`, `.on_ok(Fn(&ClickEvent,&mut Window,&mut App)->bool)` :303, `.on_cancel(same)` :314, `.on_close(Fn(..))` :292, `.keyboard/.close_button/.centered/.margin_top/.entrance`. DOC-COMMENT TRAPS: `.warning()` and `.show_icon()` mentioned in alert_dialog.rs:21,35 / window_ext.rs:95 DO NOT EXIST (grep).
```rust
use gpui_kit::component::{WindowExt as _, button::Button, dialog::{DialogAction, DialogClose, DialogFooter}};
// custom dialog (GK/tests/integration/overlays.rs:37-61)
window.open_dialog(cx, move |dialog, _w, _cx| {
    dialog.title("Edit profile").child("body or Input::new(&state)")
        .footer(DialogFooter::new()
            .child(DialogClose::new().trigger(|b| b.label("Cancel")))
            .child(DialogAction::new().child(Button::new("ok").label("Save"))))
        .on_ok(move |_ev, window, cx| { /* do work */ window.push_notification("ok", cx); true })
});
// confirm (overlays.rs:349-351; builder methods per alert_dialog.rs)
window.open_alert_dialog(cx, |a, _w, _cx| a.title("Delete?").description("This cannot be undone.").confirm()
    .ok_text("Delete").on_ok(|_, _w, _cx| true));
```

## 6. List / Table for ~10-100 rows
Simplest verified options (no component List/Table needed):
- Scroll container with scrollbar: `use gpui_kit::component::{v_flex, scroll::ScrollableElement as _}; v_flex().size_full().overflow_y_scrollbar().children(rows)` -> `Scrollable<Div>`, wraps element, has own id via `#[track_caller] caller_id()` so NO `.id()` needed (GC/scroll/scrollable.rs:43-60, used w/o id at :393 test). Optional `.id(..)` on the Scrollable when one call site makes several.
- Plain gpui: `.overflow_y_scroll()` is on trait `StatefulInteractiveElement` (GP/elements/div.rs:1353,1582) => element must be `Stateful<Div>`: `div().id("rows").overflow_y_scroll().h_full()`. (`overflow_hidden()` is on `Styled`, no id needed.) `.track_scroll(&ScrollHandle)` also on that trait :1596.
- Virtualised: `uniform_list(id: impl Into<ElementId>, item_count: usize, f: impl 'static + Fn(Range<usize>, &mut Window, &mut App) -> Vec<R>) -> UniformList` (GP/elements/uniform_list.rs:22; overflow.y=Scroll built in). Example GP/examples/uniform_list.rs:17-31 uses `cx.processor(|this, range, window, cx| ..)` (GP/app/context.rs:253). Rows must have uniform height.
- Component options exist if wanted later: `component::table::{DataTable, TableState, TableDelegate}`, `component::list::{List, ListState, ListDelegate}`, `component::{v_virtual_list, VirtualList}` (not signature-verified here).
```rust
use gpui_kit::component::{v_flex, h_flex, button::{Button, ButtonVariants as _}, Sizable as _, scroll::ScrollableElement as _};
v_flex().h_full().overflow_y_scrollbar().gap_1().children(self.rows.iter().map(|r| {
    let id = r.id;
    h_flex().gap_2().child(format!("{}", r.id)).child(r.name.clone()).child(format!("{} chars", r.chars))
        .child(Button::new(("del", id)).label("Delete").small().ghost()
            .on_click(cx.listener(move |this, _e, _w, cx| { this.remove(id); cx.notify(); })))
}))
```
(`Button::new(("del", id))`: ElementId has `From<(&'static str, usize)>`, GP/window.rs:7981.)

## 7. Label / text, Button, Icon, Spinner, Progress
- Text: use plain `div().child("text")` / `String` / `SharedString` + gpui Styled (`.text_sm() .text_xs() .font_semibold() .text_color(cx.theme().muted_foreground)`). `component::label::Label::new(impl Into<SharedString>)` (GC/label.rs:65) with `.secondary(impl Into<SharedString>)` :78, `.masked(bool)` :84, `.highlights(impl Into<HighlightsMatch>)` :90; `Styled`, `RenderOnce`.
- `Button` (`component::button::Button`, GC/button/button.rs:190): `Button::new(id: impl Into<ElementId>)` :243; `.label(impl Into<SharedString>)` :375; `.icon(impl Into<ButtonIcon>)` :401 (IconName/Icon/Spinner/ProgressCircle convert); `.outline()` :336 (INHERENT); `.compact()` :453; `.loading(bool)` :447; `.tooltip(impl Into<SharedString>)` :407; `.on_click(impl Fn(&ClickEvent,&mut Window,&mut App)+'static)` :459; `.on_hover(Fn(&bool,..))`; `.rounded(..)`; `.dropdown_caret(bool)`; `.toggled(bool)`; `.track_focus(&FocusHandle)`. Via traits: `ButtonVariants` (GC/button/button.rs:48; MUST import `button::ButtonVariants`): `.primary() .secondary() .danger() .warning() .success() .info() .ghost() .link() .text() .custom(ButtonCustomVariant)`, `.with_variant(ButtonVariant)`; `Sizable`: `.xsmall() .small() .medium()? .large()` (GC/sizing.rs:190-215, import `Sizable`); `Disableable::disabled(bool)`; `Selectable::selected(bool)`; also Styled/ParentElement/InteractiveElement. `ButtonVariant { Default, Primary, Secondary, Danger, Info, Success, Warning, Ghost, Link, Text, Custom }` :146.
- `Icon::new(impl Into<Icon>)` (GC/icon.rs:136; `.path(..)`, `.rotate(..)`, Sizable `.small()/.size_4()` via Styled, Styled `.text_color(..)`); `IconName` is the legacy PascalCase enum of the bundled 101 icons: ArrowDown/Left/Right/Up, Bell, Calendar, Check, ChevronDown/Left/Right/Up, CircleAlert, CircleCheck, CircleUser, CircleX, Close, Copy, Delete, Ellipsis, EllipsisVertical, ExternalLink, Eye, EyeOff, File, FileText, Folder, FolderClosed, FolderOpen, Globe, Heart, Inbox, Info, LayoutDashboard, Loader, LoaderCircle, Menu, Minus, Moon, Palette, PanelLeft(Open/Close), Pause, Play, Plus, Redo, RefreshCw, RotateCw, Search, Settings, Settings2, Star, Sun, TriangleAlert, Undo, User ... (derived from gpui-kit-assets-0.9.0/default-icons.txt, CamelCase of file names; NOT present: Trash/Pencil/Download -> use `gpui_kit::assets::IconName` + `AllAssets`). Usable as element: `IconName::Check` implements IntoElement (icon.rs:53). Icon in a div: `Icon::new(IconName::Check).size_4().text_color(..)`.
- `Spinner::new()` (GC/spinner.rs:20), `.icon(impl Into<Icon>)`, `.color(Hsla)`, `.ease(..)`, Sizable. `component::progress::Progress::new(id: impl Into<ElementId>)` (progress.rs:26), `.value(f32)` 0.0..=100.0 (:56), `.loading(bool)` (indeterminate) :42, `.color(impl Into<Hsla>)`, `.accessibility_label`, Sizable/Styled. Also `progress::ProgressCircle`. Button `.loading(true)` shows spinner.

## 8. Input & InputState  (`component::input::{Input, InputState, InputEvent}`)
InputState is gpui_base's `type InputState = InputBaseState<InputMode>` (GB/input/input/mod.rs:10); `InputEvent` GB/input/base/state.rs:137.
- Construct (single-line only fn): `InputState::new(window: &mut Window, cx: &mut Context<Self>) -> Self` (state.rs:12555) inside `cx.new(|cx| ..)`.
- Builders on the state (consume self): `.placeholder(impl Into<SharedString>)` :1084; `.default_value(impl Into<SharedString>)` :1610; `.masked(bool)` :12579; `.pattern(regex::Regex)`; `.disabled`-style via setter below.
- Methods: `value(&self) -> SharedString` :1626; `text(&self) -> &Rope`; `set_value(&mut self, value: impl Into<InputContent>, window: &mut Window, cx: &mut Context<Self>)` :1258 (str/String/&String/SharedString ok; does NOT emit `Change` - it sets `emit_events=false` during the replace, :1266-1272; clears undo); `replace_all(text, window, cx)` keeps undo; `set_placeholder(impl Into<SharedString>, &mut Window, &mut Context<Self>)` :1209; `set_masked(bool, &mut Window, &mut Context<Self>)` :12585; `set_disabled(bool, &mut Context<Self>)` :1408; `focus(&self, window, cx)` :1709.
- View: `Input::new(state: &Entity<InputState>) -> Input` (GC/input/input.rs:234): `.id(..)`, `.prefix/.suffix(impl IntoElement)`, `.cleanable(bool)` :338, `.mask_toggle()` :344 (password eye), `.disabled(bool)` :367, `.readonly(bool)` :377, `.appearance(bool)`, `.bordered(bool)`, `.h_full()`, `.content_type(InputContentType)`, `.tab_index`, Sizable, Styled (`.w_80()`), Selectable.
- Events: `#[derive(Clone)] pub enum InputEvent { Change, PressEnter { secondary: bool, shift: bool }, Focus, Blur }`; `InputBaseState<M>: EventEmitter<InputEvent>` (state.rs:745).
- Subscribe, exact gpui-pre 0.3.11 signature (GP/app/context.rs:336):
  `pub fn subscribe_in<Emitter, Evt>(&mut self, emitter: &Entity<Emitter>, window: &Window, mut on_event: impl FnMut(&mut T, &Entity<Emitter>, &Evt, &mut Window, &mut Context<T>) + 'static) -> Subscription where Emitter: EventEmitter<Evt>, Evt: 'static`
  Non-window variant (context.rs:93): `subscribe(&mut self, entity: &Entity<T2>, impl FnMut(&mut T, Entity<T2>, &Evt, &mut Context<T>)) -> Subscription` (note: Entity by value here, `&Entity` in subscribe_in). Keep the `Subscription` in a struct field (`_sub: Subscription`) or call `.detach()` (GP/subscription.rs:278).
```rust
use gpui_kit::component::input::{Input, InputEvent, InputState};
// in MyView::new(window: &mut Window, cx: &mut Context<Self>):
let user = cx.new(|cx| InputState::new(window, cx).placeholder("Username").default_value(saved));
let pass = cx.new(|cx| InputState::new(window, cx).placeholder("Password").masked(true));
let _sub = cx.subscribe_in(&pass, window, |this: &mut Self, state, ev: &InputEvent, window, cx| match ev {
    InputEvent::PressEnter { .. } => { let pw = state.read(cx).value(); this.submit(pw.to_string(), window, cx); }
    InputEvent::Change => { /* user typed; not fired by set_value */ }
    _ => {}
});
// render: Input::new(&self.user).cleanable(true)   Input::new(&self.pass).mask_toggle()
// reset: self.user.update(cx, |s, cx| s.set_value("", window, cx));
// sources: GC/setting/fields/number.rs:91-113 (subscribe_in on InputEvent), GK/tests/integration/input/editing.rs:51-56, overlays.rs:50-52 (value()/set_value)
```

## 9. gpui-pre 0.3.11 async & globals
- `Context<T>::spawn` (GP/app/context.rs:225): `pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R> where T: 'static, AsyncFn: AsyncFnOnce(WeakEntity<T>, &mut AsyncApp) -> R + 'static, R: 'static`. Closure is an async closure: `cx.spawn(async move |this, cx| { .. })` (`this: WeakEntity<T>`, `cx: &mut AsyncApp`). Task must be held or `.detach()`ed (dropping cancels it). `App::spawn(f: AsyncFnOnce(&mut AsyncApp)->R) -> Task<R>` (app.rs:2456).
- With window: `Context<T>::spawn_in(&self, window: &Window, f: AsyncFnOnce(WeakEntity<T>, &mut AsyncWindowContext) -> R) -> Task<R>` (context.rs:649).
- `WeakEntity<T>::update<C: AppContext, R>(&self, cx: &mut C, update: impl FnOnce(&mut T, &mut Context<T>) -> R) -> anyhow::Result<R>` (GP/app/entity_map.rs:803; Err if entity released). `update_in(&self, cx, impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R) -> Result<R>` :819 (use with spawn_in's AsyncWindowContext). `read_with(&self, cx: &C, impl FnOnce(&T,&App)->R) -> Result<R>` :838; `upgrade() -> Option<Entity<T>>`. `AsyncApp::update<R>(&self, f: impl FnOnce(&mut App)->R) -> R` (async_context.rs:176); `AsyncWindowContext::update<R>(&mut self, impl FnOnce(&mut Window,&mut App)->R) -> Result<R>` (:316).
- Background work: `cx.background_executor()` -> `&BackgroundExecutor` (App app.rs:2388, AsyncApp async_context.rs:159; Context derefs to App). `BackgroundExecutor::spawn<R: Send+'static>(&self, future: impl Future<Output=R> + Send + 'static) -> Task<R>` (GP/executor.rs:120). `Task<T>: Future<Output=T>` (scheduler executor.rs:749; `gpui::Task` via `pub use executor::*`, executor.rs:13) -> just `.await` it inside the `cx.spawn` closure, then `this.update(cx, ..)`. Also `.detach()`, `Task::ready(v)`, `TaskExt::detach_and_log_err(cx)` for `Task<Result<..>>`. (Note: gpui's executor is not tokio; async libs needing a tokio reactor must be run via a blocking client or their own runtime - not verified here.)
- Timer: there is NO public `Timer` type; use `BackgroundExecutor::timer(&self, duration: Duration) -> Task<()>` (executor.rs:200). Heartbeat pattern (verbatim shape from GP/examples/layer_shell.rs:36-46 and move_entity_between_windows.rs:38-52):
```rust
fn start_heartbeat(&mut self, cx: &mut Context<Self>) {
    self._hb = Some(cx.spawn(async move |this, cx| {        // field: _hb: Option<Task<()>>, drop = stop
        loop {
            cx.background_executor().timer(Duration::from_secs(30)).await;
            let ok = cx.background_executor().spawn(async move { do_http_blocking() }).await;
            if this.update(cx, |this, cx| { this.status = ok; cx.notify(); }).is_err() { break; } // entity gone
        }
    }));
}
// toast from async: cx.spawn_in(window, async move |this, cx| { this.update_in(cx, |_, window, cx| window.push_notification("x", cx)).ok(); }).detach();
```
- Globals (GP/global.rs:23 `pub trait Global: 'static {}`; impl with `impl Global for MyState {}`). On `App` (Context<T>/AsyncApp deref/delegate): `set_global<G: Global>(&mut self, global: G)` (app.rs:2584); `global<G: Global>(&self) -> &G` (:2546, panics if unset); `try_global<G>(&self) -> Option<&G>` :2554; `global_mut<G>(&mut self) -> &mut G` (~:2560); `has_global::<G>()` :2540; `default_global<G: Global + Default>(&mut self) -> &mut G` :2573; `remove_global`. Also `ReadGlobal::global(cx: &App)` (global.rs:34). From async: `AsyncApp::read_global(read: impl FnOnce(&G,&App)->R) -> R` (async_context.rs:239), `update_global(update: impl FnOnce(&mut G,&mut App)->R) -> R` (:273). Real usage GP/examples/set_menus.rs:33,97,100,131. `component::GlobalState` is gpui-component's own, not needed.

## 10. ActiveTheme  (`gpui_kit::component::ActiveTheme`; GC/theme/mod.rs)
- `pub trait ActiveTheme { fn theme(&self) -> &Theme; }` implemented for `App` (mod.rs:44-53); `cx.theme()` works on `&App`, `&mut App`, `&mut Context<T>` (deref), NOT on `Window`. `Theme: Deref<Target = ThemeColor>` (mod.rs:227) so fields are direct: `cx.theme().background`.
- All `Hsla` (theme_color.rs): `background`(:69) `foreground`(:167) `muted`(:197) `muted_foreground`(:199) `border`(:71) `input`(:177) `primary`(:205) `primary_foreground`(:209) `primary_hover`(:211) `secondary`(:223) `accent`(:63) `accent_foreground`(:65) `danger`(:151) `danger_foreground` `success`(:253) `success_foreground` `warning`(:305) `warning_foreground` `info`(:169) `popover`(:201) `ring`(:215) `selection`(:231) `link`(:179) `overlay`(:313) `red`(:322) `green`(:326); sidebar: `sidebar`(:233) `sidebar_foreground`(:241) `sidebar_border`(:239) `sidebar_accent`(:235) `sidebar_accent_foreground`(:237) `sidebar_primary`(:243) `sidebar_primary_foreground`(:245); also `list*`, `tab*`, `table*`, `title_bar*`, `scrollbar*`.
- Other Theme fields: `mode: ThemeMode{Light(default),Dark}`, `font_family: SharedString`, `font_size: Pixels`, `mono_font_family`, `radius: Pixels`, `radius_lg`, `shadow: bool`, `tokens: ThemeTokens` (Root paints `tokens.background`; Sidebar uses `tokens.sidebar`). `theme.is_dark()` :347. Switch: `Theme::change(mode: impl Into<ThemeMode>, window: Option<&mut Window>, cx: &mut App)` (:396), `Theme::sync_system_appearance(Option<&mut Window>, cx)` (:361).
```rust
div().bg(cx.theme().background).text_color(cx.theme().foreground).border_1().border_color(cx.theme().border)
    .child(div().text_sm().text_color(cx.theme().muted_foreground).child("hint"))
```

## Caveats
- Nothing compiled; signatures quoted from source. `.medium()/.large()` on Sizable and the exact IconName variant spellings beyond those used in-source (e.g. `Settings`, `Loader`, `PanelLeftOpen`, `Check`, `Search`, `Plus`) were derived from file names/`sizing.rs` pattern, not each grepped.
- Table/List/VirtualList component signatures not inspected.
- `Sidebar` renders via a virtual `list` and needs a bounded height (`h_full` inside a sized parent).

## 11. Ribbon widgets (verified 2026-11-07 against gpui-component 0.9.0 source; used in app/ribbon_tabs.rs)
- **Icons:**
  - `Icon::new(icon).with_size(px(26.))` (Sizable) or `.size_8()` (Styled); colour with `.text_color`.
  - Two enums exist:
    - `component::IconName`: the default ~100-icon bundle;
    - `assets::IconName`: all 1830 Lucide icons, which render blank unless embedded.
  - We embed the extras with `gpui_kit::assets::icon_assets!(pub RibbonIcons, [Save, ...])` and an
    `AssetSource` that tries them before `assets::Assets` (`src/icons.rs`).
  - Unknown names fail to compile; check `target/debug/build/gpui-kit-assets-*/out/icon_name.rs`.
- **Menus** (`component::menu::{DropdownMenu as _, PopupMenuItem}`):
  - `Button::new(id).dropdown_menu(|m, _w, _cx| m.item(PopupMenuItem::new(label).checked(b).icon(i).on_click(|_, w, cx| ..)).separator())`.
  - The closure's cx is the menu's, so capture `cx.entity().downgrade()` to reach your view.
- **Select:**
  - `SelectState::new(Vec<String>, Option<IndexPath>, window, cx)`, with `set_items` and
    `set_selected_index`.
  - Event: `SelectEvent<Vec<String>>::Confirm(Option<String>)`.
  - Element: `Select::new(&state).small().placeholder(..).menu_width(..)`.
- **NumberInput:**
  - `NumberInput::new(&input_state).small().suffix("mm")`, where the state is built with
    `InputState::new(..).step(x).min(a).max(b)`.
  - Stepping is internal and emits `InputEvent::Change`.
- **Switch:** `Switch::new(id).small().label(..).checked(b).on_click(|v: &bool, w, cx| ..)`. It is
  controlled: you store the value yourself.
- **Theme accent:**
  - `Theme::global_mut(cx)` is mutable after `Theme::change`.
  - `Button::primary` paints from `theme.tokens.button_primary*` (`ThemeToken`, from `Hsla.into()`),
    so set those **and** `primary*` / `button_primary*` (see `ribbon::set_theme`).
- A `Button` can take custom children (`.child(v_flex()..)`) and styles (`.h(px(68.))`). This is how
  the large icon-over-label ribbon buttons are built.
- **Debug-build stack:** a single function building every ribbon tab overflowed the 1 MB main stack.
  Keep one `#[inline(never)]` fn per tab or panel.
