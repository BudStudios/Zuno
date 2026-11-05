//! WebView2 implementation of the rendering engine.
//!
//! Every tab is an independent WebView2 controller created as a child webview of the main
//! window through Tauri's multi-webview API (built on Wry). All tabs share one WebView2 profile,
//! so cookies and site storage persist across tabs and restarts like in any desktop browser,
//! while navigation history, processes and page state are independent per tab. In an incognito
//! process every webview is InPrivate and uses a throwaway user-data folder instead.
//!
//! Tauri/Wry expose navigation, page-load, title and new-window hooks. Everything else a browser
//! needs (back/forward state, favicons, stop, accelerator keys, download progress, profile
//! settings) is wired directly against the WebView2 COM API through `webview2-com`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use base64::Engine as _;
use parking_lot::Mutex;
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Position, Rect, Size, Webview, WebviewBuilder, WebviewUrl,
    WebviewWindowBuilder, Window, Wry,
};
use url::Url;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::*;
use windows::core::{Interface, PWSTR};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};

use super::engine::*;
use super::shortcuts::{map_key, Modifiers};
use super::tabs::TabId;
use crate::error::{Error, Result};
use crate::security::{self, SchemePolicy, UI_WEBVIEW};
use crate::storage::settings::{Theme, TrackingPrevention};

/// Pseudo tab id used for events raised by Zuno's own interface webview.
pub const UI_TAB: TabId = 0;

thread_local! {
    /// Live download operations. COM objects are apartment-bound, so they are only ever touched
    /// on the UI thread that created them.
    static DOWNLOADS: RefCell<HashMap<i64, ICoreWebView2DownloadOperation>> = RefCell::new(HashMap::new());
}

static POPUP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// How long a tab must stay hidden before it is suspended (scripts and timers frozen). Long enough
/// that quick tab switching and background chat/notification pages keep working.
const SUSPEND_AFTER: Duration = Duration::from_secs(5 * 60);

pub struct WebView2Engine {
    app: AppHandle,
    window_label: String,
    prefs: Mutex<ProfilePrefs>,
    /// Incognito: the throwaway WebView2 user-data folder; every webview is InPrivate.
    incognito: Option<std::path::PathBuf>,
}

impl WebView2Engine {
    pub fn new(app: AppHandle, window_label: &str, prefs: ProfilePrefs, incognito: Option<std::path::PathBuf>) -> Self {
        Self { app, window_label: window_label.to_string(), prefs: Mutex::new(prefs), incognito }
    }

    fn window(&self) -> Result<Window<Wry>> {
        self.app.get_window(&self.window_label).ok_or_else(|| Error::msg("Main window is not available"))
    }

    fn ui(&self) -> Result<Webview<Wry>> {
        self.app.get_webview(UI_WEBVIEW).ok_or_else(|| Error::msg("Interface webview is not available"))
    }

    /// Hooks keyboard shortcuts on Zuno's interface webview and applies profile preferences.
    pub fn attach_ui(&self, sink: Arc<dyn EngineSink>) -> Result<()> {
        let prefs = *self.prefs.lock();
        self.ui()?.with_webview(move |pw| {
            let controller = pw.controller();
            unsafe {
                if let Err(e) = attach_accelerators(&controller, UI_TAB, sink) {
                    tracing::warn!(error = %e, "failed to attach UI accelerators");
                }
                if let Ok(core) = controller.CoreWebView2() {
                    if let Err(e) = apply_profile_prefs(&core, prefs) {
                        tracing::warn!(error = %e, "failed to apply profile preferences");
                    }
                }
            }
        })?;
        Ok(())
    }

    /// Runs `f` with the shared ICoreWebView2 of the interface webview on the UI thread and
    /// waits for its result.
    fn with_ui_core<T: Send + 'static>(
        &self,
        f: impl FnOnce(&ICoreWebView2) -> windows::core::Result<T> + Send + 'static,
    ) -> Result<T> {
        let (tx, rx) = mpsc::channel();
        self.ui()?.with_webview(move |pw| {
            let result = unsafe { pw.controller().CoreWebView2() }.and_then(|core| f(&core));
            let _ = tx.send(result);
        })?;
        let result = rx.recv_timeout(Duration::from_secs(30)).map_err(|_| Error::msg("The browser engine did not respond"))?;
        Ok(result?)
    }
}

