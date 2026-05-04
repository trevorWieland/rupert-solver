//! BDD harness for rupert-bench. Scenarios live under `tests/bdd/features/`.

use std::num::NonZeroU64;

use cucumber::{World, given, then, when};
use rupert_bench::{SweepConfig, run_one, run_sweep};
use rupert_core::{RunOutcome, RunResult};
use rupert_solvers::FaceNormalPairs;

#[derive(Debug, Default, World)]
struct BenchWorld {
    last_result: Option<RunResult>,
    sweep_a: Vec<RunResult>,
    sweep_b: Vec<RunResult>,
}

#[given(regex = r"^a cube polyhedron$")]
async fn given_cube(_w: &mut BenchWorld) {}

#[when(regex = r"^FaceNormalPairs runs against the cube with budget (\d+)$")]
async fn run_with_budget(w: &mut BenchWorld, budget_evals: u64) {
    let p = rupert_shapes::cube();
    let mut solver = FaceNormalPairs;
    let r = run_one(
        &p,
        &mut solver,
        NonZeroU64::new(budget_evals).expect("nz"),
        0,
        None,
    );
    w.last_result = Some(r);
}

#[then(regex = r"^the outcome is Solved$")]
async fn outcome_solved(w: &mut BenchWorld) {
    let r = w.last_result.as_ref().expect("ran");
    assert!(matches!(r.outcome, RunOutcome::Solved), "got {:?}", r.outcome);
}

#[then(regex = r"^the outcome is Exhausted$")]
async fn outcome_exhausted(w: &mut BenchWorld) {
    let r = w.last_result.as_ref().expect("ran");
    assert!(matches!(r.outcome, RunOutcome::Exhausted), "got {:?}", r.outcome);
}

#[then(regex = r"^the result has a certified solution$")]
async fn certified(w: &mut BenchWorld) {
    let r = w.last_result.as_ref().expect("ran");
    let sol = r.solution.as_ref().expect("solution present");
    assert!(sol.certification.is_some());
}

#[when(regex = r"^I run a sweep over cube/octahedron with seeds 0,1,2 twice$")]
async fn sweep_twice(w: &mut BenchWorld) {
    let cfg = SweepConfig {
        max_evaluations: NonZeroU64::new(2_000).expect("nz"),
        seeds: vec![0, 1, 2],
        max_wall_time: None,
    };
    let shapes = vec![rupert_shapes::cube(), rupert_shapes::octahedron()];
    let solvers = vec!["face_normal_pairs".to_string()];
    w.sweep_a = run_sweep(&shapes, &solvers, &cfg).expect("sweep_a");
    w.sweep_b = run_sweep(&shapes, &solvers, &cfg).expect("sweep_b");
}

#[then(regex = r"^both sweeps yield byte-equal solution payloads$")]
async fn byte_equal_solutions(w: &mut BenchWorld) {
    assert_eq!(w.sweep_a.len(), w.sweep_b.len(), "different sweep lengths");
    // Sort both by (poly_name, solver_name, seed) so order is canonical.
    let mut a = w.sweep_a.clone();
    let mut b = w.sweep_b.clone();
    let key = |r: &RunResult| (r.poly_name.clone(), r.solver_name.clone(), r.seed);
    a.sort_by_key(&key);
    b.sort_by_key(&key);
    for (ar, br) in a.iter().zip(b.iter()) {
        let aj = serde_json::to_string(&ar.solution).expect("ser a");
        let bj = serde_json::to_string(&br.solution).expect("ser b");
        assert_eq!(
            aj, bj,
            "non-deterministic solution for {:?}: {aj} vs {bj}",
            key(ar)
        );
    }
}

fn main() {
    futures::executor::block_on(BenchWorld::run("tests/bdd/features"));
}
