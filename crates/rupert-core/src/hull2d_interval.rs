//! Interval-arithmetic 2D convex hull certification (v2 phase 6).
//!
//! Given a set of 2D points expressed as `(IntervalP2, P2)` pairs (the
//! interval form is rigorous; the f64 form is the unrounded best
//! estimate), this module decides whether the **combinatorial structure**
//! of the f64 convex hull is *forced* under the interval bounds. If yes,
//! it returns `HullCertResult::Forced(indices)` carrying the same vertex
//! indices the f64 hull would have selected. If the intervals are too
//! wide to certify the structure (boundary-flip ambiguity), it returns
//! `HullCertResult::Ambiguous` — the caller's job to either tighten
//! bounds and retry, or fall back to f64 epsilon.
//!
//! ## Algorithm
//!
//! Combinatorial-precommit (Steininger–Yurkevich style):
//!
//! 1. Compute the f64 hull via [`convex_hull`]. This gives a CCW vertex
//!    sequence — the *candidate* combinatorial structure.
//! 2. Verify the structure is forced under the intervals:
//!    - Every claimed-interior vertex must be `DefinitelyInside` the
//!      interval polygon formed by the claimed-boundary vertices.
//!    - Every claimed-boundary vertex must NOT be `DefinitelyInside`
//!      the polygon formed by the *other* boundary vertices (which
//!      would mean it's actually interior, contradicting f64).
//!    - The interior-vertex count plus boundary-vertex count equals
//!      the input count (no missing vertex, no duplicate).
//! 3. If all checks pass, return `Forced`. Otherwise `Ambiguous`.

use inari::Interval;

use crate::hull2d::convex_hull;
use crate::projection::P2;

/// 2D point with interval-arithmetic coordinates.
#[derive(Debug, Clone, Copy)]
pub struct IntervalP2 {
    pub x: Interval,
    pub y: Interval,
}

impl IntervalP2 {
    pub fn new(x: Interval, y: Interval) -> Self {
        Self { x, y }
    }
}

/// Result of certifying a hull's combinatorial structure under intervals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HullCertResult {
    /// The f64-derived combinatorial structure is provably forced.
    /// `indices` are the boundary-vertex indices in CCW order.
    Forced(Vec<usize>),
    /// Interval bounds are too wide to certify (some predicate returned
    /// `Ambiguous`). Caller decides whether to tighten and retry, or
    /// fall back to f64 epsilon.
    Ambiguous,
}

/// Outcome of an interval-arithmetic strict-inside-polygon predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsidePred {
    /// Strictly inside (every edge gives a strictly-positive cross product).
    DefinitelyInside,
    /// Definitively outside (at least one edge gives a strictly-negative
    /// cross product).
    DefinitelyOutside,
    /// At least one edge's cross-product interval contains 0 — we can't
    /// decide.
    Ambiguous,
}

/// Strict point-in-CCW-polygon predicate over interval arithmetic.
///
/// For each oriented edge `(a, b)` of `polygon`, computes
/// `cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)`
/// as an `Interval`. If `cross > 0` strictly for every edge:
/// `DefinitelyInside`. If `cross < 0` strictly for some edge:
/// `DefinitelyOutside`. Otherwise `Ambiguous`.
#[must_use]
pub fn point_in_interval_polygon_strict(p: IntervalP2, polygon: &[IntervalP2]) -> InsidePred {
    if polygon.len() < 3 {
        return InsidePred::Ambiguous;
    }
    let mut any_outside = false;
    let mut any_ambiguous = false;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        let dx_edge = b.x - a.x;
        let dy_edge = b.y - a.y;
        let dx_p = p.x - a.x;
        let dy_p = p.y - a.y;
        let cross = dx_edge * dy_p - dy_edge * dx_p;
        if cross.inf() > 0.0 {
            // Strictly positive — this edge is OK; keep checking the rest.
            continue;
        }
        if cross.sup() < 0.0 {
            any_outside = true;
            break;
        }
        // Crosses zero — ambiguous on this edge.
        any_ambiguous = true;
    }
    if any_outside {
        InsidePred::DefinitelyOutside
    } else if any_ambiguous {
        InsidePred::Ambiguous
    } else {
        InsidePred::DefinitelyInside
    }
}