fn theme_background(theme: Theme) -> tauri::window::Color {
    let dark = match theme {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::System => crate::system_prefers_dark(),
    };
    if dark {
        tauri::window::Color(0x11, 0x12, 0x14, 0xff)
    } else {
        tauri::window::Color(0xff, 0xff, 0xff, 0xff)
    }
}

impl RenderingEngine for WebView2Engine {
    fn create_view(
        &self,
        tab: TabId,
        url: &Url,
        bounds: Bounds,
        visible: bool,
        sink: Arc<dyn EngineSink>,
    ) -> Result<Arc<dyn ContentView>> {
        let label = format!("tab-{tab}");
        let prefs = *self.prefs.lock();
        let (s_nav, s_load, s_title, s_new) = (sink.clone(), sink.clone(), sink.clone(), sink.clone());
        let app = self.app.clone();
        let incognito = self.incognito.clone();

        let mut builder = WebviewBuilder::new(&label, WebviewUrl::External(url.clone()))
            .on_navigation(move |u| s_nav.navigation_starting(tab, u) == NavigationDecision::Allow)
            .on_page_load(move |_, payload| {
                let url = payload.url().to_string();
                let event = match payload.event() {
                    PageLoadEvent::Started => EngineEvent::LoadStarted { url },
                    PageLoadEvent::Finished => EngineEvent::LoadFinished,
                };
                s_load.event(tab, event);
            })
            .on_document_title_changed(move |_, title| s_title.event(tab, EngineEvent::TitleChanged { title }))
            .on_new_window(move |target, features| {
                tracing::debug!(tab, sized = features.size().is_some(), "new window requested");
                if security::scheme_policy(target.scheme()) != SchemePolicy::Allow {
                    return NewWindowResponse::Deny;
                }
                // Scripted popups that request a size (typically "Sign in with ..." windows) keep
                // their `window.opener` relationship so the sign-in can report back to the page.
                if features.size().is_some() {
                    match open_popup(&app, &target, features, incognito.clone()) {
                        Ok(window) => return NewWindowResponse::Create { window },
                        Err(e) => tracing::warn!(error = %e, "popup creation failed, opening as tab"),
                    }
                }
                s_new.event(tab, EngineEvent::OpenInNewTab { url: target.to_string() });
                NewWindowResponse::Deny
            })
            .background_color(theme_background(prefs.theme))
            .zoom_hotkeys_enabled(true)
            .devtools(true)
            .focused(visible);
        if let Some(dir) = &self.incognito {
            builder = builder.data_directory(dir.clone()).incognito(true);
        }

        let webview = self.window()?.add_child(
            builder,
            LogicalPosition::new(bounds.x, bounds.y),
            LogicalSize::new(bounds.width.max(1.0), bounds.height.max(1.0)),
        )?;
        webview.with_webview(move |pw| {
            if let Err(e) = unsafe { attach_content_handlers(&pw.controller(), tab, sink, prefs) } {
                tracing::error!(error = %e, tab, "failed to attach WebView2 handlers");
            }
        })?;
        let view = TabView { tab, webview, visibility: Arc::new(AtomicU64::new(0)) };
        if !visible {
            view.set_visible(false)?;
        }
        Ok(Arc::new(view))
    }

