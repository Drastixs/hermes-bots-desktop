//! UI-independent state: Bots, transcripts, streaming, tool activity, screen state.
//! Pure logic, so it is unit-tested without a window.

use hermes_api::types::*;
use iced::Color;
use std::collections::HashMap;

/// Upper bounds (bounded memory).
pub const MAX_ITEMS_PER_CHAT: usize = 400;
pub const MAX_TOOLS_PER_TURN: usize = 64;
pub const MAX_TEXT: usize = 200_000;

#[derive(Debug, Clone, PartialEq)]
pub enum BotStatus {
    Idle,
    Thinking,
    Working(String),
    /// The Bot waits for the user (approval, sign-in).
    NeedsYou(String),
    Error,
}

#[derive(Debug, Clone)]
pub struct Bot {
    pub name: String,
    pub title: String,
    pub description: String,
    pub color: Color,
    pub preview: String,
    pub last_active: f64,
    pub is_default: bool,
    pub model: String,
    pub worker_active: bool,
}

impl Bot {
    pub fn from_profile(p: &Profile) -> Self {
        let meta = p.bot_meta().unwrap_or_default();
        let color = match meta.color.as_deref() {
            Some(c) if !c.trim().is_empty() => crate::theme::parse_hex(Some(c)),
            _ => crate::theme::color_for_name(&p.name),
        };
        let session = p.canonical_session.as_ref().or(p.last_session.as_ref());
        let now = now_secs();
        Self {
            name: p.name.clone(),
            title: p.title(),
            description: meta.description.filter(|d| !d.is_empty()).unwrap_or_else(|| p.description.clone()),
            color,
            preview: session.map(|s| s.preview.clone()).unwrap_or_default(),
            last_active: session.map(|s| s.last_active).unwrap_or(0.0),
            is_default: p.is_default,
            model: p.model.clone().unwrap_or_default(),
            worker_active: p.worker_session.as_ref().is_some_and(|w| now - w.last_active < 120.0),
        }
    }
}

pub fn now_secs() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolState {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub context: String,
    pub summary: String,
    pub state: ToolState,
    pub percent: Option<i64>,
}

impl ToolCall {
    /// A short human label: verb + object.
    pub fn label(&self) -> String {
        let verb = match self.name.as_str() {
            "web_search" => "Searched the web",
            "read_file" => "Read a file",
            "write_file" | "patch" => "Edited a file",
            "terminal" => "Ran a command",
            "browser_navigate" => "Opened a page",
            "computer_use" => "Used the computer",
            "message_agent" => "Messaged a teammate",
            other => return humanise(other),
        };
        verb.to_string()
    }
    pub fn icon(&self) -> char {
        crate::icons::for_tool(&self.name)
    }
}

