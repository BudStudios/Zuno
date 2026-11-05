//! Browser data import.
//!
//! Supported sources: Chromium-based browsers (Chrome, Edge, Brave, Vivaldi, Opera, Chromium) and
//! Firefox. Data is read from copies of the source profile's files (so a running browser's locks
//! are respected and the original files are never modified) using only the current Windows
//! user's normal file permissions.
//!
//! What is deliberately *not* imported:
//! * Chromium cookies and passwords — they are encrypted with keys bound to the source browser
//!   (DPAPI and, since Chrome 127, app-bound encryption). Decrypting them would mean
//!   circumventing another application's protection, which Zuno does not do.
//! * Firefox passwords — encrypted with the Firefox (NSS) key store, and WebView2 offers no API
//!   for adding entries to its password store.

pub mod chromium;
pub mod firefox;
#[cfg(test)]
mod e2e_tests;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::browser::engine::ImportedCookie;
use crate::error::{Error, Result};
use crate::storage::settings::{Settings, Startup};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Bookmarks,
    History,
    Cookies,
    Passwords,
    Settings,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Chromium,
    Firefox,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfile {
    pub id: String,
    pub name: String,
    #[serde(skip)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSource {
    pub id: String,
    pub name: String,
    pub engine: Engine,
    pub profiles: Vec<BrowserProfile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInfo {
    pub category: Category,
    pub available: bool,
    pub count: Option<usize>,
    /// Why the category cannot be imported, or extra detail.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingChange {
    pub key: String,
    pub label: String,
    pub current: String,
    pub imported: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub categories: Vec<CategoryInfo>,
    pub setting_changes: Vec<SettingChange>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub source: String,
    pub profile: String,
    pub categories: Vec<Category>,
    /// The user confirmed that existing Zuno settings may be replaced.
    #[serde(default)]
    pub confirm_settings_overwrite: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Imported,
    Skipped,
    Failed,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryResult {
    pub category: Category,
    pub status: Status,
    pub count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub category: Category,
    pub message: String,
    pub done: usize,
    pub total: usize,
}

/// Bookmark tree read from a source browser.
#[derive(Debug, Clone, PartialEq)]
pub enum BookmarkNode {
    Folder { title: String, children: Vec<BookmarkNode> },
    Link { title: String, url: String },
}

impl BookmarkNode {
    pub fn count_links(nodes: &[BookmarkNode]) -> usize {
        nodes
            .iter()
            .map(|n| match n {
                BookmarkNode::Folder { children, .. } => Self::count_links(children),
                BookmarkNode::Link { .. } => 1,
            })
            .sum()
    }
}

#[derive(Debug, Clone)]
pub struct Visit {
    pub url: String,
    pub title: String,
    pub visited_at: i64,
}

/// Settings readable from a source profile.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportedSettings {
    pub homepage: Option<String>,
    pub startup: Option<Startup>,
    pub search_engine: Option<String>,
}

impl ImportedSettings {
    pub fn changes(&self, current: &Settings) -> Vec<SettingChange> {
        let mut out = Vec::new();
        if let Some(h) = &self.homepage {
            if *h != current.homepage {
                out.push(SettingChange { key: "homepage".into(), label: "Homepage".into(), current: current.homepage.clone(), imported: h.clone() });
            }
        }
        if let Some(s) = self.startup {
            if s != current.startup {
                let name = |s: Startup| match s {
                    Startup::NewTab => "Open a new tab",
                    Startup::Restore => "Continue where you left off",
                    Startup::Homepage => "Open the homepage",
                };
                out.push(SettingChange { key: "startup".into(), label: "On startup".into(), current: name(current.startup).into(), imported: name(s).into() });
            }
        }
        if let Some(e) = &self.search_engine {
            if *e != current.search_engine {
                let name = |id: &str| crate::browser::navigation::find_engine(id).map(|e| e.name.to_string()).unwrap_or_else(|| id.to_string());
                out.push(SettingChange { key: "searchEngine".into(), label: "Search engine".into(), current: name(&current.search_engine), imported: name(e) });
            }
        }
        out
    }

    pub fn apply(&self, settings: &mut Settings) {
        if let Some(h) = &self.homepage {
            settings.homepage = h.clone();
        }
        if let Some(s) = self.startup {
            settings.startup = s;
        }
        if let Some(e) = &self.search_engine {
            settings.search_engine = e.clone();
        }
    }
}

/// A readable source profile.
pub trait SourceProfile {
    fn bookmarks(&self) -> Result<Vec<BookmarkNode>>;
    fn history(&self, limit: usize) -> Result<Vec<Visit>>;
    fn history_count(&self) -> Result<usize>;
    fn cookies(&self) -> Result<Vec<ImportedCookie>>;
    fn settings(&self) -> Result<ImportedSettings>;
    /// Reason a category is unavailable regardless of the profile contents.
    fn unsupported(&self, category: Category) -> Option<String>;
}

pub fn detect() -> Vec<BrowserSource> {
    let mut sources = chromium::detect();
    sources.extend(firefox::detect());
    sources
}

pub fn open(source_id: &str, profile_id: &str) -> Result<(BrowserSource, Box<dyn SourceProfile>)> {
    let source = detect().into_iter().find(|s| s.id == source_id).ok_or_else(|| Error::msg("That browser was not found"))?;
    let profile = source.profiles.iter().find(|p| p.id == profile_id).ok_or_else(|| Error::msg("That profile was not found"))?.clone();
    let reader: Box<dyn SourceProfile> = match source.engine {
        Engine::Chromium => Box::new(chromium::ChromiumProfile::new(&source.name, profile.path.clone())),
        Engine::Firefox => Box::new(firefox::FirefoxProfile::new(profile.path.clone())),
    };
    Ok((source, reader))
}

pub fn inspect(reader: &dyn SourceProfile, current: &Settings) -> Inspection {
    let mut categories = Vec::new();
    let mut setting_changes = Vec::new();
    for category in [Category::Bookmarks, Category::History, Category::Cookies, Category::Passwords, Category::Settings] {
        if let Some(reason) = reader.unsupported(category) {
            categories.push(CategoryInfo { category, available: false, count: None, reason: Some(reason) });
            continue;
        }
        let result: Result<(usize, Option<String>)> = match category {
            Category::Bookmarks => reader.bookmarks().map(|b| (BookmarkNode::count_links(&b), None)),
            Category::History => reader.history_count().map(|n| (n, None)),
            Category::Cookies => reader.cookies().map(|c| (c.len(), Some("Signs you in to sites where you were signed in. Cookie values are never shown or logged.".into()))),
            Category::Settings => reader.settings().map(|s| {
                setting_changes = s.changes(current);
                let n = setting_changes.len();
                (n, (n == 0).then(|| "Nothing differs from your current Zuno settings.".to_string()))
            }),
            Category::Passwords => Ok((0, None)),
        };
        categories.push(match result {
            Ok((count, reason)) => CategoryInfo {
                category,
                available: count > 0,
                count: Some(count),
                reason: reason.or_else(|| (count == 0).then(|| "Nothing to import from this profile.".to_string())),
            },
            Err(e) => CategoryInfo { category, available: false, count: None, reason: Some(e.to_string()) },
        });
    }
    Inspection { categories, setting_changes }
}

/// Copies a database (and its journal/WAL companions) to a private temp directory so it can be
/// read while the source browser is running, without touching the original.
pub fn copy_database(src: &Path, browser: &str) -> Result<(tempfile::TempDir, PathBuf)> {
    if !src.exists() {
        return Err(Error::msg("This data was not found in the selected profile"));
    }
    let dir = tempfile::tempdir()?;
    let name = src.file_name().ok_or(Error::NotFound)?;
    let dest = dir.path().join(name);
    std::fs::copy(src, &dest).map_err(|e| {
        tracing::warn!(error = %e, "could not copy source database");
        Error::msg(format!("The {browser} database is locked. Close {browser} and try again."))
    })?;
    for suffix in ["-wal", "-journal"] {
        let companion = PathBuf::from(format!("{}{suffix}", src.display()));
        if companion.exists() {
            let _ = std::fs::copy(&companion, PathBuf::from(format!("{}{suffix}", dest.display())));
        }
    }
    Ok((dir, dest))
}

pub fn open_sqlite(path: &Path) -> Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(2))?;
    Ok(conn)
}

