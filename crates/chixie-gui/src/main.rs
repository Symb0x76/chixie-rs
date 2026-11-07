//! ChiXie desktop client (gpui + gpui-component via gpui-kit).
//!
//! Environment switches:
//! - `CHIXIE_GUI_FAKE=1`: offline demo backend (any account, password `demo`), settings in a temp
//!   dir so the real settings file and credential store are untouched.
//! - `CHIXIE_GUI_AUTOTEST=1` (implies FAKE): log in, open Fonts, load a font, print one
//!   `AUTOTEST ...` line and quit. Used to verify the UI without input injection.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod fake;
mod fonts;
mod login;
mod state;

use std::path::PathBuf;
use std::time::Duration;

use chixie_core::config::Settings;
use chixie_core::session::{self, HEARTBEAT_EVERY};
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

use crate::app::{ChiXieApp, Page};
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
        if settings.ui.dark {
            Theme::change(ThemeMode::Dark, None, cx);
        }
        cx.set_global(AppState { settings, settings_path, session, user: None, active_font: None });

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1180.), px(760.)), cx)),
            titlebar: Some(TitlebarOptions { title: Some("赤写 ChiXie".into()), ..Default::default() }),
            ..Default::default()
        };
        let (_handle, app) = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| ChiXieApp::new(events, window, cx)))
            .expect("open window");
        cx.activate(true);

        if autotest {
            run_autotest(app, cx);
        }
    });
}

/// Scripted smoke run against the fake backend; prints one result line, then quits.
fn run_autotest(app: Entity<ChiXieApp>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let pause = |ms| Duration::from_millis(ms);
        cx.background_executor().timer(pause(400)).await;
        app.update(cx, |a, cx| {
            a.login.update(cx, |l, cx| l.submit_values("13800000000".into(), "demo".into(), false, cx));
        });
        cx.background_executor().timer(pause(1200)).await;
        app.update(cx, |a, cx| {
            a.set_page(Page::Fonts, cx);
            a.fonts.update(cx, |f, cx| f.use_font(1, cx));
        });
        cx.background_executor().timer(pause(1500)).await;
        cx.update(|cx| {
            let st = cx.global::<AppState>();
            println!(
                "AUTOTEST user={:?} active_font={:?} glyphs={} account_saved={:?} font_id_saved={:?}",
                st.user.as_ref().map(|u| u.name.clone()),
                st.active_font.as_ref().map(|f| f.name.clone()),
                st.active_font.as_ref().map(|f| f.font.glyphs.len()).unwrap_or(0),
                st.settings.account,
                st.settings.page.font_id,
            );
        });
        cx.background_executor().timer(pause(600)).await; // leave a frame for a screenshot
        cx.update(|cx| cx.quit());
    })
    .detach();
}
