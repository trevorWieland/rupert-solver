//! `xtask check-cargo-fields` — every workspace crate's `Cargo.toml` must
//! inherit the standard fields (`edition.workspace`, `rust-version.workspace`,
//! `license.workspace`, `repository.workspace`, `publish = false`,
//! `[lints] workspace = true`). The xtask itself follows the same template.

use std::io::Write as _;

use anyhow::{Context, Result, bail};

use crate::workspace_root;

const REQUIRED_INHERITS: &[&str] = &[
    "edition.workspace",
    "rust-version.workspace",
    "license.workspace",
    "repository.workspace",
];

pub(crate) fn run() -> Result<()> {
    let root = workspace_root();
    let mut violations: Vec<String> = Vec::new();
    for parent in ["bin", "crates"] {
        let dir = root.join(parent);
        if !dir.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)
            .with_context(|| format!("read {}", dir.display()))?
        {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let cargo = entry.path().join("Cargo.toml");
            if cargo.is_file() {
                check_one(&cargo, &mut violations)?;
            }
        }
    }
    let xtask = root.join("xtask/Cargo.toml");
    if xtask.is_file() {
        check_one(&xtask, &mut violations)?;
    }
    let mut out = std::io::stdout().lock();
    if violations.is_empty() {
        writeln!(out, "check-cargo-fields: OK")?;
        return Ok(());
    }
    let mut err = std::io::stderr().lock();
    for v in &violations {
        writeln!(err, "{v}")?;
    }
    bail!(
        "check-cargo-fields: {} crate(s) missing required fields",
        violations.len()
    );
}

fn check_one(cargo: &std::path::Path, violations: &mut Vec<String>) -> Result<()> {
    let text = std::fs::read_to_string(cargo)
        .with_context(|| format!("read {}", cargo.display()))?;
    for required in REQUIRED_INHERITS {
        if !text.contains(required) {
            violations.push(format!(
                "{}: missing `{required} = true` (or equivalent inherited declaration)",
                cargo.display()
            ));
        }
    }
    if !text.contains("publish = false") {
        violations.push(format!("{}: missing `publish = false`", cargo.display()));
    }
    let has_lints_block = text
        .lines()
        .any(|l| l.trim_start().starts_with("workspace = true"))
        && text.contains("[lints]");
    if !has_lints_block {
        violations.push(format!(
            "{}: missing `[lints]\\nworkspace = true`",
            cargo.display()
        ));
    }
    Ok(())
}
