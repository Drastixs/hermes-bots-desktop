//! A mock Hermes Agent gateway that implements the subset of the v0.23.3 API that
//! Hermes Bots Desktop uses. It serves mock data only.
//!
//! Test controls (mock only, under `/__mock/*`):
//! * `POST /__mock/expire-tokens`  every issued access token becomes invalid (401 session_expired)
//! * `POST /__mock/drop-sockets`   close every live WebSocket (simulates a server drop)
//! * `POST /__mock/offline?secs=N` answer 503 / refuse sockets for N seconds
//! * `POST /__mock/auto-approve`   the next `/auth/native/authorize` redirects at once (no browser form)
//! * `GET  /__mock/state`          counters for tests

pub mod fixtures;
pub mod screen;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, broadcast, mpsc};

pub const VERSION: &str = "0.23.3";

#[derive(Default)]
pub struct Inner {
    pending: HashMap<String, (String, String, String)>, // broker -> (challenge, redirect, state)
    codes: HashMap<String, String>,                    // code -> challenge
    access: HashSet<String>,
    refresh: HashSet<String>,
    tickets: HashMap<String, Instant>,
    display_tickets: HashMap<String, (String, Instant)>, // ticket -> (profile, minted)
    sessions: HashMap<String, String>,                  // live session id -> profile
    stored: HashMap<String, Vec<Value>>,                // profile -> extra transcript rows
    lease: HashMap<String, (String, String, i64)>,      // profile -> (holder, viewer_id, epoch)
    pub screens: HashMap<String, screen::Screen>,
    screens_running: HashSet<String>,
    created: Vec<Value>,
    offline_until: Option<Instant>,
    auto_approve: bool,
    pub prompts: Vec<(String, String)>,
}

pub struct Mock {
    pub inner: Mutex<Inner>,
    /// Gateway events broadcast to every /api/ws client.
    events: broadcast::Sender<Value>,
    /// Kill switch for live sockets.
    drop_tx: broadcast::Sender<()>,
    counter: AtomicU64,
    /// Speed factor for streaming (1.0 = normal; tests use a small value).
    pub speed: f64,
}

pub type Shared = Arc<Mock>;

fn token() -> String {
    let mut b = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut b);
    URL_SAFE_NO_PAD.encode(b)
}

fn s256(v: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(v.as_bytes()))
}

impl Mock {
    pub fn new(speed: f64) -> Shared {
        let (events, _) = broadcast::channel(1024);
        let (drop_tx, _) = broadcast::channel(4);
        let mut inner = Inner::default();
        for b in fixtures::BOTS.iter() {
            inner.lease.insert(b.name.to_string(), ("agent".into(), String::new(), 1));
        }
        // Two Bots have a running screen at start.
        inner.screens_running.insert("scout".into());
        inner.screens_running.insert("default".into());
        Arc::new(Mock { inner: Mutex::new(inner), events, drop_tx, counter: AtomicU64::new(1), speed })
    }

    fn next(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }

    async fn offline(&self) -> bool {
        let g = self.inner.lock().await;
        g.offline_until.is_some_and(|t| Instant::now() < t)
    }

    fn emit(&self, kind: &str, sid: &str, payload: Value) {
        let _ = self.events.send(json!({"jsonrpc": "2.0", "method": "event", "params": {"type": kind, "session_id": sid, "payload": payload}}));
    }

    async fn sleep(&self, ms: u64) {
        tokio::time::sleep(Duration::from_millis(((ms as f64) * self.speed) as u64)).await;
    }
}

pub fn router(mock: Shared) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/status", get(status))
        .route("/api/auth/providers", get(providers))
        .route("/auth/native/authorize", get(authorize))
        .route("/login", get(login_page))
        .route("/__mock/approve", post(approve_form))
        .route("/auth/native/token", post(native_token))
        .route("/auth/native/refresh", post(native_refresh))
        .route("/api/auth/me", get(me))
        .route("/api/auth/ws-ticket", post(ws_ticket))
        .route("/api/profiles", get(profiles))
        .route("/api/ws", get(gateway_ws))
        .route("/api/display/ws", get(display_ws))
        .route("/__mock/expire-tokens", post(expire_tokens))
        .route("/__mock/drop-sockets", post(drop_sockets))
        .route("/__mock/offline", post(go_offline))
        .route("/__mock/auto-approve", post(auto_approve))
        .route("/__mock/state", get(state))
        .with_state(mock)
}