    fn control_download(&self, id: i64, action: DownloadAction) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        self.app.run_on_main_thread(move || {
            let result = DOWNLOADS.with(|d| {
                let map = d.borrow();
                let Some(op) = map.get(&id) else { return Err(Error::msg("This download is no longer active")) };
                unsafe {
                    match action {
                        DownloadAction::Pause => op.Pause()?,
                        DownloadAction::Resume => op.Resume()?,
                        DownloadAction::Cancel => op.Cancel()?,
                    }
                }
                Ok(())
            });
            let _ = tx.send(result);
        })?;
        rx.recv_timeout(Duration::from_secs(10)).map_err(|_| Error::msg("The browser engine did not respond"))?
    }

    fn apply_profile(&self, prefs: ProfilePrefs) -> Result<()> {
        *self.prefs.lock() = prefs;
        // The profile is shared, so applying it through the UI webview covers every tab;
        // per-webview settings (autofill) are applied to each live webview.
        for webview in self.app.webviews().into_values() {
            let is_ui = webview.label() == UI_WEBVIEW;
            let _ = webview.set_background_color(Some(theme_background(prefs.theme)));
            webview.with_webview(move |pw| unsafe {
                if let Ok(core) = pw.controller().CoreWebView2() {
                    let result = if is_ui { apply_profile_prefs(&core, prefs) } else { apply_webview_settings(&core, prefs) };
                    if let Err(e) = result {
                        tracing::warn!(error = %e, "failed to apply preferences");
                    }
                }
            })?;
        }
        Ok(())
    }

    fn clear_browsing_data(&self, kinds: ClearKinds) -> Result<()> {
        let mut flags = 0i32;
        if kinds.cookies {
            flags |= COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES.0;
        }
        if kinds.cache {
            flags |= COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE.0 | COREWEBVIEW2_BROWSING_DATA_KINDS_CACHE_STORAGE.0;
        }
        if kinds.site_storage {
            flags |= COREWEBVIEW2_BROWSING_DATA_KINDS_ALL_DOM_STORAGE.0;
        }
        if kinds.passwords {
            flags |= COREWEBVIEW2_BROWSING_DATA_KINDS_PASSWORD_AUTOSAVE.0;
        }
        if kinds.autofill {
            flags |= COREWEBVIEW2_BROWSING_DATA_KINDS_GENERAL_AUTOFILL.0;
        }
        if flags == 0 {
            return Ok(());
        }
        let (tx, rx) = mpsc::channel::<windows::core::Result<()>>();
        self.with_ui_core(move |core| unsafe {
            let profile: ICoreWebView2Profile2 = core.cast::<ICoreWebView2_13>()?.Profile()?.cast()?;
            profile.ClearBrowsingData(
                COREWEBVIEW2_BROWSING_DATA_KINDS(flags),
                &ClearBrowsingDataCompletedHandler::create(Box::new(move |result| {
                    let _ = tx.send(result);
                    Ok(())
                })),
            )
        })?;
        rx.recv_timeout(Duration::from_secs(120)).map_err(|_| Error::msg("Clearing data timed out"))??;
        Ok(())
    }

    fn import_cookies(&self, cookies: Vec<ImportedCookie>) -> Result<usize> {
        self.with_ui_core(move |core| unsafe {
            let manager = core.cast::<ICoreWebView2_2>()?.CookieManager()?;
            let mut added = 0usize;
            for c in &cookies {
                let name = CoTaskMemPWSTR::from(c.name.as_str());
                let value = CoTaskMemPWSTR::from(c.value.as_str());
                let domain = CoTaskMemPWSTR::from(c.domain.as_str());
                let path = CoTaskMemPWSTR::from(c.path.as_str());
                let cookie = match manager.CreateCookie(
                    *name.as_ref().as_pcwstr(),
                    *value.as_ref().as_pcwstr(),
                    *domain.as_ref().as_pcwstr(),
                    *path.as_ref().as_pcwstr(),
                ) {
                    Ok(cookie) => cookie,
                    Err(_) => continue,
                };
                if let Some(expires) = c.expires {
                    let _ = cookie.SetExpires(expires);
                }
                let _ = cookie.SetIsSecure(c.secure);
                let _ = cookie.SetIsHttpOnly(c.http_only);
                if let Some(same_site) = c.same_site {
                    let kind = match same_site {
                        SameSite::None => COREWEBVIEW2_COOKIE_SAME_SITE_KIND_NONE,
                        SameSite::Lax => COREWEBVIEW2_COOKIE_SAME_SITE_KIND_LAX,
                        SameSite::Strict => COREWEBVIEW2_COOKIE_SAME_SITE_KIND_STRICT,
                    };
                    let _ = cookie.SetSameSite(kind);
                }
                if manager.AddOrUpdateCookie(&cookie).is_ok() {
                    added += 1;
                }
            }
            Ok(added)
        })
    }
}

