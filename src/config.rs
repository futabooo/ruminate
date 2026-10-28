//! `~/.config/ruminate/config.toml` and the local state directory.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Git URL of the private data repository, e.g.
    /// `git@github.com:you/ruminate-data.git`.
    pub data_repo: String,
    /// Where ruminate keeps its own clone of the data repository.
    #[serde(default = "default_clone_dir")]
    pub clone_dir: PathBuf,
    /// Project names that are never recorded. `*` matches any run of
    /// characters, e.g. `work-*`.
    #[serde(default)]
    pub exclude_projects: Vec<String>,
    /// Code blocks longer than this keep only their first lines.
    #[serde(default = "default_code_max_lines")]
    pub code_max_lines: usize,
    /// Extra regular expressions to replace with `[REDACTED]`.
    #[serde(default)]
    pub redact_patterns: Vec<String>,
    /// Extra rules for naming the project from the directory a session
    /// started in: regular expressions with a `(?P<repo>...)` group, tried
    /// after the built-in worktree rules. See `project.rs`.
    #[serde(default)]
    pub project_patterns: Vec<String>,
    #[serde(default)]
    pub claude_code: ClaudeCodeConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClaudeCodeConfig {
    /// Defaults to `$CLAUDE_CONFIG_DIR/projects` or `~/.claude/projects`.
    pub projects_dir: Option<PathBuf>,
}

fn default_clone_dir() -> PathBuf {
    home().join(".local/share/ruminate/data")
}

fn default_code_max_lines() -> usize {
    40
}

impl Config {
    pub fn new(data_repo: String) -> Self {
        Self {
            data_repo,
            clone_dir: default_clone_dir(),
            exclude_projects: Vec::new(),
            code_max_lines: default_code_max_lines(),
            redact_patterns: Vec::new(),
            project_patterns: Vec::new(),
            claude_code: ClaudeCodeConfig::default(),
        }
    }

    pub fn load() -> Result<Self> {
        let path = config_path();
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "設定ファイルがありません: {}（先に `ruminate init` を実行してください）",
                path.display()
            )
        })?;
        let mut cfg: Config = toml::from_str(&text)
            .with_context(|| format!("設定ファイルを読めません: {}", path.display()))?;
        cfg.clone_dir = expand_tilde(&cfg.clone_dir);
        if let Some(dir) = &cfg.claude_code.projects_dir {
            cfg.claude_code.projects_dir = Some(expand_tilde(dir));
        }
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn is_excluded(&self, project: &str) -> bool {
        self.exclude_projects
            .iter()
            .any(|p| wildcard_match(p, project))
    }

    pub fn claude_projects_dir(&self) -> PathBuf {
        if let Some(dir) = &self.claude_code.projects_dir {
            return dir.clone();
        }
        match std::env::var_os("CLAUDE_CONFIG_DIR") {
            Some(dir) => PathBuf::from(dir).join("projects"),
            None => home().join(".claude/projects"),
        }
    }
}

pub fn home() -> PathBuf {
    home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("RUMINATE_CONFIG") {
        return PathBuf::from(p);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"));
    base.join("ruminate/config.toml")
}

/// Local-only state: what has been synced, the lock file and the sync log.
pub fn state_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("RUMINATE_STATE_DIR") {
        return PathBuf::from(p);
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"));
    base.join("ruminate")
}

pub fn expand_tilde(p: &Path) -> PathBuf {
    match p.strip_prefix("~") {
        Ok(rest) => home().join(rest),
        Err(_) => p.to_path_buf(),
    }
}

/// Glob-style match where `*` is the only wildcard.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let mut rest = text;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            match rest.strip_prefix(part) {
                Some(r) => rest = r,
                None => return false,
            }
        } else if i == parts.len() - 1 {
            return rest.ends_with(part);
        } else {
            match rest.find(part) {
                Some(pos) => rest = &rest[pos + part.len()..],
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::wildcard_match;

    #[test]
    fn wildcard() {
        assert!(wildcard_match("work-*", "work-api"));
        assert!(!wildcard_match("work-*", "my-work-api"));
        assert!(wildcard_match("*-secret", "app-secret"));
        assert!(wildcard_match("a*c*e", "abcde"));
        assert!(wildcard_match("exact", "exact"));
        assert!(!wildcard_match("exact", "exactly"));
    }
}
