use std::path::{Path, PathBuf};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::write_atomic;
use crate::browser::navigation;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Startup {
    NewTab,
    #[default]
    Restore,
    Homepage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TrackingPrevention {
    None,
    Basic,
    #[default]
    Balanced,
    Strict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    pub search_engine: String,
    /// Template containing `%s`, used when `search_engine == "custom"`.
    pub custom_search_url: String,
    pub startup: Startup,
    pub homepage: String,
    pub show_bookmarks_bar: bool,
    /// `None` means the user's Downloads folder.
    pub download_dir: Option<String>,
    pub save_history: bool,
    pub clear_history_on_exit: bool,
    pub tracking_prevention: TrackingPrevention,
    pub password_autosave: bool,
    pub general_autofill: bool,
    /// Give active network adapters a random MAC address while incognito windows are open.
    pub incognito_randomize_mac: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            search_engine: "duckduckgo".into(),
            custom_search_url: String::new(),
            startup: Startup::Restore,
            homepage: "zuno://newtab".into(),
            show_bookmarks_bar: false,
            download_dir: None,
            save_history: true,
            clear_history_on_exit: false,
            tracking_prevention: TrackingPrevention::Balanced,
            password_autosave: true,
            general_autofill: true,
            incognito_randomize_mac: true,
        }
    }
}

impl Settings {
    /// Normalises and validates user-supplied values before they are persisted.
    pub fn validated(mut self) -> Result<Self> {
        if self.search_engine == "custom" {
            let t = self.custom_search_url.trim().to_string();
            navigation::validate_search_template(&t)?;
            self.custom_search_url = t;
        } else if navigation::find_engine(&self.search_engine).is_none() {
            return Err(Error::msg("Unknown search engine"));
        }
        let home = self.homepage.trim();
        self.homepage = if home.is_empty() {
            "zuno://newtab".into()
        } else {
            let parsed = url::Url::parse(home).map_err(|_| Error::msg("The homepage must be a full address, e.g. https://example.com"))?;
            if !matches!(parsed.scheme(), "http" | "https" | "zuno") {
                return Err(Error::msg("The homepage must be a web address"));
            }
            parsed.to_string()
        };
        if let Some(dir) = &self.download_dir {
            if dir.trim().is_empty() {
                self.download_dir = None;
            } else if !Path::new(dir).is_dir() {
                return Err(Error::msg("The download folder does not exist"));
            }
        }
        Ok(self)
    }
}

pub struct SettingsStore {
    /// `None` when changes must never be written (incognito windows).
    path: Option<PathBuf>,
    current: RwLock<Settings>,
}

impl SettingsStore {
    pub fn load(dir: &Path) -> Self {
        let mut store = Self::load_read_only(dir);
        store.path = Some(dir.join("settings.json"));
        store
    }

    /// Reads the settings but never writes them back: changes last until the process exits.
    pub fn load_read_only(dir: &Path) -> Self {
        let path = dir.join("settings.json");
        let current = std::fs::read(&path)
            .ok()
            .and_then(|bytes| match serde_json::from_slice::<Settings>(&bytes) {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!(error = %e, "settings file unreadable, using defaults");
                    None
                }
            })
            .unwrap_or_default();
        Self { path: None, current: RwLock::new(current) }
    }

    pub fn get(&self) -> Settings {
        self.current.read().clone()
    }

    pub fn set(&self, next: Settings) -> Result<Settings> {
        let next = next.validated()?;
        if let Some(path) = &self.path {
            write_atomic(path, &serde_json::to_vec_pretty(&next)?)?;
        }
        *self.current.write() = next.clone();
        Ok(next)
    }

    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> Result<Settings> {
        let mut next = self.get();
        f(&mut next);
        self.set(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(dir.path());
        assert_eq!(store.get().search_engine, "duckduckgo");
        store.update(|s| s.theme = Theme::Light).unwrap();
        let reloaded = SettingsStore::load(dir.path());
        assert_eq!(reloaded.get().theme, Theme::Light);
    }

    #[test]
    fn read_only_store_never_writes() {
        let dir = tempfile::tempdir().unwrap();
        SettingsStore::load(dir.path()).update(|s| s.theme = Theme::Dark).unwrap();
        let before = std::fs::read(dir.path().join("settings.json")).unwrap();
        let store = SettingsStore::load_read_only(dir.path());
        assert_eq!(store.get().theme, Theme::Dark);
        assert_eq!(store.update(|s| s.theme = Theme::Light).unwrap().theme, Theme::Light);
        assert_eq!(store.get().theme, Theme::Light, "changes apply for the session");
        assert_eq!(std::fs::read(dir.path().join("settings.json")).unwrap(), before);
    }

    #[test]
    fn partial_files_fill_defaults() {
        let s: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(s.theme, Theme::Dark);
        assert!(s.save_history);
    }

    #[test]
    fn validation() {
        let bad_engine = Settings { search_engine: "nope".into(), ..Default::default() };
        assert!(bad_engine.validated().is_err());
        let custom = Settings {
            search_engine: "custom".into(),
            custom_search_url: "https://search.example/?q=%s".into(),
            ..Default::default()
        };
        assert!(custom.validated().is_ok());
        let custom_bad = Settings {
            search_engine: "custom".into(),
            custom_search_url: "http://search.example/?q=".into(),
            ..Default::default()
        };
        assert!(custom_bad.validated().is_err());
        let home = Settings { homepage: "javascript:alert(1)".into(), ..Default::default() };
        assert!(home.validated().is_err());
    }
}
