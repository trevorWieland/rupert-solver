//! Derivative-free black-box optimization helpers.
//!
//! This module provides the analogue of Tom 7's `cc-lib::Opt::Minimize`
//! we promised in `imperts.rs`'s v1 deviation note. The two main
//! exports:
//!
//! - [`nelder_mead_box`] — single-start Nelder-Mead simplex search,
//!   constrained to a box via clamp-on-step. ~150 LOC.
//! - [`opt_minimize`] — multi-restart wrapper. Each restart picks a
//!   random anchor in the box and runs `nelder_mead_box`. Returns the
//!   best `(x, f(x))` across all restarts.
//!
//! Both functions take `f: FnMut(&[f64]) -> f64` and minimize. Maximize
//! by passing `-f` (or wrap with `|x| -inner(x)`).
//!
//! ## Algorithmic choices
//!
//! - **Nelder-Mead** rather than (1+1) ES, BFGS-FD, or BiteOpt because
//!   we need: (a) gradient-free (most of our objectives have non-smooth
//!   active-set transitions in the convex-hull pipeline), (b) low-dim
//!   (≤ 12 in practice), and (c) small constant memory. Nelder-Mead is
//!   the textbook fit. Tom's `Opt::Minimize` is a more sophisticated
//!   branch-and-restart DFO; this is the closest in-tree replacement
//!   without porting his cc-lib.
//! - **Box clamping** rather than barrier penalties: we project each
//!   trial point back into the box. Loses Nelder-Mead's exact
//!   convergence guarantees on the boundary but is robust and simple.
//! - **Multi-restart** with uniform random initial anchors. Captures
//!   the multi-restart spirit of upstream's `attempts=100` parameter.

use rand::Rng;

const NM_REFLECTION: f64 = 1.0;
const NM_EXPANSION: f64 = 2.0;
const NM_CONTRACTION: f64 = 0.5;
const NM_SHRINK: f64 = 0.5;
const NM_INIT_STEP_FRAC: f64 = 0.1;

/// Single-start Nelder-Mead with box constraints.
///
/// The initial simplex is built around `start` with spread proportional
/// to the box size. All trial points are clamped into `[lb, ub]`.
///
/// Returns `(best_x, best_f)` after `budget` evaluations or convergence
/// (simplex diameter below `1e-12 * box_diag`).
pub fn nelder_mead_box<F>(
    mut f: F,
    start: &[f64],
    lb: &[f64],
    ub: &[f64],
    budget: usize,
) -> (Vec<f64>, f64)
where
    F: FnMut(&[f64]) -> f64,
{
    let n = start.len();
    debug_assert_eq!(n, lb.len());
    debug_assert_eq!(n, ub.len());

    // Build initial simplex: start + n perturbed points.
    let mut simplex: Vec<Vec<f64>> = Vec::with_capacity(n + 1);
    simplex.push(clamp(start, lb, ub));
    for d in 0..n {
        let span = ub[d] - lb[d];
        let mut p = simplex[0].clone();
        p[d] = (p[d] + NM_INIT_STEP_FRAC * span).min(ub[d]);
        if p[d] == simplex[0][d] {
            // Already at upper bound; perturb downward.
            p[d] = (simplex[0][d] - NM_INIT_STEP_FRAC * span).max(lb[d]);
        }
        simplex.push(p);
    }

    let mut values: Vec<f64> = simplex.iter().map(|p| f(p)).collect();
    let mut evals = simplex.len();

    while evals < budget {
        sort_simplex(&mut simplex, &mut values);

        // Convergence: simplex diameter below tolerance.
        if simplex_diameter(&simplex) < 1e-12 {
            break;
        }

        let centroid = centroid_of_best(&simplex);
        let worst_idx = n;
        let worst = simplex[worst_idx].clone();
        let worst_val = values[worst_idx];

        // Reflection.
        let reflected = clamp(&combine(&centroid, &worst, NM_REFLECTION), lb, ub);
        let reflected_val = f(&reflected);
        evals += 1;

        if reflected_val < values[0] {
            // Expansion.
            if evals >= budget {
                if reflected_val < worst_val {
                    simplex[worst_idx] = reflected;
                    values[worst_idx] = reflected_val;
                }
                break;
            }
            let expanded = clamp(&combine(&centroid, &reflected, NM_EXPANSION - 1.0), lb, ub);
            let expanded_val = f(&expanded);
            evals += 1;
            if expanded_val < reflected_val {
                simplex[worst_idx] = expanded;
                values[worst_idx] = expanded_val;
            } else {
                simplex[worst_idx] = reflected;
                values[worst_idx] = reflected_val;
            }
        } else if reflected_val < values[n - 1] {
            simplex[worst_idx] = reflected;
            values[worst_idx] = reflected_val;
        } else {
            // Contraction.
            let contracted = clamp(&combine(&centroid, &worst, -NM_CONTRACTION), lb, ub);
            let contracted_val = f(&contracted);
            evals += 1;
            if contracted_val < worst_val {
                simplex[worst_idx] = contracted;
                values[worst_idx] = contracted_val;
            } else {
                // Shrink toward best.
                let best = simplex[0].clone();
                for i in 1..=n {
                    if evals >= budget {
                        break;
                    }
                    for d in 0..n {
                        simplex[i][d] = best[d] + NM_SHRINK * (simplex[i][d] - best[d]);
                    }
                    simplex[i] = clamp(&simplex[i], lb, ub);
                    values[i] = f(&simplex[i]);
                    evals += 1;
                }
            }
        }
    }
    sort_simplex(&mut simplex, &mut values);
    (simplex[0].clone(), values[0])
}

