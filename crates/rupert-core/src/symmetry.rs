//! Rotation symmetry groups for builtin polyhedra.
//!
//! Three families cover everything we ship:
//!
//! - [`tetrahedral_rotation_group`] (order 12) — tetrahedron, triakis
//!   tetrahedron, truncated tetrahedron.
//! - [`octahedral_rotation_group`] (order 24) — cube, octahedron, all
//!   their Archimedean cousins, and the **chiral snub cube**.
//! - [`icosahedral_rotation_group`] (order 60) — dodecahedron,
//!   icosahedron, snub dodecahedron, and the four Catalan duals of the
//!   icosahedral Archimedeans.
//!
//! The functions return `Vec<Quat>` rather than constants because
//! `Quat::from_axis_angle` is not const. Callers should cache the
//! result if needed (the patch_aware solver does, via a `OnceLock`
//! per shape).
//!
//! Validation: per-shape unit tests verify every `g ∈ group` permutes
//! that shape's vertex set with f64 tolerance.

use std::f64::consts::PI;

use crate::{Quat, Vec3};

/// Tetrahedral rotation group T, |T| = 12. Identity + 8 vertex
/// rotations (4 axes × {120°, 240°}) + 3 edge-pair rotations
/// ({x, y, z} × 180°).
#[must_use]
pub fn tetrahedral_rotation_group() -> Vec<Quat> {
    let mut out = Vec::with_capacity(12);
    out.push(Quat::IDENTITY);
    // 8 vertex rotations: 4 axes × {120°, 240°}.
    let vertex_axes = [
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(1.0, -1.0, -1.0),
    ];
    for axis in vertex_axes {
        out.push(Quat::from_axis_angle(axis, 2.0 * PI / 3.0));
        out.push(Quat::from_axis_angle(axis, 4.0 * PI / 3.0));
    }
    // 3 edge-pair rotations: 180° around the coordinate axes.
    out.push(Quat::from_axis_angle(Vec3::X, PI));
    out.push(Quat::from_axis_angle(Vec3::Y, PI));
    out.push(Quat::from_axis_angle(Vec3::Z, PI));
    debug_assert_eq!(out.len(), 12);
    out
}

/// Octahedral rotation group O, |O| = 24. Identity + 9 face rotations
/// (3 axes × {90°, 180°, 270°}) + 8 vertex rotations (4 axes ×
/// {120°, 240°}) + 6 edge rotations (6 axes × 180°).
#[must_use]
pub fn octahedral_rotation_group() -> Vec<Quat> {
    let mut out = Vec::with_capacity(24);
    out.push(Quat::IDENTITY);
    // 9 face rotations.
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        out.push(Quat::from_axis_angle(axis, PI / 2.0));
        out.push(Quat::from_axis_angle(axis, PI));
        out.push(Quat::from_axis_angle(axis, 3.0 * PI / 2.0));
    }
    // 8 vertex rotations: 4 cubic body-diagonal axes.
    let vertex_axes = [
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(1.0, 1.0, -1.0),
        Vec3::new(1.0, -1.0, 1.0),
        Vec3::new(-1.0, 1.0, 1.0),
    ];
    for axis in vertex_axes {
        out.push(Quat::from_axis_angle(axis, 2.0 * PI / 3.0));
        out.push(Quat::from_axis_angle(axis, 4.0 * PI / 3.0));
    }
    // 6 edge rotations: 6 cubic edge-midpoint directions.
    let edge_axes = [
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, -1.0),
        Vec3::new(0.0, 1.0, 1.0),
        Vec3::new(0.0, 1.0, -1.0),
    ];
    for axis in edge_axes {
        out.push(Quat::from_axis_angle(axis, PI));
    }
    debug_assert_eq!(out.len(), 24);
    out
}

