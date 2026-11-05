//! OAuth 2.0 / OpenID Connect support.
//!
//! Two separate concerns live here:
//!
//! 1. **Authorization-request analysis.** When a user pastes an authorization URL, Zuno parses
//!    it (RFC 6749 §4.1/§4.2, OIDC Core §3.1.2.1) so the user can see which application is asking,
//!    which provider will authenticate them, where the result will be delivered, and whether the
//!    request is protected by PKCE (RFC 7636) and `state`. Zuno never completes or approves
//!    anything on the user's behalf: the provider's own consent screens are always shown.
//!
//! 2. **Zuno-managed sign-in** (authorization code + PKCE for native apps, RFC 8252) for
//!    accounts the user explicitly connects in Settings. Tokens are stored only in the Windows
//!    Credential Manager (see [`super::credentials`]).

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RedirectKind {
    /// https:// redirect to a website.
    Web,
    /// http://127.0.0.1 / localhost — a desktop application listening locally (RFC 8252 §7.3).
    Loopback,
    /// Private-use URI scheme such as `com.example.app:/callback` (RFC 8252 §7.1).
    App,
    /// Out-of-band or missing redirect.
    None,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationRequest {
    pub url: String,
    pub provider_host: String,
    /// Display name for well-known identity providers.
    pub provider_name: Option<String>,
    pub client_id: String,
    pub response_type: String,
    pub redirect_uri: Option<String>,
    pub redirect_kind: RedirectKind,
    pub redirect_target: Option<String>,
    pub scopes: Vec<String>,
    pub is_openid: bool,
    pub has_pkce: bool,
    pub pkce_method: Option<String>,
    pub has_state: bool,
    pub warnings: Vec<String>,
}

const KNOWN_PROVIDERS: &[(&str, &str)] = &[
    ("accounts.google.com", "Google"),
    ("login.microsoftonline.com", "Microsoft"),
    ("login.live.com", "Microsoft"),
    ("github.com", "GitHub"),
    ("gitlab.com", "GitLab"),
    ("appleid.apple.com", "Apple"),
    ("discord.com", "Discord"),
    ("slack.com", "Slack"),
    ("www.facebook.com", "Facebook"),
    ("x.com", "X"),
    ("twitter.com", "X"),
    ("api.twitter.com", "X"),
    ("www.linkedin.com", "LinkedIn"),
    ("id.atlassian.com", "Atlassian"),
    ("auth.atlassian.com", "Atlassian"),
    ("www.dropbox.com", "Dropbox"),
    ("account.spotify.com", "Spotify"),
    ("accounts.spotify.com", "Spotify"),
    ("id.twitch.tv", "Twitch"),
    ("www.reddit.com", "Reddit"),
    ("login.salesforce.com", "Salesforce"),
];

fn provider_name(host: &str) -> Option<String> {
    if let Some((_, name)) = KNOWN_PROVIDERS.iter().find(|(h, _)| *h == host) {
        return Some((*name).to_string());
    }
    for (suffix, name) in [(".okta.com", "Okta"), (".auth0.com", "Auth0"), (".b2clogin.com", "Microsoft Entra"), (".amazoncognito.com", "Amazon Cognito")] {
        if host.ends_with(suffix) {
            return Some(name.to_string());
        }
    }
    None
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

/// Recognises an OAuth 2.0 / OIDC authorization request. Returns `None` for ordinary URLs.
pub fn analyze(raw: &str) -> Option<AuthorizationRequest> {
    let url = Url::parse(raw.trim()).ok()?;
    if url.scheme() != "https" {
        return None;
    }
    let get = |name: &str| url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned());
    let client_id = get("client_id")?;
    // GitHub's web flow omits response_type (it only supports "code").
    let response_type = get("response_type").or_else(|| {
        (url.host_str() == Some("github.com") && url.path().contains("/login/oauth/authorize")).then(|| "code".to_string())
    })?;
    if client_id.is_empty() || response_type.is_empty() {
        return None;
    }
    let host = url.host_str()?.to_string();
    let scopes: Vec<String> = get("scope").map(|s| s.split([' ', ',', '+']).filter(|x| !x.is_empty()).map(str::to_owned).collect()).unwrap_or_default();
    let redirect_uri = get("redirect_uri");
    let parsed_redirect = redirect_uri.as_deref().and_then(|r| Url::parse(r).ok());
    let (redirect_kind, redirect_target) = match &parsed_redirect {
        None => (RedirectKind::None, None),
        Some(r) if r.scheme() == "https" => (RedirectKind::Web, r.host_str().map(str::to_owned)),
        Some(r) if r.scheme() == "http" && is_loopback(r) => (RedirectKind::Loopback, Some(format!("{}:{}", r.host_str().unwrap_or_default(), r.port_or_known_default().unwrap_or(80)))),
        Some(r) if r.scheme() == "http" => (RedirectKind::Web, r.host_str().map(str::to_owned)),
        Some(r) => (RedirectKind::App, Some(r.scheme().to_string())),
    };
    let pkce_method = get("code_challenge").map(|_| get("code_challenge_method").unwrap_or_else(|| "plain".into()));
    let has_state = get("state").is_some_and(|s| !s.is_empty());
    let uses_code = response_type.split(' ').any(|t| t == "code");

    let mut warnings = Vec::new();
    if response_type.split(' ').any(|t| t == "token") {
        warnings.push("Uses the implicit flow, which exposes tokens in the address. Modern apps should use the authorization code flow with PKCE.".into());
    }
    if uses_code && pkce_method.is_none() && redirect_kind != RedirectKind::Web {
        warnings.push("This app sign-in does not use PKCE, so an intercepted code could be misused.".into());
    }
    if pkce_method.as_deref() == Some("plain") {
        warnings.push("PKCE uses the weaker \"plain\" method instead of S256.".into());
    }
    if !has_state && !scopes.iter().any(|s| s == "openid") {
        warnings.push("No state parameter: the request is not protected against cross-site forgery.".into());
    }
    if parsed_redirect.as_ref().is_some_and(|r| r.scheme() == "http" && !is_loopback(r)) {
        warnings.push("The result is sent over an unencrypted http:// connection.".into());
    }

    Some(AuthorizationRequest {
        url: url.to_string(),
        provider_name: provider_name(&host),
        provider_host: host,
        client_id,
        is_openid: scopes.iter().any(|s| s == "openid"),
        scopes,
        response_type,
        redirect_uri,
        redirect_kind,
        redirect_target,
        has_pkce: pkce_method.is_some(),
        pkce_method,
        has_state,
        warnings,
    })
}

/// True when `candidate` is a navigation to `redirect_uri` (same scheme, host, port and path).
pub fn matches_redirect(redirect_uri: &str, candidate: &Url) -> bool {
    let Ok(expected) = Url::parse(redirect_uri) else { return false };
    expected.scheme() == candidate.scheme()
        && expected.host_str() == candidate.host_str()
        && expected.port_or_known_default() == candidate.port_or_known_default()
        && expected.path().trim_end_matches('/') == candidate.path().trim_end_matches('/')
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CallbackOutcome {
    /// An authorization code or token was issued.
    Authorized,
    /// The user or provider declined (`error=access_denied` etc.).
    Denied { error: String, description: Option<String> },
    /// The redirect carried neither a result nor an error.
    Unknown,
}

/// Interprets an authorization response delivered to the redirect URI (query or fragment).
/// Only parameter *names* are inspected, never logged values.
pub fn callback_outcome(url: &Url) -> CallbackOutcome {
    let mut pairs: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    if let Some(fragment) = url.fragment() {
        pairs.extend(url::form_urlencoded::parse(fragment.as_bytes()).map(|(k, v)| (k.into_owned(), v.into_owned())));
    }
    let get = |n: &str| pairs.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    if let Some(error) = get("error") {
        return CallbackOutcome::Denied { error, description: get("error_description") };
    }
    if get("code").is_some() || get("access_token").is_some() || get("id_token").is_some() {
        return CallbackOutcome::Authorized;
    }
    CallbackOutcome::Unknown
}

// ---------------------------------------------------------------------------------------------
// PKCE and the Zuno-managed authorization code flow
// ---------------------------------------------------------------------------------------------

fn random_token(bytes: usize) -> Result<String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| Error::msg(format!("Secure random generator unavailable: {e}")))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    /// Generates a 43-character verifier (32 random bytes) and its S256 challenge.
    pub fn generate() -> Result<Self> {
        let verifier = random_token(32)?;
        Ok(Self { challenge: s256(&verifier), verifier })
    }
}

