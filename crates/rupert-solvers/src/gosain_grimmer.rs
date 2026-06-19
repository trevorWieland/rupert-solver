//! Ported from Gosain & Grimmer, "Some New Insights from Highly Optimized
//! Polyhedral Passages", arXiv:2509.08190 (Sep 2025).
//!
//! Original source: <https://github.com/RajGosain13/RupertResults>
//! (Python + Jupyter; primary file `PolyhedronTest.ipynb`).
//! License: not declared upstream — algorithm reimplemented from the paper.
//!
//! Notable adaptations from the upstream source:
//!
//! - The paper's Nieuwland-scale objective `μ(x) = min{ 1/(wⱼᵀvᵢ) : wⱼᵀvᵢ > 0 }`
//!   is monotonically related to our [`rupert_core`] signed-clearance metric,
//!   so this port maximizes signed clearance directly via [`EvalCounter`] —
//!   keeps integration with the harness honest (every objective evaluation
//!   is counted) and avoids reimplementing the projection-hull pipeline.
//! - Gradients are computed by finite difference (~8 evaluations per
//!   gradient step) instead of symbolic differentiation. The paper's
//!   `sympy`-based exact gradients are a v2 optimization.
//! - The trust-region linearized-min-of-smooth direction (paper Eq. 2.3,
//!   `s_k = argmax_{‖s‖≤δ} min_{i,j} { μ_{i,j}(x_k) + ∇μ_{i,j}(x_k)ᵀ s }`)
//!   is replaced by plain steepest ascent on the clearance gradient with
//!   backtracking line search. The min-norm-over-ε-active-constraints
//!   variant is the v2 fanciness.
//! - 7-parameter parametrization `x = (u, v, θ_p, φ_p, α, θ_q, φ_q)`
//!   (paper form). The upstream code carries an 8th (gauge-redundant)
//!   angle; we drop it.

use std::f64::consts::{PI, TAU};

use rand::Rng;
use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use rupert_core::{
    Budget, Candidate, EvalCounter, Polyhedron, Quat, Solution, Solver, SolverOutcome, Vec3,
};

#[derive(Debug, Default)]
pub struct GosainGrimmer;

const DIM: usize = 7;
// Parameter indices:
//   0: u  (translation x)
//   1: v  (translation y)
//   2: θ_p (inner view inclination, radians)
//   3: φ_p (inner view azimuth, radians)
//   4: α   (inner in-plane rotation around z, radians)
//   5: θ_q (outer view inclination, radians)
//   6: φ_q (outer view azimuth, radians)

/// Finite-difference step size. Smaller is more accurate but more
/// hull-membership-flip-prone; the paper's symbolic gradients sidestep
/// this entirely.
const FD_STEP: f64 = 1.0e-6;
const ALPHA_INIT: f64 = 0.1;
const ALPHA_GROW: f64 = 2.0;
const ALPHA_SHRINK: f64 = 0.5;
const ALPHA_MIN: f64 = 1.0e-10;
const LS_MAX_HALVINGS: usize = 25;
const ITERS_PER_RESTART: usize = 200;

impl Solver for GosainGrimmer {
    fn name(&self) -> &'static str {
        "gosain_grimmer"
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
        loop {
            if ec.count() >= max {
                return SolverOutcome::exhausted();
            }
            let x_init = random_init(&mut rng);
            if let Some(sol) = run_one_restart(ec, x_init, max) {
                return SolverOutcome::found(sol);
            }
        }
    }
}

fn random_init<R: Rng + ?Sized>(rng: &mut R) -> [f64; DIM] {
    [
        rng.gen_range(-0.1..0.1),
        rng.gen_range(-0.1..0.1),
        rng.gen_range(0.0..PI),
        rng.gen_range(0.0..TAU),
        rng.gen_range(0.0..TAU),
        rng.gen_range(0.0..PI),
        rng.gen_range(0.0..TAU),
    ]
}

fn x_to_candidate(x: &[f64; DIM]) -> Candidate {
    Candidate {
        outer: align_view(x[5], x[6], 0.0),
        inner: align_view(x[2], x[3], x[4]),
        translation: [x[0], x[1]],
    }
}

/// Build a unit quaternion from spherical view direction (θ, φ) and an
/// optional in-plane rotation α around z.
fn align_view(theta: f64, phi: f64, alpha: f64) -> Quat {
    let dir = Vec3::new(
        theta.sin() * phi.cos(),
        theta.sin() * phi.sin(),
        theta.cos(),
    );
    let q_view = if dir.z > 1.0 - 1.0e-12 {
        Quat::IDENTITY
    } else if dir.z < -1.0 + 1.0e-12 {
        Quat::from_axis_angle(Vec3::X, PI)
    } else {
        let axis = dir.cross(Vec3::Z).normalized();
        let angle = dir.z.acos();
        Quat::from_axis_angle(axis, angle)
    };
    let twist = Quat::from_axis_angle(Vec3::Z, alpha);
    (twist * q_view).normalized()
}

