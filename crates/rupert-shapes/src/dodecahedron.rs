//! Regular dodecahedron, edge length 2/φ (≈ 1.236), inscribed-sphere
//! radius (apothem of each face) is 1.

use rupert_core::{Polyhedron, Vec3};

/// Golden ratio (1 + √5) / 2.
const PHI: f64 = 1.618_033_988_749_895;
const INV_PHI: f64 = 0.618_033_988_749_895;

pub fn dodecahedron() -> Polyhedron {
    // Vertices: (±1, ±1, ±1) ∪ (0, ±1/φ, ±φ) ∪ (±1/φ, ±φ, 0) ∪ (±φ, 0, ±1/φ)
    let vertices = vec![
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(1.0, 1.0, -1.0),
        Vec3::new(1.0, -1.0, 1.0),
        Vec3::new(1.0, -1.0, -1.0),
        Vec3::new(-1.0, 1.0, 1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(-1.0, -1.0, -1.0),
        Vec3::new(0.0, INV_PHI, PHI),
        Vec3::new(0.0, INV_PHI, -PHI),
        Vec3::new(0.0, -INV_PHI, PHI),
        Vec3::new(0.0, -INV_PHI, -PHI),
        Vec3::new(INV_PHI, PHI, 0.0),
        Vec3::new(INV_PHI, -PHI, 0.0),
        Vec3::new(-INV_PHI, PHI, 0.0),
        Vec3::new(-INV_PHI, -PHI, 0.0),
        Vec3::new(PHI, 0.0, INV_PHI),
        Vec3::new(PHI, 0.0, -INV_PHI),
        Vec3::new(-PHI, 0.0, INV_PHI),
        Vec3::new(-PHI, 0.0, -INV_PHI),
    ];
    // 12 pentagonal faces. Each face indexed CCW from outside. The
    // adjacency is a standard table; we use vertex coordinates as a guide
    // and trust the convex-hull-of-projection pipeline downstream — the
    // exact face winding doesn't affect projection-based clearance.
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
    Polyhedron::new("dodecahedron", vertices, faces).expect("dodec is valid")
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
}