/// Opens a scripted popup in a small Zuno window that shares the tab's browsing profile.
fn open_popup(
    app: &AppHandle,
    target: &Url,
    features: tauri::webview::NewWindowFeatures,
    incognito: Option<std::path::PathBuf>,
) -> Result<tauri::WebviewWindow> {
    let label = format!("popup-{}", POPUP_COUNTER.fetch_add(1, Ordering::Relaxed));
    let host = target.host_str().unwrap_or("Sign in").to_string();
    let title_host = host.clone();
    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::External("about:blank".parse()?))
        .window_features(features)
        .title(format!("{host} — Zuno"))
        .on_navigation(|u| security::scheme_policy(u.scheme()) == SchemePolicy::Allow)
        .on_document_title_changed(move |window, title| {
            // Always show the origin so the user can verify where they are signing in.
            let host = window.url().ok().and_then(|u| u.host_str().map(str::to_owned)).unwrap_or_else(|| title_host.clone());
            let _ = window.set_title(&format!("{host} — {title}"));
        });
    if let Some(dir) = incognito {
        // The popup must use the opener's (InPrivate) profile.
        builder = builder.data_directory(dir).incognito(true);
    }
    if let Some(main) = app.get_window(crate::MAIN_WINDOW) {
        // Owned (not parented) so the popup stays above Zuno but remains a normal top-level window.
        builder = builder.owner_raw(main.hwnd()?);
    }
    Ok(builder.build()?)
}

struct TabView {
    tab: TabId,
    webview: Webview<Wry>,
    /// Bumped on every visibility change; a pending suspend only runs if it is still current.
    visibility: Arc<AtomicU64>,
}

impl TabView {
    fn with_core(&self, f: impl FnOnce(&ICoreWebView2) -> windows::core::Result<()> + Send + 'static) -> Result<()> {
        self.webview.with_webview(move |pw| unsafe {
            if let Err(e) = pw.controller().CoreWebView2().and_then(|core| f(&core)) {
                tracing::warn!(error = %e, "WebView2 call failed");
            }
        })?;
        Ok(())
    }
}

impl ContentView for TabView {
    fn navigate(&self, url: &Url) -> Result<()> {
        Ok(self.webview.navigate(url.clone())?)
    }
    fn go_back(&self) -> Result<()> {
        self.with_core(|c| unsafe { c.GoBack() })
    }
    fn go_forward(&self) -> Result<()> {
        self.with_core(|c| unsafe { c.GoForward() })
    }
    fn reload(&self) -> Result<()> {
        self.with_core(|c| unsafe { c.Reload() })
    }
    fn stop(&self) -> Result<()> {
        self.with_core(|c| unsafe { c.Stop() })
    }
    fn set_bounds(&self, b: Bounds) -> Result<()> {
        Ok(self.webview.set_bounds(Rect {
            position: Position::Logical(LogicalPosition::new(b.x, b.y)),
            size: Size::Logical(LogicalSize::new(b.width.max(1.0), b.height.max(1.0))),
        })?)
    }
    fn set_visible(&self, visible: bool) -> Result<()> {
        let generation = self.visibility.fetch_add(1, Ordering::Relaxed) + 1;
        // Showing a suspended webview resumes it automatically.
        if visible {
            self.webview.show()?;
        } else {
            self.webview.hide()?;
        }
        // Hidden tabs ask WebView2 to trim caches and page memory right away.
        self.with_core(move |c| unsafe {
            if let Ok(c19) = c.cast::<ICoreWebView2_19>() {
                c19.SetMemoryUsageTargetLevel(if visible {
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                } else {
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                })?;
            }
            Ok(())
        })?;
        if !visible {
            let (tab, webview, visibility) = (self.tab, self.webview.clone(), self.visibility.clone());
            std::thread::spawn(move || {
                std::thread::sleep(SUSPEND_AFTER);
                if visibility.load(Ordering::Relaxed) != generation {
                    return;
                }
                // The tab may have been closed meanwhile; then there is nothing to suspend.
                let _ = webview.with_webview(move |pw| {
                    // Re-checked on the UI thread, where visibility changes are applied.
                    if visibility.load(Ordering::Relaxed) != generation {
                        return;
                    }
                    if let Err(e) = unsafe { try_suspend(&pw.controller(), tab) } {
                        tracing::debug!(error = %e, tab, "tab suspend failed");
                    }
                });
            });
        }
        Ok(())
    }
    fn focus(&self) -> Result<()> {
        Ok(self.webview.set_focus()?)
    }
    fn close(&self) -> Result<()> {
        Ok(self.webview.close()?)
    }
}

