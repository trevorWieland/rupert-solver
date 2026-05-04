//! `rupert run` — single-cell run or full sweep.

use std::io::Write as _;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rupert_bench::{SweepConfig, run_full_sweep, run_sweep, write_results};

#[derive(clap::Args, Debug)]
pub(crate) struct RunArgs {
    /// Run all builtin shapes × all registered solvers (overrides --shape, --solver).
    #[arg(long)]
    all: bool,
    /// Builtin shape name (see `rupert list shapes`).
    #[arg(long)]
    shape: Option<String>,
    /// Solver name (see `rupert list solvers`).
    #[arg(long)]
    solver: Option<String>,
    /// One or more seeds to run. Repeat `--seed` for multiple values.
    #[arg(long, default_values_t = vec![0u64])]
    seed: Vec<u64>,
    /// Maximum number of clearance evaluations per seed.
    #[arg(long, default_value_t = 50_000)]
    budget_evals: u64,
    /// Optional wall-clock budget per seed, in milliseconds.
    #[arg(long)]
    budget_wall_ms: Option<u64>,
    /// Output directory; defaults to `./results`.
    #[arg(long, default_value = "results")]
    out_dir: PathBuf,
}

pub(crate) fn run(args: &RunArgs) -> Result<()> {
    let max_evaluations = NonZeroU64::new(args.budget_evals).context("budget-evals must be > 0")?;
    let max_wall_time = args.budget_wall_ms.map(Duration::from_millis);
    let cfg = SweepConfig {
        max_evaluations,
        seeds: args.seed.clone(),
        max_wall_time,
    };
    let results = if args.all {
        run_full_sweep(&cfg)?
    } else {
        let shape_name = args
            .shape
            .as_deref()
            .ok_or_else(|| anyhow!("--shape required unless --all is set"))?;
        let solver_name = args
            .solver
            .as_deref()
            .ok_or_else(|| anyhow!("--solver required unless --all is set"))?;
        let poly = rupert_shapes::lookup(shape_name)
            .ok_or_else(|| anyhow!("unknown shape '{shape_name}'; see `rupert list shapes`"))?;
        let solvers = vec![solver_name.to_string()];
        run_sweep(&[poly], &solvers, &cfg)?
    };
    let path = write_results(&args.out_dir, &results)?;
    let mut out = std::io::stdout().lock();
    writeln!(out, "wrote {} results -> {}", results.len(), path.display())?;
    let solved = results
        .iter()
        .filter(|r| matches!(r.outcome, rupert_core::RunOutcome::Solved))
        .count();
    let exhausted = results
        .iter()
        .filter(|r| matches!(r.outcome, rupert_core::RunOutcome::Exhausted))
        .count();
    writeln!(out, "solved={solved} exhausted={exhausted}")?;
    Ok(())
}
