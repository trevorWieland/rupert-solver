//! Symbolic algebraic expressions.
//!
//! v2 Phase 1: closed primitive set per `docs/v2-algebraic-coords.md`.
//! The f64, interval (`inari`), and rational (`malachite`) evaluators
//! are compiled unconditionally. Non-rational primitives return `None`
//! from the rational evaluator.
//!
//! ## Design constraints
//!
//! - Solvers must NOT consume `Expr`. They stay on the f64 hot path via
//!   `Polyhedron::vertices`. `Expr` lives behind shape definitions and
//!   the verifier.
//! - The primitive set is closed and small. New primitives require
//!   adding both a variant here and (eventually) corresponding interval
//!   /rational evaluators. Don't add cube roots, Galois conjugates, or
//!   anything beyond the documented v2 set without updating the design
//!   doc first.
//! - No canonicalization. `Mul(Two, GoldenRatio)` and `Add(Phi, Phi)`
//!   are different `Expr`s even though they evaluate equally. If you
//!   need equality, compare evaluations.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// Symbolic algebraic expression. f64 evaluation is total (always finite
/// for any well-formed expression).
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Rational number `numerator / denominator`. Stored unreduced; the
    /// reducing happens lazily via [`Expr::rat`].
    Rational(i128, i128),
    /// Square root of a non-negative expression.
    Sqrt(Box<Expr>),
    /// `(1 + √5) / 2`. Tabulated primitive.
    GoldenRatio,
    /// Real root of `t³ − t² − t − 1 = 0`. Tabulated primitive.
    Tribonacci,
    /// Cosine of the inner expression (radians).
    Cos(Box<Expr>),
    /// Sine of the inner expression (radians).
    Sin(Box<Expr>),
    /// π.
    Pi,
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
}

/// Tribonacci constant `t ≈ 1.83928675…`. The unique real root of
/// `t³ − t² − t − 1 = 0`. Stored to f64 precision; v2 phase 4 swaps in
/// a tight `inari` interval bracketing the true value.
pub const TRIBONACCI_F64: f64 = 1.839_286_755_214_161;

/// Golden ratio `φ = (1 + √5) / 2`. f64 best-effort.
pub const GOLDEN_RATIO_F64: f64 = 1.618_033_988_749_895;

impl Expr {
    /// Build an integer expression as `Rational(n, 1)`.
    #[must_use]
    pub const fn int(n: i128) -> Self {
        Self::Rational(n, 1)
    }

    /// Build a reduced rational expression. Panics on `d == 0`.
    /// Caller is responsible for `d != 0`; we reject in tests via
    /// `assert!`, not via runtime validation, since shape definitions
    /// are static and a zero denominator is a programming error.
    #[must_use]
    pub fn rat(n: i128, d: i128) -> Self {
        assert!(d != 0, "Expr::rat: zero denominator");
        let (n, d) = reduce(n, d);
        Self::Rational(n, d)
    }

    #[must_use]
    pub fn sqrt(self) -> Self {
        Self::Sqrt(Box::new(self))
    }

    #[must_use]
    pub fn cos(self) -> Self {
        Self::Cos(Box::new(self))
    }

    #[must_use]
    pub fn sin(self) -> Self {
        Self::Sin(Box::new(self))
    }

    #[must_use]
    pub fn golden_ratio() -> Self {
        Self::GoldenRatio
    }

    #[must_use]
    pub fn tribonacci() -> Self {
        Self::Tribonacci
    }

    #[must_use]
    pub fn pi() -> Self {
        Self::Pi
    }

    /// `cos(2πk/n)` — common for symmetric-rotation constructions.
    #[must_use]
    pub fn cos_two_pi_k_over(k: i128, n: i128) -> Self {
        let two_pi_k_over_n = Self::int(2) * Self::Pi * Self::rat(k, n);
        two_pi_k_over_n.cos()
    }

    /// `sin(2πk/n)`.
    #[must_use]
    pub fn sin_two_pi_k_over(k: i128, n: i128) -> Self {
        let two_pi_k_over_n = Self::int(2) * Self::Pi * Self::rat(k, n);
        two_pi_k_over_n.sin()
    }

