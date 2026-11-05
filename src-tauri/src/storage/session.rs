use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::write_atomic;
use crate::error::Result;

/// The persisted shape of the open tabs, written whenever the tab layout changes and read on
/// startup when "Continue where you left off" is enabled.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub tabs: Vec<SessionTab>,
    pub groups: Vec<SessionGroup>,
    pub active_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionTab {
    pub url: String,
    pub title: String,
    pub group: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionGroup {
    pub id: u64,
    pub name: String,
    pub color: String,
    pub collapsed: bool,
}

pub struct SessionFile {
    path: PathBuf,
}

impl SessionFile {
    pub fn new(dir: &Path) -> Self {
        Self { path: dir.join("session.json") }
    }

    pub fn load(&self) -> Option<Session> {
        let bytes = std::fs::read(&self.path).ok()?;
        serde_json::from_slice(&bytes)
            .map_err(|e| tracing::warn!(error = %e, "session file unreadable"))
            .ok()
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        write_atomic(&self.path, &serde_json::to_vec(session)?)
    }
}
