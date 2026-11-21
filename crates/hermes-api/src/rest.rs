//! REST client for `/api/*` and `/auth/native/{token,refresh}`.

use crate::redact::Secret;
use crate::types::*;
use crate::url_util;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use url::Url;

/// Errors a caller can show to the user. Messages never contain tokens.
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum ApiError {
    /// The server cannot be reached (DNS, TCP, TLS, timeout).
    #[error("Cannot reach the gateway: {0}")]
    Offline(String),
    /// 401: the access token is missing, invalid or expired.
    #[error("Your session has expired. Sign in again.")]
    Unauthorized { session_expired: bool },
    /// 403.
    #[error("The gateway refused this request (403)")]
    Forbidden,
    /// 404.
    #[error("Not found on the gateway (404)")]
    NotFound,
    /// 429.
    #[error("Too many requests. Wait a moment and try again.")]
    RateLimited,
    /// 5xx.
    #[error("The gateway had an error ({status}): {detail}")]
    Server { status: u16, detail: String },
    /// Other 4xx.
    #[error("The request failed ({status}): {detail}")]
    Client { status: u16, detail: String },
    /// The reply was not the expected JSON.
    #[error("The gateway sent a reply this app cannot read")]
    BadReply,
}

impl ApiError {
    pub fn is_auth(&self) -> bool {
        matches!(self, ApiError::Unauthorized { .. })
    }
    pub fn is_offline(&self) -> bool {
        matches!(self, ApiError::Offline(_))
    }
}

fn classify_reqwest(e: &reqwest::Error) -> ApiError {
    // reqwest error text can contain the URL; it never contains headers, and our URLs carry no secrets
    // except the WS ticket (not used through reqwest). Keep it short.
    let kind = if e.is_timeout() {
        "timed out".to_string()
    } else if e.is_connect() {
        "connection failed".to_string()
    } else {
        e.to_string().chars().take(160).collect()
    };
    ApiError::Offline(crate::redact::redact_url(&kind))
}

fn detail_of(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("detail").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| body.chars().take(160).collect())
}

async fn map_response<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, ApiError> {
    let status = resp.status().as_u16();
    // Bound the body we read (bad servers must not exhaust memory).
    let bytes = resp.bytes().await.map_err(|e| classify_reqwest(&e))?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(ApiError::BadReply);
    }
    let body = String::from_utf8_lossy(&bytes);
    match status {
        200..=299 => serde_json::from_slice(&bytes).map_err(|_| ApiError::BadReply),
        401 => {
            let expired = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("error").and_then(Value::as_str).map(|s| s == "session_expired"))
                .unwrap_or(false);
            Err(ApiError::Unauthorized { session_expired: expired })
        }
        403 => Err(ApiError::Forbidden),
        404 => Err(ApiError::NotFound),
        429 => Err(ApiError::RateLimited),
        500..=599 => Err(ApiError::Server { status, detail: detail_of(&body) }),
        _ => Err(ApiError::Client { status, detail: detail_of(&body) }),
    }
}

/// Shared token state. `RestClient` clones share it.
#[derive(Default)]
struct TokenState {
    access: Option<Secret>,
    refresh: Option<Secret>,
    provider: String,
    expires_at: i64,
}

/// A REST client for one gateway. Cheap to clone.
#[derive(Clone)]
pub struct RestClient {
    base: Url,
    http: reqwest::Client,
    tokens: Arc<RwLock<TokenState>>,
}

impl std::fmt::Debug for RestClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RestClient").field("base", &self.base.as_str()).finish()
    }
}

impl RestClient {
    pub fn new(base: Url) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .user_agent(concat!("hermes-bots-desktop/", env!("CARGO_PKG_VERSION")))
            // The redirect from Authelia/Traefik would point to an HTML page we must not follow.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("reqwest client with static config");
        Self { base, http, tokens: Arc::new(RwLock::new(TokenState::default())) }
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    pub async fn set_tokens(&self, t: &TokenSet) {
        let mut s = self.tokens.write().await;
        s.access = Some(Secret::new(t.access_token.clone()));
        s.refresh = (!t.refresh_token.is_empty()).then(|| Secret::new(t.refresh_token.clone()));
        s.provider = t.provider.clone();
        s.expires_at = t.expires_at;
    }

    pub async fn clear_tokens(&self) {
        *self.tokens.write().await = TokenState::default();
    }

    pub async fn has_token(&self) -> bool {
        self.tokens.read().await.access.is_some()
    }