// ---------------- public ----------------

async fn health(State(m): State<Shared>) -> Response {
    if m.offline().await {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    Json(json!({"ok": true, "version": VERSION, "displayVersion": VERSION, "commit": "mock", "auth_required": true})).into_response()
}

async fn status(State(m): State<Shared>) -> Response {
    if m.offline().await {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    Json(json!({
        "version": VERSION, "release_date": "2026-11-01", "auth_required": true,
        "auth_providers": ["basic"], "auth_flows": ["cookie", "native_pkce"],
        "gateway_running": true, "gateway_state": "running", "overall": "ok", "components": {}
    }))
    .into_response()
}

async fn providers() -> Json<Value> {
    Json(json!({"providers": [{"name": "basic", "display_name": "Username & Password", "supports_password": true}]}))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AuthorizeQ {
    code_challenge: String,
    code_challenge_method: String,
    redirect_uri: String,
    state: String,
}

async fn authorize(State(m): State<Shared>, Query(q): Query<AuthorizeQ>) -> Response {
    if q.code_challenge_method.to_uppercase() != "S256" {
        return (StatusCode::BAD_REQUEST, Json(json!({"detail": "code_challenge_method must be S256"}))).into_response();
    }
    if q.code_challenge.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"detail": "code_challenge required"}))).into_response();
    }
    let ok_redirect = url::Url::parse(&q.redirect_uri)
        .ok()
        .is_some_and(|u| u.scheme() == "http" && matches!(u.host_str(), Some("127.0.0.1") | Some("[::1]")));
    if !ok_redirect {
        return (StatusCode::BAD_REQUEST, Json(json!({"detail": "native redirect_uri host must be a loopback IP literal (127.0.0.1 / ::1)"}))).into_response();
    }
    let broker = token();
    let mut g = m.inner.lock().await;
    g.pending.insert(broker.clone(), (q.code_challenge, q.redirect_uri, q.state));
    if g.auto_approve {
        drop(g);
        return finish_login(&m, &broker).await;
    }
    Redirect::to(&format!("/login?broker={broker}")).into_response()
}

async fn login_page(Query(q): Query<HashMap<String, String>>) -> Html<String> {
    let broker = q.get("broker").cloned().unwrap_or_default();
    Html(format!(
        "<!doctype html><title>Mock Hermes sign-in</title><body style='font-family:sans-serif;background:#041c1c;color:#ffe6cb'>\
<h2>Mock Hermes sign-in</h2><form method=post action='/__mock/approve'>\
<input type=hidden name=broker value='{broker}'><p>User <input name=username value='demo'></p>\
<p>Password <input type=password name=password value='demo'></p><button>Sign in</button></form>"
    ))
}

async fn approve_form(State(m): State<Shared>, body: String) -> Response {
    let broker = body
        .split('&')
        .find_map(|kv| kv.strip_prefix("broker="))
        .unwrap_or_default()
        .to_string();
    finish_login(&m, &broker).await
}

async fn finish_login(m: &Shared, broker: &str) -> Response {
    let mut g = m.inner.lock().await;
    let Some((challenge, redirect, state)) = g.pending.remove(broker) else {
        return (StatusCode::BAD_REQUEST, "Native sign-in expired; start again").into_response();
    };
    let code = token();
    g.codes.insert(code.clone(), challenge);
    let sep = if redirect.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{redirect}{sep}code={code}&state={state}")).into_response()
}

#[derive(Deserialize)]
struct TokenBody {
    code: String,
    code_verifier: String,
}

fn bearer_payload(access: &str, refresh: &str) -> Value {
    json!({"access_token": access, "refresh_token": refresh, "token_type": "Bearer",
           "expires_at": (fixtures::now() as i64) + 3600, "provider": "basic", "user_id": "demo"})
}

async fn native_token(State(m): State<Shared>, Json(b): Json<TokenBody>) -> Response {
    let mut g = m.inner.lock().await;
    let Some(challenge) = g.codes.remove(&b.code) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"detail": "Invalid or expired authorization code."}))).into_response();
    };
    if s256(&b.code_verifier) != challenge {
        return (StatusCode::BAD_REQUEST, Json(json!({"detail": "Invalid or expired authorization code."}))).into_response();
    }
    let (a, r) = (token(), token());
    g.access.insert(a.clone());
    g.refresh.insert(r.clone());
    Json(bearer_payload(&a, &r)).into_response()
}

