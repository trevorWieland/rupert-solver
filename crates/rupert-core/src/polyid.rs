//! Stable polyhedron identifier (BLAKE3 over canonical vertex/face bytes).

use serde::{Deserialize, Serialize};

use crate::vec3::Vec3;

/// 32-byte BLAKE3 hash. Two polyhedra with the same `PolyId` are
/// "equal up to vertex-list permutation": the canonicalization sorts
/// vertices lexicographically before hashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolyId([u8; 32]);

impl PolyId {
    pub fn derive(vertices: &[Vec3], faces: &[Vec<usize>]) -> Self {
        // 1. Build a permutation that sorts vertices lex by (x, y, z).
        let mut order: Vec<usize> = (0..vertices.len()).collect();
        order.sort_by(|&a, &b| {
            let va = vertices[a];
            let vb = vertices[b];
            (va.x, va.y, va.z)
                .partial_cmp(&(vb.x, vb.y, vb.z))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let inv: Vec<usize> = {
            let mut tmp = vec![0usize; vertices.len()];
            for (new_idx, &old_idx) in order.iter().enumerate() {
                tmp[old_idx] = new_idx;
            }
            tmp
        };

        // 2. Hash the sorted vertex coordinates as f64 le bytes, then the
        //    faces remapped to the new vertex indices, with each face
        //    rotated to start at its lowest index (and reversed if
        //    necessary so the second element is lower than the last) to
        //    canonicalize cyclic / reflective equivalence.
        let mut hasher = blake3::Hasher::new();
        for &i in &order {
            hasher.update(&vertices[i].x.to_le_bytes());
            hasher.update(&vertices[i].y.to_le_bytes());
            hasher.update(&vertices[i].z.to_le_bytes());
        }

        let mut canonical_faces: Vec<Vec<usize>> =
            faces.iter().map(|f| canonicalize_face(f, &inv)).collect();
        canonical_faces.sort();

        for face in &canonical_faces {
            hasher.update(&(face.len() as u64).to_le_bytes());
            for &v in face {
                hasher.update(&(v as u64).to_le_bytes());
            }
        }

        Self(hasher.finalize().into())
    }

    pub fn hex(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::with_capacity(64);
        for b in self.0 {
            // Writing into a String never returns an error; the result is a
            // unit value we discard.
            let _ = write!(&mut s, "{b:02x}");
        }
        s
    }
}

fn canonicalize_face(face: &[usize], inv: &[usize]) -> Vec<usize> {
    let remapped: Vec<usize> = face.iter().map(|&v| inv[v]).collect();
    let n = remapped.len();
    if n == 0 {
        return remapped;
    }
    // Find the smallest-index rotation, in either direction.
    let mut best = remapped.clone();
    for start in 0..n {
        let rot: Vec<usize> = (0..n).map(|i| remapped[(start + i) % n]).collect();
        if rot < best {
            best = rot;
        }
        let rev: Vec<usize> = (0..n).map(|i| remapped[(start + n - i) % n]).collect();
        if rev < best {
            best = rev;
        }
    }
    best
}

impl std::fmt::Display for PolyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tetra_verts() -> Vec<Vec3> {
        vec![
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
        ]
    }

    fn tetra_faces() -> Vec<Vec<usize>> {
        vec![vec![0, 1, 2], vec![0, 2, 3], vec![0, 3, 1], vec![1, 3, 2]]
    }

    #[test]
    fn same_input_same_id() {
        let a = PolyId::derive(&tetra_verts(), &tetra_faces());
        let b = PolyId::derive(&tetra_verts(), &tetra_faces());
        assert_eq!(a, b);
    }

    #[test]
    fn vertex_permutation_same_id() {
        let v = tetra_verts();
        let f = tetra_faces();
        // Permute vertices: swap 0 and 2; remap faces accordingly.
        let v2 = vec![v[2], v[1], v[0], v[3]];
        let f2: Vec<Vec<usize>> = f
            .iter()
            .map(|face| {
                face.iter()
                    .map(|&i| match i {
                        0 => 2,
                        2 => 0,
                        x => x,
                    })
                    .collect()
            })
            .collect();
        let a = PolyId::derive(&v, &f);
        let b = PolyId::derive(&v2, &f2);
        assert_eq!(a, b, "permuted vertex labels should yield same id");
    }

    #[test]
    fn different_polyhedra_different_id() {
        let a = PolyId::derive(&tetra_verts(), &tetra_faces());
        let mut v2 = tetra_verts();
        v2[0].x += 0.1;
        let b = PolyId::derive(&v2, &tetra_faces());
        assert_ne!(a, b);
    }

    #[test]
    fn hex_is_64_chars() {
        let id = PolyId::derive(&tetra_verts(), &tetra_faces());
        assert_eq!(id.hex().len(), 64);
    }
}
