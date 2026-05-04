//! rupert-leaderboard — aggregate JSONL run results into LEADERBOARD.md.

pub mod aggregate;
pub mod render;

use std::path::Path;

use anyhow::{Context, Result};
use rupert_core::RunResult;

pub use aggregate::{AggregatedView, LeaderboardRow, aggregate};
pub use render::render;

/// Read every `.jsonl` file under `results_dir` and return an aggregated view.
/// Re-exported here so callers (CLI, tests) only need to import this crate.
pub fn build_view_from_dir(results_dir: &Path) -> Result<AggregatedView> {
    let results = read_results(results_dir)?;
    Ok(aggregate(&results))
}

/// Convenience: build the markdown directly.
pub fn build_markdown_from_dir(results_dir: &Path) -> Result<String> {
    let view = build_view_from_dir(results_dir)?;
    Ok(render(&view))
}

fn read_results(dir: &Path) -> Result<Vec<RunResult>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out: Vec<RunResult> = Vec::new();
    for entry in walkdir::WalkDir::new(dir).max_depth(2) {
        let entry = entry.context("walk results")?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "jsonl") {
            let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
            for (lineno, line) in bytes.split(|b| *b == b'\n').enumerate() {
                if line.is_empty() {
                    continue;
                }
                let r: RunResult = serde_json::from_slice(line)
                    .with_context(|| format!("parse {}:{}", path.display(), lineno + 1))?;
                out.push(r);
            }
        }
    }
    Ok(out)
}
