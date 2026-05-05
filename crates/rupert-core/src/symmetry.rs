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

/// Icosahedral rotation group I, |I| = 60. **v0.1.0 stub** — returns
/// only the identity. The full group requires:
/// - 6 vertex axes × 4 rotations each = 24
/// - 10 face axes × 2 rotations = 20
/// - 15 edge axes × 1 rotation = 15
/// - identity = 1
///
/// The face axes are permutations of `(0, φ+1, 1)` (NOT
/// `(0, 1/φ, φ)` as the dodec vertex set might suggest — the dodec
/// vertices and icos face centroids are dual but their f64
/// scales in this codebase don't coincide). Edge axes are the
/// 15 axis-canonical midpoints of the 30 icosahedral edges.
///
/// v0.2.0 work item — write the correct face/edge tables with a
/// per-element-permutes-vertex-set test. Until then, the
/// `patch_aware` solver runs on dodec/icos without symmetry
/// reduction (full O(F²) brute force, ~5× more work but
/// correct).
#[must_use]
pub fn icosahedral_rotation_group() -> Vec<Quat> {
    vec![Quat::IDENTITY]
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
    fn icosahedral_group_size_v01_stub() {
        assert_eq!(icosahedral_rotation_group().len(), 1);
    }

    #[test]
    fn tetrahedral_identity_first() {
        assert_eq!(tetrahedral_rotation_group()[0], Quat::IDENTITY);
    }

    // Per-shape "permutes vertex set" tests live in `rupert-shapes`,
    // since they require shape definitions. See the symmetry tests
    // there.
}
