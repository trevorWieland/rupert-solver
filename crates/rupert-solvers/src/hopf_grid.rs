//! Deterministic SO(3) grid via Hopf fibration sampling.
//!
//! The Hopf fibration maps `S² × S¹ → SO(3)`. Sample S² as a Fibonacci
//! lattice and S¹ as `n` equally spaced angles; each (point on S²,
//! angle on S¹) gives a unit quaternion. We enumerate the Cartesian
//! product for both inner and outer copies.

use std::f64::consts::{PI, TAU};

use rupert_core::{
    Budget, Candidate, EvalCounter, Polyhedron, Quat, Solution, Solver, SolverOutcome,
};

const RESOLUTION_S2: usize = 32;
const RESOLUTION_S1: usize = 8;

#[derive(Debug, Default)]
pub struct HopfGrid;

impl Solver for HopfGrid {
    fn name(&self) -> &'static str {
        "hopf_grid"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn solve(
        &mut self,
        _poly: &Polyhedron,
        budget: &Budget,
        ec: &mut EvalCounter<'_>,
    ) -> SolverOutcome {
        let max = budget.max_evaluations.get();
        let grid = build_grid(RESOLUTION_S2, RESOLUTION_S1);
        for outer_q in &grid {
            for inner_q in &grid {
                if ec.count() >= max {
                    return SolverOutcome::Exhausted;
                }
                let candidate = Candidate {
                    outer: *outer_q,
                    inner: *inner_q,
                    translation: [0.0, 0.0],
                };
                let c = ec.evaluate(&candidate);
                if c.is_finite() && c > 0.0 {
                    return SolverOutcome::Found(Solution {
                        candidate,
                        clearance: c,
                        found_at_eval: ec.count(),
                        certification: None,
                    });
                }
            }
        }
        SolverOutcome::Exhausted
    }
}

fn build_grid(n_s2: usize, n_s1: usize) -> Vec<Quat> {
    // Fibonacci sphere on S². For each fiber base point, sample n_s1
    // rotations around the fiber.
    let golden = f64::midpoint(1.0, 5.0_f64.sqrt());
    let mut out: Vec<Quat> = Vec::with_capacity(n_s2 * n_s1);
    for i in 0..n_s2 {
        let z = 1.0 - 2.0 * (i as f64 + 0.5) / (n_s2 as f64);
        let r = (1.0 - z * z).max(0.0).sqrt();
        let theta = TAU * (i as f64) / golden;
        let nx = r * theta.cos();
        let ny = r * theta.sin();
        let nz = z;
        // Convert (nx, ny, nz) on S² to a base quaternion via the
        // Hopf chart: the half-rotation that takes +z to (nx, ny, nz).
        let base = base_from_normal(nx, ny, nz);
        for j in 0..n_s1 {
            let phi = PI * (j as f64) / (n_s1 as f64);
            // Twist around the +z fiber by phi (multiplied on the right).
            let twist = Quat::from_axis_angle(rupert_core::Vec3::Z, 2.0 * phi);
            out.push((base * twist).normalized());
        }
    }
    out
}

fn base_from_normal(nx: f64, ny: f64, nz: f64) -> Quat {
    // Quaternion that takes +z → (nx, ny, nz). Same construction as
    // face_normal_pairs::align_to_plus_z but in the inverse direction.
    let dot = nz; // dot((+z), (nx, ny, nz)) = nz.
    if dot > 1.0 - 1e-12 {
        return Quat::IDENTITY;
    }
    if dot < -1.0 + 1e-12 {
        return Quat::from_axis_angle(rupert_core::Vec3::X, PI);
    }
    let axis = rupert_core::Vec3::Z.cross(rupert_core::Vec3::new(nx, ny, nz));
    let angle = dot.acos();
    Quat::from_axis_angle(axis.normalized(), angle)
}

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
    fn grid_size_matches_resolutions() {
        let g = build_grid(8, 4);
        assert_eq!(g.len(), 32);
    }

    #[test]
    fn finds_cube_passage() {
        let mut solver = HopfGrid;
        let p = rupert_shapes::cube();
        let mut ec = EvalCounter::new(&p);
        // 256 grid points × 256 = 65 536 candidates. Cube is reachable far
        // earlier; budget at 10k.
        let outcome = solver.solve(&p, &budget(10_000, 0), &mut ec);
        assert!(matches!(outcome, SolverOutcome::Found(_)), "got {outcome:?}");
    }
}
