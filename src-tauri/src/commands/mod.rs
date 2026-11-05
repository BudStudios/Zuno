//! Tauri commands invoked by Zuno's interface. Every command first verifies that it was called
//! from the privileged `ui` webview; website tabs can never reach these.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, Webview};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::authentication::Account;
use crate::browser::engine::ClearKinds;
use crate::browser::navigation::{self, InternalPage, SearchEngine};
use crate::browser::tabs::{GroupId, Snapshot, TabId, GROUP_COLORS};
use crate::browser::{Browser, Insets, NavigateOutcome, QuickResult, EVT_IMPORT};
use crate::downloads::DownloadItem;
use crate::error::{Error, Result};
use crate::importer;
use crate::security::ensure_ui;
use crate::storage::bookmarks::Bookmark;
use crate::storage::history::HistoryEntry;
use crate::storage::settings::Settings;

type B<'a> = State<'a, Arc<Browser>>;

/// Runs blocking work (anything that touches webviews or the database) off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| Error::msg(format!("Task failed: {e}")))?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialState {
    tabs: Snapshot,
    settings: Settings,
    search_engines: &'static [SearchEngine],
    group_colors: &'static [&'static str],
    incognito: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: &'static str,
    webview_version: String,
    tauri_version: &'static str,
    data_dir: String,
    platform: String,
}

// ---------------------------------------------------------------------------------------------
// Lifecycle and layout
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn ui_ready(webview: Webview, browser: B<'_>, insets: Insets) -> Result<InitialState> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || {
        b.start(insets)?;
        Ok(InitialState {
            tabs: b.snapshot(),
            settings: b.settings.get(),
            search_engines: navigation::SEARCH_ENGINES,
            group_colors: GROUP_COLORS,
            incognito: b.is_incognito(),
        })
    })
    .await
}

#[tauri::command]
pub async fn layout_set_insets(webview: Webview, browser: B<'_>, insets: Insets) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.set_insets(insets)).await
}

#[tauri::command]
pub async fn layout_set_overlay(webview: Webview, browser: B<'_>, overlay: bool) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.set_overlay(overlay)).await
}

#[tauri::command]
pub async fn focus_content(webview: Webview, browser: B<'_>) -> Result<()> {
    ensure_ui(&webview)?;
    browser.focus_content();
    Ok(())
}

#[tauri::command]
pub async fn app_info(webview: Webview, app: AppHandle) -> Result<AppInfo> {
    ensure_ui(&webview)?;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        webview_version: tauri::webview_version().unwrap_or_else(|_| "unknown".into()),
        tauri_version: tauri::VERSION,
        data_dir: app.path().app_data_dir().map(|p| p.display().to_string()).unwrap_or_default(),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    })
}

// ---------------------------------------------------------------------------------------------
// Tabs and navigation
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn tab_new(webview: Webview, browser: B<'_>, input: Option<String>) -> Result<TabId> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.new_tab(input.as_deref(), None, true)).await
}

#[tauri::command]
pub async fn tab_open_page(webview: Webview, browser: B<'_>, page: String) -> Result<TabId> {
    ensure_ui(&webview)?;
    let page = InternalPage::parse(&format!("zuno://{page}")).ok_or(Error::NotFound)?;
    let b = browser.inner().clone();
    blocking(move || b.open_page(page)).await
}

#[tauri::command]
pub async fn tab_close(webview: Webview, browser: B<'_>, id: TabId) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.close_tab(id)).await
}

#[tauri::command]
pub async fn tab_activate(webview: Webview, browser: B<'_>, id: TabId) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.activate(id)).await
}

#[tauri::command]
pub async fn tab_select_index(webview: Webview, browser: B<'_>, index: Option<usize>) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.select_index(index)).await
}

#[tauri::command]
pub async fn tab_cycle(webview: Webview, browser: B<'_>, forward: bool) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.cycle(forward)).await
}

#[tauri::command]
pub async fn tab_move(webview: Webview, browser: B<'_>, id: TabId, index: usize) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.move_tab(id, index)).await
}

#[tauri::command]
pub async fn tab_reopen_closed(webview: Webview, browser: B<'_>) -> Result<Option<TabId>> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.reopen_closed()).await
}

