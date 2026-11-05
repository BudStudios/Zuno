//! Address bar input resolution: decides whether typed text is a URL, an internal page or a
//! search query, and builds search URLs for the configured provider.

use serde::Serialize;
use url::Url;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchEngine {
    pub id: &'static str,
    pub name: &'static str,
    pub template: &'static str,
}

pub const SEARCH_ENGINES: &[SearchEngine] = &[
    SearchEngine { id: "duckduckgo", name: "DuckDuckGo", template: "https://duckduckgo.com/?q=%s" },
    SearchEngine { id: "google", name: "Google", template: "https://www.google.com/search?q=%s" },
    SearchEngine { id: "bing", name: "Bing", template: "https://www.bing.com/search?q=%s" },
    SearchEngine { id: "brave", name: "Brave Search", template: "https://search.brave.com/search?q=%s" },
    SearchEngine { id: "startpage", name: "Startpage", template: "https://www.startpage.com/do/search?q=%s" },
    SearchEngine { id: "ecosia", name: "Ecosia", template: "https://www.ecosia.org/search?q=%s" },
];

pub fn find_engine(id: &str) -> Option<&'static SearchEngine> {
    SEARCH_ENGINES.iter().find(|e| e.id == id)
}

pub fn validate_search_template(template: &str) -> Result<()> {
    if !template.contains("%s") {
        return Err(Error::msg("The custom search address must contain %s where the query goes"));
    }
    let probe = Url::parse(&template.replace("%s", "test"))
        .map_err(|_| Error::msg("The custom search address is not a valid URL"))?;
    if probe.scheme() != "https" {
        return Err(Error::msg("The custom search address must use https://"));
    }
    Ok(())
}

pub fn search_url(template: &str, query: &str) -> Result<Url> {
    let encoded: String = url::form_urlencoded::byte_serialize(query.trim().as_bytes()).collect();
    Ok(Url::parse(&template.replace("%s", &encoded))?)
}

/// Pages rendered by Zuno itself rather than by a website.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalPage {
    NewTab,
    Settings,
    History,
    Bookmarks,
    Downloads,
    Import,
}

impl InternalPage {
    pub fn parse(url: &str) -> Option<Self> {
        let rest = url.strip_prefix("zuno://")?;
        let page = rest.split(['/', '?', '#']).next().unwrap_or_default().to_ascii_lowercase();
        Some(match page.as_str() {
            "newtab" | "" => Self::NewTab,
            "settings" => Self::Settings,
            "history" => Self::History,
            "bookmarks" => Self::Bookmarks,
            "downloads" => Self::Downloads,
            "import" => Self::Import,
            _ => return None,
        })
    }

    pub fn url(self) -> &'static str {
        match self {
            Self::NewTab => "zuno://newtab",
            Self::Settings => "zuno://settings",
            Self::History => "zuno://history",
            Self::Bookmarks => "zuno://bookmarks",
            Self::Downloads => "zuno://downloads",
            Self::Import => "zuno://import",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::NewTab => "New Tab",
            Self::Settings => "Settings",
            Self::History => "History",
            Self::Bookmarks => "Bookmarks",
            Self::Downloads => "Downloads",
            Self::Import => "Import browser data",
        }
    }
}

pub fn is_internal(url: &str) -> bool {
    url.starts_with("zuno://")
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resolved {
    Internal(InternalPage),
    Web(Url),
    Search(Url),
}

impl Resolved {
    pub fn url_string(&self) -> String {
        match self {
            Resolved::Internal(p) => p.url().to_string(),
            Resolved::Web(u) | Resolved::Search(u) => u.to_string(),
        }
    }
}

/// Country-code TLDs (ISO 3166-1 based, as delegated in the root zone).
const CC_TLDS: &str = "ac ad ae af ag ai al am ao aq ar as at au aw ax az ba bb bd be bf bg bh bi bj bm bn bo br bs bt bw by bz ca cc cd cf cg ch ci ck cl cm cn co cr cu cv cw cx cy cz de dj dk dm do dz ec ee eg er es et eu fi fj fk fm fo fr ga gd ge gf gg gh gi gl gm gn gp gq gr gs gt gu gw gy hk hm hn hr ht hu id ie il im in io iq ir is it je jm jo jp ke kg kh ki km kn kp kr kw ky kz la lb lc li lk lr ls lt lu lv ly ma mc md me mg mh mk ml mm mn mo mp mq mr ms mt mu mv mw mx my mz na nc ne nf ng ni nl no np nr nu nz om pa pe pf pg ph pk pl pm pn pr ps pt pw py qa re ro rs ru rw sa sb sc sd se sg sh si sk sl sm sn so sr ss st su sv sx sy sz tc td tf tg th tj tk tl tm tn to tr tt tv tw tz ua ug uk us uy uz va vc ve vg vi vn vu wf ws ye yt za zm zw";

/// Generic TLDs accepted for scheme-less input, in addition to country codes. Anything else is
/// treated as a search, e.g. `node.js`.
const COMMON_TLDS: &[&str] = &[
    "com", "org", "net", "edu", "gov", "mil", "int", "info", "biz", "dev", "app", "page", "io", "ai", "xyz", "site",
    "online", "tech", "store", "shop", "blog", "cloud", "news", "wiki", "media", "live", "tv", "design", "art", "studio",
    "agency", "email", "today", "world", "space", "website", "link", "click", "top", "club", "pro", "mobi", "name",
    "travel", "museum", "aero", "coop", "jobs", "tel", "codes", "software", "systems", "network", "digital", "social",
    "games", "game", "chat", "zone", "works", "tools", "run", "build", "new", "google", "microsoft", "apple", "amazon",
    "local", "localhost", "test", "example", "onion", "lan", "home", "internal", "academy", "school", "university",
    "health", "finance", "bank", "money", "law", "legal", "photo", "photography", "music", "video", "movie", "film",
    "rocks", "ninja", "guru", "expert", "solutions", "services", "company", "center", "group", "inc", "ltd", "llc",
];

fn looks_like_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if host.parse::<std::net::Ipv4Addr>().is_ok() || (host.starts_with('[') && host.ends_with(']')) {
        return true;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 || labels.iter().any(|l| l.is_empty() || l.len() > 63) {
        return false;
    }
    if !labels.iter().all(|l| l.chars().all(|c| c.is_alphanumeric() || c == '-') && !l.starts_with('-')) {
        return false;
    }
    let tld = labels.last().copied().unwrap_or_default().to_ascii_lowercase();
    tld.starts_with("xn--")
        || (tld.len() == 2 && CC_TLDS.split(' ').any(|cc| cc == tld))
        || COMMON_TLDS.contains(&tld.as_str())
}

fn is_local_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<std::net::Ipv4Addr>().is_ok()
        || host.starts_with('[')
        || host.ends_with(".local")
        || host.ends_with(".localhost")
}