    /// f64 best-effort evaluation. Always returns a finite f64 for any
    /// well-formed expression (no division by zero, no sqrt of negative).
    /// On ill-formed inputs the result may be NaN or infinity — tests
    /// catch these.
    #[must_use]
    pub fn eval_f64(&self) -> f64 {
        match self {
            Self::Rational(n, d) => (*n as f64) / (*d as f64),
            Self::Sqrt(e) => e.eval_f64().sqrt(),
            Self::GoldenRatio => GOLDEN_RATIO_F64,
            Self::Tribonacci => TRIBONACCI_F64,
            Self::Cos(e) => e.eval_f64().cos(),
            Self::Sin(e) => e.eval_f64().sin(),
            Self::Pi => std::f64::consts::PI,
            Self::Add(a, b) => a.eval_f64() + b.eval_f64(),
            Self::Sub(a, b) => a.eval_f64() - b.eval_f64(),
            Self::Mul(a, b) => a.eval_f64() * b.eval_f64(),
            Self::Div(a, b) => a.eval_f64() / b.eval_f64(),
            Self::Neg(e) => -e.eval_f64(),
        }
    }

    #[must_use]
    pub fn eval_rational(&self) -> Option<malachite::Rational> {
        use malachite::Rational;

        match self {
            Self::Rational(n, d) => Some(Rational::from_signeds(*n, *d)),
            Self::Add(a, b) => Some(a.eval_rational()? + b.eval_rational()?),
            Self::Sub(a, b) => Some(a.eval_rational()? - b.eval_rational()?),
            Self::Mul(a, b) => Some(a.eval_rational()? * b.eval_rational()?),
            Self::Div(a, b) => Some(a.eval_rational()? / b.eval_rational()?),
            Self::Neg(e) => Some(-e.eval_rational()?),
            Self::Sqrt(_)
            | Self::GoldenRatio
            | Self::Tribonacci
            | Self::Cos(_)
            | Self::Sin(_)
            | Self::Pi => None,
        }
    }
}

/// Reduce a fraction to lowest terms, with positive denominator.
fn reduce(n: i128, d: i128) -> (i128, i128) {
    if d < 0 {
        return reduce(-n, -d);
    }
    let g = gcd(n.unsigned_abs(), d.unsigned_abs());
    if g == 0 {
        return (n, d);
    }
    // gcd of values fitting in i128 also fits in i128 (it divides both).
    let g_i128 = i128::try_from(g).unwrap_or(1);
    (n / g_i128, d / g_i128)
}

/// Euclidean GCD.
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

impl From<i128> for Expr {
    fn from(n: i128) -> Self {
        Self::int(n)
    }
}

impl From<i32> for Expr {
    fn from(n: i32) -> Self {
        Self::int(i128::from(n))
    }
}

impl Add for Expr {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::Add(Box::new(self), Box::new(rhs))
    }
}

impl Sub for Expr {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::Sub(Box::new(self), Box::new(rhs))
    }
}

impl Mul for Expr {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::Mul(Box::new(self), Box::new(rhs))
    }
}

impl Div for Expr {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self::Div(Box::new(self), Box::new(rhs))
    }
}

