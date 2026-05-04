//! Regular icosahedron, vertices at (0, ±1, ±φ), (±1, ±φ, 0), (±φ, 0, ±1).

use rupert_core::{Polyhedron, Vec3};

const PHI: f64 = 1.618_033_988_749_895;

pub fn icosahedron() -> Polyhedron {
    let vertices = vec![
        Vec3::new(0.0, 1.0, PHI),
        Vec3::new(0.0, 1.0, -PHI),
        Vec3::new(0.0, -1.0, PHI),
        Vec3::new(0.0, -1.0, -PHI),
        Vec3::new(1.0, PHI, 0.0),
        Vec3::new(1.0, -PHI, 0.0),
        Vec3::new(-1.0, PHI, 0.0),
        Vec3::new(-1.0, -PHI, 0.0),
        Vec3::new(PHI, 0.0, 1.0),
        Vec3::new(PHI, 0.0, -1.0),
        Vec3::new(-PHI, 0.0, 1.0),
        Vec3::new(-PHI, 0.0, -1.0),
    ];
    // 20 triangular faces. Like the dodecahedron, exact winding doesn't
    // affect projection-based clearance; this is a standard adjacency.
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
    Polyhedron::new("icosahedron", vertices, faces).expect("icos is valid")
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
}
