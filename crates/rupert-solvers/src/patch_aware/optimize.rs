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

const LB: [f64; 10] = [
    -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -T_BOX, -T_BOX,
];
const UB: [f64; 10] = [
    Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, T_BOX, T_BOX,
];

/// Scan every (outer, inner) canonical-patch pair; return the best
/// (clearance, candidate) found.
pub(super) fn scan_cells(
    table: &PatchTable,
    ec: &mut EvalCounter<'_>,
    budget: &Budget,
    max: u64,
    per_pair_evals: u64,
) -> Option<(f64, Candidate)> {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(budget.seed);
    let mut best_candidate = Candidate::IDENTITY;
    let mut best_clearance = f64::NEG_INFINITY;

    for outer_canon in &table.canonical {
        if ec.count() >= max {
            break;
        }
        for inner_canon in &table.canonical {
            if ec.count() >= max {
                break;
            }
            let remaining = max.saturating_sub(ec.count());
            let inner_budget = per_pair_evals.min(remaining) as usize;
            if inner_budget < 16 {
                break;
            }
            if let Some((c, cand)) = optimize_pair(
                ec,
                outer_canon,
                inner_canon,
                &table.normals,
                inner_budget,
                &mut rng,
            ) && c > best_clearance
            {
                best_clearance = c;
                best_candidate = cand;
            }
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
