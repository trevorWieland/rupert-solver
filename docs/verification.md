# Verification

The verifier (`rupert-verify`) is the integrity layer between solvers and the leaderboard. A solver may report any candidate it wants; the verifier decides whether the leaderboard accepts it — and at what certification tier.

## Two tiers shipped

```text
ExactRational  ⏵  IntervalSnap  ⏵  F64Epsilon
   (v0.3.0)        (shipped)        (shipped)
```

The bench runner (`rupert_bench::runner::run_one`) attempts the strongest tier available for the polyhedron, falling back as needed:

1. If the polyhedron carries `exact_vertices` (all 8 builtins do), try `certify_interval` → `IntervalSnap`.
2. Otherwise (or if interval certification rejects), call `certify` → `F64Epsilon`.
3. If both fail, the run becomes `RunOutcome::Disqualified { reason }`.

The certification method ends up in the `Solution::certification` field and is rendered as a column in `LEADERBOARD.md`.

## `F64Epsilon`

The baseline path. For each `Solution`:

1. Recompute the clearance via `rupert_core::evaluate_clearance(poly, &candidate)`. Same f64 pipeline the solver used; runs deterministic.
2. Reject if non-finite (NaN, infinity).
3. Reject if `recomputed ≤ F64_EPS = 1e-9`.
4. Reject if `|recomputed - solution.clearance| > 1e-6` (drift between reported and recomputed — a cross-platform integrity check).
5. Otherwise emit `Certification { method: F64Epsilon, clearance_lo: recomputed, clearance_hi: recomputed }`.

The `1e-9` threshold is chosen to be larger than the worst-case rounding error in our projection-hull-clearance pipeline (a handful of `f64` mul/adds per vertex). Tighter shapes (e.g. the triakis tetrahedron, with published clearance ~4×10⁻⁶) still clear it comfortably.

`F64Epsilon` is "f64 is consistent with itself". It does NOT prove the candidate is a true Rupert passage in real arithmetic. That's what `IntervalSnap` provides.

## `IntervalSnap`

The rigorous path. For each `Solution` against a polyhedron with `exact_vertices`:

1. **Enclose vertices.** Each vertex's `Expr` triple is evaluated via `eval_interval()` to produce a tight `[Interval; 3]` enclosure. Algebraic primitives (`GoldenRatio`, `Tribonacci`, `Sqrt`, `Cos`, `Sin`, `Pi`) are tabulated to known-good intervals; arithmetic propagates through `inari` with `libm` backend.
2. **Project under the f64 quaternion.** The candidate's `(q_outer, q_inner, t)` is f64; we treat each f64 component as a singleton `Interval`. The 3×3 quaternion rotation matrix is built and applied as an interval matrix-vector product against the enclosed vertices. Drop z and add the f64 translation (singleton).
3. **Certify the outer hull combinatorics.** `convex_hull_interval_certified(f64_pts, int_pts)` computes the f64 hull, then verifies under intervals that (a) every claimed-interior point's interval doesn't cross the boundary into the polygon, and (b) every claimed-boundary point's interval doesn't lie strictly inside the hull-of-others. If either check is ambiguous, return `Err(HullCombinatoricsAmbiguous)`.
4. **Certify each inner vertex.** For every inner interval-vertex, run `point_in_interval_polygon_strict` against the outer interval-polygon. Reject unless the result is `DefinitelyInside` (every cross-product sign is forced strictly positive across all polygon edges, under intervals).
5. **Compute interval clearance.** For each inner interval-vertex, the signed perpendicular distance to every outer hull edge is an `Interval`; the minimum across all (inner, edge) pairs gives the certified clearance enclosure.
6. **Accept iff `clearance_lo > 0` strictly.** Emit `Certification { method: IntervalSnap, clearance_lo, clearance_hi }`.

Because the rotation is f64 (singleton intervals) and the vertices are tightly enclosed, the resulting interval clearance is typically only 1–4 ULPs wider than the f64 recomputation — but the lower bound is *rigorous*, not "we hope rounding didn't bite us."