impl Neg for Expr {
    type Output = Self;
    fn neg(self) -> Self {
        Self::Neg(Box::new(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn rational_evaluates_to_quotient() {
        assert!(approx(Expr::rat(3, 4).eval_f64(), 0.75, 1e-15));
        assert!(approx(Expr::int(7).eval_f64(), 7.0, 0.0));
    }

    #[test]
    fn rational_reduces_to_lowest_terms() {
        let e = Expr::rat(6, 8);
        assert_eq!(e, Expr::Rational(3, 4));
    }

    #[test]
    fn rational_normalizes_negative_denominator() {
        let e = Expr::rat(3, -4);
        assert_eq!(e, Expr::Rational(-3, 4));
    }

    #[test]
    fn sqrt_of_two_is_irrational_value() {
        assert!(approx(Expr::int(2).sqrt().eval_f64(), 2_f64.sqrt(), 1e-15));
    }

    #[test]
    fn golden_ratio_constant() {
        assert!(approx(
            Expr::golden_ratio().eval_f64(),
            f64::midpoint(1.0, 5.0_f64.sqrt()),
            1e-15
        ));
    }

    #[test]
    fn golden_ratio_derived_from_definition_matches() {
        // φ = (1 + √5) / 2.
        let phi_derived = (Expr::int(1) + Expr::int(5).sqrt()) / Expr::int(2);
        assert!(approx(
            phi_derived.eval_f64(),
            Expr::golden_ratio().eval_f64(),
            1e-15
        ));
    }

    #[test]
    fn tribonacci_satisfies_defining_polynomial() {
        let t = Expr::tribonacci().eval_f64();
        let lhs = t * t * t;
        let rhs = t * t + t + 1.0;
        assert!(approx(lhs, rhs, 1e-13), "tribonacci off: {lhs} vs {rhs}");
    }

    #[test]
    fn pi_evaluates_to_std_pi() {
        assert_eq!(Expr::pi().eval_f64(), std::f64::consts::PI);
    }

    #[test]
    fn cos_pi_is_negative_one() {
        assert!(approx(Expr::pi().cos().eval_f64(), -1.0, 1e-15));
    }

    #[test]
    fn sin_zero_is_zero() {
        assert!(approx(Expr::int(0).sin().eval_f64(), 0.0, 1e-15));
    }

    #[test]
    fn cos_two_pi_k_over_n_matches_direct() {
        // cos(2π * 3 / 15) = cos(2π/5) ≈ 0.30901699
        let e = Expr::cos_two_pi_k_over(3, 15);
        let direct = (2.0 * std::f64::consts::PI * 3.0 / 15.0).cos();
        assert!(approx(e.eval_f64(), direct, 1e-15));
    }

    #[test]
    fn arithmetic_ops_are_composable() {
        // (3/4) + (1/4) - (1/2) * (1/3) / (1/6) = 1 - 1 = 0
        let e = (Expr::rat(3, 4) + Expr::rat(1, 4))
            - (Expr::rat(1, 2) * Expr::rat(1, 3)) / Expr::rat(1, 6);
        assert!(approx(e.eval_f64(), 0.0, 1e-15));
    }

    #[test]
    fn neg_negates() {
        assert!(approx((-Expr::int(7)).eval_f64(), -7.0, 0.0));
    }

    #[test]
    fn from_int_works() {
        let e: Expr = 42_i32.into();
        assert_eq!(e, Expr::int(42));
    }

    #[test]
    fn dodecahedron_apothem_phi_matches_golden_ratio() {
        // The dodecahedron's vertex coordinate `1/φ` should match
        // `2 / (1 + √5)`.
        let a = Expr::int(1) / Expr::golden_ratio();
        let b = Expr::int(2) / (Expr::int(1) + Expr::int(5).sqrt());
        assert!(approx(a.eval_f64(), b.eval_f64(), 1e-15));
    }

    #[test]
    fn nopert_seed_c1_evaluates_to_unit_norm() {
        // The noperthedron's first seed: (152024884, 0, 210152163) /
        // 259375205. ‖C₁‖² should be 1 by Pythagorean identity in the
        // f64 evaluator.
        let den = Expr::int(259_375_205);
        let x = Expr::int(152_024_884) / den.clone();
        let y = Expr::int(0);
        let z = Expr::int(210_152_163) / den;
        let norm_sq = x.clone() * x + y.clone() * y + z.clone() * z;
        assert!(approx(norm_sq.eval_f64(), 1.0, 1e-15));
    }

    #[test]
    fn rational_eval_rejects_non_rational_primitives() {
        assert!(Expr::rat(3, 4).eval_rational().is_some());
        assert!(Expr::golden_ratio().eval_rational().is_none());
        assert!(Expr::tribonacci().eval_rational().is_none());
        assert!(Expr::pi().eval_rational().is_none());
    }
}
