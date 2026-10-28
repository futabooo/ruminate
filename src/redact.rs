//! Replaces things that look like secrets with `[REDACTED]` before anything
//! leaves the machine. This is a safety net, not a guarantee: patterns catch
//! well-known token shapes and `key = value` style assignments.

use anyhow::{Context, Result};
use regex::Regex;

pub const MARK: &str = "[REDACTED]";

const BUILTIN: &[&str] = &[
    // Private key blocks.
    r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----",
    // Anthropic, OpenAI and similar `sk-` keys.
    r"\bsk-[A-Za-z0-9_\-]{16,}",
    // GitHub tokens.
    r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}",
    r"\bgithub_pat_[A-Za-z0-9_]{20,}",
    // AWS access key IDs.
    r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b",
    // Slack tokens.
    r"\bxox[abprs]-[A-Za-z0-9\-]{10,}",
    // Google API keys.
    r"\bAIza[A-Za-z0-9_\-]{35}",
    // JSON Web Tokens.
    r"\beyJ[A-Za-z0-9_\-]{8,}\.eyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}",
    // Bearer tokens in headers.
    r"(?i)\bbearer\s+[A-Za-z0-9._\-]{16,}",
];

/// `password = "..."`, `API_KEY: ...`, `client_secret=...`: keep the name,
/// hide the value.
const ASSIGNMENT: &str = r#"(?i)\b([A-Za-z0-9_\-]*(?:password|passwd|secret|token|api[_\-]?key|access[_\-]?key|private[_\-]?key)[A-Za-z0-9_\-]*)(\s*[:=]\s*)(["']?)([^\s"']{6,})(["']?)"#;

pub struct Redactor {
    patterns: Vec<Regex>,
    assignment: Regex,
}

impl Redactor {
    pub fn new(extra: &[String]) -> Result<Self> {
        let mut patterns = Vec::new();
        for p in BUILTIN {
            patterns.push(Regex::new(p).expect("built-in pattern"));
        }
        for p in extra {
            patterns.push(
                Regex::new(p)
                    .with_context(|| format!("redact_patterns の正規表現が不正です: {p}"))?,
            );
        }
        Ok(Self {
            patterns,
            assignment: Regex::new(ASSIGNMENT).expect("built-in pattern"),
        })
    }

    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for re in &self.patterns {
            if re.is_match(&out) {
                out = re.replace_all(&out, MARK).into_owned();
            }
        }
        self.assignment
            .replace_all(&out, |c: &regex::Captures| {
                if &c[4] == MARK {
                    c[0].to_string()
                } else {
                    format!("{}{}{}{}{}", &c[1], &c[2], &c[3], MARK, &c[5])
                }
            })
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> String {
        Redactor::new(&[]).unwrap().redact(s)
    }

    #[test]
    fn known_token_shapes() {
        assert_eq!(r("key sk-ant-api03-EXAMPLEexample0000"), "key [REDACTED]");
        assert_eq!(r("ghp_EXAMPLEexampleEXAMPLEexample00"), "[REDACTED]");
        assert_eq!(r("AKIAIOSFODNN7EXAMPLE"), "[REDACTED]");
        assert_eq!(
            r("Authorization: Bearer abcdefghijklmnop.qrstu"),
            "Authorization: [REDACTED]"
        );
    }

    #[test]
    fn assignments_keep_the_name() {
        assert_eq!(
            r(r#"DB_PASSWORD="hunter2hunter2""#),
            r#"DB_PASSWORD="[REDACTED]""#
        );
        assert_eq!(r("api_key: abcdef123456"), "api_key: [REDACTED]");
        assert_eq!(r("client_secret=zzzzzzzz"), "client_secret=[REDACTED]");
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let s = "The token bucket algorithm limits the rate. password length must be >= 8";
        assert_eq!(r(s), s);
    }

    #[test]
    fn private_key_block() {
        let s = "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\ndef\n-----END OPENSSH PRIVATE KEY-----";
        assert_eq!(r(s), "[REDACTED]");
    }

    #[test]
    fn extra_patterns() {
        let red = Redactor::new(&["acme-[0-9]{4}".to_string()]).unwrap();
        assert_eq!(red.redact("id acme-1234"), "id [REDACTED]");
    }
}