The combinatorial-precommit pattern is the technique Steininger & Yurkevich use in arXiv:2508.18475. It reduces the interval hull problem to a linear-time hull-membership check; without it, naive interval propagation through Andrew's monotone chain explodes.

## What `IntervalSnap` doesn't catch

Two corner cases:

- **The candidate's f64 quaternion isn't unit-norm.** We treat `q` as singleton intervals, so any drift from `‖q‖ = 1` propagates as a (small) rotation error. v0.3.0 work item: snap `q` to a nearby rational and treat its components as intervals containing both the nearby rational and the original f64.
- **The exact vertices' algebraic primitives have wider-than-necessary intervals.** Tribonacci is hand-tabulated to ~4 ULPs; if a future shape needs root-isolation algorithms with looser bounds, the certified clearance would tighten only as far as those primitives permit.

For the 8 shipped shapes, neither case bites in practice: cube/tet/octa/triakis are pure rational; dodec/icos use `GoldenRatio` (1-ULP-equivalent enclosure); snub cube uses `Tribonacci` (4-ULP-equivalent enclosure); noperthedron uses rationals + cos/sin of rational multiples of π via the cos/sin Taylor series with rigorous remainder.

## `ExactRational` (roadmapped, not shipped)

For shapes whose vertices are exact rationals (cube, octa, tet, triakis tet, noperthedron seeds), `certify_exact` would recompute everything in `malachite::Rational`. Slow but bulletproof — the headline regression for the noperthedron would graduate to "no f64 candidate can ever produce a positive-clearance witness against the exact rational polyhedron". See [`roadmap.md`](roadmap.md) §"Exact rational verifier" for design and triggers.

## The noperthedron — the verifier's headline correctness gate

The shape we ship as `noperthedron` uses the exact rational seeds from arXiv:2508.18475 (Steininger & Yurkevich 2025) as published in their reference implementation `Jakob256/Rupert/src/noperthedron.py`:

- C₁ = (152024884, 0, 210152163) / 259375205 — exactly unit-norm via Pythagorean triple.
- C₂ = (6632738028, 6106948881, 3980949609) / 10¹⁰ — ‖C₂‖ ≈ 0.985576.
- C₃ = (8193990033, 5298215096, 1230614493) / 10¹⁰ — ‖C₃‖ ≈ 0.983499.

The 90 vertices are generated via the order-30 cyclic group action `(-1)^ℓ · R_z(2πk/15) · C_{i+1}`. **For these specific seeds the polyhedron is proven non-Rupert** (Theorems 17 + 36 of the paper, verified via a 2.5GB SageMath certificate in 30 hours of compute).

The headline regression test in `rupert-bench` runs every shipped solver × seed ∈ {0, 1, 2} × 50 000 evals against the noperthedron; the test fails on any `RunOutcome::Solved`. This is the single most important test in the repo. If it ever flips, **the verifier is broken** — that's a P0 bug.

The exact vertices are stored symbolically: rational seeds × `Expr::cos_two_pi_k_over(k, 15)` rotation entries. So the IntervalSnap path *can* run against the noperthedron — and would be the right place to graduate the headline regression in v0.3.0 (currently the regression is empirically robust at 50 000 evals; the IntervalSnap upgrade would make it formally robust at any candidate).

## The snub cube caveat (still unsolved)

The snub cube remains the open mathematical problem. Vertex coordinates are exact (`Expr::Tribonacci`), so the IntervalSnap path is fully operational; if any solver finds a positive-clearance candidate, the verifier will certify it rigorously. The challenge is finding the candidate at all — patch_aware v0.2.0 still resists at 2M evals, consistent with the open status of the problem since arXiv:2112.13754.

## What the verifier integrates with

- `rupert_bench::runner::run_one` calls the strongest available certifier automatically after a solver returns `SolverOutcome::Found`. On certification failure, the run becomes `RunOutcome::Disqualified { reason }`.
- `rupert verify <results>` re-runs the verifier on stored JSONL files and rewrites them in place.
- `rupert lead build` excludes uncertified rows from the headline ranking and surfaces the `cert` column (interval / f64ε / —) so contributors can see which results carry the strongest guarantee.