fn run_one_restart(
    ec: &mut EvalCounter<'_>,
    x_init: [f64; DIM],
    max_evals: u64,
) -> Option<Solution> {
    let mut x = x_init;
    let mut alpha = ALPHA_INIT;
    let mut current = ec.evaluate(&x_to_candidate(&x));
    if current.is_finite() && current > 0.0 {
        return Some(make_solution(x, current, ec.count()));
    }

    for _ in 0..ITERS_PER_RESTART {
        if ec.count() >= max_evals {
            return None;
        }
        let grad = finite_diff_grad(ec, &x, current, max_evals)?;
        let gnorm: f64 = grad.iter().map(|g| g * g).sum::<f64>().sqrt();
        if gnorm < 1.0e-12 {
            return None;
        }
        let dir: [f64; DIM] = std::array::from_fn(|i| grad[i] / gnorm);
        let next = backtrack(ec, &x, current, &dir, &mut alpha, max_evals)?;
        if next.0.is_finite() && next.0 > 0.0 {
            return Some(make_solution(next.1, next.0, ec.count()));
        }
        if next.0 <= current {
            return None;
        }
        current = next.0;
        x = next.1;
        if alpha < ALPHA_MIN {
            return None;
        }
    }
    None
}

fn finite_diff_grad(
    ec: &mut EvalCounter<'_>,
    x: &[f64; DIM],
    at_clearance: f64,
    max_evals: u64,
) -> Option<[f64; DIM]> {
    let mut grad = [0.0_f64; DIM];
    for d in 0..DIM {
        if ec.count() >= max_evals {
            return None;
        }
        let mut x_plus = *x;
        x_plus[d] += FD_STEP;
        let plus = ec.evaluate(&x_to_candidate(&x_plus));
        if !plus.is_finite() {
            return None;
        }
        grad[d] = (plus - at_clearance) / FD_STEP;
    }
    Some(grad)
}

/// Backtracking line search. Returns `(new_clearance, new_x)` on success.
/// Mutates `alpha` (caller carries state across iterations).
fn backtrack(
    ec: &mut EvalCounter<'_>,
    x: &[f64; DIM],
    at_clearance: f64,
    dir: &[f64; DIM],
    alpha: &mut f64,
    max_evals: u64,
) -> Option<(f64, [f64; DIM])> {
    *alpha *= ALPHA_GROW;
    for _ in 0..LS_MAX_HALVINGS {
        if ec.count() >= max_evals {
            return None;
        }
        let trial: [f64; DIM] = std::array::from_fn(|i| x[i] + *alpha * dir[i]);
        let trial_clearance = ec.evaluate(&x_to_candidate(&trial));
        if trial_clearance.is_finite() && trial_clearance > at_clearance {
            return Some((trial_clearance, trial));
        }
        *alpha *= ALPHA_SHRINK;
        if *alpha < ALPHA_MIN {
            return None;
        }
    }
    None
}

fn make_solution(x: [f64; DIM], clearance: f64, eval_count: u64) -> Solution {
    Solution {
        candidate: x_to_candidate(&x),
        clearance,
        found_at_eval: eval_count,
        certification: None,
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
    fn solves_cube_within_budget() {
        let p = rupert_shapes::cube();
        let mut hits = 0;
        let trials: u64 = 25;
        for seed in 0..trials {
            let mut solver = GosainGrimmer;
            let mut ec = EvalCounter::new(&p);
            let outcome = solver.solve(&p, &budget(50_000, seed), &mut ec);
            if matches!(outcome, SolverOutcome::Found { .. }) {
                hits += 1;
            }
        }
        assert!(
            hits >= 20,
            "only {hits}/{trials} cube seeds found a passage"
        );
    }

    #[test]
    fn deterministic_for_same_seed() {
        let p = rupert_shapes::cube();
        let mut a = GosainGrimmer;
        let mut b = GosainGrimmer;
        let mut ec_a = EvalCounter::new(&p);
        let mut ec_b = EvalCounter::new(&p);
        let _ = a.solve(&p, &budget(20_000, 11), &mut ec_a);
        let _ = b.solve(&p, &budget(20_000, 11), &mut ec_b);
        assert_eq!(ec_a.count(), ec_b.count());
    }

    #[test]
    fn align_view_brings_direction_to_plus_z() {
        // align_view(θ, φ, 0) takes the spherical (θ, φ) direction to +z
        // when applied to the original direction.
        let theta = 0.7;
        let phi = 1.2;
        let q = align_view(theta, phi, 0.0);
        let dir = Vec3::new(
            theta.sin() * phi.cos(),
            theta.sin() * phi.sin(),
            theta.cos(),
        );
        let r = q.rotate(dir);
        assert!(r.x.abs() < 1e-12, "x = {}", r.x);
        assert!(r.y.abs() < 1e-12, "y = {}", r.y);
        assert!((r.z - 1.0).abs() < 1e-12, "z = {}", r.z);
    }
}
