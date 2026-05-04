//! Signed clearance: positive when every inner point is strictly inside the
//! outer hull; negative when at least one inner point is outside or on the
//! boundary; zero on the boundary.

use crate::hull2d::convex_hull;
use crate::projection::P2;

/// Returns the signed clearance of `inner_points` against the convex hull
/// of `outer_points`. The metric is:
///
/// `clearance = min over i of distance(inner_points[i], outer_hull_complement)`
///
/// where the distance is signed (positive inside the hull, negative outside).
///
/// A value > 0 means every inner point is strictly inside the outer hull.
pub fn clearance(inner_points: &[P2], outer_points: &[P2]) -> f64 {
    let Ok(outer_hull) = convex_hull(outer_points) else {
        return f64::NEG_INFINITY;
    };
    let mut min_signed = f64::INFINITY;
    for p in inner_points {
        let d = signed_distance_to_polygon(*p, &outer_hull);
        if d < min_signed {
            min_signed = d;
        }
    }
    min_signed
}

/// Signed distance from a point to a CCW-oriented convex polygon.
/// Positive = inside, negative = outside, zero = on the boundary.
pub fn signed_distance_to_polygon(p: P2, poly: &[P2]) -> f64 {
    if poly.len() < 3 {
        return f64::NEG_INFINITY;
    }
    // Compute the signed perpendicular distance to each oriented edge,
    // taking the *minimum* (most-constraining edge) — this gives the
    // signed inscribed-distance.
    let mut min_dist = f64::INFINITY;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            continue;
        }
        // Inward normal for CCW polygon: rotate edge by +90°: (-dy, dx) / len.
        let nx = -dy / len;
        let ny = dx / len;
        let signed = (p.x - a.x) * nx + (p.y - a.y) * ny;
        if signed < min_dist {
            min_dist = signed;
        }
    }
    min_dist
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<P2> {
        vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0, 1.0),
            P2::new(0.0, 1.0),
        ]
    }

    #[test]
    fn point_at_center_distance_half() {
        let s = square();
        let d = signed_distance_to_polygon(P2::new(0.5, 0.5), &s);
        assert!((d - 0.5).abs() < 1e-12);
    }

    #[test]
    fn point_on_boundary_zero() {
        let s = square();
        let d = signed_distance_to_polygon(P2::new(0.0, 0.5), &s);
        assert!(d.abs() < 1e-12);
    }

    #[test]
    fn point_outside_negative() {
        let s = square();
        let d = signed_distance_to_polygon(P2::new(-0.5, 0.5), &s);
        assert!(d < 0.0);
    }

    #[test]
    fn clearance_inner_inside_outer() {
        let outer = vec![
            P2::new(-2.0, -2.0),
            P2::new(2.0, -2.0),
            P2::new(2.0, 2.0),
            P2::new(-2.0, 2.0),
        ];
        let inner = vec![
            P2::new(-1.0, -1.0),
            P2::new(1.0, -1.0),
            P2::new(1.0, 1.0),
            P2::new(-1.0, 1.0),
        ];
        let c = clearance(&inner, &outer);
        // Inner square is 1x1 inside a 2x2 — minimum margin is 1 unit.
        assert!((c - 1.0).abs() < 1e-12);
    }

    #[test]
    fn clearance_inner_overlaps_outer_boundary_zero_or_negative() {
        let outer = vec![
            P2::new(0.0, 0.0),
            P2::new(2.0, 0.0),
            P2::new(2.0, 2.0),
            P2::new(0.0, 2.0),
        ];
        // Inner has a corner ON the outer boundary.
        let inner = vec![P2::new(0.0, 1.0), P2::new(1.0, 1.0), P2::new(1.0, 1.5)];
        let c = clearance(&inner, &outer);
        assert!(c <= 1e-12, "expected ≤ 0, got {c}");
    }
}
