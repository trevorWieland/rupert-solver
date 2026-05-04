//! `RunResult` — one JSONL line per solver run.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::polyid::PolyId;
use crate::solver::{Solution, SolverError};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub schema_version: u32,
    pub timestamp_utc: String,
    pub poly_id: PolyId,
    pub poly_name: String,
    pub solver_name: String,
    pub solver_version: String,
    pub seed: u64,
    pub budget: BudgetSnapshot,
    pub outcome: RunOutcome,
    pub eval_count: u64,
    pub wall_time_ms: u64,
    pub solution: Option<Solution>,
    pub host: HostInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    pub max_evaluations: u64,
    pub max_wall_time_ms: Option<u64>,
}

impl BudgetSnapshot {
    pub fn from_budget(b: &crate::solver::Budget) -> Self {
        Self {
            max_evaluations: b.max_evaluations.get(),
            max_wall_time_ms: b
                .max_wall_time
                .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64),
        }
    }
}

impl From<Duration> for BudgetSnapshot {
    fn from(d: Duration) -> Self {
        Self {
            max_evaluations: 0,
            max_wall_time_ms: Some(d.as_millis().min(u128::from(u64::MAX)) as u64),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RunOutcome {
    Solved,
    Exhausted,
    Error { message: String },
    Disqualified { reason: String },
}

impl RunOutcome {
    pub fn from_solver_error(e: &SolverError) -> Self {
        Self::Error {
            message: e.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub rustc: String,
    pub target: String,
    pub git_rev: String,
}

impl HostInfo {
    /// Best-effort host introspection. Falls back to "unknown" rather than
    /// failing — host info is for diagnostics, not correctness.
    pub fn collect() -> Self {
        Self {
            rustc: option_env!("RUSTC_SEMVER").unwrap_or("unknown").to_string(),
            target: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            git_rev: option_env!("RUPERT_GIT_REV")
                .unwrap_or("unknown")
                .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_outcome_round_trips() {
        let o = RunOutcome::Solved;
        let s = serde_json::to_string(&o).expect("serialize");
        let back: RunOutcome = serde_json::from_str(&s).expect("deserialize");
        assert!(matches!(back, RunOutcome::Solved));
    }

    #[test]
    fn disqualified_carries_reason() {
        let o = RunOutcome::Disqualified {
            reason: "drift".into(),
        };
        let s = serde_json::to_string(&o).expect("serialize");
        assert!(s.contains("drift"));
    }
}
