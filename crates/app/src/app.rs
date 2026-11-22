//! Application state and update logic.

use crate::model::*;
use crate::net::{self, Command, ConnState, NetEvent};
use crate::{signin, store};
use hermes_api::rfb::{self, RfbEvent};
use hermes_api::types::*;
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{image, text_editor};
use iced::{Subscription, Task};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    /// Choose the gateway and sign in.
    SignIn(SignInState),
    /// Signed in.
    Main,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SignInState {
    pub url_input: String,
    pub checking: bool,
    pub waiting_browser: bool,
    pub authorize_url: Option<String>,
    pub status: Option<String>,
    pub error: Option<String>,
    pub expired_notice: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailsTab {
    Computer,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPane {
    General,
    Connection,
    About,
}

#[derive(Debug, Clone, Default)]
pub struct CreateForm {
    pub name: String,
    pub title: String,
    pub description: String,
    pub color: usize,
    pub busy: bool,
    pub error: Option<String>,
}

pub const SWATCHES: [&str; 8] = ["#4a84fe", "#e0a14a", "#5fc88f", "#c86fd8", "#5fb8c8", "#e06a6a", "#9fc85f", "#8b5cf6"];

pub struct App {
    pub settings: store::Settings,
    /// `HERMES_GATEWAY_URL` at start. It is used for this run only and never saved.
    pub env_gateway: Option<String>,
    /// The gateway of the sign-in that is in progress.
    pub pending_gateway: Option<url::Url>,
    pub phase: Phase,
    pub session: Option<net::Session>,
    pub cmd: Option<UnboundedSender<Command>>,
    pub conn: ConnState,
    pub me: Option<Me>,
    pub bots: Vec<Bot>,
    pub bots_loaded: bool,
    pub statuses: Statuses,
    pub unread: HashMap<String, bool>,
    pub selected: Option<String>,
    pub chats: HashMap<String, Chat>,
    /// session id -> profile
    pub session_index: HashMap<String, String>,
    pub composer: text_editor::Content,
    pub search: String,
    pub details_open: bool,
    pub details_tab: DetailsTab,
    pub screen: ScreenState,
    pub settings_open: Option<SettingsPane>,
    pub create: Option<CreateForm>,
    pub wallpaper: image::Handle,
    pub phase_t: f32,
    pub toast: Option<String>,
    pub modifiers: keyboard::Modifiers,
}

#[derive(Debug, Clone)]
pub enum Msg {
    // sign-in
    UrlInput(String),
    SignInPressed,
    Probed(Result<Status, String>),
    SignInStarted(signin::Started),
    SignedIn(Result<TokenSet, String>),
    OpenBrowserAgain,
    CancelSignIn,
    // net
    Net(NetEvent),
    // roster
    Search(String),
    SelectBot(String),
    NewBot,
    // chat
    Composer(text_editor::Action),
    Send,
    Stop,
    ToggleActivity(usize),
    Approve(usize, String),
    RetryChat,
    // details / screen
    ToggleDetails,
    DetailsTab(DetailsTab),
    WakeScreen,
    OpenScreen { take_over: bool },
    CloseScreen,
    TakeControl,
    HandBack,
    ScreenBytes(Vec<Vec<u8>>),
    // settings
    OpenSettings(SettingsPane),
    CloseSettings,
    ToggleWallpaper,
    ToggleReduceMotion,
    SignOut,
    // create
    CreateTitle(String),
    CreateDesc(String),
    CreateColor(usize),
    CreateSubmit,
    CreateCancel,
    // misc
    Tick,
    Key(keyboard::Event),
    DismissToast,
    Noop,
}

impl App {
    pub fn boot() -> (Self, Task<Msg>) {
        let settings = store::load_settings();
        let env_url = std::env::var("HERMES_GATEWAY_URL").ok();
        let url = env_url.clone().unwrap_or_else(|| settings.gateway_url.clone());
        let mut app = App {
            settings: settings.clone(),
            env_gateway: env_url.clone(),
            pending_gateway: None,
            phase: Phase::SignIn(SignInState { url_input: url.clone(), ..Default::default() }),
            session: None,
            cmd: None,
            conn: ConnState::Connecting,
            me: None,
            bots: vec![],
            bots_loaded: false,
            statuses: HashMap::new(),
            unread: HashMap::new(),
            selected: settings.last_bot.clone(),
            chats: HashMap::new(),
            session_index: HashMap::new(),
            composer: text_editor::Content::new(),
            search: String::new(),
            details_open: false,
            details_tab: DetailsTab::Computer,
            screen: ScreenState::default(),
            settings_open: None,
            create: None,
            wallpaper: image::Handle::from_bytes(crate::theme::WALLPAPER),
            phase_t: 0.0,
            toast: None,
            modifiers: keyboard::Modifiers::default(),
        };
        // Resume a stored session.
        if let Ok(gw) = hermes_api::url_util::normalise_gateway(&url)
            && let Some(tokens) = store::load_tokens(gw.as_str())
        {
            app.session = Some(net::Session { gateway: gw, tokens });
            app.phase = Phase::Main;
        }
        // Test hook: sign in at once (mock gateways with /__mock/auto-approve).
        if std::env::var("HERMES_BOTS_AUTO_SIGNIN").is_ok_and(|v| v == "1") && app.session.is_none() {
            return (app, Task::done(Msg::SignInPressed));
        }
        let focus = if app.session.is_none() { iced::widget::operation::focus("gateway-url") } else { Task::none() };
        (app, focus)
    }

    pub fn title(&self) -> String {
        match (&self.phase, self.selected_bot()) {
            (Phase::Main, Some(b)) => format!("{} · Hermes Bots", b.title),
            _ => "Hermes Bots".into(),
        }
    }

    pub fn selected_bot(&self) -> Option<&Bot> {
        let sel = self.selected.as_ref()?;
        self.bots.iter().find(|b| &b.name == sel)
    }

    pub fn status_of(&self, name: &str) -> BotStatus {
        if let Some(s) = self.statuses.get(name) {
            return s.clone();
        }
        match self.bots.iter().find(|b| b.name == name) {
            Some(b) if b.worker_active => BotStatus::Working("Running a routine".into()),
            _ => BotStatus::Idle,
        }
    }

    fn send(&self, c: Command) {
        if let Some(tx) = &self.cmd {
            let _ = tx.send(c);
        }
    }

    pub fn online(&self) -> bool {
        self.conn == ConnState::Online
    }

    pub fn subscription(&self) -> Subscription<Msg> {
        let mut subs = vec![iced::event::listen_with(|ev, status, _| match ev {
            // Shortcuts (Ctrl+key, Escape) work even when a text field has focus.
            iced::Event::Keyboard(k @ keyboard::Event::KeyPressed { .. }) => {
                let is_shortcut = matches!(&k, keyboard::Event::KeyPressed { modifiers, key, .. }
                    if modifiers.command() || matches!(key, Key::Named(Named::Escape | Named::Tab)));
                (is_shortcut || status == iced::event::Status::Ignored).then_some(Msg::Key(k))
            }
            iced::Event::Keyboard(k @ keyboard::Event::ModifiersChanged(_)) => Some(Msg::Key(k)),
            _ => None,
        })];
        if let Some(s) = &self.session {
            subs.push(Subscription::run_with(s.clone(), net::run).map(Msg::Net));
        }
        let animating = !self.settings.reduce_motion
            && (self.statuses.values().any(|s| matches!(s, BotStatus::Thinking | BotStatus::Working(_)))
                || self.chats.values().any(|c| c.busy)
                || matches!(&self.phase, Phase::SignIn(s) if s.waiting_browser || s.checking));
        if animating {
            subs.push(iced::time::every(Duration::from_millis(120)).map(|_| Msg::Tick));
        }
        if self.toast.is_some() {
            subs.push(iced::time::every(Duration::from_secs(5)).map(|_| Msg::DismissToast));
        }
        Subscription::batch(subs)
    }

    fn open_selected_chat(&mut self) -> Task<Msg> {
        if let Some(name) = self.selected.clone() {
            self.unread.remove(&name);
            if !self.chats.get(&name).is_some_and(|c| c.session_id.is_some()) {
                self.chats.entry(name.clone()).or_default().loading = true;
                self.send(Command::OpenChat { profile: name.clone() });
            }
            self.send(Command::ScreenStatus { profile: name });
        }
        Task::none()
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::UrlInput(s) => {
                if let Phase::SignIn(st) = &mut self.phase {
                    st.url_input = s;
                    st.error = None;
                }
            }
            Msg::SignInPressed => {
                let Phase::SignIn(st) = &mut self.phase else {
                    return Task::none();
                };
                match hermes_api::url_util::normalise_gateway(&st.url_input) {
                    Ok(url) => {
                        st.checking = true;
                        st.error = None;
                        st.status = Some("Checking the gateway…".into());
                        return Task::perform(signin::probe(url), Msg::Probed);
                    }
                    Err(e) => st.error = Some(e.to_string()),
                }
            }
            Msg::Probed(res) => {
                let Phase::SignIn(st) = &mut self.phase else {
                    return Task::none();
                };
                st.checking = false;
                match res {
                    Ok(_status) => {
                        let Ok(url) = hermes_api::url_util::normalise_gateway(&st.url_input) else {
                            return Task::none();
                        };
                        st.waiting_browser = true;
                        st.status = Some("Finish signing in in your browser.".into());
                        store::remember_gateway(&mut self.settings, url.as_str(), self.env_gateway.as_deref());
                        self.pending_gateway = Some(url.clone());
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        let open_browser = std::env::var("HERMES_BOTS_NO_BROWSER").map(|v| v != "1").unwrap_or(true);
                        let sign = Task::perform(signin::sign_in(url, open_browser, tx), Msg::SignedIn);
                        let started = Task::perform(async move { rx.await.ok() }, |s| match s {
                            Some(s) => Msg::SignInStarted(s),
                            None => Msg::Noop,
                        });
                        return Task::batch([sign, started]);
                    }
                    Err(e) => {
                        st.status = None;
                        st.error = Some(e);
                    }
                }
            }
            Msg::SignInStarted(s) => {
                if let Phase::SignIn(st) = &mut self.phase {
                    st.authorize_url = Some(s.authorize_url.clone());
                }
                // Test hook: drive the mock's approve step without a browser.
                if std::env::var("HERMES_BOTS_NO_BROWSER").is_ok_and(|v| v == "1") {
                    let url = s.authorize_url;
                    return Task::perform(
                        async move {
                            let c = reqwest_like_get(&url).await;
                            if let Err(e) = c {
                                tracing::warn!("auto sign-in failed: {e}");
                            }
                        },
                        |_| Msg::Noop,
                    );
                }
            }
            Msg::OpenBrowserAgain => {
                if let Phase::SignIn(st) = &self.phase
                    && let Some(u) = &st.authorize_url
                {
                    let _ = open::that_detached(u);
                }
            }
            Msg::CancelSignIn => {
                if let Phase::SignIn(st) = &mut self.phase {
                    *st = SignInState { url_input: st.url_input.clone(), ..Default::default() };
                }
            }
            Msg::SignedIn(res) => match res {
                Ok(tokens) => {
                    let Some(url) = self.pending_gateway.take() else {
                        return Task::none();
                    };
                    store::save_tokens(url.as_str(), &tokens);
                    self.session = Some(net::Session { gateway: url, tokens });
                    self.phase = Phase::Main;
                    self.conn = ConnState::Connecting;
                }
                Err(e) => {
                    if let Phase::SignIn(st) = &mut self.phase {
                        if !st.waiting_browser {
                            return Task::none();
                        }
                        st.waiting_browser = false;
                        st.status = None;
                        st.error = Some(format!("Sign-in failed. {e}"));
                    }
                }
            },
            Msg::Net(ev) => return self.on_net(ev),
            Msg::Search(s) => self.search = s,
            Msg::SelectBot(name) => {
                if self.selected.as_deref() != Some(&name) {
                    self.selected = Some(name.clone());
                    self.settings.last_bot = Some(name);
                    store::save_settings(&self.settings);
                    self.composer = text_editor::Content::new();
                    if self.screen.mode != ScreenMode::Closed {
                        self.close_screen();
                    }
                    self.maybe_preview(false);
                    self.screen.status = None;
                    self.screen.frame = None;
                    self.screen.error = None;
                }
                return Task::batch([self.open_selected_chat(), iced::widget::operation::focus("composer")]);
            }
            Msg::NewBot => {
                self.create = Some(CreateForm::default());
                return iced::widget::operation::focus("create-name");
            }
            Msg::Composer(action) => {
                let is_edit = action.is_edit();
                self.composer.perform(action);
                if is_edit && self.composer.text().len() > 20_000 {
                    let t: String = self.composer.text().chars().take(20_000).collect();
                    self.composer = text_editor::Content::with_text(&t);
                }
            }
            Msg::Send => {
                let text = self.composer.text().trim().to_string();
                let Some(name) = self.selected.clone() else {
                    return Task::none();
                };
                if text.is_empty() || !self.online() {
                    return Task::none();
                }
                let chat = self.chats.entry(name.clone()).or_default();
                let Some(sid) = chat.session_id.clone() else {
                    return Task::none();
                };
                chat.push(Item::User { text: text.clone() });
                chat.busy = true;
                self.statuses.insert(name.clone(), BotStatus::Thinking);
                self.composer = text_editor::Content::new();
                self.send(Command::Send { profile: name, session_id: sid, text });
                return iced::widget::operation::snap_to_end("transcript");
            }
            Msg::Stop => {
                if let Some(name) = self.selected.clone()
                    && let Some(chat) = self.chats.get(&name)
                    && let Some(sid) = chat.session_id.clone()
                {
                    let generation = chat.generation;
                    self.send(Command::Stop { profile: name, session_id: sid, generation });
                }
            }
            Msg::ToggleActivity(i) => {
                if let Some(name) = &self.selected
                    && let Some(chat) = self.chats.get_mut(name)
                    && let Some(Item::Activity { expanded, .. }) = chat.items.get_mut(i)
                {
                    *expanded = !*expanded;
                }
            }
            Msg::Approve(i, choice) => {
                let Some(name) = self.selected.clone() else {
                    return Task::none();
                };
                if let Some(chat) = self.chats.get_mut(&name) {
                    let sid = chat.session_id.clone().unwrap_or_default();
                    let chat_gen = chat.generation;
                    if let Some(Item::Approval { request_id, rpc_id, generation, answer, .. }) = chat.items.get_mut(i)
                        && answer.is_none()
                    {
                        *answer = Some(choice.clone());
                        let cmd = Command::Approve { session_id: sid, request_id: request_id.clone(), choice, rpc_id: rpc_id.clone(), generation: generation.or(chat_gen) };
                        self.statuses.insert(name, BotStatus::Working("Continuing".into()));
                        self.send(cmd);
                    }
                }
            }
            Msg::RetryChat => {
                if let Some(name) = self.selected.clone() {
                    self.chats.remove(&name);
                    return self.open_selected_chat();
                }
            }
            Msg::ToggleDetails => {
                self.details_open = !self.details_open;
                if self.details_open
                    && let Some(n) = self.selected.clone()
                {
                    self.send(Command::ScreenStatus { profile: n });
                } else {
                    self.maybe_preview(false);
                }
            }
            Msg::DetailsTab(t) => {
                self.details_tab = t;
                let running = self.screen.status.as_ref().is_some_and(|s| s.running);
                self.maybe_preview(running);
            }
            Msg::WakeScreen => {
                if let Some(n) = self.selected.clone() {
                    self.screen.connecting = true;
                    self.send(Command::ScreenStart { profile: n });
                }
            }
            Msg::OpenScreen { take_over } => {
                let Some(name) = self.selected.clone() else {
                    return Task::none();
                };
                self.screen.profile = Some(name.clone());
                self.screen.mode = ScreenMode::Watching;
                self.screen.connecting = true;
                self.screen.error = None;
                let reuse = self.screen.previewing.as_deref() == Some(name.as_str()) && self.screen.frame.is_some();
                self.screen.previewing = None;
                if reuse {
                    self.screen.connecting = false;
                } else {
                    self.screen.frame = None;
                    let viewer = self.screen.viewer_id.clone();
                    self.send(Command::ScreenWatch { profile: name.clone(), viewer_id: viewer });
                }
                if take_over {
                    self.screen.mode = ScreenMode::Control;
                    // With a reused stream the viewer id exists now; else lease on ScreenObserved.
                    if reuse && let Some(v) = self.screen.viewer_id.clone() {
                        self.send(Command::TakeControl { profile: name, viewer_id: v });
                    }
                }
            }
            Msg::CloseScreen => self.close_screen(),
            Msg::TakeControl => {
                if let (Some(p), Some(v)) = (self.screen.profile.clone(), self.screen.viewer_id.clone()) {
                    self.send(Command::TakeControl { profile: p, viewer_id: v });
                    self.screen.mode = ScreenMode::Control;
                }
            }
            Msg::HandBack => return self.hand_back(),
            Msg::ScreenBytes(msgs) => {
                if self.screen.mode == ScreenMode::Control {
                    for m in msgs {
                        self.send(Command::ScreenInput(m));
                    }
                }
            }
            Msg::OpenSettings(p) => self.settings_open = Some(p),
            Msg::CloseSettings => self.settings_open = None,
            Msg::ToggleWallpaper => {
                self.settings.show_wallpaper = !self.settings.show_wallpaper;
                store::save_settings(&self.settings);
            }
            Msg::ToggleReduceMotion => {
                self.settings.reduce_motion = !self.settings.reduce_motion;
                store::save_settings(&self.settings);
            }
            Msg::SignOut => {
                self.send(Command::SignOut);
                if let Some(s) = &self.session {
                    store::delete_tokens(s.gateway.as_str());
                }
                self.reset_to_signin(false);
            }
            Msg::CreateTitle(s) => {
                if let Some(f) = &mut self.create {
                    if f.name.is_empty() || f.name == slug(&f.title) {
                        f.name = slug(&s);
                    }
                    f.title = s.chars().take(60).collect();
                }
            }
            Msg::CreateDesc(s) => {
                if let Some(f) = &mut self.create {
                    f.description = s.chars().take(300).collect();
                }
            }
            Msg::CreateColor(i) => {
                if let Some(f) = &mut self.create {
                    f.color = i % SWATCHES.len();
                }
            }
            Msg::CreateSubmit => {
                if let Some(f) = &mut self.create {
                    if f.name.is_empty() || f.title.trim().is_empty() {
                        f.error = Some("Give the Bot a name.".into());
                        return Task::none();
                    }
                    f.busy = true;
                    f.error = None;
                    let cmd = Command::CreateBot { name: f.name.clone(), title: f.title.trim().to_string(), description: f.description.trim().to_string(), color: SWATCHES[f.color].to_string() };
                    self.send(cmd);
                }
            }
            Msg::CreateCancel => self.create = None,
            Msg::Tick => self.phase_t += 0.12,
            Msg::Key(ev) => return self.on_key(ev),
            Msg::DismissToast => self.toast = None,
            Msg::Noop => {}
        }
        Task::none()
    }

    /// Watch the selected Bot's screen as a small live preview in the details pane.
    fn maybe_preview(&mut self, running: bool) {
        let Some(sel) = self.selected.clone() else {
            return;
        };
        let want = running && self.details_open && self.details_tab == DetailsTab::Computer && self.screen.mode == ScreenMode::Closed;
        if want && !self.screen.previewing.as_ref().is_some_and(|p| p == &sel) {
            self.screen.previewing = Some(sel.clone());
            self.screen.frame = None;
            self.send(Command::ScreenWatch { profile: sel, viewer_id: self.screen.viewer_id.clone() });
        } else if !want && self.screen.previewing.is_some() && self.screen.mode == ScreenMode::Closed {
            self.screen.previewing = None;
            self.send(Command::ScreenClose);
        }
    }

    fn close_screen(&mut self) {
        if self.screen.mode == ScreenMode::Control {
            let _ = self.hand_back();
        }
        self.screen.mode = ScreenMode::Closed;
        self.screen.connecting = false;
        self.screen.previewing = None;
        let running = self.screen.status.as_ref().is_some_and(|s| s.running);
        self.maybe_preview(running);
        if self.screen.previewing.is_none() {
            self.send(Command::ScreenClose);
            self.screen.frame = None;
        }
    }

    fn hand_back(&mut self) -> Task<Msg> {
        if let (Some(p), Some(v)) = (self.screen.profile.clone(), self.screen.viewer_id.clone()) {
            self.send(Command::HandBack { profile: p.clone(), viewer_id: v });
            self.screen.mode = ScreenMode::Watching;
            if let Some(chat) = self.chats.get_mut(&p) {
                let had_card = chat.items.iter().any(|i| matches!(i, Item::ComputerCard { resolved: false, .. }));
                chat.resolve_computer_cards();
                if had_card && let Some(sid) = chat.session_id.clone() {
                    let title = self.bots.iter().find(|b| b.name == p).map(|b| b.title.clone()).unwrap_or_default();
                    chat.push(Item::Notice { text: format!("You handed the screen back. {title} continues."), error: false });
                    let text = "I finished on your screen and handed control back. Please continue.".to_string();
                    chat.push(Item::User { text: text.clone() });
                    chat.busy = true;
                    self.statuses.insert(p.clone(), BotStatus::Thinking);
                    self.send(Command::Send { profile: p, session_id: sid, text });
                }
            }
        }
        Task::none()
    }

    fn reset_to_signin(&mut self, expired: bool) {
        // Show the address of this session again (it can be the env override).
        let url = match &self.session {
            Some(s) => s.gateway.to_string(),
            None => self.env_gateway.clone().unwrap_or_else(|| self.settings.gateway_url.clone()),
        };
        self.session = None;
        self.cmd = None;
        self.bots.clear();
        self.bots_loaded = false;
        self.chats.clear();
        self.statuses.clear();
        self.screen = ScreenState::default();
        self.me = None;
        self.phase = Phase::SignIn(SignInState { url_input: url, expired_notice: expired, ..Default::default() });
    }

    fn on_net(&mut self, ev: NetEvent) -> Task<Msg> {
        match ev {
            NetEvent::Ready(tx) => self.cmd = Some(tx),
            NetEvent::Connection(c) => {
                let was_online = self.online();
                self.conn = c;
                if !self.online() && was_online {
                    // Sessions are per socket: reopen chats after reconnect.
                    for chat in self.chats.values_mut() {
                        chat.session_id = None;
                        chat.busy = false;
                    }
                    self.session_index.clear();
                    if self.screen.mode != ScreenMode::Closed {
                        self.screen.connecting = true;
                    }
                }
                if self.online() && !was_online {
                    let t = self.open_selected_chat();
                    if self.screen.mode != ScreenMode::Closed
                        && let Some(p) = self.screen.profile.clone()
                    {
                        self.send(Command::ScreenWatch { profile: p, viewer_id: self.screen.viewer_id.clone() });
                    }
                    return t;
                }
            }
            NetEvent::Me(me) => self.me = Some(me),
            NetEvent::Bots(profiles) => {
                let mut bots: Vec<Bot> = profiles.iter().map(Bot::from_profile).collect();
                // Default Bot first, then most recent activity.
                bots.sort_by(|a, b| b.is_default.cmp(&a.is_default).then(b.last_active.total_cmp(&a.last_active)));
                self.bots = bots;
                self.bots_loaded = true;
                if self.selected.as_ref().is_none_or(|s| !self.bots.iter().any(|b| &b.name == s))
                    && let Some(first) = self.bots.first()
                {
                    self.selected = Some(first.name.clone());
                }
                return self.open_selected_chat();
            }
            NetEvent::ChatOpened { profile, chat } => {
                self.session_index.insert(chat.session_id.clone(), profile.clone());
                let mut c = Chat::from_open(&chat);
                // Keep a waiting computer card across reopen.
                if let Some(old) = self.chats.get(&profile) {
                    c.items.extend(old.items.iter().filter(|i| matches!(i, Item::ComputerCard { resolved: false, .. })).cloned());
                }
                self.chats.insert(profile.clone(), c);
                if self.selected.as_deref() == Some(&profile) {
                    return iced::widget::operation::snap_to_end("transcript");
                }
            }
            NetEvent::ChatError { profile, message } => {
                let c = self.chats.entry(profile).or_default();
                c.loading = false;
                c.error = Some(message);
            }
            NetEvent::Gateway(ge) => {
                let Some(sid) = &ge.session_id else {
                    if let Event::DisplayStatus(st) = ge.typed() {
                        let p = st.profile.clone();
                        if self.screen.profile.as_deref() == Some(&p) || self.selected.as_deref() == Some(&p) {
                            self.screen.status = Some(st);
                        }
                    }
                    return Task::none();
                };
                let Some(profile) = self.session_index.get(sid).cloned() else {
                    return Task::none();
                };
                let typed = ge.typed();
                let chat = self.chats.entry(profile.clone()).or_default();
                if let Some(g) = ge.execution_generation {
                    chat.generation = Some(g);
                }
                let change = chat.apply(&typed);
                if let Some(s) = change {
                    self.statuses.insert(profile.clone(), s);
                }
                if let Event::MessageComplete { text: Some(t), .. } = &typed
                    && let Some(b) = self.bots.iter_mut().find(|b| b.name == profile)
                {
                    b.preview = one_line(t, 120);
                    b.last_active = now_secs();
                }
                if self.selected.as_deref() != Some(&profile) {
                    if matches!(typed, Event::MessageComplete { .. }) {
                        self.unread.insert(profile, true);
                    }
                } else if matches!(typed, Event::MessageDelta { .. } | Event::ToolStart { .. } | Event::MessageComplete { .. }) {
                    return iced::widget::operation::snap_to_end("transcript");
                }
            }
            NetEvent::ServerRequest(req) => {
                let Some(profile) = req.session_id.as_ref().and_then(|s| self.session_index.get(s)).cloned() else {
                    return Task::none();
                };
                if req.method == "approval" {
                    let p = &req.params;
                    let s = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let chat = self.chats.entry(profile.clone()).or_default();
                    chat.push(Item::Approval {
                        request_id: s("request_id"),
                        rpc_id: Some(req.id.clone()),
                        generation: None,
                        command: s("command"),
                        description: s("description"),
                        choices: vec![],
                        answer: None,
                    });
                    self.statuses.insert(profile, BotStatus::NeedsYou("Needs your approval".into()));
                }
            }
            NetEvent::SendFailed { profile, message } => {
                let c = self.chats.entry(profile.clone()).or_default();
                c.busy = false;
                c.push(Item::Notice { text: format!("Not sent: {message}"), error: true });
                self.statuses.insert(profile, BotStatus::Error);
            }
            NetEvent::Screen { profile, status } => {
                if self.selected.as_deref() == Some(&profile) || self.screen.profile.as_deref() == Some(&profile) {
                    self.screen.connecting = self.screen.connecting && self.screen.mode != ScreenMode::Closed;
                    let running = status.running;
                    self.screen.status = Some(status);
                    if self.screen.mode == ScreenMode::Closed {
                        self.screen.profile = Some(profile);
                    }
                    self.maybe_preview(running);
                }
            }
            NetEvent::ScreenObserved { profile, viewer_id } => {
                self.screen.viewer_id = Some(viewer_id.clone());
                if self.screen.mode == ScreenMode::Control {
                    self.send(Command::TakeControl { profile, viewer_id });
                }
            }
            NetEvent::ScreenRfb(ev) => match ev {
                RfbEvent::Ready { width, height, .. } => {
                    self.screen.frame_size = (width, height);
                    self.screen.connecting = false;
                }
                RfbEvent::Frame(fb) => {
                    self.screen.frame_size = (fb.width, fb.height);
                    self.screen.connecting = false;
                    self.screen.frame = Some(image::Handle::from_rgba(fb.width as u32, fb.height as u32, fb.rgba.clone()));
                }
                RfbEvent::Closed { code, reason } => {
                    if self.screen.mode != ScreenMode::Closed {
                        match code {
                            Some(4000) => {
                                self.screen.mode = ScreenMode::Watching;
                                self.toast = Some("Someone else took control of this screen.".into());
                                if let Some(p) = self.screen.profile.clone() {
                                    self.send(Command::ScreenWatch { profile: p, viewer_id: self.screen.viewer_id.clone() });
                                }
                            }
                            Some(4001) => self.screen.error = Some("The Bot's computer is not running.".into()),
                            _ if self.online() => {
                                self.screen.connecting = true;
                                if let Some(p) = self.screen.profile.clone() {
                                    self.send(Command::ScreenWatch { profile: p, viewer_id: self.screen.viewer_id.clone() });
                                }
                                tracing::info!("screen stream closed: {reason}");
                            }
                            _ => self.screen.connecting = true,
                        }
                    }
                }
            },
            NetEvent::ScreenError { profile, message } => {
                if self.screen.profile.as_deref() == Some(&profile) || self.selected.as_deref() == Some(&profile) {
                    self.screen.connecting = false;
                    self.screen.error = Some(message);
                }
            }
            NetEvent::Lease { profile, lease } => {
                if self.screen.profile.as_deref() == Some(&profile)
                    && let Some(st) = &mut self.screen.status
                {
                    st.lease = lease.clone();
                }
                if self.screen.profile.as_deref() == Some(&profile) && !lease.human_holds() && self.screen.mode == ScreenMode::Control {
                    self.screen.mode = ScreenMode::Watching;
                }
            }
            NetEvent::BotCreated { name } => {
                self.create = None;
                self.selected = Some(name);
                self.toast = Some("Bot created.".into());
            }
            NetEvent::CreateFailed(e) => {
                if let Some(f) = &mut self.create {
                    f.busy = false;
                    f.error = Some(e);
                }
            }
            NetEvent::SignedOut { expired } => self.reset_to_signin(expired),
        }
        Task::none()
    }

    fn on_key(&mut self, ev: keyboard::Event) -> Task<Msg> {
        if let keyboard::Event::ModifiersChanged(m) = ev {
            self.modifiers = m;
        }
        // Keys go to the Bot screen (through its input layer) while the user holds control.
        if self.screen.mode == ScreenMode::Control {
            return Task::none();
        }
        let keyboard::Event::KeyPressed { key, modifiers, .. } = ev else {
            return Task::none();
        };
        match key.as_ref() {
            Key::Named(Named::Escape) => {
                if self.create.is_some() {
                    self.create = None;
                } else if self.settings_open.is_some() {
                    self.settings_open = None;
                } else if self.screen.mode != ScreenMode::Closed {
                    self.close_screen();
                } else if self.details_open {
                    self.details_open = false;
                }
            }
            Key::Named(Named::Tab) if self.create.is_some() || matches!(self.phase, Phase::SignIn(_)) => {
                return if modifiers.shift() { iced::widget::operation::focus_previous() } else { iced::widget::operation::focus_next() };
            }
            Key::Character(",") if modifiers.command() => self.settings_open = Some(SettingsPane::General),
            Key::Character("n") if modifiers.command() && matches!(self.phase, Phase::Main) => {
                return self.update(Msg::NewBot);
            }
            Key::Character(d) if modifiers.command() && matches!(d.as_bytes(), [b'1'..=b'9']) => {
                let i = usize::from(d.as_bytes()[0] - b'1');
                if let Some(b) = self.visible_bots().get(i) {
                    let name = b.name.clone();
                    return self.update(Msg::SelectBot(name));
                }
            }
            Key::Named(Named::Enter) if !modifiers.shift() && self.create.is_none() && self.settings_open.is_none() => {
                // Enter in the composer sends (the editor key binding handles it there).
            }
            _ => {}
        }
        Task::none()
    }

    pub fn visible_bots(&self) -> Vec<&Bot> {
        let q = self.search.trim().to_lowercase();
        self.bots.iter().filter(|b| q.is_empty() || b.title.to_lowercase().contains(&q) || b.name.contains(&q) || b.description.to_lowercase().contains(&q)).collect()
    }
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for ch in s.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(40).collect()
}

/// Map an iced key to an X11 keysym for the Bot screen.
pub fn keysym_for(key: &Key, text: Option<&str>, modifiers: keyboard::Modifiers) -> Option<u32> {
    use hermes_api::rfb::keysym as ks;
    if let Key::Named(n) = key {
        let v = match n {
            Named::Enter => ks::RETURN,
            Named::Backspace => ks::BACKSPACE,
            Named::Tab => ks::TAB,
            Named::Escape => ks::ESCAPE,
            Named::Delete => ks::DELETE,
            Named::ArrowLeft => ks::LEFT,
            Named::ArrowRight => ks::RIGHT,
            Named::ArrowUp => ks::UP,
            Named::ArrowDown => ks::DOWN,
            Named::Home => ks::HOME,
            Named::End => ks::END,
            Named::PageUp => ks::PAGE_UP,
            Named::PageDown => ks::PAGE_DOWN,
            Named::Space => 0x20,
            _ => return None,
        };
        return Some(v);
    }
    if modifiers.control() || modifiers.alt() {
        // Chords are not forwarded in this version (see known gaps).
        return None;
    }
    let t = text?;
    let c = t.chars().next()?;
    (!c.is_control()).then(|| rfb::keysym_for_char(c))
}

/// Follow the mock's sign-in in-process (test hook only; used with HERMES_BOTS_NO_BROWSER=1).
async fn reqwest_like_get(url: &str) -> Result<(), String> {
    // Minimal HTTP client: we only need the redirect chain authorize -> loopback.
    let client = hermes_api::RestClient::new(url::Url::parse(url).map_err(|e| e.to_string())?);
    let _ = client;
    let mut next = url.to_string();
    for _ in 0..5 {
        let u = url::Url::parse(&next).map_err(|e| e.to_string())?;
        let host = u.host_str().ok_or("no host")?.to_string();
        let port = u.port_or_known_default().ok_or("no port")?;
        let mut path = u.path().to_string();
        if let Some(q) = u.query() {
            path.push('?');
            path.push_str(q);
        }
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect((host.as_str(), port)).await.map_err(|e| e.to_string())?;
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes()).await.map_err(|e| e.to_string())?;
        let mut buf = String::new();
        s.read_to_string(&mut buf).await.map_err(|e| e.to_string())?;
        match buf.lines().find(|l| l.to_ascii_lowercase().starts_with("location:")) {
            Some(loc) => {
                let loc = loc[9..].trim();
                next = if loc.starts_with("http") { loc.to_string() } else { format!("{}://{}:{}{}", u.scheme(), host, port, loc) };
            }
            None => return Ok(()),
        }
    }
    Ok(())
}

impl App {
    pub fn view(&self) -> iced::Element<'_, Msg> {
        crate::views::view(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("Research Buddy!"), "research-buddy");
        assert_eq!(slug("  "), "");
    }

    #[test]
    fn keysyms() {
        let m = keyboard::Modifiers::default();
        assert_eq!(keysym_for(&Key::Named(Named::Enter), None, m), Some(0xff0d));
        assert_eq!(keysym_for(&Key::Character("a".into()), Some("a"), m), Some(0x61));
        assert_eq!(keysym_for(&Key::Character("c".into()), Some("c"), keyboard::Modifiers::CTRL), None);
    }
}
