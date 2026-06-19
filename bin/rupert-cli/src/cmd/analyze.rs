//! `rupert analyze` — summarize experiment results and solver telemetry.

use std::io::Write as _;
use std::path::PathBuf;

use anyhow::Result;
use rupert_core::{CLEARANCE_EPS, PatchAwareCellSummary, RunOutcome, RunResult, SolverTelemetry};
use serde_json::json;

#[derive(clap::Args, Debug)]
pub(crate) struct AnalyzeArgs {
    #[command(subcommand)]
    command: AnalyzeCommand,
}

#[derive(clap::Subcommand, Debug)]
enum AnalyzeCommand {
    /// Summarize solved/open/disqualified run counts.
    Summary(SummaryArgs),
    /// Inspect patch-aware cell telemetry.
    PatchAware(PatchAwareArgs),
}

#[derive(clap::Args, Debug)]
struct SummaryArgs {
    /// Result directory containing JSONL files.
    #[arg(long, default_value = "results")]
    results_dir: PathBuf,
    /// Emit JSON instead of Markdown.
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args, Debug)]
struct PatchAwareArgs {
    /// Result directory containing JSONL files.
    #[arg(long, default_value = "results")]
    results_dir: PathBuf,
    /// Shape name to inspect.
    #[arg(long)]
    shape: String,
    /// Number of top cells to show.
    #[arg(long, default_value_t = 25)]
    top: usize,
    /// Emit JSON instead of Markdown.
    #[arg(long)]
    json: bool,
}

pub(crate) fn run(args: &AnalyzeArgs) -> Result<()> {
    match &args.command {
        AnalyzeCommand::Summary(summary) => run_summary(summary),
        AnalyzeCommand::PatchAware(patch_aware) => run_patch_aware(patch_aware),
    }
}

fn run_summary(args: &SummaryArgs) -> Result<()> {
    let results = rupert_bench::read_all_results(&args.results_dir)?;
    let summary = Summary::from_results(&results);
    let mut out = std::io::stdout().lock();
    if args.json {
        writeln!(
            out,
            "{}",
            json!({
                "total": summary.total,
                "solved": summary.solved,
                "exhausted": summary.exhausted,
                "disqualified": summary.disqualified,
                "error": summary.error,
                "with_best_near_miss": summary.with_best_near_miss,
                "with_best_boundary": summary.with_best_boundary,
                "with_best_positive": summary.with_best_positive,
            })
        )?;
    } else {
        writeln!(out, "| outcome | count |")?;
        writeln!(out, "|---------|------:|")?;
        writeln!(out, "| total | {} |", summary.total)?;
        writeln!(out, "| solved | {} |", summary.solved)?;
        writeln!(out, "| exhausted | {} |", summary.exhausted)?;
        writeln!(out, "| disqualified | {} |", summary.disqualified)?;
        writeln!(out, "| error | {} |", summary.error)?;
        writeln!(out)?;
        writeln!(out, "| observation class | runs |")?;
        writeln!(out, "|-------------------|-----:|")?;
        writeln!(out, "| best_positive | {} |", summary.with_best_positive)?;
        writeln!(out, "| best_near_miss | {} |", summary.with_best_near_miss)?;
        writeln!(out, "| best_boundary | {} |", summary.with_best_boundary)?;
    }
    Ok(())
}

fn run_patch_aware(args: &PatchAwareArgs) -> Result<()> {
    let results = rupert_bench::read_all_results(&args.results_dir)?;
    let runs = patch_aware_runs(&results, &args.shape);
    let mut rows = patch_aware_rows(&results, &args.shape);
    sort_patch_rows(&mut rows);
    rows.truncate(args.top);

    let mut out = std::io::stdout().lock();
    if args.json {
        write_patch_json(&mut out, &runs, &rows)?;
    } else {
        write_patch_markdown(&mut out, &runs, &rows)?;
    }
    Ok(())
}

