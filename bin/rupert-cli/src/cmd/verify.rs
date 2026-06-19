//! `rupert verify` — re-run the verifier on stored results, rewriting
//! disqualified rows.

use std::io::Write as _;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rupert_core::{Certification, RunOutcome, RunResult, Solution};
use rupert_verify::{VerifyError, certify, certify_exact, certify_interval};

#[derive(clap::Args, Debug)]
pub(crate) struct VerifyArgs {
    /// JSONL file or directory of `*.jsonl` files. Files are rewritten in
    /// place.
    path: PathBuf,
}

pub(crate) fn run(args: &VerifyArgs) -> Result<()> {
    let mut out = std::io::stdout().lock();
    let mut total = 0usize;
    let mut promoted = 0usize;
    let mut disqualified = 0usize;
    let files = collect_files(&args.path)?;
    for file in &files {
        let updated = process_file(file)?;
        total += updated.processed;
        promoted += updated.promoted;
        disqualified += updated.disqualified;
        writeln!(
            out,
            "{}: processed={} promoted={} disqualified={}",
            file.display(),
            updated.processed,
            updated.promoted,
            updated.disqualified
        )?;
    }
    writeln!(
        out,
        "total: files={} processed={} promoted={} disqualified={}",
        files.len(),
        total,
        promoted,
        disqualified
    )?;
    Ok(())
}

#[derive(Default)]
struct UpdateStats {
    processed: usize,
    promoted: usize,
    disqualified: usize,
}

fn collect_files(path: &PathBuf) -> Result<Vec<PathBuf>> {
    if path.is_dir() {
        let mut out: Vec<PathBuf> = Vec::new();
        for entry in walkdir::WalkDir::new(path).max_depth(2) {
            let entry = entry.context("walk dir")?;
            if entry.file_type().is_file() && entry.path().extension().is_some_and(|e| e == "jsonl")
            {
                out.push(entry.path().to_path_buf());
            }
        }
        Ok(out)
    } else if path.is_file() {
        Ok(vec![path.clone()])
    } else {
        anyhow::bail!("path is neither file nor dir: {}", path.display())
    }
}

fn process_file(path: &PathBuf) -> Result<UpdateStats> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut lines: Vec<RunResult> = Vec::new();
    for (lineno, line) in bytes.split(|b| *b == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let r: RunResult = serde_json::from_slice(line)
            .with_context(|| format!("parse {}:{}", path.display(), lineno + 1))?;
        lines.push(r);
    }
    let mut stats = UpdateStats::default();
    for r in &mut lines {
        stats.processed += 1;
        if !matches!(r.outcome, RunOutcome::Solved) {
            continue;
        }
        let Some(sol) = r.solution.as_mut() else {
            continue;
        };
        let Some(poly) = rupert_shapes::lookup(&r.poly_name) else {
            continue;
        };
        match strongest_certification(sol, &poly) {
            Ok(cert) => {
                sol.certification = Some(cert);
                stats.promoted += 1;
            }
            Err(e) => {
                let reason = format!("verifier rejected: {e}");
                r.outcome = RunOutcome::Disqualified { reason };
                sol.certification = None;
                stats.disqualified += 1;
            }
        }
        if let Some(obs) = r.best_positive.as_mut() {
            let obs_sol = Solution {
                candidate: obs.candidate,
                clearance: obs.clearance,
                found_at_eval: obs.observed_at_eval,
                certification: None,
            };
            obs.certification = strongest_certification(&obs_sol, &poly).ok();
        }
    }
    let mut buf: Vec<u8> = Vec::with_capacity(bytes.len() + 64);
    for r in &lines {
        let line = serde_json::to_string(r).context("serialize")?;
        buf.extend_from_slice(line.as_bytes());
        buf.push(b'\n');
    }
    std::fs::write(path, &buf).with_context(|| format!("write {}", path.display()))?;
    Ok(stats)
}

fn strongest_certification(
    sol: &Solution,
    poly: &rupert_core::Polyhedron,
) -> Result<Certification, VerifyError> {
    if let Ok(cert) = certify_exact(sol, poly) {
        return Ok(cert);
    }

    let interval_attempt = if poly.exact_vertices.is_some() {
        certify_interval(sol, poly).ok()
    } else {
        None
    };
    match interval_attempt {
        Some(cert) => Ok(cert),
        None => certify(sol, poly),
    }
}
