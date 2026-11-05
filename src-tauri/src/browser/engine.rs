//! Rendering-engine abstraction. The browser core talks to web content only through these
//! traits, so the WebView2 implementation in [`super::webview`] can be replaced (or mocked)
//! without touching tab management, storage or commands.

use std::path::PathBuf;
use std::sync::Arc;

use url::Url;

use super::shortcuts::Shortcut;
use super::tabs::TabId;
use crate::error::Result;
use crate::storage::settings::{Theme, TrackingPrevention};

/// A rectangle in logical (DPI-independent) pixels relative to the window's client area.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDecision {
    Allow,
    Cancel,
}

/// Events raised by a content view, delivered to an [`EngineSink`].
#[derive(Debug, Clone)]
pub enum EngineEvent {
    LoadStarted { url: String },
    LoadFinished,
    UrlChanged { url: String },
    TitleChanged { title: String },
    FaviconChanged { data_url: Option<String> },
    HistoryChanged { can_go_back: bool, can_go_forward: bool },
    /// `url` is the document actually shown afterwards (unchanged when a navigation became a download).
    NavigationCompleted { success: bool, error: Option<String>, url: String },
    OpenInNewTab { url: String },
    Shortcut(Shortcut),
    FullscreenChanged(bool),
    ProcessFailed { reason: String },
}

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub suggested_path: PathBuf,
    pub mime_type: Option<String>,
    pub total_bytes: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct DownloadTicket {
    pub id: i64,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum DownloadUpdate {
    Progress { received: i64, total: Option<i64> },
    Paused,
    Resumed,
    Completed { received: i64 },
    Interrupted { reason: String, can_resume: bool, cancelled: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadAction {
    Pause,
    Resume,
    Cancel,
}

/// Receives events from content views. Implemented by the browser core.
pub trait EngineSink: Send + Sync + 'static {
    fn navigation_starting(&self, tab: TabId, url: &Url) -> NavigationDecision;
    fn event(&self, tab: TabId, event: EngineEvent);
    fn download_starting(&self, tab: TabId, request: DownloadRequest) -> Option<DownloadTicket>;
    fn download_update(&self, id: i64, update: DownloadUpdate);
}

/// Preferences applied to the shared browsing profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProfilePrefs {
    pub theme: Theme,
    pub tracking_prevention: TrackingPrevention,
    pub password_autosave: bool,
    pub general_autofill: bool,
}

#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearKinds {
    pub cookies: bool,
    pub cache: bool,
    pub site_storage: bool,
    pub passwords: bool,
    pub autofill: bool,
}

#[derive(Debug, Clone)]
pub struct ImportedCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    /// Seconds since the Unix epoch; `None` for session cookies.
    pub expires: Option<f64>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<SameSite>,
}

#[derive(Debug, Clone, Copy)]
pub enum SameSite {
    None,
    Lax,
    Strict,
}

/// One rendered web page (a tab's browsing context).
pub trait ContentView: Send + Sync {
    fn navigate(&self, url: &Url) -> Result<()>;
    fn go_back(&self) -> Result<()>;
    fn go_forward(&self) -> Result<()>;
    fn reload(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn set_bounds(&self, bounds: Bounds) -> Result<()>;
    fn set_visible(&self, visible: bool) -> Result<()>;
    fn focus(&self) -> Result<()>;
    fn close(&self) -> Result<()>;
}

pub trait RenderingEngine: Send + Sync {
    /// Creates a view for `tab` and starts loading `url`. Must not be called from the UI thread.
    fn create_view(
        &self,
        tab: TabId,
        url: &Url,
        bounds: Bounds,
        visible: bool,
        sink: Arc<dyn EngineSink>,
    ) -> Result<Arc<dyn ContentView>>;
    fn control_download(&self, id: i64, action: DownloadAction) -> Result<()>;
    fn apply_profile(&self, prefs: ProfilePrefs) -> Result<()>;
    fn clear_browsing_data(&self, kinds: ClearKinds) -> Result<()>;
    /// Adds cookies to the shared profile; returns how many were accepted.
    fn import_cookies(&self, cookies: Vec<ImportedCookie>) -> Result<usize>;
}