#[derive(Deserialize)]
struct RefreshBody {
    refresh_token: String,
}

async fn native_refresh(State(m): State<Shared>, Json(b): Json<RefreshBody>) -> Response {
    let mut g = m.inner.lock().await;
    if !g.refresh.remove(&b.refresh_token) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "session_expired", "detail": "Refresh token expired or invalid; start a new sign-in."}))).into_response();
    }
    let (a, r) = (token(), token());
    g.access.insert(a.clone());
    g.refresh.insert(r.clone());
    Json(bearer_payload(&a, &r)).into_response()
}

// ---------------- authenticated ----------------

async fn check_bearer(m: &Shared, headers: &HeaderMap) -> Result<(), Response> {
    if m.offline().await {
        return Err(StatusCode::SERVICE_UNAVAILABLE.into_response());
    }
    let tok = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
        .unwrap_or("");
    if tok.is_empty() || !m.inner.lock().await.access.contains(tok) {
        return Err((StatusCode::UNAUTHORIZED, Json(json!({"error": "session_expired", "detail": "Unauthorized", "reason": "token", "login_url": "/login"}))).into_response());
    }
    Ok(())
}

async fn me(State(m): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = check_bearer(&m, &headers).await {
        return r;
    }
    Json(json!({"user_id": "demo", "email": "demo@example.com", "display_name": "Demo User", "org_id": "", "provider": "basic", "expires_at": 0})).into_response()
}

async fn ws_ticket(State(m): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = check_bearer(&m, &headers).await {
        return r;
    }
    let t = token();
    m.inner.lock().await.tickets.insert(t.clone(), Instant::now());
    Json(json!({"ticket": t, "ttl_seconds": 30})).into_response()
}

async fn profiles_value(m: &Shared) -> Value {
    let mut rows: Vec<Value> = fixtures::BOTS.iter().enumerate().map(|(i, b)| fixtures::profile_row(b, i)).collect();
    let g = m.inner.lock().await;
    rows.extend(g.created.iter().cloned());
    json!({"profiles": rows, "install_id": "mock-install"})
}

async fn profiles(State(m): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = check_bearer(&m, &headers).await {
        return r;
    }
    Json(profiles_value(&m).await).into_response()
}

// ---------------- mock controls ----------------

async fn expire_tokens(State(m): State<Shared>) -> &'static str {
    m.inner.lock().await.access.clear();
    "ok"
}

async fn drop_sockets(State(m): State<Shared>) -> &'static str {
    let _ = m.drop_tx.send(());
    "ok"
}

async fn go_offline(State(m): State<Shared>, Query(q): Query<HashMap<String, String>>) -> &'static str {
    let secs: u64 = q.get("secs").and_then(|s| s.parse().ok()).unwrap_or(5);
    m.inner.lock().await.offline_until = Some(Instant::now() + Duration::from_secs(secs));
    let _ = m.drop_tx.send(());
    "ok"
}

async fn auto_approve(State(m): State<Shared>) -> &'static str {
    m.inner.lock().await.auto_approve = true;
    "ok"
}

async fn state(State(m): State<Shared>) -> Json<Value> {
    let g = m.inner.lock().await;
    Json(json!({
        "prompts": g.prompts.iter().map(|(p, t)| json!({"profile": p, "text": t})).collect::<Vec<_>>(),
        "leases": g.lease.iter().map(|(k, v)| (k.clone(), json!(v.0))).collect::<serde_json::Map<_, _>>(),
        "screens": g.screens.iter().map(|(k, s)| (k.clone(), json!({"email": s.email, "signed_in": s.signed_in}))).collect::<serde_json::Map<_, _>>(),
    }))
}

// ---------------- /api/ws ----------------

async fn gateway_ws(State(m): State<Shared>, Query(q): Query<HashMap<String, String>>, ws: WebSocketUpgrade) -> Response {
    let ok = {
        let mut g = m.inner.lock().await;
        let offline = g.offline_until.is_some_and(|t| Instant::now() < t);
        let t = q.get("ticket").cloned().unwrap_or_default();
        !offline && g.tickets.remove(&t).is_some_and(|at| at.elapsed() < Duration::from_secs(30))
    };
    ws.on_upgrade(move |socket| async move {
        if !ok {
            let mut s = socket;
            let _ = s.send(Message::Close(Some(axum::extract::ws::CloseFrame { code: 4401, reason: "auth: ticket_invalid".into() }))).await;
            return;
        }
        run_gateway(m, socket).await
    })
}

