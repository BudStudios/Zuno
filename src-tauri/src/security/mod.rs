//! Security policy helpers shared by the browser core.
//!
//! Zuno runs two kinds of webviews in one window:
//! * the privileged `ui` webview, which renders Zuno's own interface from bundled assets, and
//! * unprivileged `tab-N` webviews, which render remote websites.
//!
//! Only the `ui` webview may invoke Zuno commands. Tauri's capability system already refuses IPC
//! from remote origins; every command additionally checks the caller's label (defence in depth).

use crate::error::{Error, Result};

pub const UI_WEBVIEW: &str = "ui";

/// Rejects commands that did not originate from Zuno's own interface webview.
pub fn ensure_ui<R: tauri::Runtime>(webview: &tauri::Webview<R>) -> Result<()> {
    if webview.label() != UI_WEBVIEW {
        tracing::warn!(label = webview.label(), "blocked privileged command from content webview");
        return Err(Error::Forbidden);
    }
    Ok(())
}

/// What should happen when a content webview tries to navigate to a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemePolicy {
    /// Load inside the tab.
    Allow,
    /// Hand off to the operating system after asking the user (mailto:, app callbacks, ...).
    External,
    /// Never navigate (script URLs, privileged internal schemes).
    Block,
}

pub fn scheme_policy(scheme: &str) -> SchemePolicy {
    match scheme.to_ascii_lowercase().as_str() {
        "http" | "https" | "about" | "blob" | "file" => SchemePolicy::Allow,
        // Script-bearing and privileged schemes must never be opened from web content.
        "javascript" | "vbscript" | "data" | "zuno" | "tauri" | "ipc" | "asset" | "view-source" | "edge"
        | "chrome" | "ms-settings" | "ms-msdt" | "search-ms" | "file-ms" | "shell" => SchemePolicy::Block,
        s if s.starts_with("ms-") => SchemePolicy::Block,
        _ => SchemePolicy::External,
    }
}

pub fn is_bookmarkable_scheme(scheme: &str) -> bool {
    matches!(scheme, "http" | "https" | "file" | "zuno")
}

/// Query parameters that commonly carry credentials or authorization artefacts.
const SENSITIVE_PARAMS: &[&str] = &[
    "code",
    "access_token",
    "id_token",
    "refresh_token",
    "token",
    "password",
    "passwd",
    "pwd",
    "client_secret",
    "code_verifier",
    "session_state",
    "oauth_token",
    "oauth_verifier",
    "saml_response",
    "samlresponse",
    "otp",
];

/// Returns the URL in a form safe to store in history, or `None` if it should not be recorded.
/// Credentials embedded in the URL and known authorization parameters are stripped.
pub fn sanitize_for_history(raw: &str) -> Option<String> {
    let mut url = url::Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);

    if url.query().is_some() {
        let kept: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| !SENSITIVE_PARAMS.contains(&k.to_ascii_lowercase().as_str()))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if kept.is_empty() {
            url.set_query(None);
        } else {
            url.query_pairs_mut().clear().extend_pairs(kept);
        }
    }
    if let Some(fragment) = url.fragment() {
        let lower = fragment.to_ascii_lowercase();
        if SENSITIVE_PARAMS.iter().any(|p| lower.contains(&format!("{p}="))) {
            url.set_fragment(None);
        }
    }
    Some(url.to_string())
}

/// Produces a safe file name for a download: no path separators, reserved characters or
/// Windows device names, and a bounded length.
pub fn sanitize_file_name(name: &str) -> String {
    let mut cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    while cleaned.starts_with('.') {
        cleaned.remove(0);
    }
    if cleaned.is_empty() {
        cleaned = "download".into();
    }
    let stem = cleaned.split('.').next().unwrap_or_default().to_ascii_uppercase();
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
        "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.as_str()) {
        cleaned.insert(0, '_');
    }
    if cleaned.chars().count() > 180 {
        let (stem, ext) = split_extension(&cleaned);
        let stem: String = stem.chars().take(180 - ext.chars().count()).collect();
        cleaned = format!("{stem}{ext}");
    }
    cleaned
}

/// Splits `name` into (stem, ".ext"). Names without an extension return an empty extension.
pub fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_policies() {
        assert_eq!(scheme_policy("https"), SchemePolicy::Allow);
        assert_eq!(scheme_policy("JavaScript"), SchemePolicy::Block);
        assert_eq!(scheme_policy("data"), SchemePolicy::Block);
        assert_eq!(scheme_policy("ms-officecmd"), SchemePolicy::Block);
        assert_eq!(scheme_policy("mailto"), SchemePolicy::External);
        assert_eq!(scheme_policy("com.example.app"), SchemePolicy::External);
    }

    #[test]
    fn history_sanitizer_strips_secrets() {
        assert_eq!(
            sanitize_for_history("https://user:pw@example.com/cb?code=abc&state=xyz&lang=en").as_deref(),
            Some("https://example.com/cb?state=xyz&lang=en")
        );
        assert_eq!(
            sanitize_for_history("https://example.com/cb#access_token=abc&token_type=bearer").as_deref(),
            Some("https://example.com/cb")
        );
        assert_eq!(sanitize_for_history("https://example.com/p?code=1").as_deref(), Some("https://example.com/p"));
        assert_eq!(sanitize_for_history("https://example.com/#section").as_deref(), Some("https://example.com/#section"));
        assert_eq!(sanitize_for_history("file:///C:/secret.txt"), None);
        assert_eq!(sanitize_for_history("about:blank"), None);
    }

    #[test]
    fn file_name_sanitizer() {
        assert_eq!(sanitize_file_name("..\\..\\evil.exe"), "_.._evil.exe");
        assert_eq!(sanitize_file_name("a/b:c?.txt"), "a_b_c_.txt");
        assert_eq!(sanitize_file_name("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_file_name("   "), "download");
        assert_eq!(sanitize_file_name("report. "), "report");
        let long = format!("{}.pdf", "x".repeat(400));
        let s = sanitize_file_name(&long);
        assert!(s.ends_with(".pdf") && s.chars().count() == 180);
    }
}
