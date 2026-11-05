use rusqlite::{params, Connection, Row};
use serde::Serialize;

use super::{like_pattern, Storage};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visited_at: i64,
}

impl HistoryEntry {
    fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self { id: r.get(0)?, url: r.get(1)?, title: r.get(2)?, visited_at: r.get(3)? })
    }
}

/// Inserts a visit, ignoring exact duplicates (same URL and timestamp). Returns whether a row was added.
pub fn insert_visit(conn: &Connection, url: &str, title: &str, visited_at: i64) -> Result<bool> {
    let title: String = title.chars().take(500).collect();
    let n = conn.execute(
        "INSERT OR IGNORE INTO history (url, title, visited_at) VALUES (?1, ?2, ?3)",
        params![url, title, visited_at],
    )?;
    Ok(n > 0)
}

impl Storage {
    /// Records a visit. Consecutive visits to the same URL within a short window are merged so
    /// reloads and redirects do not flood the history list.
    pub fn history_record(&self, url: &str, title: &str, visited_at: i64) -> Result<i64> {
        self.with(|c| {
            let recent: Option<(i64, String, i64)> = c
                .query_row("SELECT id, url, visited_at FROM history ORDER BY visited_at DESC LIMIT 1", [], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .ok();
            if let Some((id, last_url, last_at)) = recent {
                if last_url == url && visited_at - last_at < 30_000 {
                    if !title.is_empty() {
                        c.execute("UPDATE history SET title = ?1 WHERE id = ?2", params![title, id])?;
                    }
                    return Ok(id);
                }
            }
            insert_visit(c, url, title, visited_at)?;
            Ok(c.last_insert_rowid())
        })
    }

    pub fn history_update_title(&self, id: i64, title: &str) -> Result<()> {
        let title: String = title.chars().take(500).collect();
        self.with(|c| {
            c.execute("UPDATE history SET title = ?1 WHERE id = ?2", params![title, id])?;
            Ok(())
        })
    }

    /// Searches history newest-first. `before` enables pagination by timestamp.
    pub fn history_search(&self, query: &str, before: Option<i64>, limit: i64) -> Result<Vec<HistoryEntry>> {
        let pattern = like_pattern(query.trim());
        let before = before.unwrap_or(i64::MAX);
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, url, title, visited_at FROM history
                 WHERE visited_at < ?1 AND (title LIKE ?2 ESCAPE '\\' OR url LIKE ?2 ESCAPE '\\')
                 ORDER BY visited_at DESC LIMIT ?3",
            )?;
            let rows = stmt
                .query_map(params![before, pattern, limit], HistoryEntry::from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    /// Most recent visit per distinct URL matching `query` (used by the quick switcher).
    pub fn history_search_distinct(&self, query: &str, limit: i64) -> Result<Vec<HistoryEntry>> {
        let pattern = like_pattern(query.trim());
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT MAX(id), url, title, MAX(visited_at) AS last FROM history
                 WHERE title LIKE ?1 ESCAPE '\\' OR url LIKE ?1 ESCAPE '\\'
                 GROUP BY url ORDER BY COUNT(*) DESC, last DESC LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![pattern, limit], HistoryEntry::from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    /// Most frequently visited sites, for the new-tab page.
    pub fn history_top_sites(&self, limit: i64) -> Result<Vec<HistoryEntry>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT MAX(id), url, title, MAX(visited_at) FROM history
                 WHERE visited_at > ?1
                 GROUP BY url ORDER BY COUNT(*) DESC LIMIT ?2",
            )?;
            let since = super::now_ms() - 90 * 24 * 3600 * 1000;
            let rows = stmt
                .query_map(params![since, limit], HistoryEntry::from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn history_delete(&self, id: i64) -> Result<()> {
        self.with(|c| {
            if c.execute("DELETE FROM history WHERE id = ?1", [id])? == 0 {
                return Err(Error::NotFound);
            }
            Ok(())
        })
    }

    pub fn history_clear(&self) -> Result<()> {
        self.with(|c| {
            c.execute("DELETE FROM history", [])?;
            Ok(())
        })
    }

    #[cfg(test)]
    pub fn history_count(&self) -> Result<i64> {
        self.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM history", [], |r| r.get(0))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_merge_search_delete() {
        let s = Storage::open_in_memory().unwrap();
        let a = s.history_record("https://a.test/", "A", 1_000).unwrap();
        // A reload a few seconds later merges into the same row.
        let a2 = s.history_record("https://a.test/", "A updated", 5_000).unwrap();
        assert_eq!(a, a2);
        s.history_record("https://b.test/", "Bee", 100_000).unwrap();
        s.history_record("https://a.test/", "A", 200_000).unwrap();
        assert_eq!(s.history_count().unwrap(), 3);

        let found = s.history_search("bee", None, 10).unwrap();
        assert_eq!(found.len(), 1);
        let distinct = s.history_search_distinct("a.test", 10).unwrap();
        assert_eq!(distinct.len(), 1);
        assert_eq!(distinct[0].visited_at, 200_000);

        // Pagination by timestamp.
        assert_eq!(s.history_search("", Some(100_000), 10).unwrap().len(), 1);

        s.history_delete(found[0].id).unwrap();
        assert_eq!(s.history_count().unwrap(), 2);
        s.history_clear().unwrap();
        assert_eq!(s.history_count().unwrap(), 0);
    }

    #[test]
    fn duplicate_import_rows_are_ignored() {
        let s = Storage::open_in_memory().unwrap();
        s.with(|c| {
            assert!(insert_visit(c, "https://x.test/", "X", 42)?);
            assert!(!insert_visit(c, "https://x.test/", "X", 42)?);
            Ok(())
        })
        .unwrap();
    }
}
