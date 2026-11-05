use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use super::{like_pattern, now_ms, Storage};
use crate::error::{Error, Result};
use crate::security;

pub const BOOKMARKS_BAR: i64 = 1;
pub const OTHER_BOOKMARKS: i64 = 2;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub is_folder: bool,
    pub title: String,
    pub url: Option<String>,
    pub position: i64,
    pub created_at: i64,
}

impl Bookmark {
    fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            parent_id: r.get(1)?,
            is_folder: r.get::<_, i64>(2)? != 0,
            title: r.get(3)?,
            url: r.get(4)?,
            position: r.get(5)?,
            created_at: r.get(6)?,
        })
    }
}

const COLUMNS: &str = "id, parent_id, is_folder, title, url, position, created_at";

fn is_root(id: i64) -> bool {
    id == BOOKMARKS_BAR || id == OTHER_BOOKMARKS
}

fn validate_url(url: &str) -> Result<String> {
    let parsed = url::Url::parse(url.trim())?;
    if !security::is_bookmarkable_scheme(parsed.scheme()) {
        return Err(Error::msg("Only web addresses can be bookmarked"));
    }
    Ok(parsed.to_string())
}

fn clean_title(title: &str, fallback: &str) -> String {
    let t = title.trim();
    let t = if t.is_empty() { fallback } else { t };
    t.chars().take(500).collect()
}

fn ensure_folder(conn: &Connection, id: i64) -> Result<()> {
    let is_folder: Option<i64> = conn
        .query_row("SELECT is_folder FROM bookmarks WHERE id = ?1", [id], |r| r.get(0))
        .optional()?;
    match is_folder {
        Some(1) => Ok(()),
        Some(_) => Err(Error::msg("Bookmarks can only be placed inside folders")),
        None => Err(Error::NotFound),
    }
}

fn next_position(conn: &Connection, parent: i64) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM bookmarks WHERE parent_id = ?1",
        [parent],
        |r| r.get(0),
    )?)
}

/// Inserts a bookmark or folder using an existing connection/transaction (used by the importer).
pub fn insert(conn: &Connection, parent: i64, is_folder: bool, title: &str, url: Option<&str>) -> Result<i64> {
    ensure_folder(conn, parent)?;
    let url = match (is_folder, url) {
        (true, _) => None,
        (false, Some(u)) => Some(validate_url(u)?),
        (false, None) => return Err(Error::msg("A bookmark needs an address")),
    };
    let fallback = url.as_deref().unwrap_or("New folder");
    let position = next_position(conn, parent)?;
    conn.execute(
        "INSERT INTO bookmarks (parent_id, is_folder, title, url, position, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![parent, is_folder as i64, clean_title(title, fallback), url, position, now_ms()],
    )?;
    Ok(conn.last_insert_rowid())
}

