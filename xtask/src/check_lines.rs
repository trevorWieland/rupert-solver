//! `xtask check-lines` — reject any `.rs` file over 500 lines.

use std::io::Write as _;

use anyhow::{Context, Result, bail};

use crate::{SOURCE_ROOTS, workspace_root};

const MAX_LINES: usize = 500;

pub(crate) fn run() -> Result<()> {
    let root = workspace_root();
    let mut violations: Vec<(std::path::PathBuf, usize)> = Vec::new();
    for src in SOURCE_ROOTS {
        let dir = root.join(src);
        if !dir.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&dir) {
            let entry = entry.with_context(|| format!("walk {}", dir.display()))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(path)
                    .with_context(|| format!("read {}", path.display()))?;
                let count = text.lines().count();
                if count > MAX_LINES {
                    violations.push((path.to_path_buf(), count));
                }
            }
        }
    }
    let mut out = std::io::stderr().lock();
    if violations.is_empty() {
        let mut so = std::io::stdout().lock();
        writeln!(so, "check-lines: OK ({MAX_LINES} threshold)")?;
        return Ok(());
    }
    for (p, n) in &violations {
        writeln!(out, "{}: {n} lines (limit {MAX_LINES})", p.display())?;
    }
    bail!(
        "check-lines: {} files over the line limit",
        violations.len()
    );
}
