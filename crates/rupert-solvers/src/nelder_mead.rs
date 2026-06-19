//! Nelder-Mead local refinement over an 8-dimensional simplex
//! `(outer_xyz_tangent, inner_xyz_tangent, t_x, t_y)` — three-component
//! tangent perturbations to each unit quaternion (composed via small-angle
//! axis-angle rotation), plus the 2D translation.
//!
//! Restarts from a random base point if seeded standalone; or from a
//! fixed base point when used as the refinement leg of `random_then_refine`.

use rand::Rng;
use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use rupert_core::{
    Budget, Candidate, EvalCounter, Polyhedron, Quat, Solution, Solver, SolverOutcome, Vec3,
};

use crate::sample::random_unit_quat;

const DIM: usize = 8;

#[derive(Debug, Default)]
pub struct NelderMead;

impl Solver for NelderMead {
    fn name(&self) -> &'static str {
        "nelder_mead"
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
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(budget.seed);
        let max = budget.max_evaluations.get();
        // Outer loop: random restarts. Each restart anchors on a random
        // (outer, inner) pair, then perturbs in a tangent simplex.
        loop {
            if ec.count() >= max {
                return SolverOutcome::exhausted();
            }
            let base_outer = random_unit_quat(&mut rng);
            let base_inner = random_unit_quat(&mut rng);
            let outcome = run_simplex(&mut rng, ec, base_outer, base_inner, max);
            if let Some(s) = outcome {
                return SolverOutcome::found(s);
            }
        }
    }
}

/// Run one Nelder-Mead simplex pass anchored on `(base_outer, base_inner)`.
/// Returns `Some(Solution)` if a positive-clearance candidate is found.
fn run_simplex<R: Rng + ?Sized>(
    rng: &mut R,
    ec: &mut EvalCounter<'_>,
    base_outer: Quat,
    base_inner: Quat,
    max_evals: u64,
) -> Option<Solution> {
    let initial_step = 0.2_f64;
    let mut simplex: Vec<[f64; DIM]> = Vec::with_capacity(DIM + 1);
    simplex.push([0.0; DIM]);
    for d in 0..DIM {
        let mut p = [0.0; DIM];
        p[d] = initial_step * (1.0 + 0.1 * rng.gen_range(-1.0..1.0));
        simplex.push(p);
    }
    let mut values: Vec<f64> = simplex
        .iter()
        .map(|p| -evaluate_at(ec, base_outer, base_inner, p))
        .collect();
    if let Some(s) = check_positive(&simplex, &values, ec.count(), base_outer, base_inner) {
        return Some(s);
    }

    // Limit simplex iterations per restart to keep the outer loop responsive.
    let iters_per_restart = 200;
    for _ in 0..iters_per_restart {
        if ec.count() >= max_evals {
            return None;
        }
        sort_simplex(&mut simplex, &mut values);
        if let Some(s) = check_positive(&simplex, &values, ec.count(), base_outer, base_inner) {
            return Some(s);
        }
        let centroid = centroid_of_best(&simplex);
        // Reflection.
        let reflected = reflect(&centroid, &simplex[DIM]);
        let reflected_v = -evaluate_at(ec, base_outer, base_inner, &reflected);
        if reflected_v < values[0] {
            // Expansion.
            let expanded = expand(&centroid, &reflected);
            let expanded_v = -evaluate_at(ec, base_outer, base_inner, &expanded);
            if expanded_v < reflected_v {
                simplex[DIM] = expanded;
                values[DIM] = expanded_v;
            } else {
                simplex[DIM] = reflected;
                values[DIM] = reflected_v;
            }
        } else if reflected_v < values[DIM - 1] {
            simplex[DIM] = reflected;
            values[DIM] = reflected_v;
        } else {
            // Contraction.
            let contracted = contract(&centroid, &simplex[DIM]);
            let contracted_v = -evaluate_at(ec, base_outer, base_inner, &contracted);
            if contracted_v < values[DIM] {
                simplex[DIM] = contracted;
                values[DIM] = contracted_v;
            } else {
                shrink_simplex(&mut simplex, &mut values, ec, base_outer, base_inner);
            }
        }
    }
    None
}

