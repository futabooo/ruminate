//! Parsing and rendering of a synthetic Claude Code transcript.
//! `fixtures/claude_code_basic.jsonl` is made up; it mirrors the shapes seen
//! in real transcripts (Claude Code 2.1.x) without any real conversation.

use ruminate::model::{Channel, Role};
use ruminate::output::{Renderer, relative_path};
use ruminate::project::ProjectResolver;
use ruminate::redact::Redactor;
use ruminate::source::Source;
use ruminate::source::claude_code::{ClaudeCode, describe_tool, parse_transcript};
use serde_json::json;

const FIXTURE: &str = include_str!("fixtures/claude_code_basic.jsonl");

#[test]
fn file_name_is_the_session_id() {
    // A resumed session's file starts with lines from the older session.
    let older = FIXTURE.replacen("sess-1234abcd-0000", "old-99999999-0000", 1);
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("-p");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join("new-session-id.jsonl"), older).unwrap();
    let src = ClaudeCode {
        projects_dir: dir.path().to_path_buf(),
        projects: ProjectResolver::default(),
    };
    let files = src.discover().unwrap();
    assert_eq!(files.len(), 1);
    let s = src.parse(&files[0]).unwrap().unwrap();
    assert_eq!(s.id, "new-session-id");
}

#[test]
fn keeps_only_the_conversation() {
    let s = parse_transcript(FIXTURE, &ProjectResolver::default()).expect("session");
    assert_eq!(s.agent, "claude-code");
    assert_eq!(s.channel, Channel::Cli);
    assert_eq!(s.id, "sess-1234abcd-0000-0000-0000-000000000000");
    assert_eq!(s.project, "rss-reader");
    assert_eq!(s.git_branch.as_deref(), Some("main"));
    // custom-title wins over ai-title.
    assert_eq!(s.title.as_deref(), Some("RSSリーダー: Worker連携"));
    // Only the broken JSON line is unknown; bookkeeping kinds are not.
    assert_eq!(s.unknown_lines, 1);

    let roles: Vec<Role> = s.turns.iter().map(|t| t.role).collect();
    assert_eq!(
        roles,
        [Role::User, Role::Assistant, Role::User, Role::Assistant]
    );

    // System reminders are stripped from what the person typed.
    assert_eq!(s.turns[0].text, "Service Bindingってなに？");
    assert_eq!(s.turns[2].text, "APIキーは api_key=abcdef123456 でいい？");
}

#[test]
fn assistant_steps_merge_into_one_turn() {
    let s = parse_transcript(FIXTURE, &ProjectResolver::default()).unwrap();
    let reply = &s.turns[1];
    assert!(
        reply
            .text
            .starts_with("Worker同士をネットワークを通さずにつなぐ仕組みです。")
    );
    assert!(reply.text.contains("例です。"));
    assert_eq!(
        reply.actions,
        ["Read: wrangler.toml", "Bash: cargo test --all"]
    );
    // Thinking and tool results never make it in.
    assert!(!reply.text.contains("secret reasoning"));
    assert!(!reply.text.contains("should-not-appear"));
}

#[test]
fn skipped_lines_do_not_leak() {
    let s = parse_transcript(FIXTURE, &ProjectResolver::default()).unwrap();
    let all: String = s
        .turns
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for hidden in [
        "subagent prompt",
        "Caveat",
        "/clear",
        "Continue with the task",
        "continued from a previous conversation",
        "task-notification",
        "Injected context",
    ] {
        assert!(!all.contains(hidden), "leaked: {hidden}");
    }
}

#[test]
fn rendering_redacts_and_truncates() {
    let s = parse_transcript(FIXTURE, &ProjectResolver::default()).unwrap();
    let redactor = Redactor::new(&[]).unwrap();
    let r = Renderer {
        redactor: &redactor,
        code_max_lines: 5,
    };
    let recs = r.records(&s);
    assert_eq!(recs.len(), 4);
    assert_eq!(recs[2].text, "APIキーは api_key=[REDACTED] でいい？");
    assert!(recs[1].text.contains("キーは [REDACTED] です"));
    assert!(recs[1].text.contains("let x5 = 5;"));
    assert!(!recs[1].text.contains("let x6 = 6;"));
    assert!(recs[1].text.contains("… (45 lines omitted)"));
    assert_eq!(recs[3].seq, 3);

    // Every line round-trips as JSON with the agreed field names.
    let jsonl = r.to_jsonl(&s).unwrap();
    let first: serde_json::Value = serde_json::from_str(jsonl.lines().next().unwrap()).unwrap();
    for key in [
        "agent", "channel", "session", "title", "project", "date", "seq", "role", "text",
    ] {
        assert!(first.get(key).is_some(), "missing {key}");
    }
    assert_eq!(first["channel"], "cli");
    assert_eq!(first["role"], "user");
    assert!(first.get("actions").is_none(), "empty actions are omitted");
}

#[test]
fn output_path_is_stable() {
    let s = parse_transcript(FIXTURE, &ProjectResolver::default()).unwrap();
    let p = relative_path(&s).to_string_lossy().into_owned();
    assert!(p.starts_with("logs/claude-code/2026/10/2026-10-2"), "{p}");
    assert!(p.ends_with("_rss-reader_sess1234.jsonl"), "{p}");
    assert_eq!(
        p,
        relative_path(&parse_transcript(FIXTURE, &ProjectResolver::default()).unwrap())
            .to_string_lossy()
    );
}

#[test]
fn empty_or_anonymous_transcripts_yield_nothing() {
    assert!(parse_transcript("", &ProjectResolver::default()).is_none());
    assert!(parse_transcript("{\"type\":\"cost-state\"}\n", &ProjectResolver::default()).is_none());
}

#[test]
fn tool_descriptions() {
    assert_eq!(
        describe_tool("Grep", &json!({"pattern": "fn main"}), "/x"),
        "Grep: fn main"
    );
    assert_eq!(
        describe_tool("Edit", &json!({"file_path": "/elsewhere/a.rs"}), "/x"),
        "Edit: /elsewhere/a.rs"
    );
    assert_eq!(
        describe_tool("mcp__github__create_issue", &json!({"title": "t"}), "/x"),
        "mcp__github__create_issue"
    );
    let long = "x".repeat(100);
    assert!(describe_tool("Bash", &json!({"command": long}), "/x").ends_with('…'));
}