fn humanise(s: &str) -> String {
    let mut out: String = s.replace(['_', '.'], " ");
    if let Some(first) = out.get(0..1) {
        out.replace_range(0..1, &first.to_uppercase());
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    User {
        text: String,
    },
    Assistant {
        text: String,
        streaming: bool,
    },
    /// Tool activity of one turn, shown as one foldable group.
    Activity {
        tools: Vec<ToolCall>,
        expanded: bool,
    },
    /// The Bot asks the user to take over its screen (for example to sign in).
    ComputerCard {
        text: String,
        resolved: bool,
    },
    Approval {
        request_id: String,
        rpc_id: Option<serde_json::Value>,
        generation: Option<i64>,
        command: String,
        description: String,
        choices: Vec<String>,
        answer: Option<String>,
    },
    Notice {
        text: String,
        error: bool,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Chat {
    pub session_id: Option<String>,
    pub items: Vec<Item>,
    pub loading: bool,
    pub error: Option<String>,
    pub busy: bool,
    pub status_text: String,
    pub thinking: String,
    /// Canonical gateways: the latest execution generation seen for this session.
    pub generation: Option<i64>,
}

impl Chat {
    pub fn from_open(open: &SessionOpen) -> Self {
        let mut chat = Chat { session_id: Some(open.session_id.clone()), generation: open.execution_generation, ..Default::default() };
        for m in open.render_rows() {
            let text = m.display_text();
            match m.role.as_str() {
                "user" if !text.is_empty() => chat.push(Item::User { text }),
                "assistant" if !text.trim().is_empty() => chat.push(Item::Assistant { text, streaming: false }),
                "tool" => {
                    let call = ToolCall {
                        id: m.tool_call_id.clone().unwrap_or_default(),
                        name: m.name.clone().unwrap_or_else(|| "tool".into()),
                        context: m.context.clone().unwrap_or_default(),
                        summary: truncate(&text, 80),
                        state: ToolState::Done,
                        percent: None,
                    };
                    chat.push_tool(call);
                }
                _ => {}
            }
        }
        chat.busy = open.running.unwrap_or(false);
        // Approval prompts that already wait (canonical snapshot).
        for p in &open.prompts {
            if p.get("kind").and_then(|k| k.as_str()) == Some("approval") {
                let s = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                chat.push(Item::Approval {
                    request_id: s("prompt_id"),
                    rpc_id: None,
                    generation: p.get("execution_generation").and_then(|v| v.as_i64()).or(open.execution_generation),
                    command: s("command"),
                    description: s("description"),
                    choices: p.get("choices").and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
                    answer: None,
                });
            }
        }
        chat
    }

    pub fn push(&mut self, item: Item) {
        self.items.push(item);
        if self.items.len() > MAX_ITEMS_PER_CHAT {
            let drop = self.items.len() - MAX_ITEMS_PER_CHAT;
            self.items.drain(..drop);
        }
    }

    fn push_tool(&mut self, call: ToolCall) {
        if let Some(Item::Activity { tools, .. }) = self.items.last_mut() {
            if tools.len() < MAX_TOOLS_PER_TURN {
                tools.push(call);
            }
            return;
        }
        self.push(Item::Activity { tools: vec![call], expanded: false });
    }

    fn streaming_text(&mut self) -> &mut String {
        let needs_new = !matches!(self.items.last(), Some(Item::Assistant { streaming: true, .. }));
        if needs_new {
            self.push(Item::Assistant { text: String::new(), streaming: true });
        }
        match self.items.last_mut() {
            Some(Item::Assistant { text, .. }) => text,
            _ => unreachable!("an assistant item was just pushed"),
        }
    }

    fn finish_stream(&mut self, final_text: Option<String>) {
        // Find the open streaming item (it may not be last if a tool started after text).
        if let Some(Item::Assistant { text, streaming }) = self.items.iter_mut().rev().find(|i| matches!(i, Item::Assistant { streaming: true, .. })) {
            if let Some(t) = final_text.filter(|t| !t.is_empty()) {
                *text = truncate(&t, MAX_TEXT);
            }
            *streaming = false;
        } else if let Some(t) = final_text.filter(|t| !t.trim().is_empty()) {
            self.push(Item::Assistant { text: truncate(&t, MAX_TEXT), streaming: false });
        }
    }

    /// Apply one gateway event for this chat's session. Returns the Bot status change, if any.
    pub fn apply(&mut self, ev: &Event) -> Option<BotStatus> {
        match ev {
            Event::MessageStart => {
                self.busy = true;
                self.thinking.clear();
                Some(BotStatus::Thinking)
            }
            Event::MessageDelta { text } => {
                self.busy = true;
                let buf = self.streaming_text();
                if buf.len() + text.len() <= MAX_TEXT {
                    buf.push_str(text);
                }
                None
            }
            Event::ThinkingDelta { text } => {
                if self.thinking.len() < 4000 {
                    self.thinking.push_str(text);
                }
                None
            }
            Event::ToolStart { tool_id, name, context } => {
                // Close a text segment so tools show after it.
                if let Some(Item::Assistant { streaming, text }) = self.items.last_mut()
                    && *streaming
                    && !text.is_empty()
                {
                    *streaming = false;
                }
                let call = ToolCall { id: tool_id.clone(), name: name.clone(), context: context.clone().unwrap_or_default(), summary: String::new(), state: ToolState::Running, percent: None };
                let label = call.label();
                self.push_tool(call);
                if name == "computer_use" {
                    return Some(BotStatus::Working("Working on its computer".into()));
                }
                Some(BotStatus::Working(label))
            }
            Event::ToolProgress { tool_id, percent, .. } => {
                if let Some(t) = self.find_tool(tool_id) {
                    t.percent = *percent;
                }
                None
            }
            Event::ToolComplete { tool_id, summary, failed, name, .. } => {
                if let Some(t) = self.find_tool(tool_id) {
                    t.state = if *failed { ToolState::Failed } else { ToolState::Done };
                    t.summary = summary.clone().unwrap_or_default();
                }
                if name == "computer_use" && summary.as_deref().is_some_and(|s| s.to_lowercase().contains("waiting for you")) {
                    self.push(Item::ComputerCard { text: "Sign in on the Bot's screen so it can continue.".into(), resolved: false });
                    return Some(BotStatus::NeedsYou("Waiting for you to sign in".into()));
                }
                None
            }
            Event::StatusUpdate { kind, text } => {
                self.status_text = text.clone();
                match kind.as_str() {
                    "idle" if text.is_empty() => None,
                    "idle" => Some(BotStatus::NeedsYou(text.clone())),
                    _ => None,
                }
            }
            Event::MessageComplete { text, status, error } => {
                self.finish_stream(text.clone());
                self.busy = false;
                self.status_text.clear();
                // Any tool still "running" at the end is done.
                for item in self.items.iter_mut() {
                    if let Item::Activity { tools, .. } = item {
                        for t in tools.iter_mut().filter(|t| t.state == ToolState::Running) {
                            t.state = ToolState::Done;
                        }
                    }
                }
                if status.as_deref() == Some("error") || error.is_some() {
                    self.push(Item::Notice { text: error.clone().unwrap_or_else(|| "The Bot failed to respond.".into()), error: true });
                    return Some(BotStatus::Error);
                }
                let waiting = self.items.iter().rev().take(4).any(|i| matches!(i, Item::ComputerCard { resolved: false, .. }));
                if waiting { None } else { Some(BotStatus::Idle) }
            }
            Event::ApprovalRequest { prompt_id, command, description, choices, generation } => {
                if generation.is_some() {
                    self.generation = *generation;
                }
                let known = self.items.iter().any(|i| matches!(i, Item::Approval { request_id, .. } if request_id == prompt_id));
                if !known {
                    self.push(Item::Approval {
                        request_id: prompt_id.clone(),
                        rpc_id: None,
                        generation: *generation,
                        command: command.clone(),
                        description: description.clone(),
                        choices: choices.clone(),
                        answer: None,
                    });
                }
                Some(BotStatus::NeedsYou("Needs your approval".into()))
            }
            Event::ApprovalSettled { prompt_id } => {
                for i in self.items.iter_mut() {
                    if let Item::Approval { request_id, answer, .. } = i
                        && request_id == prompt_id
                        && answer.is_none()
                    {
                        *answer = Some("settled".into());
                    }
                }
                Some(BotStatus::Working("Continuing".into()))
            }
            Event::Error { message } => {
                self.push(Item::Notice { text: message.clone(), error: true });
                self.busy = false;
                Some(BotStatus::Error)
            }
            _ => None,
        }
    }

    fn find_tool(&mut self, id: &str) -> Option<&mut ToolCall> {
        self.items.iter_mut().rev().find_map(|i| match i {
            Item::Activity { tools, .. } => tools.iter_mut().find(|t| t.id == id),
            _ => None,
        })
    }

    pub fn resolve_computer_cards(&mut self) {
        for i in self.items.iter_mut() {
            if let Item::ComputerCard { resolved, .. } = i {
                *resolved = true;
            }
        }
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// One line preview (roster rows).
pub fn one_line(s: &str, max_chars: usize) -> String {
    let flat: String = s.lines().map(|l| l.trim().trim_start_matches(['#', '*', '-', '>', ' ']).trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
    let flat: String = flat.replace("**", "").replace('`', "").split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max_chars { flat } else { format!("{}…", flat.chars().take(max_chars).collect::<String>()) }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ScreenMode {
    #[default]
    Closed,
    /// Full view, input does not go to the Bot computer.
    Watching,
    /// Full view, the user holds the lease.
    Control,
}

#[derive(Debug, Default)]
pub struct ScreenState {
    pub profile: Option<String>,
    pub status: Option<DisplayStatus>,
    pub viewer_id: Option<String>,
    pub mode: ScreenMode,
    pub frame: Option<iced::widget::image::Handle>,
    pub frame_size: (u16, u16),
    pub connecting: bool,
    pub error: Option<String>,
    /// Profile whose screen is streamed as a small preview (details pane).
    pub previewing: Option<String>,
}

/// Map a point in the view (size `view`) to framebuffer coords for a `ContentFit::Contain` image.
pub fn view_to_fb(p: (f32, f32), view: (f32, f32), fb: (u16, u16)) -> Option<(u16, u16)> {
    let (fw, fh) = (fb.0 as f32, fb.1 as f32);
    if fw <= 0.0 || fh <= 0.0 || view.0 <= 0.0 || view.1 <= 0.0 {
        return None;
    }
    let scale = (view.0 / fw).min(view.1 / fh);
    let (dw, dh) = (fw * scale, fh * scale);
    let (ox, oy) = ((view.0 - dw) / 2.0, (view.1 - dh) / 2.0);
    let x = (p.0 - ox) / scale;
    let y = (p.1 - oy) / scale;
    if x < 0.0 || y < 0.0 || x >= fw || y >= fh {
        return None;
    }
    Some((x as u16, y as u16))
}

/// Status per Bot (by profile name).
pub type Statuses = HashMap<String, BotStatus>;

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(kind: &str, payload: serde_json::Value) -> Event {
        GatewayEvent { kind: kind.into(), session_id: Some("s".into()), payload, execution_generation: None }.typed()
    }

    #[test]
    fn streaming_turn_with_tools() {
        let mut c = Chat::default();
        assert_eq!(c.apply(&ev("message.start", serde_json::json!({}))), Some(BotStatus::Thinking));
        c.apply(&ev("tool.start", serde_json::json!({"tool_id": "1", "name": "web_search", "context": "x"})));
        c.apply(&ev("tool.start", serde_json::json!({"tool_id": "2", "name": "terminal"})));
        c.apply(&ev("tool.complete", serde_json::json!({"tool_id": "1", "name": "web_search", "summary": "4 results"})));
        c.apply(&ev("message.delta", serde_json::json!({"text": "Hel"})));
        c.apply(&ev("message.delta", serde_json::json!({"text": "lo"})));
        assert!(c.busy);
        match &c.items[0] {
            Item::Activity { tools, .. } => {
                assert_eq!(tools.len(), 2);
                assert_eq!(tools[0].state, ToolState::Done);
                assert_eq!(tools[0].summary, "4 results");
                assert_eq!(tools[1].state, ToolState::Running);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(c.items[1], Item::Assistant { text: "Hello".into(), streaming: true });
        let s = c.apply(&ev("message.complete", serde_json::json!({"text": "Hello!", "status": "complete"})));
        assert_eq!(s, Some(BotStatus::Idle));
        assert_eq!(c.items[1], Item::Assistant { text: "Hello!".into(), streaming: false });
        assert!(!c.busy);
        match &c.items[0] {
            Item::Activity { tools, .. } => assert_eq!(tools[1].state, ToolState::Done),
            _ => unreachable!(),
        }
    }

    #[test]
    fn login_handoff_card() {
        let mut c = Chat::default();
        c.apply(&ev("tool.start", serde_json::json!({"tool_id": "1", "name": "computer_use"})));
        let s = c.apply(&ev("tool.complete", serde_json::json!({"tool_id": "1", "name": "computer_use", "summary": "Waiting for you"})));
        assert!(matches!(s, Some(BotStatus::NeedsYou(_))));
        assert!(c.items.iter().any(|i| matches!(i, Item::ComputerCard { resolved: false, .. })));
        assert_eq!(c.apply(&ev("message.complete", serde_json::json!({"text": "x"}))), None);
        c.resolve_computer_cards();
        assert!(c.items.iter().all(|i| !matches!(i, Item::ComputerCard { resolved: false, .. })));
    }

    #[test]
    fn errors_and_bounds() {
        let mut c = Chat::default();
        assert_eq!(c.apply(&ev("message.complete", serde_json::json!({"status": "error", "error": "Provider overloaded"}))), Some(BotStatus::Error));
        assert!(matches!(c.items.last(), Some(Item::Notice { error: true, .. })));
        for i in 0..(MAX_ITEMS_PER_CHAT + 50) {
            c.push(Item::User { text: i.to_string() });
        }
        assert_eq!(c.items.len(), MAX_ITEMS_PER_CHAT);
        // Unknown tool ids are ignored.
        c.apply(&ev("tool.complete", serde_json::json!({"tool_id": "zzz", "name": "x"})));
    }

    #[test]
    fn history_mapping() {
        let open: SessionOpen = serde_json::from_value(serde_json::json!({
        "session_id": "s", "messages": [
            {"role": "user", "text": "hi"}, {"role": "tool", "name": "web_search", "text": "r"},
            {"role": "assistant", "text": "yo"}, {"role": "system", "text": "hidden"}, {"role": "assistant", "text": "  "}
        ]}))
        .unwrap();
        let c = Chat::from_open(&open);
        assert_eq!(c.items.len(), 3);
    }

    #[test]
    fn text_helpers() {
        assert_eq!(truncate("héllo", 2), "h…");
        assert_eq!(one_line("## Title\n\nmore  text", 50), "Title more text");
        assert_eq!(one_line("Done:\n\n- **3 items** need", 50), "Done: 3 items need");
        assert_eq!(one_line("abcdef", 3), "abc…");
    }

    #[test]
    fn pointer_mapping() {
        assert_eq!(view_to_fb((50.0, 50.0), (100.0, 100.0), (200, 100)), Some((100, 50)));
        assert_eq!(view_to_fb((50.0, 10.0), (100.0, 100.0), (200, 100)), None); // letterbox
        assert_eq!(view_to_fb((1.0, 1.0), (0.0, 0.0), (1, 1)), None);
    }

    #[test]
    fn bot_from_profile() {
        let p: Profile = serde_json::from_value(serde_json::json!({
            "name": "scout", "ui_meta": {"hermes-bots": {"title": "Scout", "color": "#e0a14a"}},
            "canonical_session": {"id": "c", "resolved_id": "c", "preview": "p", "last_active": 1.0}
        }))
        .unwrap();
        let b = Bot::from_profile(&p);
        assert_eq!(b.title, "Scout");
        assert_eq!(b.preview, "p");
    }
}
