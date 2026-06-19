//! Within-patch optimization (Nelder-Mead) and cross-cell scan.

use rand::Rng;
use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use rupert_core::{
    Budget, Candidate, ClearanceHistogram, EvalCounter, PatchAwareBoundHistogram,
    PatchAwareCellSummary, PatchAwareSkipReason, PatchAwareTelemetry, Vec3,
};

use super::bounds::{BoundOutcome, bound_cell};
use super::cell_record::CellRecorder;
use super::table::{CanonicalPatch, PatchTable, face_sign_vector};
use crate::dfo::{apply_quat_delta, nelder_mead_box};

const Q_BOX: f64 = 0.15;
const T_BOX: f64 = 0.5;
const LAMBDA_PENALTY: f64 = 0.1;
const PENALTY_FLOOR: f64 = -10_000.0;
/// In Phase C, skip cells whose recon clearance is more than this far
/// below the running best. Inner Nelder-Mead can't realistically catch
/// up across this gap inside one patch's local neighborhood.
const SKIP_SLACK: f64 = 0.5;
const MIN_OPT_EVALS: u64 = 16;
const MAX_INITIAL_CELL_EVALS: u64 = 64;
const INITIAL_PHASE_NUMERATOR: u64 = 2;
const INITIAL_PHASE_DENOMINATOR: u64 = 5;
const REFINE_CELL_EVALS: u64 = 512;
const ADAPTIVE_CELL_LIMIT: usize = 64;

const LB: [f64; 10] = [
    -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -T_BOX, -T_BOX,
];
const UB: [f64; 10] = [
    Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, T_BOX, T_BOX,
];

type ReconCell = (usize, usize, f64);

/// v0.3.0 strategy: **reconnaissance-first adaptive scan**.
///
/// Phase A: 1 eval per cell at the canonical anchor (zero-delta).
/// Cheap, gives a baseline clearance estimate per (outer, inner) cell.
///
/// Phase B: sort cells by descending recon score. The most promising
/// cells get optimized first; the least promising might never be
/// reached if the budget runs out — but the budget runs out worst on
/// hopeless cells, which is exactly where we wanted to spend less.
///
/// Phase C: shallow per-cell Nelder-Mead in score order. After every
/// cell, update `best_so_far`. **Skip** cells whose recon score is worse
/// than a positive `best_so_far` by more than `SKIP_SLACK`, since the
/// inner Nelder-Mead is unlikely to recover that gap inside the patch's
/// local neighborhood.
///
/// Phase D: spend remaining budget on the strongest positive/near-miss
/// cells, round-robin, preserving deterministic seed/order behavior.
pub(super) struct ScanResult {
    pub best_clearance: f64,
    pub best_candidate: Candidate,
    pub telemetry: PatchAwareTelemetry,
}

pub(super) fn scan_cells(
    table: &PatchTable,
    ec: &mut EvalCounter<'_>,
    budget: &Budget,
    max: u64,
) -> ScanResult {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(budget.seed);
    let cell_pairs = table.canonical.len() * table.canonical.len();
    let mut histogram = ClearanceHistogram::default();
    let mut recorder = CellRecorder::new(cell_pairs);
    let (mut recon, recon_cells_evaluated) =
        run_recon(table, ec, max, &mut histogram, &mut recorder);
    sort_recon(&mut recon);

    let mut best_candidate = Candidate::IDENTITY;
    let mut best_clearance = f64::NEG_INFINITY;
    let initial_cell_evals = initial_cell_budget(max.saturating_sub(ec.count()), recon.len());
    let initial = run_initial_optimization(InitialContext {
        table,
        ec,
        rng: &mut rng,
        max,
        recon: &recon,
        initial_cell_evals,
        histogram: &mut histogram,
        recorder: &mut recorder,
        best_clearance: &mut best_clearance,
        best_candidate: &mut best_candidate,
    });

    let adaptive = {
        let mut refine_context = RefineContext {
            table,
            ec,
            rng: &mut rng,
            max,
            histogram: &mut histogram,
            recorder: &mut recorder,
        };
        refine_ranked_cells(
            &mut refine_context,
            &mut best_clearance,
            &mut best_candidate,
        )
    };
    recorder.finalize();

    ScanResult {
        best_clearance,
        best_candidate,
        telemetry: PatchAwareTelemetry {
            canonical_cells: table.canonical.len(),
            cell_pairs,
            recon_cells_evaluated,
            optimized_cells: initial.optimized_cells,
            cells_skipped_by_slack: initial.cells_skipped_by_slack,
            cells_skipped_by_interval_bound: initial.cells_skipped_by_interval_bound,
            bound_cells_evaluated: initial.bound_cells_evaluated,
            bound_cells_ambiguous: initial.bound_cells_ambiguous,
            bound_histogram: initial.bound_histogram,
            adaptive_refinement_cells: adaptive.cells,
            adaptive_refinement_evals: adaptive.evals,
            best_positive_cell: recorder.best_positive_cell,
            best_near_miss_cell: recorder.best_near_miss_cell,
            best_boundary_cell: recorder.best_boundary_cell,
            cell_summaries: recorder.cell_summaries,
            top_cells: recorder.top_cells,
            clearance_histogram: histogram,
        },
    }
}

