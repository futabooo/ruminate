//! Thin wrapper over the `git` command, so the person's existing
//! credentials (SSH keys, `gh auth`, credential helpers) just work.

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::{Command, Output};

fn run(dir: &Path, args: &[&str]) -> Result<Output> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .context("git を実行できません（インストールされていますか？）")?;
    Ok(out)
}

fn check(dir: &Path, args: &[&str]) -> Result<String> {
    let out = run(dir, args)?;
    if !out.status.success() {
        bail!(
            "git {} に失敗しました: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn clone(url: &str, dir: &Path) -> Result<()> {
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let out = Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(dir)
        .output()
        .context("git を実行できません")?;
    if !out.status.success() {
        bail!(
            "clone に失敗しました: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

fn has_commits(dir: &Path) -> bool {
    run(dir, &["rev-parse", "--verify", "HEAD"]).is_ok_and(|o| o.status.success())
}

/// Commits everything under `paths` (if anything changed), rebases onto the
/// remote and pushes. Safe to call when there is nothing to do.
pub fn commit_and_push(dir: &Path, paths: &[&str], message: &str) -> Result<bool> {
    let mut add = vec!["add", "-A", "--"];
    add.extend_from_slice(paths);
    check(dir, &add)?;
    let staged = !run(dir, &["diff", "--cached", "--quiet"])?.status.success();
    if staged {
        check(dir, &["commit", "-q", "-m", message])?;
    }
    if !has_commits(dir) {
        return Ok(false);
    }
    let has_upstream = run(dir, &["rev-parse", "--abbrev-ref", "@{u}"])?
        .status
        .success();
    if has_upstream {
        // The nightly cloud tasks push to the same repo (other folders), so
        // pick up their commits first. Different files, so no conflicts.
        check(dir, &["pull", "-q", "--rebase", "--autostash"])?;
        let ahead: u32 = check(dir, &["rev-list", "--count", "@{u}..HEAD"])?
            .parse()
            .unwrap_or(0);
        if ahead > 0 {
            check(dir, &["push", "-q"])?;
        }
    } else {
        check(dir, &["push", "-q", "-u", "origin", "HEAD"])?;
    }
    Ok(staged)
}
