//! `xtask check-rust-test-surface` — enforce per-crate test surface mode
//! per `xtask/test-surface.toml`.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::workspace_root;

#[derive(Debug, Deserialize)]
struct SurfaceFile {
    crates: BTreeMap<String, String>,
}

pub(crate) fn run() -> Result<()> {
    let root = workspace_root();
    let surface_path = root.join("xtask/test-surface.toml");
    let text = std::fs::read_to_string(&surface_path)
        .with_context(|| format!("read {}", surface_path.display()))?;
    let surface: SurfaceFile =
        toml::from_str(&text).with_context(|| format!("parse {}", surface_path.display()))?;
    let mut violations: Vec<String> = Vec::new();
    for (crate_name, mode) in &surface.crates {
        let crate_dir = locate_crate(&root, crate_name).with_context(|| {
            format!("locate crate {crate_name} (declared in test-surface.toml)")
        })?;
        match mode.as_str() {
            "rust" => check_rust_only(crate_name, &crate_dir, &mut violations),
            "bdd" => check_bdd_present(crate_name, &crate_dir, &mut violations),
            other => violations.push(format!(
                "{crate_name}: invalid mode '{other}' in test-surface.toml; expected 'rust' or 'bdd'"
            )),
        }
    }
    let mut out = std::io::stdout().lock();
    if violations.is_empty() {
        writeln!(out, "check-rust-test-surface: OK")?;
        return Ok(());
    }
    let mut err = std::io::stderr().lock();
    for v in &violations {
        writeln!(err, "{v}")?;
    }
    bail!(
        "check-rust-test-surface: {} violation(s)",
        violations.len()
    );
}

fn locate_crate(root: &Path, name: &str) -> Result<std::path::PathBuf> {
    for parent in ["bin", "crates"] {
        let p = root.join(parent).join(name);
        if p.is_dir() {
            return Ok(p);
        }
    }
    let p = root.join(name);
    if p.is_dir() {
        return Ok(p);
    }
    anyhow::bail!("crate '{name}' not found under bin/, crates/, or workspace root");
}

fn check_rust_only(name: &str, crate_dir: &Path, violations: &mut Vec<String>) {
    // Crates marked "rust" must NOT have a tests/bdd directory — the BDD
    // harness is reserved for orchestration crates.
    let bdd_dir = crate_dir.join("tests/bdd");
    if bdd_dir.is_dir() {
        violations.push(format!(
            "{name}: tests/bdd/ exists but mode is 'rust'; either move scenarios to a 'bdd' crate, or update test-surface.toml"
        ));
    }
}

fn check_bdd_present(name: &str, crate_dir: &Path, violations: &mut Vec<String>) {
    // Crates marked "bdd" must have a tests/bdd directory (with at least
    // a feature file). We don't check feature-file content yet — that's
    // a separate gate the agent can add later.
    let bdd_dir = crate_dir.join("tests/bdd");
    if !bdd_dir.is_dir() {
        violations.push(format!(
            "{name}: tests/bdd/ missing but mode is 'bdd'; create tests/bdd/main.rs and tests/bdd/features/*.feature"
        ));
    }
}
