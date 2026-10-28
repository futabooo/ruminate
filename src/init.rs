//! `ruminate init`: first-time setup on a machine.

use crate::config::{Config, config_path};
use crate::git;
use crate::sync::{self, SyncOptions};
use anyhow::{Context, Result, bail};
use std::process::Command;

pub struct InitOptions {
    /// Use an existing repository.
    pub repo: Option<String>,
    /// Create a private repository with `gh`.
    pub create: Option<String>,
    pub force: bool,
}

pub fn run(opts: &InitOptions) -> Result<()> {
    let path = config_path();
    if path.exists() && !opts.force {
        bail!(
            "設定ファイルがもうあります: {}（作り直すなら --force）",
            path.display()
        );
    }

    let url = match (&opts.repo, &opts.create) {
        (Some(url), None) => url.clone(),
        (None, Some(name)) => create_private_repo(name)?,
        _ => bail!("--repo <URL> か --create <名前> のどちらかを指定してください"),
    };

    let cfg = Config::new(url.clone());
    println!("データリポジトリ: {url}");
    if !git::is_repo(&cfg.clone_dir) {
        git::clone(&url, &cfg.clone_dir)?;
    }
    for dir in ["logs", "review"] {
        let keep = cfg.clone_dir.join(dir).join(".gitkeep");
        std::fs::create_dir_all(keep.parent().expect("has parent"))?;
        if !keep.exists() {
            std::fs::write(&keep, "")?;
        }
    }
    let readme = cfg.clone_dir.join("README.md");
    if !readme.exists() {
        std::fs::write(&readme, DATA_README)?;
    }
    git::commit_and_push(
        &cfg.clone_dir,
        &["."],
        "ruminate: initialize data repository",
    )?;
    cfg.save()?;
    println!("設定ファイルを書きました: {}", path.display());

    // Convert past sessions locally, but do not push them yet: let the
    // person look first.
    let report = sync::run(
        &cfg,
        &SyncOptions {
            no_push: true,
            ..Default::default()
        },
    )?;
    println!(
        "これまでの会話を {} 件変換しました（まだ送っていません）。",
        report.written
    );
    println!();
    println!("次にやること:");
    println!("  1. 中身を確認する:        ruminate db");
    println!("     記録したくないプロジェクトがあれば、設定ファイルの exclude_projects に足す");
    println!("  2. 問題なければ送る:      ruminate sync");
    println!("  3. Claude Code にプラグインを入れる（README を参照）");
    println!("  4. claude.ai で ruminate-setup スキルを入れて「ruminateをセットアップして」と頼む");
    Ok(())
}

fn create_private_repo(name: &str) -> Result<String> {
    let out = Command::new("gh")
        .args([
            "repo",
            "create",
            name,
            "--private",
            "--description",
            "ruminate data (private)",
        ])
        .output()
        .context("gh が見つかりません。リポジトリを自分で作って --repo で指定してください")?;
    if !out.status.success() {
        bail!(
            "リポジトリを作れませんでした: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let out = Command::new("gh")
        .args(["repo", "view", name, "--json", "sshUrl", "-q", ".sshUrl"])
        .output()?;
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if url.is_empty() {
        bail!("作ったリポジトリのURLを取れませんでした");
    }
    Ok(url)
}

const DATA_README: &str = "# ruminate data

Private data for [ruminate](https://github.com/futabooo/ruminate). Do not make this repository public.

- `logs/<agent>/` — conversation logs, one JSONL file per session
- `review/` — review questions and answers, written by the nightly task

Explore with `ruminate db`.
";