fn sort_patch_rows(rows: &mut [PatchAwareRow]) {
    rows.sort_by(|a, b| {
        a.class_rank()
            .cmp(&b.class_rank())
            .then_with(|| {
                b.cell
                    .best_clearance
                    .partial_cmp(&a.cell.best_clearance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| b.cell.evals_spent.cmp(&a.cell.evals_spent))
            .then_with(|| a.solver.cmp(&b.solver))
            .then_with(|| a.seed.cmp(&b.seed))
            .then_with(|| a.cell.outer_cell.cmp(&b.cell.outer_cell))
            .then_with(|| a.cell.inner_cell.cmp(&b.cell.inner_cell))
    });
}

fn write_patch_json(
    out: &mut impl std::io::Write,
    runs: &[PatchAwareRun],
    rows: &[PatchAwareRow],
) -> Result<()> {
    let json_runs: Vec<_> = runs.iter().map(PatchAwareRun::to_json).collect();
    let json_rows: Vec<_> = rows.iter().map(PatchAwareRow::to_json).collect();
    writeln!(out, "{}", json!({ "runs": json_runs, "rows": json_rows }))?;
    Ok(())
}

fn write_patch_markdown(
    out: &mut impl std::io::Write,
    runs: &[PatchAwareRun],
    rows: &[PatchAwareRow],
) -> Result<()> {
    write_patch_run_table(out, runs)?;
    writeln!(out)?;
    write_patch_cell_table(out, rows)
}

fn write_patch_run_table(out: &mut impl std::io::Write, runs: &[PatchAwareRun]) -> Result<()> {
    writeln!(
        out,
        "| shape | solver | seed | canonical | pairs | recon | optimized | slack | bound | bound eval | bound ambig | bound <=+1e-3 | bound <=+1e-2 | adaptive cells | adaptive evals |"
    )?;
    writeln!(
        out,
        "|-------|--------|-----:|----------:|------:|------:|----------:|------:|------:|-----------:|------------:|--------------:|--------------:|---------------:|---------------:|"
    )?;
    for run in runs {
        writeln!(
            out,
            "| `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            run.shape,
            run.solver,
            run.seed,
            run.canonical_cells,
            run.cell_pairs,
            run.recon_cells_evaluated,
            run.optimized_cells,
            run.cells_skipped_by_slack,
            run.cells_skipped_by_interval_bound,
            run.bound_cells_evaluated,
            run.bound_cells_ambiguous,
            run.bound_le_plus_1e_minus_3(),
            run.bound_le_plus_1e_minus_2(),
            run.adaptive_refinement_cells,
            run.adaptive_refinement_evals
        )?;
    }
    Ok(())
}

fn write_patch_cell_table(out: &mut impl std::io::Write, rows: &[PatchAwareRow]) -> Result<()> {
    writeln!(
        out,
        "| shape | solver | seed | outer | inner | start | end | spent | recon | best | class | skip |"
    )?;
    writeln!(
        out,
        "|-------|--------|-----:|------:|------:|------:|----:|------:|------:|-----:|-------|------|"
    )?;
    for row in rows {
        writeln!(
            out,
            "| `{}` | `{}` | {} | {} | {} | {} | {} | {} | {:.6e} | {:.6e} | {} | {} |",
            row.shape,
            row.solver,
            row.seed,
            row.cell.outer_cell,
            row.cell.inner_cell,
            row.cell.start_eval,
            row.cell.end_eval,
            row.cell.evals_spent,
            row.cell.recon_clearance,
            row.cell.best_clearance,
            row.class(),
            row.skip_reason()
        )?;
    }
    Ok(())
}

#[derive(Debug, Default)]
struct Summary {
    total: usize,
    solved: usize,
    exhausted: usize,
    disqualified: usize,
    error: usize,
    with_best_positive: usize,
    with_best_near_miss: usize,
    with_best_boundary: usize,
}

impl Summary {
    fn from_results(results: &[RunResult]) -> Self {
        let mut summary = Self::default();
        for result in results {
            summary.total += 1;
            match result.outcome {
                RunOutcome::Solved => summary.solved += 1,
                RunOutcome::Exhausted => summary.exhausted += 1,
                RunOutcome::Disqualified { .. } => summary.disqualified += 1,
                RunOutcome::Error { .. } => summary.error += 1,
            }
            summary.with_best_positive += usize::from(result.best_positive.is_some());
            summary.with_best_near_miss += usize::from(result.best_near_miss.is_some());
            summary.with_best_boundary += usize::from(result.best_boundary.is_some());
        }
        summary
    }
}

#[derive(Debug)]
struct PatchAwareRow {
    shape: String,
    solver: String,
    seed: u64,
    cell: PatchAwareCellSummary,
}

#[derive(Debug)]
struct PatchAwareRun {
    shape: String,
    solver: String,
    seed: u64,
    canonical_cells: usize,
    cell_pairs: usize,
    recon_cells_evaluated: usize,
    optimized_cells: usize,
    cells_skipped_by_slack: usize,
    cells_skipped_by_interval_bound: usize,
    bound_cells_evaluated: usize,
    bound_cells_ambiguous: usize,
    bound_histogram: rupert_core::PatchAwareBoundHistogram,
    adaptive_refinement_cells: usize,
    adaptive_refinement_evals: u64,
}

impl PatchAwareRow {
    fn to_json(&self) -> serde_json::Value {
        json!({
            "shape": self.shape,
            "solver": self.solver,
            "seed": self.seed,
            "outer_cell": self.cell.outer_cell,
            "inner_cell": self.cell.inner_cell,
            "start_eval": self.cell.start_eval,
            "end_eval": self.cell.end_eval,
            "evals_spent": self.cell.evals_spent,
            "recon_clearance": self.cell.recon_clearance,
            "best_clearance": self.cell.best_clearance,
            "class": self.class(),
            "skip_reason": self.skip_reason(),
        })
    }

    fn skip_reason(&self) -> &'static str {
        match self.cell.skip_reason {
            rupert_core::PatchAwareSkipReason::None => "none",
            rupert_core::PatchAwareSkipReason::Slack => "slack",
            rupert_core::PatchAwareSkipReason::Bound => "bound",
        }
    }

    fn class(&self) -> &'static str {
        if self.cell.best_clearance > CLEARANCE_EPS {
            "positive"
        } else if self.cell.best_clearance < -CLEARANCE_EPS {
            "near_miss"
        } else {
            "boundary"
        }
    }

    fn class_rank(&self) -> u8 {
        match self.class() {
            "positive" => 0,
            "near_miss" => 1,
            _ => 2,
        }
    }
}