/// Maps a search URL to one of Zuno's built-in engines.
pub fn engine_for_search_url(url: &str) -> Option<String> {
    let host = url::Url::parse(&url.replace("{searchTerms}", "x")).ok()?.host_str()?.to_ascii_lowercase();
    let id = if host.contains("duckduckgo.") {
        "duckduckgo"
    } else if host.contains("google.") {
        "google"
    } else if host.contains("bing.") {
        "bing"
    } else if host.contains("search.brave.") {
        "brave"
    } else if host.contains("startpage.") {
        "startpage"
    } else if host.contains("ecosia.") {
        "ecosia"
    } else {
        return None;
    };
    Some(id.to_string())
}

pub fn local_app_data() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

pub fn roaming_app_data() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

/// Maximum number of history visits imported from one profile.
const HISTORY_LIMIT: usize = 200_000;

fn insert_bookmarks(tx: &rusqlite::Connection, parent: i64, nodes: &[BookmarkNode], count: &mut usize) -> Result<()> {
    for node in nodes {
        match node {
            BookmarkNode::Folder { title, children } => {
                let id = crate::storage::bookmarks::insert(tx, parent, true, title, None)?;
                insert_bookmarks(tx, id, children, count)?;
            }
            BookmarkNode::Link { title, url } => {
                // Skip entries Zuno refuses (javascript: bookmarklets, unsupported schemes).
                if crate::storage::bookmarks::insert(tx, parent, false, title, Some(url)).is_ok() {
                    *count += 1;
                }
            }
        }
    }
    Ok(())
}

