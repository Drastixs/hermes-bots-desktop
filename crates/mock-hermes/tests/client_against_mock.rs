//! Integration tests: the real `hermes-api` client against the mock gateway.

use hermes_api::auth::{LoopbackListener, Pkce, authorize_url, generate_state};
use hermes_api::gateway::{Gateway, Incoming};
use hermes_api::rfb::{RfbEvent, RfbSession, key_event, keysym};
use hermes_api::types::Event;
use hermes_api::{ApiError, RestClient};
use std::time::Duration;

async fn start() -> (RestClient, String, mock_hermes::Shared) {
    let (addr, mock, _h) = mock_hermes::serve("127.0.0.1:0", 0.02).await.unwrap();
    let base = format!("http://{addr}");
    let rest = RestClient::new(hermes_api::url_util::normalise_gateway(&base).unwrap());
    (rest, base, mock)
}

/// Do the full PKCE flow. The "browser" is a plain HTTP client that follows the redirects.
async fn sign_in(rest: &RestClient, base: &str) {
    let pkce = Pkce::generate();
    let state = generate_state();
    let listener = LoopbackListener::bind().await.unwrap();
    let url = authorize_url(rest.base(), &pkce, &listener.redirect_uri, &state, None);
    let wait = tokio::spawn({
        let state = state.clone();
        async move { listener.wait(&state, Duration::from_secs(10)).await }
    });
    let browser = reqwest::Client::builder().redirect(reqwest::redirect::Policy::limited(5)).build().unwrap();
    // authorize -> /login form
    let page = browser.get(url).send().await.unwrap();
    let html = page.text().await.unwrap();
    let broker = html.split("name=broker value='").nth(1).unwrap().split('\'').next().unwrap().to_string();
    // submit form -> redirect to loopback -> listener answers
    let r = browser.post(format!("{base}/__mock/approve")).body(format!("broker={broker}")).send().await.unwrap();
    assert!(r.text().await.unwrap().contains("Signed in"));
    let code = wait.await.unwrap().unwrap();
    let tokens = rest.exchange_code(&code, &pkce.verifier).await.unwrap();
    assert!(!tokens.access_token.is_empty());
    // A code is single use.
    assert!(rest.exchange_code(&code, &pkce.verifier).await.is_err());
}

#[tokio::test]
async fn public_endpoints_and_auth_gate() {
    let (rest, _, _) = start().await;
    let h = rest.health().await.unwrap();
    assert!(h.ok && h.auth_required);
    let s = rest.status().await.unwrap();
    assert!(s.auth_flows.contains(&"native_pkce".to_string()));
    assert_eq!(rest.auth_providers().await.unwrap().providers[0].name, "basic");
    assert!(matches!(rest.profiles().await, Err(ApiError::Unauthorized { session_expired: true })));
}

#[tokio::test]
async fn sign_in_list_bots_and_refresh() {
    let (rest, base, _) = start().await;
    sign_in(&rest, &base).await;
    assert_eq!(rest.me().await.unwrap().display_name, "Demo User");
    let list = rest.profiles().await.unwrap();
    assert_eq!(list.profiles.len(), 7);
    assert!(list.profiles.iter().all(|p| p.is_bot()));
    assert_eq!(list.profiles[0].title(), "Atlas");
    // Expired tokens: one refresh fixes it.
    reqwest::Client::new().post(format!("{base}/__mock/expire-tokens")).send().await.unwrap();
    assert!(rest.profiles().await.unwrap_err().is_auth());
    let again = rest.with_refresh(|c| async move { c.profiles().await }).await.unwrap();
    assert_eq!(again.profiles.len(), 7);
}

#[tokio::test]
async fn chat_streams_reply_and_tools() {
    let (rest, base, _) = start().await;
    sign_in(&rest, &base).await;
    let (gw, mut rx) = Gateway::connect(&rest).await.unwrap();
    // A ticket is single use: the same URL cannot be reused (tested via a fresh mint working).
    let chat = gw.open_bot_chat("scout").await.unwrap();
    assert!(!chat.messages.is_empty());
    let sub = gw.prompt_submit(&chat.session_id, "scout", "Give me an update").await.unwrap();
    assert_eq!(sub.status.as_deref(), Some("streaming"));
    let mut text = String::new();
    let mut tools = 0;
    let mut done = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !done {
        let item = tokio::time::timeout_at(deadline, rx.recv()).await.unwrap().unwrap();
        if let Incoming::Event(e) = item {
            if e.session_id.as_deref() != Some(chat.session_id.as_str()) {
                continue;
            }
            match e.typed() {
                Event::MessageDelta { text: t } => text.push_str(&t),
                Event::ToolComplete { .. } => tools += 1,
                Event::MessageComplete { text: Some(full), .. } => {
                    assert_eq!(full, text);
                    done = true;
                }
                _ => {}
            }
        }
    }
    assert_eq!(tools, 3);
    assert!(text.contains("Done."));
}

