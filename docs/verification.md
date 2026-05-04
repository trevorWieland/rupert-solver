# Verification

The verifier (`rupert-verify`) is the integrity layer between solvers and the leaderboard. A solver may report any candidate it wants; the verifier decides whether the leaderboard accepts it.

## v1: F64Epsilon

The shipped path. For each `Solution`:

1. Recompute the clearance via `rupert_core::evaluate_clearance(poly, &candidate)`. Same f64 pipeline the solver used; runs deterministic.
2. Reject if non-finite (NaN, infinity).
3. Reject if `recomputed ≤ F64_EPS = 1e-9`.
4. Reject if `|recomputed - solution.clearance| > 1e-6` (drift between reported and recomputed — a cross-platform integrity check).
5. Otherwise emit a `Certification { method: F64Epsilon, clearance_lo: recomputed, clearance_hi: recomputed }`.

The `1e-9` threshold is chosen to be larger than the worst-case rounding error in our projection-hull-clearance pipeline (a handful of `f64` mul/adds per vertex). Tighter shapes (e.g. the triakis tetrahedron, with published clearance ~4×10⁻⁶) still clear it comfortably.

## What this does NOT certify

`F64Epsilon` is "f64 is consistent with itself". It does NOT prove the candidate is a true Rupert passage in real arithmetic. Solutions whose true clearance is `> 0` but `< 1e-9` would be (correctly) rejected. Solutions where rounding errors flipped a true negative to a faux-positive *might* slip through if the recomputed clearance also rounds positive — though by similar logic, both rounds would have to agree on the lie.

For the eight v1 builtins (cube, tetra, octa, dodec, icos with rational vertices; triakis tetra, snub cube, noperthedron with mixed rational/algebraic), this is acceptable. The headline regression test — "verifier rejects identity solution for every builtin" — passes.

## v2: IntervalSnap and ExactRational

The two feature-gated paths in `Cargo.toml`:

```toml
[features]
default = []
exact = ["dep:malachite"]
interval = ["dep:inari"]
```

### `IntervalSnap` (interval feature)

For a candidate `(q_outer, q_inner, t)`:

1. Snap each quaternion component to a nearby rational (continued-fraction rounding bounded by denominator 2³²).
2. Recompute the projection-hull-clearance pipeline using `inari::Interval` over `f64`. Each operation tracks `[lo, hi]` bounds.
3. Accept only if the *lower* bound of clearance is strictly positive.

The challenge: convex hull is a combinatorial operation. When two interval-points are within each other's intervals along the hull boundary, the hull membership decision can flip. Robust IntervalSnap requires either (a) restricting to shapes whose hull combinatorics are forced by symmetry (cube, octa, tet, noperthedron with rational vertices), or (b) extending the interval bookkeeping to track combinatorial alternatives.

v2 will likely ship `IntervalSnap` only for the rational-vertex shapes (cube, octa, tet, dodec, icos, noperthedron) and leave snub cube on `F64Epsilon` until algebraic-number support lands.

### `ExactRational` (exact feature)

For shapes whose vertices are exact rationals (cube, octa, tet, noperthedron-actual), recompute everything in `malachite::Rational`. Slow but bulletproof. Used for the headline regressions, not for every leaderboard entry.

## The noperthedron — the verifier's headline correctness gate

The shape we ship as `noperthedron` uses the exact rational seeds from
arXiv:2508.18475 (Steininger & Yurkevich 2025) as published in their
reference implementation `Jakob256/Rupert/src/noperthedron.py`:

- C₁ = (152024884, 0, 210152163) / 259375205 — exactly unit-norm via Pythagorean triple.
- C₂ = (6632738028, 6106948881, 3980949609) / 10¹⁰ — ‖C₂‖ ≈ 0.985576.
- C₃ = (8193990033, 5298215096, 1230614493) / 10¹⁰ — ‖C₃‖ ≈ 0.983499.

The 90 vertices are generated via the order-30 cyclic group action
`(-1)^ℓ · R_z(2πk/15) · C_{i+1}`. **For these specific seeds the polyhedron
is proven non-Rupert** (Theorems 17 + 36 of the paper, verified via a
2.5GB SageMath certificate in 30 hours of compute).

The headline regression test in `rupert-bench` runs every v1 solver ×
seed ∈ {0, 1, 2} × 50 000 evals against the noperthedron; the test
fails on any `RunOutcome::Solved`. This is the single most important
test in the repo. If it ever flips, **the verifier is broken** —
that's a P0 bug.

v1 stores seeds as f64 (rounded from the rationals above). The empirical
soundness check is robust to this rounding. v2 will:

1. Wire the `ExactRational` verifier path against the integer-rational
   seed numerators directly, so candidates against the noperthedron get
   rejected via exact rational arithmetic rather than f64 approximation.
2. Add a sweep test that asserts certified rejection on N=10 000 random
   rotation pairs (currently we cover 5 solvers × 3 seeds = 15
   configurations).

## The snub cube caveat

The snub cube's vertices involve the **tribonacci constant** `t ≈ 1.83928675…`, the real root of `t³ = t² + t + 1`. v1 hardcodes `t` to a 16-digit f64 approximation. Solutions for the snub cube are accepted only via `F64Epsilon`. v2 ships an algebraic-number coordinate DSL (`Sqrt(N)`, `Tribonacci`, `GoldenRatio`, …) with both `f64` and `inari` evaluators; once that lands, the snub cube graduates to `IntervalSnap`.

## What the verifier integrates with

- `rupert_bench::runner::run_one` calls `certify` automatically after a solver returns `SolverOutcome::Found`. On certification failure, the run becomes `RunOutcome::Disqualified { reason }`.
- `rupert verify <results>` re-runs the verifier on stored JSONL files and rewrites them in place.
- `rupert lead build` excludes uncertified rows from the headline ranking.
