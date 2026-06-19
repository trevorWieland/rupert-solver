//! Pentagonal icositetrahedron — the Catalan dual of the snub cube.
//!
//! Calibration target: vertices are generated as the polar dual of the
//! builtin snub cube. The exact algebraic coordinates are built from the
//! same snub-cube face planes, so interval certification is available.

use std::collections::BTreeSet;

use rupert_core::{ExactVec3, Expr, Polyhedron, Vec3};

const PLANE_TOL: f64 = 1.0e-8;

#[derive(Debug, Clone)]
struct NativeFace {
    normal: Vec3,
    offset: f64,
    vertices: BTreeSet<usize>,
}

pub fn pentagonal_icositetrahedron() -> Polyhedron {
    let snub = crate::snub_cube::snub_cube();
    let native_faces = native_snub_faces(&snub);
    let Some(snub_exact) = snub.exact_vertices.as_ref() else {
        return pentagonal_icositetrahedron_f64(&snub, &native_faces);
    };
    let exact_vertices: Vec<ExactVec3> = native_faces
        .iter()
        .filter_map(|face| dual_vertex_exact(face, snub_exact))
        .collect();
    if exact_vertices.len() != native_faces.len() {
        return pentagonal_icositetrahedron_f64(&snub, &native_faces);
    }
    let f64_vertices: Vec<Vec3> = exact_vertices.iter().map(ExactVec3::eval_f64).collect();
    let faces = dual_faces(&snub.vertices, &native_faces, &f64_vertices);
    Polyhedron::with_exact("pentagonal_icositetrahedron", exact_vertices, faces)
        .expect("pentagonal icositetrahedron is valid")
}

fn pentagonal_icositetrahedron_f64(snub: &Polyhedron, native_faces: &[NativeFace]) -> Polyhedron {
    let vertices: Vec<Vec3> = native_faces
        .iter()
        .map(|face| face.normal * (1.0 / face.offset))
        .collect();
    let faces = dual_faces(&snub.vertices, native_faces, &vertices);
    Polyhedron::new("pentagonal_icositetrahedron", vertices, faces)
        .expect("pentagonal icositetrahedron is valid")
}

fn native_snub_faces(snub: &Polyhedron) -> Vec<NativeFace> {
    let mut out: Vec<NativeFace> = Vec::new();
    for face in &snub.faces {
        let Some(mut native) = native_face_from_triangle(snub, face) else {
            continue;
        };
        if let Some(existing) = out.iter_mut().find(|f| same_plane(f, &native)) {
            existing.vertices.append(&mut native.vertices);
        } else {
            out.push(native);
        }
    }
    out
}

fn native_face_from_triangle(snub: &Polyhedron, face: &[usize]) -> Option<NativeFace> {
    let a = snub.vertices[face[0]];
    let b = snub.vertices[face[1]];
    let c = snub.vertices[face[2]];
    let mut normal = (b - a).cross(c - a).normalized();
    if normal == Vec3::ZERO {
        return None;
    }
    if normal.dot(a) < 0.0 {
        normal = -normal;
    }
    let offset = normal.dot(a);
    if offset.abs() <= PLANE_TOL {
        return None;
    }
    Some(NativeFace {
        normal,
        offset,
        vertices: face.iter().copied().collect(),
    })
}

fn same_plane(a: &NativeFace, b: &NativeFace) -> bool {
    a.normal.dot(b.normal) > 1.0 - PLANE_TOL && (a.offset - b.offset).abs() <= PLANE_TOL
}

fn dual_vertex_exact(face: &NativeFace, snub_exact: &[ExactVec3]) -> Option<ExactVec3> {
    let mut indices = face.vertices.iter().copied();
    let a_idx = indices.next()?;
    let b_idx = indices.next()?;
    let c_idx = indices.next()?;
    let a = snub_exact.get(a_idx)?;
    let b = snub_exact.get(b_idx)?;
    let c = snub_exact.get(c_idx)?;
    let ab = exact_sub(b, a);
    let ac = exact_sub(c, a);
    let normal = exact_cross(&ab, &ac);
    let offset = exact_dot(&normal, a);
    Some(ExactVec3::new(
        normal.x / offset.clone(),
        normal.y / offset.clone(),
        normal.z / offset,
    ))
}