    pub async fn refresh_token(&self) -> Option<(Secret, String)> {
        let s = self.tokens.read().await;
        s.refresh.clone().map(|r| (r, s.provider.clone()))
    }

    fn url(&self, path: &str) -> Url {
        url_util::join(&self.base, path)
    }

    async fn bearer(&self) -> Option<String> {
        self.tokens.read().await.access.as_ref().map(|s| format!("Bearer {}", s.expose()))
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str, auth: bool) -> Result<T, ApiError> {
        let mut req = self.http.get(self.url(path)).header("Accept", "application/json");
        if auth && let Some(b) = self.bearer().await {
            req = req.header("Authorization", b);
        }
        let resp = req.send().await.map_err(|e| classify_reqwest(&e))?;
        map_response(resp).await
    }

    async fn post_json<T: DeserializeOwned>(&self, path: &str, body: &Value, auth: bool) -> Result<T, ApiError> {
        let mut req = self.http.post(self.url(path)).header("Accept", "application/json").json(body);
        if auth && let Some(b) = self.bearer().await {
            req = req.header("Authorization", b);
        }
        let resp = req.send().await.map_err(|e| classify_reqwest(&e))?;
        map_response(resp).await
    }

    // ---- public, before sign-in ----

    pub async fn health(&self) -> Result<Health, ApiError> {
        self.get_json("/api/health", false).await
    }

    pub async fn status(&self) -> Result<Status, ApiError> {
        self.get_json("/api/status", false).await
    }

    pub async fn auth_providers(&self) -> Result<AuthProviders, ApiError> {
        self.get_json("/api/auth/providers", false).await
    }

    // ---- native token exchange ----

    pub async fn exchange_code(&self, code: &Secret, verifier: &Secret) -> Result<TokenSet, ApiError> {
        let body = json!({"code": code.expose(), "code_verifier": verifier.expose()});
        let t: TokenSet = self.post_json("/auth/native/token", &body, false).await?;
        if t.access_token.is_empty() {
            return Err(ApiError::BadReply);
        }
        self.set_tokens(&t).await;
        Ok(t)
    }

    /// Rotate the tokens with the stored refresh token. A 401 means a new sign-in is needed.
    pub async fn refresh(&self) -> Result<TokenSet, ApiError> {
        let Some((rt, provider)) = self.refresh_token().await else {
            return Err(ApiError::Unauthorized { session_expired: true });
        };
        let body = json!({"refresh_token": rt.expose(), "provider": provider});
        let t: TokenSet = self.post_json("/auth/native/refresh", &body, false).await?;
        if t.access_token.is_empty() {
            return Err(ApiError::BadReply);
        }
        self.set_tokens(&t).await;
        Ok(t)
    }

    // ---- authenticated ----

    pub async fn me(&self) -> Result<Me, ApiError> {
        self.get_json("/api/auth/me", true).await
    }

    pub async fn ws_ticket(&self) -> Result<WsTicket, ApiError> {
        let t: WsTicket = self.post_json("/api/auth/ws-ticket", &json!({}), true).await?;
        if t.ticket.is_empty() {
            return Err(ApiError::BadReply);
        }
        Ok(t)
    }

    pub async fn profiles(&self) -> Result<ProfileList, ApiError> {
        self.get_json("/api/profiles", true).await
    }

    /// Run `f` and, on a 401, refresh the tokens once and run it again.
    pub async fn with_refresh<T, F, Fut>(&self, f: F) -> Result<T, ApiError>
    where
        F: Fn(RestClient) -> Fut,
        Fut: std::future::Future<Output = Result<T, ApiError>>,
    {
        match f(self.clone()).await {
            Err(ApiError::Unauthorized { .. }) if self.refresh_token().await.is_some() => {
                self.refresh().await?;
                f(self.clone()).await
            }
            other => other,
        }
    }

    /// The token set as stored now (for writing to the keyring after a refresh).
    pub async fn current_tokens(&self) -> Option<TokenSet> {
        let s = self.tokens.read().await;
        let access = s.access.as_ref()?;
        Some(TokenSet {
            access_token: access.expose().to_string(),
            refresh_token: s.refresh.as_ref().map(|r| r.expose().to_string()).unwrap_or_default(),
            token_type: "Bearer".into(),
            expires_at: s.expires_at,
            provider: s.provider.clone(),
            user_id: String::new(),
        })
    }
}
