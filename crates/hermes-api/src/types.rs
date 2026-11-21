//! Wire types for the Hermes API.
//!
//! The shapes follow the hermes-agent v0.23.3 source:
//! `hermes_cli/dashboard_auth/routes.py` (auth), `hermes_cli/web_routers/profiles.py` (REST),
//! and `apps/shared/src/gateway-contract.openrpc.json` (gateway JSON-RPC).
//!
//! Every type parses leniently: unknown keys are ignored and missing keys take a default,
//! so new or partial server data never makes the client fail.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `GET /api/health`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Health {
    pub ok: bool,
    pub version: String,
    pub auth_required: bool,
}

/// `GET /api/status` (public; redacted for an unauthenticated caller).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Status {
    pub version: String,
    pub auth_required: bool,
    pub auth_providers: Vec<String>,
    pub auth_flows: Vec<String>,
    pub gateway_running: bool,
    pub gateway_state: Option<String>,
    pub overall: Option<String>,
}

/// One row of `GET /api/auth/providers`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct AuthProvider {
    pub name: String,
    pub display_name: String,
    pub supports_password: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct AuthProviders {
    pub providers: Vec<AuthProvider>,
}

/// `POST /auth/native/token` and `/auth/native/refresh` body (`_bearer_payload`).
#[derive(Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    /// Unix seconds (the access token's `exp`).
    pub expires_at: i64,
    pub provider: String,
    pub user_id: String,
}

impl std::fmt::Debug for TokenSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenSet")
            .field("access_token", &"***")
            .field("refresh_token", &"***")
            .field("expires_at", &self.expires_at)
            .field("provider", &self.provider)
            .finish()
    }
}

/// `GET /api/auth/me`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Me {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    pub provider: String,
    pub expires_at: i64,
}

/// `POST /api/auth/ws-ticket`.
#[derive(Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct WsTicket {
    pub ticket: String,
    pub ttl_seconds: u64,
}

impl std::fmt::Debug for WsTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WsTicket").field("ticket", &"***").field("ttl_seconds", &self.ttl_seconds).finish()
    }
}

/// Bot Mode metadata: `ui_meta["hermes-bots"]` on a profile (desktop `BotMeta`).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct BotMeta {
    pub title: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub shape: Option<String>,
    pub hidden: Option<bool>,
    pub pinned: Option<bool>,
    #[serde(rename = "sectionName")]
    pub section_name: Option<String>,
    pub created: Option<f64>,
}

/// `ProfileSessionPreview` / `ProfileCanonicalSession` (fields shared by both).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SessionPreview {
    pub id: String,
    pub resolved_id: Option<String>,
    pub title: String,
    pub preview: String,
    pub started_at: f64,
    pub last_active: f64,
    pub message_count: i64,
}

/// `ProfileWorkerSession`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct WorkerSession {
    pub id: String,
    pub source: String,
    pub title: String,
    pub last_active: f64,
}

/// One profile row (`ProfileRow` in the contract; REST `/api/profiles` rows are a superset).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub path: String,
    pub is_default: bool,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub description: String,
    pub display_name: String,
    pub bot_title: String,
    pub skill_count: i64,
    pub gateway_running: bool,
    pub last_session: Option<SessionPreview>,
    pub worker_session: Option<WorkerSession>,
    pub canonical_session: Option<SessionPreview>,
    pub ui_meta: Option<Value>,
    pub has_avatar: bool,
}

impl Profile {
    /// The Bot Mode block, if this profile carries one.
    pub fn bot_meta(&self) -> Option<BotMeta> {
        let block = self.ui_meta.as_ref()?.get("hermes-bots")?;
        if !block.is_object() {
            return None;
        }
        serde_json::from_value(block.clone()).ok()
    }

    /// True when the profile is Bot-Mode managed (has a `hermes-bots` block).
    pub fn is_bot(&self) -> bool {
        self.ui_meta
            .as_ref()
            .and_then(|m| m.get("hermes-bots"))
            .is_some_and(Value::is_object)
    }

    /// The name shown in the roster: Bot title, then display name, then bot_title, then the profile name.
    pub fn title(&self) -> String {
        let meta_title = self.bot_meta().and_then(|m| m.title).unwrap_or_default();
        [meta_title.as_str(), self.display_name.as_str(), self.bot_title.as_str(), self.name.as_str()]
            .into_iter()
            .map(str::trim)
            .find(|s| !s.is_empty())
            .unwrap_or("Bot")
            .to_string()
    }
}

