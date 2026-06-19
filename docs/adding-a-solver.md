# Adding a Solver

Three steps. No build-script magic, no `inventory` registry, no Cargo edits if you don't pull a new dep.

## 1. Drop a new module

`crates/rupert-solvers/src/<name>.rs`. Use the minimal template:

```rust
use rupert_core::{
    Budget, Candidate, EvalCounter, Polyhedron, Solution, Solver, SolverOutcome,
};

#[derive(Debug, Default)]
pub struct MyNewSolver;

impl Solver for MyNewSolver {
    fn name(&self) -> &'static str { "my_new_solver" }

    fn version(&self) -> &'static str { "0.1.0" }

    fn solve(
        &mut self,
        poly: &Polyhedron,
        budget: &Budget,
        ec: &mut EvalCounter<'_>,
    ) -> SolverOutcome {
        let max = budget.max_evaluations.get();
        // Your search here. Example:
        let candidate = Candidate::IDENTITY; // <- replace
        while ec.count() < max {
            let c = ec.evaluate(&candidate);
            if c.is_finite() && c > 0.0 {
                return SolverOutcome::Found(Solution {
                    candidate,
                    clearance: c,
                    found_at_eval: ec.count(),
                    certification: None,   // verifier fills this in
                });
            }
            // ... mutate candidate ...
        }
        SolverOutcome::Exhausted
    }
}
```

Constraints:

- All randomness must come from `rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(budget.seed)`. No `OsRng`. No clock seeds.
- `EvalCounter::evaluate` is the only legitimate path to a clearance number. Do NOT call `rupert_core::evaluate_clearance` directly — that bypasses the counter.
- File ≤ 500 lines. Function ≤ 100 lines (clippy enforces). Split into helpers.
- No `unwrap`, `panic!`, `todo!`, `unimplemented!`, `println!`, `eprintln!`, `dbg!` — workspace lints deny these.
- Error returns: panic-free. Use `SolverOutcome::Error(SolverError::Internal(...))` if something goes catastrophically wrong.

## 2. Register one line

In `crates/rupert-solvers/src/lib.rs::registered_solvers()`:

```rust
pub fn registered_solvers() -> Vec<Box<dyn Solver>> {
    vec![
        Box::new(RandomQuat),
        Box::new(FaceNormalPairs),
        Box::new(NelderMead),
        Box::new(RandomThenRefine),
        Box::new(HopfGrid),
        Box::new(MyNewSolver),  // <- add me
    ]
}
```

Also add `pub mod my_new_solver;` and `pub use my_new_solver::MyNewSolver;` near the top of `lib.rs`.

## 3. Write tests + iterate

Per-solver test pattern:

```rust
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
    fn solves_cube_within_budget() {
        let mut s = MyNewSolver;
        let p = rupert_shapes::cube();
        let mut ec = EvalCounter::new(&p);
        let outcome = s.solve(&p, &budget(50_000, 0), &mut ec);
        assert!(matches!(outcome, SolverOutcome::Found(_)));
    }
}
```

Then:

```bash
just check                            # fast workspace gate
just ci                               # full local gate before committing
cargo run --release -p rupert-cli -- run \
    --shape cube --solver my_new_solver --seed 0 --budget-evals 50000 \
    --out-dir results/baseline
cargo run --release -p rupert-cli -- verify results/baseline
cargo run --release -p rupert-cli -- lead build
```

If `lead build` shows your row in the headline, you're done. Commit `crates/rupert-solvers/src/my_new_solver.rs` and the registry change. Optionally commit a baseline result file under `results/baseline/` (gitignored by default; tracked subdirectories under `results/` are allowed by the `.gitignore` whitelist).
