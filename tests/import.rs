//! Parsing of official exports (synthetic fixtures mirroring the real
//! shapes) and `import` end to end against a local bare repository.

use ruminate::config::Config;
use ruminate::import::{ImportOptions, run};
use ruminate::model::{Channel, Role};
use ruminate::output::{Renderer, relative_path};
use ruminate::redact::Redactor;
use ruminate::source::{chatgpt, claude_ai, conversations, detect_export};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

const CLAUDE_AI: &str = include_str!("fixtures/claude_ai_export.json");
const CHATGPT: &str = include_str!("fixtures/chatgpt_export.json");

fn doc(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn formats_are_told_apart_by_shape() {
    assert_eq!(detect_export(&doc(CLAUDE_AI)).unwrap().name(), "claude-ai");
    assert_eq!(detect_export(&doc(CHATGPT)).unwrap().name(), "chatgpt");
    assert!(detect_export(&json!([{"foo": 1}])).is_none());
    assert!(detect_export(&json!([])).is_none());
    // A single conversation saved on its own is fine too.
    let one = conversations(&doc(CLAUDE_AI))[0].clone();
    assert_eq!(detect_export(&one).unwrap().name(), "claude-ai");
}

#[test]
fn claude_ai_keeps_only_the_conversation() {
    let d = doc(CLAUDE_AI);
    let convs = conversations(&d);
    let s = claude_ai::parse_conversation(convs[0]).expect("session");
    assert_eq!(s.agent, "claude-ai");
    assert_eq!(s.channel, Channel::Web);
    assert_eq!(s.id, "c0ffee00-1111-2222-3333-444444444444");
    assert_eq!(s.title.as_deref(), Some("DuckDB で JSONL を読む"));
    assert_eq!(s.project, "");
    assert_eq!(
        s.url.as_deref(),
        Some("https://claude.ai/chat/c0ffee00-1111-2222-3333-444444444444")
    );
    assert_eq!(
        s.started_at.to_rfc3339(),
        "2026-10-20T03:00:00.123456+00:00"
    );
    assert_eq!(s.unknown_lines, 0);

    let roles: Vec<Role> = s.turns.iter().map(|t| t.role).collect();
    assert_eq!(
        roles,
        [Role::User, Role::Assistant, Role::User, Role::Assistant]
    );
    // Attachments are named, never read.
    assert_eq!(
        s.turns[0].text,
        "DuckDB で JSONL のフォルダをまとめて読むには？"
    );
    assert_eq!(s.turns[0].actions, ["attachment: sample.jsonl"]);
    // Text blocks join; thinking and tool results are dropped.
    assert!(s.turns[1].text.starts_with("`read_json("));
    assert!(s.turns[1].text.contains("\n\nトークンは"));
    assert_eq!(s.turns[1].actions, ["web_search: duckdb read_json glob"]);
    // Older shape: only `text`, no `content`.
    assert_eq!(s.turns[3].text, "どういたしまして。");
    let all: String = s.turns.iter().map(|t| t.text.as_str()).collect();
    assert!(!all.contains("secret reasoning"));
    assert!(!all.contains("should-not-appear"));

    // Nothing said: nothing kept.
    assert!(claude_ai::parse_conversation(convs[1]).is_none());
}

#[test]
fn chatgpt_follows_the_current_branch() {
    let d = doc(CHATGPT);
    let convs = conversations(&d);
    let s = chatgpt::parse_conversation(convs[0]).expect("session");
    assert_eq!(s.agent, "chatgpt");
    assert_eq!(s.channel, Channel::Web);
    assert_eq!(s.id, "7a7a7a7a-0000-0000-0000-000000000000");
    assert_eq!(s.title.as_deref(), Some("Rust のライフタイム"));
    assert_eq!(
        s.url.as_deref(),
        Some("https://chatgpt.com/c/7a7a7a7a-0000-0000-0000-000000000000")
    );
    assert_eq!(s.started_at.timestamp_millis(), 1_792_460_000_500);
    assert_eq!(s.unknown_lines, 0);

    let roles: Vec<Role> = s.turns.iter().map(|t| t.role).collect();
    assert_eq!(
        roles,
        [Role::User, Role::Assistant, Role::User, Role::Assistant]
    );
    assert_eq!(s.turns[0].text, "ライフタイムの 'a ってなに？");
    // Reply steps merge; citation markers go; image parts are dropped;
    // a message without a time takes the previous one.
    assert_eq!(
        s.turns[1].text,
        "参照が有効な期間に付ける名前です。\n\n図も付けました。"
    );
    assert_eq!(
        s.turns[1].actions,
        ["browser: search(\"rust lifetime 'a\")"]
    );
    assert_eq!(s.turns[3].actions, ["python: print(1)"]);
    assert_eq!(s.turns[3].text, "それは貼らないでください。");
    let all: String = s.turns.iter().map(|t| t.text.as_str()).collect();
    for hidden in [
        "abandoned branch",
        "secret reasoning",
        "should-not-appear",
        "I am a developer",
        "Be brief",
    ] {
        assert!(!all.contains(hidden), "leaked: {hidden}");
    }

    assert!(chatgpt::parse_conversation(convs[1]).is_none());
}

#[test]
fn chatgpt_without_current_node_takes_the_newest_leaf() {
    let d = doc(CHATGPT);
    let mut c = conversations(&d)[0].clone();
    c.as_object_mut().unwrap().remove("current_node");
    let s = chatgpt::parse_conversation(&c).unwrap();
    assert_eq!(s.turns.len(), 4);
    assert_eq!(s.turns[3].text, "それは貼らないでください。");
}

#[test]
fn web_sessions_have_no_project_in_the_path() {
    let d = doc(CLAUDE_AI);
    let s = claude_ai::parse_conversation(conversations(&d)[0]).unwrap();
    let p = relative_path(&s);
    assert!(p.starts_with("logs/claude-ai/2026/10"), "{}", p.display());
    let name = p.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.ends_with("_c0ffee00.jsonl"), "{name}");
    assert!(!name.contains("unknown"));
    // Rendering redacts as for Claude Code.
    let redactor = Redactor::new(&[]).unwrap();
    let r = Renderer {
        redactor: &redactor,
        code_max_lines: 40,
    };
    let recs = r.records(&s);
    assert!(recs[1].text.contains("トークンは [REDACTED] です"));
    assert_eq!(recs[0].project, "");
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// One test so the environment variables below are never shared between
/// parallel tests.
#[test]
fn import_writes_and_pushes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    unsafe {
        std::env::set_var("RUMINATE_STATE_DIR", root.join("state"));
        std::env::set_var("GIT_AUTHOR_NAME", "test");
        std::env::set_var("GIT_AUTHOR_EMAIL", "test@example.com");
        std::env::set_var("GIT_COMMITTER_NAME", "test");
        std::env::set_var("GIT_COMMITTER_EMAIL", "test@example.com");
    }
    let remote = root.join("remote.git");
    Command::new("git")
        .args(["init", "-q", "--bare", "-b", "main"])
        .arg(&remote)
        .status()
        .unwrap();
    let claude = root.join("claude/conversations.json");
    let chatgpt = root.join("chatgpt/conversations.json");
    std::fs::create_dir_all(claude.parent().unwrap()).unwrap();
    std::fs::create_dir_all(chatgpt.parent().unwrap()).unwrap();
    std::fs::write(&claude, CLAUDE_AI).unwrap();
    std::fs::write(&chatgpt, CHATGPT).unwrap();

    let mut cfg = Config::new(remote.to_string_lossy().into_owned());
    cfg.clone_dir = root.join("clone");

    // Unknown format: refused before anything is touched.
    let bad = root.join("bad.json");
    std::fs::write(&bad, "[{\"foo\": 1}]").unwrap();
    assert!(run(&cfg, &ImportOptions::default(), &[bad]).is_err());
    assert!(!cfg.clone_dir.exists());

    // Dry run touches nothing.
    let r = run(
        &cfg,
        &ImportOptions {
            dry_run: true,
            ..Default::default()
        },
        &[claude.clone(), chatgpt.clone()],
    )
    .unwrap();
    assert_eq!((r.written, r.skipped), (2, 2));
    assert_eq!(r.agents, ["claude-ai", "chatgpt"]);
    assert!(!cfg.clone_dir.exists());

    let r = run(&cfg, &ImportOptions::default(), &[claude.clone(), chatgpt]).unwrap();
    assert_eq!((r.written, r.skipped), (2, 2));
    assert!(r.pushed);
    let files = git(&remote, &["ls-tree", "-r", "--name-only", "main"]);
    assert_eq!(files.lines().count(), 2, "{files}");
    assert!(files.contains("logs/claude-ai/2026/10/"));
    assert!(files.contains("logs/chatgpt/2026/10/"));
    let msg = git(&remote, &["log", "-1", "--format=%s", "main"]);
    assert_eq!(
        msg.trim(),
        "ruminate: import 2 conversation(s) from claude-ai, chatgpt"
    );

    // The same export again: same files, nothing to commit.
    let r = run(&cfg, &ImportOptions::default(), &[claude]).unwrap();
    assert_eq!(r.written, 1);
    assert!(!r.pushed);
    assert_eq!(git(&remote, &["rev-list", "--count", "main"]).trim(), "1");
}
