//! The background connection worker.
//!
//! One worker per signed-in gateway. It keeps the `/api/ws` socket open (reconnect with backoff,
//! heartbeat), refreshes tokens on 401, polls the Bot list, and forwards events to the UI as
//! [`NetEvent`]s. The UI sends [`Command`]s back through an unbounded channel.

use crate::store;
use hermes_api::backoff::Backoff;
use hermes_api::gateway::{Gateway, GatewayError, Incoming};
use hermes_api::rfb::{RfbEvent, RfbSession};
use hermes_api::types::*;
use hermes_api::{ApiError, RestClient};
use iced::futures::channel::mpsc as fmpsc;
use iced::futures::{SinkExt, Stream};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

/// Commands from the UI to the worker.
#[derive(Debug, Clone)]
pub enum Command {
    OpenChat {
        profile: String,
    },
    Send {
        profile: String,
        session_id: String,
        text: String,
    },
    Stop {
        profile: String,
        session_id: String,
        generation: Option<i64>,
    },
    /// `rpc_id`: set for a legacy server request (answered in place); None for a canonical `approval.request` event.
    Approve {
        session_id: String,
        request_id: String,
        choice: String,
        rpc_id: Option<Value>,
        generation: Option<i64>,
    },
    ScreenStatus {
        profile: String,
    },
    ScreenStart {
        profile: String,
    },
    ScreenWatch {
        profile: String,
        viewer_id: Option<String>,
    },
    ScreenClose,
    TakeControl {
        profile: String,
        viewer_id: String,
    },
    HandBack {
        profile: String,
        viewer_id: String,
    },
    /// Raw RFB client messages (key and pointer events).
    ScreenInput(Vec<u8>),
    CreateBot {
        name: String,
        title: String,
        description: String,
        color: String,
    },
    SignOut,
}

/// Events from the worker to the UI.
#[derive(Debug, Clone)]
pub enum NetEvent {
    /// The worker is ready; the sender takes commands.
    Ready(mpsc::UnboundedSender<Command>),
    Connection(ConnState),
    Me(Me),
    Bots(Vec<Profile>),
    ChatOpened {
        profile: String,
        chat: SessionOpen,
    },
    ChatError {
        profile: String,
        message: String,
    },
    Gateway(GatewayEvent),
    ServerRequest(ServerRequest),
    SendFailed {
        profile: String,
        message: String,
    },
    Screen {
        profile: String,
        status: DisplayStatus,
    },
    ScreenObserved {
        profile: String,
        viewer_id: String,
    },
    ScreenRfb(RfbEvent),
    ScreenError {
        profile: String,
        message: String,
    },
    Lease {
        profile: String,
        lease: DisplayLease,
    },
    BotCreated {
        name: String,
    },
    CreateFailed(String),
    /// The session cannot be refreshed; the user must sign in again.
    SignedOut {
        expired: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnState {
    Connecting,
    Online,
    /// Lost; the worker retries in `retry_in` seconds.
    Offline {
        reason: String,
        retry_in: u64,
    },
}

/// The tokens and gateway the worker runs with.
#[derive(Debug, Clone)]
pub struct Session {
    pub gateway: url::Url,
    pub tokens: TokenSet,
}

impl std::hash::Hash for Session {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.gateway.as_str().hash(state);
        self.tokens.provider.hash(state);
        // Do not hash the token value (it must not leave this struct); expires_at changes on new sign-in.
        self.tokens.expires_at.hash(state);
    }
}

/// The iced subscription stream for one session.
pub fn run(session: &Session) -> impl Stream<Item = NetEvent> + use<> {
    let session = session.clone();
    iced::stream::channel(256, async move |out: fmpsc::Sender<NetEvent>| {
        worker(session, out).await;
    })
}

struct Worker {
    rest: RestClient,
    out: fmpsc::Sender<NetEvent>,
    gw: Option<Gateway>,
    /// Live session id -> profile (to route events).
    sessions: HashMap<String, String>,
    rfb: Option<RfbSession>,
    rfb_tx: mpsc::Sender<RfbEvent>,
}

impl Worker {
    async fn emit(&mut self, e: NetEvent) {
        let _ = self.out.send(e).await;
    }

    /// Persist rotated tokens after a refresh.
    async fn save_tokens(&self) {
        if let Some(t) = self.rest.current_tokens().await {
            store::save_tokens(self.rest.base().as_str(), &t);
        }
    }

    async fn refresh_or_sign_out(&mut self) -> bool {
        match self.rest.refresh().await {
            Ok(_) => {
                self.save_tokens().await;
                true
            }
            Err(ApiError::Unauthorized { .. }) => {
                store::delete_tokens(self.rest.base().as_str());
                self.emit(NetEvent::SignedOut { expired: true }).await;
                false
            }
            Err(_) => false,
        }
    }

