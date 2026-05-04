//! Parallel sweep over `(shape × solver × seed)` triples.
//!
//! Each triple becomes one task; tasks run on a rayon thread pool. Each
//! task instantiates its own solver via `rupert-solvers::lookup`, so
//! solvers are never shared across threads.

use std::num::NonZeroU64;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rayon::prelude::*;
use rupert_core::{Polyhedron, RunResult};
use rupert_solvers::registered_solvers;

use crate::runner::run_one;

#[derive(Debug, Clone)]
pub struct SweepConfig {
    pub max_evaluations: NonZeroU64,
    pub seeds: Vec<u64>,
    pub max_wall_time: Option<Duration>,
}

/// Run every (shape, solver, seed) triple and collect the results.
/// `shapes` and `solver_names` are filtered against the builtin catalog
/// and the registered solvers respectively.
pub fn run_sweep(
    shapes: &[Polyhedron],
    solver_names: &[String],
    cfg: &SweepConfig,
) -> Result<Vec<RunResult>> {
    if cfg.seeds.is_empty() {
        return Err(anyhow!("sweep requires at least one seed"));
    }
    if shapes.is_empty() {
        return Err(anyhow!("sweep requires at least one shape"));
    }
    if solver_names.is_empty() {
        return Err(anyhow!("sweep requires at least one solver"));
    }
    let mut tasks: Vec<(usize, String, u64)> =
        Vec::with_capacity(shapes.len() * solver_names.len() * cfg.seeds.len());
    for (i, _) in shapes.iter().enumerate() {
        for solver in solver_names {
            for &seed in &cfg.seeds {
                tasks.push((i, solver.clone(), seed));
            }
        }
    }
    let results: Vec<Result<RunResult>> = tasks
        .par_iter()
        .map(|(shape_idx, solver_name, seed)| {
            let poly = &shapes[*shape_idx];
            let mut solver = rupert_solvers::lookup(solver_name)
                .with_context(|| format!("unknown solver '{solver_name}'"))?;
            Ok(run_one(
                poly,
                solver.as_mut(),
                cfg.max_evaluations,
                *seed,
                cfg.max_wall_time,
            ))
        })
        .collect();
    let mut out: Vec<RunResult> = Vec::with_capacity(results.len());
    for r in results {
        out.push(r?);
    }
    Ok(out)
}

/// Convenience: every builtin shape × every registered solver × the
/// caller-supplied seed list.
pub fn run_full_sweep(cfg: &SweepConfig) -> Result<Vec<RunResult>> {
    let shapes = rupert_shapes::builtins();
    let solver_names: Vec<String> = registered_solvers()
        .iter()
        .map(|s| s.name().to_string())
        .collect();
    run_sweep(&shapes, &solver_names, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_sweep_completes() {
        let cfg = SweepConfig {
            max_evaluations: NonZeroU64::new(2_000).expect("nz"),
            seeds: vec![0, 1],
            max_wall_time: None,
        };
        let shapes = vec![rupert_shapes::cube()];
        let solvers = vec!["face_normal_pairs".to_string()];
        let results = run_sweep(&shapes, &solvers, &cfg).expect("sweep");
        // 1 shape × 1 solver × 2 seeds = 2 results.
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn sweep_rejects_unknown_solver() {
        let cfg = SweepConfig {
            max_evaluations: NonZeroU64::new(100).expect("nz"),
            seeds: vec![0],
            max_wall_time: None,
        };
        let shapes = vec![rupert_shapes::cube()];
        let solvers = vec!["does_not_exist".to_string()];
        let r = run_sweep(&shapes, &solvers, &cfg);
        assert!(r.is_err());
    }
}
