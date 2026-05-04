//! rupert-verify — snap-and-certify verifier.
//!
//! v1 ships [`CertMethod::F64Epsilon`]: recompute the candidate's clearance
//! against the polyhedron in `f64`, accept if margin > [`F64_EPS`]. The
//! interval and exact paths are gated behind feature flags and arrive in v2.

pub mod fallback_f64;

use rupert_core::{CertMethod, Certification, Polyhedron, Solution, evaluate_clearance};
use thiserror::Error;

/// Recomputed clearance must exceed this threshold to be accepted as a
/// certified passage. 1e-9 is chosen to be larger than the expected
/// rounding error in the projection-hull-clearance pipeline (which
/// involves a handful of multiplications and additions per vertex).
pub const F64_EPS: f64 = 1.0e-9;

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("solution clearance is non-finite")]
    NonFiniteClearance,
    #[error("recomputed clearance {recomputed} disagrees with reported {reported} (tolerance {tol})")]
    ClearanceMismatch {
        recomputed: f64,
        reported: f64,
        tol: f64,
    },
    #[error("recomputed clearance {0} not strictly positive (≤ {1})")]
    NotStrictlyPositive(f64, f64),
}

/// Run the verifier on a single solution. Returns a [`Certification`]
/// (with `method = F64Epsilon`) on success, or a [`VerifyError`].
///
/// On success, the caller MUST set `solution.certification` to the returned
/// value before promoting the solution to the leaderboard.
pub fn certify(solution: &Solution, poly: &Polyhedron) -> Result<Certification, VerifyError> {
    if !solution.clearance.is_finite() {
        return Err(VerifyError::NonFiniteClearance);
    }
    let recomputed = evaluate_clearance(poly, &solution.candidate);
    if !recomputed.is_finite() {
        return Err(VerifyError::NonFiniteClearance);
    }
    if recomputed <= F64_EPS {
        return Err(VerifyError::NotStrictlyPositive(recomputed, F64_EPS));
    }
    // The recomputation should match the solver's reported clearance to
    // within rounding error. The candidates are bitwise identical, so any
    // disagreement reflects platform-level non-determinism (FMA ordering
    // etc.) — flag generously.
    let tol = 1e-6_f64;
    if (recomputed - solution.clearance).abs() > tol {
        return Err(VerifyError::ClearanceMismatch {
            recomputed,
            reported: solution.clearance,
            tol,
        });
    }
    Ok(Certification {
        method: CertMethod::F64Epsilon,
        clearance_lo: recomputed,
        clearance_hi: recomputed,
    })
}

#[cfg(test)]
mod tests {
    use rupert_core::{Budget, Candidate, EvalCounter, Solver, SolverOutcome};
    use rupert_solvers::FaceNormalPairs;
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
    fn rejects_identity_solution_for_every_builtin() {
        // The identity candidate yields clearance 0 (touching shadows) for
        // every centrally-symmetric builtin. The verifier MUST reject it.
        let identity_solution = Solution {
            candidate: Candidate::IDENTITY,
            clearance: 0.0,
            found_at_eval: 0,
            certification: None,
        };
        for poly in rupert_shapes::builtins() {
            let result = certify(&identity_solution, &poly);
            assert!(
                result.is_err(),
                "verifier accepted identity for {}: {result:?}",
                poly.name
            );
        }
    }

    #[test]
    fn accepts_genuine_cube_passage() {
        let poly = rupert_shapes::cube();
        let mut solver = FaceNormalPairs;
        let mut ec = EvalCounter::new(&poly);
        let outcome = solver.solve(&poly, &budget(110_000, 0), &mut ec);
        let SolverOutcome::Found(sol) = outcome else {
            unreachable!("FaceNormalPairs failed to solve cube");
        };
        let cert = certify(&sol, &poly).expect("certify");
        assert!(cert.clearance_lo > F64_EPS);
        assert_eq!(cert.method, CertMethod::F64Epsilon);
    }

    #[test]
    fn rejects_non_finite_clearance() {
        let bad = Solution {
            candidate: Candidate::IDENTITY,
            clearance: f64::NAN,
            found_at_eval: 0,
            certification: None,
        };
        let r = certify(&bad, &rupert_shapes::cube());
        assert!(matches!(r, Err(VerifyError::NonFiniteClearance)));
    }

    #[test]
    fn flags_clearance_drift() {
        // Construct a solution whose reported clearance disagrees wildly
        // with the recomputed clearance. The verifier's job is to surface
        // this as ClearanceMismatch.
        let poly = rupert_shapes::cube();
        let mut solver = FaceNormalPairs;
        let mut ec = EvalCounter::new(&poly);
        let outcome = solver.solve(&poly, &budget(110_000, 0), &mut ec);
        let SolverOutcome::Found(mut sol) = outcome else {
            unreachable!("FaceNormalPairs failed to solve cube");
        };
        sol.clearance = 999.0;
        let r = certify(&sol, &poly);
        assert!(matches!(r, Err(VerifyError::ClearanceMismatch { .. })), "got {r:?}");
    }
}
