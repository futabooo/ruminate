//! Which project (repository) a session belongs to, from the directory it
//! started in.
//!
//! The directory's own name is often wrong: sessions run in git worktrees
//! (`<repo>/.claude/worktrees/<name>` from Claude Code itself,
//! `~/.herdr/worktrees/<repo>/<name>` from Herdr) or in a subdirectory of
//! the repository. Resolution goes through a fixed list of steps and the
//! first answer wins:
//!
//! 1. path rules — regular expressions with a `repo` capture group, matched
//!    against the path. [`BUILTIN_RULES`] covers known worktree layouts;
//!    `project_patterns` in the config file adds more. These come first
//!    because a worktree is usually gone by the time an old session is
//!    converted, so only its path is left to look at. When several rules
//!    match, the one matching furthest left wins: the outermost layout is
//!    the repository (a Claude Code worktree inside a Herdr worktree still
//!    belongs to the Herdr worktree's repository);
//! 2. git — if the directory still exists, the main repository's name via
//!    `git rev-parse --git-common-dir` (a linked worktree points back at
//!    the main `.git`; a subdirectory resolves to its repository).
//!    Submodules and a repository at `$HOME` (dotfiles) are not used;
//! 3. the directory's own name.
//!
//! To support a new layout, add a rule to [`BUILTIN_RULES`] and a case to
//! the tests below. Each rule is only ever asked one thing: "given this
//! path, what is the repository called?"

use anyhow::{Context, Result};
use regex::Regex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// `(name, pattern)`; the pattern must have a capture group called `repo`.
/// Paths are matched with `/` separators. Among matching rules the leftmost
/// match wins; order only breaks ties.
pub const BUILTIN_RULES: &[(&str, &str)] = &[
    // Claude Code's own worktrees: <repo>/.claude/worktrees/<name>[/...]
    (
        "claude-code-worktree",
        r"/(?P<repo>[^/]+)/\.claude/worktrees/[^/]+",
    ),
    // Herdr (and the common "one folder of worktrees per repo" layout):
    // <somewhere>/worktrees/<repo>/<name>[/...]
    ("worktrees-dir", r"/worktrees/(?P<repo>[^/]+)/[^/]+"),
];

pub const FALLBACK: &str = "unknown";

struct Rule {
    name: String,
    re: Regex,
}

/// Turns a session's starting directory into a project name.
pub struct ProjectResolver {
    rules: Vec<Rule>,
    /// Ask git when the directory still exists. Off in tests that use made-up
    /// paths and must not depend on the machine they run on.
    use_git: bool,
    /// Sessions start in the same handful of directories; ask git once each.
    git_cache: RefCell<HashMap<String, Option<String>>>,
}

impl Default for ProjectResolver {
    fn default() -> Self {
        Self::new(&[]).expect("built-in rules compile")
    }
}

impl ProjectResolver {
    /// Built-in rules followed by `extra` (from `project_patterns`), each a
    /// regular expression with a `repo` capture group.
    pub fn new(extra: &[String]) -> Result<Self> {
        let mut rules = Vec::new();
        for (name, pattern) in BUILTIN_RULES {
            rules.push(Rule {
                name: (*name).to_string(),
                re: Regex::new(pattern).expect("built-in rule"),
            });
        }
        for (i, pattern) in extra.iter().enumerate() {
            let re = Regex::new(pattern)
                .with_context(|| format!("project_patterns の正規表現が不正です: {pattern}"))?;
            if !re.capture_names().any(|n| n == Some("repo")) {
                anyhow::bail!(
                    "project_patterns の正規表現に (?P<repo>...) がありません: {pattern}"
                );
            }
            rules.push(Rule {
                name: format!("project_patterns[{i}]"),
                re,
            });
        }
        Ok(Self {
            rules,
            use_git: true,
            git_cache: RefCell::new(HashMap::new()),
        })
    }

    /// Path rules and the directory name only; never runs git.
    pub fn without_git(mut self) -> Self {
        self.use_git = false;
        self
    }

    pub fn resolve(&self, cwd: &str) -> String {
        if cwd.is_empty() {
            return FALLBACK.to_string();
        }
        if let Some((_, name)) = self.match_rule(cwd) {
            return name;
        }
        if self.use_git
            && let Some(name) = self
                .git_cache
                .borrow_mut()
                .entry(cwd.to_string())
                .or_insert_with(|| repo_name_from_git(cwd))
                .clone()
        {
            return name;
        }
        Path::new(cwd)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| FALLBACK.to_string())
    }

    /// The rule whose match starts furthest left, with the repository name
    /// it found. Public so a `--explain` style command can show which rule
    /// fired.
    pub fn match_rule(&self, cwd: &str) -> Option<(&str, String)> {
        let path = cwd.replace('\\', "/");
        self.rules
            .iter()
            .filter_map(|r| {
                let c = r.re.captures(&path)?;
                let whole = c.get(0)?;
                let repo = c.name("repo")?;
                Some((whole.start(), r.name.as_str(), repo.as_str().to_string()))
            })
            .min_by_key(|(start, _, _)| *start)
            .map(|(_, name, repo)| (name, repo))
    }
}

