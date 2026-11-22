//! Helpers that keep secrets out of logs and error text.

/// Replace the value of secret query parameters (`ticket`, `display_ticket`,
/// `token`, `code`, `code_verifier`) in a URL-like string with `***`.
pub fn redact_url(input: &str) -> String {
    const KEYS: [&str; 6] = ["display_ticket=", "ticket=", "token=", "code_verifier=", "code=", "refresh_token="];
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    'outer: while !rest.is_empty() {
        for key in KEYS {
            if let Some(stripped) = rest.strip_prefix(key) {
                let preceded_ok = out.is_empty() || out.ends_with('?') || out.ends_with('&');
                if preceded_ok {
                    out.push_str(key);
                    out.push_str("***");
                    let end = stripped.find('&').unwrap_or(stripped.len());
                    rest = &stripped[end..];
                    continue 'outer;
                }
            }
        }
        let ch = rest.chars().next().unwrap_or_default();
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

/// A string wrapper whose `Debug` and `Display` never show the value.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_tickets() {
        assert_eq!(redact_url("wss://h/api/ws?ticket=abc&x=1"), "wss://h/api/ws?ticket=***&x=1");
        assert_eq!(redact_url("wss://h/api/display/ws?display_ticket=abc"), "wss://h/api/display/ws?display_ticket=***");
        assert_eq!(redact_url("http://h/a?mytoken=1"), "http://h/a?mytoken=1");
    }

    #[test]
    fn secret_debug_hides_value() {
        let s = Secret::new("hunter2");
        assert_eq!(format!("{s:?} {s}"), "Secret(***) ***");
        assert_eq!(s.expose(), "hunter2");
    }
}
