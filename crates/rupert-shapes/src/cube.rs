//! Unit cube `[-1, 1]^3`. Rational coordinates throughout — fully
//! `ExactRational`-eligible once that verifier path lands.

use rupert_core::{ExactVec3, Polyhedron};

pub fn cube() -> Polyhedron {
    let vertices = vec![
        ExactVec3::int(-1, -1, -1),
        ExactVec3::int(1, -1, -1),
        ExactVec3::int(1, 1, -1),
        ExactVec3::int(-1, 1, -1),
        ExactVec3::int(-1, -1, 1),
        ExactVec3::int(1, -1, 1),
        ExactVec3::int(1, 1, 1),
        ExactVec3::int(-1, 1, 1),
    ];
    let faces = vec![
        vec![0, 1, 2, 3],
        vec![4, 5, 6, 7],
        vec![0, 1, 5, 4],
        vec![2, 3, 7, 6],
        vec![0, 3, 7, 4],
        vec![1, 2, 6, 5],
    ];
    Polyhedron::with_exact("cube", vertices, faces).expect("cube is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_8() {
        assert_eq!(cube().vertex_count(), 8);
    }

    #[test]
    fn face_count_6() {
        assert_eq!(cube().face_count(), 6);
    }

    #[test]
    fn euler_characteristic_2() {
        // V - E + F = 2 (cube has 12 edges, 8 vertices, 6 faces).
        let c = cube();
        let mut edges: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for face in &c.faces {
            for i in 0..face.len() {
                let a = face[i];
                let b = face[(i + 1) % face.len()];
                let key = if a < b { (a, b) } else { (b, a) };
                edges.insert(key);
            }
        }
        let v = c.vertex_count();
        let f = c.face_count();
        let e = edges.len();
        assert_eq!(v + f, e + 2);
    }
}
