//! Aggregate raw `RunResult` records into a leaderboard view.

use std::collections::BTreeMap;

use rupert_core::{RunOutcome, RunResult};

/// One row of the headline leaderboard. We aggregate over `(shape, solver)`
/// using the BEST seed: the result with the smallest `eval_count` among
/// all seeds. Ties broken by clearance (more clearance is better).
#[derive(Debug, Clone)]
pub struct LeaderboardRow {
    pub shape: String,
    pub solver: String,
    pub solver_version: String,
    pub best_seed: u64,
    pub best_eval_count: u64,
    pub best_clearance: f64,
    pub wall_time_ms: u64,
    pub samples: usize,
}

/// Result categories presented in the rendered leaderboard.
#[derive(Debug, Clone, Default)]
pub struct AggregatedView {
    /// Certified Solved rows — the headline ranking.
    pub headline: Vec<LeaderboardRow>,
    /// Solved-but-uncertified rows (e.g. solver returned Found but
    /// verifier's snap-and-recompute disagreed; preserved for diagnostics).
    pub uncertified: Vec<LeaderboardRow>,
    /// Shapes for which NO solver has produced a certified passage.
    pub open_problems: Vec<String>,
}

pub fn aggregate(results: &[RunResult]) -> AggregatedView {
    type Key = (String, String);
    let mut headline_buckets: BTreeMap<Key, Vec<&RunResult>> = BTreeMap::new();
    let mut uncertified_buckets: BTreeMap<Key, Vec<&RunResult>> = BTreeMap::new();
    let mut shapes_with_certified: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    let mut all_shapes: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for r in results {
        all_shapes.insert(r.poly_name.clone());
        let key = (r.poly_name.clone(), r.solver_name.clone());
        match &r.outcome {
            RunOutcome::Solved => {
                let certified = r
                    .solution
                    .as_ref()
                    .is_some_and(|s| s.certification.is_some());
                if certified {
                    headline_buckets.entry(key).or_default().push(r);
                    shapes_with_certified.insert(r.poly_name.clone());
                } else {
                    uncertified_buckets.entry(key).or_default().push(r);
                }
            }
            RunOutcome::Disqualified { .. } | RunOutcome::Error { .. } => {
                uncertified_buckets.entry(key).or_default().push(r);
            }
            RunOutcome::Exhausted => {
                // Excluded from leaderboard; they signal "solver never
                // solved this shape with this seed".
            }
        }
    }

    let headline = headline_buckets
        .into_iter()
        .map(|(k, group)| best_row(k.0, k.1, &group))
        .collect();
    let uncertified = uncertified_buckets
        .into_iter()
        .map(|(k, group)| best_row(k.0, k.1, &group))
        .collect();
    let open_problems: Vec<String> = all_shapes
        .difference(&shapes_with_certified)
        .cloned()
        .collect();

    AggregatedView {
        headline,
        uncertified,
        open_problems,
    }
}

fn best_row(shape: String, solver: String, group: &[&RunResult]) -> LeaderboardRow {
    // Pick the run with the smallest eval_count. Ties broken by clearance
    // (highest wins).
    let mut best: &RunResult = group[0];
    for r in &group[1..] {
        if r.eval_count < best.eval_count {
            best = r;
        } else if r.eval_count == best.eval_count {
            let r_c = r.solution.as_ref().map_or(0.0, |s| s.clearance);
            let b_c = best.solution.as_ref().map_or(0.0, |s| s.clearance);
            if r_c > b_c {
                best = r;
            }
        }
    }
    LeaderboardRow {
        shape,
        solver,
        solver_version: best.solver_version.clone(),
        best_seed: best.seed,
        best_eval_count: best.eval_count,
        best_clearance: best.solution.as_ref().map_or(0.0, |s| s.clearance),
        wall_time_ms: best.wall_time_ms,
        samples: group.len(),
    }
}

#[cfg(test)]
mod tests {
    use rupert_core::{
        BudgetSnapshot, Candidate, CertMethod, Certification, HostInfo, RunOutcome, RunResult,
        SCHEMA_VERSION, Solution,
    };

    use super::*;

    fn certified_run(
        shape: &str,
        solver: &str,
        seed: u64,
        evals: u64,
        clearance: f64,
    ) -> RunResult {
        RunResult {
            schema_version: SCHEMA_VERSION,
            timestamp_utc: "x".into(),
            poly_id: *rupert_core::Polyhedron::new(
                "p",
                vec![
                    rupert_core::Vec3::ZERO,
                    rupert_core::Vec3::X,
                    rupert_core::Vec3::Y,
                    rupert_core::Vec3::Z,
                ],
                vec![],
            )
            .expect("poly")
            .id(),
            poly_name: shape.into(),
            solver_name: solver.into(),
            solver_version: "0.1.0".into(),
            seed,
            budget: BudgetSnapshot {
                max_evaluations: 1000,
                max_wall_time_ms: None,
            },
            outcome: RunOutcome::Solved,
            eval_count: evals,
            wall_time_ms: 1,
            solution: Some(Solution {
                candidate: Candidate::IDENTITY,
                clearance,
                found_at_eval: evals,
                certification: Some(Certification {
                    method: CertMethod::F64Epsilon,
                    clearance_lo: clearance,
                    clearance_hi: clearance,
                }),
            }),
            host: HostInfo::collect(),
        }
    }

    fn exhausted_run(shape: &str, solver: &str, seed: u64) -> RunResult {
        let mut r = certified_run(shape, solver, seed, 1000, 0.0);
        r.outcome = RunOutcome::Exhausted;
        r.solution = None;
        r
    }

    #[test]
    fn picks_best_eval_count() {
        let runs = vec![
            certified_run("cube", "rq", 0, 100, 0.1),
            certified_run("cube", "rq", 1, 50, 0.05),
            certified_run("cube", "rq", 2, 80, 0.2),
        ];
        let view = aggregate(&runs);
        assert_eq!(view.headline.len(), 1);
        let row = &view.headline[0];
        assert_eq!(row.best_eval_count, 50);
        assert_eq!(row.best_seed, 1);
        assert_eq!(row.samples, 3);
    }

    #[test]
    fn open_problems_are_shapes_with_no_certified_solution() {
        let runs = vec![
            certified_run("cube", "rq", 0, 50, 0.1),
            exhausted_run("noperthedron", "rq", 0),
        ];
        let view = aggregate(&runs);
        assert_eq!(view.open_problems, vec!["noperthedron".to_string()]);
    }

    #[test]
    fn uncertified_solutions_excluded_from_headline() {
        let mut r = certified_run("cube", "rq", 0, 50, 0.1);
        r.solution.as_mut().expect("sol").certification = None;
        let view = aggregate(&[r]);
        assert!(view.headline.is_empty());
        assert_eq!(view.uncertified.len(), 1);
    }
}
