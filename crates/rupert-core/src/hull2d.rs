//! 2D convex hull (Andrew's monotone chain) and convex polygon helpers.

use crate::error::CoreError;
use crate::projection::P2;

/// Returns the convex hull of `points` as a CCW-ordered polygon. Duplicate
/// or near-duplicate points are tolerated. Returns at least 3 points or an
/// error.
pub fn convex_hull(points: &[P2]) -> Result<Vec<P2>, CoreError> {
    let mut sorted: Vec<P2> = points.to_vec();
    // Sort by x ascending, then y ascending.
    sorted.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    sorted.dedup_by(|a, b| (a.x - b.x).abs() < 1e-15 && (a.y - b.y).abs() < 1e-15);
    if sorted.len() < 3 {
        return Err(CoreError::InsufficientPoints);
    }

    let mut lower: Vec<P2> = Vec::with_capacity(sorted.len());
    for &p in &sorted {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0
        {
            lower.pop();
        }
        lower.push(p);
    }

    let mut upper: Vec<P2> = Vec::with_capacity(sorted.len());
    for &p in sorted.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0
        {
            upper.pop();
        }
        upper.push(p);
    }

    lower.pop();
    upper.pop();
    lower.append(&mut upper);

    if lower.len() < 3 {
        return Err(CoreError::InsufficientPoints);
    }
    Ok(lower)
}

#[inline]
fn cross(o: P2, a: P2, b: P2) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// True if `p` lies strictly inside the CCW-oriented convex polygon.
/// Boundary points return `false`.
pub fn point_in_convex_strict(p: P2, hull: &[P2]) -> bool {
    if hull.len() < 3 {
        return false;
    }
    for i in 0..hull.len() {
        let a = hull[i];
        let b = hull[(i + 1) % hull.len()];
        if cross(a, b, p) <= 0.0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_hull() {
        let pts = vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0, 1.0),
            P2::new(0.0, 1.0),
            P2::new(0.5, 0.5),
        ];
        let h = convex_hull(&pts).expect("hull");
        assert_eq!(h.len(), 4);
        // CCW ordering: cross of consecutive edges is positive.
        for i in 0..h.len() {
            let a = h[i];
            let b = h[(i + 1) % h.len()];
            let c = h[(i + 2) % h.len()];
            assert!(cross(a, b, c) > 0.0);
        }
    }

    #[test]
    fn point_in_square_strict() {
        let h = vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0, 1.0),
            P2::new(0.0, 1.0),
        ];
        assert!(point_in_convex_strict(P2::new(0.5, 0.5), &h));
        assert!(!point_in_convex_strict(P2::new(0.0, 0.5), &h)); // boundary
        assert!(!point_in_convex_strict(P2::new(2.0, 0.5), &h)); // outside
    }

    #[test]
    fn rejects_collinear_input() {
        let pts = vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 1.0),
            P2::new(2.0, 2.0),
        ];
        let r = convex_hull(&pts);
        assert!(matches!(r, Err(CoreError::InsufficientPoints)));
    }

    #[test]
    fn random_input_all_points_inside_or_on() {
        // Pseudo-randomized but deterministic.
        let mut pts: Vec<P2> = Vec::new();
        let mut x: u64 = 0xdead_beef;
        for _ in 0..200 {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let fx = ((x >> 32) as u32 as f64) / (u32::MAX as f64);
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let fy = ((x >> 32) as u32 as f64) / (u32::MAX as f64);
            pts.push(P2::new(fx, fy));
        }
        let h = convex_hull(&pts).expect("hull");
        for &p in &pts {
            // Either strictly inside, or on the boundary (cross == 0 on at
            // least one edge). Outside is forbidden.
            let mut outside = false;
            for i in 0..h.len() {
                if cross(h[i], h[(i + 1) % h.len()], p) < -1e-12 {
                    outside = true;
                    break;
                }
            }
            assert!(!outside, "point {p:?} reported outside hull");
        }
    }
}
