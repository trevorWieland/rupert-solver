//! Orthographic projection of a rotated polyhedron's vertices to the xy plane.

use crate::poly::Polyhedron;
use crate::quat::Quat;

/// 2D point. Bare struct (not a newtype on `[f64; 2]`) so it serializes
/// transparently and arithmetic is ergonomic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct P2 {
    pub x: f64,
    pub y: f64,
}

impl P2 {
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Apply the rotation to every vertex of `poly` and orthographically project
/// to the xy plane (drop z). The output array preserves the input vertex
/// indexing.
pub fn project_xy(poly: &Polyhedron, q: Quat) -> Vec<P2> {
    poly.vertices
        .iter()
        .map(|&v| {
            let r = q.rotate(v);
            P2::new(r.x, r.y)
        })
        .collect()
}

/// Translate every 2D point by `(tx, ty)`.
pub fn translate_xy(points: &[P2], tx: f64, ty: f64) -> Vec<P2> {
    points.iter().map(|p| P2::new(p.x + tx, p.y + ty)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::Vec3;

    fn cube() -> Polyhedron {
        let v = vec![
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ];
        let f = vec![
            vec![0, 1, 2, 3],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![2, 3, 7, 6],
            vec![0, 3, 7, 4],
            vec![1, 2, 6, 5],
        ];
        Polyhedron::new("cube", v, f).expect("valid cube")
    }

    #[test]
    fn identity_projection_drops_z() {
        let c = cube();
        let pts = project_xy(&c, Quat::IDENTITY);
        assert_eq!(pts.len(), 8);
        // The projection of a unit cube under identity is the unit square
        // (eight points clustered onto four corners with z=±1 dropped).
        for p in &pts {
            assert!((p.x.abs() - 1.0).abs() < 1e-12);
            assert!((p.y.abs() - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn translate_offsets_each_point() {
        let pts = vec![P2::new(0.0, 0.0), P2::new(1.0, 2.0)];
        let out = translate_xy(&pts, 3.0, 4.0);
        assert_eq!(out[0], P2::new(3.0, 4.0));
        assert_eq!(out[1], P2::new(4.0, 6.0));
    }
}
