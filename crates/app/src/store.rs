//! Settings (plain file, no secrets) and tokens (OS keyring, Secret Service on Linux).
//!
//! The settings file holds only the gateway URL and UI preferences. Tokens go to the keyring
//! under service `hermes-bots-desktop`, account = gateway URL. If no keyring is available, tokens
//! stay in memory for this run only (the user signs in again next time); they never go to a file.

use hermes_api::types::TokenSet;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SERVICE: &str = "hermes-bots-desktop";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub gateway_url: String,
    pub show_wallpaper: bool,
    pub reduce_motion: bool,
    pub last_bot: Option<String>,
}

impl Settings {
    pub fn defaults() -> Self {
        Self { show_wallpaper: true, ..Default::default() }
    }
}

fn config_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("HERMES_BOTS_CONFIG_DIR") {
        return Some(PathBuf::from(d));
    }
    directories::ProjectDirs::from("", "", "hermes-bots-desktop").map(|p| p.config_dir().to_path_buf())
}

pub fn load_settings() -> Settings {
    let Some(dir) = config_dir() else { return Settings::defaults() };
    match std::fs::read(dir.join("settings.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| Settings::defaults()),
        Err(_) => Settings::defaults(),
    }
}

pub fn save_settings(s: &Settings) {
    let Some(dir) = config_dir() else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_vec_pretty(s) {
        let tmp = dir.join("settings.json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(tmp, dir.join("settings.json"));
        }
    }
}

/// Tokens are kept in memory as a fallback when there is no keyring, or when the env var
/// `HERMES_BOTS_NO_KEYRING=1` is set (tests and headless screenshot runs).
static MEMORY: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);

fn keyring_disabled() -> bool {
    std::env::var("HERMES_BOTS_NO_KEYRING").is_ok_and(|v| v == "1")
}

pub fn save_tokens(gateway: &str, t: &TokenSet) {
    let Ok(json) = serde_json::to_string(t) else { return };
    if !keyring_disabled()
        && let Ok(e) = keyring::Entry::new(SERVICE, gateway)
        && e.set_password(&json).is_ok()
    {
        return;
    }
    if let Ok(mut m) = MEMORY.lock() {
        *m = Some((gateway.to_string(), json));
    }
}

pub fn load_tokens(gateway: &str) -> Option<TokenSet> {
    if !keyring_disabled()
        && let Ok(e) = keyring::Entry::new(SERVICE, gateway)
        && let Ok(json) = e.get_password()
    {
        return serde_json::from_str(&json).ok();
    }
    let m = MEMORY.lock().ok()?;
    let (g, json) = m.as_ref()?;
    (g == gateway).then(|| serde_json::from_str(json).ok()).flatten()
}

pub fn delete_tokens(gateway: &str) {
    if !keyring_disabled()
        && let Ok(e) = keyring::Entry::new(SERVICE, gateway)
    {
        let _ = e.delete_credential();
    }
    if let Ok(mut m) = MEMORY.lock() {
        *m = None;
    }
}

/// True when a keyring backend is usable (shown in Settings).
pub fn keyring_available() -> bool {
    !keyring_disabled() && keyring::Entry::new(SERVICE, "probe").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_token_fallback() {
        // SAFETY: test-only env change before any thread reads it in this test binary.
        unsafe { std::env::set_var("HERMES_BOTS_NO_KEYRING", "1") };
        let t = TokenSet { access_token: "a".into(), refresh_token: "r".into(), ..Default::default() };
        save_tokens("http://g", &t);
        assert_eq!(load_tokens("http://g").unwrap().access_token, "a");
        assert!(load_tokens("http://other").is_none());
        delete_tokens("http://g");
        assert!(load_tokens("http://g").is_none());
    }

    #[test]
    fn settings_round_trip() {
        let dir = std::env::temp_dir().join(format!("hbd-test-{}", std::process::id()));
        // SAFETY: test-only.
        unsafe { std::env::set_var("HERMES_BOTS_CONFIG_DIR", &dir) };
        let s = Settings { gateway_url: "https://g.example".into(), ..Settings::defaults() };
        save_settings(&s);
        assert_eq!(load_settings(), s);
        let raw = std::fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(!raw.contains("token"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
