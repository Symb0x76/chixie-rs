//! Offline demo backend (`CHIXIE_GUI_FAKE=1`): lets the UI run and be autotested without a
//! ChiXie account. Any account logs in with password `demo`; fonts come from the test fixture.

use chixie_core::api::FontInfo;
use chixie_core::font::Font;
use chixie_core::session::Backend;
use chixie_core::{Error, Result};

const FIXTURE: &[u8] = include_bytes!("../../chixie-core/tests/fixtures/font_fixture.json");

pub struct FakeBackend;

fn info(id: i64, name: &str) -> FontInfo {
    FontInfo { id, font_name: name.into(), character_number: 4, default_load_size: None, ..Default::default() }
}

impl Backend for FakeBackend {
    fn login(&mut self, account: &str, password: &str) -> Result<(String, String)> {
        std::thread::sleep(std::time::Duration::from_millis(300)); // feel like a network call
        if password == "demo" {
            Ok((format!("演示用户 {account}"), "vip_user".into()))
        } else {
            Err(Error::Api("user/login: HTTP 400 Bad Request: 账号或密码错误（演示模式密码为 demo）".into()))
        }
    }
    fn logout(&mut self) -> Result<()> {
        Ok(())
    }
    fn heartbeat(&self) -> Result<()> {
        Ok(())
    }
    fn collected_fonts(&self, _page: i64) -> Result<Vec<FontInfo>> {
        Ok(vec![info(1, "演示字体（测试夹具）"), info(2, "演示字体二")])
    }
    fn online_fonts(&self, page: i64, search: &str) -> Result<Vec<FontInfo>> {
        Ok((0..9).map(|i| info(page * 100 + i, &format!("{search}在线字体 {page}-{i}"))).collect())
    }
    fn font(&self, _id: i64) -> Result<Font> {
        Font::from_json(FIXTURE)
    }
}
