//! App-wide state shared by all views, stored as a gpui global. Views observe it with
//! `cx.observe_global::<AppState>` and re-render; mutate it via `cx.global_mut::<AppState>()`
//! (which notifies observers).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chixie_core::config::Settings;
use chixie_core::font::Font;
use chixie_core::font_store::FontStore;
use chixie_core::session::SessionHandle;
use chixie_core::template::{Template, TemplateStore};
use gpui_kit::Global;

use crate::cloud_preview::CloudPreview;
use crate::font_preview::FontPreview;

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
    pub templates: TemplateStore,
    /// Downloaded fonts kept on this computer (work without login).
    pub fonts: std::sync::Arc<FontStore>,
    pub templates_path: PathBuf,
    /// Font picker previews by font id (built from local fonts in the background).
    pub previews: HashMap<i64, Arc<FontPreview>>,
    /// Server preview images of fonts that are not downloaded.
    pub cloud_previews: HashMap<i64, CloudPreview>,
}

impl Global for AppState {}

impl AppState {
    /// Persist settings; failures are logged, never fatal for the UI.
    pub fn save_settings(&self) {
        if let Err(e) = self.settings.save(&self.settings_path) {
            log::warn!("saving settings to {}: {e}", self.settings_path.display());
        }
    }

    /// The page template in use (`settings.page.template`), if it still exists.
    pub fn active_template(&self) -> Option<Template> {
        self.settings.page.template.as_ref().and_then(|n| self.templates.get(n)).cloned()
    }

    pub fn save_templates(&self) {
        if let Err(e) = self.templates.save(&self.templates_path) {
            log::warn!("saving templates to {}: {e}", self.templates_path.display());
        }
    }
}
