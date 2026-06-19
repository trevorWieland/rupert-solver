//! The `Solver` trait and the contract types every solver targets.

use std::num::NonZeroU64;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::quat::Quat;

/// Clearance epsilon shared by run-result telemetry. It matches the f64
/// verifier threshold: strict passages must clear this margin.
pub const CLEARANCE_EPS: f64 = 1.0e-9;

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
    Found {
        solution: Solution,
        telemetry: Option<SolverTelemetry>,
    },
    /// Budget was exhausted with no positive-clearance candidate.
    Exhausted { telemetry: Option<SolverTelemetry> },
    /// Internal solver error (NaN, invariant violation, etc.).
    Error(SolverError),
}

impl SolverOutcome {
    pub fn found(solution: Solution) -> Self {
        Self::Found {
            solution,
            telemetry: None,
        }
    }

    pub fn exhausted() -> Self {
        Self::Exhausted { telemetry: None }
    }
}

/// A solution found by a solver. Until `certification` is filled in by the
/// verifier, the solution is "uncertified" and excluded from leaderboard
/// rankings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    pub candidate: Candidate,
    /// f64 clearance value as observed by the solver. Strictly > 0.
    pub clearance: f64,
    /// `EvalCounter::count()` reading at the moment the solution was accepted.
    pub found_at_eval: u64,
    /// `None` until `rupert-verify` runs.
    pub certification: Option<Certification>,
}

/// Best candidate observed by a run, including exhausted runs. This is
/// diagnostic telemetry for experiments, not a leaderboard certification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedCandidate {
    pub candidate: Candidate,
    pub clearance: f64,
    pub observed_at_eval: u64,
    pub certification: Option<Certification>,
}

/// Solver-specific telemetry that helps interpret exhausted long runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SolverTelemetry {
    PatchAware(PatchAwareTelemetry),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchAwareTelemetry {
    pub canonical_cells: usize,
    pub cell_pairs: usize,
    pub recon_cells_evaluated: usize,
    pub optimized_cells: usize,
    pub cells_skipped_by_slack: usize,
    pub cells_skipped_by_interval_bound: usize,
    #[serde(default)]
    pub bound_cells_evaluated: usize,
    #[serde(default)]
    pub bound_cells_ambiguous: usize,
    #[serde(default)]
    pub bound_histogram: PatchAwareBoundHistogram,
    #[serde(default)]
    pub adaptive_refinement_cells: usize,
    #[serde(default)]
    pub adaptive_refinement_evals: u64,
    #[serde(default)]
    pub best_positive_cell: Option<PatchAwareCellSummary>,
    #[serde(default)]
    pub best_near_miss_cell: Option<PatchAwareCellSummary>,
    #[serde(default)]
    pub best_boundary_cell: Option<PatchAwareCellSummary>,
    #[serde(default)]
    pub cell_summaries: Vec<PatchAwareCellSummary>,
    pub top_cells: Vec<PatchAwareCellSummary>,
    pub clearance_histogram: ClearanceHistogram,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatchAwareBoundHistogram {
    pub le_current_best: usize,
    pub current_best_to_plus_1e_minus_3: usize,
    pub plus_1e_minus_3_to_plus_1e_minus_2: usize,
    pub plus_1e_minus_2_to_plus_1e_minus_1: usize,
    pub plus_1e_minus_1_to_plus_1: usize,
    pub plus_1_or_more: usize,
}

impl PatchAwareBoundHistogram {
    pub fn record(&mut self, upper_bound: f64, current_best: f64) {
        if !upper_bound.is_finite() || !current_best.is_finite() {
            self.plus_1_or_more += 1;
            return;
        }
        let gap = upper_bound - current_best;
        if gap <= 0.0 {
            self.le_current_best += 1;
        } else if gap <= 1.0e-3 {
            self.current_best_to_plus_1e_minus_3 += 1;
        } else if gap <= 1.0e-2 {
            self.plus_1e_minus_3_to_plus_1e_minus_2 += 1;
        } else if gap <= 1.0e-1 {
            self.plus_1e_minus_2_to_plus_1e_minus_1 += 1;
        } else if gap <= 1.0 {
            self.plus_1e_minus_1_to_plus_1 += 1;
        } else {
            self.plus_1_or_more += 1;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchAwareCellSummary {
    pub outer_cell: usize,
    pub inner_cell: usize,
    #[serde(default)]
    pub start_eval: u64,
    #[serde(default)]
    pub end_eval: u64,
    #[serde(default)]
    pub evals_spent: u64,
    pub recon_clearance: f64,
    pub best_clearance: f64,
    #[serde(default)]
    pub skip_reason: PatchAwareSkipReason,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchAwareSkipReason {
    #[default]
    None,
    Slack,
    Bound,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClearanceHistogram {
    pub neg_inf_to_neg_one: usize,
    pub neg_one_to_neg_point_one: usize,
    pub neg_point_one_to_neg_point_zero_one: usize,
    pub neg_point_zero_one_to_neg_point_zero_zero_one: usize,
    pub neg_point_zero_zero_one_to_zero: usize,
    pub positive: usize,
}

impl ClearanceHistogram {
    pub fn record(&mut self, clearance: f64) {
        if clearance > 0.0 {
            self.positive += 1;
        } else if clearance > -0.001 {
            self.neg_point_zero_zero_one_to_zero += 1;
        } else if clearance > -0.01 {
            self.neg_point_zero_one_to_neg_point_zero_zero_one += 1;
        } else if clearance > -0.1 {
            self.neg_point_one_to_neg_point_zero_one += 1;
        } else if clearance > -1.0 {
            self.neg_one_to_neg_point_one += 1;
        } else {
            self.neg_inf_to_neg_one += 1;
        }
    }
}

/// Verifier output. Stronger methods are preferred when available.
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
    /// f64 recomputation with margin > epsilon.
    F64Epsilon,
    /// Inari interval arithmetic over exact vertex tables.
    IntervalSnap,
    /// Exact rational verification via malachite.
    ExactRational,
}

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum SolverError {
    #[error("nan or inf in candidate clearance")]
    NumericNonFinite,
    #[error("internal: {0}")]
    Internal(String),
}

/// Trait every registered solver implements. Contract:
///
/// - `solve()` is invoked single-threaded; the harness re-instantiates a
///   solver per `(shape, solver, seed)` task in `rupert_bench::sweep`.
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