#[tauri::command]
pub async fn tab_navigate(webview: Webview, browser: B<'_>, id: TabId, input: String) -> Result<NavigateOutcome> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.navigate_input(id, &input)).await
}

#[tauri::command]
pub async fn tab_navigate_authorization(webview: Webview, browser: B<'_>, id: TabId, url: String) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.navigate_authorization(id, &url)).await
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum NavAction {
    Back,
    Forward,
    Reload,
    Stop,
}

#[tauri::command]
pub async fn tab_action(webview: Webview, browser: B<'_>, id: TabId, action: NavAction) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || match action {
        NavAction::Back => b.go_back(id),
        NavAction::Forward => b.go_forward(id),
        NavAction::Reload => b.reload(id),
        NavAction::Stop => b.stop(id),
    })
    .await
}

#[tauri::command]
pub async fn quick_search(webview: Webview, browser: B<'_>, query: String) -> Result<Vec<QuickResult>> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.quick_search(&query)).await
}

#[tauri::command]
pub async fn external_confirm(webview: Webview, browser: B<'_>, token: String, allow: bool) -> Result<()> {
    ensure_ui(&webview)?;
    if allow {
        browser.confirm_external(&token)
    } else {
        browser.dismiss_external(&token);
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Groups and menus
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn group_create(webview: Webview, browser: B<'_>, name: String, tabs: Vec<TabId>) -> Result<GroupId> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.group_create(&name, &tabs)).await
}

#[tauri::command]
pub async fn group_assign(webview: Webview, browser: B<'_>, tab: TabId, group: Option<GroupId>) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.group_assign(tab, group)).await
}

#[tauri::command]
pub async fn group_rename(webview: Webview, browser: B<'_>, group: GroupId, name: String) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.group_rename(group, &name)).await
}

#[tauri::command]
pub async fn group_collapse(webview: Webview, browser: B<'_>, group: GroupId, collapsed: bool) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.group_collapse(group, collapsed)).await
}

#[tauri::command]
pub async fn group_remove(webview: Webview, browser: B<'_>, group: GroupId, close_tabs: bool) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.group_remove(group, close_tabs)).await
}

#[tauri::command]
pub async fn menu_show(webview: Webview, app: AppHandle, browser: B<'_>, menu: crate::menus::MenuRequest) -> Result<()> {
    ensure_ui(&webview)?;
    crate::menus::show(&app, browser.inner(), menu)
}

// ---------------------------------------------------------------------------------------------
// Bookmarks, history, favicons
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn bookmarks_list(webview: Webview, browser: B<'_>) -> Result<Vec<Bookmark>> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.storage.bookmarks_all()).await
}

#[tauri::command]
pub async fn bookmark_find(webview: Webview, browser: B<'_>, url: String) -> Result<Option<Bookmark>> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.storage.bookmark_find_by_url(&url)).await
}

#[tauri::command]
pub async fn bookmark_add(webview: Webview, browser: B<'_>, parent: Option<i64>, title: String, url: String) -> Result<Bookmark> {
    ensure_ui(&webview)?;
    browser.ensure_regular("Changing bookmarks")?;
    let b = browser.inner().clone();
    blocking(move || b.storage.bookmark_add(parent, &title, &url)).await
}

#[tauri::command]
pub async fn bookmark_add_folder(webview: Webview, browser: B<'_>, parent: Option<i64>, title: String) -> Result<Bookmark> {
    ensure_ui(&webview)?;
    browser.ensure_regular("Changing bookmarks")?;
    let b = browser.inner().clone();
    blocking(move || b.storage.bookmark_add_folder(parent, &title)).await
}

#[tauri::command]
pub async fn bookmark_update(webview: Webview, browser: B<'_>, id: i64, title: String, url: Option<String>) -> Result<Bookmark> {
    ensure_ui(&webview)?;
    browser.ensure_regular("Changing bookmarks")?;
    let b = browser.inner().clone();
    blocking(move || b.storage.bookmark_update(id, &title, url.as_deref())).await
}

#[tauri::command]
pub async fn bookmark_move(webview: Webview, browser: B<'_>, id: i64, parent: i64, position: Option<i64>) -> Result<()> {
    ensure_ui(&webview)?;
    browser.ensure_regular("Changing bookmarks")?;
    let b = browser.inner().clone();
    blocking(move || b.storage.bookmark_move(id, parent, position)).await
}

#[tauri::command]
pub async fn bookmark_remove(webview: Webview, browser: B<'_>, id: i64) -> Result<()> {
    ensure_ui(&webview)?;
    browser.ensure_regular("Changing bookmarks")?;
    let b = browser.inner().clone();
    blocking(move || b.storage.bookmark_remove(id)).await
}

#[tauri::command]
pub async fn bookmarks_export(webview: Webview, app: AppHandle, browser: B<'_>) -> Result<Option<String>> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || {
        let html = crate::storage::bookmarks::export_html(&b.storage.bookmarks_all()?);
        let picked = app
            .dialog()
            .file()
            .set_title("Export bookmarks")
            .set_file_name("zuno-bookmarks.html")
            .add_filter("HTML document", &["html", "htm"])
            .blocking_save_file();
        let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
        std::fs::write(&path, html)?;
        tracing::info!("bookmarks exported");
        Ok(Some(path.display().to_string()))
    })
    .await
}

