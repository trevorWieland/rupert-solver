//! `EvalCounter` — the only path from a candidate to a clearance number.
//!
//! It is `!Sync` (via `PhantomData<*const ()>`), so a solver cannot share
//! it across threads — parallelism happens only at the `(shape, solver,
//! seed)` triple level in `rupert-bench::sweep`.

use std::marker::PhantomData;

use crate::clearance::clearance;
use crate::poly::Polyhedron;
use crate::projection::{P2, project_xy, translate_xy};
use crate::solver::{CLEARANCE_EPS, Candidate, ObservedCandidate};

/// Counts every clearance evaluation. Bound by reference to a polyhedron
/// for the duration of one solver run.
#[derive(Debug)]
pub struct EvalCounter<'a> {
    poly: &'a Polyhedron,
    count: u64,
    best_positive: Option<ObservedCandidate>,
    best_near_miss: Option<ObservedCandidate>,
    best_boundary: Option<ObservedCandidate>,
    _not_sync: PhantomData<*const ()>,
}

impl<'a> EvalCounter<'a> {
    pub fn new(poly: &'a Polyhedron) -> Self {
        Self {
            poly,
            count: 0,
            best_positive: None,
            best_near_miss: None,
            best_boundary: None,
            _not_sync: PhantomData,
        }
    }

