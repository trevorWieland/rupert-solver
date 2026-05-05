//! Symbolic 3D vector — `Expr` per coordinate.
//!
//! v2 Phase 2 of the algebraic-coord DSL plan. Lives alongside the f64
//! [`crate::vec3::Vec3`] type, which solvers consume on the hot path;
//! `ExactVec3` is for shape definitions and the verifier.

use crate::expr::Expr;
use crate::vec3::Vec3;

/// 3D vector with symbolic-algebraic coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ExactVec3 {
    pub x: Expr,
    pub y: Expr,
    pub z: Expr,
}

impl ExactVec3 {
    pub fn new(x: Expr, y: Expr, z: Expr) -> Self {
        Self { x, y, z }
    }

    /// Build from rational coordinates `(numerator/denominator)` per axis.
    /// Common shorthand for shape vertex tables.
    pub fn rat(nx: i128, dx: i128, ny: i128, dy: i128, nz: i128, dz: i128) -> Self {
        Self::new(Expr::rat(nx, dx), Expr::rat(ny, dy), Expr::rat(nz, dz))
    }

    /// Build from integer coordinates.
    pub fn int(x: i128, y: i128, z: i128) -> Self {
        Self::new(Expr::int(x), Expr::int(y), Expr::int(z))
    }

    /// Evaluate to an f64 [`Vec3`]. Always succeeds.
    #[must_use]
    pub fn eval_f64(&self) -> Vec3 {
        Vec3::new(self.x.eval_f64(), self.y.eval_f64(), self.z.eval_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rational_vec3_evaluates_componentwise() {
        let v = ExactVec3::rat(1, 2, 3, 4, 5, 6);
        let f = v.eval_f64();
        assert!((f.x - 0.5).abs() < 1e-15);
        assert!((f.y - 0.75).abs() < 1e-15);
        assert!((f.z - 5.0_f64 / 6.0).abs() < 1e-15);
    }

    #[test]
    fn integer_vec3_evaluates_to_doubles() {
        let v = ExactVec3::int(1, -2, 3);
        let f = v.eval_f64();
        assert_eq!(f, Vec3::new(1.0, -2.0, 3.0));
    }

    #[test]
    fn golden_ratio_components_in_dodec_match_f64() {
        // Dodecahedron uses (0, ±1/φ, ±φ) and similar permutations. Build
        // one via Expr and verify it matches the f64-precision PHI.
        let phi = Expr::golden_ratio();
        let inv_phi = Expr::int(1) / phi.clone();
        let v = ExactVec3::new(Expr::int(0), inv_phi, phi);
        let f = v.eval_f64();
        let expected_phi = f64::midpoint(1.0, 5.0_f64.sqrt());
        let expected_inv = 1.0 / expected_phi;
        assert_eq!(f.x, 0.0);
        assert!((f.y - expected_inv).abs() < 1e-15, "f.y = {}", f.y);
        assert!((f.z - expected_phi).abs() < 1e-15, "f.z = {}", f.z);
    }

    #[test]
    fn noperthedron_seed_c1_has_unit_norm() {
        // ‖C₁‖² = 1 by Pythagorean triple (152024884² + 210152163² = 259375205²).
        let den = Expr::int(259_375_205);
        let c1 = ExactVec3::new(
            Expr::int(152_024_884) / den.clone(),
            Expr::int(0),
            Expr::int(210_152_163) / den,
        );
        let f = c1.eval_f64();
        let norm_sq = f.x * f.x + f.y * f.y + f.z * f.z;
        assert!((norm_sq - 1.0).abs() < 1e-15);
    }
}
