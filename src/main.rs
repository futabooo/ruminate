use anyhow::Result;
use clap::{Parser, Subcommand};
use ruminate::config::Config;
use ruminate::{db, import, init, sync};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(
    version,
    about = "Keep your conversations with AI agents, and come back to them."
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// First-time setup: data repository, config file, first conversion.
    Init {
        /// URL of an existing (private) data repository.
        #[arg(long, conflicts_with = "create")]
        repo: Option<String>,
        /// Create a private repository with this name using `gh`.
        #[arg(long)]
        create: Option<String>,
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
    },
    /// Convert new or changed conversations and push them.
    Sync {
        /// Write files but do not commit or push.
        #[arg(long)]
        no_push: bool,
        /// Only print what would be written.
        #[arg(long)]
        dry_run: bool,
        /// Seconds to wait before starting.
        #[arg(long, default_value_t = 0)]
        delay: u64,
        /// Return at once and sync in a detached process.
        #[arg(long, conflicts_with_all = ["no_push", "dry_run", "all"])]
        background: bool,
        /// Convert every conversation again (e.g. after changing redaction rules).
        #[arg(long)]
        all: bool,
    },
    /// Import official export files (claude.ai, ChatGPT: `conversations.json`).
    Import {
        /// Export files to read. The format is told from the contents.
        #[arg(required = true, value_name = "FILE")]
        files: Vec<PathBuf>,
        /// Write files but do not commit or push.
        #[arg(long)]
        no_push: bool,
        /// Only print what would be written.
        #[arg(long)]
        dry_run: bool,
    },
    /// Entry point for Claude Code hooks (SessionEnd / SessionStart).
    Hook {
        /// Seconds the background sync waits for the transcript to settle.
        #[arg(long, default_value_t = 3)]
        delay: u64,
    },
    /// Open DuckDB with views over the data repository.
    Db {
        /// Print the view definitions (for a GUI client) instead of starting duckdb.
        #[arg(long)]
        sql: bool,
        /// Extra arguments for duckdb, e.g. `-ui`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Cmd::Init {
            repo,
            create,
            force,
        } => init::run(&init::InitOptions {
            repo,
            create,
            force,
        }),
        Cmd::Sync {
            no_push,
            dry_run,
            delay,
            background,
            all,
        } => {
            if background {
                return sync::spawn_background(delay);
            }
            let cfg = Config::load()?;
            let opts = SyncOptions {
                no_push,
                dry_run,
                delay: Duration::from_secs(delay),
                all,
            };
            let r = sync::run(&cfg, &opts)?;
            println!(
                "{} ruminate: written {}, unchanged {}, skipped {}{}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                r.written,
                r.unchanged,
                r.skipped,
                if r.pushed { ", pushed" } else { "" }
            );
            Ok(())
        }
        Cmd::Import {
            files,
            no_push,
            dry_run,
        } => {
            let cfg = Config::load()?;
            let opts = import::ImportOptions { no_push, dry_run };
            let r = import::run(&cfg, &opts, &files)?;
            println!(
                "{} ruminate: imported {} conversation(s) from {}, skipped {}{}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                r.written,
                r.agents.join(", "),
                r.skipped,
                if r.pushed { ", pushed" } else { "" }
            );
            Ok(())
        }
        Cmd::Hook { delay } => sync::hook(delay),
        Cmd::Db { sql: true, .. } => {
            print!("{}", db::init_sql(&Config::load()?.clone_dir)?);
            Ok(())
        }
        Cmd::Db { args, .. } => db::run(&Config::load()?, &args),
    }
}

use sync::SyncOptions;
