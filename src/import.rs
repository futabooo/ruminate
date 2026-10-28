//! `ruminate import`: official export files (claude.ai, ChatGPT) into the
//! data repository.
//!
//! An export is a one-off download, so unlike `sync` nothing is remembered
//! about it: every conversation in the file is converted and written. The
//! output path depends only on the conversation's start time and ID, so
//! importing a newer export of the same account rewrites the same files.

use crate::config::Config;
use crate::git;
use crate::output::{Renderer, relative_path};
use crate::redact::Redactor;
use crate::source::{conversations, detect_export};
use crate::sync;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct ImportOptions {
    /// Write files but do not commit or push.
    pub no_push: bool,
    /// Print what would be written; touch nothing.
    pub dry_run: bool,
}

#[derive(Debug, Default)]
pub struct Report {
    pub written: usize,
    pub skipped: usize,
    pub unknown_lines: usize,
    pub pushed: bool,
    /// Agent names seen, in order of first appearance.
    pub agents: Vec<&'static str>,
}

pub fn run(cfg: &Config, opts: &ImportOptions, files: &[PathBuf]) -> Result<Report> {
    if files.is_empty() {
        bail!("取り込むファイルを指定してください（例: conversations.json）");
    }
    // Parse everything before touching the repository, so a bad file
    // leaves nothing half done.
    let mut sessions = Vec::new();
    let mut report = Report::default();
    for file in files {
        let (agent, parsed, skipped) = parse_file(file)?;
        if !report.agents.contains(&agent) {
            report.agents.push(agent);
        }
        report.skipped += skipped;
        sessions.extend(parsed);
    }

    let _lock = sync::lock()?;
    if !opts.dry_run && !git::is_repo(&cfg.clone_dir) {
        git::clone(&cfg.data_repo, &cfg.clone_dir)?;
    }
    let redactor = Redactor::new(&cfg.redact_patterns)?;
    let renderer = Renderer {
        redactor: &redactor,
        code_max_lines: cfg.code_max_lines,
    };

    // Two conversations can only share a path if they share an ID and a
    // start time; the later one in the file wins, as a newer export would.
    let mut outputs: BTreeMap<PathBuf, String> = BTreeMap::new();
    for s in &sessions {
        report.unknown_lines += s.unknown_lines;
        let rel = relative_path(s);
        if opts.dry_run {
            println!(
                "{}  ({} turns, {})",
                rel.display(),
                s.turns.len(),
                s.title.as_deref().unwrap_or("-")
            );
        }
        outputs.insert(rel, renderer.to_jsonl(s)?);
    }
    report.written = outputs.len();
    if opts.dry_run {
        return Ok(report);
    }
    for (rel, jsonl) in &outputs {
        let full = cfg.clone_dir.join(rel);
        std::fs::create_dir_all(full.parent().expect("has parent"))?;
        std::fs::write(&full, jsonl)?;
    }
    if report.unknown_lines > 0 {
        eprintln!(
            "ruminate: 読めないメッセージが {} 件ありました。エクスポートの形式が変わったかもしれません。",
            report.unknown_lines
        );
    }
    if !opts.no_push {
        let msg = format!(
            "ruminate: import {} conversation(s) from {}",
            report.written,
            report.agents.join(", ")
        );
        report.pushed = git::commit_and_push(&cfg.clone_dir, &["logs"], &msg)?;
    }
    Ok(report)
}

/// Every conversation in one export file, with the agent it came from and
/// how many conversations held nothing worth keeping.
fn parse_file(path: &Path) -> Result<(&'static str, Vec<crate::model::Session>, usize)> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("読めません: {}", path.display()))?;
    let doc: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("JSON として読めません: {}", path.display()))?;
    let Some(export) = detect_export(&doc) else {
        bail!(
            "エクスポートの形式がわかりません: {}（claude.ai と ChatGPT の conversations.json に対応しています）",
            path.display()
        );
    };
    let mut sessions = Vec::new();
    let mut skipped = 0;
    for c in conversations(&doc) {
        if !export.matches(c) {
            skipped += 1;
            continue;
        }
        match export.parse(c) {
            Some(s) => sessions.push(s),
            None => skipped += 1,
        }
    }
    Ok((export.name(), sessions, skipped))
}
