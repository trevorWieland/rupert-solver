//! Interval-arithmetic certification (`CertMethod::IntervalSnap`).
//!
//! Given a `Solution` (an f64 candidate found by some solver) and a
//! `Polyhedron` carrying `exact_vertices`, certify that the candidate
//! is a true Rupert passage by:
//!
//! 1. Evaluate every exact vertex via [`rupert_core::Expr::eval_interval`]
//!    to get a tight interval enclosure of the 3D vertex.
//! 2. Apply the f64 outer/inner quaternion rotations as
//!    interval-arithmetic matrix-vector products. The quaternion is a
//!    singleton (no uncertainty), so the result's width is dominated
//!    by the input vertex interval.
//! 3. Project to xy (drop z); add the f64 translation.
//! 4. Run [`rupert_core::hull2d_interval::convex_hull_interval_certified`]
//!    on the outer interval-projected points. Reject if `Ambiguous`.
//! 5. For each inner interval-vertex, compute the interval signed
//!    distance to every outer hull edge. The minimum over all
//!    (inner_vertex, outer_edge) pairs is the certified clearance.
//!    If the lower bound is strictly positive, the candidate is a
//!    true Rupert passage.
//!
//! ## What `IntervalSnap` certifies vs `F64Epsilon`
//!
//! - **F64Epsilon**: f64 recomputation is positive. Catches obvious
//!   solver lies, doesn't catch f64 rounding error in pathological
//!   configurations.
//! - **IntervalSnap**: a rigorous interval-arithmetic recomputation
//!   gives a strictly-positive *lower bound* on clearance. The true
//!   clearance is provably positive — Rupert passage is real.
//!
//! Available only for polyhedra with `exact_vertices` populated. v0.1
//! `with_exact` constructors cover all 8 builtins.

use inari::Interval;
use rupert_core::hull2d_interval::{
    HullCertResult, IntervalP2, convex_hull_interval_certified, point_in_interval_polygon_strict,
};
use rupert_core::{CertMethod, Certification, ExactVec3, P2, Polyhedron, Quat, Solution};

use crate::VerifyError;

/// Certify a solution via interval arithmetic. Returns
/// `Certification { method: IntervalSnap, .. }` if and only if every
/// step succeeds with a strictly-positive lower bound on clearance.
pub fn certify_interval(
    solution: &Solution,
    poly: &Polyhedron,
) -> Result<Certification, VerifyError> {
    let exact = poly
        .exact_vertices
        .as_ref()
        .ok_or(VerifyError::NoExactVertices)?;

    // Project each vertex via the f64 quaternion-rotation as interval
    // arithmetic. Output is the 2D interval-shadow.
    let outer_pts = project_to_interval_xy(exact, &solution.candidate.outer, [0.0, 0.0]);
    let inner_pts = project_to_interval_xy(
        exact,
        &solution.candidate.inner,
        solution.candidate.translation,
    );

    // f64 outer projection, for the combinatorial-precommit hull
    // algorithm. The inner shadow doesn't need an f64 hull (we only
    // certify each inner vertex against the outer hull).
    let outer_f64 = project_to_f64_xy(exact, &solution.candidate.outer, [0.0, 0.0]);

    // Certify the outer hull's combinatorial structure under intervals.
    let HullCertResult::Forced(outer_hull_indices) =
        convex_hull_interval_certified(&outer_f64, &outer_pts)
    else {
        return Err(VerifyError::HullCombinatoricsAmbiguous);
    };

    // Build interval polygon for the outer hull.
    let outer_hull: Vec<IntervalP2> = outer_hull_indices.iter().map(|&i| outer_pts[i]).collect();

    // For each inner vertex, certify it's strictly inside the outer
    // hull AND compute its certified signed distance to the polygon.
    let mut clearance_lo = f64::INFINITY;
    let mut clearance_hi = f64::INFINITY;
    for inner_p in &inner_pts {
        match point_in_interval_polygon_strict(*inner_p, &outer_hull) {
            rupert_core::hull2d_interval::InsidePred::DefinitelyInside => {}
            _ => return Err(VerifyError::InnerNotStrictlyInside),
        }
        let d = signed_distance_to_polygon_interval(*inner_p, &outer_hull);
        if d.inf() < clearance_lo {
            clearance_lo = d.inf();
        }
        if d.sup() < clearance_hi {
            clearance_hi = d.sup();
        }
    }

    if !clearance_lo.is_finite() || clearance_lo <= 0.0 {
        return Err(VerifyError::NotStrictlyPositive(
            clearance_lo,
            crate::F64_EPS,
        ));
    }

    Ok(Certification {
        method: CertMethod::IntervalSnap,
        clearance_lo,
        clearance_hi,
    })
}

