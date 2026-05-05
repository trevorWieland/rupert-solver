//! Regular icosahedron — vertices over `Expr::GoldenRatio`.
//!
//! v2 phase 2 migration; same pattern as `dodecahedron.rs`. Solvers see
//! identical f64 coordinates; verifier gains an `IntervalSnap` path
//! once phase 4 lands.

use rupert_core::{ExactVec3, Expr, Polyhedron};

fn vertex_table() -> Vec<ExactVec3> {
    let phi = Expr::golden_ratio;
    let neg_phi = || -phi();
    let zero = || Expr::int(0);
    let one = || Expr::int(1);
    let neg_one = || Expr::int(-1);

    vec![
        // (0, ±1, ±φ)
        ExactVec3::new(zero(), one(), phi()),
        ExactVec3::new(zero(), one(), neg_phi()),
        ExactVec3::new(zero(), neg_one(), phi()),
        ExactVec3::new(zero(), neg_one(), neg_phi()),
        // (±1, ±φ, 0)
        ExactVec3::new(one(), phi(), zero()),
        ExactVec3::new(one(), neg_phi(), zero()),
        ExactVec3::new(neg_one(), phi(), zero()),
        ExactVec3::new(neg_one(), neg_phi(), zero()),
        // (±φ, 0, ±1)
        ExactVec3::new(phi(), zero(), one()),
        ExactVec3::new(phi(), zero(), neg_one()),
        ExactVec3::new(neg_phi(), zero(), one()),
        ExactVec3::new(neg_phi(), zero(), neg_one()),
    ]
}

pub fn icosahedron() -> Polyhedron {
    let vertices = vertex_table();
    let faces = vec![
        vec![0, 4, 6],
        vec![0, 6, 10],
        vec![0, 10, 2],
        vec![0, 2, 8],
        vec![0, 8, 4],
        vec![3, 1, 11],
        vec![3, 11, 7],
        vec![3, 7, 5],
        vec![3, 5, 9],
        vec![3, 9, 1],
        vec![4, 9, 8],
        vec![4, 8, 5],
        vec![4, 5, 6],
        vec![6, 5, 7],
        vec![6, 7, 11],
        vec![6, 11, 10],
        vec![10, 11, 1],
        vec![10, 1, 2],
        vec![2, 1, 9],
        vec![2, 9, 8],
    ];
    Polyhedron::with_exact("icosahedron", vertices, faces).expect("icos is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_12() {
        assert_eq!(icosahedron().vertex_count(), 12);
    }

    #[test]
    fn face_count_20() {
        assert_eq!(icosahedron().face_count(), 20);
    }

    #[test]
    fn exact_vertices_present() {
        let p = icosahedron();
        let exact = p.exact_vertices.as_ref().expect("exact present");
        assert_eq!(exact.len(), 12);
    }
}
