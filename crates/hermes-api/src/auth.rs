//! RFC 8252 native sign-in (PKCE + loopback redirect) against the Hermes gateway.
//!
//! Flow (hermes-agent `hermes_cli/dashboard_auth/routes.py`):
//! 1. The app makes a PKCE pair and a random `state`, and listens on `127.0.0.1:<ephemeral>`.
//! 2. The system browser opens `/auth/native/authorize?code_challenge=..&code_challenge_method=S256
//!    &redirect_uri=http://127.0.0.1:<port>/callback&state=..` (the user signs in there).
//! 3. The gateway redirects to the loopback URL with `?code=..&state=..`.
//! 4. The app checks `state` and POSTs `{code, code_verifier}` to `/auth/native/token`.

use crate::redact::Secret;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

/// A PKCE verifier and its S256 challenge.
#[derive(Clone)]
pub struct Pkce {
    pub verifier: Secret,
    pub challenge: String,
}

fn random_b64(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

/// The S256 transform: `base64url(sha256(verifier))` without padding.
pub fn s256(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

impl Pkce {
    pub fn generate() -> Self {
        // 32 random bytes -> 43 chars, inside RFC 7636's 43..128.
        let verifier = random_b64(32);
        let challenge = s256(&verifier);
        Self { verifier: Secret::new(verifier), challenge }
    }
}

/// A random CSRF `state`.
pub fn generate_state() -> String {
    random_b64(24)
}

/// Build the `/auth/native/authorize` URL.
pub fn authorize_url(base: &Url, pkce: &Pkce, redirect_uri: &str, state: &str, provider: Option<&str>) -> Url {
    let mut url = crate::url_util::join(base, "/auth/native/authorize");
    {
        let mut q = url.query_pairs_mut();
        if let Some(p) = provider.filter(|p| !p.is_empty()) {
            q.append_pair("provider", p);
        }
        q.append_pair("code_challenge", &pkce.challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("state", state);
    }
    url
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CallbackError {
    #[error("Sign-in was cancelled or failed in the browser: {0}")]
    Denied(String),
    #[error("The sign-in reply did not match this request (state mismatch). Try again.")]
    StateMismatch,
    #[error("The sign-in reply had no code")]
    MissingCode,
    #[error("Sign-in timed out. Try again.")]
    Timeout,
    #[error("Could not listen for the sign-in reply: {0}")]
    Io(String),
}

/// Parse the loopback callback request target (`/callback?code=..&state=..`).
pub fn parse_callback(target: &str, expected_state: &str) -> Result<Secret, CallbackError> {
    let url = Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| CallbackError::MissingCode)?;
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            "error" => error = Some(v.into_owned()),
            _ => {}
        }
    }
    if let Some(e) = error {
        return Err(CallbackError::Denied(e.chars().take(120).collect()));
    }
    // Constant-time-ish compare is not needed for a one-shot local state, but keep it strict.
    if state.as_deref() != Some(expected_state) {
        return Err(CallbackError::StateMismatch);
    }
    code.filter(|c| !c.is_empty()).map(Secret::new).ok_or(CallbackError::MissingCode)
}

/// A loopback listener waiting for one sign-in callback.
pub struct LoopbackListener {
    listener: TcpListener,
    pub redirect_uri: String,
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Hermes Bots</title>\
<body style=\"font-family:sans-serif;background:#041c1c;color:#ffe6cb;display:grid;place-items:center;height:90vh\">\
<div><h2>Signed in</h2><p>You can close this tab and go back to Hermes Bots.</p></div>";
const FAIL_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Hermes Bots</title>\
<body style=\"font-family:sans-serif;background:#041c1c;color:#ffe6cb;display:grid;place-items:center;height:90vh\">\
<div><h2>Sign-in failed</h2><p>Go back to Hermes Bots and try again.</p></div>";

impl LoopbackListener {
    /// Bind `127.0.0.1` on an ephemeral port.
    pub async fn bind() -> Result<Self, CallbackError> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(|e| CallbackError::Io(e.to_string()))?;
        let port = listener.local_addr().map_err(|e| CallbackError::Io(e.to_string()))?.port();
        Ok(Self { listener, redirect_uri: format!("http://127.0.0.1:{port}/callback") })
    }

    /// Wait for the `/callback` request, answer it with a small page, and return the code.
    /// Other paths (for example `/favicon.ico`) get a 404 and the wait goes on.
    pub async fn wait(self, expected_state: &str, timeout: Duration) -> Result<Secret, CallbackError> {
        let fut = async {
            loop {
                let (mut sock, _) = self.listener.accept().await.map_err(|e| CallbackError::Io(e.to_string()))?;
                let mut buf = vec![0u8; 8192];
                let mut used = 0usize;
                // Read until the end of the request line (we only need the target).
                let line = loop {
                    let n = match tokio::time::timeout(Duration::from_secs(5), sock.read(&mut buf[used..])).await {
                        Ok(Ok(n)) => n,
                        _ => break None,
                    };
                    if n == 0 {
                        break None;
                    }
                    used += n;
                    if let Some(pos) = buf[..used].windows(2).position(|w| w == b"\r\n") {
                        break Some(String::from_utf8_lossy(&buf[..pos]).into_owned());
                    }
                    if used == buf.len() {
                        break None;
                    }
                };
                let Some(line) = line else { continue };
                let target = line.split_whitespace().nth(1).unwrap_or("/").to_string();
                if !target.starts_with("/callback") {
                    let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                    continue;
                }
                let result = parse_callback(&target, expected_state);
                let body = if result.is_ok() { DONE_PAGE } else { FAIL_PAGE };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.shutdown().await;
                return result;
            }
        };
        tokio::time::timeout(timeout, fut).await.unwrap_or(Err(CallbackError::Timeout))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_vector() {
        // RFC 7636 appendix B.
        assert_eq!(s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let p = Pkce::generate();
        assert_eq!(p.verifier.expose().len(), 43);
        assert_eq!(s256(p.verifier.expose()), p.challenge);
    }

    #[test]
    fn authorize_url_shape() {
        let base = crate::url_util::normalise_gateway("https://g.example/pre").unwrap();
        let p = Pkce { verifier: Secret::new("v"), challenge: "c".into() };
        let u = authorize_url(&base, &p, "http://127.0.0.1:5/callback", "st", None);
        assert_eq!(u.path(), "/pre/auth/native/authorize");
        let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:5/callback");
        assert!(!q.contains_key("provider"));
    }

    #[test]
    fn callback_parsing() {
        assert_eq!(parse_callback("/callback?code=abc&state=s1", "s1").unwrap().expose(), "abc");
        assert_eq!(parse_callback("/callback?code=abc&state=s2", "s1").unwrap_err(), CallbackError::StateMismatch);
        assert_eq!(parse_callback("/callback?state=s1", "s1").unwrap_err(), CallbackError::MissingCode);
        assert!(matches!(parse_callback("/callback?error=access_denied", "s1"), Err(CallbackError::Denied(_))));
    }

    #[tokio::test]
    async fn loopback_round_trip() {
        let l = LoopbackListener::bind().await.unwrap();
        let uri = l.redirect_uri.clone();
        let port: u16 = uri.split(':').nth(2).unwrap().split('/').next().unwrap().parse().unwrap();
        let task = tokio::spawn(async move { l.wait("st", Duration::from_secs(5)).await });
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(b"GET /favicon.ico HTTP/1.1\r\n\r\n").await.unwrap();
        drop(s);
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(b"GET /callback?code=xyz&state=st HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        assert!(out.contains("Signed in"));
        assert_eq!(task.await.unwrap().unwrap().expose(), "xyz");
    }
}
