//! ChiXie desktop client (gpui + gpui-component via gpui-kit).
//!
//! Environment switches:
//! - `CHIXIE_GUI_FAKE=1`: offline demo backend (any account, password `demo`), settings in a temp
//!   dir so the real settings file and credential store are untouched.
//! - `CHIXIE_GUI_AUTOTEST=1` (implies FAKE): log in, load a font, type and edit on the page through
//!   the IME input handler and editor actions, export, print one `AUTOTEST ...` line and quit.
//!   Used to verify the UI without input injection.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod doc_view;
mod fake;
mod file_ops;
mod fonts;
mod login;
mod state;

use std::path::PathBuf;
use std::time::Duration;

use chixie_core::config::Settings;
use chixie_core::editor::EditDoc;
use chixie_core::session::{self, HEARTBEAT_EVERY};
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

use crate::app::ChiXieApp;
use crate::state::AppState;

fn flag(name: &str) -> bool {
    std::env::var_os(name).is_some()
}

fn main() {
    env_logger::init();
    let autotest = flag("CHIXIE_GUI_AUTOTEST");
    let fake = autotest || flag("CHIXIE_GUI_FAKE");

    let settings_path = if fake {
        std::env::temp_dir().join("chixie-rs-demo").join("settings.toml")
    } else {
        Settings::default_path().unwrap_or_else(|_| PathBuf::from("settings.toml"))
    };
    let settings = Settings::load(&settings_path).unwrap_or_else(|e| {
        log::warn!("{e}; using defaults");
        Settings::default()
    });

    let (session, events, _join) = if fake {
        session::spawn(fake::FakeBackend, HEARTBEAT_EVERY)
    } else {
        let client = chixie_core::api::Client::new().expect("http client");
        session::spawn(client, HEARTBEAT_EVERY)
    };

    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        doc_view::bind_keys(cx);
        if settings.ui.dark {
            Theme::change(ThemeMode::Dark, None, cx);
        }
        cx.set_global(AppState { settings, settings_path, session, user: None, active_font: None });

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1180.), px(760.)), cx)),
            titlebar: Some(TitlebarOptions { title: Some("赤写 ChiXie".into()), ..Default::default() }),
            ..Default::default()
        };
        let (handle, app) = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| ChiXieApp::new(events, window, cx)))
            .expect("open window");
        cx.activate(true);

        if autotest {
            run_autotest(handle, app, cx);
        }
    });
}

/// Scripted smoke run against the fake backend; prints one result line, then quits.
/// Login → load font → type through the IME input handler (wraps onto 2 pages, one missing
/// glyph) → caret moves, selection, delete, undo via dispatched actions → export.
fn run_autotest(handle: AnyWindowHandle, app: Entity<ChiXieApp>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let pause = |ms| Duration::from_millis(ms);
        cx.background_executor().timer(pause(400)).await;
        app.update(cx, |a, cx| {
            a.login.update(cx, |l, cx| l.submit_values("13800000000".into(), "demo".into(), false, cx));
        });
        cx.background_executor().timer(pause(1200)).await;
        app.update(cx, |a, cx| a.fonts.update(cx, |f, cx| f.use_font(1, cx)));
        cx.background_executor().timer(pause(1500)).await;
        // The fixture font has 你 好 A . ; 写 is deliberately missing.
        let mut steps = Vec::new();
        let _ = cx.update_window(handle, |_, window, cx| {
            let doc = app.read(cx).doc.clone();
            doc.update(cx, |d, cx| {
                d.focus(window, cx);
                d.replace_doc(EditDoc::from_text(""), cx);
                // Pinyin composition, then commit (what Microsoft Pinyin does).
                d.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx);
                d.replace_and_mark_text_in_range(None, "你", Some(1..1), window, cx);
                d.replace_text_in_range(None, "你", window, cx);
                d.replace_text_in_range(None, &format!("好 A.写\n{}", "你好".repeat(400)), window, cx);
                steps.push(format!("typed={}", d.doc().len()));
            });
        });
        // Editor actions through the real dispatch path (same as key bindings). Dispatch is
        // deferred to the next effect cycle and routed via the focused node of the rendered
        // frame, so wait a frame before dispatching and before reading the result.
        let head = |cx: &mut App| app.read(cx).doc.read(cx).doc().chars().iter().take(4).collect::<String>();
        let dispatch = |cx: &mut AsyncApp, actions: Vec<Box<dyn Action>>| {
            let _ = cx.update_window(handle, |_, window, cx| {
                for a in actions {
                    window.dispatch_action(a, cx);
                }
            });
        };
        cx.background_executor().timer(pause(300)).await;
        dispatch(
            cx,
            vec![Box::new(doc_view::DocStart), Box::new(doc_view::SelectRight), Box::new(doc_view::SelectRight), Box::new(doc_view::Delete)],
        );
        cx.background_executor().timer(pause(300)).await;
        steps.push(format!("after_delete={:?}", cx.update(head)));
        dispatch(cx, vec![Box::new(doc_view::Undo)]);
        cx.background_executor().timer(pause(300)).await;
        steps.push(format!("after_undo={:?}", cx.update(head)));
        dispatch(cx, vec![Box::new(doc_view::DocEnd)]);
        cx.background_executor().timer(pause(800)).await;
        cx.update(|cx| {
            let st = cx.global::<AppState>();
            let d = app.read(cx).doc.read(cx);
            let doc = d.document();
            let export = std::env::temp_dir().join("chixie-autotest.gcode");
            let files = file_ops::export_files(&doc, &st.settings.device.profile, &export).map(|f| f.len()).unwrap_or(0);
            println!(
                "AUTOTEST user={:?} active_font={:?} {} chars={} caret={} caret_page={} pages={} strokes={} missing={:?} bounds={:?} exported_files={}",
                st.user.as_ref().map(|u| u.name.clone()),
                st.active_font.as_ref().map(|f| f.name.clone()),
                steps.join(" "),
                d.doc().len(),
                d.doc().caret(),
                d.caret_page(),
                doc.pages.len(),
                doc.stroke_count(),
                doc.missing,
                doc.bounds(),
                files,
            );
        });
        cx.background_executor().timer(pause(3000)).await; // leave time for a screenshot
        cx.update(|cx| cx.quit());
    })
    .detach();
}