fn display_status(g: &Inner, profile: &str) -> Value {
    let (holder, _, epoch) = g.lease.get(profile).cloned().unwrap_or(("agent".into(), String::new(), 1));
    json!({
        "profile": profile, "supported": true, "installed": true, "missing": [],
        "running": g.screens_running.contains(profile), "geometry": format!("{}x{}", screen::W, screen::H),
        "placement": "gateway", "profile_key": format!("/home/mock/.hermes/profiles/{profile}"),
        "lease": {"holder": holder, "viewer_id": null, "since": fixtures::now(), "epoch": epoch, "reason": ""}
    })
}

async fn run_gateway(m: Shared, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Value>(512);
    let mut events = m.events.subscribe();
    let mut kill = m.drop_tx.subscribe();
    let _ = tx
        .send(json!({"jsonrpc": "2.0", "method": "event", "params": {"type": "gateway.ready", "payload": {"skin": {}, "change_events": true, "heartbeat": true, "replay_epoch": "mock"}}}))
        .await;
    let writer = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(v) = rx.recv() => { if sink.send(Message::Text(v.to_string().into())).await.is_err() { break; } }
                Ok(v) = events.recv() => { if sink.send(Message::Text(v.to_string().into())).await.is_err() { break; } }
                _ = kill.recv() => { let _ = sink.send(Message::Close(Some(axum::extract::ws::CloseFrame { code: 1012, reason: "restart".into() }))).await; break; }
                else => break,
            }
        }
    });
    while let Some(Ok(msg)) = stream.next().await {
        let Message::Text(t) = msg else { continue };
        let Ok(req) = serde_json::from_str::<Value>(t.as_str()) else {
            let _ = tx.send(json!({"jsonrpc": "2.0", "error": {"code": -32700, "message": "parse error"}, "id": null})).await;
            continue;
        };
        if req.get("method").is_none() {
            continue; // a reply to a server request
        }
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(Value::as_str).unwrap_or("").to_string();
        let params = req.get("params").cloned().unwrap_or(json!({}));
        let reply = handle_rpc(&m, &method, &params).await;
        let frame = match reply {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, msg)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": msg}}),
        };
        if tx.send(frame).await.is_err() {
            break;
        }
    }
    writer.abort();
}

fn profile_of(p: &Value) -> String {
    p.get("profile").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or("default").to_string()
}

