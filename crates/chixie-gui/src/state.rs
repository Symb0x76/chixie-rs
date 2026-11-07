//! App-wide state shared by all views, stored as a gpui global. Views observe it with
//! `cx.observe_global::<AppState>` and re-render; mutate it via `cx.global_mut::<AppState>()`
//! (which notifies observers).

use std::path::PathBuf;
use std::sync::Arc;

use chixie_core::config::Settings;
use chixie_core::font::Font;
use chixie_core::session::SessionHandle;
use gpui_kit::Global;

pub struct User {
    pub name: String,
    pub role: String,
}

/// The decrypted font currently used for writing (memory only; tied to this login).
pub struct ActiveFont {
    pub id: i64,
    pub name: String,
    pub font: Arc<Font>,
}

pub struct AppState {
    pub settings: Settings,
    pub settings_path: PathBuf,
    pub session: SessionHandle,
    pub user: Option<User>,
    pub active_font: Option<ActiveFont>,
}

impl Global for AppState {}

impl AppState {
    /// Persist settings; failures are logged, never fatal for the UI.
    pub fn save_settings(&self) {
        if let Err(e) = self.settings.save(&self.settings_path) {
            log::warn!("saving settings to {}: {e}", self.settings_path.display());
        }
    }
}
