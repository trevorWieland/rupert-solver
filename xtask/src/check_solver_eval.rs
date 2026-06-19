//! `xtask check-solver-eval` — reject direct clearance evaluation in solvers.

use std::io::Write as _;

use anyhow::{Context, Result, bail};

use crate::workspace_root;

pub(crate) fn run() -> Result<()> {
    let root = workspace_root();
    let dir = root.join("crates/rupert-solvers/src");
    let mut violations: Vec<(std::path::PathBuf, usize, String)> = Vec::new();
    for entry in walkdir::WalkDir::new(&dir) {
        let entry = entry.with_context(|| format!("walk {}", dir.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        for (lineno, line) in text.lines().enumerate() {
            if line.contains("evaluate_clearance") {
                violations.push((path.to_path_buf(), lineno + 1, line.to_string()));
            }
        }
    }
    if violations.is_empty() {
        let mut so = std::io::stdout().lock();
        writeln!(so, "check-solver-eval: OK")?;
        return Ok(());
    }
    let mut err = std::io::stderr().lock();
    for (p, n, line) in &violations {
        writeln!(err, "{}:{n}: {}", p.display(), line.trim())?;
    }
    bail!(
        "check-solver-eval: {} direct clearance references found; use EvalCounter::evaluate",
        violations.len()
    );
}