/// Freezes a hidden tab's scripts and timers. WebView2 declines (harmlessly) while the page plays
/// audio, and resumes the tab when it is shown or navigated.
unsafe fn try_suspend(controller: &ICoreWebView2Controller, tab: TabId) -> windows::core::Result<()> {
    let mut visible = windows::core::BOOL::default();
    controller.IsVisible(&mut visible)?;
    if visible.as_bool() {
        return Ok(());
    }
    let core: ICoreWebView2_3 = controller.CoreWebView2()?.cast()?;
    core.TrySuspend(&TrySuspendCompletedHandler::create(Box::new(move |result, suspended| {
        tracing::debug!(tab, ok = result.is_ok(), suspended, "tab suspend");
        Ok(())
    })))
}

unsafe fn read_string(f: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>) -> String {
    let mut value = PWSTR::null();
    match f(&mut value) {
        Ok(()) => take_pwstr(value),
        Err(_) => String::new(),
    }
}

unsafe fn modifiers() -> Modifiers {
    Modifiers {
        ctrl: GetKeyState(VK_CONTROL.0 as i32) < 0,
        shift: GetKeyState(VK_SHIFT.0 as i32) < 0,
        alt: GetKeyState(VK_MENU.0 as i32) < 0,
    }
}

unsafe fn attach_accelerators(
    controller: &ICoreWebView2Controller,
    tab: TabId,
    sink: Arc<dyn EngineSink>,
) -> windows::core::Result<()> {
    let mut token = 0i64;
    controller.add_AcceleratorKeyPressed(
        &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
            args.KeyEventKind(&mut kind)?;
            if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN {
                return Ok(());
            }
            let mut vk = 0u32;
            args.VirtualKey(&mut vk)?;
            if let Some(shortcut) = map_key(vk, modifiers()) {
                args.SetHandled(true)?;
                let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
                let repeated = args.PhysicalKeyStatus(&mut status).is_ok() && status.WasKeyDown.as_bool();
                if !repeated || shortcut.repeats() {
                    sink.event(tab, EngineEvent::Shortcut(shortcut));
                }
            }
            Ok(())
        })),
        &mut token,
    )
}

unsafe fn apply_webview_settings(core: &ICoreWebView2, prefs: ProfilePrefs) -> windows::core::Result<()> {
    let settings = core.Settings()?;
    settings.SetIsStatusBarEnabled(true)?;
    settings.SetAreDevToolsEnabled(true)?;
    settings.SetIsZoomControlEnabled(true)?;
    if let Ok(s4) = settings.cast::<ICoreWebView2Settings4>() {
        s4.SetIsPasswordAutosaveEnabled(prefs.password_autosave)?;
        s4.SetIsGeneralAutofillEnabled(prefs.general_autofill)?;
    }
    Ok(())
}

