//! Browser core: owns the tab model, maps tabs to content views, lays views out inside the
//! window, persists the session and reacts to engine events.
//!
//! Threading rules:
//! * Engine callbacks (`EngineSink`) run on the UI thread. They only update state and emit
//!   events; anything that touches a view is moved to a worker thread, because WebView2 must not
//!   be re-entered from inside its own event handlers.
//! * No lock is held while calling into the engine.

pub mod engine;
pub mod navigation;
pub mod shortcuts;
pub mod tabs;
#[cfg(windows)]
pub mod webview;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, EventTarget, Manager};
use url::Url;

use crate::authentication::{oauth, AccountStore};
use crate::downloads::DownloadManager;
use crate::error::{Error, Result};
use crate::security::{self, SchemePolicy, UI_WEBVIEW};
use crate::storage::favicons::host_of;
use crate::storage::session::{SessionFile, SessionTab};
use crate::storage::settings::{Settings, SettingsStore, Startup};
use crate::storage::Storage;
use engine::*;
use navigation::{InternalPage, Resolved};
use shortcuts::Shortcut;
use tabs::{GroupId, Snapshot, Tab, TabId, TabManager};

pub const EVT_TABS: &str = "zuno://tabs";
pub const EVT_TAB: &str = "zuno://tab";
pub const EVT_SHORTCUT: &str = "zuno://shortcut";
pub const EVT_DOWNLOAD: &str = "zuno://download";
pub const EVT_AUTH: &str = "zuno://auth";
pub const EVT_EXTERNAL: &str = "zuno://external";
pub const EVT_FULLSCREEN: &str = "zuno://fullscreen";
pub const EVT_IMPORT: &str = "zuno://import-progress";
pub const EVT_SETTINGS: &str = "zuno://settings";
pub const EVT_ACCOUNT: &str = "zuno://account";
pub const EVT_NOTICE: &str = "zuno://notice";
pub const EVT_UI_ACTION: &str = "zuno://ui-action";

/// Space reserved around the content area by Zuno's interface, in logical pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Insets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug, Default)]
struct Layout {
    insets: Insets,
    /// A modal surface (quick switcher, dialog) is open; content views are hidden beneath it.
    overlay: bool,
    fullscreen: bool,
    /// The tab whose view is currently shown.
    shown: Option<TabId>,
    ready: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NavigateOutcome {
    Navigated,
    /// The input is an OAuth authorization request; the UI shows it for review first.
    AuthReview { request: oauth::AuthorizationRequest },
    /// The address uses an external application's scheme; the UI asks for confirmation.
    External,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum QuickResult {
    Tab { id: TabId, title: String, url: String, favicon: Option<String> },
    Bookmark { id: i64, title: String, url: String, favicon: Option<String> },
    History { id: i64, title: String, url: String, favicon: Option<String>, visited_at: i64 },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExternalRequest {
    tab: TabId,
    token: String,
    scheme: String,
    display: String,
    auth_callback: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthEvent {
    tab: TabId,
    client_id: String,
    provider: String,
    delivered_to: String,
    outcome: oauth::CallbackOutcome,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub level: &'static str,
    pub message: String,
}

pub struct Browser {
    app: AppHandle,
    engine: Arc<dyn RenderingEngine>,
    tabs: Mutex<TabManager>,
    views: Mutex<HashMap<TabId, Arc<dyn ContentView>>>,
    layout: Mutex<Layout>,
    pub storage: Arc<Storage>,
    pub settings: Arc<SettingsStore>,
    pub downloads: Arc<DownloadManager>,
    pub accounts: Arc<AccountStore>,
    session_file: SessionFile,
    /// User-reviewed authorization requests per tab, awaiting their redirect.
    auth: Mutex<HashMap<TabId, oauth::AuthorizationRequest>>,
    /// Last history row recorded per tab: (url, row id), used to attach late titles.
    visits: Mutex<HashMap<TabId, (String, i64)>>,
    closed: Mutex<Vec<SessionTab>>,
    /// Tabs whose current navigation turned into a download (not an error, page stays as is).
    downloading: Mutex<std::collections::HashSet<TabId>>,
    /// External-application URLs awaiting user confirmation, by one-time token.
    external: Mutex<HashMap<String, Url>>,
    save_pending: AtomicBool,
    pub http: reqwest::Client,
    /// Set in an incognito window: nothing about the browsing is recorded (see [`crate::incognito`]).
    incognito: Option<crate::incognito::Session>,
}

struct Sink(Weak<Browser>);

impl Browser {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app: AppHandle,
        engine: Arc<dyn RenderingEngine>,
        storage: Arc<Storage>,
        settings: Arc<SettingsStore>,
        downloads: Arc<DownloadManager>,
        accounts: Arc<AccountStore>,
        session_file: SessionFile,
        incognito: Option<crate::incognito::Session>,
    ) -> Result<Arc<Self>> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("Zuno/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Arc::new(Self {
            app,
            engine,
            tabs: Mutex::new(TabManager::new()),
            views: Mutex::new(HashMap::new()),
            layout: Mutex::new(Layout::default()),
            storage,
            settings,
            downloads,
            accounts,
            session_file,
            auth: Mutex::new(HashMap::new()),
            visits: Mutex::new(HashMap::new()),
            closed: Mutex::new(Vec::new()),
            downloading: Mutex::new(Default::default()),
            external: Mutex::new(HashMap::new()),
            save_pending: AtomicBool::new(false),
            http,
            incognito,
        }))
    }

