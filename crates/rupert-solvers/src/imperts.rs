//! Ported from Tom 7 (suckerpinch), `imperts.cc` from his SIGBOVIK 2025
//! Rupert project: <http://tom7.org/ruperts/>.
//!
//! Original source: <https://sourceforge.net/p/tom7misc/svn/HEAD/tree/trunk/ruperts/imperts.cc>
//! License: not declared upstream — algorithm reimplemented per the
//! description in the SIGBOVIK 2025 paper and the source's structure.
//!
//! ## What `imperts` is
//!
//! Refinement solver — given an existing good seed,
//! search a box of `(Δq_outer, Δq_inner, Δt)` deltas around the seed
//! and accept strictly-improving steps. The upstream binary pulls seeds
//! from a SQLite database produced by `ruperts.cc` (the discovery
//! engine). Our [`Solver`] trait doesn't take a seed, so we
//! self-bootstrap: random-quaternion search keeps the best near-miss
//! candidate, then the refinement loop tries to cross zero.
//!
//! ## Faithful-port story
//!
//! v1.5 (this revision) restores Tom's static box bounds `Q=1.0`,
//! `T=0.5` and replaces our v1's "uniform random box search + adaptive
//! shrink" inner DFO with [`crate::dfo::opt_minimize`] — a
//! Nelder-Mead-with-multi-restart that's the closest in-tree analogue
//! to upstream's `cc-lib::Opt::Minimize<10>(f, lb, ub, iters=2000,
//! depth=2, attempts=100)`.
//!
//! Remaining v1.5 ↔ upstream diffs:
//!
//! - **No SQLite seed selection.** Tom's binary picks seeds from a
//!   global solution database (preferring under-improved shapes); we
//!   generate one fresh per `solve()` call via random unit-quaternion
//!   bootstrap.
//! - **No threading.** Upstream uses 8-thread `ParallelFan`; we honor
//!   the harness's per-task single-threaded contract (parallelism lives
//!   in `rupert-bench::sweep`).
//! - **Inner DFO is Nelder-Mead-with-multi-restart, not cc-lib's
//!   exact algorithm.** Same shape, different internals. Upstream's
//!   `Opt::Minimize` uses a recursive box-subdivision DFO; ours is
//!   simpler. v2 work item: port cc-lib if a faithful inner DFO
//!   matters.

use rand::Rng;
use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use rupert_core::{Budget, Candidate, EvalCounter, Polyhedron, Solution, Solver, SolverOutcome};

use crate::dfo::{apply_quat_delta, nelder_mead_box};
use crate::sample::random_unit_quat;

#[derive(Debug, Default)]
pub struct Imperts;

const MAX_OUTER_ITERS: usize = 3000;
const MIN_FLAT_ITERS: usize = 100;
/// Box half-width on quaternion-component deltas. Verbatim from upstream.
const Q_BOX: f64 = 1.0;
/// Box half-width on translation deltas. Verbatim from upstream.
const T_BOX: f64 = 0.5;
/// Evaluation budget for one Nelder-Mead restart per outer iteration.
/// Each outer iter does a single random-start NM; the outer loop itself
/// provides the multi-restart axis (Tom's `attempts=100` lives in
/// `MAX_OUTER_ITERS`).
const INNER_BUDGET: usize = 1_500;
const MAX_SEED_ATTEMPTS: u64 = 50_000;
/// Penalty added to the loss when a candidate has non-finite clearance —
/// keeps the inner DFO's objective continuous.
const PENALTY_FLOOR: f64 = -10_000.0;

