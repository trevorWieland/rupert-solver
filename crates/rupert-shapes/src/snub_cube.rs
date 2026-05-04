//! Snub cube — Archimedean solid (chiral cubic symmetry).
//!
//! 24 vertices, 38 faces (6 squares + 32 triangles). Vertex coordinates
//! involve the **tribonacci constant** `t ≈ 1.83928675…`, the real root
//! of `t³ = t² + t + 1`. v1 hardcodes `t` to 17 decimal places (≈ f64
//! precision); v2 will replace this with the algebraic-number DSL so
//! verification can be `IntervalSnap` instead of `F64Epsilon`.
//!
//! Source for vertex coordinates: standard Wikipedia / Coxeter conventions.
//! All even permutations of (±1, ±1/t, ±t), with an even number of minus
//! signs (yielding one chirality of the snub cube — the other is the
//! mirror image and equally Rupert-resistant).

use rupert_core::{Polyhedron, Vec3};

/// Tribonacci constant. Real root of `t^3 - t^2 - t - 1 = 0`.
/// Approximate to f64 precision; v2 replaces with exact algebraic form.
const TRIBONACCI: f64 = 1.839_286_755_214_161;

pub fn snub_cube() -> Polyhedron {
    let t = TRIBONACCI;
    let inv_t = 1.0 / t;

    // Even permutations of (±1, ±1/t, ±t) with an even number of minus
    // signs. There are 12 such triples per chirality; the snub cube has
    // 24 vertices when both chiralities are taken — but the standard
    // construction picks one chirality. We use the single-chirality
    // convention (even sign count, even permutations).
    let mut vertices: Vec<Vec3> = Vec::with_capacity(24);
    let triples = [
        (1.0, inv_t, t),
        (inv_t, t, 1.0),
        (t, 1.0, inv_t),
    ];
    // For each of the 3 even permutations of the magnitude triple,
    // emit the 8 sign combinations and keep those with an even count
    // of minus signs (4 combos: +++, +--, -+-, --+).
    for &(a, b, c) in &triples {
        for &(sa, sb, sc) in &[(1.0, 1.0, 1.0), (1.0, -1.0, -1.0), (-1.0, 1.0, -1.0), (-1.0, -1.0, 1.0)] {
            vertices.push(Vec3::new(a * sa, b * sb, c * sc));
        }
    }
    // The above produces 12 vertices. The snub cube has 24 — the other 12
    // come from odd permutations with odd sign count (the mirror chirality
    // joined into the same shape). Add them:
    let odd_triples = [
        (1.0, t, inv_t),
        (t, inv_t, 1.0),
        (inv_t, 1.0, t),
    ];
    for &(a, b, c) in &odd_triples {
        for &(sa, sb, sc) in &[(1.0, 1.0, -1.0), (1.0, -1.0, 1.0), (-1.0, 1.0, 1.0), (-1.0, -1.0, -1.0)] {
            vertices.push(Vec3::new(a * sa, b * sb, c * sc));
        }
    }
    debug_assert_eq!(vertices.len(), 24);
    // Faces: 38 total (6 squares + 32 triangles). Computing the exact face
    // adjacency for the snub cube takes a non-trivial table; v1 ships
    // empty faces because the projection-based clearance pipeline only
    // depends on the vertex cloud. The face_normal_pairs solver will
    // recognize the empty face list and skip enumeration on this shape.
    Polyhedron::new("snub_cube", vertices, vec![]).expect("snub cube vertices are valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_count_24() {
        assert_eq!(snub_cube().vertex_count(), 24);
    }

    #[test]
    fn face_count_zero_in_v1() {
        // Documents the v1 caveat: snub cube ships with empty faces; v2
        // will populate the 38-face adjacency along with the algebraic
        // tribonacci constant.
        assert_eq!(snub_cube().face_count(), 0);
    }

    #[test]
    fn tribonacci_satisfies_defining_equation() {
        let t = TRIBONACCI;
        let lhs = t * t * t;
        let rhs = t * t + t + 1.0;
        assert!((lhs - rhs).abs() < 1e-13, "tribonacci off: {lhs} vs {rhs}");
    }
}
