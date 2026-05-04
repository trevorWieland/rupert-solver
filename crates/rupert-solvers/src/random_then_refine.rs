//! Two-phase: random sampling until clearance > -ε, then Nelder-Mead from
//! that seed. The workhorse baseline — should be the leaderboard's
//! reference performer on regular polyhedra.

use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use rupert_core::{
    Budget, Candidate, EvalCounter, Polyhedron, Solution, Solver, SolverOutcome,
};

use crate::sample::random_unit_quat;

#[derive(Debug, Default)]
pub struct RandomThenRefine;

const NEAR_BASIN_THRESHOLD: f64 = -0.05;

impl Solver for RandomThenRefine {
    fn name(&self) -> &'static str {
        "random_then_refine"
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
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(budget.seed);
        let max = budget.max_evaluations.get();
        // Phase 1: random search; promote to refinement once we have a
        // candidate that's "close" (clearance ≥ NEAR_BASIN_THRESHOLD).
        loop {
            if ec.count() >= max {
                return SolverOutcome::Exhausted;
            }
            let candidate = Candidate {
                outer: random_unit_quat(&mut rng),
                inner: random_unit_quat(&mut rng),
                translation: [0.0, 0.0],
            };
            let c = ec.evaluate(&candidate);
            if !c.is_finite() {
                continue;
            }
            if c > 0.0 {
                return SolverOutcome::Found(Solution {
                    candidate,
                    clearance: c,
                    found_at_eval: ec.count(),
                    certification: None,
                });
            }
            if c >= NEAR_BASIN_THRESHOLD {
                if let Some(sol) = local_refine(ec, candidate, max) {
                    return SolverOutcome::Found(sol);
                }
            }
        }
    }
}

/// Coordinate-descent refinement: probe each parameter ± delta, accept if
/// it improves clearance. Cheap and effective once we're near the basin.
fn local_refine(
    ec: &mut EvalCounter<'_>,
    seed: Candidate,
    max_evals: u64,
) -> Option<Solution> {
    let mut current = seed;
    let mut current_clearance = ec.evaluate(&current);
    let mut delta = 0.05_f64;
    let min_delta = 1e-6_f64;
    while delta > min_delta {
        if ec.count() >= max_evals {
            return None;
        }
        let mut improved = false;
        let probes = [
            translate_perturbed(current, delta, 0.0),
            translate_perturbed(current, -delta, 0.0),
            translate_perturbed(current, 0.0, delta),
            translate_perturbed(current, 0.0, -delta),
        ];
        for probe in probes {
            if ec.count() >= max_evals {
                return None;
            }
            let c = ec.evaluate(&probe);
            if c.is_finite() && c > current_clearance {
                current_clearance = c;
                current = probe;
                improved = true;
                if c > 0.0 {
                    return Some(Solution {
                        candidate: current,
                        clearance: c,
                        found_at_eval: ec.count(),
                        certification: None,
                    });
                }
            }
        }
        if !improved {
            delta *= 0.5;
        }
    }
    None
}

fn translate_perturbed(c: Candidate, dx: f64, dy: f64) -> Candidate {
    Candidate {
        outer: c.outer,
        inner: c.inner,
        translation: [c.translation[0] + dx, c.translation[1] + dy],
    }
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
    fn solves_cube_in_under_5k_evals_majority_of_seeds() {
        let p = rupert_shapes::cube();
        let mut hits = 0;
        let trials: u64 = 50;
        for seed in 0..trials {
            let mut solver = RandomThenRefine;
            let mut ec = EvalCounter::new(&p);
            let outcome = solver.solve(&p, &budget(5_000, seed), &mut ec);
            if matches!(outcome, SolverOutcome::Found(_)) {
                hits += 1;
            }
        }
        assert!(
            hits >= 45,
            "only {hits}/{trials} cube seeds solved in 5k evals"
        );
    }
}