    async fn load_bots(&mut self) {
        let res = match &self.gw {
            Some(gw) => gw.profiles_list().await.map_err(|e| e.to_string()),
            None => Err("offline".into()),
        };
        let list = match res {
            Ok(l) => Ok(l),
            // Fall back to REST if the RPC is not there.
            Err(_) => self.rest.with_refresh(|c| async move { c.profiles().await }).await.map_err(|e| e.to_string()),
        };
        if let Ok(list) = list {
            self.emit(NetEvent::Bots(list.profiles)).await;
        }
    }

    async fn handle(&mut self, cmd: Command) {
        let Some(gw) = self.gw.clone() else {
            match cmd {
                Command::Send { profile, .. } => self.emit(NetEvent::SendFailed { profile, message: "You are offline. The message was not sent.".into() }).await,
                Command::SignOut => self.sign_out().await,
                Command::ScreenInput(_) => {}
                _ => {}
            }
            return;
        };
        match cmd {
            Command::OpenChat { profile } => match gw.open_bot_chat(&profile).await {
                Ok(chat) => {
                    self.sessions.insert(chat.session_id.clone(), profile.clone());
                    self.emit(NetEvent::ChatOpened { profile, chat }).await;
                }
                Err(e) => self.emit(NetEvent::ChatError { profile, message: e.to_string() }).await,
            },
            Command::Send { profile, session_id, text } => {
                if let Err(e) = gw.prompt_submit(&session_id, &profile, &text).await {
                    self.emit(NetEvent::SendFailed { profile, message: e.to_string() }).await;
                }
            }
            Command::Stop { profile, session_id, generation } => {
                let _ = gw.interrupt(&session_id, &profile, generation).await;
            }
            Command::Approve { session_id, request_id, choice, rpc_id, generation } => match rpc_id {
                // Legacy gateway: answer its server request in place.
                Some(id) => {
                    let _ = gw.respond(id, json!({"choice": choice})).await;
                }
                // Canonical gateway: approval.respond with the prompt id and generation.
                None => {
                    let _ = gw.approval_respond(&session_id, &request_id, &choice, generation).await;
                }
            },
            Command::ScreenStatus { profile } => match gw.display_status(&profile).await {
                Ok(status) => self.emit(NetEvent::Screen { profile, status }).await,
                Err(e) => self.emit(NetEvent::ScreenError { profile, message: e.to_string() }).await,
            },
            Command::ScreenStart { profile } => match gw.display_start(&profile).await {
                Ok(status) => self.emit(NetEvent::Screen { profile, status }).await,
                Err(e) => self.emit(NetEvent::ScreenError { profile, message: e.to_string() }).await,
            },
            Command::ScreenWatch { profile, viewer_id } => {
                self.rfb = None;
                match gw.display_observe(&profile, viewer_id.as_deref()).await {
                    Ok(obs) => {
                        let mut url = hermes_api::url_util::ws_join(self.rest.base(), if obs.path.is_empty() { "/api/display/ws" } else { &obs.path });
                        url.query_pairs_mut().append_pair("display_ticket", &obs.ticket);
                        self.emit(NetEvent::Screen { profile: profile.clone(), status: obs.status.clone() }).await;
                        self.emit(NetEvent::ScreenObserved { profile: profile.clone(), viewer_id: obs.viewer_id.clone() }).await;
                        match RfbSession::connect(url.as_str(), self.rfb_tx.clone()).await {
                            Ok(s) => self.rfb = Some(s),
                            Err(e) => self.emit(NetEvent::ScreenError { profile, message: e.to_string() }).await,
                        }
                    }
                    Err(e) => self.emit(NetEvent::ScreenError { profile, message: e.to_string() }).await,
                }
            }
            Command::ScreenClose => self.rfb = None,
            Command::TakeControl { profile, viewer_id } => match gw.lease_acquire(&profile, &viewer_id).await {
                Ok(r) => self.emit(NetEvent::Lease { profile, lease: r.lease }).await,
                Err(e) => self.emit(NetEvent::ScreenError { profile, message: e.to_string() }).await,
            },
            Command::HandBack { profile, viewer_id } => match gw.lease_release(&profile, &viewer_id).await {
                Ok(r) => self.emit(NetEvent::Lease { profile, lease: r.lease }).await,
                Err(e) => self.emit(NetEvent::ScreenError { profile, message: e.to_string() }).await,
            },
            Command::ScreenInput(bytes) => {
                if let Some(s) = &self.rfb {
                    let _ = s.input.try_send(bytes);
                }
            }
            Command::CreateBot { name, title, description, color } => match gw.profiles_create(&name, &description).await {
                Ok(c) => {
                    let meta = json!({"title": title, "color": color, "description": description, "created": chrono_ms()});
                    let _ = gw.profiles_configure_meta(&c.name, meta).await;
                    self.emit(NetEvent::BotCreated { name: c.name }).await;
                    self.load_bots().await;
                }
                Err(e) => self.emit(NetEvent::CreateFailed(e.to_string())).await,
            },
            Command::SignOut => self.sign_out().await,
        }
    }