impl PatchAwareRun {
    fn to_json(&self) -> serde_json::Value {
        json!({
            "shape": self.shape,
            "solver": self.solver,
            "seed": self.seed,
            "canonical_cells": self.canonical_cells,
            "cell_pairs": self.cell_pairs,
            "recon_cells_evaluated": self.recon_cells_evaluated,
            "optimized_cells": self.optimized_cells,
            "cells_skipped_by_slack": self.cells_skipped_by_slack,
            "cells_skipped_by_interval_bound": self.cells_skipped_by_interval_bound,
            "bound_cells_evaluated": self.bound_cells_evaluated,
            "bound_cells_ambiguous": self.bound_cells_ambiguous,
            "bound_histogram": {
                "le_current_best": self.bound_histogram.le_current_best,
                "current_best_to_plus_1e_minus_3": self.bound_histogram.current_best_to_plus_1e_minus_3,
                "plus_1e_minus_3_to_plus_1e_minus_2": self.bound_histogram.plus_1e_minus_3_to_plus_1e_minus_2,
                "plus_1e_minus_2_to_plus_1e_minus_1": self.bound_histogram.plus_1e_minus_2_to_plus_1e_minus_1,
                "plus_1e_minus_1_to_plus_1": self.bound_histogram.plus_1e_minus_1_to_plus_1,
                "plus_1_or_more": self.bound_histogram.plus_1_or_more,
            },
            "adaptive_refinement_cells": self.adaptive_refinement_cells,
            "adaptive_refinement_evals": self.adaptive_refinement_evals,
        })
    }

    fn bound_le_plus_1e_minus_3(&self) -> usize {
        self.bound_histogram.le_current_best + self.bound_histogram.current_best_to_plus_1e_minus_3
    }

    fn bound_le_plus_1e_minus_2(&self) -> usize {
        self.bound_le_plus_1e_minus_3() + self.bound_histogram.plus_1e_minus_3_to_plus_1e_minus_2
    }
}

fn patch_aware_rows(results: &[RunResult], shape: &str) -> Vec<PatchAwareRow> {
    let mut rows = Vec::new();
    for result in results {
        if result.poly_name != shape {
            continue;
        }
        let Some(SolverTelemetry::PatchAware(telemetry)) = result.telemetry.as_ref() else {
            continue;
        };
        let cells = if telemetry.cell_summaries.is_empty() {
            &telemetry.top_cells
        } else {
            &telemetry.cell_summaries
        };
        for cell in cells {
            rows.push(PatchAwareRow {
                shape: result.poly_name.clone(),
                solver: result.solver_name.clone(),
                seed: result.seed,
                cell: cell.clone(),
            });
        }
    }
    rows
}

fn patch_aware_runs(results: &[RunResult], shape: &str) -> Vec<PatchAwareRun> {
    let mut runs = Vec::new();
    for result in results {
        if result.poly_name != shape {
            continue;
        }
        let Some(SolverTelemetry::PatchAware(telemetry)) = result.telemetry.as_ref() else {
            continue;
        };
        runs.push(PatchAwareRun {
            shape: result.poly_name.clone(),
            solver: result.solver_name.clone(),
            seed: result.seed,
            canonical_cells: telemetry.canonical_cells,
            cell_pairs: telemetry.cell_pairs,
            recon_cells_evaluated: telemetry.recon_cells_evaluated,
            optimized_cells: telemetry.optimized_cells,
            cells_skipped_by_slack: telemetry.cells_skipped_by_slack,
            cells_skipped_by_interval_bound: telemetry.cells_skipped_by_interval_bound,
            bound_cells_evaluated: telemetry.bound_cells_evaluated,
            bound_cells_ambiguous: telemetry.bound_cells_ambiguous,
            bound_histogram: telemetry.bound_histogram.clone(),
            adaptive_refinement_cells: telemetry.adaptive_refinement_cells,
            adaptive_refinement_evals: telemetry.adaptive_refinement_evals,
        });
    }
    runs
}
