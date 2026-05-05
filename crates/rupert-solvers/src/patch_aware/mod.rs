//! Patch-decomposition solver — Tom 7's SIGBOVIK 2025 idea, lifted to
//! Rust and adapted for our harness contract.
//!
//! ## What a patch is
//!
//! A "patch" is a connected open region of `SO(3)` such that the
//! combinatorial **face-front/back assignment** is constant. As we
//! continuously rotate the polyhedron, this assignment changes only
//! when a face becomes exactly parallel to the projection axis (z) —
//! at which moment the bit flips. The intersection of the F surfaces
//! `{ q : face_i.normal · ẑ_rotated = 0 }` partitions SO(3) into
//! `O(F²)` open patches.
//!
//! Within a patch, the silhouette has a *fixed* set of front-facing
//! and back-facing faces, the silhouette edges are fixed, and the
//! clearance objective is **smooth** in the rotation parameters. So
//! standard local optimization (Nelder-Mead) converges cleanly within
//! a patch, and the missing piece for cracking the snub cube has been
//! "the unit of work needs to be a patch, not a global rotation."
//!
//! ## v0.1.0 algorithm
//!
//! 1. **Face normals** via Newell's method per face.
//! 2. **Patch enumeration** by Shoemake-uniform random sampling of
//!    SO(3), keyed by `u128` face-sign-vector. Stagnation-based stop.
//! 3. **Symmetry orbit reduction** via the per-shape rotation group.
//! 4. **Within-patch Nelder-Mead** in the 10-DOF delta box, soft
//!    Hamming-distance penalty for staying in patch.
//! 5. **Cross-patch search**: serial scan of all (outer, inner) cells.
//!
//! ## v0.2.0 path (deliberately deferred)
//!
//! - Branch-and-bound across patches via interval-arithmetic upper
//!   bounds (uses [`rupert_core::hull2d_interval`]).
//! - Adaptive per-pair budget.
//! - Cell-walking SO(3) enumeration.
//! - Plug `IntervalSnap` certification for patch_aware-found candidates.

mod optimize;
mod table;

use rupert_core::{Budget, EvalCounter, Polyhedron, Solution, Solver, SolverOutcome};

#[derive(Debug, Default)]
pub struct PatchAware;

impl Solver for PatchAware {
    fn name(&self) -> &'static str {
        "patch_aware"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn solve(
        &mut self,
        poly: &Polyhedron,
        budget: &Budget,
        ec: &mut EvalCounter<'_>,
    ) -> SolverOutcome {
        let max = budget.max_evaluations.get();
        if poly.faces.is_empty() {
            return optimize::single_cell_fallback(budget, ec, max);
        }

        let table = table::patch_table_for(poly);
        let pairs = table.canonical.len() * table.canonical.len();
        if pairs == 0 {
            return SolverOutcome::Exhausted;
        }
        let per_pair_evals = (max.saturating_mul(9) / 10) / (pairs as u64).max(1);
        if per_pair_evals < 16 {
            return optimize::single_cell_fallback(budget, ec, max);
        }

        let result = optimize::scan_cells(&table, ec, budget, max, per_pair_evals);
        match result {
            Some((clearance, candidate)) if clearance > 0.0 => SolverOutcome::Found(Solution {
                candidate,
                clearance,
                found_at_eval: ec.count(),
                certification: None,
            }),
            _ => SolverOutcome::Exhausted,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use rupert_core::EvalCounter;

    use super::*;

    fn budget(max_evals: u64, seed: u64) -> Budget {
        Budget {
            max_evaluations: NonZeroU64::new(max_evals).expect("nonzero"),
            max_wall_time: None,
            seed,
        }
    }

    #[test]
    fn solves_cube() {
        let mut solver = PatchAware;
        let p = rupert_shapes::cube();
        let mut ec = EvalCounter::new(&p);
        let outcome = solver.solve(&p, &budget(150_000, 0), &mut ec);
        assert!(
            matches!(outcome, SolverOutcome::Found(_)),
            "got {outcome:?}"
        );
    }

    #[test]
    fn deterministic_for_same_seed() {
        let p = rupert_shapes::cube();
        let mut a = PatchAware;
        let mut b = PatchAware;
        let mut ec_a = EvalCounter::new(&p);
        let mut ec_b = EvalCounter::new(&p);
        let _ = a.solve(&p, &budget(50_000, 9), &mut ec_a);
        let _ = b.solve(&p, &budget(50_000, 9), &mut ec_b);
        assert_eq!(ec_a.count(), ec_b.count());
    }

    #[test]
    fn shape_with_empty_faces_falls_back() {
        // Custom user-supplied empty-face polyhedron should still solve
        // via the single-cell fallback.
        let mut p = rupert_shapes::cube();
        p.faces.clear();
        let mut solver = PatchAware;
        let mut ec = EvalCounter::new(&p);
        let outcome = solver.solve(&p, &budget(150_000, 0), &mut ec);
        assert!(matches!(outcome, SolverOutcome::Found(_)));
    }
}