fn run_recon(
    table: &PatchTable,
    ec: &mut EvalCounter<'_>,
    max: u64,
    histogram: &mut ClearanceHistogram,
    recorder: &mut CellRecorder,
) -> (Vec<ReconCell>, usize) {
    let mut recon: Vec<ReconCell> = Vec::new();
    let mut recon_cells_evaluated = 0_usize;
    for (oi, outer_canon) in table.canonical.iter().enumerate() {
        if ec.count() >= max {
            break;
        }
        for (ii, inner_canon) in table.canonical.iter().enumerate() {
            if ec.count() >= max {
                break;
            }
            let candidate = Candidate {
                outer: outer_canon.q_rep,
                inner: inner_canon.q_rep,
                translation: [0.0, 0.0],
            };
            let start_eval = ec.count();
            let c = ec.evaluate(&candidate);
            let end_eval = ec.count();
            recon_cells_evaluated += 1;
            histogram.record(c);
            recon.push((oi, ii, c));
            recorder.record(optimized_cell(oi, ii, c, c, start_eval, end_eval));
        }
    }
    (recon, recon_cells_evaluated)
}

fn sort_recon(recon: &mut [ReconCell]) {
    recon.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
            .then_with(|| a.1.cmp(&b.1))
    });
}

#[derive(Debug, Default)]
struct InitialStats {
    optimized_cells: usize,
    cells_skipped_by_slack: usize,
    cells_skipped_by_interval_bound: usize,
    bound_cells_evaluated: usize,
    bound_cells_ambiguous: usize,
    bound_histogram: PatchAwareBoundHistogram,
}

struct InitialContext<'a, 'b> {
    table: &'a PatchTable,
    ec: &'a mut EvalCounter<'b>,
    rng: &'a mut Xoshiro256PlusPlus,
    max: u64,
    recon: &'a [ReconCell],
    initial_cell_evals: u64,
    histogram: &'a mut ClearanceHistogram,
    recorder: &'a mut CellRecorder,
    best_clearance: &'a mut f64,
    best_candidate: &'a mut Candidate,
}

fn run_initial_optimization(mut ctx: InitialContext<'_, '_>) -> InitialStats {
    let mut stats = InitialStats::default();
    for &(oi, ii, recon_score) in ctx.recon {
        if ctx.ec.count() >= ctx.max {
            break;
        }
        if should_skip_by_slack(*ctx.best_clearance, recon_score) {
            stats.cells_skipped_by_slack += 1;
            ctx.recorder.record(skipped_cell(
                oi,
                ii,
                recon_score,
                ctx.ec.count(),
                PatchAwareSkipReason::Slack,
            ));
            continue;
        }
        if should_skip_by_bound(&mut stats, &mut ctx, oi, ii, recon_score) {
            continue;
        }
        if !optimize_initial_cell(&mut stats, &mut ctx, oi, ii, recon_score) {
            break;
        }
    }
    stats
}

