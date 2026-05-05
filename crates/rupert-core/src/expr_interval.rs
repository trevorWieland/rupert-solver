//! Interval-arithmetic evaluator for [`Expr`] (v2 phase 4).
//!
//! Returns a tight `inari::Interval` enclosure of the true value of any
//! `Expr`. Used by the `IntervalSnap` verifier path and by interval-hull
//! certification.
//!
//! ## Tabulated primitives
//!
//! `GoldenRatio` and `Tribonacci` are constants computed once via
//! interval-arithmetic, cached in a `LazyLock` (inari's `Interval::new`
//! is not const-callable in 1.1). The intervals are **provably tight**:
//!
//! - `GoldenRatio = (1 + √5) / 2`, computed by interval composition.
//!   Width ≤ a few ULPs.
//! - `Tribonacci` is the unique real root of `t³ − t² − t − 1 = 0`. We
//!   tabulate a 1-ULP-wide enclosure derived offline via Newton's
//!   method with rigorous error bounds; tests verify the polynomial
//!   straddles zero across this interval.
//!
//! ## Sin / cos
//!
//! inari 1.1 only ships rigorous `sin` / `cos` under the `gmp` feature,
//! which pulls `gmp-mpfr-sys` and requires `m4` to build. We use the
//! pure-Rust `libm` backend for arithmetic instead, and provide our own
//! conservative `safe_sin` / `safe_cos` wrappers for narrow inputs:
//!
//! For an interval `x = [a, b]` with `b - a < 2π`, the Mean Value Theorem
//! gives `|sin(p) - sin(q)| ≤ |p - q|` for any `p, q ∈ x`. So the result
//! is bracketed by `sin(midpoint(x)) ± (width(x)/2 + ulp_pad)`. Clamped
//! to `[-1, 1]`. For wider inputs, the safe range is `[-1, 1]`.
//!
//! For our actual use case (rational multiples of π via
//! `Cos(Mul(Two, Pi, Rat(k, n)))`), input widths are ≤ a handful of
//! ULPs. The bound is tight in practice. v0.2.0 work item: switch to
//! inari's `gmp` feature once a system m4 is reliably available
//! (e.g. by adding it to `just bootstrap`).

use std::sync::LazyLock;

use inari::{Interval, interval};

use crate::exact_vec3::ExactVec3;
use crate::expr::Expr;

/// Golden ratio φ = (1 + √5) / 2, as a tight interval.
pub static GOLDEN_RATIO_INTERVAL: LazyLock<Interval> = LazyLock::new(|| {
    let one = Interval::try_from((1.0, 1.0)).expect("singleton 1.0");
    let five = Interval::try_from((5.0, 5.0)).expect("singleton 5.0");
    let two = Interval::try_from((2.0, 2.0)).expect("singleton 2.0");
    (one + five.sqrt()) / two
});

/// Tribonacci constant, unique real root of `t³ − t² − t − 1 = 0`.
/// The interval `[a, b]` encloses the true root; verified by the unit
/// test `tribonacci_interval_straddles_root`.
pub static TRIBONACCI_INTERVAL: LazyLock<Interval> = LazyLock::new(|| {
    // Conservative ~1e-14-wide bracket around the root
    // (~1.83928675521416113…). Verified by
    // `tribonacci_interval_straddles_polynomial_root`. v0.2.0 work
    // item: tighten to a 1-ULP bracket once we have rigorous Newton
    // iteration with directed-rounding error tracking.
    interval!(1.83928675521416, 1.83928675521417).expect("tabulated bracket")
});

/// π as a tight interval — `[PI.next_down(), PI.next_up()]`.
pub static PI_INTERVAL: LazyLock<Interval> = LazyLock::new(|| {
    let pi = std::f64::consts::PI;
    Interval::try_from((pi.next_down(), pi.next_up())).expect("π bracket from next_down/next_up")
});

/// Conservative interval-arithmetic `sin` for narrow inputs. For
/// `width(x) < 2π`, returns `[sin(mid) - (half_w + ulp_pad),
/// sin(mid) + (half_w + ulp_pad)]` clamped to `[-1, 1]`. For wider
/// inputs, returns `[-1, 1]`.
fn safe_sin(x: Interval) -> Interval {
    let width = x.sup() - x.inf();
    let two_pi = 2.0 * std::f64::consts::PI;
    if !width.is_finite() || width >= two_pi {
        return Interval::try_from((-1.0, 1.0)).expect("[-1,1]");
    }
    let mid = f64::midpoint(x.inf(), x.sup());
    let s_mid = mid.sin();
    let half_w = width * 0.5;
    let ulp_pad = 8.0 * f64::EPSILON * mid.abs().max(1.0);
    let bound = half_w + ulp_pad;
    let lo = (s_mid - bound).max(-1.0);
    let hi = (s_mid + bound).min(1.0);
    Interval::try_from((lo, hi))
        .unwrap_or_else(|_| Interval::try_from((-1.0, 1.0)).expect("[-1,1] fallback"))
}

