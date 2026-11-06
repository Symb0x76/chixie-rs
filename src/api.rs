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

#[derive(Deserialize, Debug, Clone)]
pub struct FontInfo {
    pub id: i64,
    pub font_name: String,
    pub character_number: i64,
    #[serde(default)]
    pub default_load_size: Option<i64>,
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
        let env: Envelope<T> = req.send()?.error_for_status()?.json()?;
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