fn should_skip_by_slack(best_clearance: f64, recon_score: f64) -> bool {
    best_clearance > 0.0 && recon_score < best_clearance - SKIP_SLACK
}

fn should_skip_by_bound(
    stats: &mut InitialStats,
    ctx: &mut InitialContext<'_, '_>,
    oi: usize,
    ii: usize,
    recon_score: f64,
) -> bool {
    if !ctx.best_clearance.is_finite() {
        return false;
    }
    stats.bound_cells_evaluated += 1;
    match bound_cell(
        ctx.table,
        &ctx.table.canonical[oi],
        &ctx.table.canonical[ii],
        recon_score,
        *ctx.best_clearance,
        Q_BOX,
        T_BOX,
    ) {
        BoundOutcome::Prunable { upper_bound } => {
            stats
                .bound_histogram
                .record(upper_bound, *ctx.best_clearance);
            stats.cells_skipped_by_interval_bound += 1;
            ctx.recorder.record(skipped_cell(
                oi,
                ii,
                recon_score,
                ctx.ec.count(),
                PatchAwareSkipReason::Bound,
            ));
            true
        }
        BoundOutcome::Ambiguous { upper_bound } => {
            stats
                .bound_histogram
                .record(upper_bound, *ctx.best_clearance);
            stats.bound_cells_ambiguous += 1;
            false
        }
    }
}

fn optimize_initial_cell(
    stats: &mut InitialStats,
    ctx: &mut InitialContext<'_, '_>,
    oi: usize,
    ii: usize,
    recon_score: f64,
) -> bool {
    let remaining = ctx.max.saturating_sub(ctx.ec.count());
    let inner_budget = ctx.initial_cell_evals.min(remaining);
    if inner_budget < MIN_OPT_EVALS {
        return false;
    }
    let start_eval = ctx.ec.count();
    let result = optimize_pair(
        ctx.ec,
        &ctx.table.canonical[oi],
        &ctx.table.canonical[ii],
        &ctx.table.normals,
        inner_budget as usize,
        ctx.rng,
    );
    let Some((c, cand)) = result else {
        return true;
    };
    stats.optimized_cells += 1;
    ctx.histogram.record(c);
    let end_eval = ctx.ec.count();
    ctx.recorder
        .record(optimized_cell(oi, ii, recon_score, c, start_eval, end_eval));
    if c > *ctx.best_clearance {
        *ctx.best_clearance = c;
        *ctx.best_candidate = cand;
    }
    true
}

fn skipped_cell(
    outer_cell: usize,
    inner_cell: usize,
    recon_clearance: f64,
    eval: u64,
    skip_reason: PatchAwareSkipReason,
) -> PatchAwareCellSummary {
    PatchAwareCellSummary {
        outer_cell,
        inner_cell,
        start_eval: eval,
        end_eval: eval,
        evals_spent: 0,
        recon_clearance,
        best_clearance: recon_clearance,
        skip_reason,
    }
}

fn optimized_cell(
    outer_cell: usize,
    inner_cell: usize,
    recon_clearance: f64,
    best_clearance: f64,
    start_eval: u64,
    end_eval: u64,
) -> PatchAwareCellSummary {
    PatchAwareCellSummary {
        outer_cell,
        inner_cell,
        start_eval,
        end_eval,
        evals_spent: end_eval.saturating_sub(start_eval),
        recon_clearance,
        best_clearance,
        skip_reason: PatchAwareSkipReason::None,
    }
}

fn initial_cell_budget(remaining_after_recon: u64, cells: usize) -> u64 {
    if cells == 0 {
        return MIN_OPT_EVALS;
    }
    let initial_pool =
        remaining_after_recon.saturating_mul(INITIAL_PHASE_NUMERATOR) / INITIAL_PHASE_DENOMINATOR;
    let per_cell = initial_pool / cells as u64;
    per_cell.clamp(MIN_OPT_EVALS, MAX_INITIAL_CELL_EVALS)
}

#[derive(Debug, Clone, Copy, Default)]
struct AdaptiveStats {
    cells: usize,
    evals: u64,
}

