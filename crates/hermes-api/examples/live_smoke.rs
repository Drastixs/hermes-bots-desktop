//! Live smoke test against a real gateway (manual use only).
//! `cargo run -p hermes-api --example live_smoke -- <gateway-url> <token-json-file> [--send]`
//! Prints only counts, status names and lengths: never tokens, names or chat text.

use hermes_api::gateway::{Gateway, Incoming};
use hermes_api::types::{Event, TokenSet};
use hermes_api::{RestClient, url_util};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let base = url_util::normalise_gateway(&args[1])?;
    let tokens: TokenSet = serde_json::from_str(&std::fs::read_to_string(&args[2])?)?;
    let send = args.iter().any(|a| a == "--send");
    let rest = RestClient::new(base);
    rest.set_tokens(&tokens).await;
    let (gw, mut rx) = Gateway::connect(&rest).await?;
    println!("ws connected");
    gw.ping().await?;
    println!("ping ok");
    let list = gw.profiles_list().await?;
    println!("profiles.list: {} rows, {} Bot Mode", list.profiles.len(), list.profiles.iter().filter(|p| p.is_bot()).count());
    let chat = gw.open_bot_chat("default").await?;
    println!("default Bot Chat open: {} stored rows, {} display rows, generation={:?}", chat.messages.len(), chat.display_messages.as_ref().map(|d| d.len()).unwrap_or(0), chat.execution_generation);
    match gw.display_status("default").await {
        Ok(s) => println!("display.status: supported={} running={} lease={}", s.supported, s.running, s.lease.holder),
        Err(e) => println!("display.status error: {e}"),
    }
    if send {
        let r = gw.prompt_submit(&chat.session_id, "default", "Desktop app test, please reply OK").await?;
        println!("prompt.submit: status={:?} admission={}", r.status, r.admission_id.is_some());
        let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
        let (mut deltas, mut tools) = (0, 0);
        loop {
            let item = match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Some(i)) => i,
                Ok(None) => {
                    println!("socket ended");
                    break;
                }
                Err(_) => {
                    println!("no reply within 180 s");
                    break;
                }
            };
            match item {
                Incoming::Event(e) if e.session_id.as_deref() == Some(chat.session_id.as_str()) => match e.typed() {
                    Event::MessageDelta { .. } => deltas += 1,
                    Event::ToolStart { .. } => tools += 1,
                    Event::MessageComplete { text, status, .. } => {
                        let t = text.unwrap_or_default();
                        println!("message.complete: status={status:?} deltas={deltas} tools={tools} reply_len={} contains_OK={}", t.len(), t.contains("OK"));
                        break;
                    }
                    _ => {}
                },
                Incoming::Closed { code, .. } => {
                    println!("closed {code:?}");
                    break;
                }
                _ => {}
            }
        }
    }
    gw.close().await;
    Ok(())
}
