//! rupert-solvers — Solver implementations for the Rupert problem.
//!
//! Each solver lives in its own module under 500 lines. To register a new
//! solver:
//!
//! 1. Drop a new module `crates/rupert-solvers/src/<name>.rs` exposing a
//!    type implementing [`rupert_core::Solver`].
//! 2. Append it to [`registered_solvers`].
//!
//! Adding a solver should NOT touch anything else in the workspace.

pub mod face_normal_pairs;
pub mod gosain_grimmer;
pub mod hopf_grid;
pub mod imperts;
pub mod nelder_mead;
pub mod random_quat;
pub mod random_then_refine;
pub mod sample;

use rupert_core::Solver;

pub use face_normal_pairs::FaceNormalPairs;
pub use gosain_grimmer::GosainGrimmer;
pub use hopf_grid::HopfGrid;
pub use imperts::Imperts;
pub use nelder_mead::NelderMead;
pub use random_quat::RandomQuat;
pub use random_then_refine::RandomThenRefine;

/// Every solver we ship. Append a `Box::new(MyNewSolver)` here to register
/// a new solver.
pub fn registered_solvers() -> Vec<Box<dyn Solver>> {
    vec![
        Box::new(RandomQuat),
        Box::new(FaceNormalPairs),
        Box::new(NelderMead),
        Box::new(RandomThenRefine),
        Box::new(HopfGrid),
        Box::new(GosainGrimmer),
        Box::new(Imperts),
    ]
}

/// Look up a solver by name; returns `None` if unknown.
pub fn lookup(name: &str) -> Option<Box<dyn Solver>> {
    registered_solvers().into_iter().find(|s| s.name() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_solvers_count_is_seven() {
        assert_eq!(registered_solvers().len(), 7);
    }

    #[test]
    fn solver_names_are_unique() {
        let solvers = registered_solvers();
        let mut names: Vec<&str> = solvers.iter().map(|s| s.name()).collect();
        names.sort_unstable();
        let pre = names.len();
        names.dedup();
        assert_eq!(pre, names.len(), "duplicate solver names");
    }

    #[test]
    fn lookup_finds_each_solver() {
        for s in registered_solvers() {
            assert!(lookup(s.name()).is_some(), "lookup failed for {}", s.name());
        }
    }
}
