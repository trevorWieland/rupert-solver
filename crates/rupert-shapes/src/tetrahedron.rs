//! Regular tetrahedron with vertices at alternating corners of the unit cube.

use rupert_core::{Polyhedron, Vec3};

pub fn tetrahedron() -> Polyhedron {
    let vertices = vec![
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(1.0, -1.0, -1.0),
    ];
    let faces = vec![vec![0, 1, 2], vec![0, 2, 3], vec![0, 3, 1], vec![1, 3, 2]];
    Polyhedron::new("tetrahedron", vertices, faces).expect("tetra is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_4() {
        assert_eq!(tetrahedron().vertex_count(), 4);
    }

    #[test]
    fn face_count_4() {
        assert_eq!(tetrahedron().face_count(), 4);
    }
}
