//! `xtask check-deps` — enforce workspace dependency layering.

use std::collections::BTreeMap;
use std::io::Write as _;

use anyhow::{Context, Result, bail};
use cargo_metadata::{DependencyKind, MetadataCommand};
use serde::Deserialize;

use crate::workspace_root;

#[derive(Debug, Deserialize)]
struct LayersFile {
    layers: BTreeMap<String, u32>,
}

pub(crate) fn run() -> Result<()> {
    let root = workspace_root();
    let layers_path = root.join("xtask/layers.toml");
    let text = std::fs::read_to_string(&layers_path)
        .with_context(|| format!("read {}", layers_path.display()))?;
    let file: LayersFile =
        toml::from_str(&text).with_context(|| format!("parse {}", layers_path.display()))?;
    let layers = file.layers;

    let metadata = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("cargo metadata")?;

    let workspace_members: std::collections::BTreeSet<String> = metadata
        .workspace_packages()
        .iter()
        .map(|p| p.name.clone())
        .collect();

    let mut violations: Vec<String> = Vec::new();
    for pkg in metadata.workspace_packages() {
        let src_layer = if let Some(n) = layers.get(pkg.name.as_str()) { *n } else {
            violations.push(format!(
                "{} is not declared in xtask/layers.toml — add an entry",
                pkg.name
            ));
            continue;
        };
        for dep in &pkg.dependencies {
            // Layering applies to runtime dependencies only — dev/test
            // and build deps may freely cross layers (e.g. rupert-solvers
            // tests use rupert-shapes for fixture builds).
            if dep.kind != DependencyKind::Normal {
                continue;
            }
            let dep_name = dep.name.as_str();
            if !workspace_members.contains(dep_name) {
                continue;
            }
            let dst_layer = if let Some(n) = layers.get(dep_name) { *n } else {
                violations.push(format!(
                    "{} -> {dep_name}: dependency missing from xtask/layers.toml",
                    pkg.name
                ));
                continue;
            };
            // src must be strictly higher (later) than dst.
            // Tooling layer (99) does not participate in the check.
            if src_layer != 99 && dst_layer != 99 && dst_layer >= src_layer {
                violations.push(format!(
                    "{} (layer {src_layer}) -> {dep_name} (layer {dst_layer}): \
                     dependency violates layering (target layer must be strictly less)",
                    pkg.name
                ));
            }
        }
    }

    let mut out = std::io::stdout().lock();
    if violations.is_empty() {
        writeln!(out, "check-deps: OK ({} crates)", workspace_members.len())?;
        return Ok(());
    }
    let mut err = std::io::stderr().lock();
    for v in &violations {
        writeln!(err, "{v}")?;
    }
    bail!("check-deps: {} layering violation(s)", violations.len());
}
