//! Where conversations come from. Each agent gets one implementation of
//! [`Source`]; everything after parsing (redaction, output, git) is shared.

pub mod claude_code;

use crate::model::Session;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub trait Source {
    /// Short agent name written to every log line, e.g. `claude-code`.
    fn name(&self) -> &'static str;

    /// Every transcript file this source can read right now.
    fn discover(&self) -> Result<Vec<PathBuf>>;

    /// Converts one transcript. `Ok(None)` means there is nothing worth
    /// keeping (no identity, or no prompts at all).
    fn parse(&self, path: &Path) -> Result<Option<Session>>;
}