impl Storage {
    pub fn bookmarks_all(&self) -> Result<Vec<Bookmark>> {
        self.with(|c| {
            let mut stmt = c.prepare(&format!("SELECT {COLUMNS} FROM bookmarks ORDER BY parent_id, position"))?;
            let rows = stmt.query_map([], Bookmark::from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn bookmark_add(&self, parent: Option<i64>, title: &str, url: &str) -> Result<Bookmark> {
        let parent = parent.unwrap_or(OTHER_BOOKMARKS);
        self.with_tx(|tx| {
            let id = insert(tx, parent, false, title, Some(url))?;
            get(tx, id)
        })
    }

    pub fn bookmark_add_folder(&self, parent: Option<i64>, title: &str) -> Result<Bookmark> {
        let parent = parent.unwrap_or(OTHER_BOOKMARKS);
        self.with_tx(|tx| {
            let id = insert(tx, parent, true, title, None)?;
            get(tx, id)
        })
    }

    pub fn bookmark_update(&self, id: i64, title: &str, url: Option<&str>) -> Result<Bookmark> {
        if is_root(id) {
            return Err(Error::msg("Built-in folders cannot be renamed"));
        }
        self.with_tx(|tx| {
            let current = get(tx, id)?;
            let url = if current.is_folder {
                None
            } else {
                Some(validate_url(url.or(current.url.as_deref()).unwrap_or_default())?)
            };
            let fallback = url.clone().unwrap_or_else(|| current.title.clone());
            tx.execute(
                "UPDATE bookmarks SET title = ?1, url = ?2 WHERE id = ?3",
                params![clean_title(title, &fallback), url, id],
            )?;
            get(tx, id)
        })
    }

    /// Moves `id` into `parent` at `position` (appends when `position` is `None`).
    pub fn bookmark_move(&self, id: i64, parent: i64, position: Option<i64>) -> Result<()> {
        if is_root(id) {
            return Err(Error::msg("Built-in folders cannot be moved"));
        }
        self.with_tx(|tx| {
            ensure_folder(tx, parent)?;
            // Refuse to move a folder into itself or one of its descendants.
            let mut cursor = Some(parent);
            while let Some(p) = cursor {
                if p == id {
                    return Err(Error::msg("A folder cannot be moved into itself"));
                }
                cursor = tx.query_row("SELECT parent_id FROM bookmarks WHERE id = ?1", [p], |r| r.get(0))?;
            }
            let mut siblings: Vec<i64> = {
                let mut stmt =
                    tx.prepare("SELECT id FROM bookmarks WHERE parent_id = ?1 AND id != ?2 ORDER BY position")?;
                let rows = stmt.query_map(params![parent, id], |r| r.get(0))?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            let index = position.map(|p| p.clamp(0, siblings.len() as i64) as usize).unwrap_or(siblings.len());
            siblings.insert(index, id);
            tx.execute("UPDATE bookmarks SET parent_id = ?1 WHERE id = ?2", params![parent, id])?;
            for (i, sid) in siblings.iter().enumerate() {
                tx.execute("UPDATE bookmarks SET position = ?1 WHERE id = ?2", params![i as i64, sid])?;
            }
            Ok(())
        })
    }

    pub fn bookmark_remove(&self, id: i64) -> Result<()> {
        if is_root(id) {
            return Err(Error::msg("Built-in folders cannot be removed"));
        }
        self.with(|c| {
            let n = c.execute("DELETE FROM bookmarks WHERE id = ?1", [id])?;
            if n == 0 {
                return Err(Error::NotFound);
            }
            Ok(())
        })
    }

    pub fn bookmark_find_by_url(&self, url: &str) -> Result<Option<Bookmark>> {
        self.with(|c| {
            Ok(c.query_row(
                &format!("SELECT {COLUMNS} FROM bookmarks WHERE is_folder = 0 AND url = ?1 LIMIT 1"),
                [url],
                Bookmark::from_row,
            )
            .optional()?)
        })
    }

    pub fn bookmark_search(&self, query: &str, limit: i64) -> Result<Vec<Bookmark>> {
        let pattern = like_pattern(query.trim());
        self.with(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {COLUMNS} FROM bookmarks WHERE is_folder = 0
                 AND (title LIKE ?1 ESCAPE '\\' OR url LIKE ?1 ESCAPE '\\')
                 ORDER BY created_at DESC LIMIT ?2"
            ))?;
            let rows = stmt
                .query_map(params![pattern, limit], Bookmark::from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Serialises bookmarks in the Netscape bookmark file format understood by every major browser.
pub fn export_html(all: &[Bookmark]) -> String {
    fn walk(all: &[Bookmark], parent: i64, depth: usize, out: &mut String) {
        let mut children: Vec<&Bookmark> = all.iter().filter(|b| b.parent_id == Some(parent)).collect();
        children.sort_by_key(|b| b.position);
        let indent = "    ".repeat(depth);
        for b in children {
            let added = b.created_at / 1000;
            if b.is_folder {
                out.push_str(&format!("{indent}<DT><H3 ADD_DATE=\"{added}\">{}</H3>\n{indent}<DL><p>\n", escape_html(&b.title)));
                walk(all, b.id, depth + 1, out);
                out.push_str(&format!("{indent}</DL><p>\n"));
            } else if let Some(url) = &b.url {
                out.push_str(&format!("{indent}<DT><A HREF=\"{}\" ADD_DATE=\"{added}\">{}</A>\n", escape_html(url), escape_html(&b.title)));
            }
        }
    }
    let mut out = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<!-- Exported by Zuno -->\n<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n<DL><p>\n",
    );
    out.push_str("    <DT><H3 PERSONAL_TOOLBAR_FOLDER=\"true\">Bookmarks bar</H3>\n    <DL><p>\n");
    walk(all, BOOKMARKS_BAR, 2, &mut out);
    out.push_str("    </DL><p>\n");
    walk(all, OTHER_BOOKMARKS, 1, &mut out);
    out.push_str("</DL><p>\n");
    out
}

fn get(conn: &Connection, id: i64) -> Result<Bookmark> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM bookmarks WHERE id = ?1"), [id], Bookmark::from_row)
        .optional()?
        .ok_or(Error::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_edit_search_remove() {
        let s = Storage::open_in_memory().unwrap();
        let b = s.bookmark_add(None, "Example", "https://example.com").unwrap();
        assert_eq!(b.parent_id, Some(OTHER_BOOKMARKS));
        assert_eq!(b.url.as_deref(), Some("https://example.com/"));

        let b = s.bookmark_update(b.id, "Renamed", Some("https://example.org/a")).unwrap();
        assert_eq!(b.title, "Renamed");
        assert_eq!(s.bookmark_search("example.org", 10).unwrap().len(), 1);
        assert_eq!(s.bookmark_search("nothing", 10).unwrap().len(), 0);
        assert!(s.bookmark_find_by_url("https://example.org/a").unwrap().is_some());

        s.bookmark_remove(b.id).unwrap();
        assert!(s.bookmark_find_by_url("https://example.org/a").unwrap().is_none());
    }

    #[test]
    fn exports_netscape_html() {
        let s = Storage::open_in_memory().unwrap();
        let f = s.bookmark_add_folder(Some(BOOKMARKS_BAR), "Dev & Tools").unwrap();
        s.bookmark_add(Some(f.id), "Rust <lang>", "https://www.rust-lang.org/?a=1&b=2").unwrap();
        s.bookmark_add(None, "Other", "https://other.test/").unwrap();
        let html = export_html(&s.bookmarks_all().unwrap());
        assert!(html.starts_with("<!DOCTYPE NETSCAPE-Bookmark-file-1>"));
        assert!(html.contains("<H3 ADD_DATE="));
        assert!(html.contains(">Dev &amp; Tools</H3>"));
        assert!(html.contains("HREF=\"https://www.rust-lang.org/?a=1&amp;b=2\""));
        assert!(html.contains(">Rust &lt;lang&gt;</A>"));
        assert!(html.contains("HREF=\"https://other.test/\""));
    }

    #[test]
    fn rejects_unsafe_urls_and_root_changes() {
        let s = Storage::open_in_memory().unwrap();
        assert!(s.bookmark_add(None, "x", "javascript:alert(1)").is_err());
        assert!(s.bookmark_remove(BOOKMARKS_BAR).is_err());
        assert!(s.bookmark_update(OTHER_BOOKMARKS, "x", None).is_err());
    }

    #[test]
    fn folders_and_moves() {
        let s = Storage::open_in_memory().unwrap();
        let f = s.bookmark_add_folder(Some(BOOKMARKS_BAR), "Work").unwrap();
        let sub = s.bookmark_add_folder(Some(f.id), "Sub").unwrap();
        let a = s.bookmark_add(Some(f.id), "A", "https://a.test").unwrap();
        let b = s.bookmark_add(Some(f.id), "B", "https://b.test").unwrap();
        s.bookmark_move(b.id, f.id, Some(0)).unwrap();
        let all = s.bookmarks_all().unwrap();
        let mut children: Vec<_> = all.iter().filter(|x| x.parent_id == Some(f.id)).collect();
        children.sort_by_key(|x| x.position);
        assert_eq!(children.iter().map(|x| x.id).collect::<Vec<_>>(), vec![b.id, sub.id, a.id]);
        // Cycles are refused.
        assert!(s.bookmark_move(f.id, sub.id, None).is_err());
        // Bookmarks cannot contain children.
        assert!(s.bookmark_add(Some(a.id), "C", "https://c.test").is_err());
        // Deleting a folder removes its contents.
        s.bookmark_remove(f.id).unwrap();
        assert!(s.bookmark_find_by_url("https://a.test/").unwrap().is_none());
    }
}
