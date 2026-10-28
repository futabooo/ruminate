//! Claude Code transcripts: `~/.claude/projects/<project>/<session-id>.jsonl`.
//!
//! The line format is internal to Claude Code and may change between
//! versions, so everything here is read through `serde_json::Value` and
//! anything unexpected is skipped (and counted) rather than treated as an
//! error. The filtering rules follow claude-recall (MIT,
//! https://github.com/babarot/claude-recall, `internal/parser/parser.go`):
//!
//! - only `user` and `assistant` lines carry conversation;
//! - `isSidechain` lines belong to subagents;
//! - `isMeta` lines and task notifications are injected context, not prompts;
//! - tool results arrive as `user` lines whose content is `tool_result`
//!   blocks, so a `user` line is not necessarily something the person typed;
//! - `/compact` appends a synthesized summary flagged `isCompactSummary`;
//! - `textNotTyped` marks text the harness wrote on the person's behalf;
//! - `custom-title` / `ai-title` lines carry the session title, last one wins.

use super::Source;
use crate::model::{Channel, Role, Session, Turn};
use crate::project::ProjectResolver;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct ClaudeCode {
    pub projects_dir: PathBuf,
    pub projects: ProjectResolver,
}

impl Source for ClaudeCode {
    fn name(&self) -> &'static str {
        "claude-code"
    }

    fn discover(&self) -> Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        let Ok(projects) = std::fs::read_dir(&self.projects_dir) else {
            return Ok(out);
        };
        for project in projects.flatten() {
            let Ok(files) = std::fs::read_dir(project.path()) else {
                continue;
            };
            // Only top-level `<session-id>.jsonl`; subagent transcripts live
            // in subdirectories and are skipped on purpose.
            for f in files.flatten() {
                let p = f.path();
                if p.is_file() && p.extension().is_some_and(|e| e == "jsonl") {
                    out.push(p);
                }
            }
        }
        out.sort();
        Ok(out)
    }

    fn parse(&self, path: &Path) -> Result<Option<Session>> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("読めません: {}", path.display()))?;
        let mut session = parse_transcript(&text, &self.projects);
        // The file name is the session ID. Lines inside may carry an older
        // ID: resuming a session copies earlier lines into the new file.
        if let (Some(s), Some(stem)) = (session.as_mut(), path.file_stem()) {
            s.id = stem.to_string_lossy().into_owned();
        }
        Ok(session)
    }
}

/// Text the person did not type but that arrives as a plain user string:
/// local slash-command echoes, `!` shell output, interruption notices.
const NOISE_PREFIXES: &[&str] = &[
    "<command-name>",
    "<command-message>",
    "<command-args>",
    "<local-command-",
    "<bash-input>",
    "<bash-stdout>",
    "<bash-stderr>",
    "<task-notification>",
    "[Request interrupted",
];