/// Resolves address-bar input. `search_template` must contain `%s`.
pub fn resolve_input(input: &str, search_template: &str) -> Result<Resolved> {
    let text = input.trim();
    if text.is_empty() {
        return Err(Error::msg("Type an address or search"));
    }
    if is_internal(&text.to_ascii_lowercase()) {
        return InternalPage::parse(&text.to_ascii_lowercase())
            .map(Resolved::Internal)
            .ok_or_else(|| Error::msg("Unknown Zuno page"));
    }
    if text.starts_with('?') {
        return Ok(Resolved::Search(search_url(search_template, &text[1..])?));
    }
    let has_space = text.chars().any(char::is_whitespace);

    // Explicit scheme: "https://...", "about:blank", "mailto:...". Bare "host:port" input such as
    // "localhost:3000" also parses as a URL (scheme "localhost"), so only accept "://" forms and a
    // short list of well-known opaque schemes here.
    if !has_space {
        if let Ok(url) = Url::parse(text) {
            if text.contains("://") || matches!(url.scheme(), "about" | "mailto" | "tel" | "sms" | "magnet") {
                return Ok(Resolved::Web(url));
            }
        }

        // Scheme-less address such as "example.com/path" or "localhost:3000".
        let authority = text.split(['/', '?', '#']).next().unwrap_or_default();
        let host = match authority.rsplit_once(':') {
            Some((h, port)) if !h.is_empty() && port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => h,
            _ => authority,
        };
        if looks_like_host(host) {
            let scheme = if is_local_host(host) { "http" } else { "https" };
            if let Ok(url) = Url::parse(&format!("{scheme}://{text}")) {
                return Ok(Resolved::Web(url));
            }
        }
    }
    Ok(Resolved::Search(search_url(search_template, text)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DDG: &str = "https://duckduckgo.com/?q=%s";

    fn web(s: &str) -> String {
        match resolve_input(s, DDG).unwrap() {
            Resolved::Web(u) => u.to_string(),
            other => panic!("{s} resolved to {other:?}"),
        }
    }

    fn is_search(s: &str) -> bool {
        matches!(resolve_input(s, DDG).unwrap(), Resolved::Search(_))
    }

    #[test]
    fn urls() {
        assert_eq!(web("https://example.com"), "https://example.com/");
        assert_eq!(web("example.com"), "https://example.com/");
        assert_eq!(web("sub.example.co.uk/path?q=1"), "https://sub.example.co.uk/path?q=1");
        assert_eq!(web("localhost:3000"), "http://localhost:3000/");
        assert_eq!(web("localhost"), "http://localhost/");
        assert_eq!(web("127.0.0.1:8080/x"), "http://127.0.0.1:8080/x");
        assert_eq!(web("example.com:8443/a"), "https://example.com:8443/a");
        assert_eq!(web("about:blank"), "about:blank");
        assert_eq!(web("mailto:hi@example.com"), "mailto:hi@example.com");
        assert_eq!(web("xn--bcher-kva.example"), "https://xn--bcher-kva.example/");
    }

    #[test]
    fn searches() {
        assert!(is_search("rust tauri webview"));
        assert!(is_search("hello"));
        assert!(is_search("node.js"));
        assert!(is_search("what is example.com"));
        assert!(is_search("?example.com"));
        match resolve_input("a & b", DDG).unwrap() {
            Resolved::Search(u) => assert_eq!(u.as_str(), "https://duckduckgo.com/?q=a+%26+b"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn internal_pages() {
        assert_eq!(resolve_input("zuno://settings", DDG).unwrap(), Resolved::Internal(InternalPage::Settings));
        assert_eq!(resolve_input("ZUNO://History", DDG).unwrap(), Resolved::Internal(InternalPage::History));
        assert!(resolve_input("zuno://unknown", DDG).is_err());
        assert!(resolve_input("   ", DDG).is_err());
    }

    #[test]
    fn search_templates() {
        assert!(validate_search_template("https://s.example/?q=%s").is_ok());
        assert!(validate_search_template("https://s.example/?q=").is_err());
        assert!(validate_search_template("http://s.example/?q=%s").is_err());
        assert!(SEARCH_ENGINES.iter().all(|e| validate_search_template(e.template).is_ok()));
    }
}