unsafe fn apply_profile_prefs(core: &ICoreWebView2, prefs: ProfilePrefs) -> windows::core::Result<()> {
    let profile = core.cast::<ICoreWebView2_13>()?.Profile()?;
    let scheme = match prefs.theme {
        Theme::System => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_AUTO,
        Theme::Light => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT,
        Theme::Dark => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK,
    };
    profile.SetPreferredColorScheme(scheme)?;
    if let Ok(p3) = profile.cast::<ICoreWebView2Profile3>() {
        let level = match prefs.tracking_prevention {
            TrackingPrevention::None => COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_NONE,
            TrackingPrevention::Basic => COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BASIC,
            TrackingPrevention::Balanced => COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BALANCED,
            TrackingPrevention::Strict => COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_STRICT,
        };
        p3.SetPreferredTrackingPreventionLevel(level)?;
    }
    Ok(())
}

fn web_error_message(status: COREWEBVIEW2_WEB_ERROR_STATUS) -> &'static str {
    match status {
        COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED => "The site's address could not be found",
        COREWEBVIEW2_WEB_ERROR_STATUS_CANNOT_CONNECT | COREWEBVIEW2_WEB_ERROR_STATUS_SERVER_UNREACHABLE => {
            "The site could not be reached"
        }
        COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT => "The connection timed out",
        COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED => "You appear to be offline",
        COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_RESET => "The connection was reset",
        COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_COMMON_NAME_IS_INCORRECT
        | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_EXPIRED
        | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_IS_INVALID
        | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_REVOKED
        | COREWEBVIEW2_WEB_ERROR_STATUS_CLIENT_CERTIFICATE_CONTAINS_ERRORS => {
            "The site's security certificate is not valid. The connection was blocked."
        }
        COREWEBVIEW2_WEB_ERROR_STATUS_REDIRECT_FAILED => "The page redirected too many times",
        COREWEBVIEW2_WEB_ERROR_STATUS_ERROR_HTTP_INVALID_SERVER_RESPONSE => "The server sent an invalid response",
        COREWEBVIEW2_WEB_ERROR_STATUS_VALID_AUTHENTICATION_CREDENTIALS_REQUIRED
        | COREWEBVIEW2_WEB_ERROR_STATUS_VALID_PROXY_AUTHENTICATION_REQUIRED => "Authentication is required",
        _ => "The page could not be loaded",
    }
}

fn interrupt_message(reason: COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON) -> &'static str {
    match reason {
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED => "Cancelled",
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED => "Paused",
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_NO_SPACE => "Not enough disk space",
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_ACCESS_DENIED => "Access to the download folder was denied",
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_NAME_TOO_LONG => "The file name is too long",
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_TOO_LARGE => "The file is too large for this drive",
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_MALICIOUS => "Blocked: this file was flagged as dangerous",
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_BLOCKED_BY_POLICY
        | COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_SECURITY_CHECK_FAILED => "Blocked by a security check",
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_DISCONNECTED
        | COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_FAILED
        | COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_TIMEOUT
        | COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_SERVER_DOWN => "Network error",
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_UNAUTHORIZED | COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_FORBIDDEN => {
            "The server refused the download"
        }
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_CERTIFICATE_PROBLEM => "The server's certificate is not valid",
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_SHUTDOWN => "Interrupted when Zuno closed",
        _ => "The download failed",
    }
}

