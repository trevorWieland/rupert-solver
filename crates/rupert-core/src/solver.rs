//! The `Solver` trait and the contract types every solver targets.

use std::num::NonZeroU64;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::quat::Quat;

/// A candidate configuration: two rotations (outer and inner copy of the
/// polyhedron) plus a 2D translation in the projection plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub outer: Quat,
    pub inner: Quat,
    pub translation: [f64; 2],
}

impl Candidate {
    pub const IDENTITY: Self = Self {
        outer: Quat::IDENTITY,
        inner: Quat::IDENTITY,
        translation: [0.0, 0.0],
    };
}

/// Bounds on a single solver run.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub max_evaluations: NonZeroU64,
    pub max_wall_time: Option<Duration>,
    pub seed: u64,
}

/// What a solver returns from a single run.
#[derive(Debug, Clone)]
pub enum SolverOutcome {
    /// A candidate with strictly positive (f64) clearance was found.
    Found(Solution),
    /// Budget was exhausted with no positive-clearance candidate.
    Exhausted,
    /// Internal solver error (NaN, invariant violation, etc.).
    Error(SolverError),
}

/// A solution found by a solver. Until [`certification`] is filled in by the
/// verifier, the solution is "uncertified" and excluded from leaderboard
/// rankings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    pub candidate: Candidate,
    /// f64 clearance value as observed by the solver. Strictly > 0.
    pub clearance: f64,
    /// `EvalCounter::count()` reading at the moment the solution was accepted.
    pub found_at_eval: u64,
    /// `None` until [`rupert-verify`] runs.
    pub certification: Option<Certification>,
}

/// Verifier output. v1 ships [`CertMethod::F64Epsilon`] only; v2 layers in
/// the interval and exact paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certification {
    pub method: CertMethod,
    /// Lower bound on the clearance, in interval-arithmetic terms when
    /// applicable. For F64Epsilon, this is the recomputed f64 clearance.
    pub clearance_lo: f64,
    /// Upper bound (or equal to `clearance_lo` for F64Epsilon).
    pub clearance_hi: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertMethod {
    /// f64 recomputation with margin > epsilon. v1 default.
    F64Epsilon,
    /// Inari interval arithmetic over rationally-snapped quaternions.
    /// v2; gated behind the `interval` feature in `rupert-verify`.
    IntervalSnap,
    /// Exact rational verification via malachite. v2; gated behind `exact`.
    ExactRational,
}

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum SolverError {
    #[error("nan or inf in candidate clearance")]
    NumericNonFinite,
    #[error("internal: {0}")]
    Internal(String),
}

/// Trait every registered solver implements. v1 contract:
///
/// - `solve()` is invoked single-threaded; the harness re-instantiates a
///   solver per `(shape, solver, seed)` task in [`rupert_bench::sweep`].
/// - The eval count after `solve()` returns is `ec.count()` — the harness
///   uses it as the authoritative count. Solvers MUST NOT bypass
///   [`crate::eval::EvalCounter::evaluate`] (an xtask gate enforces no
///   solver imports `rupert_core::evaluate_clearance` directly).
pub trait Solver: Send + std::fmt::Debug {
    fn name(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn solve(
        &mut self,
        poly: &crate::poly::Polyhedron,
        budget: &Budget,
        ec: &mut crate::eval::EvalCounter<'_>,
    ) -> SolverOutcome;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_identity_serializes() {
        let json = serde_json::to_string(&Candidate::IDENTITY).expect("serialize");
        let back: Candidate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, Candidate::IDENTITY);
    }

    #[test]
    fn cert_method_serializes_snake_case() {
        let m = CertMethod::F64Epsilon;
        let s = serde_json::to_string(&m).expect("serialize");
        assert_eq!(s, "\"f64_epsilon\"");
    }
}