/// Icosahedral rotation group I, |I| = 60. Identity + 24 vertex
/// rotations (6 axes × 4 rotations) + 20 face rotations (10 axes × 2)
/// + 15 edge rotations.
///
/// Edge axes are computed at runtime by deduplicating antipodal pairs
/// of icosahedron-edge midpoints. Vertex and face axes are tabulated
/// in the canonical icos coordinate system (vertices at `(0, ±1, ±φ)`
/// and permutations).
#[must_use]
pub fn icosahedral_rotation_group() -> Vec<Quat> {
    let phi = f64::midpoint(1.0, 5.0_f64.sqrt());
    let phi2 = phi + 1.0; // φ² = φ + 1.
    let mut out = Vec::with_capacity(60);
    out.push(Quat::IDENTITY);

    // 6 vertex axes (icos vertices), 4 rotations each.
    let vertex_axes = [
        Vec3::new(0.0, 1.0, phi),
        Vec3::new(0.0, 1.0, -phi),
        Vec3::new(1.0, phi, 0.0),
        Vec3::new(1.0, -phi, 0.0),
        Vec3::new(phi, 0.0, 1.0),
        Vec3::new(phi, 0.0, -1.0),
    ];
    for axis in vertex_axes {
        for k in 1..5 {
            out.push(Quat::from_axis_angle(axis, 2.0 * PI * f64::from(k) / 5.0));
        }
    }

    // 10 face axes (icos face centroids).
    // 4 cube-corner axes — (1, 1, 1)-type centroids:
    let face_axes_cube = [
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(1.0, 1.0, -1.0),
        Vec3::new(1.0, -1.0, 1.0),
        Vec3::new(-1.0, 1.0, 1.0),
    ];
    // 6 "rectangular" face axes — cyclic permutations of `(φ², 1, 0)`
    // (the icos face centroid direction up to scaling), with sign of
    // the unit component flipped. Note `(1, φ², 0)` is NOT in the
    // orbit — that would be an x↔y swap, a reflection, not a rotation.
    let face_axes_rect = [
        Vec3::new(phi2, 1.0, 0.0),
        Vec3::new(phi2, -1.0, 0.0),
        Vec3::new(0.0, phi2, 1.0),
        Vec3::new(0.0, phi2, -1.0),
        Vec3::new(1.0, 0.0, phi2),
        Vec3::new(-1.0, 0.0, phi2),
    ];
    for axis in face_axes_cube.iter().chain(face_axes_rect.iter()) {
        out.push(Quat::from_axis_angle(*axis, 2.0 * PI / 3.0));
        out.push(Quat::from_axis_angle(*axis, 4.0 * PI / 3.0));
    }

    // 15 edge axes — antipodal-deduped icos edge midpoints.
    for axis in icos_edge_axes(phi) {
        out.push(Quat::from_axis_angle(axis, PI));
    }

    debug_assert_eq!(out.len(), 60, "icos group size = {}", out.len());
    out
}

/// Compute the 15 distinct icosahedral edge axes by walking the
/// 12 vertices, finding pairs at edge length (distance 2 in our
/// coordinate system), and deduplicating antipodal midpoints.
fn icos_edge_axes(phi: f64) -> Vec<Vec3> {
    let icos_vertices = [
        Vec3::new(0.0, 1.0, phi),
        Vec3::new(0.0, 1.0, -phi),
        Vec3::new(0.0, -1.0, phi),
        Vec3::new(0.0, -1.0, -phi),
        Vec3::new(1.0, phi, 0.0),
        Vec3::new(1.0, -phi, 0.0),
        Vec3::new(-1.0, phi, 0.0),
        Vec3::new(-1.0, -phi, 0.0),
        Vec3::new(phi, 0.0, 1.0),
        Vec3::new(phi, 0.0, -1.0),
        Vec3::new(-phi, 0.0, 1.0),
        Vec3::new(-phi, 0.0, -1.0),
    ];
    let edge_length_sq = 4.0_f64;
    let mut axes: Vec<Vec3> = Vec::new();
    for i in 0..icos_vertices.len() {
        for j in (i + 1)..icos_vertices.len() {
            let d_sq = (icos_vertices[i] - icos_vertices[j]).norm_sq();
            if (d_sq - edge_length_sq).abs() < 1.0e-9 {
                let mid = (icos_vertices[i] + icos_vertices[j]) * 0.5;
                let canonical = canonicalize_axis(mid);
                if !axes.iter().any(|a| (*a - canonical).norm() < 1.0e-9) {
                    axes.push(canonical);
                }
            }
        }
    }
    axes
}

/// Canonicalize an axis direction by ensuring the lex-largest
/// representative (between v and -v) is returned. Stabilizes
/// hash/dedup across antipodal duplicates.
fn canonicalize_axis(v: Vec3) -> Vec3 {
    let neg = -v;
    let v_tuple = (v.x, v.y, v.z);
    let neg_tuple = (neg.x, neg.y, neg.z);
    if v_tuple >= neg_tuple { v } else { neg }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tetrahedral_group_size() {
        assert_eq!(tetrahedral_rotation_group().len(), 12);
    }

    #[test]
    fn octahedral_group_size() {
        assert_eq!(octahedral_rotation_group().len(), 24);
    }

    #[test]
    fn icosahedral_group_size_60() {
        assert_eq!(icosahedral_rotation_group().len(), 60);
    }

    #[test]
    fn icosahedral_edge_axes_count_15() {
        let phi = f64::midpoint(1.0, 5.0_f64.sqrt());
        let axes = icos_edge_axes(phi);
        assert_eq!(axes.len(), 15, "icos has 15 edge axes (30 edges / 2)");
    }

    #[test]
    fn tetrahedral_identity_first() {
        assert_eq!(tetrahedral_rotation_group()[0], Quat::IDENTITY);
    }

    // Per-shape "permutes vertex set" tests live in `rupert-shapes`,
    // since they require shape definitions. See the symmetry tests
    // there.
}
