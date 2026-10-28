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
    // Japanese phone numbers. ASCII word boundaries so that a number right
    // after kana/kanji still matches, but a longer run of digits (an
    // account ID) does not.
    //   +81 90-1234-5678 / +81-3-1234-5678. Must not follow a letter, digit
    //   or sign (`x+81`, `+81` inside a diff hunk); the `pre` group is kept.
    r"(?m)(?P<pre>^|[^A-Za-z0-9_+\-.])\+81[\- ]?\d{1,4}[\- ]?\d{1,4}[\- ]?\d{3,4}(?-u:\b)",
    //   mobile / IP phone: 070, 080, 090, 050
    r"(?-u:\b)0[5789]0[\-\s]?\d{4}[\-\s]?\d{4}(?-u:\b)",
    //   toll-free: 0120-xxx-xxx, 0800-xxx-xxxx
    r"(?-u:\b)(?:0120[\-\s]?\d{3}[\-\s]?\d{3}|0800[\-\s]?\d{3}[\-\s]?\d{4})(?-u:\b)",
    //   landline, hyphenated only (10 digits): 03-1234-5678, 045-123-4567, ...
    r"(?-u:\b)(?:0\d-\d{4}-\d{4}|0\d{2}-\d{3}-\d{4}|0\d{3}-\d{2}-\d{4}|0\d{4}-\d-\d{4})(?-u:\b)",
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
            if !re.is_match(&out) {
                continue;
            }
            // A pattern may name the text before the secret `pre` to keep it
            // (there is no look-behind).
            out = if re.capture_names().any(|n| n == Some("pre")) {
                re.replace_all(&out, |c: &regex::Captures| {
                    format!("{}{MARK}", c.name("pre").map_or("", |m| m.as_str()))
                })
                .into_owned()
            } else {
                re.replace_all(&out, MARK).into_owned()
            };
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
    fn phone_numbers() {
        assert_eq!(r("携帯は07020065972です"), "携帯は[REDACTED]です");
        assert_eq!(r("tel: 090-1234-5678"), "tel: [REDACTED]");
        assert_eq!(r("050 1234 5678"), "[REDACTED]");
        assert_eq!(r("+81 70-2006-5972"), "[REDACTED]");
        assert_eq!(r("電話は+817020065972です"), "電話は[REDACTED]です");
        assert_eq!(r("(+81-3-1234-5678)"), "([REDACTED])");
        assert_eq!(r("03-1234-5678 / 045-123-4567"), "[REDACTED] / [REDACTED]");
        assert_eq!(r("0120-123-456"), "[REDACTED]");
    }

    #[test]
    fn digits_that_are_not_phone_numbers() {
        // An account ID that happens to contain a mobile-looking run.
        let s = "ID 6848080702014370253";
        assert_eq!(r(s), s);
        // Dates, versions, ordinary numbers.
        let s = "2026-10-28 v0.4.45 port 8080 0x1234";
        assert_eq!(r(s), s);
        // Unhyphenated landline numbers are too ambiguous to touch.
        let s = "0312345678";
        assert_eq!(r(s), s);
        // `+81` glued to an identifier or sign, or spread over lines (a diff
        // hunk) is not a phone number.
        let s = "x+81 1234 5678 / 1e+81-1234-5678 / 0.5+81 12 3456\n+81\n1\n2345";
        assert_eq!(r(s), s);
    }

    #[test]
    fn extra_patterns() {
        let red = Redactor::new(&["acme-[0-9]{4}".to_string()]).unwrap();
        assert_eq!(red.redact("id acme-1234"), "id [REDACTED]");
    }
}
