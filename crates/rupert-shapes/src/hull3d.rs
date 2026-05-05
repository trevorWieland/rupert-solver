//! 3D convex hull helper, used by shape constructors that don't have a
//! hand-typed face adjacency table (currently snub_cube and noperthedron).
//!
//! Output is a triangulation: each face has exactly 3 vertex indices.
//! A snub cube's 6 squares come back as 12 coplanar triangles (each
//! square split along a diagonal). For our downstream consumers
//! (face_normal_pairs, patch_aware) this is fine — both compute face
//! normals from face data; coplanar triangles with shared normals are
//! redundant but not wrong.

use chull::ConvexHullWrapper;
use rupert_core::Vec3;

/// Compute a 3D convex-hull triangulation of `vertices`. Returns a list
/// of face index triples in the same numbering as the input vertices.
///
/// On error (degenerate input, < 4 vertices, all coplanar), returns an
/// empty face list — the caller's `Polyhedron::new` validation is the
/// authoritative gate.
#[must_use]
pub fn triangulate_convex_hull(vertices: &[Vec3]) -> Vec<Vec<usize>> {
    let points: Vec<Vec<f64>> = vertices.iter().map(|v| vec![v.x, v.y, v.z]).collect();
    let Ok(hull) = ConvexHullWrapper::try_new(&points, None) else {
        return Vec::new();
    };
    let (_pts, indices) = hull.vertices_indices();
    indices
        .chunks_exact(3)
        .map(|tri| vec![tri[0], tri[1], tri[2]])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rupert_core::Vec3;

    fn cube_vertices() -> Vec<Vec3> {
        vec![
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ]
    }

    #[test]
    fn cube_triangulates_to_twelve_triangles() {
        // Cube has 6 square faces, each split into 2 triangles = 12.
        let faces = triangulate_convex_hull(&cube_vertices());
        assert_eq!(faces.len(), 12);
    }

    #[test]
    fn every_triangle_has_three_distinct_indices() {
        let faces = triangulate_convex_hull(&cube_vertices());
        for f in &faces {
            assert_eq!(f.len(), 3);
            assert!(f[0] != f[1] && f[1] != f[2] && f[0] != f[2]);
        }
    }

    #[test]
    fn empty_input_yields_empty_faces() {
        let faces = triangulate_convex_hull(&[]);
        assert!(faces.is_empty());
    }
}
