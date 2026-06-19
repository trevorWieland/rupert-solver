//! Exact-rational certification for rational-coordinate shapes.

use malachite::Rational;
use rupert_core::{CertMethod, Certification, ExactVec3, P2, Polyhedron, Solution};

use crate::{F64_EPS, VerifyError};

const SNAP_DENOM: i128 = 1_i128 << 32;

#[derive(Debug, Clone)]
struct RP2 {
    x: Rational,
    y: Rational,
}

pub fn certify_exact(solution: &Solution, poly: &Polyhedron) -> Result<Certification, VerifyError> {
    let exact = poly
        .exact_vertices
        .as_ref()
        .ok_or(VerifyError::NoExactVertices)?;
    let outer_f64 = project_to_f64_xy(exact, &solution.candidate.outer, [0.0, 0.0]);
    let hull_indices = hull_indices(&outer_f64).ok_or(VerifyError::HullCombinatoricsAmbiguous)?;
    let outer = project_to_rational_xy(exact, &solution.candidate.outer, [0.0, 0.0])?;
    let inner = project_to_rational_xy(
        exact,
        &solution.candidate.inner,
        solution.candidate.translation,
    )?;
    let hull: Vec<RP2> = hull_indices.iter().map(|&i| outer[i].clone()).collect();
    if !inner
        .iter()
        .all(|p| point_in_rational_polygon_strict(p, &hull))
    {
        return Err(VerifyError::InnerNotStrictlyInside);
    }
    let clearance = rupert_core::evaluate_clearance(poly, &solution.candidate);
    if !clearance.is_finite() || clearance <= F64_EPS {
        return Err(VerifyError::NotStrictlyPositive(clearance, F64_EPS));
    }
    Ok(Certification {
        method: CertMethod::ExactRational,
        clearance_lo: clearance,
        clearance_hi: clearance,
    })
}

fn project_to_rational_xy(
    exact: &[ExactVec3],
    q: &rupert_core::Quat,
    translation: [f64; 2],
) -> Result<Vec<RP2>, VerifyError> {
    let matrix = quat_rotation_matrix(q);
    let tx = rat_from_f64(translation[0])?;
    let ty = rat_from_f64(translation[1])?;
    exact
        .iter()
        .map(|v| {
            let [vx, vy, vz] = v.eval_rational().ok_or(VerifyError::NoExactVertices)?;
            let x = rat_from_f64(matrix[0][0])? * vx.clone()
                + rat_from_f64(matrix[0][1])? * vy.clone()
                + rat_from_f64(matrix[0][2])? * vz.clone()
                + tx.clone();
            let y = rat_from_f64(matrix[1][0])? * vx
                + rat_from_f64(matrix[1][1])? * vy
                + rat_from_f64(matrix[1][2])? * vz
                + ty.clone();
            Ok(RP2 { x, y })
        })
        .collect()
}

fn project_to_f64_xy(exact: &[ExactVec3], q: &rupert_core::Quat, translation: [f64; 2]) -> Vec<P2> {
    exact
        .iter()
        .map(|v| {
            let f = v.eval_f64();
            let r = q.rotate(f);
            P2::new(r.x + translation[0], r.y + translation[1])
        })
        .collect()
}

fn rat_from_f64(x: f64) -> Result<Rational, VerifyError> {
    if !x.is_finite() {
        return Err(VerifyError::NonFiniteClearance);
    }
    let n = (x * (SNAP_DENOM as f64)).round() as i128;
    Ok(Rational::from_signeds(n, SNAP_DENOM))
}

fn quat_rotation_matrix(q: &rupert_core::Quat) -> [[f64; 3]; 3] {
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

fn hull_indices(points: &[P2]) -> Option<Vec<usize>> {
    let mut sorted: Vec<(usize, P2)> = points.iter().copied().enumerate().collect();
    sorted.sort_by(|a, b| {
        a.1.x
            .partial_cmp(&b.1.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.1.y
                    .partial_cmp(&b.1.y)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.0.cmp(&b.0))
    });
    sorted.dedup_by(|a, b| (a.1.x - b.1.x).abs() < 1e-15 && (a.1.y - b.1.y).abs() < 1e-15);
    if sorted.len() < 3 {
        return None;
    }
    let mut lower: Vec<(usize, P2)> = Vec::new();
    for &p in &sorted {
        while lower.len() >= 2
            && cross_f64(lower[lower.len() - 2].1, lower[lower.len() - 1].1, p.1) <= 0.0
        {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<(usize, P2)> = Vec::new();
    for &p in sorted.iter().rev() {
        while upper.len() >= 2
            && cross_f64(upper[upper.len() - 2].1, upper[upper.len() - 1].1, p.1) <= 0.0
        {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.append(&mut upper);
    (lower.len() >= 3).then(|| lower.into_iter().map(|(i, _)| i).collect())
}

fn cross_f64(o: P2, a: P2, b: P2) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn point_in_rational_polygon_strict(p: &RP2, hull: &[RP2]) -> bool {
    if hull.len() < 3 {
        return false;
    }
    let zero = Rational::from(0u32);
    for i in 0..hull.len() {
        if cross_rat(&hull[i], &hull[(i + 1) % hull.len()], p) <= zero {
            return false;
        }
    }
    true
}

fn cross_rat(a: &RP2, b: &RP2, p: &RP2) -> Rational {
    (b.x.clone() - a.x.clone()) * (p.y.clone() - a.y.clone())
        - (b.y.clone() - a.y.clone()) * (p.x.clone() - a.x.clone())
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
    fn certifies_genuine_cube_passage_exactly() {
        let poly = rupert_shapes::cube();
        let mut solver = FaceNormalPairs;
        let mut ec = EvalCounter::new(&poly);
        let outcome = solver.solve(&poly, &budget(110_000, 0), &mut ec);
        let SolverOutcome::Found { solution: sol, .. } = outcome else {
            unreachable!("FaceNormalPairs failed to solve cube");
        };
        let cert = certify_exact(&sol, &poly).expect("exact cert");
        assert_eq!(cert.method, CertMethod::ExactRational);
    }

    #[test]
    fn rejects_identity_exactly() {
        let poly = rupert_shapes::cube();
        let sol = Solution {
            candidate: Candidate::IDENTITY,
            clearance: 0.0,
            found_at_eval: 0,
            certification: None,
        };
        assert!(certify_exact(&sol, &poly).is_err());
    }
}
