//! Turns a [`Session`] into JSONL in the data repository.
//!
//! Each session is one file, rewritten in full whenever its transcript
//! changes. Claude Code transcripts only ever grow (`/compact` appends a
//! summary rather than shrinking the file; see claude-recall ADR-003), so a
//! full rewrite is the whole update strategy.

use crate::model::{LogRecord, Session};
use crate::redact::Redactor;
use anyhow::Result;
use chrono::Local;
use std::path::{Path, PathBuf};

pub struct Renderer<'a> {
    pub redactor: &'a Redactor,
    pub code_max_lines: usize,
}

impl Renderer<'_> {
    pub fn records(&self, s: &Session) -> Vec<LogRecord> {
        let title = s.title.as_deref().map(|t| self.redactor.redact(t));
        s.turns
            .iter()
            .enumerate()
            .map(|(i, t)| LogRecord {
                agent: s.agent.to_string(),
                channel: s.channel,
                session: s.id.clone(),
                title: title.clone(),
                project: s.project.clone(),
                git_branch: s.git_branch.clone(),
                url: s.url.clone(),
                date: t.at,
                seq: i,
                role: t.role,
                text: self
                    .redactor
                    .redact(&truncate_code_blocks(&t.text, self.code_max_lines)),
                actions: t.actions.iter().map(|a| self.redactor.redact(a)).collect(),
            })
            .collect()
    }

    pub fn to_jsonl(&self, s: &Session) -> Result<String> {
        let mut out = String::new();
        for r in self.records(s) {
            out.push_str(&serde_json::to_string(&r)?);
            out.push('\n');
        }
        Ok(out)
    }
}

/// `logs/<agent>/<YYYY>/<MM>/<YYYY-MM-DD_HHMM>_<project>_<id8>.jsonl`, in
/// local time. Depends only on fields that never change for a session, so a
/// rewrite always lands on the same file.
pub fn relative_path(s: &Session) -> PathBuf {
    let start = s.started_at.with_timezone(&Local);
    let short: String =
        s.id.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(8)
            .collect();
    let name = format!(
        "{}_{}_{}.jsonl",
        start.format("%Y-%m-%d_%H%M"),
        slug(&s.project),
        short
    );
    Path::new("logs")
        .join(s.agent)
        .join(start.format("%Y").to_string())
        .join(start.format("%m").to_string())
        .join(name)
}

fn slug(s: &str) -> String {
    let out: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let out = out.trim_matches('-');
    if out.is_empty() {
        "unknown".to_string()
    } else {
        out.to_string()
    }
}

/// Keeps the first `max` lines of each fenced code block longer than that.
pub fn truncate_code_blocks(text: &str, max: usize) -> String {
    let mut out = Vec::new();
    let mut in_block = false;
    let mut block_lines = 0;
    let mut dropped = 0;
    for line in text.lines() {
        let fence = line.trim_start().starts_with("```");
        if fence && in_block {
            if dropped > 0 {
                out.push(format!("… ({dropped} lines omitted)"));
            }
            out.push(line.to_string());
            in_block = false;
            continue;
        }
        if fence {
            in_block = true;
            block_lines = 0;
            dropped = 0;
            out.push(line.to_string());
            continue;
        }
        if in_block {
            block_lines += 1;
            if block_lines > max {
                dropped += 1;
                continue;
            }
        }
        out.push(line.to_string());
    }
    if in_block && dropped > 0 {
        out.push(format!("… ({dropped} lines omitted)"));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_code_blocks_are_cut() {
        let body: Vec<String> = (1..=10).map(|i| format!("line{i}")).collect();
        let text = format!("before\n```rust\n{}\n```\nafter", body.join("\n"));
        let got = truncate_code_blocks(&text, 3);
        assert_eq!(
            got,
            "before\n```rust\nline1\nline2\nline3\n… (7 lines omitted)\n```\nafter"
        );
    }

    #[test]
    fn short_code_blocks_stay() {
        let text = "```\na\nb\n```";
        assert_eq!(truncate_code_blocks(text, 3), text);
    }

    #[test]
    fn slug_keeps_names_readable() {
        assert_eq!(slug("rss-reader"), "rss-reader");
        assert_eq!(slug("my project/x"), "my-project-x");
        assert_eq!(slug(""), "unknown");
    }
}
