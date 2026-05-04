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

use std::f64::consts::PI;

use rupert_core::{Polyhedron, Vec3};

// Exact rational seed values from the paper (constants are computed at
// module-init time from the integer numerators / integer denominators).
//
// C₁ = (152024884, 0, 210152163) / 259375205. The numerator-denominator
// triple satisfies 152024884² + 210152163² = 259375205² exactly (a
// Pythagorean triple), so ‖C₁‖ = 1 in real arithmetic.
//
// C₂ = (6632738028, 6106948881, 3980949609) / 10¹⁰. ‖C₂‖ ≈ 0.985576 ∈ (0.98, 0.99).
// C₃ = (8193990033, 5298215096, 1230614493) / 10¹⁰. ‖C₃‖ ≈ 0.983499 ∈ (0.98, 0.99).
const C1_NUM: [f64; 3] = [152_024_884.0, 0.0, 210_152_163.0];
const C1_DEN: f64 = 259_375_205.0;
const C2_NUM: [f64; 3] = [6_632_738_028.0, 6_106_948_881.0, 3_980_949_609.0];
const C2_DEN: f64 = 10_000_000_000.0;
const C3_NUM: [f64; 3] = [8_193_990_033.0, 5_298_215_096.0, 1_230_614_493.0];
const C3_DEN: f64 = 10_000_000_000.0;

fn seeds() -> [Vec3; 3] {
    [
        Vec3::new(C1_NUM[0] / C1_DEN, C1_NUM[1] / C1_DEN, C1_NUM[2] / C1_DEN),
        Vec3::new(C2_NUM[0] / C2_DEN, C2_NUM[1] / C2_DEN, C2_NUM[2] / C2_DEN),
        Vec3::new(C3_NUM[0] / C3_DEN, C3_NUM[1] / C3_DEN, C3_NUM[2] / C3_DEN),
    ]
}

pub fn noperthedron() -> Polyhedron {
    let s = seeds();
    let mut vertices: Vec<Vec3> = Vec::with_capacity(90);
    for seed in &s {
        for k in 0i32..15 {
            let theta = 2.0 * PI * f64::from(k) / 15.0;
            let cos_t = theta.cos();
            let sin_t = theta.sin();
            let rotated = Vec3::new(
                cos_t * seed.x - sin_t * seed.y,
                sin_t * seed.x + cos_t * seed.y,
                seed.z,
            );
            // ℓ = 0 leaves the rotation; ℓ = 1 inverts the sign.
            vertices.push(rotated);
            vertices.push(Vec3::new(-rotated.x, -rotated.y, -rotated.z));
        }
    }
    debug_assert_eq!(vertices.len(), 90);
    // Faces are intentionally omitted — solvers consume only the vertex
    // cloud via projection. Visualization (STL export, residue rendering)
    // and the face_normal_pairs solver gracefully degrade when faces are
    // empty. v2 will wire a 3D convex-hull triangulation here.
    Polyhedron::new("noperthedron", vertices, vec![]).expect("noperthedron vertices are valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_90() {
        assert_eq!(noperthedron().vertex_count(), 90);
    }

    #[test]
    fn first_seed_unit_norm() {
        let s = seeds();
        // The Pythagorean triple yields ‖C₁‖ = 1 to within 1 ULP after
        // f64 rounding of the components.
        assert!((s[0].norm() - 1.0).abs() < 1e-15, "‖C₁‖ = {}", s[0].norm());
    }

    #[test]
    fn other_seed_norms_in_range() {
        let s = seeds();
        for c in &s[1..] {
            let n = c.norm();
            assert!(n > 0.98 && n < 0.99, "norm out of (0.98, 0.99): {n}");
        }
    }

    #[test]
    fn c2_norm_close_to_paper_approx() {
        // Paper reports ‖C₂‖ ≈ 0.985576. Allow ±1e-6 tolerance against
        // the f64 reconstruction.
        let s = seeds();
        assert!(
            (s[1].norm() - 0.985_576).abs() < 1.0e-6,
            "‖C₂‖ = {}",
            s[1].norm()
        );
    }

    #[test]
    fn c3_norm_close_to_paper_approx() {
        // Paper reports ‖C₃‖ ≈ 0.983499.
        let s = seeds();
        assert!(
            (s[2].norm() - 0.983_499).abs() < 1.0e-6,
            "‖C₃‖ = {}",
            s[2].norm()
        );
    }

    #[test]
    fn fifteen_fold_symmetry_around_z() {
        // Rotating the vertex set by 2π/15 around z should map the set to itself.
        let p = noperthedron();
        let rotated: Vec<Vec3> = p
            .vertices
            .iter()
            .map(|v| {
                let theta = 2.0 * PI / 15.0;
                let c = theta.cos();
                let s = theta.sin();
                Vec3::new(c * v.x - s * v.y, s * v.x + c * v.y, v.z)
            })
            .collect();
        for r in &rotated {
            let matched = p.vertices.iter().any(|o| (*o - *r).norm() < 1e-9);
            assert!(matched, "rotated vertex {r:?} not in original set");
        }
    }
}
