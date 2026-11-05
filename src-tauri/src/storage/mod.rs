//! Local persistence. Structured browser data (bookmarks, history, downloads, favicons) lives in
//! a single SQLite database; user preferences live in a JSON file (see [`settings`]).

pub mod bookmarks;
pub mod favicons;
pub mod history;
pub mod session;
pub mod settings;

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{Connection, Transaction};

use crate::error::Result;

const SCHEMA_VERSION: i64 = 1;

pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let conn = Connection::open(dir.join("zuno.db"))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    /// A database that lives only in memory (incognito windows), seeded with the bookmarks in
    /// `dir` and their icons. History and downloads start empty, and `dir` is never written to.
    /// `scratch` is a throwaway folder used to read a snapshot of the regular database.
    pub fn open_ephemeral(dir: &Path, scratch: &Path) -> Result<Self> {
        let storage = Self::init(Connection::open_in_memory()?)?;
        let source = dir.join("zuno.db");
        if source.exists() {
            if let Err(e) = storage.with(|c| copy_bookmarks(c, &source, scratch)) {
                tracing::warn!(error = %e, "could not read bookmarks for incognito window");
            }
        }
        Ok(storage)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self.conn.lock();
        f(&conn)
    }

    /// Runs `f` inside a transaction that is committed only if `f` succeeds.
    pub fn with_tx<T>(&self, f: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let value = f(&tx)?;
        tx.commit()?;
        Ok(value)
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS bookmarks (
            id          INTEGER PRIMARY KEY,
            parent_id   INTEGER REFERENCES bookmarks(id) ON DELETE CASCADE,
            is_folder   INTEGER NOT NULL,
            title       TEXT NOT NULL,
            url         TEXT,
            position    INTEGER NOT NULL,
            created_at  INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS bookmarks_parent ON bookmarks(parent_id, position);
        CREATE INDEX IF NOT EXISTS bookmarks_url ON bookmarks(url);
        INSERT OR IGNORE INTO bookmarks (id, parent_id, is_folder, title, url, position, created_at)
            VALUES (1, NULL, 1, 'Bookmarks bar', NULL, 0, 0),
                   (2, NULL, 1, 'Other bookmarks', NULL, 1, 0);

        CREATE TABLE IF NOT EXISTS history (
            id          INTEGER PRIMARY KEY,
            url         TEXT NOT NULL,
            title       TEXT NOT NULL,
            visited_at  INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS history_visited ON history(visited_at DESC);
        CREATE UNIQUE INDEX IF NOT EXISTS history_url_time ON history(url, visited_at);

        CREATE TABLE IF NOT EXISTS favicons (
            host        TEXT PRIMARY KEY,
            data        TEXT NOT NULL,
            updated_at  INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS downloads (
            id              INTEGER PRIMARY KEY,
            url             TEXT NOT NULL,
            path            TEXT NOT NULL,
            file_name       TEXT NOT NULL,
            mime_type       TEXT,
            total_bytes     INTEGER,
            received_bytes  INTEGER NOT NULL DEFAULT 0,
            state           TEXT NOT NULL,
            error           TEXT,
            started_at      INTEGER NOT NULL,
            finished_at     INTEGER
        );
        "#,
    )?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

fn copy_bookmarks(conn: &Connection, source: &Path, scratch: &Path) -> Result<()> {
    // Even a read-only connection updates the regular database's shared-memory index (`-shm`)
    // when a WAL file is present, so a snapshot of the database and its WAL is read instead.
    let with_suffix = |p: &Path, s: &str| {
        let mut os = p.as_os_str().to_owned();
        os.push(s);
        std::path::PathBuf::from(os)
    };
    let snapshot = scratch.join("bookmarks-snapshot.db");
    std::fs::create_dir_all(scratch)?;
    std::fs::copy(source, &snapshot)?;
    if with_suffix(source, "-wal").exists() {
        std::fs::copy(with_suffix(source, "-wal"), with_suffix(&snapshot, "-wal"))?;
    }
    let result = attach_and_copy(conn, &snapshot);
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(with_suffix(&snapshot, suffix));
    }
    result
}

fn attach_and_copy(conn: &Connection, snapshot: &Path) -> Result<()> {
    conn.execute("ATTACH DATABASE ?1 AS src", [snapshot.to_string_lossy()])?;
    let copied = (|| -> Result<()> {
        // Rows are copied in id order, which is not necessarily parent-first.
        conn.pragma_update(None, "foreign_keys", "OFF")?;
        conn.execute(
            "INSERT OR REPLACE INTO main.bookmarks (id, parent_id, is_folder, title, url, position, created_at)
             SELECT id, parent_id, is_folder, title, url, position, created_at FROM src.bookmarks",
            [],
        )?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let urls: Vec<String> = conn
            .prepare("SELECT url FROM main.bookmarks WHERE url IS NOT NULL")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let hosts: std::collections::HashSet<String> = urls.iter().filter_map(|u| favicons::host_of(u)).collect();
        let mut icon = conn.prepare("INSERT OR IGNORE INTO main.favicons SELECT * FROM src.favicons WHERE host = ?1")?;
        for host in hosts {
            icon.execute([host])?;
        }
        Ok(())
    })();
    let _ = conn.pragma_update(None, "foreign_keys", "ON");
    conn.execute("DETACH DATABASE src", [])?;
    copied
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Escapes `%`, `_` and `\` so user input can be used safely inside a `LIKE ... ESCAPE '\'` pattern.
pub fn like_pattern(query: &str) -> String {
    let mut out = String::with_capacity(query.len() + 2);
    out.push('%');
    for c in query.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

/// Atomically replaces `path` with `contents` (write to a sibling temp file, then rename).
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    std::io::Write::write_all(&mut tmp, contents)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern("50%_off\\"), "%50\\%\\_off\\\\%");
    }

    #[test]
    fn migration_creates_root_folders() {
        let s = Storage::open_in_memory().unwrap();
        let n: i64 = s
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM bookmarks WHERE parent_id IS NULL", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn ephemeral_copies_bookmarks_only_and_never_writes() {
        let dir = tempfile::tempdir().unwrap();
        {
            let s = Storage::open(dir.path()).unwrap();
            let folder = s.bookmark_add_folder(Some(1), "Reading").unwrap();
            s.bookmark_add(Some(folder.id), "Example", "https://example.com/").unwrap();
            s.history_record("https://visited.example/", "Visited", now_ms()).unwrap();
            s.favicon_store("example.com", "data:image/png;base64,AA==").unwrap();
            s.favicon_store("visited.example", "data:image/png;base64,AA==").unwrap();
        }
        let files = |d: &Path| {
            let mut v: Vec<_> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name()).collect();
            v.sort();
            v
        };
        let before = files(dir.path());
        let db = std::fs::read(dir.path().join("zuno.db")).unwrap();

        let scratch = tempfile::tempdir().unwrap();
        let e = Storage::open_ephemeral(dir.path(), scratch.path()).unwrap();
        assert_eq!(files(scratch.path()), Vec::<std::ffi::OsString>::new(), "the snapshot is removed after reading");
        let all = e.bookmarks_all().unwrap();
        assert!(all.iter().any(|b| b.url.as_deref() == Some("https://example.com/")));
        assert!(all.iter().any(|b| b.is_folder && b.title == "Reading"));
        assert!(e.history_search("", None, 10).unwrap().is_empty(), "history is never visible");
        let icons = e.favicons_for_hosts(&["example.com".into(), "visited.example".into()]).unwrap();
        assert!(icons.contains_key("example.com"));
        assert!(!icons.contains_key("visited.example"), "icons of visited-only sites are not copied");

        e.history_record("https://private.example/", "Private", now_ms()).unwrap();
        drop(e);
        assert_eq!(files(dir.path()), before, "no files are created next to the database");
        assert_eq!(std::fs::read(dir.path().join("zuno.db")).unwrap(), db, "the database is untouched");
    }

    #[test]
    fn ephemeral_sees_bookmarks_of_a_running_regular_window() {
        let dir = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let regular = Storage::open(dir.path()).unwrap();
        regular.bookmark_add(Some(1), "Fresh", "https://fresh.example/").unwrap();
        // The change is only in the WAL file at this point.
        let shm = std::fs::read(dir.path().join("zuno.db-shm")).unwrap();
        let e = Storage::open_ephemeral(dir.path(), scratch.path()).unwrap();
        assert_eq!(std::fs::read(dir.path().join("zuno.db-shm")).unwrap(), shm, "the regular WAL index is not touched");
        assert!(e.bookmarks_all().unwrap().iter().any(|b| b.title == "Fresh"));
        e.bookmark_add(Some(1), "Private", "https://private.example/").unwrap();
        assert!(!regular.bookmarks_all().unwrap().iter().any(|b| b.title == "Private"));
    }
}