    async fn sign_out(&mut self) {
        if let Some(gw) = self.gw.take() {
            gw.close().await;
        }
        self.rfb = None;
        self.rest.clear_tokens().await;
        store::delete_tokens(self.rest.base().as_str());
        self.emit(NetEvent::SignedOut { expired: false }).await;
    }
}

fn chrono_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

async fn worker(session: Session, out: fmpsc::Sender<NetEvent>) {
    let rest = RestClient::new(session.gateway.clone());
    rest.set_tokens(&session.tokens).await;
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (rfb_tx, mut rfb_rx) = mpsc::channel::<RfbEvent>(4);
    let mut w = Worker { rest, out, gw: None, sessions: HashMap::new(), rfb: None, rfb_tx };
    w.emit(NetEvent::Ready(cmd_tx)).await;

    let mut backoff = Backoff::new(Duration::from_millis(500), Duration::from_secs(30));
    'outer: loop {
        w.emit(NetEvent::Connection(ConnState::Connecting)).await;
        // Check the token first; refresh if needed.
        match w.rest.me().await {
            Ok(me) => w.emit(NetEvent::Me(me)).await,
            Err(ApiError::Unauthorized { .. }) => {
                if !w.refresh_or_sign_out().await && (w.rest.refresh_token().await.is_none() || !w.rest.has_token().await) {
                    // Signed out; wait for the UI to drop this subscription.
                    futures::future::pending::<()>().await;
                }
                // Refresh did not work for a network reason: retry later.
            }
            Err(_) => {}
        }
        let conn = Gateway::connect(&w.rest).await;
        let (gw, mut inc) = match conn {
            Ok(x) => x,
            Err(e) => {
                if matches!(e, GatewayError::Api(ApiError::Unauthorized { .. })) && !w.refresh_or_sign_out().await {
                    futures::future::pending::<()>().await;
                }
                let delay = backoff.next_delay();
                w.emit(NetEvent::Connection(ConnState::Offline { reason: e.to_string(), retry_in: delay.as_secs().max(1) })).await;
                // Keep serving commands while we wait (they fail fast while offline).
                let sleep = tokio::time::sleep(delay);
                tokio::pin!(sleep);
                loop {
                    tokio::select! {
                        _ = &mut sleep => continue 'outer,
                        Some(c) = cmd_rx.recv() => w.handle(c).await,
                        Some(ev) = rfb_rx.recv() => w.emit(NetEvent::ScreenRfb(ev)).await,
                    }
                }
            }
        };
        backoff.reset();
        w.gw = Some(gw.clone());
        w.emit(NetEvent::Connection(ConnState::Online)).await;
        w.load_bots().await;

        let mut heartbeat = tokio::time::interval(Duration::from_secs(20));
        heartbeat.tick().await;
        let mut roster_poll = tokio::time::interval(Duration::from_secs(30));
        roster_poll.tick().await;
        let close_reason = loop {
            tokio::select! {
                item = inc.recv() => match item {
                    Some(Incoming::Event(e)) => {
                        match e.typed() {
                            Event::SessionsChanged => { w.load_bots().await; }
                            Event::DisplayLease { profile_key, lease } => {
                                let profile = profile_key.rsplit('/').next().unwrap_or_default().to_string();
                                w.emit(NetEvent::Lease { profile, lease }).await;
                            }
                            _ => {}
                        }
                        w.emit(NetEvent::Gateway(e)).await;
                    }
                    Some(Incoming::Request(r)) => w.emit(NetEvent::ServerRequest(r)).await,
                    Some(Incoming::Closed { code, reason }) => break format!("closed ({}) {}", code.map(|c| c.to_string()).unwrap_or_default(), reason),
                    None => break "closed".to_string(),
                },
                Some(c) = cmd_rx.recv() => w.handle(c).await,
                Some(ev) = rfb_rx.recv() => {
                    if matches!(ev, RfbEvent::Closed { .. }) { w.rfb = None; }
                    w.emit(NetEvent::ScreenRfb(ev)).await;
                }
                _ = heartbeat.tick() => {
                    if let Err(e) = gw.ping().await { break e.to_string(); }
                }
                _ = roster_poll.tick() => w.load_bots().await,
            }
        };
        w.gw = None;
        w.sessions.clear();
        let delay = backoff.next_delay();
        tracing::info!("gateway connection lost: {close_reason}");
        w.emit(NetEvent::Connection(ConnState::Offline { reason: "The live connection dropped".into(), retry_in: delay.as_secs().max(1) })).await;
        tokio::time::sleep(delay).await;
    }
}
