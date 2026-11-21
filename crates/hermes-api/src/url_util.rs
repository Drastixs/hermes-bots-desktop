//! Gateway URL handling. The gateway URL is a user setting; nothing is hard-coded.

use url::Url;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UrlError {
    #[error("Enter the gateway address, for example https://hermes.example.net")]
    Empty,
    #[error("The gateway address is not a valid URL")]
    Invalid,
    #[error("The gateway address must start with http:// or https://")]
    Scheme,
}

/// Normalise a user-entered gateway URL: add `https://` if no scheme is given,
/// drop query, fragment and a trailing slash. A path prefix is kept (the
/// gateway can live under a reverse-proxy prefix).
pub fn normalise_gateway(input: &str) -> Result<Url, UrlError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(UrlError::Empty);
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = Url::parse(&with_scheme).map_err(|_| UrlError::Invalid)?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(UrlError::Scheme);
    }
    if url.host_str().is_none() {
        return Err(UrlError::Invalid);
    }
    url.set_query(None);
    url.set_fragment(None);
    let path = url.path().trim_end_matches('/').to_string();
    url.set_path(&path);
    Ok(url)
}

/// Join an absolute API path (for example `/api/status`) to the gateway base.
pub fn join(base: &Url, path: &str) -> Url {
    let mut url = base.clone();
    let prefix = base.path().trim_end_matches('/');
    url.set_path(&format!("{prefix}{path}"));
    url
}

/// The WebSocket URL for an API path (`http` -> `ws`, `https` -> `wss`).
pub fn ws_join(base: &Url, path: &str) -> Url {
    let mut url = join(base, path);
    let scheme = if base.scheme() == "https" { "wss" } else { "ws" };
    // set_scheme between special schemes is allowed.
    let _ = url.set_scheme(scheme);
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises() {
        assert_eq!(normalise_gateway("hermes.example.net").unwrap().as_str(), "https://hermes.example.net/");
        assert_eq!(
            normalise_gateway(" http://127.0.0.1:9120/ ").unwrap().as_str(),
            "http://127.0.0.1:9120/"
        );
        assert_eq!(
            normalise_gateway("https://h.example/prefix/?x#y").unwrap().as_str(),
            "https://h.example/prefix"
        );
        assert_eq!(normalise_gateway(""), Err(UrlError::Empty));
        assert_eq!(normalise_gateway("ftp://x"), Err(UrlError::Scheme));
    }

    #[test]
    fn joins() {
        let b = normalise_gateway("https://h.example/prefix").unwrap();
        assert_eq!(join(&b, "/api/status").as_str(), "https://h.example/prefix/api/status");
        assert_eq!(ws_join(&b, "/api/ws").as_str(), "wss://h.example/prefix/api/ws");
        let b = normalise_gateway("http://127.0.0.1:1").unwrap();
        assert_eq!(ws_join(&b, "/api/ws").as_str(), "ws://127.0.0.1:1/api/ws");
    }
}