#[tokio::test]
async fn bot_screen_take_over_and_hand_back() {
    let (rest, base, mock) = start().await;
    sign_in(&rest, &base).await;
    let (gw, _rx) = Gateway::connect(&rest).await.unwrap();
    let st = gw.display_status("scout").await.unwrap();
    assert!(st.running);
    let obs = gw.display_observe("scout", None).await.unwrap();
    assert_eq!(obs.path, "/api/display/ws");
    let mut url = hermes_api::url_util::ws_join(rest.base(), &obs.path);
    url.query_pairs_mut().append_pair("display_ticket", &obs.ticket);
    let (ev_tx, mut ev_rx) = tokio::sync::mpsc::channel(8);
    let session = RfbSession::connect(url.as_str(), ev_tx).await.unwrap();
    let mut got_frame = false;
    while !got_frame {
        match tokio::time::timeout(Duration::from_secs(10), ev_rx.recv()).await.unwrap().unwrap() {
            RfbEvent::Ready { width, height, .. } => assert_eq!((width, height), (1280, 800)),
            RfbEvent::Frame(fb) => {
                assert_eq!(fb.rgba.len(), 1280 * 800 * 4);
                got_frame = true;
            }
            RfbEvent::Closed { reason, .. } => panic!("closed: {reason}"),
        }
    }
    // Watching: input is dropped by the server.
    session.input.send(key_event(true, 'x' as u32)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(mock.inner.lock().await.screens["scout"].email, "");
    // Take over: input reaches the screen.
    let l = gw.lease_acquire("scout", &obs.viewer_id).await.unwrap();
    assert!(l.lease.human_holds());
    for c in "me@example.com".chars() {
        session.input.send(key_event(true, c as u32)).await.unwrap();
        session.input.send(key_event(false, c as u32)).await.unwrap();
    }
    session.input.send(key_event(true, keysym::TAB)).await.unwrap();
    session.input.send(key_event(true, 's' as u32)).await.unwrap();
    session.input.send(key_event(true, keysym::RETURN)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    {
        let g = mock.inner.lock().await;
        assert_eq!(g.screens["scout"].email, "me@example.com");
        assert!(g.screens["scout"].signed_in);
    }
    // Hand back.
    let l = gw.lease_release("scout", &obs.viewer_id).await.unwrap();
    assert!(!l.lease.human_holds());
    // A display ticket is single use.
    let (ev_tx2, mut ev_rx2) = tokio::sync::mpsc::channel(8);
    let _s2 = RfbSession::connect(url.as_str(), ev_tx2).await.unwrap();
    match tokio::time::timeout(Duration::from_secs(5), ev_rx2.recv()).await.unwrap().unwrap() {
        RfbEvent::Closed { code, .. } => assert_eq!(code, Some(4401)),
        other => panic!("expected close, got {other:?}"),
    }
}

#[tokio::test]
async fn server_drop_closes_and_reconnect_works() {
    let (rest, base, _) = start().await;
    sign_in(&rest, &base).await;
    let (gw, mut rx) = Gateway::connect(&rest).await.unwrap();
    gw.ping().await.unwrap();
    reqwest::Client::new().post(format!("{base}/__mock/drop-sockets")).send().await.unwrap();
    loop {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap() {
            Some(Incoming::Closed { code, .. }) => {
                assert_eq!(code, Some(1012));
                break;
            }
            Some(_) => continue,
            None => break,
        }
    }
    assert!(gw.ping().await.is_err());
    let (gw2, _rx2) = Gateway::connect(&rest).await.unwrap();
    gw2.ping().await.unwrap();
}

#[tokio::test]
async fn bad_ticket_is_refused() {
    let (rest, base, _) = start().await;
    sign_in(&rest, &base).await;
    let url = format!("{}?ticket=nope", hermes_api::url_util::ws_join(rest.base(), "/api/ws"));
    let (_gw, mut rx) = Gateway::connect_url(&url).await.unwrap();
    match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap() {
        Incoming::Closed { code, .. } => assert_eq!(code, Some(4401)),
        other => panic!("expected close, got {other:?}"),
    }
}

#[tokio::test]
async fn create_bot() {
    let (rest, base, _) = start().await;
    sign_in(&rest, &base).await;
    let (gw, _rx) = Gateway::connect(&rest).await.unwrap();
    assert!(gw.profiles_create("Bad Name!", "").await.is_err());
    let c = gw.profiles_create("helper", "Helps").await.unwrap();
    assert_eq!(c.name, "helper");
    assert!(gw.profiles_create("helper", "").await.is_err());
    assert_eq!(rest.profiles().await.unwrap().profiles.len(), 8);
}
