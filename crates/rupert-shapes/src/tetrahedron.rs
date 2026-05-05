//! Regular tetrahedron with vertices at alternating corners of the unit cube.

use rupert_core::{ExactVec3, Polyhedron};

pub fn tetrahedron() -> Polyhedron {
    let vertices = vec![
        ExactVec3::int(1, 1, 1),
        ExactVec3::int(-1, -1, 1),
        ExactVec3::int(-1, 1, -1),
        ExactVec3::int(1, -1, -1),
    ];
    let faces = vec![vec![0, 1, 2], vec![0, 2, 3], vec![0, 3, 1], vec![1, 3, 2]];
    Polyhedron::with_exact("tetrahedron", vertices, faces).expect("tetra is valid")
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
