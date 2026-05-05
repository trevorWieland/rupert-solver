//! Within-patch optimization (Nelder-Mead) and cross-cell scan.

use rand::Rng;
use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use rupert_core::{Budget, Candidate, EvalCounter, SolverOutcome, Vec3};

use crate::dfo::{apply_quat_delta, nelder_mead_box};
use crate::sample::random_unit_quat;

use super::table::{CanonicalPatch, PatchTable, face_sign_vector};

const Q_BOX: f64 = 0.15;
const T_BOX: f64 = 0.5;
const LAMBDA_PENALTY: f64 = 0.1;
const PENALTY_FLOOR: f64 = -10_000.0;
/// In Phase C, skip cells whose recon clearance is more than this far
/// below the running best. Inner Nelder-Mead can't realistically catch
/// up across this gap inside one patch's local neighborhood.
const SKIP_SLACK: f64 = 0.5;

const LB: [f64; 10] = [
    -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -T_BOX, -T_BOX,
];
const UB: [f64; 10] = [
    Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, T_BOX, T_BOX,
];

/// v0.2.0 strategy: **reconnaissance-first scan**.
///
/// Phase A: 1 eval per cell at the canonical anchor (zero-delta).
/// Cheap, gives a baseline clearance estimate per (outer, inner) cell.
///
/// Phase B: sort cells by descending recon score. The most promising
/// cells get optimized first; the least promising might never be
/// reached if the budget runs out — but the budget runs out worst on
/// hopeless cells, which is exactly where we wanted to spend less.
///
/// Phase C: per-cell Nelder-Mead with uniform `per_pair_evals` budget,
/// in score order. After every cell, update `best_so_far`. **Skip**
/// cells whose recon score is worse than the current `best_so_far` by
/// more than `SKIP_SLACK`, since the inner Nelder-Mead is unlikely to
/// recover that gap inside the patch's local neighborhood.
///
/// This is the v0.1.0 → v0.2.0 step that makes patch_aware
/// budget-efficient on shapes with many cells (snub cube: 1296 cells).
pub(super) fn scan_cells(
    table: &PatchTable,
    ec: &mut EvalCounter<'_>,
    budget: &Budget,
    max: u64,
    per_pair_evals: u64,
) -> Option<(f64, Candidate)> {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(budget.seed);

    // Phase A: reconnaissance.
    let mut recon: Vec<(usize, usize, f64)> = Vec::new();
    for (oi, outer_canon) in table.canonical.iter().enumerate() {
        if ec.count() >= max {
            return None;
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
            let c = ec.evaluate(&candidate);
            recon.push((oi, ii, c));
        }
    }

    // Phase B: sort cells by recon score descending. Treat non-finite
    // values as worst (they'd be skipped anyway).
    recon.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Phase C: optimize in priority order.
    let mut best_candidate = Candidate::IDENTITY;
    let mut best_clearance = f64::NEG_INFINITY;

    for (oi, ii, recon_score) in recon {
        if ec.count() >= max {
            break;
        }
        // Skip hopeless cells: if recon is far worse than current best,
        // the per-cell Nelder-Mead can't realistically catch up inside
        // a 0.15-wide quat-delta box around this anchor.
        if best_clearance > 0.0 && recon_score < best_clearance - SKIP_SLACK {
            continue;
        }
        let remaining = max.saturating_sub(ec.count());
        let inner_budget = per_pair_evals.min(remaining) as usize;
        if inner_budget < 16 {
            break;
        }
        if let Some((c, cand)) = optimize_pair(
            ec,
            &table.canonical[oi],
            &table.canonical[ii],
            &table.normals,
            inner_budget,
            &mut rng,
        ) && c > best_clearance
        {
            best_clearance = c;
            best_candidate = cand;
        }
    }

    if best_clearance.is_finite() {
        Some((best_clearance, best_candidate))
    } else {
        None
    }
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

/// Fallback when the polyhedron has no face data — degenerate to
/// random-restart Nelder-Mead (no patch awareness).
pub(super) fn single_cell_fallback(
    budget: &Budget,
    ec: &mut EvalCounter<'_>,
    max: u64,
) -> SolverOutcome {
    use rupert_core::Solution;

    let mut rng = Xoshiro256PlusPlus::seed_from_u64(budget.seed);
    let mut best_clearance = f64::NEG_INFINITY;
    let mut best_candidate = Candidate::IDENTITY;
    while ec.count() < max {
        let remaining = max.saturating_sub(ec.count());
        let inner_budget = (1500_u64).min(remaining) as usize;
        if inner_budget < 16 {
            break;
        }
        let seed = Candidate {
            outer: random_unit_quat(&mut rng),
            inner: random_unit_quat(&mut rng),
            translation: [0.0, 0.0],
        };
        let mut loss = |delta: &[f64]| -> f64 {
            let cand = apply_quat_delta(&seed, delta);
            let c = ec.evaluate(&cand);
            if !c.is_finite() {
                return -PENALTY_FLOOR;
            }
            -c
        };
        let start: [f64; 10] = std::array::from_fn(|d| rng.gen_range(LB[d]..UB[d]));
        let (delta, neg) = nelder_mead_box(&mut loss, &start, &LB, &UB, inner_budget);
        let clearance = -neg;
        if clearance.is_finite() && clearance > best_clearance {
            best_clearance = clearance;
            best_candidate = apply_quat_delta(&seed, &delta);
        }
    }
    if best_clearance.is_finite() && best_clearance > 0.0 {
        SolverOutcome::Found(Solution {
            candidate: best_candidate,
            clearance: best_clearance,
            found_at_eval: ec.count(),
            certification: None,
        })
    } else {
        SolverOutcome::Exhausted
    }
}