#[tauri::command]
pub async fn history_search(webview: Webview, browser: B<'_>, query: String, before: Option<i64>, limit: Option<i64>) -> Result<Vec<HistoryEntry>> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.storage.history_search(&query, before, limit.unwrap_or(100).clamp(1, 500))).await
}

#[tauri::command]
pub async fn history_delete(webview: Webview, browser: B<'_>, id: i64) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.storage.history_delete(id)).await
}

#[tauri::command]
pub async fn history_clear(webview: Webview, browser: B<'_>) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.storage.history_clear()).await
}

#[tauri::command]
pub async fn top_sites(webview: Webview, browser: B<'_>, limit: Option<i64>) -> Result<Vec<HistoryEntry>> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.storage.history_top_sites(limit.unwrap_or(8).clamp(1, 24))).await
}

#[tauri::command]
pub async fn favicons(webview: Webview, browser: B<'_>, hosts: Vec<String>) -> Result<std::collections::HashMap<String, String>> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.storage.favicons_for_hosts(&hosts)).await
}

// ---------------------------------------------------------------------------------------------
// Downloads
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn downloads_list(webview: Webview, browser: B<'_>) -> Result<Vec<DownloadItem>> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.downloads.list()).await
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum DownloadCommand {
    Pause,
    Resume,
    Cancel,
    Open,
    Show,
    Remove,
}

#[tauri::command]
pub async fn download_action(webview: Webview, app: AppHandle, browser: B<'_>, id: i64, action: DownloadCommand) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || {
        use crate::browser::engine::DownloadAction as A;
        match action {
            DownloadCommand::Pause => b.engine().control_download(id, A::Pause),
            DownloadCommand::Resume => b.engine().control_download(id, A::Resume),
            DownloadCommand::Cancel => b.engine().control_download(id, A::Cancel),
            DownloadCommand::Open => {
                let path = b.downloads.completed_path(id)?;
                app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(format!("Could not open the file: {e}")))
            }
            DownloadCommand::Show => {
                let path = b.downloads.completed_path(id)?;
                app.opener().reveal_item_in_dir(&path).map_err(|e| Error::msg(format!("Could not show the file: {e}")))
            }
            DownloadCommand::Remove => b.downloads.remove(id),
        }
    })
    .await
}

#[tauri::command]
pub async fn downloads_clear(webview: Webview, browser: B<'_>) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || b.downloads.clear_finished()).await
}

#[tauri::command]
pub async fn downloads_open_folder(webview: Webview, app: AppHandle, browser: B<'_>) -> Result<()> {
    ensure_ui(&webview)?;
    let dir = match browser.settings.get().download_dir {
        Some(d) => std::path::PathBuf::from(d),
        None => app.path().download_dir()?,
    };
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(format!("Could not open the folder: {e}")))
}