unsafe fn attach_content_handlers(
    controller: &ICoreWebView2Controller,
    tab: TabId,
    sink: Arc<dyn EngineSink>,
    prefs: ProfilePrefs,
) -> windows::core::Result<()> {
    let core = controller.CoreWebView2()?;
    apply_webview_settings(&core, prefs)?;
    attach_accelerators(controller, tab, sink.clone())?;
    let mut token = 0i64;

    let s = sink.clone();
    core.add_HistoryChanged(
        &HistoryChangedEventHandler::create(Box::new(move |sender, _| {
            let Some(core) = sender else { return Ok(()) };
            let (mut back, mut forward) = (Default::default(), Default::default());
            core.CanGoBack(&mut back)?;
            core.CanGoForward(&mut forward)?;
            s.event(tab, EngineEvent::HistoryChanged { can_go_back: back.as_bool(), can_go_forward: forward.as_bool() });
            Ok(())
        })),
        &mut token,
    )?;

    // SourceChanged also covers same-document navigations (pushState, fragments) that do not
    // raise page-load events.
    let s = sink.clone();
    core.add_SourceChanged(
        &SourceChangedEventHandler::create(Box::new(move |sender, _| {
            let Some(core) = sender else { return Ok(()) };
            let url = read_string(|p| core.Source(p));
            s.event(tab, EngineEvent::UrlChanged { url });
            Ok(())
        })),
        &mut token,
    )?;

    let s = sink.clone();
    core.add_NavigationCompleted(
        &NavigationCompletedEventHandler::create(Box::new(move |sender, args| {
            let Some(args) = args else { return Ok(()) };
            let mut ok = Default::default();
            args.IsSuccess(&mut ok)?;
            // WebView2 reports HTTP 4xx/5xx responses as unsuccessful, but the server's page is
            // shown and is a real visit, like in any browser.
            let mut http_status = 0;
            if let Ok(args2) = args.cast::<ICoreWebView2NavigationCompletedEventArgs2>() {
                let _ = args2.HttpStatusCode(&mut http_status);
            }
            let ok: windows::core::BOOL = (ok.as_bool() || http_status >= 400).into();
            let error = if ok.as_bool() {
                None
            } else {
                let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                args.WebErrorStatus(&mut status)?;
                tracing::debug!(tab, status = status.0, "navigation failed");
                // A cancelled navigation (stop button, superseded navigation) is not an error.
                (status != COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED).then(|| web_error_message(status).to_string())
            };
            let url = sender.map(|core| read_string(|p| core.Source(p))).unwrap_or_default();
            s.event(tab, EngineEvent::NavigationCompleted { success: ok.as_bool(), error, url });
            Ok(())
        })),
        &mut token,
    )?;

    if let Ok(core15) = core.cast::<ICoreWebView2_15>() {
        let s = sink.clone();
        core15.add_FaviconChanged(
            &FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                let Some(core) = sender else { return Ok(()) };
                let core15: ICoreWebView2_15 = core.cast()?;
                if read_string(|p| core15.FaviconUri(p)).is_empty() {
                    s.event(tab, EngineEvent::FaviconChanged { data_url: None });
                    return Ok(());
                }
                let s = s.clone();
                core15.GetFavicon(
                    COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG,
                    &GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
                        let data_url = result.ok().and(stream).and_then(|stream| read_stream(&stream)).map(|bytes| {
                            format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
                        });
                        s.event(tab, EngineEvent::FaviconChanged { data_url });
                        Ok(())
                    })),
                )
            })),
            &mut token,
        )?;
    }

    let s = sink.clone();
    core.add_ContainsFullScreenElementChanged(
        &ContainsFullScreenElementChangedEventHandler::create(Box::new(move |sender, _| {
            let Some(core) = sender else { return Ok(()) };
            let mut full = Default::default();
            core.ContainsFullScreenElement(&mut full)?;
            s.event(tab, EngineEvent::FullscreenChanged(full.as_bool()));
            Ok(())
        })),
        &mut token,
    )?;

    let s = sink.clone();
    core.add_ProcessFailed(
        &ProcessFailedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
            args.ProcessFailedKind(&mut kind)?;
            if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
                || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE
                || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED
            {
                s.event(tab, EngineEvent::ProcessFailed { reason: "This page stopped responding".into() });
            }
            Ok(())
        })),
        &mut token,
    )?;

    // Links, redirects and frames that target another application's URI scheme (mailto:, OAuth
    // app callbacks, ...) arrive here rather than as navigations. Zuno always cancels WebView2's
    // default handling and asks the user itself.
    if let Ok(core18) = core.cast::<ICoreWebView2_18>() {
        let s = sink.clone();
        core18.add_LaunchingExternalUriScheme(
            &LaunchingExternalUriSchemeEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                args.SetCancel(true)?;
                if let Ok(url) = url::Url::parse(&read_string(|p| args.Uri(p))) {
                    s.navigation_starting(tab, &url);
                }
                Ok(())
            })),
            &mut token,
        )?;
    }

    attach_downloads(&core, tab, sink)?;
    Ok(())
}