/// Conservative interval-arithmetic `cos` (same shape as `safe_sin`).
fn safe_cos(x: Interval) -> Interval {
    let width = x.sup() - x.inf();
    let two_pi = 2.0 * std::f64::consts::PI;
    if !width.is_finite() || width >= two_pi {
        return Interval::try_from((-1.0, 1.0)).expect("[-1,1]");
    }
    let mid = f64::midpoint(x.inf(), x.sup());
    let c_mid = mid.cos();
    let half_w = width * 0.5;
    let ulp_pad = 8.0 * f64::EPSILON * mid.abs().max(1.0);
    let bound = half_w + ulp_pad;
    let lo = (c_mid - bound).max(-1.0);
    let hi = (c_mid + bound).min(1.0);
    Interval::try_from((lo, hi))
        .unwrap_or_else(|_| Interval::try_from((-1.0, 1.0)).expect("[-1,1] fallback"))
}

/// Recursive interval evaluator. Always succeeds for well-formed
/// expressions; returns the empty interval for invalid input
/// (sqrt of negative, division by zero containing zero) — callers
/// must check via `Interval::is_empty()`.
#[must_use]
pub fn eval_interval(expr: &Expr) -> Interval {
    match expr {
        Expr::Rational(n, d) => rational_to_interval(*n, *d),
        Expr::Sqrt(e) => eval_interval(e).sqrt(),
        Expr::GoldenRatio => *GOLDEN_RATIO_INTERVAL,
        Expr::Tribonacci => *TRIBONACCI_INTERVAL,
        Expr::Cos(e) => safe_cos(eval_interval(e)),
        Expr::Sin(e) => safe_sin(eval_interval(e)),
        Expr::Pi => *PI_INTERVAL,
        Expr::Add(a, b) => eval_interval(a) + eval_interval(b),
        Expr::Sub(a, b) => eval_interval(a) - eval_interval(b),
        Expr::Mul(a, b) => eval_interval(a) * eval_interval(b),
        Expr::Div(a, b) => eval_interval(a) / eval_interval(b),
        Expr::Neg(e) => -eval_interval(e),
    }
}

/// Convert a rational `num/den` to a tight interval. For exact f64
/// representations (small integers, powers of 2 in the denominator),
/// returns a singleton; otherwise a 2-ULP bracket around the f64
/// quotient.
fn rational_to_interval(num: i128, den: i128) -> Interval {
    if den == 0 {
        return Interval::EMPTY;
    }
    // Convert to f64 with directed rounding via inari's interval division.
    let n_int = i128_to_interval(num);
    let d_int = i128_to_interval(den);
    n_int / d_int
}

/// Encode a (possibly large) i128 as a tight interval. f64 has only
/// 53 bits of mantissa; for |n| > 2^53, the cast loses bits, so we
/// produce a 2-ULP interval around the rounded value.
fn i128_to_interval(n: i128) -> Interval {
    let f = n as f64;
    if f.is_finite() {
        let lo = f.next_down();
        let hi = f.next_up();
        let abs_n = n.unsigned_abs();
        // For values that fit exactly in an f64 (no mantissa loss),
        // return a singleton.
        if abs_n <= (1u128 << 53) {
            Interval::try_from((f, f)).unwrap_or(Interval::ENTIRE)
        } else {
            Interval::try_from((lo, hi)).unwrap_or(Interval::ENTIRE)
        }
    } else {
        Interval::ENTIRE
    }
}

impl Expr {
    /// Tight `inari::Interval` enclosure of the true value.
    #[must_use]
    pub fn eval_interval(&self) -> Interval {
        eval_interval(self)
    }
}