struct RefineContext<'a, 'b> {
    table: &'a PatchTable,
    ec: &'a mut EvalCounter<'b>,
    rng: &'a mut Xoshiro256PlusPlus,
    max: u64,
    histogram: &'a mut ClearanceHistogram,
    recorder: &'a mut CellRecorder,
}

fn refine_ranked_cells(
    ctx: &mut RefineContext<'_, '_>,
    best_clearance: &mut f64,
    best_candidate: &mut Candidate,
) -> AdaptiveStats {
    let ranked = ctx.recorder.ranked_refinement_cells(ADAPTIVE_CELL_LIMIT);
    if ranked.is_empty() {
        return AdaptiveStats::default();
    }
    let mut stats = AdaptiveStats::default();
    let mut cursor = 0_usize;
    while ctx.max.saturating_sub(ctx.ec.count()) >= MIN_OPT_EVALS {
        let cell = &ranked[cursor % ranked.len()];
        let remaining = ctx.max.saturating_sub(ctx.ec.count());
        let inner_budget = REFINE_CELL_EVALS.min(remaining);
        let start_eval = ctx.ec.count();
        if let Some((c, cand)) = optimize_pair(
            ctx.ec,
            &ctx.table.canonical[cell.outer_cell],
            &ctx.table.canonical[cell.inner_cell],
            &ctx.table.normals,
            inner_budget as usize,
            ctx.rng,
        ) {
            stats.cells += 1;
            ctx.histogram.record(c);
            let end_eval = ctx.ec.count();
            stats.evals += end_eval.saturating_sub(start_eval);
            ctx.recorder.record(optimized_cell(
                cell.outer_cell,
                cell.inner_cell,
                cell.recon_clearance,
                c,
                start_eval,
                end_eval,
            ));
            if c > *best_clearance {
                *best_clearance = c;
                *best_candidate = cand;
            }
        }
        cursor += 1;
    }
    stats
}

/// Nelder-Mead within one (outer, inner) cell.
fn optimize_pair(
    ec: &mut EvalCounter<'_>,
    outer_canon: &CanonicalPatch,
    inner_canon: &CanonicalPatch,
    normals: &[Vec3],
    budget: usize,
    rng: &mut Xoshiro256PlusPlus,
) -> Option<(f64, Candidate)> {
    let seed = Candidate {
        outer: outer_canon.q_rep,
        inner: inner_canon.q_rep,
        translation: [0.0, 0.0],
    };
    let outer_canon_sv = outer_canon.sign_vec;
    let inner_canon_sv = inner_canon.sign_vec;
    let mut loss = |delta: &[f64]| -> f64 {
        let cand = apply_quat_delta(&seed, delta);
        let trial_sv_outer = face_sign_vector(normals, &cand.outer);
        let trial_sv_inner = face_sign_vector(normals, &cand.inner);
        let hamming = (trial_sv_outer ^ outer_canon_sv).count_ones()
            + (trial_sv_inner ^ inner_canon_sv).count_ones();
        let c = ec.evaluate(&cand);
        if !c.is_finite() {
            return -PENALTY_FLOOR;
        }
        -c + LAMBDA_PENALTY * f64::from(hamming)
    };
    let start: [f64; 10] = std::array::from_fn(|d| rng.gen_range(LB[d]..UB[d]));
    let (delta, _) = nelder_mead_box(&mut loss, &start, &LB, &UB, budget);
    let final_cand = apply_quat_delta(&seed, &delta);
    // Re-evaluate clearance directly so the returned value reflects
    // pure clearance (no penalty term). One extra eval call.
    let final_clearance = ec.evaluate(&final_cand);
    if !final_clearance.is_finite() {
        return None;
    }
    Some((final_clearance, final_cand))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_cell_budget_reserves_tail_budget() {
        assert_eq!(initial_cell_budget(1_958_000, 41_209), 19);
        assert_eq!(initial_cell_budget(8_791, 41_209), MIN_OPT_EVALS);
        assert_eq!(initial_cell_budget(1_000_000, 10), MAX_INITIAL_CELL_EVALS);
    }
}