/// Apply an f64 quaternion rotation to each `ExactVec3`, producing 2D
/// `IntervalP2` (drop z, add f64 translation in interval form).
fn project_to_interval_xy(exact: &[ExactVec3], q: &Quat, translation: [f64; 2]) -> Vec<IntervalP2> {
    // Quaternion rotation matrix in f64. Treated as singleton intervals
    // for the matrix-vector product.
    let m = quat_rotation_matrix(q);
    let tx = singleton(translation[0]);
    let ty = singleton(translation[1]);
    exact
        .iter()
        .map(|v| {
            let [vx, vy, vz] = v.eval_interval();
            // Output x = m[0][0]*vx + m[0][1]*vy + m[0][2]*vz + tx
            let x =
                singleton(m[0][0]) * vx + singleton(m[0][1]) * vy + singleton(m[0][2]) * vz + tx;
            let y =
                singleton(m[1][0]) * vx + singleton(m[1][1]) * vy + singleton(m[1][2]) * vz + ty;
            IntervalP2::new(x, y)
        })
        .collect()
}

fn project_to_f64_xy(exact: &[ExactVec3], q: &Quat, translation: [f64; 2]) -> Vec<P2> {
    exact
        .iter()
        .map(|v| {
            let f = v.eval_f64();
            let r = q.rotate(f);
            P2::new(r.x + translation[0], r.y + translation[1])
        })
        .collect()
}

/// 3×3 rotation matrix from a unit quaternion (right-handed).
/// Standard formula. Caller ensures `q` is unit-norm.
fn quat_rotation_matrix(q: &Quat) -> [[f64; 3]; 3] {
    let w = q.w;
    let x = q.x;
    let y = q.y;
    let z = q.z;
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
        ],
        [
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
        ],
        [
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ]
}

fn singleton(x: f64) -> Interval {
    Interval::try_from((x, x)).unwrap_or(Interval::ENTIRE)
}

/// Interval-arithmetic signed distance from a point to a CCW polygon.
/// Positive inside, negative outside. The minimum over all edges of
/// the signed perpendicular distance.
fn signed_distance_to_polygon_interval(p: IntervalP2, polygon: &[IntervalP2]) -> Interval {
    if polygon.len() < 3 {
        return Interval::ENTIRE;
    }
    let mut min_dist: Option<Interval> = None;
    let n = polygon.len();
    for i in 0..n {
        let a = polygon[i];
        let b = polygon[(i + 1) % n];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len_sq = dx * dx + dy * dy;
        let len = len_sq.sqrt();
        // Inward normal for CCW polygon: (-dy, dx) / len.
        // Signed perpendicular distance = ((p - a) · inward_normal) / len
        //                               = (-dy*(p.x - a.x) + dx*(p.y - a.y)) / len.
        let signed = (dx * (p.y - a.y) - dy * (p.x - a.x)) / len;
        match min_dist {
            None => min_dist = Some(signed),
            Some(prev) => {
                // interval min via convex_hull (inari's union):
                let new_min_lo = prev.inf().min(signed.inf());
                let new_min_hi = prev.sup().min(signed.sup());
                min_dist = Interval::try_from((new_min_lo, new_min_hi)).ok();
            }
        }
    }
    min_dist.unwrap_or(Interval::ENTIRE)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use rupert_core::{Budget, Candidate, EvalCounter, Solver, SolverOutcome};
    use rupert_solvers::FaceNormalPairs;

    use super::*;

    fn budget(max_evals: u64, seed: u64) -> Budget {
        Budget {
            max_evaluations: NonZeroU64::new(max_evals).expect("nonzero"),
            max_wall_time: None,
            seed,
        }
    }

    #[test]
    fn certifies_genuine_cube_passage_via_interval() {
        let poly = rupert_shapes::cube();
        assert!(poly.exact_vertices.is_some(), "cube has exact_vertices");
        let mut solver = FaceNormalPairs;
        let mut ec = EvalCounter::new(&poly);
        let outcome = solver.solve(&poly, &budget(110_000, 0), &mut ec);
        let SolverOutcome::Found(sol) = outcome else {
            unreachable!("FaceNormalPairs failed to solve cube");
        };
        let cert = certify_interval(&sol, &poly).expect("interval cert");
        assert_eq!(cert.method, CertMethod::IntervalSnap);
        assert!(cert.clearance_lo > 0.0, "lo = {}", cert.clearance_lo);
        assert!(cert.clearance_hi >= cert.clearance_lo);
    }

    #[test]
    fn rejects_identity_via_interval() {
        // Identity solution → outer and inner shadows coincide →
        // not strictly inside (touching boundary).
        let poly = rupert_shapes::cube();
        let identity_solution = Solution {
            candidate: Candidate::IDENTITY,
            clearance: 0.0,
            found_at_eval: 0,
            certification: None,
        };
        let r = certify_interval(&identity_solution, &poly);
        assert!(r.is_err(), "expected rejection, got {r:?}");
    }

    #[test]
    fn requires_exact_vertices() {
        let mut poly = rupert_shapes::cube();
        poly.exact_vertices = None;
        let identity_solution = Solution {
            candidate: Candidate::IDENTITY,
            clearance: 0.0,
            found_at_eval: 0,
            certification: None,
        };
        let r = certify_interval(&identity_solution, &poly);
        assert!(matches!(r, Err(VerifyError::NoExactVertices)));
    }
}