pub fn s256(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub userinfo_endpoint: Option<String>,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
    #[serde(default)]
    pub code_challenge_methods_supported: Vec<String>,
}

/// An in-flight Zuno-managed sign-in, held in memory only until the redirect arrives.
#[derive(Debug, Clone)]
pub struct PendingFlow {
    pub account_id: String,
    pub metadata: ProviderMetadata,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub state: String,
    pub pkce: Pkce,
    pub nonce: String,
    pub scopes: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<i64>,
}

pub async fn discover(http: &reqwest::Client, issuer: &str) -> Result<ProviderMetadata> {
    let issuer = Url::parse(issuer.trim()).map_err(|_| Error::msg("Enter the provider's issuer URL, e.g. https://accounts.google.com"))?;
    if issuer.scheme() != "https" {
        return Err(Error::msg("The issuer must use https://"));
    }
    let base = issuer.as_str().trim_end_matches('/');
    let doc_url = format!("{base}/.well-known/openid-configuration");
    let metadata: ProviderMetadata = http.get(&doc_url).send().await?.error_for_status()?.json().await?;
    for endpoint in [&metadata.authorization_endpoint, &metadata.token_endpoint] {
        if Url::parse(endpoint).map(|u| u.scheme() != "https").unwrap_or(true) {
            return Err(Error::msg("The provider returned an insecure endpoint"));
        }
    }
    if !metadata.code_challenge_methods_supported.is_empty() && !metadata.code_challenge_methods_supported.iter().any(|m| m == "S256") {
        return Err(Error::msg("The provider does not support PKCE (S256)"));
    }
    Ok(metadata)
}