/// Multi-restart wrapper. The closest in-tree replacement for
/// `cc-lib::Opt::Minimize<N>(f, lb, ub, iters, depth, attempts, seed)`.
/// Allocates `budget / attempts` evals per restart.
pub fn opt_minimize<F, R>(
    mut f: F,
    lb: &[f64],
    ub: &[f64],
    budget: usize,
    attempts: usize,
    rng: &mut R,
) -> (Vec<f64>, f64)
where
    F: FnMut(&[f64]) -> f64,
    R: Rng + ?Sized,
{
    let n = lb.len();
    let per_attempt = budget.max(attempts) / attempts.max(1);
    let mut best: Option<(Vec<f64>, f64)> = None;
    for _ in 0..attempts {
        let start: Vec<f64> = (0..n).map(|d| rng.gen_range(lb[d]..ub[d])).collect();
        let (x, y) = nelder_mead_box(&mut f, &start, lb, ub, per_attempt);
        match best.as_ref() {
            None => best = Some((x, y)),
            Some((_, by)) if y < *by => best = Some((x, y)),
            _ => {}
        }
    }
    best.unwrap_or_else(|| (vec![0.0; n], f64::INFINITY))
}

fn clamp(p: &[f64], lb: &[f64], ub: &[f64]) -> Vec<f64> {
    p.iter()
        .zip(lb.iter().zip(ub.iter()))
        .map(|(v, (l, u))| v.max(*l).min(*u))
        .collect()
}

fn sort_simplex(simplex: &mut [Vec<f64>], values: &mut [f64]) {
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

fn simplex_diameter(simplex: &[Vec<f64>]) -> f64 {
    let mut max_sq = 0.0_f64;
    for i in 0..simplex.len() {
        for j in (i + 1)..simplex.len() {
            let d_sq: f64 = simplex[i]
                .iter()
                .zip(simplex[j].iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum();
            if d_sq > max_sq {
                max_sq = d_sq;
            }
        }
    }
    max_sq.sqrt()
}

fn centroid_of_best(simplex: &[Vec<f64>]) -> Vec<f64> {
    let n = simplex[0].len();
    let mut c = vec![0.0_f64; n];
    let count = simplex.len() - 1;
    for p in &simplex[..count] {
        for d in 0..n {
            c[d] += p[d];
        }
    }
    for v in &mut c {
        *v /= count as f64;
    }
    c
}

/// `out = base + factor * (base - other)`. Used for reflection,
/// expansion, contraction directions.
fn combine(base: &[f64], other: &[f64], factor: f64) -> Vec<f64> {
    base.iter()
        .zip(other.iter())
        .map(|(b, o)| b + factor * (b - o))
        .collect()
}

#[cfg(test)]
mod tests {
    use rand_xoshiro::Xoshiro256PlusPlus;
    use rand_xoshiro::rand_core::SeedableRng;

    use super::*;

    fn rosenbrock(x: &[f64]) -> f64 {
        // 2D Rosenbrock — unique global min at (1, 1) with value 0.
        let a = 1.0_f64;
        let b = 100.0_f64;
        (a - x[0]).powi(2) + b * (x[1] - x[0] * x[0]).powi(2)
    }

    fn quadratic(x: &[f64]) -> f64 {
        // 5D quadratic — unique global min at (0,0,0,0,0).
        x.iter().map(|v| v * v).sum()
    }

    #[test]
    fn nelder_mead_finds_quadratic_minimum() {
        let lb = vec![-10.0; 5];
        let ub = vec![10.0; 5];
        let start = vec![3.0, -2.0, 1.5, -0.5, 4.0];
        let (x, y) = nelder_mead_box(quadratic, &start, &lb, &ub, 5_000);
        assert!(y < 1e-8, "got y={y} x={x:?}");
        for v in &x {
            assert!(v.abs() < 1e-3, "x = {x:?}");
        }
    }

    #[test]
    fn nelder_mead_clamps_to_box() {
        let lb = vec![0.5, 0.5, 0.5];
        let ub = vec![1.0, 1.0, 1.0];
        let start = vec![0.6, 0.6, 0.6];
        // Quadratic min at origin is OUTSIDE the box; the constrained
        // minimum is at (0.5, 0.5, 0.5).
        let (x, _) = nelder_mead_box(quadratic, &start, &lb, &ub, 1_000);
        for v in &x {
            assert!(*v >= lb[0] - 1e-9 && *v <= ub[0] + 1e-9);
        }
    }

    #[test]
    fn opt_minimize_finds_rosenbrock_basin() {
        let lb = vec![-3.0, -3.0];
        let ub = vec![3.0, 3.0];
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(7);
        let (x, y) = opt_minimize(rosenbrock, &lb, &ub, 20_000, 8, &mut rng);
        // Rosenbrock has a narrow curved basin; we want to land near the
        // global min (1, 1) but allow some slack — Nelder-Mead in 2D
        // typically reaches the basin floor within 20k evals.
        assert!(y < 0.05, "got y={y} x={x:?}");
    }

    #[test]
    fn opt_minimize_deterministic_for_same_seed() {
        let lb = vec![-1.0, -1.0, -1.0];
        let ub = vec![1.0, 1.0, 1.0];
        let mut rng_a = Xoshiro256PlusPlus::seed_from_u64(42);
        let mut rng_b = Xoshiro256PlusPlus::seed_from_u64(42);
        let (xa, ya) = opt_minimize(quadratic, &lb, &ub, 1_000, 4, &mut rng_a);
        let (xb, yb) = opt_minimize(quadratic, &lb, &ub, 1_000, 4, &mut rng_b);
        assert_eq!(ya, yb);
        assert_eq!(xa, xb);
    }
}
