//! Chromium-based browsers (Chrome, Edge, Brave, Vivaldi, Opera, Chromium).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::*;
use crate::error::Result;

struct Known {
    id: &'static str,
    name: &'static str,
    /// Path relative to %LOCALAPPDATA% (or %APPDATA% when `roaming`).
    dir: &'static str,
    roaming: bool,
    /// Opera keeps a single profile directly in the user-data directory.
    single_profile: bool,
}

const KNOWN: &[Known] = &[
    Known { id: "chrome", name: "Google Chrome", dir: "Google\\Chrome\\User Data", roaming: false, single_profile: false },
    Known { id: "edge", name: "Microsoft Edge", dir: "Microsoft\\Edge\\User Data", roaming: false, single_profile: false },
    Known { id: "brave", name: "Brave", dir: "BraveSoftware\\Brave-Browser\\User Data", roaming: false, single_profile: false },
    Known { id: "vivaldi", name: "Vivaldi", dir: "Vivaldi\\User Data", roaming: false, single_profile: false },
    Known { id: "chromium", name: "Chromium", dir: "Chromium\\User Data", roaming: false, single_profile: false },
    Known { id: "opera", name: "Opera", dir: "Opera Software\\Opera Stable", roaming: true, single_profile: true },
    Known { id: "opera-gx", name: "Opera GX", dir: "Opera Software\\Opera GX Stable", roaming: true, single_profile: true },
];

pub fn detect() -> Vec<BrowserSource> {
    KNOWN
        .iter()
        .filter_map(|k| {
            let base = if k.roaming { roaming_app_data() } else { local_app_data() }?.join(k.dir);
            if !base.is_dir() {
                return None;
            }
            let profiles = if k.single_profile {
                vec![BrowserProfile { id: "default".into(), name: "Default".into(), path: base }]
            } else {
                profiles_in(&base)
            };
            (!profiles.is_empty()).then(|| BrowserSource { id: k.id.into(), name: k.name.into(), engine: Engine::Chromium, profiles })
        })
        .collect()
}

/// Lists profiles from `Local State`, falling back to scanning for profile directories.
fn profiles_in(user_data: &Path) -> Vec<BrowserProfile> {
    let mut profiles = Vec::new();
    if let Some(cache) = std::fs::read(user_data.join("Local State"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.pointer("/profile/info_cache").cloned())
        .and_then(|v| v.as_object().cloned())
    {
        for (dir, info) in cache {
            let path = user_data.join(&dir);
            if path.is_dir() {
                let name = info.get("name").and_then(Value::as_str).unwrap_or(&dir).to_string();
                profiles.push(BrowserProfile { id: dir, name, path });
            }
        }
    }
    if profiles.is_empty() {
        if let Ok(entries) = std::fs::read_dir(user_data) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if (name == "Default" || name.starts_with("Profile ")) && e.path().join("Preferences").exists() {
                    profiles.push(BrowserProfile { id: name.clone(), name, path: e.path() });
                }
            }
        }
    }
    profiles.sort_by(|a, b| (a.id != "Default").cmp(&(b.id != "Default")).then(a.name.cmp(&b.name)));
    profiles
}

pub struct ChromiumProfile {
    browser: String,
    path: PathBuf,
}

impl ChromiumProfile {
    pub fn new(browser: &str, path: PathBuf) -> Self {
        Self { browser: browser.to_string(), path }
    }
}

/// Chromium timestamps are microseconds since 1601-01-01 UTC.
pub fn chromium_time_to_unix_ms(t: i64) -> i64 {
    t / 1000 - 11_644_473_600_000
}

fn parse_node(v: &Value) -> Option<BookmarkNode> {
    let title = v.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
    match v.get("type").and_then(Value::as_str)? {
        "url" => Some(BookmarkNode::Link { title, url: v.get("url").and_then(Value::as_str)?.to_string() }),
        "folder" => Some(BookmarkNode::Folder {
            title,
            children: v.get("children").and_then(Value::as_array).map(|c| c.iter().filter_map(parse_node).collect()).unwrap_or_default(),
        }),
        _ => None,
    }
}

pub fn parse_bookmarks(json: &[u8]) -> Result<Vec<BookmarkNode>> {
    let v: Value = serde_json::from_slice(json)?;
    let roots = v.get("roots").ok_or_else(|| Error::msg("The bookmarks file is not in a recognised format"))?;
    let mut out = Vec::new();
    for (key, title) in [("bookmark_bar", "Bookmarks bar"), ("other", "Other bookmarks"), ("synced", "Mobile bookmarks")] {
        if let Some(BookmarkNode::Folder { children, .. }) = roots.get(key).and_then(parse_node) {
            if !children.is_empty() {
                out.push(BookmarkNode::Folder { title: title.to_string(), children });
            }
        }
    }
    Ok(out)
}

