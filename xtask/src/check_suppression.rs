//! `xtask check-suppression` — reject inline `#[allow(...)]`, `#[expect(...)]`,
//! and `#![allow(...)]` in workspace source.

use std::io::Write as _;

use anyhow::{Context, Result, bail};

use crate::{SOURCE_ROOTS, workspace_root};

pub(crate) fn run() -> Result<()> {
    let root = workspace_root();
    let mut violations: Vec<(std::path::PathBuf, usize, String)> = Vec::new();
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
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("read {}", path.display()))?;
            for (lineno, line) in text.lines().enumerate() {
                let t = line.trim_start();
                if t.starts_with("#[allow(")
                    || t.starts_with("#[expect(")
                    || t.starts_with("#![allow(")
                {
                    violations.push((path.to_path_buf(), lineno + 1, line.to_string()));
                }
            }
        }
    }
    if violations.is_empty() {
        let mut so = std::io::stdout().lock();
        writeln!(so, "check-suppression: OK")?;
        return Ok(());
    }
    let mut err = std::io::stderr().lock();
    for (p, n, line) in &violations {
        writeln!(err, "{}:{n}: {}", p.display(), line.trim())?;
    }
    bail!(
        "check-suppression: {} inline-allow/expect attributes found; move to [lints.clippy] or [workspace.lints]",
        violations.len()
    );
}
