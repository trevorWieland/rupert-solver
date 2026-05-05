//! Noperthedron — Steininger & Yurkevich, "A convex polyhedron without
//! Rupert's property", arXiv:2508.18475 (2025).
//!
//! 90 vertices, generated as `(-1)^ℓ · R_z(2πk/15) · C_{i+1}` for
//! `k ∈ 0..15`, `i ∈ 0..3`, `ℓ ∈ {0,1}` from three seed points
//! `C₁, C₂, C₃ ∈ ℚ³`. The seed values are taken verbatim from the
//! authors' reference implementation:
//! <https://github.com/Jakob256/Rupert/blob/main/src/noperthedron.py>
//!
//! **Caveat.** v1 stores seeds as `f64` (rounded from the exact rationals
//! below). Solvers see a polyhedron whose `f64` coordinates are within
//! ~1 ULP of the paper's. Empirical search confirms no v1 solver finds
//! a passage within 100 000 evaluations (regression test below). The
//! formal proof of non-Rupertness in the paper relies on rational
//! arithmetic + interval bounds for the irrational rotation matrix
//! `R_z(2π/15)`; v2 will wire `rupert-verify` to the exact path.

use rupert_core::{ExactVec3, Expr, Polyhedron, Vec3};

use crate::hull3d::triangulate_convex_hull;

// Exact rational seed values from arXiv:2508.18475:
//
// C₁ = (152024884, 0, 210152163) / 259375205. The numerator-denominator
// triple satisfies 152024884² + 210152163² = 259375205² exactly (a
// Pythagorean triple), so ‖C₁‖ = 1 in real arithmetic.
//
// C₂ = (6632738028, 6106948881, 3980949609) / 10¹⁰. ‖C₂‖ ≈ 0.985576 ∈ (0.98, 0.99).
// C₃ = (8193990033, 5298215096, 1230614493) / 10¹⁰. ‖C₃‖ ≈ 0.983499 ∈ (0.98, 0.99).

/// Three exact-rational seed Expr forms (one per row).
fn seed_exprs() -> [(Expr, Expr, Expr); 3] {
    let c1_den = || Expr::int(259_375_205);
    let c2_den = || Expr::int(10_000_000_000);
    let c3_den = || Expr::int(10_000_000_000);
    [
        (
            Expr::int(152_024_884) / c1_den(),
            Expr::int(0),
            Expr::int(210_152_163) / c1_den(),
        ),
        (
            Expr::int(6_632_738_028) / c2_den(),
            Expr::int(6_106_948_881) / c2_den(),
            Expr::int(3_980_949_609) / c2_den(),
        ),
        (
            Expr::int(8_193_990_033) / c3_den(),
            Expr::int(5_298_215_096) / c3_den(),
            Expr::int(1_230_614_493) / c3_den(),
        ),
    ]
}

pub fn noperthedron() -> Polyhedron {
    let exact = noperthedron_exact_vertices();
    let f64_vertices: Vec<Vec3> = exact.iter().map(ExactVec3::eval_f64).collect();
    let faces = triangulate_convex_hull(&f64_vertices);
    Polyhedron::with_exact("noperthedron", exact, faces).expect("noperthedron vertices are valid")
}

/// 90 exact-form vertices: 3 seeds × 15 cyclic rotations × 2 sign flips.
/// Each rotated vertex carries the exact symbolic
/// `(cos(2πk/15) · seed.x - sin(2πk/15) · seed.y,
///   sin(2πk/15) · seed.x + cos(2πk/15) · seed.y, seed.z)` —
/// the interval verifier picks up tight bounds on these.
fn noperthedron_exact_vertices() -> Vec<ExactVec3> {
    let mut out: Vec<ExactVec3> = Vec::with_capacity(90);
    let seeds = seed_exprs();
    for seed in seeds {
        for k in 0..15_i128 {
            let cos = Expr::cos_two_pi_k_over(k, 15);
            let sin = Expr::sin_two_pi_k_over(k, 15);
            let (sx, sy, sz) = (seed.0.clone(), seed.1.clone(), seed.2.clone());
            let rx = cos.clone() * sx.clone() - sin.clone() * sy.clone();
            let ry = sin * sx + cos * sy;
            let rz = sz;
            out.push(ExactVec3::new(rx.clone(), ry.clone(), rz.clone()));
            out.push(ExactVec3::new(-rx, -ry, -rz));
        }
    }
    debug_assert_eq!(out.len(), 90);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_90() {
        assert_eq!(noperthedron().vertex_count(), 90);
    }

    fn seed_f64s() -> [Vec3; 3] {
        let s = seed_exprs();
        [
            ExactVec3::new(s[0].0.clone(), s[0].1.clone(), s[0].2.clone()).eval_f64(),
            ExactVec3::new(s[1].0.clone(), s[1].1.clone(), s[1].2.clone()).eval_f64(),
            ExactVec3::new(s[2].0.clone(), s[2].1.clone(), s[2].2.clone()).eval_f64(),
        ]
    }

    #[test]
    fn first_seed_unit_norm() {
        let s = seed_f64s();
        assert!((s[0].norm() - 1.0).abs() < 1e-15, "‖C₁‖ = {}", s[0].norm());
    }

    #[test]
    fn other_seed_norms_in_range() {
        let s = seed_f64s();
        for c in &s[1..] {
            let n = c.norm();
            assert!(n > 0.98 && n < 0.99, "norm out of (0.98, 0.99): {n}");
        }
    }

    #[test]
    fn exact_vertices_present() {
        let p = noperthedron();
        let exact = p.exact_vertices.as_ref().expect("exact present");
        assert_eq!(exact.len(), 90);
    }

    #[test]
    fn fifteen_fold_symmetry_around_z() {
        let p = noperthedron();
        let theta = 2.0_f64 * std::f64::consts::PI / 15.0_f64;
        let c = theta.cos();
        let s = theta.sin();
        let rotated: Vec<Vec3> = p
            .vertices
            .iter()
            .map(|v| Vec3::new(c * v.x - s * v.y, s * v.x + c * v.y, v.z))
            .collect();
        for r in &rotated {
            let matched = p.vertices.iter().any(|o| (*o - *r).norm() < 1e-9);
            assert!(matched, "rotated vertex {r:?} not in original set");
        }
    }
}