impl SourceProfile for ChromiumProfile {
    fn bookmarks(&self) -> Result<Vec<BookmarkNode>> {
        let path = self.path.join("Bookmarks");
        if !path.exists() {
            return Ok(Vec::new());
        }
        parse_bookmarks(&std::fs::read(path)?)
    }

    fn history_count(&self) -> Result<usize> {
        let (_dir, db) = copy_database(&self.path.join("History"), &self.browser)?;
        let conn = open_sqlite(&db)?;
        Ok(conn.query_row("SELECT COUNT(*) FROM visits", [], |r| r.get::<_, i64>(0))? as usize)
    }

    fn history(&self, limit: usize) -> Result<Vec<Visit>> {
        let (_dir, db) = copy_database(&self.path.join("History"), &self.browser)?;
        let conn = open_sqlite(&db)?;
        let mut stmt = conn.prepare(
            "SELECT u.url, u.title, v.visit_time FROM visits v JOIN urls u ON u.id = v.url
             ORDER BY v.visit_time DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit as i64], |r| {
                Ok(Visit { url: r.get(0)?, title: r.get::<_, Option<String>>(1)?.unwrap_or_default(), visited_at: chromium_time_to_unix_ms(r.get(2)?) })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn cookies(&self) -> Result<Vec<ImportedCookie>> {
        Ok(Vec::new())
    }

    fn settings(&self) -> Result<ImportedSettings> {
        let prefs: Value = serde_json::from_slice(&std::fs::read(self.path.join("Preferences"))?)?;
        let mut s = ImportedSettings::default();
        let startup_urls: Vec<String> = prefs
            .pointer("/session/startup_urls")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default();
        let valid = |u: &str| url::Url::parse(u).ok().filter(|u| matches!(u.scheme(), "http" | "https")).map(|u| u.to_string());
        match prefs.pointer("/session/restore_on_startup").and_then(Value::as_i64) {
            Some(1) => s.startup = Some(Startup::Restore),
            Some(5) => s.startup = Some(Startup::NewTab),
            Some(4) => {
                if let Some(first) = startup_urls.iter().find_map(|u| valid(u)) {
                    s.startup = Some(Startup::Homepage);
                    s.homepage = Some(first);
                }
            }
            _ => {}
        }
        if s.homepage.is_none() && prefs.get("homepage_is_newtabpage").and_then(Value::as_bool) == Some(false) {
            s.homepage = prefs.get("homepage").and_then(Value::as_str).and_then(valid);
        }
        s.search_engine = prefs
            .pointer("/default_search_provider_data/template_url_data/url")
            .and_then(Value::as_str)
            .and_then(engine_for_search_url);
        Ok(s)
    }

    fn unsupported(&self, category: Category) -> Option<String> {
        let b = &self.browser;
        match category {
            Category::Cookies => Some(format!(
                "{b} encrypts cookies with keys that only {b} can use. Zuno does not bypass browser encryption, so sign in to sites again in Zuno."
            )),
            Category::Passwords => Some(format!(
                "{b} encrypts saved passwords for its own use, and WebView2 does not let browsers add saved passwords. Export them from {b} to a password manager instead."
            )),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bookmark_roots() {
        let json = br#"{"roots":{
            "bookmark_bar":{"type":"folder","name":"Bookmarks bar","children":[
                {"type":"url","name":"Rust","url":"https://www.rust-lang.org/"},
                {"type":"folder","name":"Dev","children":[{"type":"url","name":"Tauri","url":"https://tauri.app/"}]}
            ]},
            "other":{"type":"folder","name":"Other","children":[]},
            "synced":{"type":"folder","name":"Mobile","children":[{"type":"url","name":"M","url":"https://m.example/"}]}
        },"version":1}"#;
        let nodes = parse_bookmarks(json).unwrap();
        assert_eq!(nodes.len(), 2, "empty roots are skipped");
        assert_eq!(BookmarkNode::count_links(&nodes), 3);
    }

    #[test]
    fn time_conversion() {
        // 2024-01-01T00:00:00Z
        assert_eq!(chromium_time_to_unix_ms(13_348_540_800_000_000), 1_704_067_200_000);
    }
}