unsafe fn read_stream(stream: &windows::Win32::System::Com::IStream) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let mut read = 0u32;
        let hr = stream.Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut read));
        if hr.is_err() {
            return None;
        }
        if read == 0 {
            break;
        }
        out.extend_from_slice(&buf[..read as usize]);
        if out.len() > 256 * 1024 {
            return None;
        }
    }
    (!out.is_empty()).then_some(out)
}

unsafe fn attach_downloads(core: &ICoreWebView2, tab: TabId, sink: Arc<dyn EngineSink>) -> windows::core::Result<()> {
    let core4: ICoreWebView2_4 = core.cast()?;
    let mut token = 0i64;
    core4.add_DownloadStarting(
        &DownloadStartingEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let op = args.DownloadOperation()?;
            let url = read_string(|p| op.Uri(p));
            let mime = read_string(|p| op.MimeType(p));
            let suggested = read_string(|p| args.ResultFilePath(p));
            let mut total = 0i64;
            let _ = op.TotalBytesToReceive(&mut total);

            let request = DownloadRequest {
                url,
                suggested_path: suggested.into(),
                mime_type: (!mime.is_empty()).then_some(mime),
                total_bytes: (total > 0).then_some(total),
            };
            let Some(ticket) = sink.download_starting(tab, request) else {
                args.SetCancel(true)?;
                return Ok(());
            };
            let path = CoTaskMemPWSTR::from(ticket.path.to_string_lossy().as_ref());
            args.SetResultFilePath(*path.as_ref().as_pcwstr())?;
            // Zuno shows its own download UI instead of WebView2's flyout.
            args.SetHandled(true)?;

            let id = ticket.id;
            DOWNLOADS.with(|d| d.borrow_mut().insert(id, op.clone()));
            let mut op_token = 0i64;

            let s = sink.clone();
            let mut last_emit = Instant::now() - Duration::from_secs(1);
            op.add_BytesReceivedChanged(
                &BytesReceivedChangedEventHandler::create(Box::new(move |sender, _| {
                    let Some(op) = sender else { return Ok(()) };
                    if last_emit.elapsed() < Duration::from_millis(200) {
                        return Ok(());
                    }
                    last_emit = Instant::now();
                    let (mut received, mut total) = (0i64, 0i64);
                    op.BytesReceived(&mut received)?;
                    let _ = op.TotalBytesToReceive(&mut total);
                    s.download_update(id, DownloadUpdate::Progress { received, total: (total > 0).then_some(total) });
                    Ok(())
                })),
                &mut op_token,
            )?;

            let s = sink.clone();
            op.add_StateChanged(
                &StateChangedEventHandler::create(Box::new(move |sender, _| {
                    let Some(op) = sender else { return Ok(()) };
                    let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
                    op.State(&mut state)?;
                    let mut received = 0i64;
                    let _ = op.BytesReceived(&mut received);
                    let update = match state {
                        COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED => {
                            DOWNLOADS.with(|d| d.borrow_mut().remove(&id));
                            DownloadUpdate::Completed { received }
                        }
                        COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED => {
                            let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON::default();
                            let _ = op.InterruptReason(&mut reason);
                            if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED {
                                DownloadUpdate::Paused
                            } else {
                                let mut can_resume = Default::default();
                                let _ = op.CanResume(&mut can_resume);
                                let cancelled = reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED;
                                if cancelled || !can_resume.as_bool() {
                                    DOWNLOADS.with(|d| d.borrow_mut().remove(&id));
                                }
                                DownloadUpdate::Interrupted {
                                    reason: interrupt_message(reason).to_string(),
                                    can_resume: can_resume.as_bool() && !cancelled,
                                    cancelled,
                                }
                            }
                        }
                        _ => DownloadUpdate::Resumed,
                    };
                    s.download_update(id, update);
                    Ok(())
                })),
                &mut op_token,
            )?;
            Ok(())
        })),
        &mut token,
    )
}
