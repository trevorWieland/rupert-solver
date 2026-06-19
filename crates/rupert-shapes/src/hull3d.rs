//! 3D convex hull helper, used by shape constructors that don't have a
//! hand-typed face adjacency table (currently snub_cube and noperthedron).
//!
//! Output merges coplanar hull triangles back into deterministic native
//! polygon faces. This avoids QHull's arbitrary diagonal choices for
//! square/polygonal faces leaking into `PolyId` and patch-aware ordering.

use std::collections::BTreeSet;

use chull::ConvexHullWrapper;
use rupert_core::Vec3;

const PLANE_TOL: f64 = 1.0e-8;

#[derive(Debug, Clone)]
struct FaceGroup {
    normal: Vec3,
    offset: f64,
    vertices: BTreeSet<usize>,
}

/// Compute a deterministic 3D convex hull of `vertices`. Returns a list
/// of face vertex-index loops in the same numbering as the input vertices.
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
    let center = vertex_center(vertices);
    let groups = group_coplanar_triangles(vertices, center, &indices);
    let mut faces: Vec<Vec<usize>> = groups
        .iter()
        .map(|group| ordered_face(vertices, group))
        .collect();
    faces.sort();
    faces
}

fn vertex_center(vertices: &[Vec3]) -> Vec3 {
    let sum = vertices.iter().fold(Vec3::ZERO, |acc, v| acc + *v);
    sum * (1.0 / vertices.len() as f64)
}

fn group_coplanar_triangles(vertices: &[Vec3], center: Vec3, indices: &[usize]) -> Vec<FaceGroup> {
    let mut groups: Vec<FaceGroup> = Vec::new();
    for tri in indices.chunks_exact(3) {
        let Some(group) = face_group_from_triangle(vertices, center, [tri[0], tri[1], tri[2]])
        else {
            continue;
        };
        if let Some(existing) = groups
            .iter_mut()
            .find(|existing| same_plane(existing, &group))
        {
            existing.vertices.extend(group.vertices);
        } else {
            groups.push(group);
        }
    }
    groups
}

fn face_group_from_triangle(vertices: &[Vec3], center: Vec3, tri: [usize; 3]) -> Option<FaceGroup> {
    let a = vertices[tri[0]];
    let b = vertices[tri[1]];
    let c = vertices[tri[2]];
    let mut normal = (b - a).cross(c - a).normalized();
    if normal == Vec3::ZERO {
        return None;
    }
    if normal.dot(a - center) < 0.0 {
        normal = -normal;
    }
    Some(FaceGroup {
        normal,
        offset: normal.dot(a),
        vertices: tri.into_iter().collect(),
    })
}

fn same_plane(a: &FaceGroup, b: &FaceGroup) -> bool {
    a.normal.dot(b.normal) > 1.0 - PLANE_TOL && (a.offset - b.offset).abs() <= PLANE_TOL
}

fn ordered_face(vertices: &[Vec3], group: &FaceGroup) -> Vec<usize> {
    let mut face: Vec<usize> = group.vertices.iter().copied().collect();
    let center = face_center(&face, vertices);
    let reference = if group.normal.z.abs() < 0.9 {
        Vec3::Z
    } else {
        Vec3::X
    };
    let u = group.normal.cross(reference).normalized();
    let v = group.normal.cross(u).normalized();
    face.sort_by(|a, b| {
        let pa = vertices[*a] - center;
        let pb = vertices[*b] - center;
        let aa = pa.dot(v).atan2(pa.dot(u));
        let ab = pb.dot(v).atan2(pb.dot(u));
        aa.partial_cmp(&ab).unwrap_or(std::cmp::Ordering::Equal)
    });
    orient_and_rotate_min(vertices, group.normal, &face)
}

fn face_center(face: &[usize], vertices: &[Vec3]) -> Vec3 {
    let sum = face
        .iter()
        .fold(Vec3::ZERO, |acc, &idx| acc + vertices[idx]);
    sum * (1.0 / face.len() as f64)
}

fn orient_and_rotate_min(vertices: &[Vec3], normal: Vec3, face: &[usize]) -> Vec<usize> {
    let mut oriented = face.to_vec();
    if newell_normal(vertices, &oriented).dot(normal) < 0.0 {
        oriented.reverse();
    }
    rotate_min_first(&oriented)
}

fn newell_normal(vertices: &[Vec3], face: &[usize]) -> Vec3 {
    let mut normal = Vec3::ZERO;
    for i in 0..face.len() {
        let curr = vertices[face[i]];
        let next = vertices[face[(i + 1) % face.len()]];
        normal.x += (curr.y - next.y) * (curr.z + next.z);
        normal.y += (curr.z - next.z) * (curr.x + next.x);
        normal.z += (curr.x - next.x) * (curr.y + next.y);
    }
    normal
}

fn rotate_min_first(face: &[usize]) -> Vec<usize> {
    let min_pos = face
        .iter()
        .enumerate()
        .min_by_key(|(_, idx)| *idx)
        .map_or(0, |(idx, _)| idx);
    let mut out = Vec::with_capacity(face.len());
    for offset in 0..face.len() {
        out.push(face[(min_pos + offset) % face.len()]);
    }
    out
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
    fn cube_hull_has_six_square_faces() {
        let faces = triangulate_convex_hull(&cube_vertices());
        assert_eq!(faces.len(), 6);
        for face in faces {
            assert_eq!(face.len(), 4);
        }
    }

    #[test]
    fn triangulation_order_is_stable() {
        let a = triangulate_convex_hull(&cube_vertices());
        let b = triangulate_convex_hull(&cube_vertices());
        assert_eq!(a, b);
    }

    #[test]
    fn every_face_has_distinct_indices() {
        let faces = triangulate_convex_hull(&cube_vertices());
        for f in &faces {
            assert!(f.len() >= 3);
            let mut sorted = f.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(f.len(), sorted.len());
        }
    }

    #[test]
    fn empty_input_yields_empty_faces() {
        let faces = triangulate_convex_hull(&[]);
        assert!(faces.is_empty());
    }
}
