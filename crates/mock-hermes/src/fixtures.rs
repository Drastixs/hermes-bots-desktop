//! Mock data. Generic Bot names only; no real people, hosts or chat content.

use serde_json::{Value, json};

pub struct BotFixture {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub color: &'static str,
    pub preview: &'static str,
    /// True: this Bot is working when the mock starts.
    pub working: bool,
}

/// Seven Bots, each with one job (colours from the Hermes profile swatch wheel, hsl(n*30 68% 58%)).
pub const BOTS: [BotFixture; 7] = [
    BotFixture { name: "default", title: "Atlas", description: "Chief of staff. Plans the day and hands work to the team.", color: "#4a84fe", preview: "Your morning brief is ready.", working: false },
    BotFixture { name: "scout", title: "Scout", description: "Watches job boards and finds roles that fit.", color: "#e0a14a", preview: "Found 3 new roles that match your filters.", working: true },
    BotFixture {
        name: "inbox",
        title: "Inbox Zero",
        description: "Sorts email, drafts replies, flags what needs you.",
        color: "#5fc88f",
        preview: "12 emails sorted. 2 need a reply from you.",
        working: false,
    },
    BotFixture { name: "scribe", title: "Scribe", description: "Writes and edits posts in your voice.", color: "#c86fd8", preview: "Draft 2 of the launch post is ready to review.", working: false },
    BotFixture { name: "ledger", title: "Ledger", description: "Tracks spending and reconciles receipts.", color: "#5fb8c8", preview: "October is reconciled. 1 receipt is missing.", working: false },
    BotFixture { name: "sentinel", title: "Sentinel", description: "Monitors services and opens tickets on alerts.", color: "#e06a6a", preview: "All 14 checks are green.", working: true },
    BotFixture { name: "courier", title: "Courier", description: "Books travel and tracks deliveries.", color: "#9fc85f", preview: "Your parcel arrives Thursday.", working: false },
];

pub fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn profile_row(b: &BotFixture, idx: usize) -> Value {
    let t = now() - (idx as f64) * 1800.0 - 120.0;
    json!({
        "name": b.name,
        "path": format!("/home/mock/.hermes/profiles/{}", b.name),
        "is_default": b.name == "default",
        "model": "mock-model-1",
        "provider": "mock",
        "description": b.description,
        "display_name": "",
        "skill_count": 12 + idx,
        "previous_names": [],
        "has_avatar": false,
        "ui_meta": {"hermes-bots": {"title": b.title, "color": b.color, "description": b.description, "shape": "circle", "created": 1_760_000_000_000u64 + idx as u64}},
        "ui_meta_revisions": {"hermes-bots": 1},
        "canonical_session": {
            "id": format!("chat-{}", b.name), "resolved_id": format!("chat-{}", b.name),
            "root_title": "Bot Chat", "title": "Bot Chat", "preview": b.preview,
            "started_at": t - 86400.0, "last_active": t, "message_count": 6
        },
        "last_session": {"id": format!("chat-{}", b.name), "title": "Bot Chat", "preview": b.preview, "started_at": t - 86400.0, "last_active": t, "message_count": 6},
        "worker_session": if b.working { json!({"id": format!("w-{}", b.name), "source": "cron", "title": "Routine", "last_active": now()}) } else { Value::Null },
    })
}

/// The stored transcript of a Bot Chat.
pub fn history(name: &str) -> Vec<Value> {
    let b = BOTS.iter().find(|b| b.name == name).unwrap_or(&BOTS[0]);
    let t = now() - 3600.0;
    vec![
        json!({"role": "user", "text": "Hi! What can you do for me?", "timestamp": t}),
        json!({"role": "assistant", "text": format!("Hi, I am {}. {} Send me a task at any time; I keep working when you close the app.", b.title, b.description), "timestamp": t + 5.0}),
        json!({"role": "user", "text": "Great. Give me a short status update.", "timestamp": t + 60.0}),
        json!({"role": "tool", "name": "web_search", "context": "status sources", "text": "4 results", "timestamp": t + 62.0}),
        json!({"role": "assistant", "text": b.preview, "timestamp": t + 70.0}),
    ]
}

/// A scripted reply. Mentions of "sign in" or "log in" trigger the login hand-off script.
pub fn reply_for(text: &str, bot_title: &str) -> (Vec<(&'static str, &'static str)>, String) {
    let lower = text.to_lowercase();
    if lower.contains("sign in") || lower.contains("log in") || lower.contains("login") {
        let tools = vec![("browser_navigate", "example-jobs.test/login"), ("computer_use", "Waiting for you to sign in")];
        let msg = "I opened the sign-in page on my screen. Please take over and sign in; I never ask for passwords in chat. Hand back when you are done and I will continue.".to_string();
        return (tools, msg);
    }
    let tools = vec![("web_search", "latest updates"), ("read_file", "notes/today.md"), ("terminal", "make report")];
    let msg =
        format!("Done. Here is what I found:\n\n- **3 items** need your review today.\n- The weekly report is up to date.\n- Nothing is blocked.\n\nI am {bot_title}; ask me for details on any item.");
    (tools, msg)
}
