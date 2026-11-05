//! Download manager: decides where files are saved (never overwriting existing files), tracks
//! progress reported by the engine, and persists the download list.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{params, Row};
use serde::Serialize;

use crate::browser::engine::{DownloadRequest, DownloadTicket, DownloadUpdate};
use crate::error::{Error, Result};
use crate::security::{sanitize_file_name, split_extension};
use crate::storage::{now_ms, Storage};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadState {
    InProgress,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl DownloadState {
    fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "inprogress",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            "paused" | "inprogress" => Self::Failed,
            _ => Self::Failed,
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::InProgress | Self::Paused)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub id: i64,
    pub url: String,
    pub path: String,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub total_bytes: Option<i64>,
    pub received_bytes: i64,
    pub state: DownloadState,
    pub error: Option<String>,
    pub can_resume: bool,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub exists: bool,
}

impl DownloadItem {
    fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        let state: String = r.get(7)?;
        let parsed = DownloadState::parse(&state);
        let path: String = r.get(2)?;
        Ok(Self {
            id: r.get(0)?,
            url: r.get(1)?,
            exists: Path::new(&path).exists(),
            path,
            file_name: r.get(3)?,
            mime_type: r.get(4)?,
            total_bytes: r.get(5)?,
            received_bytes: r.get(6)?,
            state: parsed,
            error: if parsed == DownloadState::Failed && state != "failed" {
                Some("Interrupted when Zuno closed".into())
            } else {
                r.get(8)?
            },
            can_resume: false,
            started_at: r.get(9)?,
            finished_at: r.get(10)?,
        })
    }
}

pub struct DownloadManager {
    storage: Arc<Storage>,
    /// Downloads started in this session (newest last). Older ones are read from the database.
    active: Mutex<Vec<DownloadItem>>,
    /// Paths handed to the engine that may not exist on disk yet.
    reserved: Mutex<HashSet<PathBuf>>,
}