// ---------------------------------------------------------------------------------------------
// Settings and privacy
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn settings_set(webview: Webview, browser: B<'_>, settings: Settings) -> Result<Settings> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || {
        let saved = b.settings.set(settings)?;
        b.apply_settings(&saved)?;
        Ok(saved)
    })
    .await
}

#[tauri::command]
pub async fn choose_download_dir(webview: Webview, app: AppHandle) -> Result<Option<String>> {
    ensure_ui(&webview)?;
    let picked = blocking(move || Ok(app.dialog().file().set_title("Choose a download folder").blocking_pick_folder())).await?;
    Ok(picked.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()))
}

#[derive(Deserialize, Clone, Copy, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClearRequest {
    #[serde(default)]
    history: bool,
    #[serde(default)]
    downloads: bool,
    #[serde(flatten)]
    site: ClearKinds,
}

#[tauri::command]
pub async fn clear_browsing_data(webview: Webview, browser: B<'_>, request: ClearRequest) -> Result<()> {
    ensure_ui(&webview)?;
    let b = browser.inner().clone();
    blocking(move || {
        if request.history {
            b.storage.history_clear()?;
            b.storage.favicons_clear()?;
        }
        if request.downloads {
            b.downloads.clear_all_history()?;
        }
        b.engine().clear_browsing_data(request.site)?;
        tracing::info!("browsing data cleared");
        Ok(())
    })
    .await
}

// ---------------------------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn import_detect(webview: Webview) -> Result<Vec<importer::BrowserSource>> {
    ensure_ui(&webview)?;
    blocking(|| Ok(importer::detect())).await
}

#[tauri::command]
pub async fn import_inspect(webview: Webview, browser: B<'_>, source: String, profile: String) -> Result<importer::Inspection> {
    ensure_ui(&webview)?;
    browser.ensure_regular("Importing browser data")?;
    let b = browser.inner().clone();
    blocking(move || {
        let (_, reader) = importer::open(&source, &profile)?;
        Ok(importer::inspect(reader.as_ref(), &b.settings.get()))
    })
    .await
}

#[tauri::command]
pub async fn import_run(webview: Webview, app: AppHandle, browser: B<'_>, request: importer::ImportRequest) -> Result<Vec<importer::CategoryResult>> {
    ensure_ui(&webview)?;
    browser.ensure_regular("Importing browser data")?;
    let b = browser.inner().clone();
    blocking(move || {
        let emitter = app.clone();
        let results = importer::run(&request, &b.storage, &b.settings, b.engine(), &|p| {
            let _ = emitter.emit_to(tauri::EventTarget::webview(crate::security::UI_WEBVIEW), EVT_IMPORT, p);
        })?;
        // Settings may have changed during the import.
        b.apply_settings(&b.settings.get())?;
        Ok(results)
    })
    .await
}

// ---------------------------------------------------------------------------------------------
// Authentication
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn auth_analyze(webview: Webview, url: String) -> Result<Option<crate::authentication::oauth::AuthorizationRequest>> {
    ensure_ui(&webview)?;
    Ok(crate::authentication::oauth::analyze(&url))
}

#[tauri::command]
pub async fn accounts_list(webview: Webview, browser: B<'_>) -> Result<Vec<Account>> {
    ensure_ui(&webview)?;
    Ok(browser.accounts.list())
}

#[tauri::command]
pub async fn account_connect(
    webview: Webview,
    browser: B<'_>,
    issuer: String,
    client_id: String,
    client_secret: Option<String>,
    scopes: String,
) -> Result<()> {
    ensure_ui(&webview)?;
    browser.ensure_regular("Connecting accounts")?;
    browser.inner().account_connect(&issuer, &client_id, client_secret, &scopes).await
}

#[tauri::command]
pub async fn account_disconnect(webview: Webview, browser: B<'_>, id: String) -> Result<()> {
    ensure_ui(&webview)?;
    browser.ensure_regular("Disconnecting accounts")?;
    browser.inner().account_disconnect(&id).await
}

// ---------------------------------------------------------------------------------------------
// Incognito
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn incognito_open(webview: Webview) -> Result<()> {
    ensure_ui(&webview)?;
    crate::incognito::open_window()
}