    pub fn is_incognito(&self) -> bool {
        self.incognito.is_some()
    }

    pub fn incognito_dir(&self) -> Option<&std::path::Path> {
        self.incognito.as_ref().map(|s| s.dir.as_path())
    }

    /// Rejects actions that would write to the regular profile from an incognito window.
    pub fn ensure_regular(&self, what: &str) -> Result<()> {
        if self.is_incognito() {
            return Err(Error::msg(format!("{what} isn't available in an incognito window")));
        }
        Ok(())
    }

    pub fn sink(self: &Arc<Self>) -> Arc<dyn EngineSink> {
        Arc::new(Sink(Arc::downgrade(self)))
    }

    pub fn engine(&self) -> &dyn RenderingEngine {
        self.engine.as_ref()
    }

    // -----------------------------------------------------------------------------------------
    // Events
    // -----------------------------------------------------------------------------------------

    pub fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        if let Err(e) = self.app.emit_to(EventTarget::webview(UI_WEBVIEW), event, payload) {
            tracing::warn!(event, error = %e, "failed to emit event");
        }
    }

    pub fn notice(&self, level: &'static str, message: impl Into<String>) {
        self.emit(EVT_NOTICE, Notice { level, message: message.into() });
    }

    pub fn snapshot(&self) -> Snapshot {
        self.tabs.lock().snapshot()
    }

    fn emit_tabs(self: &Arc<Self>) {
        let snapshot = self.snapshot();
        self.emit(EVT_TABS, snapshot);
        self.mark_session_dirty();
    }

    fn emit_tab(&self, id: TabId) {
        let tab = self.tabs.lock().get(id).cloned();
        if let Some(tab) = tab {
            self.emit(EVT_TAB, tab);
        }
    }

    fn update_tab(&self, id: TabId, f: impl FnOnce(&mut Tab)) -> bool {
        let mut tabs = self.tabs.lock();
        match tabs.get_mut(id) {
            Some(tab) => {
                f(tab);
                true
            }
            None => false,
        }
    }

    /// Runs `f` on a worker thread. Used from engine callbacks, which run on the UI thread.
    fn spawn(self: &Arc<Self>, f: impl FnOnce(Arc<Self>) + Send + 'static) {
        let this = self.clone();
        tauri::async_runtime::spawn_blocking(move || f(this));
    }

    // -----------------------------------------------------------------------------------------
    // Session
    // -----------------------------------------------------------------------------------------

    fn mark_session_dirty(self: &Arc<Self>) {
        if self.is_incognito() || self.save_pending.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(800));
            if let Some(this) = weak.upgrade() {
                this.save_pending.store(false, Ordering::SeqCst);
                this.save_session();
            }
        });
    }

    pub fn save_session(&self) {
        if self.is_incognito() {
            return;
        }
        let session = self.tabs.lock().to_session();
        if let Err(e) = self.session_file.save(&session) {
            tracing::warn!(error = %e, "failed to save session");
        }
    }

    /// Opens the startup tabs. Called once when the interface has loaded and measured itself.
    pub fn start(self: &Arc<Self>, insets: Insets) -> Result<()> {
        let already_started = {
            let mut layout = self.layout.lock();
            layout.insets = insets;
            std::mem::replace(&mut layout.ready, true)
        };
        if already_started {
            // The interface reloaded (e.g. during development): keep the tabs, refresh the layout.
            return self.relayout();
        }
        let settings = self.settings.get();
        // Incognito windows always start empty and never reopen the regular session.
        let restored = match settings.startup {
            Startup::Restore if !self.is_incognito() => self.session_file.load().filter(|s| !s.tabs.is_empty()),
            _ => None,
        };
        {
            let mut tabs = self.tabs.lock();
            match restored {
                Some(session) => tabs.restore(&session),
                None => {
                    let url = match settings.startup {
                        Startup::Homepage if !self.is_incognito() => settings.homepage.clone(),
                        _ => InternalPage::NewTab.url().to_string(),
                    };
                    tabs.add(&url, &title_for_url(&url), None, true);
                }
            }
        }
        self.emit_tabs();
        if self.is_incognito() && settings.incognito_randomize_mac {
            self.randomize_mac();
        }
        self.sync_views()
    }

    /// Starts MAC address randomization for this incognito window and reports the outcome.
    fn randomize_mac(self: &Arc<Self>) {
        let Some(status) = self.incognito.as_ref().map(|s| s.mac_status_file()) else { return };
        let owner = self.app.get_window(crate::MAIN_WINDOW).and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize);
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            use crate::incognito::mac;
            let outcome = mac::start(&status, owner);
            let Some(this) = weak.upgrade() else { return };
            match outcome {
                Ok(mac::Outcome::AlreadyActive) => this.notice("info", "Your MAC address is already randomized by another incognito window"),
                Ok(mac::Outcome::Declined) => this.notice("error", "MAC address not changed: administrator approval was declined"),
                Ok(mac::Outcome::Started) => {
                    this.notice("info", "Randomizing your MAC address — the network reconnects for a few seconds");
                    drop(this);
                    let results = mac::wait_for_status(&status, Duration::from_secs(90));
                    let Some(this) = weak.upgrade() else { return };
                    match results {
                        Some(results) => {
                            let (level, message) = mac::describe(&results);
                            this.notice(level, message);
                        }
                        None => this.notice("error", "The MAC address helper did not report back; your address may be unchanged"),
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "MAC randomization failed");
                    this.notice("error", format!("MAC address not changed: {e}"));
                }
            }
        });
    }

    // -----------------------------------------------------------------------------------------
    // Layout
    // -----------------------------------------------------------------------------------------

    fn content_bounds(&self) -> Result<Bounds> {
        let window = self.app.get_window(crate::MAIN_WINDOW).ok_or(Error::NotFound)?;
        let scale = window.scale_factor()?;
        let size = window.inner_size()?.to_logical::<f64>(scale);
        let layout = self.layout.lock();
        if layout.fullscreen {
            return Ok(Bounds { x: 0.0, y: 0.0, width: size.width, height: size.height });
        }
        let i = layout.insets;
        Ok(Bounds {
            x: i.left,
            y: i.top,
            width: (size.width - i.left - i.right).max(1.0),
            height: (size.height - i.top - i.bottom).max(1.0),
        })
    }

    pub fn set_insets(self: &Arc<Self>, insets: Insets) -> Result<()> {
        self.layout.lock().insets = insets;
        self.relayout()
    }

    pub fn set_overlay(self: &Arc<Self>, overlay: bool) -> Result<()> {
        let changed = {
            let mut layout = self.layout.lock();
            let changed = layout.overlay != overlay;
            layout.overlay = overlay;
            changed
        };
        if changed {
            self.sync_views()?;
        }
        Ok(())
    }

    /// Repositions the visible view (window resized or interface insets changed).
    pub fn relayout(&self) -> Result<()> {
        let shown = self.layout.lock().shown;
        if let Some(id) = shown {
            let view = self.views.lock().get(&id).cloned();
            if let Some(view) = view {
                view.set_bounds(self.content_bounds()?)?;
            }
        }
        Ok(())
    }

    /// Ensures exactly the active tab's view is visible (or none for internal pages and while an
    /// overlay is open), creating the view lazily for restored tabs.
    pub fn sync_views(self: &Arc<Self>) -> Result<()> {
        let (active, url) = {
            let tabs = self.tabs.lock();
            match tabs.active().and_then(|id| tabs.get(id)) {
                Some(t) => (Some(t.id), t.url.clone()),
                None => (None, String::new()),
            }
        };
        let (overlay, ready) = {
            let l = self.layout.lock();
            (l.overlay, l.ready)
        };
        if !ready {
            return Ok(());
        }
        let wants_view = active.filter(|_| !navigation::is_internal(&url));
        if let Some(id) = wants_view {
            if !self.views.lock().contains_key(&id) {
                if let Ok(parsed) = Url::parse(&url) {
                    self.create_view(id, &parsed, !overlay)?;
                }
            }
        }
        let target = wants_view.filter(|_| !overlay);
        let previous = std::mem::replace(&mut self.layout.lock().shown, target);
        let bounds = self.content_bounds()?;
        if let Some(id) = target {
            let view = self.views.lock().get(&id).cloned();
            if let Some(view) = view {
                view.set_bounds(bounds)?;
                view.set_visible(true)?;
                if previous != target {
                    let _ = view.focus();
                }
            }
        }
        if previous != target {
            if let Some(prev) = previous {
                let view = self.views.lock().get(&prev).cloned();
                if let Some(view) = view {
                    view.set_visible(false)?;
                }
            }
        }
        Ok(())
    }

    fn create_view(self: &Arc<Self>, id: TabId, url: &Url, visible: bool) -> Result<()> {
        let bounds = self.content_bounds()?;
        let view = self.engine.create_view(id, url, bounds, visible, self.sink())?;
        self.views.lock().insert(id, view);
        self.update_tab(id, |t| {
            t.live = true;
            t.loading = true;
        });
        self.emit_tab(id);
        Ok(())
    }

    fn view(&self, id: TabId) -> Option<Arc<dyn ContentView>> {
        self.views.lock().get(&id).cloned()
    }

    fn drop_view(&self, id: TabId) {
        let view = self.views.lock().remove(&id);
        if let Some(view) = view {
            if let Err(e) = view.close() {
                tracing::warn!(error = %e, "failed to close view");
            }
        }
        let mut layout = self.layout.lock();
        if layout.shown == Some(id) {
            layout.shown = None;
        }
    }

    pub fn focus_ui(&self) {
        if let Some(ui) = self.app.get_webview(UI_WEBVIEW) {
            let _ = ui.set_focus();
        }
    }

    pub fn focus_content(&self) {
        let shown = self.layout.lock().shown;
        if let Some(view) = shown.and_then(|id| self.view(id)) {
            let _ = view.focus();
        }
    }

    // -----------------------------------------------------------------------------------------
    // Tabs
    // -----------------------------------------------------------------------------------------

    fn search_template(&self) -> String {
        search_template(&self.settings.get())
    }

    pub fn new_tab(self: &Arc<Self>, input: Option<&str>, after: Option<TabId>, activate: bool) -> Result<TabId> {
        let resolved = match input.map(str::trim).filter(|s| !s.is_empty()) {
            Some(text) => navigation::resolve_input(text, &self.search_template())?,
            None => Resolved::Internal(InternalPage::NewTab),
        };
        let url = resolved.url_string();
        let id = self.tabs.lock().add(&url, &title_for_url(&url), after, activate);
        self.emit_tabs();
        if !matches!(resolved, Resolved::Internal(_)) {
            self.load(id, resolved)?;
        }
        self.sync_views()?;
        Ok(id)
    }

    /// Opens an internal page, reusing an existing tab that already shows it.
    pub fn open_page(self: &Arc<Self>, page: InternalPage) -> Result<TabId> {
        let (existing, active) = {
            let tabs = self.tabs.lock();
            (tabs.tabs().iter().find(|t| InternalPage::parse(&t.url) == Some(page)).map(|t| t.id), tabs.active())
        };
        match existing {
            Some(id) => {
                self.activate(id)?;
                Ok(id)
            }
            None => self.new_tab(Some(page.url()), active, true),
        }
    }

    pub fn activate(self: &Arc<Self>, id: TabId) -> Result<()> {
        if !self.tabs.lock().activate(id) {
            return Err(Error::NotFound);
        }
        self.emit_tabs();
        self.sync_views()
    }

    pub fn close_tab(self: &Arc<Self>, id: TabId) -> Result<()> {
        let closed = {
            let mut tabs = self.tabs.lock();
            let Some(tab) = tabs.get(id).cloned() else { return Err(Error::NotFound) };
            tabs.close(id);
            tab
        };
        self.drop_view(id);
        self.auth.lock().remove(&id);
        self.visits.lock().remove(&id);
        if !navigation::is_internal(&closed.url) || InternalPage::parse(&closed.url) != Some(InternalPage::NewTab) {
            let mut stack = self.closed.lock();
            stack.push(SessionTab { url: closed.url, title: closed.title, group: None });
            if stack.len() > 25 {
                stack.remove(0);
            }
        }
        // Zuno always keeps one tab open.
        if self.tabs.lock().tabs().is_empty() {
            self.tabs.lock().add(InternalPage::NewTab.url(), InternalPage::NewTab.title(), None, true);
        }
        self.emit_tabs();
        self.sync_views()
    }

    pub fn close_other_tabs(self: &Arc<Self>, keep: TabId) -> Result<()> {
        let others: Vec<TabId> = self.tabs.lock().tabs().iter().map(|t| t.id).filter(|t| *t != keep).collect();
        self.activate(keep)?;
        for id in others {
            self.close_tab(id)?;
        }
        Ok(())
    }

    pub fn reopen_closed(self: &Arc<Self>) -> Result<Option<TabId>> {
        let last = self.closed.lock().pop();
        match last {
            Some(tab) => Ok(Some(self.new_tab(Some(&tab.url), None, true)?)),
            None => Ok(None),
        }
    }

    pub fn duplicate(self: &Arc<Self>, id: TabId) -> Result<TabId> {
        let url = self.tabs.lock().get(id).map(|t| t.url.clone()).ok_or(Error::NotFound)?;
        self.new_tab(Some(&url), Some(id), true)
    }

    pub fn move_tab(self: &Arc<Self>, id: TabId, index: usize) -> Result<()> {
        if !self.tabs.lock().move_tab(id, index) {
            return Err(Error::NotFound);
        }
        self.emit_tabs();
        Ok(())
    }

    pub fn select_index(self: &Arc<Self>, index: Option<usize>) -> Result<()> {
        let id = {
            let tabs = self.tabs.lock();
            match index {
                Some(i) => tabs.nth(i),
                None => tabs.last(),
            }
        };
        match id {
            Some(id) => self.activate(id),
            None => Ok(()),
        }
    }

    pub fn cycle(self: &Arc<Self>, forward: bool) -> Result<()> {
        let next = self.tabs.lock().neighbour(forward);
        match next {
            Some(id) => self.activate(id),
            None => Ok(()),
        }
    }

    /// Resolves address-bar input for a tab.
    pub fn navigate_input(self: &Arc<Self>, id: TabId, input: &str) -> Result<NavigateOutcome> {
        let resolved = navigation::resolve_input(input, &self.search_template())?;
        if let Resolved::Web(url) = &resolved {
            if let Some(request) = oauth::analyze(url.as_str()) {
                return Ok(NavigateOutcome::AuthReview { request });
            }
            if security::scheme_policy(url.scheme()) == SchemePolicy::External {
                self.request_external(id, url.clone(), None);
                return Ok(NavigateOutcome::External);
            }
        }
        self.load(id, resolved)?;
        self.sync_views()?;
        Ok(NavigateOutcome::Navigated)
    }

    /// Continues an authorization request the user reviewed and approved.
    pub fn navigate_authorization(self: &Arc<Self>, id: TabId, url: &str) -> Result<()> {
        let request = oauth::analyze(url).ok_or_else(|| Error::msg("This is no longer a valid authorization request"))?;
        let parsed = Url::parse(&request.url)?;
        self.auth.lock().insert(id, request);
        self.load(id, Resolved::Web(parsed))?;
        self.sync_views()
    }

    fn load(self: &Arc<Self>, id: TabId, resolved: Resolved) -> Result<()> {
        match resolved {
            Resolved::Internal(page) => {
                self.drop_view(id);
                self.update_tab(id, |t| {
                    t.url = page.url().to_string();
                    t.title = page.title().to_string();
                    t.favicon = None;
                    t.loading = false;
                    t.error = None;
                    t.can_go_back = false;
                    t.can_go_forward = false;
                    t.live = false;
                });
                self.emit_tabs();
            }
            Resolved::Web(url) | Resolved::Search(url) => {
                if security::scheme_policy(url.scheme()) == SchemePolicy::Block {
                    return Err(Error::msg("Zuno does not open this type of address"));
                }
                self.update_tab(id, |t| {
                    t.url = url.to_string();
                    t.loading = true;
                    t.error = None;
                    if navigation::is_internal(&t.title) || t.title == InternalPage::NewTab.title() {
                        t.title = url.host_str().unwrap_or(url.as_str()).to_string();
                    }
                });
                self.emit_tab(id);
                match self.view(id) {
                    Some(view) => view.navigate(&url)?,
                    None => {
                        let active = self.tabs.lock().active() == Some(id);
                        let overlay = self.layout.lock().overlay;
                        self.create_view(id, &url, active && !overlay)?;
                    }
                }
                self.mark_session_dirty();
            }
        }
        Ok(())
    }

    pub fn go_back(&self, id: TabId) -> Result<()> {
        self.view(id).ok_or(Error::NotFound)?.go_back()
    }

    pub fn go_forward(&self, id: TabId) -> Result<()> {
        self.view(id).ok_or(Error::NotFound)?.go_forward()
    }

    pub fn reload(self: &Arc<Self>, id: TabId) -> Result<()> {
        match self.view(id) {
            Some(view) => view.reload(),
            None => self.sync_views(),
        }
    }

    pub fn stop(&self, id: TabId) -> Result<()> {
        self.view(id).ok_or(Error::NotFound)?.stop()
    }

    pub fn active_tab(&self) -> Option<Tab> {
        let tabs = self.tabs.lock();
        tabs.active().and_then(|id| tabs.get(id)).cloned()
    }

    // -----------------------------------------------------------------------------------------
    // Groups
    // -----------------------------------------------------------------------------------------

    pub fn group_create(self: &Arc<Self>, name: &str, tabs: &[TabId]) -> Result<GroupId> {
        let id = self.tabs.lock().create_group(name, tabs).ok_or(Error::NotFound)?;
        self.emit_tabs();
        Ok(id)
    }

    pub fn group_assign(self: &Arc<Self>, tab: TabId, group: Option<GroupId>) -> Result<()> {
        if !self.tabs.lock().assign(tab, group) {
            return Err(Error::NotFound);
        }
        self.emit_tabs();
        Ok(())
    }

    pub fn group_rename(self: &Arc<Self>, group: GroupId, name: &str) -> Result<()> {
        if !self.tabs.lock().rename_group(group, name) {
            return Err(Error::NotFound);
        }
        self.emit_tabs();
        Ok(())
    }

    pub fn group_set_color(self: &Arc<Self>, group: GroupId, color: &str) -> Result<()> {
        if !self.tabs.lock().set_group_color(group, color) {
            return Err(Error::msg("Unknown group or colour"));
        }
        self.emit_tabs();
        Ok(())
    }

    pub fn group_collapse(self: &Arc<Self>, group: GroupId, collapsed: bool) -> Result<()> {
        if self.tabs.lock().group(group).is_none() {
            return Err(Error::NotFound);
        }
        self.tabs.lock().set_collapsed(group, collapsed);
        self.emit_tabs();
        self.sync_views()
    }

    pub fn group_remove(self: &Arc<Self>, group: GroupId, close_tabs: bool) -> Result<()> {
        let to_close = self.tabs.lock().remove_group(group, close_tabs);
        for id in to_close {
            self.close_tab(id)?;
        }
        self.emit_tabs();
        Ok(())
    }

    // -----------------------------------------------------------------------------------------
    // External applications and authorization
    // -----------------------------------------------------------------------------------------

    fn request_external(self: &Arc<Self>, tab: TabId, url: Url, auth_callback: Option<String>) {
        let mut token_bytes = [0u8; 12];
        let _ = getrandom::fill(&mut token_bytes);
        let token: String = token_bytes.iter().map(|b| format!("{b:02x}")).collect();
        let display = match url.scheme() {
            "mailto" | "tel" | "sms" => url.as_str().to_string(),
            scheme => format!("{scheme}:"),
        };
        self.external.lock().insert(token.clone(), url.clone());
        self.emit(EVT_EXTERNAL, ExternalRequest { tab, token, scheme: url.scheme().to_string(), display, auth_callback });
    }

    /// Opens a previously requested external-application URL after the user confirmed it.
    pub fn confirm_external(&self, token: &str) -> Result<()> {
        let url = self.external.lock().remove(token).ok_or_else(|| Error::msg("This request has expired"))?;
        use tauri_plugin_opener::OpenerExt;
        self.app.opener().open_url(url.as_str(), None::<&str>).map_err(|e| Error::msg(format!("No application could open this link: {e}")))
    }

    pub fn dismiss_external(&self, token: &str) {
        self.external.lock().remove(token);
    }

    fn handle_auth_redirect(self: &Arc<Self>, tab: TabId, url: &Url) {
        let request = {
            let mut auth = self.auth.lock();
            let matches = auth.get(&tab).and_then(|r| r.redirect_uri.as_deref()).is_some_and(|r| oauth::matches_redirect(r, url));
            if matches {
                auth.remove(&tab)
            } else {
                None
            }
        };
        if let Some(request) = request {
            let delivered_to = match request.redirect_kind {
                oauth::RedirectKind::App => format!("the {} application", url.scheme()),
                oauth::RedirectKind::Loopback => "the desktop application that started the sign-in".into(),
                _ => url.host_str().unwrap_or_default().to_string(),
            };
            tracing::info!(tab, provider = %request.provider_host, "authorization redirect detected");
            self.emit(
                EVT_AUTH,
                AuthEvent {
                    tab,
                    client_id: request.client_id,
                    provider: request.provider_name.unwrap_or(request.provider_host),
                    delivered_to,
                    outcome: oauth::callback_outcome(url),
                },
            );
        }
    }

    /// Starts a Zuno-managed sign-in for an OpenID Connect provider.
    pub async fn account_connect(self: &Arc<Self>, issuer: &str, client_id: &str, client_secret: Option<String>, scopes: &str) -> Result<()> {
        let client_id = client_id.trim();
        if client_id.is_empty() {
            return Err(Error::msg("Enter the client ID registered with the provider"));
        }
        let metadata = oauth::discover(&self.http, issuer).await?;
        let id = crate::authentication::account_id(&metadata.issuer, client_id);
        let scopes = if scopes.trim().is_empty() { "openid profile email".to_string() } else { scopes.trim().to_string() };
        let flow = oauth::PendingFlow::new(id, metadata, client_id.to_string(), client_secret.filter(|s| !s.is_empty()), scopes)?;
        let url = flow.authorization_url()?;
        self.accounts.begin(flow);
        let this = self.clone();
        tauri::async_runtime::spawn_blocking(move || this.new_tab(Some(url.as_str()), None, true))
            .await
            .map_err(|e| Error::msg(e.to_string()))??;
        Ok(())
    }

    async fn account_finish(self: Arc<Self>, tab: TabId, flow: oauth::PendingFlow, callback: Url) {
        let result: Result<crate::authentication::Account> = async {
            let code = flow.code_from_callback(&callback)?;
            let tokens = flow.exchange(&self.http, &code).await?;
            let profile = match &flow.metadata.userinfo_endpoint {
                Some(endpoint) => self.http.get(endpoint).bearer_auth(&tokens.access_token).send().await.ok().and_then(|r| r.error_for_status().ok()),
                None => None,
            };
            let profile = match profile {
                Some(r) => r.json::<serde_json::Value>().await.ok(),
                None => None,
            };
            self.accounts.complete(&flow, &tokens, profile)
        }
        .await;
        #[derive(Serialize, Clone)]
        #[serde(rename_all = "camelCase")]
        struct AccountEvent {
            ok: bool,
            message: String,
        }
        let event = match result {
            Ok(account) => {
                tracing::info!(issuer = %account.issuer, "account connected");
                AccountEvent { ok: true, message: format!("Connected {}", account.email.or(account.display_name).unwrap_or(account.issuer)) }
            }
            Err(e) => AccountEvent { ok: false, message: e.to_string() },
        };
        self.emit(EVT_ACCOUNT, event);
        let _ = tauri::async_runtime::spawn_blocking(move || self.load(tab, Resolved::Internal(InternalPage::Settings)).and_then(|_| self.sync_views())).await;
    }

    pub async fn account_disconnect(self: &Arc<Self>, id: &str) -> Result<()> {
        let issuer = self.accounts.list().into_iter().find(|a| a.id == id).map(|a| a.issuer);
        let token = self.accounts.remove(id)?;
        // Best-effort revocation at the provider (RFC 7009).
        if let (Some(issuer), Some(token)) = (issuer, token) {
            if let Ok(meta) = oauth::discover(&self.http, &issuer).await {
                if let Some(endpoint) = meta.revocation_endpoint {
                    let _ = self.http.post(endpoint).form(&[("token", token.as_str())]).send().await;
                }
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------------------------
    // Quick switcher
    // -----------------------------------------------------------------------------------------

    pub fn quick_search(&self, query: &str) -> Result<Vec<QuickResult>> {
        let q = query.trim().to_lowercase();
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for t in self.tabs.lock().tabs() {
            if q.is_empty() || t.title.to_lowercase().contains(&q) || t.url.to_lowercase().contains(&q) {
                seen.insert(t.url.clone());
                out.push(QuickResult::Tab { id: t.id, title: t.title.clone(), url: t.url.clone(), favicon: t.favicon.clone() });
            }
        }
        if q.is_empty() {
            return Ok(out);
        }
        let bookmarks = self.storage.bookmark_search(&q, 8)?;
        let history = if self.is_incognito() { Vec::new() } else { self.storage.history_search_distinct(&q, 12)? };
        let hosts: Vec<String> = bookmarks.iter().filter_map(|b| b.url.as_deref()).chain(history.iter().map(|h| h.url.as_str())).filter_map(host_of).collect();
        let icons = self.storage.favicons_for_hosts(&hosts)?;
        let icon = |url: &str| host_of(url).and_then(|h| icons.get(&h).cloned());
        for b in bookmarks {
            let url = b.url.unwrap_or_default();
            if seen.insert(url.clone()) {
                out.push(QuickResult::Bookmark { id: b.id, favicon: icon(&url), title: b.title, url });
            }
        }
        for h in history {
            if seen.insert(h.url.clone()) {
                out.push(QuickResult::History { id: h.id, favicon: icon(&h.url), title: h.title, url: h.url, visited_at: h.visited_at });
            }
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------------------------
    // Settings and window
    // -----------------------------------------------------------------------------------------

    pub fn apply_settings(&self, settings: &Settings) -> Result<()> {
        if let Some(window) = self.app.get_window(crate::MAIN_WINDOW) {
            let _ = window.set_theme(crate::window_theme(settings.theme));
        }
        self.engine.apply_profile(profile_prefs(settings, self.is_incognito()))?;
        self.emit(EVT_SETTINGS, settings.clone());
        Ok(())
    }

    fn set_fullscreen(self: &Arc<Self>, fullscreen: bool) {
        self.layout.lock().fullscreen = fullscreen;
        if let Some(window) = self.app.get_window(crate::MAIN_WINDOW) {
            let _ = window.set_fullscreen(fullscreen);
        }
        self.emit(EVT_FULLSCREEN, fullscreen);
        let _ = self.relayout();
    }

    pub fn toggle_fullscreen(self: &Arc<Self>) {
        let current = self.app.get_window(crate::MAIN_WINDOW).and_then(|w| w.is_fullscreen().ok()).unwrap_or(false);
        // F11 fullscreen keeps Zuno's interface; page-requested fullscreen (video) hides it.
        if let Some(window) = self.app.get_window(crate::MAIN_WINDOW) {
            let _ = window.set_fullscreen(!current);
        }
        if current {
            self.layout.lock().fullscreen = false;
            self.emit(EVT_FULLSCREEN, false);
        }
        let _ = self.relayout();
    }

    /// Handles a shortcut raised by any webview.
    fn shortcut(self: &Arc<Self>, s: Shortcut) {
        // Shortcuts that type into Zuno's interface need it to have keyboard focus first.
        if matches!(s, Shortcut::FocusAddress | Shortcut::QuickSwitcher | Shortcut::Bookmark) {
            self.focus_ui();
        }
        if s == Shortcut::Fullscreen {
            self.spawn(|this| this.toggle_fullscreen());
            return;
        }
        if s == Shortcut::IncognitoWindow {
            if let Err(e) = crate::incognito::open_window() {
                self.notice("error", format!("Could not open an incognito window: {e}"));
            }
            return;
        }
        self.emit(EVT_SHORTCUT, s);
    }

    pub fn on_exit(&self) {
        self.save_session();
        if self.settings.get().clear_history_on_exit {
            let _ = self.storage.history_clear();
        }
    }
}

pub fn search_template(settings: &Settings) -> String {
    if settings.search_engine == "custom" && !settings.custom_search_url.is_empty() {
        return settings.custom_search_url.clone();
    }
    navigation::find_engine(&settings.search_engine).unwrap_or(&navigation::SEARCH_ENGINES[0]).template.to_string()
}

pub fn profile_prefs(s: &Settings, incognito: bool) -> ProfilePrefs {
    ProfilePrefs {
        theme: s.theme,
        tracking_prevention: s.tracking_prevention,
        // Nothing typed into an incognito window is offered for saving.
        password_autosave: s.password_autosave && !incognito,
        general_autofill: s.general_autofill && !incognito,
    }
}

fn title_for_url(url: &str) -> String {
    if let Some(page) = InternalPage::parse(url) {
        return page.title().to_string();
    }
    Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned)).unwrap_or_else(|| url.to_string())
}

impl EngineSink for Sink {
    fn navigation_starting(&self, tab: TabId, url: &Url) -> NavigationDecision {
        let Some(this) = self.0.upgrade() else { return NavigationDecision::Cancel };
        match security::scheme_policy(url.scheme()) {
            SchemePolicy::Block => {
                tracing::info!(tab, scheme = url.scheme(), "blocked navigation");
                NavigationDecision::Cancel
            }
            SchemePolicy::External => {
                // App callbacks (e.g. com.example.app:/oauth) and mailto:/tel: links are handed to
                // Windows only after the user confirms.
                let callback = this.auth.lock().get(&tab).filter(|r| r.redirect_uri.as_deref().is_some_and(|ru| oauth::matches_redirect(ru, url))).map(|r| r.provider_name.clone().unwrap_or(r.provider_host.clone()));
                this.handle_auth_redirect(tab, url);
                this.request_external(tab, url.clone(), callback);
                NavigationDecision::Cancel
            }
            SchemePolicy::Allow => {
                if let Some(flow) = this.accounts.match_callback(url) {
                    tauri::async_runtime::spawn(this.clone().account_finish(tab, flow, url.clone()));
                    return NavigationDecision::Cancel;
                }
                this.handle_auth_redirect(tab, url);
                NavigationDecision::Allow
            }
        }
    }

    fn event(&self, tab: TabId, event: EngineEvent) {
        let Some(this) = self.0.upgrade() else { return };
        match event {
            EngineEvent::Shortcut(s) => this.shortcut(s),
            EngineEvent::LoadStarted { url } => {
                this.downloading.lock().remove(&tab);
                this.update_tab(tab, |t| {
                    t.loading = true;
                    t.error = None;
                    if url != "about:blank" {
                        t.url = url;
                    }
                });
                this.emit_tab(tab);
            }
            EngineEvent::LoadFinished => {
                this.update_tab(tab, |t| t.loading = false);
                this.emit_tab(tab);
            }
            EngineEvent::UrlChanged { url } => {
                if url.is_empty() || url == "about:blank" {
                    return;
                }
                let changed = {
                    let mut tabs = this.tabs.lock();
                    match tabs.get_mut(tab) {
                        Some(t) if t.url != url => {
                            t.url = url;
                            true
                        }
                        _ => false,
                    }
                };
                if changed {
                    this.emit_tab(tab);
                    this.mark_session_dirty();
                }
            }
            EngineEvent::TitleChanged { title } => {
                let mut url = String::new();
                this.update_tab(tab, |t| {
                    url = t.url.clone();
                    t.title = if title.trim().is_empty() { title_for_url(&t.url) } else { title.clone() };
                });
                this.emit_tab(tab);
                this.mark_session_dirty();
                let visit = this.visits.lock().get(&tab).cloned();
                if let Some((visited_url, id)) = visit {
                    if security::sanitize_for_history(&url).as_deref() == Some(visited_url.as_str()) && !title.trim().is_empty() {
                        this.spawn(move |this| {
                            let _ = this.storage.history_update_title(id, &title);
                        });
                    }
                }
            }
            EngineEvent::FaviconChanged { data_url } => {
                let mut host = None;
                this.update_tab(tab, |t| {
                    t.favicon = data_url.clone();
                    host = host_of(&t.url);
                });
                this.emit_tab(tab);
                if this.is_incognito() {
                    return;
                }
                if let (Some(host), Some(data)) = (host, data_url) {
                    this.spawn(move |this| {
                        let _ = this.storage.favicon_store(&host, &data);
                    });
                }
            }
            EngineEvent::HistoryChanged { can_go_back, can_go_forward } => {
                this.update_tab(tab, |t| {
                    t.can_go_back = can_go_back;
                    t.can_go_forward = can_go_forward;
                });
                this.emit_tab(tab);
            }
            EngineEvent::NavigationCompleted { success, error, url: shown } => {
                let became_download = this.downloading.lock().remove(&tab);
                let mut snapshot = None;
                this.update_tab(tab, |t| {
                    t.loading = false;
                    t.error = if became_download { None } else { error.clone() };
                    if !shown.is_empty() && shown != "about:blank" {
                        t.url = shown.clone();
                    }
                    snapshot = Some((t.url.clone(), t.title.clone()));
                });
                this.emit_tab(tab);
                if success && !this.is_incognito() && this.settings.get().save_history {
                    if let Some((url, title)) = snapshot {
                        if let Some(clean) = security::sanitize_for_history(&url) {
                            this.spawn(move |this| {
                                let title = if title == title_for_url(&url) { String::new() } else { title };
                                if let Ok(id) = this.storage.history_record(&clean, &title, crate::storage::now_ms()) {
                                    this.visits.lock().insert(tab, (clean, id));
                                }
                            });
                        }
                    }
                }
            }
            EngineEvent::OpenInNewTab { url } => this.spawn(move |this| {
                if let Err(e) = this.new_tab(Some(&url), Some(tab), true) {
                    tracing::warn!(error = %e, "failed to open link in new tab");
                }
            }),
            EngineEvent::FullscreenChanged(full) => this.spawn(move |this| this.set_fullscreen(full)),
            EngineEvent::ProcessFailed { reason } => {
                this.update_tab(tab, |t| {
                    t.loading = false;
                    t.error = Some(reason);
                });
                this.emit_tab(tab);
            }
        }
    }

    fn download_starting(&self, tab: TabId, request: DownloadRequest) -> Option<DownloadTicket> {
        let this = self.0.upgrade()?;
        // WebView2 may report the navigation that became a download as aborted, either before or
        // after this event; in both cases the page simply stays as it was.
        this.downloading.lock().insert(tab);
        this.update_tab(tab, |t| {
            t.error = None;
            t.loading = false;
        });
        this.emit_tab(tab);
        let dir = match this.settings.get().download_dir {
            Some(dir) => dir.into(),
            None => this.app.path().download_dir().ok()?,
        };
        match this.downloads.start(&dir, &request) {
            Ok((ticket, item)) => {
                this.emit(EVT_DOWNLOAD, item);
                Some(ticket)
            }
            Err(e) => {
                tracing::warn!(error = %e, "download could not start");
                this.notice("error", format!("The download could not start: {e}"));
                None
            }
        }
    }

    fn download_update(&self, id: i64, update: DownloadUpdate) {
        let Some(this) = self.0.upgrade() else { return };
        if let Some(item) = this.downloads.update(id, update) {
            if this.is_incognito() && item.state == crate::downloads::DownloadState::Completed {
                let path = std::path::PathBuf::from(&item.path);
                this.spawn(move |_| {
                    crate::incognito::scrub_download_origin(&path);
                    // Windows may attach the zone information just after the file is finalised.
                    std::thread::sleep(Duration::from_secs(2));
                    crate::incognito::scrub_download_origin(&path);
                });
            }
            this.emit(EVT_DOWNLOAD, item);
        }
    }
}
