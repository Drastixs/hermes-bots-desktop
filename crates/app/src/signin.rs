//! Sign-in task: check the gateway, run the PKCE flow in the system browser, exchange the code.

use hermes_api::RestClient;
use hermes_api::auth::{LoopbackListener, Pkce, authorize_url, generate_state};
use hermes_api::types::{Status, TokenSet};
use std::time::Duration;
use url::Url;

/// Check a gateway URL: is it a Hermes gateway, does it need sign-in, does it offer native PKCE?
pub async fn probe(gateway: Url) -> Result<Status, String> {
    let rest = RestClient::new(gateway);
    let status = rest.status().await.map_err(|e| e.to_string())?;
    if status.version.is_empty() {
        return Err("This address does not look like a Hermes gateway (no version in /api/status).".into());
    }
    if status.auth_required && !status.auth_flows.is_empty() && !status.auth_flows.iter().any(|f| f == "native_pkce") {
        return Err("This gateway does not offer native sign-in (native_pkce). Update Hermes Agent.".into());
    }
    Ok(status)
}

/// What the UI needs to show while the browser is open.
#[derive(Debug, Clone)]
pub struct Started {
    pub authorize_url: String,
}

/// Run the full sign-in. `on_url` gets the authorize URL (to show a "open again" link).
pub async fn sign_in(gateway: Url, open_browser: bool, url_tx: tokio::sync::oneshot::Sender<Started>) -> Result<TokenSet, String> {
    let rest = RestClient::new(gateway.clone());
    let pkce = Pkce::generate();
    let state = generate_state();
    let listener = LoopbackListener::bind().await.map_err(|e| e.to_string())?;
    let url = authorize_url(&gateway, &pkce, &listener.redirect_uri, &state, None);
    let _ = url_tx.send(Started { authorize_url: url.to_string() });
    if open_browser && let Err(e) = open::that_detached(url.as_str()) {
        tracing::warn!("could not open the browser: {e}");
    }
    let code = listener.wait(&state, Duration::from_secs(600)).await.map_err(|e| e.to_string())?;
    let tokens = rest.exchange_code(&code, &pkce.verifier).await.map_err(|e| e.to_string())?;
    Ok(tokens)
}
