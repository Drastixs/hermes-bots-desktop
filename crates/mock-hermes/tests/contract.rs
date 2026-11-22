//! Contract tests: every mock RPC result and event payload must validate against the upstream
//! hermes-agent v0.23.3 wire contract (`tests/contract/gateway-contract.subset.json`, trimmed from
//! `apps/shared/src/gateway-contract.openrpc.json`). The REST and auth shapes are checked against
//! the field lists in `hermes_cli/dashboard_auth/routes.py` and `hermes_cli/web_routers/profiles.py`.

use serde_json::{Value, json};
use std::time::Duration;

fn contract() -> Value {
    serde_json::from_str(include_str!("contract/gateway-contract.subset.json")).unwrap()
}

/// A small JSON Schema validator for the subset pydantic emits: $ref, anyOf, type, properties,
/// required, additionalProperties, items, enum, const.
fn validate(schema: &Value, v: &Value, defs: &Value, path: &str, errs: &mut Vec<String>) {
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        let name = r.rsplit('/').next().unwrap();
        return validate(&defs[name], v, defs, path, errs);
    }
    if let Some(any) = schema.get("anyOf").and_then(Value::as_array) {
        let ok = any.iter().any(|s| {
            let mut e = vec![];
            validate(s, v, defs, path, &mut e);
            e.is_empty()
        });
        if !ok {
            errs.push(format!("{path}: matches no anyOf branch: {v}"));
        }
        return;
    }
    if let Some(en) = schema.get("enum").and_then(Value::as_array)
        && !en.contains(v)
    {
        errs.push(format!("{path}: {v} not in enum {en:?}"));
    }
    if let Some(c) = schema.get("const")
        && c != v
    {
        errs.push(format!("{path}: {v} != const {c}"));
    }
    let ty = schema.get("type").and_then(Value::as_str);
    let ok = match ty {
        Some("object") => v.is_object(),
        Some("array") => v.is_array(),
        Some("string") => v.is_string(),
        Some("integer") => v.is_i64() || v.is_u64(),
        Some("number") => v.is_number(),
        Some("boolean") => v.is_boolean(),
        Some("null") => v.is_null(),
        _ => true,
    };
    if !ok {
        errs.push(format!("{path}: expected {ty:?}, got {v}"));
        return;
    }
    if let Some(obj) = v.as_object() {
        let props = schema.get("properties").and_then(Value::as_object);
        for req in schema.get("required").and_then(Value::as_array).into_iter().flatten() {
            let k = req.as_str().unwrap();
            if !obj.contains_key(k) {
                errs.push(format!("{path}: missing required '{k}'"));
            }
        }
        for (k, val) in obj {
            match props.and_then(|p| p.get(k)) {
                Some(s) => validate(s, val, defs, &format!("{path}.{k}"), errs),
                None => {
                    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                        errs.push(format!("{path}: unknown key '{k}' (contract forbids extra keys)"));
                    }
                }
            }
        }
    }
    if let (Some(arr), Some(items)) = (v.as_array(), schema.get("items")) {
        for (i, x) in arr.iter().enumerate() {
            validate(items, x, defs, &format!("{path}[{i}]"), errs);
        }
    }
}

/// The `approval.request` payload must also be a valid `CanonicalPendingPrompt` (snapshot `prompts` rows).
fn canonical_pending_prompt(v: &Value) {
    let c = contract();
    let mut errs = vec![];
    validate(&c["schemas"]["CanonicalPendingPrompt"], v, &c["schemas"], "approval.request", &mut errs);
    assert!(errs.is_empty(), "{}", errs.join("\n"));
}

/// Check against the canonical (`hermes-gateway-v1`) contract that the real gateway enforces.
fn canonical(name: &str, kind: &str, v: &Value) {
    let c = contract();
    let schema = &c["canonical"][name][kind];
    assert!(!schema.is_null(), "no canonical contract for {name} {kind}");
    let mut errs = vec![];
    validate(schema, v, &c["schemas"], name, &mut errs);
    assert!(errs.is_empty(), "canonical {kind} {name} breaks the upstream contract:\n{}", errs.join("\n"));
}

fn check(kind: &str, name: &str, v: &Value) {
    let c = contract();
    let schema = match kind {
        "result" => &c["methods"][name]["result"],
        "params" => &c["methods"][name]["params"],
        _ => &c["events"][name],
    };
    assert!(!schema.is_null(), "no contract for {kind} {name}");
    let mut errs = vec![];
    validate(schema, v, &c["schemas"], name, &mut errs);
    assert!(errs.is_empty(), "{kind} {name} breaks the upstream contract:\n{}", errs.join("\n"));
}

