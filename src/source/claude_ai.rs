//! claude.ai's official export (`conversations.json` inside the ZIP from
//! Settings → Privacy → Export data).
//!
//! Each conversation is an object with `uuid`, `name`, `created_at` and
//! `chat_messages`; each message has `sender` (`human` / `assistant`),
//! `created_at`, a `content` array of blocks (`text`, `tool_use`,
//! `tool_result`, ...) and, in older exports, only a plain `text`. Files
//! the person attached are listed in `attachments` and `files`. Everything
//! is read through `serde_json::Value` and unexpected shapes are skipped,
//! as for Claude Code.

use super::{Export, shorten};
use crate::model::{Channel, Role, Session, Turn};
use chrono::{DateTime, Utc};
use serde_json::Value;

pub struct ClaudeAi;

impl Export for ClaudeAi {
    fn name(&self) -> &'static str {
        "claude-ai"
    }

    fn matches(&self, conversation: &Value) -> bool {
        conversation
            .get("chat_messages")
            .is_some_and(Value::is_array)
    }

    fn parse(&self, conversation: &Value) -> Option<Session> {
        parse_conversation(conversation)
    }
}

pub fn parse_conversation(c: &Value) -> Option<Session> {
    let id = c.get("uuid").and_then(Value::as_str)?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let title = c
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from);
    let messages = c.get("chat_messages").and_then(Value::as_array)?;

    // Exports are in order already; sorting by time is only insurance, and
    // a stable sort keeps the order of messages with the same timestamp.
    let mut ordered: Vec<(Option<DateTime<Utc>>, &Value)> = messages
        .iter()
        .map(|m| (time(m, "created_at"), m))
        .collect();
    ordered.sort_by_key(|(t, _)| *t);

    let mut turns: Vec<Turn> = Vec::new();
    let mut unknown = 0;
    let mut last_at = time(c, "created_at");
    for (at, m) in ordered {
        let role = match m.get("sender").and_then(Value::as_str) {
            Some("human") => Role::User,
            Some("assistant") => Role::Assistant,
            _ => {
                unknown += 1;
                continue;
            }
        };
        let at = match at.or(last_at) {
            Some(t) => t,
            None => {
                unknown += 1;
                continue;
            }
        };
        last_at = Some(at);
        let (text, mut actions) = content(m);
        if role == Role::User {
            actions.extend(attachments(m));
        }
        if text.is_empty() && actions.is_empty() {
            continue;
        }
        match turns.last_mut() {
            // A regenerated or continued reply: one assistant turn per prompt.
            Some(last) if last.role == Role::Assistant && role == Role::Assistant => {
                if !text.is_empty() {
                    if !last.text.is_empty() {
                        last.text.push_str("\n\n");
                    }
                    last.text.push_str(&text);
                }
                last.actions.extend(actions);
            }
            _ => turns.push(Turn {
                role,
                at,
                text,
                actions,
            }),
        }
    }

    if !turns.iter().any(|t| t.role == Role::User) {
        return None;
    }
    let started_at = time(c, "created_at").or_else(|| turns.first().map(|t| t.at))?;
    Some(Session {
        agent: "claude-ai",
        channel: Channel::Web,
        url: Some(format!("https://claude.ai/chat/{id}")),
        id,
        title,
        project: String::new(),
        git_branch: None,
        started_at,
        turns,
        unknown_lines: unknown,
    })
}

fn time(v: &Value, key: &str) -> Option<DateTime<Utc>> {
    v.get(key)
        .and_then(Value::as_str)
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.with_timezone(&Utc))
}

/// Visible text and one line per tool call. `tool_result` blocks and
/// anything else (thinking, voice notes) are dropped. Older exports have
/// no `content` array, only `text`.
fn content(m: &Value) -> (String, Vec<String>) {
    let mut text = Vec::new();
    let mut actions = Vec::new();
    match m.get("content").and_then(Value::as_array) {
        Some(blocks) => {
            for b in blocks {
                match b.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = b.get("text").and_then(Value::as_str) {
                            let t = t.trim();
                            if !t.is_empty() {
                                text.push(t.to_string());
                            }
                        }
                    }
                    Some("tool_use") => {
                        let name = b.get("name").and_then(Value::as_str).unwrap_or("tool");
                        let input = b.get("input").cloned().unwrap_or(Value::Null);
                        actions.push(describe_tool(name, &input));
                    }
                    _ => {}
                }
            }
        }
        None => {
            if let Some(t) = m.get("text").and_then(Value::as_str) {
                let t = t.trim();
                if !t.is_empty() {
                    text.push(t.to_string());
                }
            }
        }
    }
    (text.join("\n\n"), actions)
}

/// `web_search: duckdb read_json`, `Artifact: Budget sheet`, `Edit: src/main.rs`
/// (Claude Code on the web uses the same tools as the terminal). Inputs can
/// hold whole documents, so only a short hint is kept.
pub fn describe_tool(name: &str, input: &Value) -> String {
    let hint = [
        "query",
        "title",
        "command",
        "file_path",
        "notebook_path",
        "pattern",
        "url",
        "skill",
        "description",
        "id",
    ]
    .iter()
    .find_map(|k| input.get(k).and_then(Value::as_str))
    .map(|s| shorten(s.lines().next().unwrap_or("").trim(), 80))
    .filter(|s| !s.is_empty());
    match hint {
        Some(h) => format!("{name}: {h}"),
        None => name.to_string(),
    }
}

/// `attachment: notes.txt` for each file the person added. Their contents
/// (`extracted_content`) are not kept: that is where pasted secrets live.
fn attachments(m: &Value) -> Vec<String> {
    ["attachments", "files"]
        .iter()
        .filter_map(|k| m.get(k).and_then(Value::as_array))
        .flatten()
        .filter_map(|f| f.get("file_name").and_then(Value::as_str))
        .map(|n| format!("attachment: {n}"))
        .collect()
}
