//! Snub cube — Archimedean solid (chiral cubic symmetry).
//!
//! 24 vertices using the **tribonacci constant** `t ≈ 1.83928675…`,
//! the real root of `t³ = t² + t + 1`. v0.1 hardcoded `t` as f64;
//! v2.0 stores it symbolically as `Expr::Tribonacci`, so the
//! `IntervalSnap` verifier path can recompute via tight intervals.
//!
//! Faces are computed at construction time via `chull` (3D convex hull
//! triangulation): 32 native triangles + 6 squares × 2 = 44 triangles.

use rupert_core::{ExactVec3, Expr, Polyhedron};

use crate::hull3d::triangulate_convex_hull;

fn snub_cube_vertices_exact() -> Vec<ExactVec3> {
    let t = || Expr::tribonacci();
    let inv_t = || Expr::int(1) / t();
    let one = || Expr::int(1);

    let mut out: Vec<ExactVec3> = Vec::with_capacity(24);
    let signed = |sign: i64, e: Expr| -> Expr { if sign > 0 { e } else { -e } };

    // Even permutations of (1, 1/t, t) with even sign count.
    let even_signs: [(i64, i64, i64); 4] = [(1, 1, 1), (1, -1, -1), (-1, 1, -1), (-1, -1, 1)];
    for trip_idx in 0..3 {
        let (ax, bx, cx): (Expr, Expr, Expr) = match trip_idx {
            0 => (one(), inv_t(), t()),
            1 => (inv_t(), t(), one()),
            _ => (t(), one(), inv_t()),
        };
        for &(sa, sb, sc) in &even_signs {
            out.push(ExactVec3::new(
                signed(sa, ax.clone()),
                signed(sb, bx.clone()),
                signed(sc, cx.clone()),
            ));
        }
    }

    // Odd permutations of (1, t, 1/t) with odd sign count.
    let odd_signs: [(i64, i64, i64); 4] = [(1, 1, -1), (1, -1, 1), (-1, 1, 1), (-1, -1, -1)];
    for trip_idx in 0..3 {
        let (ax, bx, cx): (Expr, Expr, Expr) = match trip_idx {
            0 => (one(), t(), inv_t()),
            1 => (t(), inv_t(), one()),
            _ => (inv_t(), one(), t()),
        };
        for &(sa, sb, sc) in &odd_signs {
            out.push(ExactVec3::new(
                signed(sa, ax.clone()),
                signed(sb, bx.clone()),
                signed(sc, cx.clone()),
            ));
        }
    }
    debug_assert_eq!(out.len(), 24);
    out
}

pub fn snub_cube() -> Polyhedron {
    let exact = snub_cube_vertices_exact();
    // Triangulate via chull on the f64 evaluations. Faces consume
    // f64 vertex coordinates only.
    let f64_vertices: Vec<_> = exact.iter().map(ExactVec3::eval_f64).collect();
    let faces = triangulate_convex_hull(&f64_vertices);
    Polyhedron::with_exact("snub_cube", exact, faces).expect("snub cube vertices are valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_24() {
        assert_eq!(snub_cube().vertex_count(), 24);
    }

    #[test]
    fn face_count_is_triangulation() {
        // chull triangulates: 32 native triangles + 6 squares × 2 = 44
        // triangles. patch_aware and face_normal_pairs consume face
        // normals, so the redundancy is harmless.
        assert_eq!(snub_cube().face_count(), 44);
    }

    #[test]
    fn every_face_is_triangle() {
        for f in &snub_cube().faces {
            assert_eq!(f.len(), 3);
        }
    }

    #[test]
    fn faces_reference_valid_vertex_indices() {
        let p = snub_cube();
        for face in &p.faces {
            for &i in face {
                assert!(i < p.vertex_count());
            }
        }
    }

    #[test]
    fn exact_vertices_present() {
        let p = snub_cube();
        assert!(p.exact_vertices.is_some());
        assert_eq!(p.exact_vertices.as_ref().expect("exact").len(), 24);
    }
}
