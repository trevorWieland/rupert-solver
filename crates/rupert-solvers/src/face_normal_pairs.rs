//! Deterministic enumeration of "axis-aligned" candidate pairs.
//!
//! For each pair of vertices `(v_outer, v_inner)` of the polyhedron, build
//! a rotation that aligns each vertex with `+z`, then evaluate. Vertex
//! alignment is a strict subset of face/edge/vertex alignment but it
//! exhibits the same essential property: deterministic, instant on
//! highly-symmetric solids, useless on shapes whose Rupert passage
//! requires off-axis configurations.
//!
//! For shapes whose vertex set is small (≤ 30), this enumerates `O(V²)`
//! candidates. For shapes whose vertex set is huge (e.g. the noperthedron
//! at 90 vertices = 8100 pairs), the budget is the soft limit.

use rupert_core::{
    Budget, Candidate, EvalCounter, Polyhedron, Quat, Solution, Solver, SolverOutcome, Vec3,
};

#[derive(Debug, Default)]
pub struct FaceNormalPairs;

impl Solver for FaceNormalPairs {
    fn name(&self) -> &'static str {
        "face_normal_pairs"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn solve(
        &mut self,
        poly: &Polyhedron,
        budget: &Budget,
        ec: &mut EvalCounter<'_>,
    ) -> SolverOutcome {
        let max = budget.max_evaluations.get();
        let directions = collect_directions(poly);
        // For each direction we enumerate `IN_PLANE_STEPS` rotations
        // around z (post-multiplied), since the Rupert passage of the
        // cube etc. requires an in-plane twist to fit the inner silhouette
        // inside the outer one. Identity is included as the first
        // candidate.
        let mut base_quats: Vec<Quat> = vec![Quat::IDENTITY];
        for d in &directions {
            base_quats.push(align_to_plus_z(*d));
        }
        let mut quats: Vec<Quat> = Vec::with_capacity(base_quats.len() * IN_PLANE_STEPS);
        for q in &base_quats {
            for k in 0..IN_PLANE_STEPS {
                let phi = std::f64::consts::PI * (k as f64) / (IN_PLANE_STEPS as f64);
                let twist = Quat::from_axis_angle(Vec3::Z, phi);
                quats.push((twist * *q).normalized());
            }
        }
        for q_outer in &quats {
            for q_inner in &quats {
                if ec.count() >= max {
                    return SolverOutcome::Exhausted;
                }
                let candidate = Candidate {
                    outer: *q_outer,
                    inner: *q_inner,
                    translation: [0.0, 0.0],
                };
                let c = ec.evaluate(&candidate);
                if c.is_finite() && c > 0.0 {
                    return SolverOutcome::Found(Solution {
                        candidate,
                        clearance: c,
                        found_at_eval: ec.count(),
                        certification: None,
                    });
                }
            }
        }
        SolverOutcome::Exhausted
    }
}

const IN_PLANE_STEPS: usize = 12;

/// Build the enumeration set: vertices ∪ face centroids ∪ edge midpoints.
/// For shapes shipped with empty faces (snub_cube, noperthedron in v1),
/// only vertices contribute.
fn collect_directions(poly: &Polyhedron) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::with_capacity(poly.vertices.len() + poly.faces.len() * 2);
    out.extend(poly.vertices.iter().copied());
    for face in &poly.faces {
        let mut centroid = Vec3::ZERO;
        for &i in face {
            centroid = centroid + poly.vertices[i];
        }
        centroid = centroid * (1.0 / face.len() as f64);
        out.push(centroid);
        for i in 0..face.len() {
            let a = poly.vertices[face[i]];
            let b = poly.vertices[face[(i + 1) % face.len()]];
            out.push((a + b) * 0.5);
        }
    }
    out
}

fn align_to_plus_z(target: Vec3) -> Quat {
    // Rotation that takes `target.normalized()` to +z. Identity if target is
    // already +z; π rotation around any axis perpendicular to z if target
    // is -z. Otherwise: axis = target × z, angle = arccos(target.z / |target|).
    let n = target.norm();
    if n == 0.0 {
        return Quat::IDENTITY;
    }
    let unit = target * (1.0 / n);
    let dot_z = unit.z;
    if dot_z > 1.0 - 1e-12 {
        return Quat::IDENTITY;
    }
    if dot_z < -1.0 + 1e-12 {
        return Quat::from_axis_angle(Vec3::X, std::f64::consts::PI);
    }
    let axis = unit.cross(Vec3::Z);
    let angle = dot_z.acos();
    Quat::from_axis_angle(axis, angle)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    fn budget(max_evals: u64, seed: u64) -> Budget {
        Budget {
            max_evaluations: NonZeroU64::new(max_evals).expect("nonzero"),
            max_wall_time: None,
            seed,
        }
    }

    #[test]
    fn align_z_identity_for_plus_z() {
        let q = align_to_plus_z(Vec3::new(0.0, 0.0, 1.0));
        // Rotating Z by q should still be Z.
        let r = q.rotate(Vec3::Z);
        assert!((r.z - 1.0).abs() < 1e-12);
    }

    #[test]
    fn align_z_brings_arbitrary_vector_to_z() {
        let v = Vec3::new(1.0, 1.0, 1.0).normalized();
        let q = align_to_plus_z(v);
        let r = q.rotate(v);
        assert!(r.x.abs() < 1e-12);
        assert!(r.y.abs() < 1e-12);
        assert!((r.z - 1.0).abs() < 1e-12);
    }

    #[test]
    fn solves_cube() {
        // Cube enumeration: 27 base orientations × 12 in-plane steps = 324
        // candidates; 324² ≈ 105k pairs, but the cube passage is reached
        // far earlier in the iteration order. Budget at 100k is generous.
        let mut solver = FaceNormalPairs;
        let p = rupert_shapes::cube();
        let mut ec = EvalCounter::new(&p);
        let outcome = solver.solve(&p, &budget(110_000, 0), &mut ec);
        assert!(
            matches!(outcome, SolverOutcome::Found(_)),
            "expected cube solution, got {outcome:?}"
        );
    }

    #[test]
    fn deterministic_against_cube() {
        let p = rupert_shapes::cube();
        let mut ec_a = EvalCounter::new(&p);
        let mut ec_b = EvalCounter::new(&p);
        FaceNormalPairs.solve(&p, &budget(110_000, 0), &mut ec_a);
        FaceNormalPairs.solve(&p, &budget(110_000, 0), &mut ec_b);
        assert_eq!(ec_a.count(), ec_b.count());
    }
}