/// Name of the main repository directory, via git's common dir. `None` for
/// a submodule (its common dir is `<super>/.git/modules/<name>`, which
/// names nothing useful), a bare repository, or a repository at `$HOME`
/// (dotfiles: every stray directory would otherwise be "the home repo").
fn repo_name_from_git(cwd: &str) -> Option<String> {
    if !Path::new(cwd).is_dir() {
        return None;
    }
    repo_root_from_git(cwd)
        .filter(|root| Some(root.as_path()) != crate::config::home_dir().as_deref())
        .and_then(|root| root.file_name().map(|n| n.to_string_lossy().into_owned()))
}

fn repo_root_from_git(cwd: &str) -> Option<PathBuf> {
    let out = std::process::Command::new("git")
        .args([
            "-C",
            cwd,
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let git_dir = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    if git_dir.file_name()? != ".git" {
        return None;
    }
    git_dir.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> ProjectResolver {
        ProjectResolver::default().without_git()
    }

    #[test]
    fn claude_code_worktrees() {
        let p = "/Users/me/src/Riamo/.claude/worktrees/agent-a216732f";
        assert_eq!(r().resolve(p), "Riamo");
        assert_eq!(r().match_rule(p).unwrap().0, "claude-code-worktree");
        assert_eq!(
            r().resolve("/Users/me/src/Riamo/.claude/worktrees/deploy-1839/firebase"),
            "Riamo"
        );
    }

    #[test]
    fn worktrees_directory_per_repo() {
        let p = "/Users/me/.herdr/worktrees/Riamo/worktree-brave-forest-5518";
        assert_eq!(r().resolve(p), "Riamo");
        assert_eq!(r().match_rule(p).unwrap().0, "worktrees-dir");
        assert_eq!(
            r().resolve("/Users/me/.herdr/worktrees/Riamo/issue-814/firebase/functions"),
            "Riamo"
        );
    }

    #[test]
    fn outermost_layout_wins() {
        // A Claude Code worktree made inside a Herdr worktree.
        let p =
            "/Users/me/.herdr/worktrees/Riamo/worktree-brave-forest-5518/.claude/worktrees/agent-1";
        assert_eq!(r().resolve(p), "Riamo");
        assert_eq!(r().match_rule(p).unwrap().0, "worktrees-dir");
        // And the plain Claude Code layout still beats the generic rule.
        assert_eq!(
            r().resolve("/Users/me/src/Riamo/.claude/worktrees/deploy-1839/firebase"),
            "Riamo"
        );
    }

    #[test]
    fn plain_directories_keep_their_name() {
        assert_eq!(r().resolve("/Users/me/src/ruminate"), "ruminate");
        // A directory merely called "worktrees" is not a worktree.
        assert_eq!(r().resolve("/Users/me/src/worktrees"), "worktrees");
        assert_eq!(r().resolve("/Users/me/worktrees/only-repo"), "only-repo");
        assert_eq!(r().resolve(""), FALLBACK);
    }

    #[test]
    fn extra_patterns_from_config() {
        let res = ProjectResolver::new(&[r"/wt/(?P<repo>[^/]+)-[0-9]+$".to_string()])
            .unwrap()
            .without_git();
        assert_eq!(res.resolve("/Users/me/wt/ruminate-42"), "ruminate");
        assert_eq!(
            res.match_rule("/Users/me/wt/ruminate-42").unwrap().0,
            "project_patterns[0]"
        );
        // Built-in rules still come first.
        assert_eq!(res.resolve("/x/worktrees/Riamo/wt/ruminate-42"), "Riamo");
    }

    #[test]
    fn extra_patterns_are_validated() {
        assert!(ProjectResolver::new(&["(".to_string()]).is_err());
        assert!(ProjectResolver::new(&["/wt/([^/]+)".to_string()]).is_err());
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git runs")
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    #[test]
    fn git_resolves_subdirectories_and_worktrees() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("acme-app");
        std::fs::create_dir_all(repo.join("lib/src")).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(
            &repo,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "init",
            ],
        );
        let wt = tmp.path().join("acme-app-wt");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                &wt.to_string_lossy(),
                "-b",
                "topic",
            ],
        );

        let res = ProjectResolver::default();
        assert_eq!(
            res.resolve(&repo.join("lib/src").to_string_lossy()),
            "acme-app"
        );
        assert_eq!(res.resolve(&wt.to_string_lossy()), "acme-app");
        // Not a repository: the directory name.
        let plain = tmp.path().join("notes");
        std::fs::create_dir(&plain).unwrap();
        assert_eq!(res.resolve(&plain.to_string_lossy()), "notes");
        // Second call for the same directory is served from the cache.
        assert_eq!(res.git_cache.borrow().len(), 3);
        assert_eq!(res.resolve(&wt.to_string_lossy()), "acme-app");
        assert_eq!(res.git_cache.borrow().len(), 3);
    }
}
