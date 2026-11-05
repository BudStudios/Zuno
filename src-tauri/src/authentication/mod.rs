//! Authentication support.
//!
//! Zuno distinguishes three kinds of authentication:
//! * **Website-managed** – sites keep their own sessions in cookies/site storage inside the
//!   persistent WebView2 profile. Zuno never reads or exports those credentials.
//! * **External OAuth authorization** – an authorization URL pasted into the address bar is
//!   analysed and shown to the user before navigating; completion is detected when the provider
//!   redirects to the registered callback, and app callbacks are handed back to the OS only after
//!   confirmation.
//! * **Zuno-managed** – accounts the user explicitly connects in Settings use the authorization
//!   code flow with PKCE; tokens live only in the Windows Credential Manager.

pub mod credentials;
pub mod oauth;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::storage::{now_ms, write_atomic};

/// Non-secret description of a connected account (persisted in `accounts.json`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub issuer: String,
    pub client_id: String,
    pub scopes: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub subject: Option<String>,
    pub connected_at: i64,
    /// Whether a refresh token is held in the Credential Manager.
    pub has_refresh_token: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
    client_secret: Option<String>,
}

pub struct AccountStore {
    path: PathBuf,
    accounts: Mutex<Vec<Account>>,
    pending: Mutex<HashMap<String, oauth::PendingFlow>>,
}

impl AccountStore {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("accounts.json");
        let accounts = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Self { path, accounts: Mutex::new(accounts), pending: Mutex::new(HashMap::new()) }
    }

    pub fn list(&self) -> Vec<Account> {
        self.accounts.lock().clone()
    }

    fn persist(&self, accounts: &[Account]) -> Result<()> {
        write_atomic(&self.path, &serde_json::to_vec_pretty(accounts)?)
    }

    pub fn begin(&self, flow: oauth::PendingFlow) {
        self.pending.lock().insert(flow.account_id.clone(), flow);
    }

    /// Finds the pending flow whose redirect URI matches a navigation.
    pub fn match_callback(&self, url: &url::Url) -> Option<oauth::PendingFlow> {
        let mut pending = self.pending.lock();
        let id = pending.values().find(|f| oauth::matches_redirect(&f.redirect_uri, url))?.account_id.clone();
        pending.remove(&id)
    }

    pub fn complete(&self, flow: &oauth::PendingFlow, tokens: &oauth::TokenResponse, profile: Option<serde_json::Value>) -> Result<Account> {
        let stored = StoredTokens {
            access_token: tokens.access_token.clone(),
            refresh_token: tokens.refresh_token.clone(),
            expires_at: tokens.expires_in.map(|s| now_ms() + s * 1000),
            client_secret: flow.client_secret.clone(),
        };
        credentials::store(&secret_key(&flow.account_id), &serde_json::to_string(&stored)?)?;
        let field = |name: &str| profile.as_ref().and_then(|p| p.get(name)).and_then(|v| v.as_str()).map(str::to_owned);
        let account = Account {
            id: flow.account_id.clone(),
            issuer: flow.metadata.issuer.clone(),
            client_id: flow.client_id.clone(),
            scopes: flow.scopes.clone(),
            display_name: field("name").or_else(|| field("preferred_username")),
            email: field("email"),
            subject: field("sub"),
            connected_at: now_ms(),
            has_refresh_token: tokens.refresh_token.is_some(),
        };
        let mut accounts = self.accounts.lock();
        accounts.retain(|a| a.id != account.id);
        accounts.push(account.clone());
        self.persist(&accounts)?;
        Ok(account)
    }

    /// Removes the account and its secrets. Returns the access token so the caller can attempt
    /// revocation at the provider.
    pub fn remove(&self, id: &str) -> Result<Option<String>> {
        let mut accounts = self.accounts.lock();
        if !accounts.iter().any(|a| a.id == id) {
            return Err(Error::NotFound);
        }
        let token = credentials::load(&secret_key(id))
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<StoredTokens>(&s).ok())
            .map(|t| t.refresh_token.unwrap_or(t.access_token));
        credentials::delete(&secret_key(id));
        accounts.retain(|a| a.id != id);
        self.persist(&accounts)?;
        Ok(token)
    }
}

fn secret_key(id: &str) -> String {
    format!("oauth/{id}")
}

pub fn account_id(issuer: &str, client_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("{issuer}\n{client_id}").as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}
