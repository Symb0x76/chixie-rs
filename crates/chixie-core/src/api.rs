//! ChiXie backend client (see docs/server-api.md). Minimal: login + font fetch.

use std::time::Duration;

use serde::Deserialize;

use crate::font::Font;
use crate::{Error, Result};

const DEFAULT_BASE: &str = "https://chixie.com.cn/client_api/";

/// Standard response envelope `{status, msg, data, pagination}`.
#[derive(Deserialize)]
struct Envelope<T> {
    status: i64,
    #[serde(default)]
    msg: String,
    data: Option<T>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Login {
    pub token: String,
    pub access_token: String,
    pub backup_token: String,
    pub username: String,
    pub role_code: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub expiry_date: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct FontInfo {
    pub id: i64,
    pub font_name: String,
    pub character_number: i64,
    #[serde(default)]
    pub default_load_size: Option<i64>,
    /// Public PNG of the font's name written in the font (black on white, ~6600×2250),
    /// e.g. `https://chixie.oss-cn-shanghai.aliyuncs.com/font_preview/font_20.png`. Verified 2026-11-07.
    #[serde(default)]
    pub font_preview: Option<String>,
    #[serde(default)]
    pub font_description: Option<String>,
    /// e.g. "中文6900".
    #[serde(default)]
    pub type_name: Option<String>,
    #[serde(default)]
    pub collection_count: Option<i64>,
    /// The server sends a JSON array encoded as a string (`"[\"中文字体\", \"手写字体\"]"`).
    #[serde(default)]
    pub tags: Option<serde_json::Value>,
}

impl FontInfo {
    /// Tags with the redundant "字体" suffix dropped ("手写字体" → "手写").
    pub fn tag_list(&self) -> Vec<String> {
        let list: Vec<String> = match &self.tags {
            Some(serde_json::Value::String(s)) => serde_json::from_str(s).unwrap_or_default(),
            Some(v @ serde_json::Value::Array(_)) => serde_json::from_value(v.clone()).unwrap_or_default(),
            _ => Vec::new(),
        };
        list.into_iter().map(|t| t.strip_suffix("字体").filter(|s| !s.is_empty()).map(str::to_owned).unwrap_or(t)).collect()
    }
}

/// GET a public URL (font preview images); no login, no token.
pub fn fetch_url(url: &str) -> Result<Vec<u8>> {
    let http = reqwest::blocking::Client::builder().timeout(Duration::from_secs(30)).user_agent("chixie-rs").build()?;
    let resp = http.get(url).send()?;
    if !resp.status().is_success() {
        return Err(Error::Api(format!("{url}: HTTP {}", resp.status())));
    }
    Ok(resp.bytes()?.to_vec())
}

pub struct Client {
    base: String,
    http: reqwest::blocking::Client,
    login: Option<Login>,
}

impl Client {
    pub fn new() -> Result<Self> {
        Self::with_base(DEFAULT_BASE)
    }

    pub fn with_base(base: &str) -> Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("chixie-rs")
            .build()?;
        Ok(Self { base: base.trim_end_matches('/').to_string(), http, login: None })
    }

    fn post<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let mut req = self.http.post(format!("{}/{}", self.base, path)).json(body);
        if let Some(l) = &self.login {
            req = req.header("token", &l.token);
        }
        let resp = req.send()?;
        let code = resp.status();
        if !code.is_success() {
            // The backend explains 4xx failures in the envelope's `msg`; surface it.
            let body = resp.text().unwrap_or_default();
            let msg = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("msg").and_then(|m| m.as_str()).map(str::to_owned))
                .unwrap_or(body);
            return Err(Error::Api(format!("{path}: HTTP {code}: {msg}")));
        }
        let env: Envelope<T> = resp.json()?;
        if env.status != 200 {
            return Err(Error::Api(format!("{}: {}", path, env.msg)));
        }
        env.data.ok_or_else(|| Error::Api(format!("{path}: empty data")))
    }

    pub fn login(&mut self, account: &str, password: &str) -> Result<&Login> {
        let body = serde_json::json!({ "account": account, "password": password, "platform": "windows" });
        let l: Login = self.post("user/login", &body)?;
        self.login = Some(l);
        Ok(self.login.as_ref().unwrap())
    }

    pub fn is_logged_in(&self) -> bool {
        self.login.is_some()
    }

    /// Keep-alive; ChiXie3 calls this periodically (`HeartbeatInterval`). Verified by capture:
    /// request `{}`, response `data: "ok"`.
    pub fn heartbeat(&self) -> Result<()> {
        let _: serde_json::Value = self.post("user/heartbeat", &serde_json::json!({}))?;
        Ok(())
    }

    /// End the session server-side, then forget the tokens locally (even if the call fails,
    /// so a dead session never lingers). The request body was not captured; `{}` is a best guess.
    /// VERIFY.
    pub fn logout(&mut self) -> Result<()> {
        let r = self.post::<_, serde_json::Value>("user/logout", &serde_json::json!({})).map(|_| ());
        self.login = None;
        r
    }

    pub fn collected_fonts(&self, page_num: i64) -> Result<Vec<FontInfo>> {
        self.post("font/get_collected_fonts", &serde_json::json!({ "page_num": page_num }))
    }

    /// One page of the full online catalog. `page_num` is 1-based.
    pub fn online_fonts(&self, page_num: i64, search: &str) -> Result<Vec<FontInfo>> {
        let body = serde_json::json!({
            "filter_tags": ["chinese", "english", "Japanese", "handwritten", "standard", "formula_symbol", "scribble"],
            "sort_rule": "collections",
            "page_num": page_num,
            "search_name": search,
        });
        self.post("font/get_online_fonts", &body)
    }

    /// Fetch and decrypt one font by id (requires prior login).
    pub fn font(&self, font_id: i64) -> Result<Font> {
        let l = self.login.as_ref().ok_or_else(|| Error::Api("not logged in".into()))?;
        let body = serde_json::json!({ "font_id": font_id, "token": l.token });
        let blob: String = self.post("font/get_font_access", &body)?;
        Font::from_encrypted_with_backup_token(&blob, &l.backup_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_info_parses_listing_with_string_tags() {
        // Shape of a real get_online_fonts item (2026-11-07).
        let j = r#"{"id":20,"font_name":"最丑字体","character_number":6898,"collection_count":12,
            "content_preview":null,"font_description":"来源于老版用户录制","permission":1,"type_name":"中文6900",
            "font_preview":"https://chixie.oss-cn-shanghai.aliyuncs.com/font_preview/font_20.png",
            "tags":"[\"中文字体\", \"手写字体\", \"潦草字体\"]"}"#;
        let f: FontInfo = serde_json::from_str(j).unwrap();
        assert_eq!(f.tag_list(), ["中文", "手写", "潦草"]);
        assert!(f.font_preview.unwrap().ends_with("font_20.png"));
        assert_eq!(f.collection_count, Some(12));
        let bare: FontInfo = serde_json::from_str(r#"{"id":1,"font_name":"x","character_number":1}"#).unwrap();
        assert!(bare.tag_list().is_empty() && bare.font_preview.is_none());
    }
}
