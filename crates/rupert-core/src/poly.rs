//! Convex polyhedron type.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::polyid::PolyId;
use crate::vec3::Vec3;

/// A convex polyhedron defined by its vertices and the indices of vertices
/// forming each face. Faces are oriented consistently with outward normals
/// (right-hand rule), but the solver/verifier code only depends on the
/// vertex set for the silhouette computation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polyhedron {
    pub name: String,
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Vec<usize>>,
    #[serde(skip)]
    cached_id: std::sync::OnceLock<PolyId>,
}

impl Polyhedron {
    pub fn new(
        name: impl Into<String>,
        vertices: Vec<Vec3>,
        faces: Vec<Vec<usize>>,
    ) -> Result<Self, CoreError> {
        if vertices.is_empty() {
            return Err(CoreError::EmptyPolyhedron);
        }
        for (fi, f) in faces.iter().enumerate() {
            if f.len() < 3 {
                return Err(CoreError::DegenerateFace { face_index: fi });
            }
            for &vi in f {
                if vi >= vertices.len() {
                    return Err(CoreError::FaceIndexOutOfRange {
                        face_index: fi,
                        vertex_index: vi,
                        vertex_count: vertices.len(),
                    });
                }
            }
        }
        Ok(Self {
            name: name.into(),
            vertices,
            faces,
            cached_id: std::sync::OnceLock::new(),
        })
    }

    /// Stable BLAKE3 hash of the canonical (lex-sorted vertex bytes + face
    /// adjacency) representation. Equivalent shapes hash identically.
    pub fn id(&self) -> &PolyId {
        self.cached_id
            .get_or_init(|| PolyId::derive(&self.vertices, &self.faces))
    }

    /// Mean edge length — handy for normalizing clearance margins.
    pub fn mean_edge_length(&self) -> f64 {
        let mut total = 0.0;
        let mut n: usize = 0;
        for face in &self.faces {
            for i in 0..face.len() {
                let a = self.vertices[face[i]];
                let b = self.vertices[face[(i + 1) % face.len()]];
                total += (a - b).norm();
                n += 1;
            }
        }
        if n == 0 { 0.0 } else { total / n as f64 }
    }

    /// Vertex count.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Face count.
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_tetra() -> Polyhedron {
        let verts = vec![
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
        ];
        let faces = vec![vec![0, 1, 2], vec![0, 2, 3], vec![0, 3, 1], vec![1, 3, 2]];
        Polyhedron::new("test_tetra", verts, faces).expect("valid tetra")
    }

    #[test]
    fn rejects_empty_vertices() {
        let r = Polyhedron::new("empty", vec![], vec![]);
        assert!(matches!(r, Err(CoreError::EmptyPolyhedron)));
    }

    #[test]
    fn rejects_degenerate_face() {
        let verts = vec![Vec3::ZERO, Vec3::X, Vec3::Y];
        let r = Polyhedron::new("bad", verts, vec![vec![0, 1]]);
        assert!(matches!(r, Err(CoreError::DegenerateFace { .. })));
    }

    #[test]
    fn rejects_oob_face_index() {
        let verts = vec![Vec3::ZERO, Vec3::X, Vec3::Y];
        let r = Polyhedron::new("bad", verts, vec![vec![0, 1, 9]]);
        assert!(matches!(r, Err(CoreError::FaceIndexOutOfRange { .. })));
    }

    #[test]
    fn id_is_stable() {
        let a = unit_tetra();
        let b = unit_tetra();
        assert_eq!(a.id(), b.id());
    }

    #[test]
    fn vertex_and_face_counts() {
        let t = unit_tetra();
        assert_eq!(t.vertex_count(), 4);
        assert_eq!(t.face_count(), 4);
    }

    #[test]
    fn mean_edge_length_positive() {
        let t = unit_tetra();
        assert!(t.mean_edge_length() > 0.0);
    }
}