/// Runs an import. Each category is written in its own transaction, so it is either imported
/// completely or not at all. Existing Zuno data is never modified, except settings when the
/// user explicitly confirmed the replacement.
pub fn run(
    request: &ImportRequest,
    storage: &crate::storage::Storage,
    settings: &crate::storage::settings::SettingsStore,
    engine: &dyn crate::browser::engine::RenderingEngine,
    progress: &dyn Fn(Progress),
) -> Result<Vec<CategoryResult>> {
    let (source, reader) = open(&request.source, &request.profile)?;
    let mut results = Vec::new();
    for &category in &request.categories {
        let report = |message: &str, done: usize, total: usize| progress(Progress { category, message: message.to_string(), done, total });
        report("Reading", 0, 0);
        let result = run_category(category, &source, reader.as_ref(), request, storage, settings, engine, &report);
        results.push(match result {
            Ok((status, count, message)) => CategoryResult { category, status, count, message },
            Err(e) => {
                tracing::warn!(?category, error = %e, "import category failed");
                CategoryResult { category, status: Status::Failed, count: 0, message: e.to_string() }
            }
        });
    }
    tracing::info!(source = %source.id, categories = request.categories.len(), "import finished");
    Ok(results)
}

#[allow(clippy::too_many_arguments)]
fn run_category(
    category: Category,
    source: &BrowserSource,
    reader: &dyn SourceProfile,
    request: &ImportRequest,
    storage: &crate::storage::Storage,
    settings: &crate::storage::settings::SettingsStore,
    engine: &dyn crate::browser::engine::RenderingEngine,
    report: &dyn Fn(&str, usize, usize),
) -> Result<(Status, usize, String)> {
    if let Some(reason) = reader.unsupported(category) {
        return Ok((Status::Unavailable, 0, reason));
    }
    match category {
        Category::Bookmarks => {
            let nodes = reader.bookmarks()?;
            let total = BookmarkNode::count_links(&nodes);
            if total == 0 {
                return Ok((Status::Skipped, 0, "No bookmarks found".into()));
            }
            report("Saving bookmarks", 0, total);
            let folder_name = format!("Imported from {}", source.name);
            let mut count = 0;
            storage.with_tx(|tx| {
                let folder = crate::storage::bookmarks::insert(tx, crate::storage::bookmarks::OTHER_BOOKMARKS, true, &folder_name, None)?;
                insert_bookmarks(tx, folder, &nodes, &mut count)
            })?;
            report("Saving bookmarks", count, total);
            Ok((Status::Imported, count, format!("Added to \"{folder_name}\" in Other bookmarks")))
        }
        Category::History => {
            let visits = reader.history(HISTORY_LIMIT)?;
            let total = visits.len();
            report("Saving history", 0, total);
            let mut added = 0;
            storage.with_tx(|tx| {
                for (i, v) in visits.iter().enumerate() {
                    if let Some(url) = crate::security::sanitize_for_history(&v.url) {
                        if crate::storage::history::insert_visit(tx, &url, &v.title, v.visited_at)? {
                            added += 1;
                        }
                    }
                    if i % 5000 == 0 {
                        report("Saving history", i, total);
                    }
                }
                Ok(())
            })?;
            report("Saving history", total, total);
            let note = if added < total { format!("{} entries were already present or not web pages", total - added) } else { String::new() };
            Ok((Status::Imported, added, note))
        }
        Category::Cookies => {
            let cookies = reader.cookies()?;
            let total = cookies.len();
            if total == 0 {
                return Ok((Status::Skipped, 0, "No unexpired cookies found".into()));
            }
            report("Adding cookies", 0, total);
            let added = engine.import_cookies(cookies)?;
            let note = if added < total { format!("{} cookies were rejected by WebView2", total - added) } else { String::new() };
            Ok((Status::Imported, added, note))
        }
        Category::Settings => {
            let imported = reader.settings()?;
            let changes = imported.changes(&settings.get());
            if changes.is_empty() {
                return Ok((Status::Skipped, 0, "Your settings already match".into()));
            }
            if !request.confirm_settings_overwrite {
                return Ok((Status::Skipped, 0, "Not applied: replacing your current settings was not confirmed".into()));
            }
            settings.update(|s| imported.apply(s))?;
            Ok((Status::Imported, changes.len(), changes.iter().map(|c| c.label.clone()).collect::<Vec<_>>().join(", ")))
        }
        Category::Passwords => Ok((Status::Unavailable, 0, "Not supported".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_changes_only_report_differences() {
        let current = Settings::default();
        let imported = ImportedSettings {
            homepage: Some("https://home.example/".into()),
            startup: Some(Startup::Restore),
            search_engine: Some("google".into()),
        };
        let changes = imported.changes(&current);
        assert_eq!(changes.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), vec!["homepage", "searchEngine"]);
        let mut s = current.clone();
        imported.apply(&mut s);
        assert_eq!(s.search_engine, "google");
    }

    #[test]
    fn search_engine_mapping() {
        assert_eq!(engine_for_search_url("https://www.google.com/search?q={searchTerms}").as_deref(), Some("google"));
        assert_eq!(engine_for_search_url("https://duckduckgo.com/?q={searchTerms}").as_deref(), Some("duckduckgo"));
        assert_eq!(engine_for_search_url("https://unknown.example/?q={searchTerms}"), None);
    }
}