fn exact_sub(a: &ExactVec3, b: &ExactVec3) -> ExactVec3 {
    ExactVec3::new(
        a.x.clone() - b.x.clone(),
        a.y.clone() - b.y.clone(),
        a.z.clone() - b.z.clone(),
    )
}

fn exact_cross(a: &ExactVec3, b: &ExactVec3) -> ExactVec3 {
    ExactVec3::new(
        a.y.clone() * b.z.clone() - a.z.clone() * b.y.clone(),
        a.z.clone() * b.x.clone() - a.x.clone() * b.z.clone(),
        a.x.clone() * b.y.clone() - a.y.clone() * b.x.clone(),
    )
}

fn exact_dot(a: &ExactVec3, b: &ExactVec3) -> Expr {
    a.x.clone() * b.x.clone() + a.y.clone() * b.y.clone() + a.z.clone() * b.z.clone()
}

fn dual_faces(
    primal_vertices: &[Vec3],
    native_faces: &[NativeFace],
    dual_vertices: &[Vec3],
) -> Vec<Vec<usize>> {
    let mut out = Vec::with_capacity(primal_vertices.len());
    for (vi, primal_vertex) in primal_vertices.iter().enumerate() {
        let mut incident: Vec<usize> = native_faces
            .iter()
            .enumerate()
            .filter_map(|(fi, face)| face.vertices.contains(&vi).then_some(fi))
            .collect();
        sort_incident_face(&mut incident, *primal_vertex, dual_vertices);
        out.push(incident);
    }
    out
}

fn sort_incident_face(incident: &mut [usize], primal_vertex: Vec3, dual_vertices: &[Vec3]) {
    let axis = primal_vertex.normalized();
    let reference = if axis.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
    let u = axis.cross(reference).normalized();
    let v = axis.cross(u).normalized();
    let center = face_center(incident, dual_vertices);
    incident.sort_by(|a, b| {
        let pa = dual_vertices[*a] - center;
        let pb = dual_vertices[*b] - center;
        let aa = pa.dot(v).atan2(pa.dot(u));
        let ab = pb.dot(v).atan2(pb.dot(u));
        aa.partial_cmp(&ab).unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn face_center(face: &[usize], vertices: &[Vec3]) -> Vec3 {
    let sum = face
        .iter()
        .fold(Vec3::ZERO, |acc, &idx| acc + vertices[idx]);
    sum * (1.0 / face.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_38() {
        assert_eq!(pentagonal_icositetrahedron().vertex_count(), 38);
    }

    #[test]
    fn face_count_24() {
        assert_eq!(pentagonal_icositetrahedron().face_count(), 24);
    }

    #[test]
    fn every_face_is_pentagonal() {
        for face in &pentagonal_icositetrahedron().faces {
            assert_eq!(face.len(), 5);
        }
    }

    #[test]
    fn faces_reference_valid_vertex_indices() {
        let p = pentagonal_icositetrahedron();
        for face in &p.faces {
            for &idx in face {
                assert!(idx < p.vertex_count());
            }
        }
    }

    #[test]
    fn exact_vertices_present() {
        let p = pentagonal_icositetrahedron();
        let exact = p.exact_vertices.as_ref().expect("exact present");
        assert_eq!(exact.len(), 38);
        assert_eq!(p.vertices.len(), exact.len());
    }

    #[test]
    fn exact_vertices_are_interval_eligible() {
        let p = pentagonal_icositetrahedron();
        let exact = p.exact_vertices.as_ref().expect("exact present");
        for v in exact {
            let [x, y, z] = v.eval_interval();
            assert!(!x.is_empty() && !y.is_empty() && !z.is_empty());
            assert!(x.inf().is_finite() && y.inf().is_finite() && z.inf().is_finite());
            assert!(x.sup().is_finite() && y.sup().is_finite() && z.sup().is_finite());
        }
    }
}
