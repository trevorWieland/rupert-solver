//! Triakis tetrahedron — the Catalan dual of the truncated tetrahedron.
//!
//! 8 vertices, 12 triangular faces. This is the famously "tight" Rupert
//! solid — the published clearance margin is ~4 × 10⁻⁶ of edge length
//! (Fredriksson 2022). Solvers should not be able to find it without
//! refinement.

use rupert_core::{Polyhedron, Vec3};

pub fn triakis_tetrahedron() -> Polyhedron {
    // Construction: regular tetrahedron with each face augmented by a small
    // pyramid. Use the four tetra vertices at (±1, ±1, ±1) with even sign
    // count, then place four "tip" vertices at the centroids of the
    // original tetra faces, scaled outward.
    let tetra = [
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(1.0, -1.0, -1.0),
    ];
    // Tip scale factor: for a regular triakis tetrahedron the apex extends
    // to k × face_centroid, where k = 5/3 yields the canonical Catalan.
    let k: f64 = 5.0 / 3.0;
    let tips = vec![
        // Face (1,2,3): opposite vertex 0. Centroid is mean of tetra[1..4].
        face_apex(&tetra, 1, 2, 3, k),
        face_apex(&tetra, 0, 2, 3, k),
        face_apex(&tetra, 0, 1, 3, k),
        face_apex(&tetra, 0, 1, 2, k),
    ];
    let mut vertices: Vec<Vec3> = tetra.to_vec();
    vertices.extend(tips);
    // 12 triangular faces: each original tetra face (3 vertices) is now
    // 3 triangles meeting at the new apex. Apex i sits opposite tetra
    // vertex i. So apex 4 (= idx 4) caps the face (1,2,3).
    let faces = vec![
        // apex 4 caps face opposite vertex 0 (vertices 1,2,3)
        vec![1, 2, 4],
        vec![2, 3, 4],
        vec![3, 1, 4],
        // apex 5 caps face opposite vertex 1 (vertices 0,2,3)
        vec![0, 3, 5],
        vec![3, 2, 5],
        vec![2, 0, 5],
        // apex 6 caps face opposite vertex 2 (vertices 0,1,3)
        vec![0, 1, 6],
        vec![1, 3, 6],
        vec![3, 0, 6],
        // apex 7 caps face opposite vertex 3 (vertices 0,1,2)
        vec![0, 2, 7],
        vec![2, 1, 7],
        vec![1, 0, 7],
    ];
    Polyhedron::new("triakis_tetrahedron", vertices, faces).expect("triakis tetra is valid")
}

fn face_apex(tetra: &[Vec3; 4], a: usize, b: usize, c: usize, k: f64) -> Vec3 {
    let centroid = Vec3::new(
        (tetra[a].x + tetra[b].x + tetra[c].x) / 3.0,
        (tetra[a].y + tetra[b].y + tetra[c].y) / 3.0,
        (tetra[a].z + tetra[b].z + tetra[c].z) / 3.0,
    );
    centroid * k
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
