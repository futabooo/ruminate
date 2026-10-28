//! Where conversations come from. Each agent gets one implementation of
//! [`Source`] (transcripts on this machine, found by `sync`) or [`Export`]
//! (an official export file handed to `import`); everything after parsing
//! (redaction, output, git) is shared.

pub mod chatgpt;
pub mod claude_ai;
pub mod claude_code;

use crate::model::Session;
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub trait Source {
    /// Short agent name written to every log line, e.g. `claude-code`.
    fn name(&self) -> &'static str;

    /// Every transcript file this source can read right now.
    fn discover(&self) -> Result<Vec<PathBuf>>;

    /// Converts one transcript. `Ok(None)` means there is nothing worth
    /// keeping (no identity, or no prompts at all).
    fn parse(&self, path: &Path) -> Result<Option<Session>>;
}

/// An official "export your data" file: one JSON document holding many
/// conversations. Formats are told apart by shape, not by file name, since
/// every service calls its file `conversations.json`.
pub trait Export {
    /// Short agent name written to every log line, e.g. `claude-ai`.
    fn name(&self) -> &'static str;

    /// Whether this looks like one conversation of this service.
    fn matches(&self, conversation: &Value) -> bool;

    /// Converts one conversation. `None` means there is nothing worth
    /// keeping (no identity, or no prompts at all).
    fn parse(&self, conversation: &Value) -> Option<Session>;
}

/// Every export format ruminate understands.
pub fn exports() -> Vec<Box<dyn Export>> {
    vec![Box::new(claude_ai::ClaudeAi), Box::new(chatgpt::ChatGpt)]
}

/// The export format of a parsed file, decided by its first conversation.
pub fn detect_export(doc: &Value) -> Option<Box<dyn Export>> {
    let first = *conversations(doc).first()?;
    exports().into_iter().find(|e| e.matches(first))
}

/// An export is normally an array of conversations; a single object is
/// accepted too (one conversation saved on its own).
pub fn conversations(doc: &Value) -> Vec<&Value> {
    match doc {
        Value::Array(items) => items.iter().collect(),
        Value::Object(_) => vec![doc],
        _ => Vec::new(),
    }
}

/// Removes ChatGPT's citation markers, which are spans wrapped in Unicode
/// private-use code points (`\u{e200}cite\u{e202}turn0search1\u{e201}`),
/// and any other private-use character, which no one types.
pub(crate) fn strip_private_use(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_marker = false;
    for c in s.chars() {
        match c {
            '\u{e200}' => in_marker = true,
            '\u{e201}' => in_marker = false,
            _ if in_marker || ('\u{e000}'..='\u{f8ff}').contains(&c) => {}
            _ => out.push(c),
        }
    }
    out
}

pub(crate) fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}
