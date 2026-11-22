//! Gateway JSON-RPC over `/api/ws` (hermes-agent `tui_gateway/ws.py`).
//!
//! Frames are JSON-RPC 2.0 text frames:
//! * client request:  `{"jsonrpc":"2.0","id":N,"method":"session.create","params":{...}}`
//! * server response: `{"jsonrpc":"2.0","id":N,"result":{...}}` or `{"error":{"code","message"}}`
//! * server event:    `{"jsonrpc":"2.0","method":"event","params":{"type":"message.delta","session_id":..,"payload":{..}}}`
//! * server request:  `{"jsonrpc":"2.0","id":"str","method":"approval","params":{...}}` (needs a reply)
//!
//! The socket is opened with a single-use ticket from `POST /api/auth/ws-ticket` as `?ticket=`.

use crate::redact::redact_url;
use crate::rest::{ApiError, RestClient};
use crate::types::*;
use futures::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

/// Largest frame we accept from the server (bounded memory).
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
/// How long an RPC may wait for its reply.
pub const RPC_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum GatewayError {
    #[error("{0}")]
    Api(#[from] ApiError),
    #[error("Cannot open the live connection: {0}")]
    Connect(String),
    #[error("The live connection closed")]
    Closed,
    #[error("The gateway did not answer in time")]
    Timeout,
    #[error("The gateway refused the request: {message}")]
    Rpc { code: i64, message: String },
    #[error("The gateway sent a reply this app cannot read")]
    BadReply,
}

/// Something that arrives from the gateway outside of an RPC reply.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Event(GatewayEvent),
    Request(ServerRequest),
    /// The socket closed. `code` is the WebSocket close code if any.
    Closed {
        code: Option<u16>,
        reason: String,
    },
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, GatewayError>>>>>;

/// A live gateway connection. Cheap to clone; all clones share one socket.
#[derive(Clone)]
pub struct Gateway {
    out: mpsc::Sender<WsMessage>,
    pending: Pending,
    next_id: Arc<AtomicU64>,
    /// Set by the reader task when the socket ends.
    closed: Arc<std::sync::atomic::AtomicBool>,
}

/// Parse one text frame from the server. Pure, so it can be unit-tested.
pub fn parse_frame(text: &str) -> Option<ParsedFrame> {
    let v: Value = serde_json::from_str(text).ok()?;
    let obj = v.as_object()?;
    let method = obj.get("method").and_then(Value::as_str);
    match method {
        Some("event") => {
            let params = obj.get("params")?;
            let kind = params.get("type")?.as_str()?.to_string();
            let session_id = params.get("session_id").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
            let payload = params.get("payload").cloned().unwrap_or(Value::Null);
            let execution_generation = params.get("execution_generation").and_then(Value::as_i64);
            Some(ParsedFrame::Event(GatewayEvent { kind, session_id, payload, execution_generation }))
        }
        Some(m) => {
            let id = obj.get("id")?.clone();
            let params = obj.get("params").cloned().unwrap_or(Value::Null);
            let session_id = params.get("session_id").and_then(Value::as_str).map(str::to_string);
            Some(ParsedFrame::Request(ServerRequest { id, method: m.to_string(), session_id, params }))
        }
        None => {
            let id = obj.get("id")?.as_u64()?;
            if let Some(err) = obj.get("error") {
                let code = err.get("code").and_then(Value::as_i64).unwrap_or(-1);
                let message = err.get("message").and_then(Value::as_str).unwrap_or("error").chars().take(300).collect();
                Some(ParsedFrame::Response(id, Err(GatewayError::Rpc { code, message })))
            } else {
                Some(ParsedFrame::Response(id, Ok(obj.get("result").cloned().unwrap_or(Value::Null))))
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ParsedFrame {
    Event(GatewayEvent),
    Request(ServerRequest),
    Response(u64, Result<Value, GatewayError>),
}

impl Gateway {
    /// Mint a ticket and open `/api/ws`. Returns the gateway handle and a stream of incoming
    /// events. The caller owns reconnection (see `connect_with_retry` in the app).
    pub async fn connect(rest: &RestClient) -> Result<(Gateway, mpsc::Receiver<Incoming>), GatewayError> {
        let ticket = rest.ws_ticket().await?;
        let mut url = crate::url_util::ws_join(rest.base(), "/api/ws");
        url.query_pairs_mut().append_pair("ticket", &ticket.ticket);
        Self::connect_url(url.as_str()).await
    }

    /// Open a gateway socket at an explicit URL (used by tests and by `connect`).
    pub async fn connect_url(url: &str) -> Result<(Gateway, mpsc::Receiver<Incoming>), GatewayError> {
        let req = url.into_client_request().map_err(|e| GatewayError::Connect(redact_url(&e.to_string())))?;
        let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default().max_message_size(Some(MAX_FRAME_BYTES)).max_frame_size(Some(MAX_FRAME_BYTES));
        let connect = tokio_tungstenite::connect_async_with_config(req, Some(config), true);
        let (ws, _) = tokio::time::timeout(Duration::from_secs(15), connect)
            .await
            .map_err(|_| GatewayError::Connect("timed out".into()))?
            .map_err(|e| GatewayError::Connect(redact_url(&e.to_string()).chars().take(200).collect()))?;
        let (mut sink, mut stream) = ws.split();
        let (out_tx, mut out_rx) = mpsc::channel::<WsMessage>(256);
        // Bounded: a slow UI drops nothing silently, it applies back-pressure to the socket reader.
        let (in_tx, in_rx) = mpsc::channel::<Incoming>(1024);
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));

        // Writer task.
        tokio::spawn(async move {
            while let Some(msg) = out_rx.recv().await {
                if sink.send(msg).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });

        // Reader task.
        let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let closed_r = closed.clone();
        let pending_r = pending.clone();
        let out_for_pong = out_tx.clone();
        tokio::spawn(async move {
            let mut close_code = None;
            let mut close_reason = String::new();
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(WsMessage::Text(t)) => match parse_frame(t.as_str()) {
                        Some(ParsedFrame::Response(id, r)) => {
                            if let Some(tx) = pending_r.lock().await.remove(&id) {
                                let _ = tx.send(r);
                            }
                        }
                        Some(ParsedFrame::Event(e)) => {
                            if in_tx.send(Incoming::Event(e)).await.is_err() {
                                break;
                            }
                        }
                        Some(ParsedFrame::Request(r)) => {
                            if in_tx.send(Incoming::Request(r)).await.is_err() {
                                break;
                            }
                        }
                        None => tracing::debug!("ignored an unreadable gateway frame"),
                    },
                    Ok(WsMessage::Ping(p)) => {
                        let _ = out_for_pong.send(WsMessage::Pong(p)).await;
                    }
                    Ok(WsMessage::Close(frame)) => {
                        if let Some(f) = frame {
                            close_code = Some(u16::from(f.code));
                            close_reason = f.reason.to_string();
                        }
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        close_reason = e.to_string().chars().take(160).collect();
                        break;
                    }
                }
            }
            // Fail every pending call so no caller waits forever.
            closed_r.store(true, Ordering::SeqCst);
            for (_, tx) in pending_r.lock().await.drain() {
                let _ = tx.send(Err(GatewayError::Closed));
            }
            let _ = in_tx.send(Incoming::Closed { code: close_code, reason: close_reason }).await;
        });

        let gw = Gateway { out: out_tx, pending, next_id: Arc::new(AtomicU64::new(1)), closed };
        Ok((gw, in_rx))
    }

    /// Send a request and wait for its result.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, GatewayError> {
        if self.is_closed() {
            return Err(GatewayError::Closed);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        // The reader may have closed between the check and the insert.
        if self.is_closed() {
            self.pending.lock().await.remove(&id);
            return Err(GatewayError::Closed);
        }
        let frame = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        if self.out.send(WsMessage::Text(frame.to_string().into())).await.is_err() {
            self.pending.lock().await.remove(&id);
            return Err(GatewayError::Closed);
        }
        match tokio::time::timeout(RPC_TIMEOUT, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err(GatewayError::Closed),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(GatewayError::Timeout)
            }
        }
    }

    /// `call` and decode the result.
    pub async fn call_as<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T, GatewayError> {
        let v = self.call(method, params).await?;
        serde_json::from_value(v).map_err(|_| GatewayError::BadReply)
    }

    /// Answer a server -> client request.
    pub async fn respond(&self, id: Value, result: Value) -> Result<(), GatewayError> {
        let frame = json!({"jsonrpc": "2.0", "id": id, "result": result});
        self.out.send(WsMessage::Text(frame.to_string().into())).await.map_err(|_| GatewayError::Closed)
    }

    /// Heartbeat (`gateway.ping`, answered by the socket loop itself).
    pub async fn ping(&self) -> Result<(), GatewayError> {
        self.call("gateway.ping", json!({})).await.map(|_| ())
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst) || self.out.is_closed()
    }

    /// Close the socket.
    pub async fn close(&self) {
        let _ = self.out.send(WsMessage::Close(None)).await;
    }

    // ---- typed helpers for the methods this app uses ----

    pub async fn profiles_list(&self) -> Result<ProfileList, GatewayError> {
        self.call_as("profiles.list", json!({"include_sessions": true})).await
    }

    /// Find the canonical "Bot Chat" session of a profile.
    pub async fn find_bot_chat(&self, profile: &str) -> Result<Option<SessionRow>, GatewayError> {
        let list: SessionList = self.call_as("session.list", json!({"profile": profile, "title": BOT_CHAT_TITLE, "limit": 50, "include_hidden": true})).await?;
        Ok(list.sessions.into_iter().find(|s| s.title == BOT_CHAT_TITLE))
    }

    /// Open (resume or create) the Bot Chat of a profile. Returns the live session.
    pub async fn open_bot_chat(&self, profile: &str) -> Result<SessionOpen, GatewayError> {
        if let Some(row) = self.find_bot_chat(profile).await? {
            let sid = row.resolved_id.clone().filter(|s| !s.is_empty()).unwrap_or(row.id.clone());
            return self.call_as("session.resume", json!({"session_id": sid, "profile": profile, "display_projection": true})).await;
        }
        self.call_as("session.create", json!({"profile": profile, "title": BOT_CHAT_TITLE, "hidden": true, "follow_profile_config": true})).await
    }

    /// Send a message. Canonical gateways need a client `submission_id` (idempotency key) and
    /// refuse unknown keys, so this sends exactly `session_id`, `text` and `submission_id`. The
    /// session id already names the profile (the socket opened it).
    pub async fn prompt_submit(&self, session_id: &str, _profile: &str, text: &str) -> Result<PromptSubmit, GatewayError> {
        let submission_id = new_submission_id();
        self.call_as("prompt.submit", json!({"session_id": session_id, "text": text, "submission_id": submission_id})).await
    }

    /// Stop the running turn. Canonical gateways need the execution generation.
    pub async fn interrupt(&self, session_id: &str, _profile: &str, generation: Option<i64>) -> Result<Value, GatewayError> {
        let mut p = json!({"session_id": session_id});
        if let Some(g) = generation {
            p["execution_generation"] = json!(g);
        }
        self.call("session.interrupt", p).await
    }

    /// Answer an approval prompt (`CanonicalApprovalParams`).
    pub async fn approval_respond(&self, session_id: &str, prompt_id: &str, choice: &str, generation: Option<i64>) -> Result<Value, GatewayError> {
        let mut p = json!({"session_id": session_id, "prompt_id": prompt_id, "choice": choice});
        if let Some(g) = generation {
            p["execution_generation"] = json!(g);
        }
        self.call("approval.respond", p).await
    }

    pub async fn display_status(&self, profile: &str) -> Result<DisplayStatus, GatewayError> {
        self.call_as("display.status", json!({"profile": profile})).await
    }

    pub async fn display_start(&self, profile: &str) -> Result<DisplayStatus, GatewayError> {
        self.call_as("display.start", json!({"profile": profile})).await
    }

    pub async fn display_observe(&self, profile: &str, viewer_id: Option<&str>) -> Result<DisplayObserve, GatewayError> {
        let mut p = json!({"profile": profile});
        if let Some(v) = viewer_id {
            p["viewer_id"] = json!(v);
        }
        self.call_as("display.observe", p).await
    }

    pub async fn lease_acquire(&self, profile: &str, viewer_id: &str) -> Result<LeaseResult, GatewayError> {
        self.call_as("display.lease.acquire", json!({"profile": profile, "viewer_id": viewer_id, "reason": "sign-in"})).await
    }

    pub async fn lease_release(&self, profile: &str, viewer_id: &str) -> Result<LeaseResult, GatewayError> {
        self.call_as("display.lease.release", json!({"profile": profile, "viewer_id": viewer_id})).await
    }

    pub async fn thumbnail(&self, profile: &str) -> Result<Thumbnail, GatewayError> {
        self.call_as("display.thumbnail", json!({"profile": profile})).await
    }

    pub async fn profiles_create(&self, name: &str, description: &str) -> Result<ProfileCreated, GatewayError> {
        self.call_as("profiles.create", json!({"name": name, "description": description})).await
    }

    pub async fn profiles_configure_meta(&self, profile: &str, meta: Value) -> Result<Value, GatewayError> {
        self.call("profiles.configure", json!({"profile": profile, "ui_meta": {"hermes-bots": meta}})).await
    }
}