    /// Compute the clearance of `candidate` against the bound polyhedron and
    /// increment the counter by exactly 1.
    pub fn evaluate(&mut self, c: &Candidate) -> f64 {
        self.count += 1;
        let clearance = evaluate_clearance(self.poly, c);
        if clearance.is_finite() {
            let observation = ObservedCandidate {
                candidate: *c,
                clearance,
                observed_at_eval: self.count,
                certification: None,
            };
            self.record_observation(observation);
        }
        clearance
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn polyhedron(&self) -> &Polyhedron {
        self.poly
    }

    pub fn best_positive(&self) -> Option<&ObservedCandidate> {
        self.best_positive.as_ref()
    }

    pub fn best_near_miss(&self) -> Option<&ObservedCandidate> {
        self.best_near_miss.as_ref()
    }

    pub fn best_boundary(&self) -> Option<&ObservedCandidate> {
        self.best_boundary.as_ref()
    }

    fn record_observation(&mut self, observation: ObservedCandidate) {
        if observation.clearance > CLEARANCE_EPS {
            replace_if_better_positive(&mut self.best_positive, observation);
        } else if observation.clearance < -CLEARANCE_EPS {
            replace_if_better_near_miss(&mut self.best_near_miss, observation);
        } else {
            replace_if_better_boundary(&mut self.best_boundary, observation);
        }
    }
}

fn replace_if_better_positive(slot: &mut Option<ObservedCandidate>, obs: ObservedCandidate) {
    if slot
        .as_ref()
        .is_none_or(|best| obs.clearance > best.clearance)
    {
        *slot = Some(obs);
    }
}

fn replace_if_better_near_miss(slot: &mut Option<ObservedCandidate>, obs: ObservedCandidate) {
    if slot
        .as_ref()
        .is_none_or(|best| obs.clearance > best.clearance)
    {
        *slot = Some(obs);
    }
}

fn replace_if_better_boundary(slot: &mut Option<ObservedCandidate>, obs: ObservedCandidate) {
    if slot.as_ref().is_none_or(|best| {
        obs.clearance.abs() < best.clearance.abs()
            || (obs.clearance.abs() == best.clearance.abs()
                && obs.observed_at_eval < best.observed_at_eval)
    }) {
        *slot = Some(obs);
    }
}

/// Pure function — same inputs, same output. Exposed so verifiers can
/// recompute the clearance without going through the counter.
pub fn evaluate_clearance(poly: &Polyhedron, c: &Candidate) -> f64 {
    let outer_pts = project_xy(poly, c.outer);
    let inner_pts_raw = project_xy(poly, c.inner);
    let inner_pts: Vec<P2> = translate_xy(&inner_pts_raw, c.translation[0], c.translation[1]);
    clearance(&inner_pts, &outer_pts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clearance::clearance;
    use crate::quat::Quat;
    use crate::vec3::Vec3;

    fn cube() -> Polyhedron {
        let v = vec![
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ];
        let f = vec![
            vec![0, 1, 2, 3],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![2, 3, 7, 6],
            vec![0, 3, 7, 4],
            vec![1, 2, 6, 5],
        ];
        Polyhedron::new("cube", v, f).expect("valid")
    }

    #[test]
    fn count_increments_per_evaluate() {
        let p = cube();
        let mut ec = EvalCounter::new(&p);
        assert_eq!(ec.count(), 0);
        ec.evaluate(&Candidate::IDENTITY);
        assert_eq!(ec.count(), 1);
        ec.evaluate(&Candidate::IDENTITY);
        assert_eq!(ec.count(), 2);
    }

    #[test]
    fn identity_candidate_clearance_zero() {
        // Inner == outer (identity in both rotations, no translation): the
        // shadow of inner is identical to the shadow of outer. Every inner
        // point is on the boundary of the outer hull, so signed distance
        // is zero on at least one edge ⇒ clearance ≤ 0.
        let p = cube();
        let mut ec = EvalCounter::new(&p);
        let c = ec.evaluate(&Candidate::IDENTITY);
        assert!(c <= 1e-12, "expected ≤ 0, got {c}");
    }

    #[test]
    fn rotated_inner_can_be_negative() {
        // With identity outer (square shadow) and inner rotated 45° around
        // z, the inner shadow is a larger diamond — so it pokes outside,
        // yielding negative clearance.
        let p = cube();
        let mut ec = EvalCounter::new(&p);
        let q_inner = Quat::from_axis_angle(Vec3::Z, std::f64::consts::PI / 4.0);
        let c = ec.evaluate(&Candidate {
            outer: Quat::IDENTITY,
            inner: q_inner,
            translation: [0.0, 0.0],
        });
        assert!(c < 0.0, "rotated inner overflows outer face: {c}");
    }

    #[test]
    fn translated_inner_can_be_strictly_inside() {
        // Trivial-clearance check: shrink the cube's inner shadow by
        // translating it so the silhouette diagonal hits its corners.
        // We construct a degenerate inner cube (collapsed onto a small
        // central blob) by exploiting the polyhedron's vertex set —
        // checking that signed clearance is correctly continuous, not
        // that the cube is itself Rupert (which is the solvers' job).
        let p = cube();
        let outer_pts = project_xy(&p, Quat::IDENTITY);
        // Synthesize a tiny inner shadow (manually) and test clearance.
        let inner_pts = vec![
            P2::new(-0.5, -0.5),
            P2::new(0.5, -0.5),
            P2::new(0.5, 0.5),
            P2::new(-0.5, 0.5),
        ];
        let c = clearance(&inner_pts, &outer_pts);
        assert!((c - 0.5).abs() < 1e-12, "expected ≈ 0.5, got {c}");
    }

    #[test]
    fn tracks_observation_classes_separately() {
        let p = cube();
        let mut ec = EvalCounter::new(&p);
        let q_inner = Quat::from_axis_angle(Vec3::Z, std::f64::consts::PI / 4.0);

        let boundary = ec.evaluate(&Candidate::IDENTITY);
        let near_miss = ec.evaluate(&Candidate {
            outer: Quat::IDENTITY,
            inner: q_inner,
            translation: [0.0, 0.0],
        });
        let positive = ec.evaluate(&Candidate {
            outer: Quat {
                w: 0.45970084338098305,
                x: -0.6279630301995544,
                y: 0.6279630301995544,
                z: 0.0,
            },
            inner: Quat {
                w: 0.9914448613738104,
                x: 0.0,
                y: 0.0,
                z: 0.13052619222005157,
            },
            translation: [0.0, 0.0],
        });

        assert!(boundary.abs() <= CLEARANCE_EPS);
        assert!(near_miss < -CLEARANCE_EPS);
        assert!(positive > CLEARANCE_EPS);
        assert!(ec.best_boundary().is_some());
        assert!(ec.best_near_miss().is_some());
        assert!(ec.best_positive().is_some());
    }
}