impl PendingFlow {
    pub fn new(
        account_id: String,
        metadata: ProviderMetadata,
        client_id: String,
        client_secret: Option<String>,
        scopes: String,
    ) -> Result<Self> {
        // RFC 8252 §7.3: loopback redirect with an ephemeral port. Zuno intercepts the navigation
        // inside the sign-in tab, so nothing needs to listen on the port.
        let mut port_bytes = [0u8; 2];
        getrandom::fill(&mut port_bytes).map_err(|e| Error::msg(e.to_string()))?;
        let port = 49152 + (u16::from_le_bytes(port_bytes) % 16000);
        Ok(Self {
            account_id,
            metadata,
            client_id,
            client_secret,
            redirect_uri: format!("http://127.0.0.1:{port}/zuno/oauth/callback"),
            state: random_token(24)?,
            pkce: Pkce::generate()?,
            nonce: random_token(24)?,
            scopes,
        })
    }

    pub fn authorization_url(&self) -> Result<Url> {
        let mut url = Url::parse(&self.metadata.authorization_endpoint)?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", &self.scopes)
            .append_pair("state", &self.state)
            .append_pair("nonce", &self.nonce)
            .append_pair("code_challenge", &self.pkce.challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(url)
    }

    /// Validates the redirect and extracts the authorization code.
    pub fn code_from_callback(&self, callback: &Url) -> Result<String> {
        let get = |n: &str| callback.query_pairs().find(|(k, _)| k == n).map(|(_, v)| v.into_owned());
        if let Some(err) = get("error") {
            return Err(Error::msg(match err.as_str() {
                "access_denied" => "Sign-in was cancelled".to_string(),
                other => format!("The provider returned an error: {other}"),
            }));
        }
        if get("state").as_deref() != Some(self.state.as_str()) {
            return Err(Error::msg("The sign-in response did not match this request (state mismatch)"));
        }
        if let Some(iss) = get("iss") {
            if iss.trim_end_matches('/') != self.metadata.issuer.trim_end_matches('/') {
                return Err(Error::msg("The sign-in response came from an unexpected issuer"));
            }
        }
        get("code").filter(|c| !c.is_empty()).ok_or_else(|| Error::msg("The provider did not return an authorization code"))
    }

    pub async fn exchange(&self, http: &reqwest::Client, code: &str) -> Result<TokenResponse> {
        let mut form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", code.to_string()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("client_id", self.client_id.clone()),
            ("code_verifier", self.pkce.verifier.clone()),
        ];
        if let Some(secret) = &self.client_secret {
            form.push(("client_secret", secret.clone()));
        }
        let response = http.post(&self.metadata.token_endpoint).form(&form).send().await?;
        if !response.status().is_success() {
            #[derive(Deserialize)]
            struct OAuthError {
                error: String,
                #[serde(default)]
                error_description: Option<String>,
            }
            let status = response.status();
            let detail = response.json::<OAuthError>().await.ok();
            return Err(Error::msg(match detail {
                Some(e) => format!("Token request rejected: {}{}", e.error, e.error_description.map(|d| format!(" ({d})")).unwrap_or_default()),
                None => format!("Token request failed with HTTP {status}"),
            }));
        }
        Ok(response.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_rfc7636_appendix_b() {
        assert_eq!(s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let p = Pkce::generate().unwrap();
        assert_eq!(p.verifier.len(), 43);
        assert_eq!(p.challenge, s256(&p.verifier));
    }

    #[test]
    fn analyzes_google_pkce_request() {
        let req = analyze(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id=abc.apps.googleusercontent.com&response_type=code\
             &redirect_uri=http%3A%2F%2F127.0.0.1%3A53682%2F&scope=openid%20email&state=s1\
             &code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256",
        )
        .unwrap();
        assert_eq!(req.provider_name.as_deref(), Some("Google"));
        assert_eq!(req.redirect_kind, RedirectKind::Loopback);
        assert_eq!(req.redirect_target.as_deref(), Some("127.0.0.1:53682"));
        assert!(req.is_openid && req.has_pkce && req.has_state);
        assert_eq!(req.pkce_method.as_deref(), Some("S256"));
        assert!(req.warnings.is_empty());
    }

    #[test]
    fn analyzes_app_redirect_without_pkce_and_implicit_flow() {
        let req = analyze("https://login.example.com/authorize?client_id=x&response_type=token&redirect_uri=com.example.app%3A%2Fcb").unwrap();
        assert_eq!(req.redirect_kind, RedirectKind::App);
        assert_eq!(req.redirect_target.as_deref(), Some("com.example.app"));
        assert!(req.warnings.iter().any(|w| w.contains("implicit")));
        assert!(req.warnings.iter().any(|w| w.contains("state")));

        let gh = analyze("https://github.com/login/oauth/authorize?client_id=Iv1.x&redirect_uri=https%3A%2F%2Fapp.example%2Fcb&state=z").unwrap();
        assert_eq!(gh.provider_name.as_deref(), Some("GitHub"));
        assert_eq!(gh.redirect_kind, RedirectKind::Web);
    }

    #[test]
    fn ignores_ordinary_urls() {
        assert!(analyze("https://example.com/?q=client_id").is_none());
        assert!(analyze("https://example.com/search?client_id=1").is_none());
        assert!(analyze("http://insecure.example/authorize?client_id=1&response_type=code").is_none());
        assert!(analyze("not a url").is_none());
    }

    #[test]
    fn redirect_matching_and_outcomes() {
        let cb = Url::parse("https://app.example/cb/?code=1&state=2").unwrap();
        assert!(matches_redirect("https://app.example/cb", &cb));
        assert!(!matches_redirect("https://app.example/other", &cb));
        assert!(!matches_redirect("https://evil.example/cb", &cb));
        assert_eq!(callback_outcome(&cb), CallbackOutcome::Authorized);
        let denied = Url::parse("https://app.example/cb?error=access_denied").unwrap();
        assert!(matches!(callback_outcome(&denied), CallbackOutcome::Denied { .. }));
        let implicit = Url::parse("https://app.example/cb#access_token=x").unwrap();
        assert_eq!(callback_outcome(&implicit), CallbackOutcome::Authorized);
    }

    #[test]
    fn managed_flow_validates_state_and_issuer() {
        let meta = ProviderMetadata {
            issuer: "https://id.example".into(),
            authorization_endpoint: "https://id.example/authorize".into(),
            token_endpoint: "https://id.example/token".into(),
            userinfo_endpoint: None,
            revocation_endpoint: None,
            code_challenge_methods_supported: vec!["S256".into()],
        };
        let flow = PendingFlow::new("acc".into(), meta, "client".into(), None, "openid email".into()).unwrap();
        let auth = flow.authorization_url().unwrap();
        let req = analyze(auth.as_str()).unwrap();
        assert!(req.has_pkce && req.has_state && req.warnings.is_empty());
        assert_eq!(req.redirect_kind, RedirectKind::Loopback);

        let ok = Url::parse(&format!("{}?code=abc&state={}&iss=https%3A%2F%2Fid.example", flow.redirect_uri, flow.state)).unwrap();
        assert_eq!(flow.code_from_callback(&ok).unwrap(), "abc");
        let forged = Url::parse(&format!("{}?code=abc&state=forged", flow.redirect_uri)).unwrap();
        assert!(flow.code_from_callback(&forged).is_err());
        let wrong_iss = Url::parse(&format!("{}?code=abc&state={}&iss=https%3A%2F%2Fevil", flow.redirect_uri, flow.state)).unwrap();
        assert!(flow.code_from_callback(&wrong_iss).is_err());
        let denied = Url::parse(&format!("{}?error=access_denied&state={}", flow.redirect_uri, flow.state)).unwrap();
        assert!(flow.code_from_callback(&denied).unwrap_err().to_string().contains("cancelled"));
    }
}