async fn handle_rpc(m: &Shared, method: &str, p: &Value) -> Result<Value, (i64, String)> {
    let profile = profile_of(p);
    match method {
        "gateway.ping" => Ok(json!({"ok": true})),
        "profiles.list" => Ok(profiles_value(m).await),
        "session.list" => {
            let title = p.get("title").and_then(Value::as_str).unwrap_or("");
            let known = fixtures::BOTS.iter().any(|b| b.name == profile);
            let rows = if known && (title.is_empty() || title == "Bot Chat") {
                vec![json!({"id": format!("chat-{profile}"), "title": "Bot Chat", "preview": "", "started_at": fixtures::now() - 86400.0, "message_count": 6, "source": "desktop"})]
            } else {
                vec![]
            };
            Ok(json!({"sessions": rows}))
        }
        "session.resume" | "session.create" => {
            let sid = format!("live-{}-{}", profile, m.next());
            let mut g = m.inner.lock().await;
            g.sessions.insert(sid.clone(), profile.clone());
            let mut msgs = if method == "session.resume" { fixtures::history(&profile) } else { vec![] };
            msgs.extend(g.stored.get(&profile).cloned().unwrap_or_default());
            let mut r = json!({"session_id": sid, "stored_session_id": format!("chat-{profile}"), "message_count": msgs.len(), "messages": msgs, "info": {"model": "mock-model-1", "provider": "mock"}});
            if method == "session.resume" {
                r["running"] = json!(false);
            }
            Ok(r)
        }
        "prompt.submit" => {
            let sid = p.get("session_id").and_then(Value::as_str).unwrap_or("").to_string();
            let text = p.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            let prof = {
                let mut g = m.inner.lock().await;
                let prof = g.sessions.get(&sid).cloned();
                if let Some(pr) = &prof {
                    g.prompts.push((pr.clone(), text.clone()));
                    g.stored.entry(pr.clone()).or_default().push(json!({"role": "user", "text": text, "timestamp": fixtures::now()}));
                }
                prof
            };
            let Some(prof) = prof else { return Err((4004, "unknown session".into())) };
            let m2 = m.clone();
            tokio::spawn(async move { stream_reply(m2, sid, prof, text).await });
            Ok(json!({"status": "streaming"}))
        }
        "session.interrupt" => Ok(json!({"status": "interrupted", "interrupted": true})),
        "approval.respond" => Ok(json!({"resolved": 1})),
        "display.status" => Ok(display_status(&*m.inner.lock().await, &profile)),
        "display.start" => {
            let mut g = m.inner.lock().await;
            g.screens_running.insert(profile.clone());
            let st = display_status(&g, &profile);
            drop(g);
            m.emit("display.status", "", st.clone());
            Ok(st)
        }
        "display.observe" => {
            let mut g = m.inner.lock().await;
            if !g.screens_running.contains(&profile) {
                return Err((5300, "this profile's Bot Desktop is not running; call display.start first".into()));
            }
            let t = token();
            g.display_tickets.insert(t.clone(), (profile.clone(), Instant::now()));
            let viewer = p.get("viewer_id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("viewer-{}", m.next()));
            let mut st = display_status(&g, &profile);
            st["ticket"] = json!(t);
            st["path"] = json!("/api/display/ws");
            st["viewer_id"] = json!(viewer);
            Ok(st)
        }
        "display.lease.acquire" => {
            let viewer = p.get("viewer_id").and_then(Value::as_str).unwrap_or("").to_string();
            if viewer.is_empty() {
                return Err((4000, "viewer_id required".into()));
            }
            let mut g = m.inner.lock().await;
            let e = g.lease.get(&profile).map(|l| l.2).unwrap_or(1) + 1;
            g.lease.insert(profile.clone(), ("human".into(), viewer, e));
            let lease = json!({"holder": "human", "viewer_id": null, "since": fixtures::now(), "epoch": e, "reason": "sign-in"});
            drop(g);
            m.emit("display.lease", "", json!({"profile_key": format!("/home/mock/.hermes/profiles/{profile}"), "lease": lease.clone()}));
            Ok(json!({"lease": lease}))
        }
        "display.lease.release" => {
            let mut g = m.inner.lock().await;
            let e = g.lease.get(&profile).map(|l| l.2).unwrap_or(1) + 1;
            g.lease.insert(profile.clone(), ("agent".into(), String::new(), e));
            let lease = json!({"holder": "agent", "viewer_id": null, "since": fixtures::now(), "epoch": e, "reason": ""});
            drop(g);
            m.emit("display.lease", "", json!({"profile_key": format!("/home/mock/.hermes/profiles/{profile}"), "lease": lease.clone()}));
            Ok(json!({"lease": lease}))
        }
        "display.thumbnail" => Ok(json!({"data_url": null, "suppressed": null})),
        "profiles.create" => {
            let name = p.get("name").and_then(Value::as_str).unwrap_or("").trim().to_lowercase();
            let valid = !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if !valid {
                return Err((4000, "Profile names use letters, digits, - and _".into()));
            }
            let mut g = m.inner.lock().await;
            if fixtures::BOTS.iter().any(|b| b.name == name) || g.created.iter().any(|r| r["name"] == json!(name)) {
                return Err((4009, format!("Profile '{name}' already exists")));
            }
            let desc = p.get("description").and_then(Value::as_str).unwrap_or("").to_string();
            g.created.push(json!({"name": name, "path": format!("/home/mock/.hermes/profiles/{name}"), "description": desc,
                "ui_meta": {"hermes-bots": {"title": name, "color": "#8b5cf6"}}}));
            Ok(json!({"ok": true, "name": name, "path": format!("/home/mock/.hermes/profiles/{name}"), "mirrored": {}, "warnings": []}))
        }
        "profiles.configure" => {
            let mut g = m.inner.lock().await;
            if let Some(meta) = p.get("ui_meta").and_then(|u| u.get("hermes-bots"))
                && let Some(row) = g.created.iter_mut().find(|r| r["name"] == json!(profile))
            {
                for (k, v) in meta.as_object().cloned().unwrap_or_default() {
                    row["ui_meta"]["hermes-bots"][k] = v;
                }
            }
            Ok(json!({"ok": true, "applied": {}}))
        }
        _ => Err((-32601, format!("unknown method {method}"))),
    }
}

async fn stream_reply(m: Shared, sid: String, profile: String, text: String) {
    let title = fixtures::BOTS.iter().find(|b| b.name == profile).map(|b| b.title).unwrap_or("Bot");
    let (tools, reply) = fixtures::reply_for(&text, title);
    m.sleep(250).await;
    m.emit("message.start", &sid, json!({}));
    m.emit("status.update", &sid, json!({"kind": "thinking", "text": "Thinking"}));
    m.sleep(500).await;
    m.emit("thinking.delta", &sid, json!({"text": "Planning the steps."}));
    let login = tools.iter().any(|t| t.0 == "computer_use");
    for (i, (name, ctx)) in tools.iter().enumerate() {
        let tid = format!("t{}-{}", m.next(), i);
        m.emit("tool.start", &sid, json!({"tool_id": tid, "name": name, "context": ctx}));
        m.emit("status.update", &sid, json!({"kind": "tool", "text": format!("Using {name}")}));
        if *name == "browser_navigate" || *name == "computer_use" {
            let mut g = m.inner.lock().await;
            g.screens_running.insert(profile.clone());
            g.screens.entry(profile.clone()).or_default();
            let st = display_status(&g, &profile);
            drop(g);
            m.emit("display.status", "", st);
        }
        m.sleep(700).await;
        m.emit("tool.progress", &sid, json!({"tool_id": tid, "name": name, "progress": 0.5, "percent": 50, "message": "Working"}));
        m.sleep(600).await;
        let summary = match *name {
            "web_search" => "4 results",
            "read_file" => "Read 1 file",
            "terminal" => "Exit 0",
            "browser_navigate" => "Opened the sign-in page",
            _ => "Waiting for you",
        };
        m.emit("tool.complete", &sid, json!({"tool_id": tid, "name": name, "duration_s": 1.3, "summary": summary}));
    }
    let mut acc = String::new();
    for word in reply.split_inclusive(' ') {
        acc.push_str(word);
        m.emit("message.delta", &sid, json!({"text": word}));
        m.sleep(45).await;
    }
    m.emit("message.complete", &sid, json!({"text": acc, "status": "complete"}));
    m.emit("status.update", &sid, json!({"kind": "idle", "text": if login { "Waiting for you" } else { "" }}));
    m.inner.lock().await.stored.entry(profile).or_default().push(json!({"role": "assistant", "text": acc, "timestamp": fixtures::now()}));
}

// ---------------- /api/display/ws (fake RFB) ----------------

async fn display_ws(State(m): State<Shared>, Query(q): Query<HashMap<String, String>>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| async move {
        let mut socket = socket;
        let t = q.get("display_ticket").cloned().unwrap_or_default();
        let found = m.inner.lock().await.display_tickets.remove(&t).filter(|(_, at)| at.elapsed() < Duration::from_secs(30));
        let Some((profile, _)) = found else {
            let _ = socket.send(Message::Close(Some(axum::extract::ws::CloseFrame { code: 4401, reason: "display ticket missing, expired or used".into() }))).await;
            return;
        };
        run_rfb(m, profile, socket).await;
    })
}

