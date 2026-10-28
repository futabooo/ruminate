//! Agent-independent shapes. Every source converts its own transcripts into
//! a [`Session`], and every session is written out as [`LogRecord`] lines.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// A terminal agent such as Claude Code or Codex CLI.
    Cli,
    /// A chat in a web or mobile app, such as claude.ai or ChatGPT.
    Web,
}

/// One exchange step: a prompt from the person, or everything the assistant
/// said (and did) before the next prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    pub role: Role,
    pub at: DateTime<Utc>,
    pub text: String,
    /// One-line summaries of tool calls, such as `Edit: src/main.rs`.
    pub actions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub agent: &'static str,
    pub channel: Channel,
    pub id: String,
    pub title: Option<String>,
    pub project: String,
    pub git_branch: Option<String>,
    pub url: Option<String>,
    pub started_at: DateTime<Utc>,
    pub turns: Vec<Turn>,
    /// Lines the parser did not understand. A sudden rise usually means the
    /// agent changed its transcript format.
    pub unknown_lines: usize,
}

/// One line of a log file in the data repository. The DuckDB views in
/// `duckdb/init.sql` read exactly these fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogRecord {
    pub agent: String,
    pub channel: Channel,
    pub session: String,
    pub title: Option<String>,
    pub project: String,
    pub git_branch: Option<String>,
    pub url: Option<String>,
    pub date: DateTime<Utc>,
    pub seq: usize,
    pub role: Role,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
}
