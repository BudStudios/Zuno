//! Firefox profiles (bookmarks and history from places.sqlite, cookies from cookies.sqlite).

use std::path::PathBuf;

use super::*;
use crate::browser::engine::SameSite;
use crate::error::Result;

pub fn detect() -> Vec<BrowserSource> {
    let Some(base) = roaming_app_data().map(|p| p.join("Mozilla").join("Firefox")) else { return Vec::new() };
    let Ok(ini) = std::fs::read_to_string(base.join("profiles.ini")) else { return Vec::new() };
    let profiles: Vec<BrowserProfile> = parse_profiles_ini(&ini)
        .into_iter()
        .filter_map(|(name, path, relative)| {
            let full = if relative { base.join(&path) } else { PathBuf::from(&path) };
            full.join("places.sqlite").exists().then(|| BrowserProfile { id: path.replace('\\', "/"), name, path: full })
        })
        .collect();
    if profiles.is_empty() {
        return Vec::new();
    }
    vec![BrowserSource { id: "firefox".into(), name: "Mozilla Firefox".into(), engine: Engine::Firefox, profiles }]
}

/// Returns (name, path, is_relative) for each `[ProfileN]` section.
pub fn parse_profiles_ini(ini: &str) -> Vec<(String, String, bool)> {
    let mut out = Vec::new();
    let mut current: Option<(Option<String>, Option<String>, bool)> = None;
    let flush = |c: Option<(Option<String>, Option<String>, bool)>, out: &mut Vec<_>| {
        if let Some((name, Some(path), rel)) = c {
            out.push((name.unwrap_or_else(|| path.clone()), path, rel));
        }
    };
    for line in ini.lines().map(str::trim) {
        if line.starts_with('[') {
            flush(current.take(), &mut out);
            if line.to_ascii_lowercase().starts_with("[profile") {
                current = Some((None, None, true));
            }
        } else if let (Some(c), Some((k, v))) = (current.as_mut(), line.split_once('=')) {
            match k.trim() {
                "Name" => c.0 = Some(v.trim().to_string()),
                "Path" => c.1 = Some(v.trim().to_string()),
                "IsRelative" => c.2 = v.trim() == "1",
                _ => {}
            }
        }
    }
    flush(current, &mut out);
    out
}

pub struct FirefoxProfile {
    path: PathBuf,
}

impl FirefoxProfile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn places(&self) -> Result<(tempfile::TempDir, rusqlite::Connection)> {
        let (dir, db) = copy_database(&self.path.join("places.sqlite"), "Firefox")?;
        Ok((dir, open_sqlite(&db)?))
    }
}