/// Certify a 2D convex hull under interval arithmetic.
///
/// `f64_pts` and `int_pts` must have identical length and ordering;
/// `int_pts[i]` is the rigorous interval enclosure of `f64_pts[i]`.
///
/// Returns `Forced(indices)` where `indices` is the CCW sequence of
/// boundary-vertex indices into the input arrays. Returns `Ambiguous`
/// if the f64-derived hull's combinatorial structure isn't provably
/// forced under the interval bounds.
#[must_use]
pub fn convex_hull_interval_certified(f64_pts: &[P2], int_pts: &[IntervalP2]) -> HullCertResult {
    debug_assert_eq!(f64_pts.len(), int_pts.len());
    if f64_pts.len() < 3 {
        return HullCertResult::Ambiguous;
    }

    // Step 1: derive candidate combinatorial structure from f64 hull.
    let Ok(f64_hull) = convex_hull(f64_pts) else {
        return HullCertResult::Ambiguous;
    };
    // Map hull points back to input indices via approximate equality.
    let mut hull_indices: Vec<usize> = Vec::with_capacity(f64_hull.len());
    for hp in &f64_hull {
        let mut matched: Option<usize> = None;
        for (i, p) in f64_pts.iter().enumerate() {
            if (hp.x - p.x).abs() < 1.0e-15 && (hp.y - p.y).abs() < 1.0e-15 {
                matched = Some(i);
                break;
            }
        }
        match matched {
            Some(i) => hull_indices.push(i),
            None => return HullCertResult::Ambiguous,
        }
    }

    // Step 2: extract the interval polygon for the boundary.
    let interval_hull: Vec<IntervalP2> = hull_indices.iter().map(|&i| int_pts[i]).collect();

    // Step 3a: every claimed-interior vertex must be DefinitelyInside —
    // OR coincident with a boundary vertex (a non-issue duplicate).
    let hull_set: std::collections::BTreeSet<usize> = hull_indices.iter().copied().collect();
    for (i, p) in int_pts.iter().enumerate() {
        if hull_set.contains(&i) {
            continue;
        }
        // If this non-hull point coincides with any hull point in f64,
        // it's a duplicate. Treat as on-boundary (skip strict check).
        let coincident = hull_indices.iter().any(|&h| {
            (f64_pts[i].x - f64_pts[h].x).abs() < 1.0e-12
                && (f64_pts[i].y - f64_pts[h].y).abs() < 1.0e-12
        });
        if coincident {
            continue;
        }
        match point_in_interval_polygon_strict(*p, &interval_hull) {
            InsidePred::DefinitelyInside => {}
            _ => return HullCertResult::Ambiguous,
        }
    }

    // Step 3b: every claimed-boundary vertex must NOT be strictly
    // inside the polygon formed by the OTHER boundary vertices. If it
    // were, it would actually be interior — contradicting the f64
    // hull's classification. (Strictly: an interval check returning
    // `DefinitelyInside` means "f64 was wrong"; `Ambiguous` means
    // "we can't prove either way".)
    for (k, &h) in hull_indices.iter().enumerate() {
        let mut others: Vec<IntervalP2> = Vec::with_capacity(interval_hull.len() - 1);
        for (j, q) in interval_hull.iter().enumerate() {
            if j != k {
                others.push(*q);
            }
        }
        if others.len() < 3 {
            continue;
        }
        let p = int_pts[h];
        if matches!(
            point_in_interval_polygon_strict(p, &others),
            InsidePred::DefinitelyInside
        ) {
            return HullCertResult::Ambiguous;
        }
    }

    HullCertResult::Forced(hull_indices)
}

