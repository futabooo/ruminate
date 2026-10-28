//! ChatGPT's official export (`conversations.json` inside the ZIP from
//! Settings → Data controls → Export data).
//!
//! A conversation is a tree, not a list: `mapping` holds every message
//! node (`id`, `message`, `parent`, `children`), and editing a prompt or
//! regenerating a reply starts a new branch. `current_node` is the leaf of
//! the branch the person last looked at, so the conversation is read by
//! walking from there up to the root. Messages carry `author.role`
//! (`user`, `assistant`, `system`, `tool`), `create_time` (Unix seconds,
//! sometimes `null`), `recipient` (`all` for a visible reply, otherwise the
//! tool it was sent to) and `content.content_type` (`text`,
//! `multimodal_text`, `code`, `thoughts`, `execution_output`, ...).

use super::{Export, shorten, strip_private_use};
use crate::model::{Channel, Role, Session, Turn};
use chrono::{DateTime, Utc};
use serde_json::Value;

pub struct ChatGpt;

impl Export for ChatGpt {
    fn name(&self) -> &'static str {
        "chatgpt"
    }

    fn matches(&self, conversation: &Value) -> bool {
        conversation.get("mapping").is_some_and(Value::is_object)
    }

    fn parse(&self, conversation: &Value) -> Option<Session> {
        parse_conversation(conversation)
    }
}

pub fn parse_conversation(c: &Value) -> Option<Session> {
    let id = c
        .get("conversation_id")
        .or_else(|| c.get("id"))
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if id.is_empty() {
        return None;
    }
    let title = c
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from);
    let mapping = c.get("mapping").and_then(Value::as_object)?;

    let mut turns: Vec<Turn> = Vec::new();
    let mut unknown = 0;
    let mut last_at = time(c.get("create_time"));
    for node in active_branch(c, mapping) {
        let Some(m) = node.get("message").filter(|m| m.is_object()) else {
            // The root node and placeholders have no message.
            continue;
        };
        let role = m.pointer("/author/role").and_then(Value::as_str);
        let role = match role {
            Some("user") => Role::User,
            Some("assistant") => Role::Assistant,
            // Custom instructions, tool output, and the odd empty root.
            Some("system") | Some("tool") => continue,
            _ => {
                unknown += 1;
                continue;
            }
        };
        if m.pointer("/metadata/is_visually_hidden_from_conversation")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let at = match time(m.get("create_time")).or(last_at) {
            Some(t) => t,
            None => {
                unknown += 1;
                continue;
            }
        };
        last_at = Some(at);
        let Some((text, actions)) = content(m, role) else {
            unknown += 1;
            continue;
        };
        if text.is_empty() && actions.is_empty() {
            continue;
        }
        match turns.last_mut() {
            // Replies are split around every tool call; one turn per prompt.
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
    let started_at = time(c.get("create_time")).or_else(|| turns.first().map(|t| t.at))?;
    Some(Session {
        agent: "chatgpt",
        channel: Channel::Web,
        url: Some(format!("https://chatgpt.com/c/{id}")),
        id,
        title,
        project: String::new(),
        git_branch: None,
        started_at,
        turns,
        unknown_lines: unknown,
    })
}

/// Nodes from the root down to `current_node`. Without a `current_node`,
/// the newest leaf stands in for it.
fn active_branch<'a>(c: &'a Value, mapping: &'a serde_json::Map<String, Value>) -> Vec<&'a Value> {
    let leaf = c
        .get("current_node")
        .and_then(Value::as_str)
        .filter(|id| mapping.contains_key(*id))
        .map(String::from)
        .or_else(|| {
            mapping
                .iter()
                .filter(|(_, n)| {
                    n.get("children")
                        .and_then(Value::as_array)
                        .is_none_or(Vec::is_empty)
                })
                .max_by_key(|(_, n)| time(n.pointer("/message/create_time")))
                .map(|(id, _)| id.clone())
        });
    let mut chain = Vec::new();
    let mut cursor = leaf;
    // A cycle would be a broken export; the length bound ends it anyway.
    while let Some(id) = cursor
        && chain.len() <= mapping.len()
        && let Some(node) = mapping.get(&id)
    {
        chain.push(node);
        cursor = node.get("parent").and_then(Value::as_str).map(String::from);
    }
    chain.reverse();
    chain
}

fn time(v: Option<&Value>) -> Option<DateTime<Utc>> {
    let secs = v?.as_f64()?;
    DateTime::from_timestamp_millis((secs * 1000.0) as i64)
}

/// Visible text and tool calls. `None` for a content type that is not
/// understood at all; `Some` with nothing in it for one that is known and
/// deliberately dropped (thinking, tool output shown inline).
fn content(m: &Value, role: Role) -> Option<(String, Vec<String>)> {
    let content = m.get("content")?;
    let kind = content.get("content_type").and_then(Value::as_str)?;
    let recipient = m.get("recipient").and_then(Value::as_str).unwrap_or("all");
    match kind {
        "text" | "multimodal_text" => {
            let text = parts_text(content);
            // An assistant message addressed to a tool is a call, not a reply
            // (e.g. a search query sent to `browser`).
            if role == Role::Assistant && recipient != "all" {
                return Some((String::new(), action(recipient, &text)));
            }
            Some((text, Vec::new()))
        }
        "code" => {
            let code = content.get("text").and_then(Value::as_str).unwrap_or("");
            let tool = if recipient == "all" {
                "python"
            } else {
                recipient
            };
            Some((String::new(), action(tool, code)))
        }
        // Reasoning, tool output rendered inline, system notices: known
        // shapes that carry nothing the person said or read as a reply.
        "thoughts"
        | "reasoning_recap"
        | "execution_output"
        | "tether_browsing_display"
        | "tether_quote"
        | "system_error"
        | "user_editable_context"
        | "model_editable_context"
        | "sonic_webpage" => Some((String::new(), Vec::new())),
        _ => None,
    }
}

/// `parts` holds strings; in `multimodal_text` also objects for images and
/// audio, which are dropped.
fn parts_text(content: &Value) -> String {
    let joined = content
        .get("parts")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    strip_private_use(&joined).trim().to_string()
}

/// `python: import pandas as pd`, `browser: search("duckdb read_json")`.
fn action(tool: &str, input: &str) -> Vec<String> {
    let tool = tool.strip_prefix("dalle.").unwrap_or(tool);
    let first = input.lines().map(str::trim).find(|l| !l.is_empty());
    match first {
        Some(l) => vec![format!("{tool}: {}", shorten(l, 80))],
        None => vec![tool.to_string()],
    }
}
