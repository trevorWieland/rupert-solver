//! Triakis tetrahedron — the Catalan dual of the truncated tetrahedron.
//!
//! 8 vertices, 12 triangular faces. This is the famously "tight" Rupert
//! solid — the published clearance margin is ~4 × 10⁻⁶ of edge length
//! (Fredriksson 2022). Solvers should not be able to find it without
//! refinement.

use rupert_core::{ExactVec3, Polyhedron};

pub fn triakis_tetrahedron() -> Polyhedron {
    // 4 tetra vertices at (±1, ±1, ±1) (even sign count).
    // 4 apex vertices at (5/3) × centroid of opposite tetra face. The
    // centroids work out to ±(1/3, 1/3, 1/3)-pattern; multiplying by
    // 5/3 gives ±(5/9, 5/9, 5/9)-pattern. All rational, all
    // ExactRational-eligible once that verifier path lands.
    let vertices = vec![
        // Tetra vertices
        ExactVec3::int(1, 1, 1),
        ExactVec3::int(-1, -1, 1),
        ExactVec3::int(-1, 1, -1),
        ExactVec3::int(1, -1, -1),
        // Apex 0: opposite v0; centroid (-1/3, -1/3, -1/3) × 5/3 = (-5/9, -5/9, -5/9).
        ExactVec3::rat(-5, 9, -5, 9, -5, 9),
        // Apex 1: opposite v1; centroid (1/3, 1/3, -1/3) × 5/3 = (5/9, 5/9, -5/9).
        ExactVec3::rat(5, 9, 5, 9, -5, 9),
        // Apex 2: opposite v2; centroid (1/3, -1/3, 1/3) × 5/3 = (5/9, -5/9, 5/9).
        ExactVec3::rat(5, 9, -5, 9, 5, 9),
        // Apex 3: opposite v3; centroid (-1/3, 1/3, 1/3) × 5/3 = (-5/9, 5/9, 5/9).
        ExactVec3::rat(-5, 9, 5, 9, 5, 9),
    ];
    let faces = vec![
        vec![1, 2, 4],
        vec![2, 3, 4],
        vec![3, 1, 4],
        vec![0, 3, 5],
        vec![3, 2, 5],
        vec![2, 0, 5],
        vec![0, 1, 6],
        vec![1, 3, 6],
        vec![3, 0, 6],
        vec![0, 2, 7],
        vec![2, 1, 7],
        vec![1, 0, 7],
    ];
    Polyhedron::with_exact("triakis_tetrahedron", vertices, faces).expect("triakis tetra is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_8() {
        assert_eq!(triakis_tetrahedron().vertex_count(), 8);
    }

    #[test]
    fn face_count_12() {
        assert_eq!(triakis_tetrahedron().face_count(), 12);
    }
}