/// `GET /api/profiles` and the `profiles.list` RPC result.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ProfileList {
    pub profiles: Vec<Profile>,
    pub install_id: String,
}

/// `SessionListRow`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SessionRow {
    pub id: String,
    pub resolved_id: Option<String>,
    pub title: String,
    pub preview: String,
    pub started_at: f64,
    pub message_count: i64,
    pub source: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SessionList {
    pub sessions: Vec<SessionRow>,
}

/// `TranscriptMessage`. `text` is the rendered text; tool rows carry `name`, `context`, `args`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct TranscriptMessage {
    pub role: String,
    pub text: Option<String>,
    pub content: Option<Value>,
    pub tool_call_id: Option<String>,
    pub timestamp: Option<f64>,
    pub name: Option<String>,
    pub context: Option<String>,
}

impl TranscriptMessage {
    /// Best text for display: `text`, else a string `content`.
    pub fn display_text(&self) -> String {
        if let Some(t) = &self.text {
            return t.clone();
        }
        match &self.content {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        }
    }
}

/// `session.create` / `session.resume` result (fields this client reads).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SessionOpen {
    pub session_id: String,
    pub stored_session_id: Option<String>,
    pub message_count: i64,
    pub messages: Vec<TranscriptMessage>,
    pub running: Option<bool>,
}

/// `DisplayLease`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct DisplayLease {
    /// `agent` or `human`.
    pub holder: String,
    pub viewer_hash: Option<String>,
    pub since: f64,
    pub epoch: i64,
    pub reason: String,
}

impl DisplayLease {
    pub fn human_holds(&self) -> bool {
        self.holder == "human"
    }
}

/// `DisplayStatus` (and the `display.status` event payload).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct DisplayStatus {
    pub profile: String,
    pub supported: bool,
    pub installed: bool,
    pub missing: Vec<String>,
    pub running: bool,
    pub geometry: String,
    pub blocker: Option<String>,
    pub lease: DisplayLease,
    pub profile_key: String,
}

/// `display.observe` result: a status snapshot plus the single-use ticket.
#[derive(Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct DisplayObserve {
    #[serde(flatten)]
    pub status: DisplayStatus,
    pub ticket: String,
    pub path: String,
    pub viewer_id: String,
}

impl std::fmt::Debug for DisplayObserve {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DisplayObserve")
            .field("status", &self.status)
            .field("ticket", &"***")
            .field("path", &self.path)
            .field("viewer_id", &self.viewer_id)
            .finish()
    }
}

/// `display.lease.acquire` / `.release` result.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct LeaseResult {
    pub lease: DisplayLease,
}

/// `display.thumbnail` result.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Thumbnail {
    pub data_url: Option<String>,
    pub suppressed: Option<String>,
}

/// `prompt.submit` result.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct PromptSubmit {
    /// `streaming`, `queued`, `steered` or `redirected`.
    pub status: Option<String>,
}

/// `profiles.create` result.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ProfileCreated {
    pub ok: bool,
    pub name: String,
    pub path: String,
    pub warnings: Vec<String>,
}

/// A gateway event (JSON-RPC notification `method: "event"`).
#[derive(Debug, Clone, PartialEq)]
pub struct GatewayEvent {
    /// `params.type`, for example `message.delta`.
    pub kind: String,
    pub session_id: Option<String>,
    pub payload: Value,
}

/// Typed view of the gateway events this client renders.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Ready,
    MessageStart,
    MessageDelta { text: String },
    MessageComplete { text: Option<String>, status: Option<String>, error: Option<String> },
    ThinkingDelta { text: String },
    ToolStart { tool_id: String, name: String, context: Option<String> },
    ToolProgress { tool_id: String, name: String, message: Option<String>, percent: Option<i64> },
    ToolComplete { tool_id: String, name: String, summary: Option<String>, duration_s: Option<f64>, failed: bool },
    StatusUpdate { kind: String, text: String },
    DisplayStatus(DisplayStatus),
    DisplayLease { profile_key: String, lease: DisplayLease },
    SessionsChanged,
    Notice { message: String },
    Error { message: String },
    Other,
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

