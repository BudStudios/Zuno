//! Zuno — Less noise. More web.
//!
//! A minimal Windows browser: Rust core on Tauri 2, website rendering through Microsoft WebView2
//! (via Wry), interface in React.

mod authentication;
mod browser;
mod commands;
mod downloads;
mod error;
mod importer;
mod incognito;
mod menus;
mod security;
mod storage;

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize, RunEvent, WebviewBuilder, WebviewUrl, WindowEvent};

use browser::Browser;
use storage::settings::{SettingsStore, Theme};

pub const MAIN_WINDOW: &str = "main";

/// Whether Windows is set to dark mode for apps.
pub fn system_prefers_dark() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::core::w;
        use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
        let mut value = 1u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let ok = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        );
        ok.is_ok() && value == 0
    }
    #[cfg(not(windows))]
    false
}

pub fn window_theme(theme: Theme) -> Option<tauri::Theme> {
    match theme {
        Theme::System => None,
        Theme::Light => Some(tauri::Theme::Light),
        Theme::Dark => Some(tauri::Theme::Dark),
    }
}

fn background(theme: Theme) -> tauri::window::Color {
    let dark = match theme {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::System => system_prefers_dark(),
    };
    if dark {
        tauri::window::Color(0x11, 0x12, 0x14, 0xff)
    } else {
        tauri::window::Color(0xf6, 0xf6, 0xf4, 0xff)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct WindowState {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

fn load_window_state(dir: &Path) -> Option<WindowState> {
    serde_json::from_slice(&std::fs::read(dir.join("window.json")).ok()?).ok()
}

fn save_window_state(dir: &Path, window: &tauri::Window) {
    let maximized = window.is_maximized().unwrap_or(false);
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) else { return };
    if window.is_minimized().unwrap_or(false) || size.width < 200 {
        return;
    }
    let previous = load_window_state(dir);
    // Keep the restored (non-maximized) bounds when the window is maximized.
    let state = match (maximized, previous) {
        (true, Some(prev)) => WindowState { maximized: true, ..prev },
        _ => WindowState { x: pos.x, y: pos.y, width: size.width, height: size.height, maximized },
    };
    if let Ok(bytes) = serde_json::to_vec(&state) {
        let _ = storage::write_atomic(&dir.join("window.json"), &bytes);
    }
}

fn init_logging(dir: &Path) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let filter = EnvFilter::try_from_env("ZUNO_LOG").unwrap_or_else(|_| EnvFilter::new("zuno_lib=info,warn"));
    let logs = dir.join("logs");
    let _ = std::fs::create_dir_all(&logs);
    let appender = tracing_appender::rolling::daily(&logs, "zuno.log");
    let (file, guard) = tracing_appender::non_blocking(appender);
    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(file).with_ansi(false).with_target(false));
    #[cfg(debug_assertions)]
    let registry = registry.with(fmt::layer().with_target(false));
    registry.try_init().ok()?;
    Some(guard)
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let incognito_root = incognito::root(&app.path().app_local_data_dir()?);
    // An incognito window writes no log file and keeps its website data in a throwaway folder.
    let session = if incognito::requested() {
        Some(incognito::Session::create(&incognito_root)?)
    } else {
        std::thread::spawn(move || incognito::sweep(&incognito_root));
        if let Some(guard) = init_logging(&data_dir) {
            app.manage(guard);
        }
        None
    };
    let private = session.is_some();
    let private_dir = session.as_ref().map(|s| s.dir.clone());
    tracing::info!(version = env!("CARGO_PKG_VERSION"), webview = %tauri::webview_version().unwrap_or_default(), incognito = private, "starting Zuno");

    let (settings, storage, scratch) = match &private_dir {
        Some(dir) => (SettingsStore::load_read_only(&data_dir), storage::Storage::open_ephemeral(&data_dir, dir)?, dir.clone()),
        None => (SettingsStore::load(&data_dir), storage::Storage::open(&data_dir)?, data_dir.clone()),
    };
    let settings = Arc::new(settings);
    let current = settings.get();
    let storage = Arc::new(storage);
    let downloads = Arc::new(downloads::DownloadManager::new(storage.clone()));
    // Incognito: no connected accounts, and the session file points into the throwaway folder.
    let accounts = Arc::new(authentication::AccountStore::load(&scratch));

    // Main window: frameless, Zuno draws its own title/tab bar.
    let mut builder = tauri::window::WindowBuilder::new(app, MAIN_WINDOW)
        .title(if private { "Zuno — Incognito" } else { "Zuno" })
        .inner_size(1280.0, 820.0)
        .min_inner_size(560.0, 380.0)
        .decorations(false)
        .shadow(true)
        .background_color(background(current.theme))
        .theme(window_theme(current.theme))
        .visible(false);
    let saved = load_window_state(&data_dir);
    if saved.is_none() {
        builder = builder.center();
    }
    let window = builder.build()?;
    if saved.is_none() {
        // First run: fit comfortably inside the monitor's work area (excluding the taskbar).
        if let Ok(Some(monitor)) = window.current_monitor() {
            let area = monitor.work_area().size.to_logical::<f64>(monitor.scale_factor());
            let size = LogicalSize::new((area.width * 0.85).clamp(560.0, 1440.0), (area.height * 0.88).clamp(380.0, 960.0));
            let _ = window.set_size(size);
            let _ = window.center();
        }
    }
    if let Some(s) = saved {
        let _ = window.set_position(PhysicalPosition::new(s.x, s.y));
        let _ = window.set_size(PhysicalSize::new(s.width, s.height));
        if s.maximized {
            let _ = window.maximize();
        }
    }

    // Zuno's interface: a single local webview spanning the whole window. Website tabs are
    // separate child webviews placed on top of its content area.
    let size = window.inner_size()?.to_logical::<f64>(window.scale_factor()?);
    let mut ui = WebviewBuilder::new(security::UI_WEBVIEW, WebviewUrl::App("index.html".into()))
        .auto_resize()
        .background_color(background(current.theme))
        .zoom_hotkeys_enabled(false)
        // Inspecting Zuno's own interface is a development-only affordance.
        .devtools(cfg!(debug_assertions))
        .disable_drag_drop_handler();
    if let Some(dir) = &private_dir {
        // The interface shares the tabs' InPrivate profile, so profile preferences reach them.
        ui = ui.data_directory(dir.clone()).incognito(true);
    }
    window.add_child(ui, LogicalPosition::new(0.0, 0.0), LogicalSize::new(size.width, size.height))?;
    window.show()?;

    let engine = Arc::new(browser::webview::WebView2Engine::new(
        app.handle().clone(),
        MAIN_WINDOW,
        browser::profile_prefs(&current, private),
        private_dir.clone(),
    ));
    let browser = Browser::new(
        app.handle().clone(),
        engine.clone(),
        storage,
        settings,
        downloads,
        accounts,
        storage::session::SessionFile::new(&scratch),
        session,
    )?;
    engine.attach_ui(browser.sink())?;
    app.manage(browser.clone());

    let dir = data_dir.clone();
    let b = browser.clone();
    let w = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
            let b = b.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(e) = b.relayout() {
                    tracing::debug!(error = %e, "relayout failed");
                }
            });
        }
        WindowEvent::CloseRequested { .. } => {
            if !b.is_incognito() {
                save_window_state(&dir, &w);
            }
            b.on_exit();
        }
        _ => {}
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // The opener plugin would otherwise inject a script into every webview (including websites)
        // that hijacks target=_blank and Ctrl+clicks to open the system browser. Zuno handles new
        // windows itself and only uses the plugin from Rust.
        .plugin(tauri_plugin_opener::Builder::new().open_js_links_on_click(false).build())
        .setup(setup)
        .on_menu_event(menus::handle)
        .invoke_handler(tauri::generate_handler![
            commands::ui_ready,
            commands::layout_set_insets,
            commands::layout_set_overlay,
            commands::focus_content,
            commands::app_info,
            commands::tab_new,
            commands::tab_open_page,
            commands::tab_close,
            commands::tab_activate,
            commands::tab_select_index,
            commands::tab_cycle,
            commands::tab_move,
            commands::tab_reopen_closed,
            commands::tab_navigate,
            commands::tab_navigate_authorization,
            commands::tab_action,
            commands::quick_search,
            commands::external_confirm,
            commands::group_create,
            commands::group_assign,
            commands::group_rename,
            commands::group_collapse,
            commands::group_remove,
            commands::menu_show,
            commands::bookmarks_list,
            commands::bookmark_find,
            commands::bookmark_add,
            commands::bookmark_add_folder,
            commands::bookmark_update,
            commands::bookmark_move,
            commands::bookmark_remove,
            commands::bookmarks_export,
            commands::history_search,
            commands::history_delete,
            commands::history_clear,
            commands::top_sites,
            commands::favicons,
            commands::downloads_list,
            commands::download_action,
            commands::downloads_clear,
            commands::downloads_open_folder,
            commands::settings_set,
            commands::choose_download_dir,
            commands::clear_browsing_data,
            commands::import_detect,
            commands::import_inspect,
            commands::import_run,
            commands::auth_analyze,
            commands::accounts_list,
            commands::account_connect,
            commands::account_disconnect,
            commands::incognito_open,
        ])
        .build(tauri::generate_context!());

    match app {
        Ok(app) => app.run(|handle, event| {
            if let RunEvent::Exit = event {
                if let Some(b) = handle.try_state::<Arc<Browser>>() {
                    b.save_session();
                    if let Some(dir) = b.incognito_dir() {
                        incognito::schedule_cleanup(dir);
                    }
                }
            }
        }),
        Err(e) => {
            eprintln!("Zuno failed to start: {e}");
            std::process::exit(1);
        }
    }
}
