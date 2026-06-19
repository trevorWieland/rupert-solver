//! Fallback search for user shapes without face data.

use rand::Rng;
use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use rupert_core::{Budget, Candidate, EvalCounter, Solution, SolverOutcome};

use crate::dfo::{apply_quat_delta, nelder_mead_box};
use crate::sample::random_unit_quat;

const Q_BOX: f64 = 0.15;
const T_BOX: f64 = 0.5;
const PENALTY_FLOOR: f64 = -10_000.0;
const MIN_OPT_EVALS: u64 = 16;
const FALLBACK_CELL_EVALS: u64 = 1500;

const LB: [f64; 10] = [
    -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -T_BOX, -T_BOX,
];
const UB: [f64; 10] = [
    Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, T_BOX, T_BOX,
];

/// Degenerate to random-restart Nelder-Mead when a custom polyhedron has
/// no face data, so patch decomposition cannot be built.
pub(super) fn single_cell_fallback(
    budget: &Budget,
    ec: &mut EvalCounter<'_>,
    max: u64,
) -> SolverOutcome {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(budget.seed);
    let mut best_clearance = f64::NEG_INFINITY;
    let mut best_candidate = Candidate::IDENTITY;
    while ec.count() < max {
        let remaining = max.saturating_sub(ec.count());
        let inner_budget = FALLBACK_CELL_EVALS.min(remaining) as usize;
        if inner_budget < MIN_OPT_EVALS as usize {
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
        SolverOutcome::found(Solution {
            candidate: best_candidate,
            clearance: best_clearance,
            found_at_eval: ec.count(),
            certification: None,
        })
    } else {
        SolverOutcome::exhausted()
    }
}
