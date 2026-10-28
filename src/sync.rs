//! `ruminate sync` and `ruminate hook`.

use crate::config::{Config, state_dir};
use crate::git;
use crate::output::{Renderer, relative_path};
use crate::project::ProjectResolver;
use crate::redact::Redactor;
use crate::source::{Source, claude_code::ClaudeCode};
use crate::state::{Entry, FileStamp, State};
use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Default, Clone)]
pub struct SyncOptions {
    /// Write files but do not commit or push.
    pub no_push: bool,
    /// Print what would be written; touch nothing.
    pub dry_run: bool,
    /// Wait this long first (Claude Code writes the transcript lazily).
    pub delay: Duration,
    /// Convert every conversation again, even ones that have not changed
    /// (after changing redaction rules or `code_max_lines`).
    pub all: bool,
}

#[derive(Debug, Default)]
pub struct Report {
    pub written: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub unknown_lines: usize,
    pub pushed: bool,
}

pub fn sources(cfg: &Config) -> Result<Vec<Box<dyn Source>>> {
    let projects = ProjectResolver::new(&cfg.project_patterns)?;
    Ok(vec![Box::new(ClaudeCode {
        projects_dir: cfg.claude_projects_dir(),
        projects,
    })])
}

pub fn run(cfg: &Config, opts: &SyncOptions) -> Result<Report> {
    std::thread::sleep(opts.delay);
    let state_dir = state_dir();
    std::fs::create_dir_all(&state_dir)?;
    // Hooks from several sessions can fire at once; take turns.
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(state_dir.join("lock"))?;
    lock.lock_exclusive().context("ロックを取れません")?;

    if !opts.dry_run && !git::is_repo(&cfg.clone_dir) {
        git::clone(&cfg.data_repo, &cfg.clone_dir)?;
    }

    let state_path = state_dir.join("state.json");
    let mut state = State::load(&state_path);
    let redactor = Redactor::new(&cfg.redact_patterns)?;
    let renderer = Renderer {
        redactor: &redactor,
        code_max_lines: cfg.code_max_lines,
    };
    let mut report = Report::default();

    for source in sources(cfg)? {
        for path in source.discover()? {
            let Some(stamp) = FileStamp::of(&path) else {
                continue;
            };
            if !opts.all && state.is_unchanged(&path, &stamp) {
                report.unchanged += 1;
                continue;
            }
            let session = match source.parse(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("ruminate: {}: {e:#}", path.display());
                    continue;
                }
            };
            // The output path carries the project name and start time, so
            // it can change (fixed project detection, excluded project).
            // Drop the file written last time *before* writing the new one:
            // on a case-insensitive filesystem the two names may be the same
            // file, and removing afterwards would delete the fresh copy.
            let previous = state.files.get(&path).and_then(|e| e.output.clone());
            let drop_previous = |keep: Option<&PathBuf>| {
                if let Some(old) = &previous
                    && keep != Some(old)
                    && !opts.dry_run
                    && let Err(e) = std::fs::remove_file(cfg.clone_dir.join(old))
                    && e.kind() != std::io::ErrorKind::NotFound
                {
                    eprintln!("ruminate: 古い出力を消せません: {}: {e}", old.display());
                }
            };
            let output = match session {
                Some(s) if !s.turns.is_empty() && !cfg.is_excluded(&s.project) => {
                    report.unknown_lines += s.unknown_lines;
                    let rel = relative_path(&s);
                    if opts.dry_run {
                        println!(
                            "{}  ({} turns, {})",
                            rel.display(),
                            s.turns.len(),
                            s.title.as_deref().unwrap_or("-")
                        );
                    } else {
                        drop_previous(Some(&rel));
                        let full = cfg.clone_dir.join(&rel);
                        std::fs::create_dir_all(full.parent().expect("has parent"))?;
                        std::fs::write(&full, renderer.to_jsonl(&s)?)?;
                    }
                    report.written += 1;
                    Some(rel)
                }
                _ => {
                    drop_previous(None);
                    report.skipped += 1;
                    None
                }
            };
            state.files.insert(path, Entry { stamp, output });
        }
    }

    if opts.dry_run {
        return Ok(report);
    }
    state.save(&state_path)?;
    if report.unknown_lines > 0 {
        eprintln!(
            "ruminate: 読めない行が {} 行ありました。会話ファイルの形式が変わったかもしれません。",
            report.unknown_lines
        );
    }
    if !opts.no_push {
        let msg = format!("ruminate: sync {} session(s)", report.written);
        report.pushed = git::commit_and_push(&cfg.clone_dir, &["logs"], &msg)?;
    }
    Ok(report)
}

/// Called from Claude Code hooks. Returns at once and leaves the real work
/// to a detached `ruminate sync`, so the hook's time limit never matters.
pub fn hook(delay_secs: u64) -> Result<()> {
    // Drain the hook's JSON input so Claude Code never sees a broken pipe.
    let mut sink = String::new();
    let _ = std::io::stdin().read_to_string(&mut sink);
    spawn_background(delay_secs)
}

pub fn spawn_background(delay_secs: u64) -> Result<()> {
    let dir = state_dir();
    std::fs::create_dir_all(&dir)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("sync.log"))?;
    let exe: PathBuf = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("sync")
        .arg("--delay")
        .arg(delay_secs.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group, so it outlives the session that started it.
        cmd.process_group(0);
    }
    cmd.spawn()
        .context("バックグラウンドの同期を起動できません")?;
    Ok(())
}