fn evaluate_at(
    ec: &mut EvalCounter<'_>,
    base_outer: Quat,
    base_inner: Quat,
    params: &[f64; DIM],
) -> f64 {
    let outer = perturb_quat(base_outer, params[0], params[1], params[2]);
    let inner = perturb_quat(base_inner, params[3], params[4], params[5]);
    let candidate = Candidate {
        outer,
        inner,
        translation: [params[6], params[7]],
    };
    ec.evaluate(&candidate)
}

fn perturb_quat(base: Quat, x: f64, y: f64, z: f64) -> Quat {
    let axis = Vec3::new(x, y, z);
    let angle = axis.norm();
    if angle == 0.0 {
        return base;
    }
    let dq = Quat::from_axis_angle(axis * (1.0 / angle), angle);
    (dq * base).normalized()
}

fn check_positive(
    simplex: &[[f64; DIM]],
    values: &[f64],
    eval_count: u64,
    base_outer: Quat,
    base_inner: Quat,
) -> Option<Solution> {
    for (i, &v) in values.iter().enumerate() {
        if v < 0.0 {
            // Clearance is -v (since we negated). Strictly > 0 ⇒ valid.
            let clearance = -v;
            if clearance > 0.0 {
                let p = &simplex[i];
                let outer = perturb_quat(base_outer, p[0], p[1], p[2]);
                let inner = perturb_quat(base_inner, p[3], p[4], p[5]);
                return Some(Solution {
                    candidate: Candidate {
                        outer,
                        inner,
                        translation: [p[6], p[7]],
                    },
                    clearance,
                    found_at_eval: eval_count,
                    certification: None,
                });
            }
        }
    }
    None
}

fn sort_simplex(simplex: &mut [[f64; DIM]], values: &mut [f64]) {
    // Simple selection-sort by `values`, ascending.
    for i in 0..values.len() {
        let mut min_idx = i;
        for j in (i + 1)..values.len() {
            if values[j] < values[min_idx] {
                min_idx = j;
            }
        }
        if min_idx != i {
            values.swap(i, min_idx);
            simplex.swap(i, min_idx);
        }
    }
}

fn centroid_of_best(simplex: &[[f64; DIM]]) -> [f64; DIM] {
    let mut c = [0.0; DIM];
    for p in &simplex[..DIM] {
        for d in 0..DIM {
            c[d] += p[d];
        }
    }
    let n = DIM as f64;
    for v in &mut c {
        *v /= n;
    }
    c
}

fn reflect(centroid: &[f64; DIM], worst: &[f64; DIM]) -> [f64; DIM] {
    let mut out = [0.0; DIM];
    for d in 0..DIM {
        out[d] = centroid[d] + (centroid[d] - worst[d]);
    }
    out
}

fn expand(centroid: &[f64; DIM], reflected: &[f64; DIM]) -> [f64; DIM] {
    let mut out = [0.0; DIM];
    for d in 0..DIM {
        out[d] = centroid[d] + 2.0 * (reflected[d] - centroid[d]);
    }
    out
}

fn contract(centroid: &[f64; DIM], worst: &[f64; DIM]) -> [f64; DIM] {
    let mut out = [0.0; DIM];
    for d in 0..DIM {
        out[d] = centroid[d] + 0.5 * (worst[d] - centroid[d]);
    }
    out
}

fn shrink_simplex(
    simplex: &mut [[f64; DIM]],
    values: &mut [f64],
    ec: &mut EvalCounter<'_>,
    base_outer: Quat,
    base_inner: Quat,
) {
    let best = simplex[0];
    for i in 1..simplex.len() {
        for d in 0..DIM {
            simplex[i][d] = best[d] + 0.5 * (simplex[i][d] - best[d]);
        }
        values[i] = -evaluate_at(ec, base_outer, base_inner, &simplex[i]);
    }
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
    fn finds_cube_passage_eventually() {
        let mut solver = NelderMead;
        let p = rupert_shapes::cube();
        let mut ec = EvalCounter::new(&p);
        let outcome = solver.solve(&p, &budget(50_000, 7), &mut ec);
        assert!(
            matches!(outcome, SolverOutcome::Found { .. }),
            "got {outcome:?}"
        );
    }
}