async fn run_rfb(m: Shared, profile: String, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let mut kill = m.drop_tx.subscribe();
    // ServerInit pieces.
    if sink.send(Message::Binary(b"RFB 003.008\n".to_vec().into())).await.is_err() {
        return;
    }
    let mut inbuf: Vec<u8> = Vec::new();
    let mut stage = 0u8; // 0 version, 1 security, 2 clientinit, 3 normal
    let mut zlib = screen::new_zlib();
    let mut want_update = false;
    let mut buttons_prev = 0u8;
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut last_sent_hash = 0u64;
    loop {
        tokio::select! {
            _ = kill.recv() => { let _ = sink.send(Message::Close(None)).await; return; }
            _ = tick.tick() => {
                if stage == 3 && want_update {
                    let (frame, hash) = {
                        let mut g = m.inner.lock().await;
                        let s = g.screens.entry(profile.clone()).or_default();
                        let hash = { use std::hash::{Hash, Hasher}; let mut h = std::collections::hash_map::DefaultHasher::new(); s.email.hash(&mut h); s.password_len.hash(&mut h); s.focus.hash(&mut h); s.signed_in.hash(&mut h); h.finish() };
                        (if hash != last_sent_hash { Some(s.zrle(&mut zlib)) } else { None }, hash)
                    };
                    if let Some(z) = frame {
                        last_sent_hash = hash;
                        want_update = false;
                        let mut msg = vec![0u8, 0, 0, 1];
                        for v in [0u16, 0, screen::W, screen::H] { msg.extend_from_slice(&v.to_be_bytes()); }
                        msg.extend_from_slice(&16i32.to_be_bytes());
                        msg.extend_from_slice(&(z.len() as u32).to_be_bytes());
                        msg.extend_from_slice(&z);
                        if sink.send(Message::Binary(msg.into())).await.is_err() { return; }
                    }
                }
            }
            msg = stream.next() => {
                let Some(Ok(msg)) = msg else { return };
                let data = match msg { Message::Binary(b) => b.to_vec(), Message::Close(_) => return, _ => continue };
                inbuf.extend_from_slice(&data);
                loop {
                    match stage {
                        0 => { if inbuf.len() < 12 { break; } inbuf.drain(..12); stage = 1; let _ = sink.send(Message::Binary(vec![1u8, 1].into())).await; }
                        1 => { if inbuf.is_empty() { break; } inbuf.drain(..1); stage = 2; let _ = sink.send(Message::Binary(0u32.to_be_bytes().to_vec().into())).await; }
                        2 => {
                            if inbuf.is_empty() { break; }
                            inbuf.drain(..1); stage = 3;
                            let mut si = Vec::new();
                            si.extend_from_slice(&screen::W.to_be_bytes()); si.extend_from_slice(&screen::H.to_be_bytes());
                            si.extend_from_slice(&[32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 16, 8, 0, 0, 0, 0]);
                            let name = format!("{profile} screen");
                            si.extend_from_slice(&(name.len() as u32).to_be_bytes()); si.extend_from_slice(name.as_bytes());
                            let _ = sink.send(Message::Binary(si.into())).await;
                        }
                        _ => {
                            if inbuf.is_empty() { break; }
                            let need = match inbuf[0] { 0 => 20, 2 => if inbuf.len() >= 4 { 4 + 4 * u16::from_be_bytes([inbuf[2], inbuf[3]]) as usize } else { usize::MAX }, 3 => 10, 4 => 8, 5 => 6,
                                6 => if inbuf.len() >= 8 { 8 + u32::from_be_bytes([inbuf[4], inbuf[5], inbuf[6], inbuf[7]]).min(1 << 20) as usize } else { usize::MAX }, _ => { return; } };
                            if inbuf.len() < need { break; }
                            let msg: Vec<u8> = inbuf.drain(..need).collect();
                            let human = { let g = m.inner.lock().await; g.lease.get(&profile).is_some_and(|l| l.0 == "human") };
                            match msg[0] {
                                3 => { if msg[1] == 0 { last_sent_hash = 0; } want_update = true; }
                                4 if human && msg[1] == 1 => {
                                    let ks = u32::from_be_bytes([msg[4], msg[5], msg[6], msg[7]]);
                                    m.inner.lock().await.screens.entry(profile.clone()).or_default().key(ks);
                                }
                                5 if human => {
                                    let b = msg[1]; let x = u16::from_be_bytes([msg[2], msg[3]]); let y = u16::from_be_bytes([msg[4], msg[5]]);
                                    let mut g = m.inner.lock().await;
                                    let s = g.screens.entry(profile.clone()).or_default();
                                    s.cursor = (x, y);
                                    if b & 1 == 1 && buttons_prev & 1 == 0 { s.click(x, y); }
                                    buttons_prev = b;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Bind and serve on `addr`. Returns the bound address.
pub async fn serve(addr: &str, speed: f64) -> anyhow::Result<(std::net::SocketAddr, Shared, tokio::task::JoinHandle<()>)> {
    let mock = Mock::new(speed);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local = listener.local_addr()?;
    let app = router(mock.clone());
    let h = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok((local, mock, h))
}
