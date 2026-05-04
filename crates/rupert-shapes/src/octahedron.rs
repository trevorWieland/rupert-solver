//! Regular octahedron with vertices at ±1 along each axis.

use rupert_core::{Polyhedron, Vec3};

pub fn octahedron() -> Polyhedron {
    let vertices = vec![
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 0.0, -1.0),
    ];
    let faces = vec![
        vec![0, 2, 4],
        vec![2, 1, 4],
        vec![1, 3, 4],
        vec![3, 0, 4],
        vec![2, 0, 5],
        vec![1, 2, 5],
        vec![3, 1, 5],
        vec![0, 3, 5],
    ];
    Polyhedron::new("octahedron", vertices, faces).expect("octa is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_6() {
        assert_eq!(octahedron().vertex_count(), 6);
    }

    #[test]
    fn face_count_8() {
        assert_eq!(octahedron().face_count(), 8);
    }
}