impl Solver for Imperts {
    fn name(&self) -> &'static str {
        "imperts"
    }

    fn version(&self) -> &'static str {
        "0.3.0"
    }

    fn solve(
        &mut self,
        _poly: &Polyhedron,
        budget: &Budget,
        ec: &mut EvalCounter<'_>,
    ) -> SolverOutcome {
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(budget.seed);
        let max = budget.max_evaluations.get();

        // Phase 1 — bootstrap a seed via random unit-quaternion search.
        let Some((mut best_candidate, mut best_clearance)) = bootstrap_seed(ec, &mut rng, max)
        else {
            return SolverOutcome::exhausted();
        };

        // Phase 2 — Tom 7's outer refinement loop. Each outer iter runs
        // a multi-restart Nelder-Mead inside a box of deltas anchored at
        // the current best.
        let mut last_improved: usize = 0;
        let lb: [f64; 10] = [
            -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -Q_BOX, -T_BOX, -T_BOX,
        ];
        let ub: [f64; 10] = [
            Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, Q_BOX, T_BOX, T_BOX,
        ];
        for outer in 0..MAX_OUTER_ITERS {
            if ec.count() >= max {
                break;
            }
            let remaining = max.saturating_sub(ec.count());
            let inner_budget = (INNER_BUDGET as u64).min(remaining) as usize;
            if inner_budget < 16 {
                break;
            }
            let seed_for_loss = best_candidate;
            let mut loss = |delta: &[f64]| -> f64 {
                let cand = apply_quat_delta(&seed_for_loss, delta);
                let c = ec.evaluate(&cand);
                if !c.is_finite() {
                    return -PENALTY_FLOOR;
                }
                -c
            };
            // Random anchor in the box; the outer loop provides the
            // multi-restart axis (each iter, fresh start).
            let start: [f64; 10] = std::array::from_fn(|d| rng.gen_range(lb[d]..ub[d]));
            let (delta, neg_clearance) = nelder_mead_box(&mut loss, &start, &lb, &ub, inner_budget);
            let clearance_found = -neg_clearance;
            if clearance_found.is_finite() && clearance_found > best_clearance {
                best_candidate = apply_quat_delta(&seed_for_loss, &delta);
                best_clearance = clearance_found;
                last_improved = outer;
            }
            if outer > last_improved + MIN_FLAT_ITERS {
                break;
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
}

fn bootstrap_seed<R: Rng + ?Sized>(
    ec: &mut EvalCounter<'_>,
    rng: &mut R,
    max_evals: u64,
) -> Option<(Candidate, f64)> {
    let bootstrap_cap = ec.count().saturating_add(MAX_SEED_ATTEMPTS).min(max_evals);
    let mut best: Option<(Candidate, f64)> = None;
    while ec.count() < bootstrap_cap {
        let candidate = Candidate {
            outer: random_unit_quat(rng),
            inner: random_unit_quat(rng),
            translation: [0.0, 0.0],
        };
        let c = ec.evaluate(&candidate);
        if !c.is_finite() {
            continue;
        }
        match best {
            Some((_, best_c)) if c <= best_c => {}
            _ => best = Some((candidate, c)),
        }
        if c > 0.0 {
            return best;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    fn budget(max_evals: u64, seed: u64) -> Budget {
        Budget {
            max_evaluations: NonZeroU64::new(max_evals).expect("nonzero"),
            max_wall_time: None,
            seed,
        }
    }

    #[test]
    fn solves_cube() {
        let mut solver = Imperts;
        let p = rupert_shapes::cube();
        let mut ec = EvalCounter::new(&p);
        let outcome = solver.solve(&p, &budget(150_000, 0), &mut ec);
        assert!(
            matches!(outcome, SolverOutcome::Found { .. }),
            "got {outcome:?}"
        );
    }

    #[test]
    fn refines_above_random_quat_clearance() {
        // After Nelder-Mead-with-multi-restart inside the upstream-bounds
        // delta box, imperts should clearly beat random_quat's typical
        // ~0.02 cube clearance. Target: > 0.05.
        let mut solver = Imperts;
        let p = rupert_shapes::cube();
        let mut ec = EvalCounter::new(&p);
        let outcome = solver.solve(&p, &budget(200_000, 17), &mut ec);
        match outcome {
            SolverOutcome::Found { solution: sol, .. } => {
                assert!(
                    sol.clearance > 0.05,
                    "imperts cube clearance {} ≤ refinement target",
                    sol.clearance
                );
            }
            _ => unreachable!("imperts should find cube passage"),
        }
    }

    #[test]
    fn deterministic_for_same_seed() {
        let p = rupert_shapes::cube();
        let mut a = Imperts;
        let mut b = Imperts;
        let mut ec_a = EvalCounter::new(&p);
        let mut ec_b = EvalCounter::new(&p);
        let _ = a.solve(&p, &budget(50_000, 9), &mut ec_a);
        let _ = b.solve(&p, &budget(50_000, 9), &mut ec_b);
        assert_eq!(ec_a.count(), ec_b.count());
    }
}