fn read_children(conn: &rusqlite::Connection, parent: i64, depth: usize) -> Result<Vec<BookmarkNode>> {
    if depth > 32 {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT b.id, b.type, COALESCE(b.title, ''), p.url FROM moz_bookmarks b
         LEFT JOIN moz_places p ON p.id = b.fk WHERE b.parent = ?1 ORDER BY b.position",
    )?;
    let rows: Vec<(i64, i64, String, Option<String>)> =
        stmt.query_map([parent], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for (id, kind, title, url) in rows {
        match (kind, url) {
            (1, Some(url)) if url.starts_with("http") || url.starts_with("file:") => out.push(BookmarkNode::Link { title, url }),
            (2, _) => out.push(BookmarkNode::Folder { title, children: read_children(conn, id, depth + 1)? }),
            _ => {}
        }
    }
    Ok(out)
}

impl SourceProfile for FirefoxProfile {
    fn bookmarks(&self) -> Result<Vec<BookmarkNode>> {
        let (_dir, conn) = self.places()?;
        let mut out = Vec::new();
        for (guid, title) in [("toolbar_____", "Bookmarks toolbar"), ("menu________", "Bookmarks menu"), ("unfiled_____", "Other bookmarks"), ("mobile______", "Mobile bookmarks")] {
            let id: Option<i64> = conn.query_row("SELECT id FROM moz_bookmarks WHERE guid = ?1", [guid], |r| r.get(0)).ok();
            if let Some(id) = id {
                let children = read_children(&conn, id, 0)?;
                if !children.is_empty() {
                    out.push(BookmarkNode::Folder { title: title.into(), children });
                }
            }
        }
        Ok(out)
    }

    fn history_count(&self) -> Result<usize> {
        let (_dir, conn) = self.places()?;
        Ok(conn.query_row("SELECT COUNT(*) FROM moz_historyvisits", [], |r| r.get::<_, i64>(0))? as usize)
    }

    fn history(&self, limit: usize) -> Result<Vec<Visit>> {
        let (_dir, conn) = self.places()?;
        let mut stmt = conn.prepare(
            "SELECT p.url, COALESCE(p.title, ''), v.visit_date FROM moz_historyvisits v
             JOIN moz_places p ON p.id = v.place_id ORDER BY v.visit_date DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit as i64], |r| Ok(Visit { url: r.get(0)?, title: r.get(1)?, visited_at: r.get::<_, i64>(2)? / 1000 }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn cookies(&self) -> Result<Vec<ImportedCookie>> {
        let src = self.path.join("cookies.sqlite");
        if !src.exists() {
            return Ok(Vec::new());
        }
        let (_dir, db) = copy_database(&src, "Firefox")?;
        let conn = open_sqlite(&db)?;
        let now_s = crate::storage::now_ms() as f64 / 1000.0;
        let mut stmt = conn.prepare("SELECT name, value, host, path, expiry, isSecure, isHttpOnly, sameSite FROM moz_cookies")?;
        let rows = stmt
            .query_map([], |r| {
                let expiry: i64 = r.get(4)?;
                // Newer Firefox versions store milliseconds; older ones seconds.
                let expires = if expiry > 100_000_000_000 { expiry as f64 / 1000.0 } else { expiry as f64 };
                Ok(ImportedCookie {
                    name: r.get(0)?,
                    value: r.get(1)?,
                    domain: r.get(2)?,
                    path: r.get(3)?,
                    expires: Some(expires),
                    secure: r.get::<_, i64>(5)? != 0,
                    http_only: r.get::<_, i64>(6)? != 0,
                    same_site: match r.get::<_, i64>(7)? {
                        1 => Some(SameSite::Lax),
                        2 => Some(SameSite::Strict),
                        _ => Some(SameSite::None),
                    },
                })
            })?
            .filter_map(|c| c.ok())
            .filter(|c| c.expires.is_some_and(|e| e > now_s))
            .collect();
        Ok(rows)
    }

    fn settings(&self) -> Result<ImportedSettings> {
        let prefs = std::fs::read_to_string(self.path.join("prefs.js")).unwrap_or_default();
        Ok(parse_prefs(&prefs))
    }

    fn unsupported(&self, category: Category) -> Option<String> {
        match category {
            Category::Passwords => Some(
                "Firefox encrypts saved passwords with its own key store, and WebView2 does not let browsers add saved passwords. Export them from Firefox to a password manager instead."
                    .into(),
            ),
            _ => None,
        }
    }
}

fn pref_value(prefs: &str, name: &str) -> Option<String> {
    let needle = format!("user_pref(\"{name}\",");
    let line = prefs.lines().find(|l| l.trim_start().starts_with(&needle))?;
    let raw = line.trim_start()[needle.len()..].trim().trim_end_matches(';').trim_end_matches(')').trim();
    Some(raw.trim_matches('"').to_string())
}

pub fn parse_prefs(prefs: &str) -> ImportedSettings {
    let mut s = ImportedSettings::default();
    match pref_value(prefs, "browser.startup.page").as_deref() {
        Some("3") => s.startup = Some(Startup::Restore),
        Some("0") => s.startup = Some(Startup::NewTab),
        Some("1") => s.startup = Some(Startup::Homepage),
        _ => {}
    }
    s.homepage = pref_value(prefs, "browser.startup.homepage")
        .and_then(|h| h.split('|').next().map(str::to_owned))
        .and_then(|h| url::Url::parse(&h).ok())
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .map(|u| u.to_string());
    if s.homepage.is_none() && s.startup == Some(Startup::Homepage) {
        s.startup = None;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_ini() {
        let ini = "[Install4F96D1932A9F858E]\nDefault=Profiles/abc.default-release\n\n[Profile1]\nName=default\nIsRelative=1\nPath=Profiles/xyz.default\n\n[Profile0]\nName=default-release\nIsRelative=1\nPath=Profiles/abc.default-release\nDefault=1\n\n[General]\nStartWithLastProfile=1\n";
        let p = parse_profiles_ini(ini);
        assert_eq!(p.len(), 2);
        assert_eq!(p[1], ("default-release".into(), "Profiles/abc.default-release".into(), true));
    }

    #[test]
    fn prefs() {
        let prefs = "user_pref(\"browser.startup.homepage\", \"https://start.example/|https://two.example/\");\nuser_pref(\"browser.startup.page\", 3);\n";
        let s = parse_prefs(prefs);
        assert_eq!(s.homepage.as_deref(), Some("https://start.example/"));
        assert_eq!(s.startup, Some(Startup::Restore));
    }
}
