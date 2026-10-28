//! `ruminate db`: DuckDB with the views from `duckdb/init.sql`, pointed at
//! the data repository.

use crate::config::{Config, state_dir};
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

const TEMPLATE: &str = include_str!("../duckdb/init.sql");

const REVIEW_VIEWS: &str = "
CREATE OR REPLACE VIEW review_items AS
SELECT * FROM read_json('{{DATA_DIR}}/review/items.jsonl', format = 'newline_delimited');
";

const HISTORY_VIEW: &str = "
CREATE OR REPLACE VIEW review_history AS
SELECT * FROM read_json('{{DATA_DIR}}/review/history.jsonl', format = 'newline_delimited');
";

pub fn init_sql(data_dir: &Path) -> Result<String> {
    if !has_logs(&data_dir.join("logs")) {
        bail!("ログがまだありません。先に `ruminate sync` を実行してください。");
    }
    let mut sql = TEMPLATE.to_string();
    if data_dir.join("review/items.jsonl").is_file() {
        sql.push_str(REVIEW_VIEWS);
    }
    if data_dir.join("review/history.jsonl").is_file() {
        sql.push_str(HISTORY_VIEW);
    }
    let dir = data_dir.to_string_lossy().replace('\'', "''");
    Ok(sql.replace("{{DATA_DIR}}", &dir))
}

fn has_logs(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        if p.is_dir() {
            has_logs(&p)
        } else {
            p.extension().is_some_and(|x| x == "jsonl")
        }
    })
}

/// Replaces this process with `duckdb -init <views> [args...]`.
pub fn run(cfg: &Config, args: &[String]) -> Result<()> {
    let sql = init_sql(&cfg.clone_dir)?;
    let dir = state_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("init.sql");
    std::fs::write(&path, sql)?;
    let status = Command::new("duckdb")
        .arg("-init")
        .arg(&path)
        .args(args)
        .status()
        .context("duckdb が見つかりません（`brew install duckdb` などで入れてください）")?;
    std::process::exit(status.code().unwrap_or(1));
}
