//! Regular dodecahedron — vertices over `Expr::GoldenRatio`.
//!
//! v2 phase 2 migration: vertices are stored as exact algebraic
//! expressions; the f64 vertex array is derived via `eval_f64`. The
//! migration is non-breaking — solvers still see the same f64 vertex
//! coordinates (bit-identical to the previous hand-typed `PHI` constant
//! up to f64 rounding) — and unlocks the `IntervalSnap` verifier path
//! once phase 4 lands.

use rupert_core::{ExactVec3, Expr, Polyhedron};

fn vertex_table() -> Vec<ExactVec3> {
    let phi = Expr::golden_ratio;
    let inv_phi = || Expr::int(1) / phi();
    let zero = || Expr::int(0);
    let one = || Expr::int(1);
    let neg_one = || Expr::int(-1);
    let neg_phi = || -phi();
    let neg_inv_phi = || -inv_phi();

    vec![
        // (±1, ±1, ±1)
        ExactVec3::new(one(), one(), one()),
        ExactVec3::new(one(), one(), neg_one()),
        ExactVec3::new(one(), neg_one(), one()),
        ExactVec3::new(one(), neg_one(), neg_one()),
        ExactVec3::new(neg_one(), one(), one()),
        ExactVec3::new(neg_one(), one(), neg_one()),
        ExactVec3::new(neg_one(), neg_one(), one()),
        ExactVec3::new(neg_one(), neg_one(), neg_one()),
        // (0, ±1/φ, ±φ)
        ExactVec3::new(zero(), inv_phi(), phi()),
        ExactVec3::new(zero(), inv_phi(), neg_phi()),
        ExactVec3::new(zero(), neg_inv_phi(), phi()),
        ExactVec3::new(zero(), neg_inv_phi(), neg_phi()),
        // (±1/φ, ±φ, 0)
        ExactVec3::new(inv_phi(), phi(), zero()),
        ExactVec3::new(inv_phi(), neg_phi(), zero()),
        ExactVec3::new(neg_inv_phi(), phi(), zero()),
        ExactVec3::new(neg_inv_phi(), neg_phi(), zero()),
        // (±φ, 0, ±1/φ)
        ExactVec3::new(phi(), zero(), inv_phi()),
        ExactVec3::new(phi(), zero(), neg_inv_phi()),
        ExactVec3::new(neg_phi(), zero(), inv_phi()),
        ExactVec3::new(neg_phi(), zero(), neg_inv_phi()),
    ]
}

pub fn dodecahedron() -> Polyhedron {
    let vertices = vertex_table();
    // 12 pentagonal faces. Indexing must match the `vertex_table` order.
    let faces = vec![
        vec![0, 8, 4, 14, 12],
        vec![0, 12, 1, 17, 16],
        vec![0, 16, 2, 10, 8],
        vec![1, 12, 14, 5, 9],
        vec![1, 9, 11, 3, 17],
        vec![2, 16, 17, 3, 13],
        vec![2, 13, 15, 6, 10],
        vec![3, 11, 7, 15, 13],
        vec![4, 8, 10, 6, 18],
        vec![4, 18, 19, 5, 14],
        vec![5, 19, 7, 11, 9],
        vec![6, 15, 7, 19, 18],
    ];
    Polyhedron::with_exact("dodecahedron", vertices, faces).expect("dodec is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_20() {
        assert_eq!(dodecahedron().vertex_count(), 20);
    }

    #[test]
    fn face_count_12() {
        assert_eq!(dodecahedron().face_count(), 12);
    }

    #[test]
    fn pentagonal_faces() {
        for face in &dodecahedron().faces {
            assert_eq!(face.len(), 5);
        }
    }

    #[test]
    fn exact_vertices_present() {
        let p = dodecahedron();
        let exact = p.exact_vertices.as_ref().expect("exact present");
        assert_eq!(exact.len(), 20);
        // First vertex is (1, 1, 1).
        let f = exact[0].eval_f64();
        assert_eq!(f.x, 1.0);
        assert_eq!(f.y, 1.0);
        assert_eq!(f.z, 1.0);
    }

    #[test]
    fn f64_vertices_match_legacy_phi_constant() {
        // The old hand-typed PHI = 1.618_033_988_749_895; vertex (0, 1/φ, φ)
        // at index 8.
        let p = dodecahedron();
        let v = p.vertices[8];
        let phi_legacy = 1.618_033_988_749_895_f64;
        let inv_phi_legacy = 1.0_f64 / phi_legacy;
        assert_eq!(v.x, 0.0);
        assert!((v.y - inv_phi_legacy).abs() < 1e-15);
        assert!((v.z - phi_legacy).abs() < 1e-15);
    }
}
