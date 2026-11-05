use std::collections::HashMap;

use rusqlite::params;

use super::{now_ms, Storage};
use crate::error::Result;

/// Favicons are cached per host as small PNG data URLs so bookmarks, history and the quick
/// switcher can show them without the UI fetching anything from the network.
impl Storage {
    pub fn favicon_store(&self, host: &str, data_url: &str) -> Result<()> {
        if host.is_empty() || data_url.len() > 256 * 1024 {
            return Ok(());
        }
        self.with(|c| {
            c.execute(
                "INSERT INTO favicons (host, data, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(host) DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
                params![host, data_url, now_ms()],
            )?;
            Ok(())
        })
    }

    pub fn favicons_for_hosts(&self, hosts: &[String]) -> Result<HashMap<String, String>> {
        let mut out = HashMap::new();
        if hosts.is_empty() {
            return Ok(out);
        }
        self.with(|c| {
            let mut stmt = c.prepare_cached("SELECT data FROM favicons WHERE host = ?1")?;
            for host in hosts {
                if out.contains_key(host) {
                    continue;
                }
                if let Ok(data) = stmt.query_row([host], |r| r.get::<_, String>(0)) {
                    out.insert(host.clone(), data);
                }
            }
            Ok(out)
        })
    }

    pub fn favicons_clear(&self) -> Result<()> {
        self.with(|c| {
            c.execute("DELETE FROM favicons", [])?;
            Ok(())
        })
    }
}

pub fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url).ok()?.host_str().map(str::to_owned)
}