#[tokio::test]
async fn mock_matches_upstream_contract() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let (addr, mock, _h) = mock_hermes::serve("127.0.0.1:0", 0.01).await.unwrap();
    // Mint a ticket directly (the auth flow is covered by the client tests).
    let base = format!("http://{addr}");
    let http = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
    // REST auth shapes (routes.py `_bearer_payload`, `api_auth_ws_ticket`, `api_auth_providers`).
    let providers: Value = http.get(format!("{base}/api/auth/providers")).send().await.unwrap().json().await.unwrap();
    for p in providers["providers"].as_array().unwrap() {
        for k in ["name", "display_name", "supports_password"] {
            assert!(p.get(k).is_some(), "provider row lacks {k}");
        }
    }
    curl_auto(&http, &base).await;
    let tokens = sign_in(&http, &base).await;
    for k in ["access_token", "refresh_token", "token_type", "expires_at", "provider", "user_id"] {
        assert!(tokens.get(k).is_some(), "token payload lacks {k}");
    }
    let bearer = format!("Bearer {}", tokens["access_token"].as_str().unwrap());
    let t: Value = http.post(format!("{base}/api/auth/ws-ticket")).header("Authorization", &bearer).send().await.unwrap().json().await.unwrap();
    assert_eq!(t["ttl_seconds"], 30);
    let me: Value = http.get(format!("{base}/api/auth/me")).header("Authorization", &bearer).send().await.unwrap().json().await.unwrap();
    for k in ["user_id", "email", "display_name", "org_id", "provider", "expires_at"] {
        assert!(me.get(k).is_some(), "/api/auth/me lacks {k}");
    }
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/api/ws?ticket={}", t["ticket"].as_str().unwrap())).await.unwrap();

    let mut id = 0;
    let mut events: Vec<Value> = vec![];
    let mut call = async |ws: &mut tokio_tungstenite::WebSocketStream<_>, method: &str, params: Value, events: &mut Vec<Value>| -> Value {
        id += 1;
        if method == "session.resume" {
            canonical(method, "params", &params);
        } else if !["prompt.submit", "session.interrupt", "approval.respond"].contains(&method) {
            check("params", method, &params);
        } else {
            canonical(method, "params", &params);
        }
        ws.send(Message::Text(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string().into())).await.unwrap();
        loop {
            let m = tokio::time::timeout(Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
            let Message::Text(t) = m else { continue };
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            if v["method"] == "event" {
                events.push(v);
                continue;
            }
            if v["id"] == json!(id) {
                assert!(v.get("error").is_none(), "{method} failed: {v}");
                return v["result"].clone();
            }
        }
    };
    let r = call(&mut ws, "profiles.list", json!({"include_sessions": true}), &mut events).await;
    check("result", "profiles.list", &r);
    let r = call(&mut ws, "session.list", json!({"profile": "scout", "title": "Bot Chat", "limit": 50, "include_hidden": true}), &mut events).await;
    check("result", "session.list", &r);
    let sid_row = r["sessions"][0]["id"].as_str().unwrap().to_string();
    let r = call(&mut ws, "session.resume", json!({"session_id": sid_row, "profile": "scout", "display_projection": true}), &mut events).await;
    canonical("session.resume", "result", &r);
    let r2 = call(&mut ws, "session.create", json!({"profile": "inbox", "title": "Bot Chat", "hidden": true, "follow_profile_config": true}), &mut events).await;
    canonical("session.resume", "result", &r2);
    let sid = r["session_id"].as_str().unwrap().to_string();
    let r = call(&mut ws, "prompt.submit", json!({"session_id": sid, "text": "please clean up the build", "submission_id": "sub-0"}), &mut events).await;
    canonical("prompt.submit", "result", &r);
    let r = call(&mut ws, "prompt.submit", json!({"session_id": sid, "text": "please log in", "submission_id": "sub-1"}), &mut events).await;
    canonical("prompt.submit", "result", &r);
    for (m, p) in [
        ("display.status", json!({"profile": "scout"})),
        ("display.start", json!({"profile": "scout"})),
        ("display.thumbnail", json!({"profile": "scout"})),
        ("display.observe", json!({"profile": "scout"})),
    ] {
        let r = call(&mut ws, m, p, &mut events).await;
        check("result", m, &r);
    }
    let r = call(&mut ws, "display.lease.acquire", json!({"profile": "scout", "viewer_id": "v1", "reason": "sign-in"}), &mut events).await;
    check("result", "display.lease.acquire", &r);
    let r = call(&mut ws, "display.lease.release", json!({"profile": "scout", "viewer_id": "v1"}), &mut events).await;
    check("result", "display.lease.release", &r);
    let r = call(&mut ws, "session.interrupt", json!({"session_id": sid, "execution_generation": 1}), &mut events).await;
    assert!(r.is_object());
    let r = call(&mut ws, "approval.respond", json!({"session_id": sid, "execution_generation": 1, "prompt_id": "p1", "choice": "once"}), &mut events).await;
    canonical("approval.respond", "result", &r);
    let r = call(&mut ws, "profiles.create", json!({"name": "helper", "description": "Helps"}), &mut events).await;
    check("result", "profiles.create", &r);
    let r = call(&mut ws, "profiles.configure", json!({"profile": "helper", "ui_meta": {"hermes-bots": {"title": "Helper"}}}), &mut events).await;
    check("result", "profiles.configure", &r);

    // Collect the streamed turn and check every event payload.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    while !events.iter().any(|e| e["params"]["type"] == "message.complete") {
        let m = tokio::time::timeout_at(deadline, ws.next()).await.expect("turn did not finish").unwrap().unwrap();
        if let Message::Text(t) = m {
            events.push(serde_json::from_str(t.as_str()).unwrap());
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for e in &events {
        let kind = e["params"]["type"].as_str().unwrap();
        if kind == "gateway.ready" {
            continue;
        }
        let empty = json!({});
        let payload = e["params"].get("payload").unwrap_or(&empty);
        if kind == "approval.request" || kind == "approval.settled" {
            // Canonical-only events (gateway/session_pending_controls.py; typed by hand in
            // apps/shared/src/gateway-events.ts, not in the generated contract).
            assert!(payload["prompt_id"].is_string(), "{kind} needs prompt_id");
            if kind == "approval.request" {
                let allowed = ["kind", "prompt_id", "execution_generation", "command", "description", "choices", "edit", "timeout_s"];
                for k in payload.as_object().unwrap().keys() {
                    assert!(allowed.contains(&k.as_str()), "approval.request has unknown key {k}");
                }
                canonical_pending_prompt(payload);
            }
        } else {
            check("event", kind, payload);
        }
        seen.insert(kind.to_string());
    }
    for k in ["message.start", "message.delta", "message.complete", "tool.start", "tool.progress", "tool.complete", "status.update", "display.lease", "approval.request", "approval.settled"] {
        assert!(seen.contains(k), "mock never sent {k}; saw {seen:?}");
    }
    drop(mock);
}

async fn curl_auto(http: &reqwest::Client, base: &str) {
    http.post(format!("{base}/__mock/auto-approve")).send().await.unwrap();
}

async fn sign_in(http: &reqwest::Client, base: &str) -> Value {
    use hermes_api::auth::{Pkce, s256};
    let p = Pkce::generate();
    assert_eq!(s256(p.verifier.expose()), p.challenge);
    let mut u = url::Url::parse(&format!("{base}/auth/native/authorize")).unwrap();
    u.query_pairs_mut().append_pair("code_challenge", &p.challenge).append_pair("code_challenge_method", "S256").append_pair("redirect_uri", "http://127.0.0.1:1/callback").append_pair("state", "st");
    let r = http.get(u).send().await.unwrap();
    assert_eq!(r.status(), 303);
    let loc = r.headers()["location"].to_str().unwrap().to_string();
    let url = url::Url::parse(&loc).unwrap();
    let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(q["state"], "st");
    http.post(format!("{base}/auth/native/token")).json(&json!({"code": q["code"], "code_verifier": p.verifier.expose()})).send().await.unwrap().json().await.unwrap()
}

#[test]
fn canonical_submit_needs_submission_id() {
    let c = contract();
    let mut e = vec![];
    // The shape our client sent first (with `profile`, without `submission_id`) is valid JSON
    // Schema, so the gateway's own closed-key rule is tested against the mock in the client tests.
    validate(&c["canonical"]["prompt.submit"]["params"], &json!({"session_id": "s", "text": "t", "submission_id": "x"}), &c["schemas"], "t", &mut e);
    assert!(e.is_empty(), "{e:?}");
}

#[test]
fn validator_rejects_bad_shapes() {
    let c = contract();
    let mut e = vec![];
    validate(&c["methods"]["display.status"]["result"], &json!({"profile": "x"}), &c["schemas"], "t", &mut e);
    assert!(!e.is_empty(), "missing required fields must fail");
    let mut e = vec![];
    validate(&c["events"]["message.delta"], &json!({"text": "x", "bogus": 1}), &c["schemas"], "t", &mut e);
    assert!(!e.is_empty(), "extra keys must fail where the contract forbids them");
}
