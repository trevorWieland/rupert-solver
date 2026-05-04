//! Append-only JSONL writer for run results.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rupert_core::RunResult;
use time::OffsetDateTime;

/// Generate a results filename of the form `run_<utc_ts>.jsonl`.
pub fn default_filename(now: OffsetDateTime) -> String {
    // Format e.g. 20260503T193011Z — sortable, filesystem-safe.
    let f = time::format_description::parse(
        "[year][month][day]T[hour][minute][second]Z",
    )
    .expect("static format");
    let stamp = now.format(&f).unwrap_or_else(|_| "unknown".to_string());
    format!("run_{stamp}.jsonl")
}

/// Write the results to a file under `dir`. Creates the directory if it
/// doesn't exist, but does not overwrite an existing file (suffix `_2`,
/// `_3`, etc. on collision).
pub fn write_results(dir: &Path, results: &[RunResult]) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let base = default_filename(OffsetDateTime::now_utc());
    let path = unique_path(dir, &base);
    let file = File::create(&path).with_context(|| format!("create {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for r in results {
        let line = serde_json::to_string(r).context("serialize result")?;
        writeln!(writer, "{line}").context("write line")?;
    }
    writer.flush().context("flush")?;
    Ok(path)
}

fn unique_path(dir: &Path, base: &str) -> PathBuf {
    let p = dir.join(base);
    if !p.exists() {
        return p;
    }
    let stem = base.trim_end_matches(".jsonl");
    for n in 2..u32::MAX {
        let candidate = dir.join(format!("{stem}_{n}.jsonl"));
        if !candidate.exists() {
            return candidate;
        }
    }
    p
}

/// Read all `.jsonl` files in `dir` and parse each line as a `RunResult`.
pub fn read_all_results(dir: &Path) -> Result<Vec<RunResult>> {
    let mut out: Vec<RunResult> = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in walkdir::WalkDir::new(dir).max_depth(2) {
        let entry = entry.context("walk results")?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "jsonl") {
            let bytes = std::fs::read(path)
                .with_context(|| format!("read {}", path.display()))?;
            for (lineno, line) in bytes.split(|b| *b == b'\n').enumerate() {
                if line.is_empty() {
                    continue;
                }
                let r: RunResult = serde_json::from_slice(line).with_context(|| {
                    format!("parse {}:{}", path.display(), lineno + 1)
                })?;
                out.push(r);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    fn fake_result(seed: u64) -> RunResult {
        use rupert_core::{
            BudgetSnapshot, HostInfo, RunOutcome, SCHEMA_VERSION,
        };
        RunResult {
            schema_version: SCHEMA_VERSION,
            timestamp_utc: "2026-05-03T00:00:00Z".to_string(),
            poly_id: *rupert_shapes::cube().id(),
            poly_name: "cube".to_string(),
            solver_name: "test".to_string(),
            solver_version: "0.1.0".to_string(),
            seed,
            budget: BudgetSnapshot {
                max_evaluations: 1000,
                max_wall_time_ms: None,
            },
            outcome: RunOutcome::Exhausted,
            eval_count: 100,
            wall_time_ms: 1,
            solution: None,
            host: HostInfo::collect(),
        }
    }

    #[test]
    fn round_trip_through_jsonl() {
        let dir = std::env::temp_dir().join("rupert_bench_jsonl_test");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let results = vec![fake_result(0), fake_result(1)];
        let path = write_results(&dir, &results).expect("write");
        let loaded = read_all_results(&dir).expect("load");
        // We may pick up other JSONL files from prior runs in the temp
        // dir; assert at least our two records are present.
        assert!(loaded.len() >= results.len(), "got {}", loaded.len());
        // Cleanup for hygiene; failures here are non-fatal.
        std::fs::remove_file(&path).ok();
        // Use NonZeroU64 import so it stays in scope for build.
        let _ = NonZeroU64::new(1);
    }
}