/// A random client submission id (prompt.submit idempotency key).
pub fn new_submission_id() -> String {
    let mut b = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut b);
    format!("hbd-{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>())
}

/// The canonical Bot Chat title (`apps/desktop/src/plugins/hermes-bots/canonical-chat.ts`).
pub const BOT_CHAT_TITLE: &str = "Bot Chat";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frames() {
        let f = parse_frame(r#"{"jsonrpc":"2.0","id":3,"result":{"ok":true}}"#).unwrap();
        assert_eq!(f, ParsedFrame::Response(3, Ok(json!({"ok": true}))));
        let f = parse_frame(r#"{"jsonrpc":"2.0","id":4,"error":{"code":4000,"message":"bad"}}"#).unwrap();
        assert!(matches!(f, ParsedFrame::Response(4, Err(GatewayError::Rpc { code: 4000, .. }))));
        let f = parse_frame(r#"{"jsonrpc":"2.0","method":"event","params":{"type":"message.delta","session_id":"s","payload":{"text":"x"}}}"#).unwrap();
        match f {
            ParsedFrame::Event(e) => {
                assert_eq!(e.kind, "message.delta");
                assert_eq!(e.session_id.as_deref(), Some("s"));
            }
            _ => panic!("expected event"),
        }
        let f = parse_frame(r#"{"jsonrpc":"2.0","id":"r1","method":"approval","params":{"session_id":"s"}}"#).unwrap();
        assert!(matches!(f, ParsedFrame::Request(ServerRequest { ref method, .. }) if method == "approval"));
        assert!(parse_frame("not json").is_none());
        assert!(parse_frame("[1,2]").is_none());
        assert!(parse_frame(r#"{"jsonrpc":"2.0","method":"event","params":{}}"#).is_none());
    }
}
