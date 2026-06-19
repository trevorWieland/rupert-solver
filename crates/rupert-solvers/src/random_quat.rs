//! Pure rejection sampling: draw random unit-quat pairs and return the
//! first that yields strictly positive clearance.

use std::time::Instant;

use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use rupert_core::{Budget, Candidate, EvalCounter, Polyhedron, Solution, Solver, SolverOutcome};

use crate::sample::random_unit_quat;

/// `random_quat` solver. Pure rotation-pair sampling at fixed (0, 0)
/// translation — many regular solids have their optimal translation near
/// origin; combining random translation with
/// random rotation makes the hit rate disastrously low. Translation search
/// is the job of `random_then_refine`'s phase 2.
#[derive(Debug, Default)]
pub struct RandomQuat;

impl Solver for RandomQuat {
    fn name(&self) -> &'static str {
        "random_quat"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn solve(
        &mut self,
        _poly: &Polyhedron,
        budget: &Budget,
        ec: &mut EvalCounter<'_>,
    ) -> SolverOutcome {
        let started = Instant::now();
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(budget.seed);
        let max_evals = budget.max_evaluations.get();

        while ec.count() < max_evals {
            if let Some(limit) = budget.max_wall_time {
                if started.elapsed() >= limit {
                    break;
                }
            }
            let candidate = Candidate {
                outer: random_unit_quat(&mut rng),
                inner: random_unit_quat(&mut rng),
                translation: [0.0, 0.0],
            };
            let c = ec.evaluate(&candidate);
            if c.is_finite() && c > 0.0 {
                return SolverOutcome::found(Solution {
                    candidate,
                    clearance: c,
                    found_at_eval: ec.count(),
                    certification: None,
                });
            }
        }
        SolverOutcome::exhausted()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use rupert_core::Polyhedron;

    use super::*;

    fn cube() -> Polyhedron {
        rupert_shapes::cube()
    }

    fn budget(max_evals: u64, seed: u64) -> Budget {
        Budget {
            max_evaluations: NonZeroU64::new(max_evals).expect("nonzero"),
            max_wall_time: None,
            seed,
        }
    }

    #[test]
    fn finds_cube_passage_within_10k_evals_majority_of_seeds() {
        // Stat test: ≥ 95 / 100 seeds find a passage.
        let p = cube();
        let mut hits = 0;
        let trials: u64 = 100;
        for seed in 0..trials {
            let mut solver = RandomQuat;
            let mut ec = EvalCounter::new(&p);
            let outcome = solver.solve(&p, &budget(10_000, seed), &mut ec);
            if matches!(outcome, SolverOutcome::Found { .. }) {
                hits += 1;
            }
        }
        assert!(hits >= 95, "only {hits}/{trials} seeds found a passage");
    }

    #[test]
    fn deterministic_for_same_seed() {
        let p = cube();
        let mut solver_a = RandomQuat;
        let mut solver_b = RandomQuat;
        let mut ec1 = EvalCounter::new(&p);
        let outcome_a = solver_a.solve(&p, &budget(5_000, 42), &mut ec1);
        let count_a = ec1.count();
        let mut ec2 = EvalCounter::new(&p);
        let outcome_b = solver_b.solve(&p, &budget(5_000, 42), &mut ec2);
        let count_b = ec2.count();
        assert_eq!(count_a, count_b);
        let determinism_ok = match (outcome_a, outcome_b) {
            (
                SolverOutcome::Found { solution: a, .. },
                SolverOutcome::Found { solution: b, .. },
            ) => a.found_at_eval == b.found_at_eval && a.candidate == b.candidate,
            (SolverOutcome::Exhausted { .. }, SolverOutcome::Exhausted { .. }) => true,
            _ => false,
        };
        assert!(determinism_ok, "non-deterministic outcome");
    }

    // Note: exhaustion-against-hard-shapes regression lives in
    // `rupert-bench` rather than per-solver, since the property we care
    // about is fleet-wide ("no solver finds a passage") rather than
    // solver-specific.
}
