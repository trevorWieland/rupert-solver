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
//! ## v0.3.0 path
//!
//! - Branch-and-bound across patches via deterministic support-width
//!   upper bounds and quaternion subcell subdivision.
//! - Adaptive per-pair budgeting runs a shallow pass over ranked cells,
//!   then reallocates remaining budget to the top positive/near-miss cells.
//! - Cell-walking SO(3) enumeration.
//! - Plug `IntervalSnap` certification for patch_aware-found candidates.

mod bounds;
mod cell_record;
mod fallback;
mod optimize;
mod table;

use rupert_core::{
    Budget, EvalCounter, Polyhedron, Solution, Solver, SolverOutcome, SolverTelemetry,
};

#[derive(Debug, Default)]
pub struct PatchAware;

impl Solver for PatchAware {
    fn name(&self) -> &'static str {
        "patch_aware"
    }

    fn version(&self) -> &'static str {
        "0.3.0"
    }

    fn solve(
        &mut self,
        poly: &Polyhedron,
        budget: &Budget,
        ec: &mut EvalCounter<'_>,
    ) -> SolverOutcome {
        let max = budget.max_evaluations.get();
        if poly.faces.is_empty() {
            return fallback::single_cell_fallback(budget, ec, max);
        }

        let table = table::patch_table_for(poly);
        if table.canonical.is_empty() {
            return SolverOutcome::exhausted();
        }
        let result = optimize::scan_cells(&table, ec, budget, max);
        let telemetry = Some(SolverTelemetry::PatchAware(result.telemetry));
        if result.best_clearance > 0.0 {
            SolverOutcome::Found {
                solution: Solution {
                    candidate: result.best_candidate,
                    clearance: result.best_clearance,
                    found_at_eval: ec.count(),
                    certification: None,
                },
                telemetry,
            }
        } else {
            SolverOutcome::Exhausted { telemetry }
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
            matches!(outcome, SolverOutcome::Found { .. }),
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
    fn emits_full_cell_and_bound_telemetry() {
        let p = rupert_shapes::cube();
        let mut solver = PatchAware;
        let mut ec = EvalCounter::new(&p);
        let outcome = solver.solve(&p, &budget(5_000, 1), &mut ec);
        let (SolverOutcome::Found {
            telemetry: Some(SolverTelemetry::PatchAware(telemetry)),
            ..
        }
        | SolverOutcome::Exhausted {
            telemetry: Some(SolverTelemetry::PatchAware(telemetry)),
        }) = outcome
        else {
            unreachable!("patch-aware telemetry expected");
        };
        assert_eq!(
            telemetry.cell_summaries.len(),
            telemetry.recon_cells_evaluated
        );
        assert!(telemetry.bound_cells_evaluated > 0);
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
        assert!(matches!(outcome, SolverOutcome::Found { .. }));
    }
}