impl GatewayEvent {
    /// Map the raw event to a typed [`Event`]. Malformed payloads map to `Other`.
    pub fn typed(&self) -> Event {
        let p = &self.payload;
        match self.kind.as_str() {
            "gateway.ready" => Event::Ready,
            "message.start" => Event::MessageStart,
            "message.delta" => match p.get("text") {
                Some(Value::String(t)) => Event::MessageDelta { text: t.clone() },
                // `null` text is a segment boundary.
                _ => Event::Other,
            },
            "message.complete" => Event::MessageComplete {
                text: str_of(p, "text"),
                status: str_of(p, "status"),
                error: str_of(p, "error"),
            },
            "thinking.delta" | "reasoning.delta" => match str_of(p, "text") {
                Some(text) => Event::ThinkingDelta { text },
                None => Event::Other,
            },
            "tool.start" => Event::ToolStart {
                tool_id: str_of(p, "tool_id").unwrap_or_default(),
                name: str_of(p, "name").unwrap_or_else(|| "tool".into()),
                context: str_of(p, "context").or_else(|| str_of(p, "preview")),
            },
            "tool.progress" => Event::ToolProgress {
                tool_id: str_of(p, "tool_id").unwrap_or_default(),
                name: str_of(p, "name").unwrap_or_default(),
                message: str_of(p, "message"),
                percent: p.get("percent").and_then(Value::as_i64),
            },
            "tool.complete" => {
                let failed = p.get("result").and_then(|r| r.get("error")).is_some_and(|e| !e.is_null())
                    || p.get("labels").and_then(Value::as_array).is_some_and(|l| {
                        l.iter().any(|x| x.as_str() == Some("error") || x.as_str() == Some("failed"))
                    });
                Event::ToolComplete {
                    tool_id: str_of(p, "tool_id").unwrap_or_default(),
                    name: str_of(p, "name").unwrap_or_default(),
                    summary: str_of(p, "summary"),
                    duration_s: p.get("duration_s").and_then(Value::as_f64),
                    failed,
                }
            }
            "status.update" => Event::StatusUpdate {
                kind: str_of(p, "kind").unwrap_or_default(),
                text: str_of(p, "text").unwrap_or_default(),
            },
            "display.status" => match serde_json::from_value(p.clone()) {
                Ok(s) => Event::DisplayStatus(s),
                Err(_) => Event::Other,
            },
            "display.lease" => Event::DisplayLease {
                profile_key: str_of(p, "profile_key").unwrap_or_default(),
                lease: p.get("lease").and_then(|l| serde_json::from_value(l.clone()).ok()).unwrap_or_default(),
            },
            "sessions.changed" => Event::SessionsChanged,
            "notice" => Event::Notice { message: str_of(p, "message").unwrap_or_default() },
            "error" => Event::Error { message: str_of(p, "message").unwrap_or_else(|| "Unknown error".into()) },
            _ => Event::Other,
        }
    }
}

/// A server -> client JSON-RPC request (for example `approval`), which needs an answer.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerRequest {
    pub id: Value,
    pub method: String,
    pub session_id: Option<String>,
    pub params: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn profile_title_and_meta() {
        let p: Profile = serde_json::from_value(json!({
            "name": "researcher", "path": "/x",
            "ui_meta": {"hermes-bots": {"title": "Research Buddy", "color": "#38bdf8"}},
            "unknown_future_key": 42
        }))
        .unwrap();
        assert!(p.is_bot());
        assert_eq!(p.title(), "Research Buddy");
        assert_eq!(p.bot_meta().unwrap().color.as_deref(), Some("#38bdf8"));
    }

    #[test]
    fn profile_tolerates_bad_meta() {
        let p: Profile = serde_json::from_value(json!({"name": "x", "ui_meta": {"hermes-bots": "oops"}})).unwrap();
        assert!(!p.is_bot());
        assert!(p.bot_meta().is_none());
        assert_eq!(p.title(), "x");
        let p: Profile = serde_json::from_value(json!({})).unwrap();
        assert_eq!(p.title(), "Bot");
    }

    #[test]
    fn events_map() {
        let ev = GatewayEvent { kind: "message.delta".into(), session_id: None, payload: json!({"text": "hi"}) };
        assert_eq!(ev.typed(), Event::MessageDelta { text: "hi".into() });
        let ev = GatewayEvent { kind: "message.delta".into(), session_id: None, payload: json!({"text": null}) };
        assert_eq!(ev.typed(), Event::Other);
        let ev = GatewayEvent { kind: "tool.start".into(), session_id: None, payload: json!(5) };
        assert!(matches!(ev.typed(), Event::ToolStart { .. }));
        let ev = GatewayEvent { kind: "display.status".into(), session_id: None, payload: json!("bad") };
        assert_eq!(ev.typed(), Event::Other);
    }

    #[test]
    fn token_debug_hides_secrets() {
        let t = TokenSet { access_token: "AT".into(), refresh_token: "RT".into(), ..Default::default() };
        let s = format!("{t:?}");
        assert!(!s.contains("AT\"") && !s.contains("RT\""));
    }
}
