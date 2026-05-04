//! Single-cell run harness.

use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use rupert_core::{
    Budget, BudgetSnapshot, EvalCounter, HostInfo, Polyhedron, RunOutcome, RunResult,
    SCHEMA_VERSION, Solver, SolverOutcome,
};
use rupert_verify::certify;
use time::OffsetDateTime;

/// Run `solver` against `poly` once, with the given budget and seed.
///
/// On a `SolverOutcome::Found` outcome, the harness invokes `rupert-verify`
/// to fill in the [`Solution::certification`] field. If certification
/// fails (e.g. the snap-and-recompute clearance falls below threshold),
/// the run becomes `RunOutcome::Disqualified`.
pub fn run_one(
    poly: &Polyhedron,
    solver: &mut dyn Solver,
    max_evaluations: NonZeroU64,
    seed: u64,
    max_wall_time: Option<Duration>,
) -> RunResult {
    let budget = Budget {
        max_evaluations,
        max_wall_time,
        seed,
    };
    let started = Instant::now();
    let mut ec = EvalCounter::new(poly);
    let outcome = solver.solve(poly, &budget, &mut ec);
    let wall_time_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let eval_count = ec.count();

    let (run_outcome, solution) = match outcome {
        SolverOutcome::Found(mut sol) => match certify(&sol, poly) {
            Ok(cert) => {
                sol.certification = Some(cert);
                (RunOutcome::Solved, Some(sol))
            }
            Err(err) => {
                let reason = format!("verifier rejected: {err}");
                (RunOutcome::Disqualified { reason }, Some(sol))
            }
        },
        SolverOutcome::Exhausted => (RunOutcome::Exhausted, None),
        SolverOutcome::Error(e) => (RunOutcome::from_solver_error(&e), None),
    };

    RunResult {
        schema_version: SCHEMA_VERSION,
        timestamp_utc: OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "unknown".to_string()),
        poly_id: *poly.id(),
        poly_name: poly.name.clone(),
        solver_name: solver.name().to_string(),
        solver_version: solver.version().to_string(),
        seed,
        budget: BudgetSnapshot::from_budget(&budget),
        outcome: run_outcome,
        eval_count,
        wall_time_ms,
        solution,
        host: HostInfo::collect(),
    }
}

#[cfg(test)]
mod tests {
    use rupert_solvers::FaceNormalPairs;
    use rupert_solvers::registered_solvers;

    use super::*;

    #[test]
    fn cube_with_face_normal_pairs_yields_solved() {
        let p = rupert_shapes::cube();
        let mut solver = FaceNormalPairs;
        let result = run_one(
            &p,
            &mut solver,
            NonZeroU64::new(110_000).expect("nz"),
            0,
            None,
        );
        assert!(
            matches!(result.outcome, RunOutcome::Solved),
            "got {:?}",
            result.outcome
        );
        assert!(result.solution.is_some());
        let sol = result.solution.expect("solution");
        assert!(sol.certification.is_some());
    }

    #[test]
    fn budget_exhaustion_yields_exhausted() {
        let p = rupert_shapes::cube();
        let mut solver = FaceNormalPairs;
        let result = run_one(
            &p,
            &mut solver,
            // Far too few evals for face_normal_pairs to reach the cube
            // passage (requires ≳ 1000 in our enumeration order).
            NonZeroU64::new(10).expect("nz"),
            0,
            None,
        );
        assert!(matches!(result.outcome, RunOutcome::Exhausted));
    }

    #[test]
    fn noperthedron_resists_every_v1_solver() {
        // Headline soundness regression. The noperthedron from
        // arXiv:2508.18475 is a proven non-Rupert convex polyhedron; if
        // any v1 solver finds a passage that the verifier accepts, either
        // (a) the seeds drifted from the paper values, (b) the verifier
        // is broken, or (c) the paper proof is wrong. Test fails on any
        // certified Solved outcome across all five solvers, three seeds,
        // 50_000 evals each.
        let p = rupert_shapes::noperthedron();
        let max_evals = NonZeroU64::new(50_000).expect("nz");
        for mut solver in registered_solvers() {
            for seed in [0u64, 1, 2] {
                let r = run_one(&p, solver.as_mut(), max_evals, seed, None);
                assert!(
                    !matches!(r.outcome, RunOutcome::Solved),
                    "{} seed={seed} unexpectedly Solved noperthedron — outcome={:?}, clearance={:?}",
                    solver.name(),
                    r.outcome,
                    r.solution.as_ref().map(|s| s.clearance)
                );
            }
        }
    }
}