impl DownloadManager {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage, active: Mutex::new(Vec::new()), reserved: Mutex::new(HashSet::new()) }
    }

    /// Picks a destination in `dir` and records the download.
    pub fn start(&self, dir: &Path, request: &DownloadRequest) -> Result<(DownloadTicket, DownloadItem)> {
        std::fs::create_dir_all(dir)?;
        let name = suggested_name(request);
        let path = {
            let mut reserved = self.reserved.lock();
            let path = unique_path(dir, &name, |p| p.exists() || reserved.contains(p));
            reserved.insert(path.clone());
            path
        };
        let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(name);
        let started_at = now_ms();
        let id = self.storage.with(|c| {
            c.execute(
                "INSERT INTO downloads (url, path, file_name, mime_type, total_bytes, received_bytes, state, started_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, 'inprogress', ?6)",
                params![request.url, path.to_string_lossy(), file_name, request.mime_type, request.total_bytes, started_at],
            )?;
            Ok(c.last_insert_rowid())
        })?;
        let item = DownloadItem {
            id,
            url: request.url.clone(),
            path: path.to_string_lossy().into_owned(),
            file_name,
            mime_type: request.mime_type.clone(),
            total_bytes: request.total_bytes,
            received_bytes: 0,
            state: DownloadState::InProgress,
            error: None,
            can_resume: false,
            started_at,
            finished_at: None,
            exists: false,
        };
        self.active.lock().push(item.clone());
        tracing::info!(id, "download started");
        Ok((DownloadTicket { id, path }, item))
    }

    /// Applies an engine update. Returns the updated item for the UI.
    pub fn update(&self, id: i64, update: DownloadUpdate) -> Option<DownloadItem> {
        let mut active = self.active.lock();
        let item = active.iter_mut().find(|d| d.id == id)?;
        let mut persist = false;
        match update {
            DownloadUpdate::Progress { received, total } => {
                item.received_bytes = received;
                if total.is_some() {
                    item.total_bytes = total;
                }
            }
            DownloadUpdate::Paused => {
                item.state = DownloadState::Paused;
                item.can_resume = true;
            }
            DownloadUpdate::Resumed => {
                item.state = DownloadState::InProgress;
                item.error = None;
            }
            DownloadUpdate::Completed { received } => {
                item.state = DownloadState::Completed;
                item.received_bytes = received;
                item.total_bytes = Some(received);
                item.finished_at = Some(now_ms());
                item.can_resume = false;
                item.exists = true;
                persist = true;
            }
            DownloadUpdate::Interrupted { reason, can_resume, cancelled } => {
                item.can_resume = can_resume;
                if cancelled {
                    item.state = DownloadState::Cancelled;
                    item.error = None;
                    item.finished_at = Some(now_ms());
                } else if can_resume {
                    item.state = DownloadState::Paused;
                    item.error = Some(reason);
                } else {
                    item.state = DownloadState::Failed;
                    item.error = Some(reason);
                    item.finished_at = Some(now_ms());
                }
                persist = !can_resume;
            }
        }
        let item = item.clone();
        drop(active);
        if !item.state.is_active() {
            self.reserved.lock().remove(Path::new(&item.path));
        }
        if persist {
            let _ = self.storage.with(|c| {
                c.execute(
                    "UPDATE downloads SET received_bytes = ?1, total_bytes = ?2, state = ?3, error = ?4, finished_at = ?5 WHERE id = ?6",
                    params![item.received_bytes, item.total_bytes, item.state.as_str(), item.error, item.finished_at, item.id],
                )?;
                Ok(())
            });
            tracing::info!(id, state = item.state.as_str(), "download finished");
        }
        Some(item)
    }

    pub fn list(&self) -> Result<Vec<DownloadItem>> {
        let active = self.active.lock().clone();
        let mut stored: Vec<DownloadItem> = self.storage.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, url, path, file_name, mime_type, total_bytes, received_bytes, state, error, started_at, finished_at
                 FROM downloads ORDER BY started_at DESC LIMIT 500",
            )?;
            let rows = stmt.query_map([], DownloadItem::from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })?;
        // Live state from this session takes precedence over the stored row.
        for item in stored.iter_mut() {
            if let Some(live) = active.iter().find(|a| a.id == item.id) {
                *item = live.clone();
                item.exists = Path::new(&item.path).exists();
            }
        }
        Ok(stored)
    }

    pub fn get(&self, id: i64) -> Result<DownloadItem> {
        self.list()?.into_iter().find(|d| d.id == id).ok_or(Error::NotFound)
    }

    /// Path of a completed download, validated to still exist. Only paths recorded by the download
    /// manager can be opened, so the UI cannot be used to launch arbitrary files.
    pub fn completed_path(&self, id: i64) -> Result<PathBuf> {
        let item = self.get(id)?;
        if item.state != DownloadState::Completed {
            return Err(Error::msg("The download has not finished"));
        }
        let path = PathBuf::from(item.path);
        if !path.exists() {
            return Err(Error::msg("The file was moved or deleted"));
        }
        Ok(path)
    }

    pub fn remove(&self, id: i64) -> Result<()> {
        if self.active.lock().iter().any(|d| d.id == id && d.state.is_active()) {
            return Err(Error::msg("Cancel the download before removing it"));
        }
        self.active.lock().retain(|d| d.id != id);
        self.storage.with(|c| {
            c.execute("DELETE FROM downloads WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    pub fn clear_finished(&self) -> Result<()> {
        self.active.lock().retain(|d| d.state.is_active());
        self.storage.with(|c| {
            c.execute("DELETE FROM downloads WHERE state NOT IN ('inprogress', 'paused')", [])?;
            Ok(())
        })
    }

    pub fn clear_all_history(&self) -> Result<()> {
        self.clear_finished()
    }
}

fn suggested_name(request: &DownloadRequest) -> String {
    let from_engine = request.suggested_path.file_name().map(|n| n.to_string_lossy().into_owned());
    let from_url = url::Url::parse(&request.url).ok().and_then(|u| {
        u.path_segments()?.next_back().filter(|s| !s.is_empty()).map(|s| {
            percent_decode(s)
        })
    });
    sanitize_file_name(&from_engine.or(from_url).unwrap_or_else(|| "download".into()))
}

fn percent_decode(s: &str) -> String {
    url::form_urlencoded::parse(format!("x={s}").as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| s.to_string())
}

/// Returns `dir/name`, or `dir/name (n).ext` with the smallest free `n`.
pub fn unique_path(dir: &Path, name: &str, taken: impl Fn(&Path) -> bool) -> PathBuf {
    let candidate = dir.join(name);
    if !taken(&candidate) {
        return candidate;
    }
    let (stem, ext) = split_extension(name);
    // Keep compound extensions such as ".tar.gz" together.
    let (stem, ext) = match split_extension(stem) {
        (inner, e2) if !e2.is_empty() && e2.len() <= 4 && ext.len() <= 4 && matches!(e2, ".tar") => {
            (inner, format!("{e2}{ext}"))
        }
        _ => (stem, ext.to_string()),
    };
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !taken(p))
        .unwrap_or(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_paths() {
        let dir = Path::new("C:/dl");
        let taken: HashSet<PathBuf> =
            ["C:/dl/a.txt", "C:/dl/a (1).txt", "C:/dl/x.tar.gz", "C:/dl/noext"].iter().map(PathBuf::from).collect();
        let is_taken = |p: &Path| taken.contains(p);
        assert_eq!(unique_path(dir, "b.txt", is_taken), dir.join("b.txt"));
        assert_eq!(unique_path(dir, "a.txt", is_taken), dir.join("a (2).txt"));
        assert_eq!(unique_path(dir, "x.tar.gz", is_taken), dir.join("x (1).tar.gz"));
        assert_eq!(unique_path(dir, "noext", is_taken), dir.join("noext (1)"));
    }

    #[test]
    fn lifecycle() {
        let storage = Arc::new(Storage::open_in_memory().unwrap());
        let dl = DownloadManager::new(storage);
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file.zip"), b"existing").unwrap();
        let request = DownloadRequest {
            url: "https://example.com/files/file.zip".into(),
            suggested_path: PathBuf::from("C:/Users/x/Downloads/file.zip"),
            mime_type: Some("application/zip".into()),
            total_bytes: Some(100),
        };
        let (t1, _) = dl.start(dir.path(), &request).unwrap();
        let (t2, _) = dl.start(dir.path(), &request).unwrap();
        assert_eq!(t1.path, dir.path().join("file (1).zip"), "existing files are never overwritten");
        assert_eq!(t2.path, dir.path().join("file (2).zip"), "concurrent downloads get distinct names");

        dl.update(t1.id, DownloadUpdate::Progress { received: 50, total: Some(100) });
        assert_eq!(dl.update(t1.id, DownloadUpdate::Paused).unwrap().state, DownloadState::Paused);
        assert_eq!(dl.update(t1.id, DownloadUpdate::Resumed).unwrap().state, DownloadState::InProgress);
        let done = dl.update(t1.id, DownloadUpdate::Completed { received: 100 }).unwrap();
        assert_eq!(done.state, DownloadState::Completed);
        let cancelled = dl
            .update(t2.id, DownloadUpdate::Interrupted { reason: "Cancelled".into(), can_resume: false, cancelled: true })
            .unwrap();
        assert_eq!(cancelled.state, DownloadState::Cancelled);

        let list = dl.list().unwrap();
        assert_eq!(list.len(), 2);
        assert!(dl.completed_path(t2.id).is_err());
        dl.clear_finished().unwrap();
        assert!(dl.list().unwrap().is_empty());
    }

    #[test]
    fn names_from_url_are_sanitized() {
        let r = DownloadRequest {
            url: "https://example.com/a/..%5C..%5Cevil.exe".into(),
            suggested_path: PathBuf::new(),
            mime_type: None,
            total_bytes: None,
        };
        assert_eq!(suggested_name(&r), "_.._evil.exe");
    }
}