pub fn parse_transcript(text: &str, projects: &ProjectResolver) -> Option<Session> {
    let mut session_id = None;
    let mut cwd = String::new();
    let mut git_branch = None;
    let mut started_at = None;
    let mut custom_title = None;
    let mut ai_title = None;
    let mut turns: Vec<Turn> = Vec::new();
    let mut unknown = 0;

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            unknown += 1;
            continue;
        };
        let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "custom-title" => {
                custom_title = str_field(&v, "customTitle").or(custom_title);
                continue;
            }
            "ai-title" => {
                ai_title = str_field(&v, "aiTitle").or(ai_title);
                continue;
            }
            "user" | "assistant" => {}
            // Everything else is bookkeeping (system, attachment,
            // queue-operation, cost-state, ...), and new kinds appear often,
            // so they are skipped without counting as unknown.
            _ => continue,
        }
        if bool_field(&v, "isSidechain") {
            continue;
        }
        // Text the harness wrote on the person's behalf ("Continue with...").
        if bool_field(&v, "textNotTyped") {
            continue;
        }
        if session_id.is_none() {
            session_id = str_field(&v, "sessionId");
            cwd = str_field(&v, "cwd").unwrap_or_default();
            git_branch = str_field(&v, "gitBranch").filter(|b| b != "HEAD");
        }
        let at = v
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            .map(|t| t.with_timezone(&Utc));
        let Some(at) = at else {
            unknown += 1;
            continue;
        };
        if bool_field(&v, "isMeta")
            || bool_field(&v, "isCompactSummary")
            || v.pointer("/origin/kind").and_then(Value::as_str) == Some("task-notification")
        {
            continue;
        }
        let Some(content) = v
            .pointer("/message/content")
            .filter(|c| c.is_string() || c.is_array())
        else {
            unknown += 1;
            continue;
        };

        if kind == "user" {
            let typed = user_text(content);
            if typed.is_empty() {
                // Only tool results: the assistant's turn is still going.
                continue;
            }
            started_at.get_or_insert(at);
            turns.push(Turn {
                role: Role::User,
                at,
                text: typed,
                actions: Vec::new(),
            });
        } else {
            let (said, actions) = assistant_content(content, &cwd);
            if said.is_empty() && actions.is_empty() {
                continue;
            }
            // One assistant turn per prompt: Claude Code writes each block
            // of a reply (and each step around tool calls) as its own line.
            match turns.last_mut() {
                Some(last) if last.role == Role::Assistant => {
                    if !said.is_empty() {
                        if !last.text.is_empty() {
                            last.text.push_str("\n\n");
                        }
                        last.text.push_str(&said);
                    }
                    last.actions.extend(actions);
                }
                _ => turns.push(Turn {
                    role: Role::Assistant,
                    at,
                    text: said,
                    actions,
                }),
            }
        }
    }

    let id = session_id?;
    let started_at = started_at?;
    let project = projects.resolve(&cwd);
    Some(Session {
        agent: "claude-code",
        channel: Channel::Cli,
        id,
        title: custom_title.or(ai_title),
        project,
        git_branch,
        url: None,
        started_at,
        turns,
        unknown_lines: unknown,
    })
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn bool_field(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// What the person typed: a plain string, or the `text` blocks of an array.
/// `tool_result` and `image` blocks are dropped.
fn user_text(content: &Value) -> String {
    let raw = match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    let cleaned = strip_system_reminders(&raw);
    let trimmed = cleaned.trim();
    if NOISE_PREFIXES.iter().any(|p| trimmed.starts_with(p)) {
        return String::new();
    }
    trimmed.to_string()
}

/// The assistant's visible text and one line per tool call. `thinking`
/// blocks are dropped.
fn assistant_content(content: &Value, cwd: &str) -> (String, Vec<String>) {
    let mut text = Vec::new();
    let mut actions = Vec::new();
    match content {
        Value::String(s) => text.push(s.trim().to_string()),
        Value::Array(blocks) => {
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
                        actions.push(describe_tool(name, &input, cwd));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    (text.join("\n\n"), actions)
}

/// `Edit: src/main.rs`, `Bash: cargo test`, `WebSearch: duckdb read_json`.
/// Only the gist of the input is kept; full inputs can hold file contents.
pub fn describe_tool(name: &str, input: &Value, cwd: &str) -> String {
    let field = |k: &str| input.get(k).and_then(Value::as_str).map(str::to_string);
    let detail = match name {
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => field("file_path")
            .or_else(|| field("notebook_path"))
            .map(|p| relative_to(&p, cwd)),
        "Bash" => field("command").map(|c| shorten(c.lines().next().unwrap_or(""), 80)),
        "Grep" | "Glob" => field("pattern"),
        "WebFetch" => field("url"),
        "WebSearch" => field("query"),
        "Task" | "Agent" => field("description"),
        "Skill" => field("skill"),
        _ => None,
    };
    match detail {
        Some(d) if !d.is_empty() => format!("{name}: {d}"),
        _ => name.to_string(),
    }
}

fn relative_to(path: &str, cwd: &str) -> String {
    if cwd.is_empty() {
        return path.to_string();
    }
    match Path::new(path).strip_prefix(cwd) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => path.to_string(),
    }
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// Claude Code wraps injected context in `<system-reminder>` tags inside
/// user messages; it is not something the person wrote.
pub fn strip_system_reminders(s: &str) -> String {
    const OPEN: &str = "<system-reminder>";
    const CLOSE: &str = "</system-reminder>";
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        match rest[start..].find(CLOSE) {
            Some(end) => rest = &rest[start + end + CLOSE.len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}