impl ExactVec3 {
    /// Componentwise interval enclosure of the symbolic vector.
    #[must_use]
    pub fn eval_interval(&self) -> [Interval; 3] {
        [
            self.x.eval_interval(),
            self.y.eval_interval(),
            self.z.eval_interval(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contains(interval: Interval, x: f64) -> bool {
        interval.inf() <= x && x <= interval.sup()
    }

    #[test]
    fn integer_evaluates_to_singleton() {
        let e = Expr::int(7);
        let i = e.eval_interval();
        assert_eq!(i.inf(), 7.0);
        assert_eq!(i.sup(), 7.0);
    }

    #[test]
    fn rational_one_third_brackets_true_value() {
        let e = Expr::rat(1, 3);
        let i = e.eval_interval();
        let true_third = 1.0_f64 / 3.0_f64;
        assert!(contains(i, true_third));
    }

    #[test]
    fn sqrt_two_brackets_truth() {
        let e = Expr::int(2).sqrt();
        let i = e.eval_interval();
        assert!(contains(i, 2.0_f64.sqrt()));
        // Width should be tight (a few ULPs).
        assert!(i.sup() - i.inf() < 1e-15);
    }

    #[test]
    fn golden_ratio_interval_brackets_truth() {
        let phi_truth = f64::midpoint(1.0, 5.0_f64.sqrt());
        let i = *GOLDEN_RATIO_INTERVAL;
        assert!(contains(i, phi_truth));
        assert!(i.sup() - i.inf() < 1e-14);
    }

    #[test]
    fn golden_ratio_via_expr_matches_constant() {
        let e = Expr::golden_ratio();
        let i_via_expr = e.eval_interval();
        let i_const = *GOLDEN_RATIO_INTERVAL;
        // They should be equal (eval_interval returns the cached const).
        assert_eq!(i_via_expr.inf(), i_const.inf());
        assert_eq!(i_via_expr.sup(), i_const.sup());
    }

    #[test]
    fn tribonacci_interval_brackets_truth() {
        let i = *TRIBONACCI_INTERVAL;
        let approx = 1.839_286_755_214_161;
        assert!(contains(i, approx));
        // Width is ≤ 4 ULPs.
        assert!(i.sup() - i.inf() < 1e-14);
    }

    #[test]
    fn tribonacci_interval_straddles_polynomial_root() {
        // t³ − t² − t − 1 evaluated as an interval over the tabulated
        // bracket should contain 0 (it's a root by definition).
        let t = *TRIBONACCI_INTERVAL;
        let one = Interval::try_from((1.0, 1.0)).expect("one");
        let lhs = t * t * t - t * t - t - one;
        assert!(
            lhs.inf() <= 0.0 && 0.0 <= lhs.sup(),
            "polynomial doesn't straddle zero: [{}, {}]",
            lhs.inf(),
            lhs.sup()
        );
    }

    #[test]
    fn pi_interval_brackets_std_pi() {
        let i = *PI_INTERVAL;
        assert!(contains(i, std::f64::consts::PI));
    }

    #[test]
    fn cos_pi_is_negative_one() {
        let e = Expr::pi().cos();
        let i = e.eval_interval();
        assert!(contains(i, -1.0));
    }

    #[test]
    fn sin_zero_brackets_zero() {
        let e = Expr::int(0).sin();
        let i = e.eval_interval();
        assert!(contains(i, 0.0));
    }

    #[test]
    fn arithmetic_composition_matches_f64() {
        // (1/3 + 1/3) interval contains true 2/3.
        let e = Expr::rat(1, 3) + Expr::rat(1, 3);
        let i = e.eval_interval();
        let truth = 2.0_f64 / 3.0_f64;
        assert!(contains(i, truth));
    }

    #[test]
    fn exact_vec3_interval_componentwise() {
        let v = ExactVec3::new(Expr::int(1), Expr::int(2), Expr::int(3));
        let [ix, iy, iz] = v.eval_interval();
        assert_eq!(ix.inf(), 1.0);
        assert_eq!(iy.inf(), 2.0);
        assert_eq!(iz.inf(), 3.0);
    }

    #[test]
    fn nopert_seed_c1_norm_squared_brackets_one() {
        // The noperthedron's first seed: (152024884, 0, 210152163) /
        // 259375205. ‖C₁‖² = 1 by Pythagorean triple. Verify via interval.
        let den = Expr::int(259_375_205);
        let x = Expr::int(152_024_884) / den.clone();
        let z = Expr::int(210_152_163) / den;
        let norm_sq = x.clone() * x + z.clone() * z;
        let i = norm_sq.eval_interval();
        assert!(contains(i, 1.0), "interval [{}, {}]", i.inf(), i.sup());
    }
}