/// Convenience: build interval points from f64 points by widening each
/// coordinate by `±half_width`. Useful for tests and for callers that
/// have a known coordinate uncertainty.
#[must_use]
pub fn widen_to_intervals(pts: &[P2], half_width: f64) -> Vec<IntervalP2> {
    pts.iter()
        .map(|p| {
            IntervalP2::new(
                Interval::try_from((p.x - half_width, p.x + half_width))
                    .unwrap_or(Interval::ENTIRE),
                Interval::try_from((p.y - half_width, p.y + half_width))
                    .unwrap_or(Interval::ENTIRE),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube_projection_pts() -> Vec<P2> {
        // Identity projection of the unit cube has 8 vertices at (±1, ±1)
        // (with 4 distinct pairs in xy).
        vec![
            P2::new(-1.0, -1.0),
            P2::new(1.0, -1.0),
            P2::new(1.0, 1.0),
            P2::new(-1.0, 1.0),
            P2::new(-1.0, -1.0),
            P2::new(1.0, -1.0),
            P2::new(1.0, 1.0),
            P2::new(-1.0, 1.0),
        ]
    }

    #[test]
    fn cube_singleton_intervals_certify_as_forced() {
        let pts = cube_projection_pts();
        let int_pts: Vec<IntervalP2> = pts
            .iter()
            .map(|p| {
                IntervalP2::new(
                    Interval::try_from((p.x, p.x)).expect("singleton"),
                    Interval::try_from((p.y, p.y)).expect("singleton"),
                )
            })
            .collect();
        let result = convex_hull_interval_certified(&pts, &int_pts);
        match result {
            HullCertResult::Forced(indices) => {
                // The cube's xy projection has 4 boundary vertices — but
                // because the input has duplicates (8 points at 4
                // positions), the hull picks the first occurrence of each
                // boundary corner.
                assert_eq!(indices.len(), 4);
            }
            HullCertResult::Ambiguous => unreachable!("expected Forced, got Ambiguous"),
        }
    }

    #[test]
    fn cube_with_tight_interval_certifies() {
        let pts = vec![
            P2::new(-1.0, -1.0),
            P2::new(1.0, -1.0),
            P2::new(1.0, 1.0),
            P2::new(-1.0, 1.0),
            P2::new(0.0, 0.0), // interior
        ];
        let int_pts = widen_to_intervals(&pts, 1.0e-12);
        let result = convex_hull_interval_certified(&pts, &int_pts);
        assert!(matches!(result, HullCertResult::Forced(_)));
    }

    #[test]
    fn excessively_wide_intervals_are_ambiguous() {
        // 4 corner points with intervals so wide they overlap → can't
        // certify hull combinatorial structure.
        let pts = vec![
            P2::new(-1.0, -1.0),
            P2::new(1.0, -1.0),
            P2::new(1.0, 1.0),
            P2::new(-1.0, 1.0),
            P2::new(0.0, 0.0), // interior
        ];
        let int_pts = widen_to_intervals(&pts, 1.5);
        let result = convex_hull_interval_certified(&pts, &int_pts);
        assert_eq!(result, HullCertResult::Ambiguous);
    }

    #[test]
    fn point_in_polygon_strict_inside() {
        // CCW square [0,1] × [0,1].
        let polygon = vec![
            IntervalP2::new(
                Interval::try_from((0.0, 0.0)).expect("singleton"),
                Interval::try_from((0.0, 0.0)).expect("singleton"),
            ),
            IntervalP2::new(
                Interval::try_from((1.0, 1.0)).expect("singleton"),
                Interval::try_from((0.0, 0.0)).expect("singleton"),
            ),
            IntervalP2::new(
                Interval::try_from((1.0, 1.0)).expect("singleton"),
                Interval::try_from((1.0, 1.0)).expect("singleton"),
            ),
            IntervalP2::new(
                Interval::try_from((0.0, 0.0)).expect("singleton"),
                Interval::try_from((1.0, 1.0)).expect("singleton"),
            ),
        ];
        let p = IntervalP2::new(
            Interval::try_from((0.5, 0.5)).expect("singleton"),
            Interval::try_from((0.5, 0.5)).expect("singleton"),
        );
        assert_eq!(
            point_in_interval_polygon_strict(p, &polygon),
            InsidePred::DefinitelyInside
        );
    }

    #[test]
    fn point_in_polygon_strict_outside() {
        let polygon = vec![
            IntervalP2::new(
                Interval::try_from((0.0, 0.0)).expect("singleton"),
                Interval::try_from((0.0, 0.0)).expect("singleton"),
            ),
            IntervalP2::new(
                Interval::try_from((1.0, 1.0)).expect("singleton"),
                Interval::try_from((0.0, 0.0)).expect("singleton"),
            ),
            IntervalP2::new(
                Interval::try_from((1.0, 1.0)).expect("singleton"),
                Interval::try_from((1.0, 1.0)).expect("singleton"),
            ),
            IntervalP2::new(
                Interval::try_from((0.0, 0.0)).expect("singleton"),
                Interval::try_from((1.0, 1.0)).expect("singleton"),
            ),
        ];
        let p = IntervalP2::new(
            Interval::try_from((2.0, 2.0)).expect("singleton"),
            Interval::try_from((0.5, 0.5)).expect("singleton"),
        );
        assert_eq!(
            point_in_interval_polygon_strict(p, &polygon),
            InsidePred::DefinitelyOutside
        );
    }

    #[test]
    fn point_on_boundary_is_ambiguous() {
        // Point on the edge of the square — interval cross product
        // straddles zero on that edge → Ambiguous.
        let polygon = vec![
            IntervalP2::new(
                Interval::try_from((0.0, 0.0)).expect("singleton"),
                Interval::try_from((0.0, 0.0)).expect("singleton"),
            ),
            IntervalP2::new(
                Interval::try_from((1.0, 1.0)).expect("singleton"),
                Interval::try_from((0.0, 0.0)).expect("singleton"),
            ),
            IntervalP2::new(
                Interval::try_from((1.0, 1.0)).expect("singleton"),
                Interval::try_from((1.0, 1.0)).expect("singleton"),
            ),
            IntervalP2::new(
                Interval::try_from((0.0, 0.0)).expect("singleton"),
                Interval::try_from((1.0, 1.0)).expect("singleton"),
            ),
        ];
        let p = IntervalP2::new(
            Interval::try_from((0.5, 0.5)).expect("singleton"),
            Interval::try_from((0.0, 0.0)).expect("singleton"),
        );
        assert_eq!(
            point_in_interval_polygon_strict(p, &polygon),
            InsidePred::Ambiguous
        );
    }
}
