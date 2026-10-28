//! `sync` end to end against a local bare repository standing in for GitHub.

use ruminate::config::Config;
use ruminate::sync::{SyncOptions, run};
use std::path::Path;
use std::process::Command;

const FIXTURE: &str = include_str!("fixtures/claude_code_basic.jsonl");

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
fn sync_writes_pushes_and_skips_unchanged() {
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

    let projects = root.join("projects");
    let proj = projects.join("-Users-me-src-rss-reader");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join("sess.jsonl"), FIXTURE).unwrap();
    let excluded = projects.join("-Users-me-src-work-api");
    std::fs::create_dir_all(&excluded).unwrap();
    std::fs::write(
        excluded.join("w.jsonl"),
        FIXTURE.replace("/Users/me/src/rss-reader", "/Users/me/src/work-api"),
    )
    .unwrap();

    let mut cfg = Config::new(remote.to_string_lossy().into_owned());
    cfg.clone_dir = root.join("clone");
    cfg.exclude_projects = vec!["work-*".into()];
    cfg.claude_code.projects_dir = Some(projects.clone());

    // Dry run touches nothing.
    let r = run(
        &cfg,
        &SyncOptions {
            dry_run: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((r.written, r.skipped), (1, 1));
    assert!(!cfg.clone_dir.exists());

    // First real run clones, writes one file, pushes.
    let r = run(&cfg, &SyncOptions::default()).unwrap();
    assert_eq!((r.written, r.skipped, r.unchanged), (1, 1, 0));
    assert!(r.pushed);
    let files = git(&remote, &["ls-tree", "-r", "--name-only", "main"]);
    assert_eq!(files.lines().count(), 1, "{files}");
    assert!(files.starts_with("logs/claude-code/2026/10/"));
    assert!(files.contains("rss-reader"));

    // Nothing changed: nothing written, no new commit.
    let r = run(&cfg, &SyncOptions::default()).unwrap();
    assert_eq!((r.written, r.unchanged), (0, 2));
    assert!(!r.pushed);

    // The session continues: the same file is rewritten, not duplicated.
    let more = r#"{"type":"user","sessionId":"sess-1234abcd-0000-0000-0000-000000000000","cwd":"/Users/me/src/rss-reader","timestamp":"2026-10-27T12:00:00.000Z","uuid":"late","message":{"role":"user","content":"続きです"}}"#;
    std::fs::write(proj.join("sess.jsonl"), format!("{FIXTURE}{more}\n")).unwrap();
    let r = run(&cfg, &SyncOptions::default()).unwrap();
    assert_eq!(r.written, 1);
    assert!(r.pushed);
    let files = git(&remote, &["ls-tree", "-r", "--name-only", "main"]);
    assert_eq!(files.lines().count(), 1);
    let content = git(&remote, &["show", &format!("main:{}", files.trim())]);
    assert_eq!(content.lines().count(), 5);
    assert!(content.contains("続きです"));
    assert_eq!(git(&remote, &["rev-list", "--count", "main"]).trim(), "2");
}
