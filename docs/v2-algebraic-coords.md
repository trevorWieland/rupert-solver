# Algebraic Coordinate DSL — v2 Design Notes

This document records the algebraic-coordinate design that is now implemented. Read it before changing `Expr`, `ExactVec3`, builtin exact vertex tables, or the verifier certification order.

## Why we need this

Three forces converge:

1. **Snub cube** vertices use the **tribonacci constant** `t ≈ 1.83928675…` (real root of `t³ − t² − t − 1 = 0`).
2. **Dodecahedron / icosahedron** use the **golden ratio** `φ = (1 + √5)/2`. Same f64 truncation problem.
3. **Noperthedron** seeds are rational, but the group action `R_z(2πk/15)` introduces sin/cos of rational multiples of π — algebraic numbers of degree 4 over ℚ (more precisely, in the ring ℤ[ζ₁₅] of degree φ(15) = 8). Steininger & Yurkevich's proof leans on interval arithmetic for these rotation matrices, not on full algebraic-number arithmetic.

Without a richer numeric type than f64, `rupert-verify` could not honestly certify these shapes via the interval (`inari`) or exact (`malachite`) paths. The shipped DSL gives every builtin an interval-capable vertex table and gives rational-coordinate shapes the stronger exact-rational path.

## What we are NOT building

- A general-purpose computer algebra system. We don't need polynomial GCDs over ℚ, factorization, Gröbner bases, etc.
- Full algebraic-number arithmetic to arbitrary degree. The shapes we ship only need: rationals, square roots of rationals, tribonacci, golden ratio, and `sin/cos` of rational multiples of π. That's a closed and small set of primitives.
- Symbolic differentiation. The Gosain-Grimmer port noted symbolic gradients as a v2 want, but that's a *separate* axis from algebraic coordinates and lives in a different crate (`rupert-solvers`, not `rupert-core`/`rupert-verify`).

If a future shape needs something more exotic (cube roots of rationals, Galois conjugates, etc.), we extend the DSL primitive set. Don't generalize prematurely.

## The DSL

```rust
// crates/rupert-core/src/expr.rs (proposed)

/// Symbolic algebraic expression. Closed under +, −, ×, ÷, and the
/// algebraic primitives we ship in v2.
pub enum Expr {
    Rational(i128, i128),               // (numerator, denominator); always reduced
    Sqrt(Box<Expr>),                    // √x for x ≥ 0
    GoldenRatio,                        // (1 + √5) / 2
    Tribonacci,                         // real root of t³ − t² − t − 1
    Cos(Box<Expr>),                     // cos(x); x in radians
    Sin(Box<Expr>),                     // sin(x); x in radians
    Pi,                                 // π
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
}
```

### Evaluators

Three evaluators with progressively stricter contracts:

```rust
impl Expr {
    /// Always succeeds. f64 best-effort.
    pub fn eval_f64(&self) -> f64;

    /// Always succeeds. Result is a tight interval bracketing the true
    /// value. For a primitive like Tribonacci, this is a hand-tabulated
    /// known-good interval; for Sin/Cos it's via Taylor-series with
    /// rigorous remainder bounds; for arithmetic it's interval propagation.
    pub fn eval_interval(&self) -> inari::Interval;

    /// Succeeds only for purely rational expressions (no Sqrt/Sin/Cos/Pi
    /// /GoldenRatio/Tribonacci). Returns None on non-rational subexpression.
    pub fn eval_rational(&self) -> Option<malachite::Rational>;
}
```

### Constructor sugar

```rust
pub fn rat(n: i128, d: i128) -> Expr;        // Rational with auto-reduce
pub fn sqrt(e: Expr) -> Expr;
pub fn cos_pi_over(d: i128) -> Expr;         // cos(π/d)
pub fn cos_2pi_k_over(k: i32, n: i32) -> Expr;  // cos(2πk/n) — used by noperthedron
// ...etc.
```

These keep the shape definitions readable.

## The `ExactVec3` type

```rust
pub struct ExactVec3 {
    pub x: Expr,
    pub y: Expr,
    pub z: Expr,
}

impl ExactVec3 {
    pub fn eval_f64(&self) -> Vec3;
    pub fn eval_interval(&self) -> [inari::Interval; 3];
    pub fn eval_rational(&self) -> Option<[malachite::Rational; 3]>;
}
```

A `Polyhedron` keeps its current f64 `vertices: Vec<Vec3>` field for the solver hot path. We add an optional `pub exact_vertices: Option<Vec<ExactVec3>>`. Verifier paths (interval, exact) require `exact_vertices.is_some()`.

This is non-breaking: existing solvers ignore the new field.

## Shape upgrade plan

Order by ROI, not by source-code complexity:

1. **Dodecahedron + icosahedron.** Drop their f64 `PHI` constant; use `Expr::GoldenRatio` in the exact vertex table. No solver behavior change (f64 evaluation produces the same ULP-equivalent f64 vertex array). Verifier gains an `IntervalSnap`-eligible certification path for both shapes.
2. **Cube + tetrahedron + octahedron.** Already have rational vertices; the upgrade is just wiring `exact_vertices`. Verifier gains an `ExactRational` path for these three shapes.
3. **Snub cube.** Replace hardcoded `TRIBONACCI` constant with `Expr::Tribonacci`. Verifier gains `IntervalSnap` (not `ExactRational` — tribonacci isn't rational).
4. **Noperthedron.** Already has integer-rational seeds. The group-action multiplications `R_z(2πk/15) · C_i` use `Expr::Cos/Sin(Mul(...))` for the rotation matrix entries. Verifier gains `IntervalSnap`; `ExactRational` does not apply to the rotated vertex table.
5. **Triakis tetrahedron.** Vertices are rational (k = 5/3 scale factor times integer-coordinate centroid). Easy upgrade.
6. **Pentagonal icositetrahedron.** Build exact vertices as the snub-cube polar dual, using exact plane equations from the snub-cube face vertices.

After these upgrades, every builtin shape has at least `IntervalSnap` available.

## The hard sub-problem: interval convex hull

When two projected points have overlapping x-coordinate intervals, the lex-sort step in Andrew's monotone chain is ambiguous. Naive interval propagation on a combinatorial algorithm gives wildly conservative bounds.

Two strategies:

- **Combinatorial precommit.** First evaluate at f64 to determine the hull's combinatorial structure (which vertices are on the boundary, in what order). Then verify in interval arithmetic that this combinatorial structure is **forced**: every claimed-boundary vertex's interval is bounded away from being interior, and every claimed-interior vertex's interval is bounded away from being on the boundary. If verification fails, the candidate is "near a hull combinatorial transition" — refine the candidate or reject. This is what Steininger & Yurkevich do.
- **Branch on uncertainty.** If two points might be in either order, recurse on both. Exponential in the worst case but tractable for shapes whose interval bounds are tight.

Strategy 1 is the right v2 choice. Document strategy 2 as a fallback for pathological cases.

## Verifier path matrix after v2

| Shape | F64Epsilon (v1) | IntervalSnap (v2) | ExactRational (v2) |
|---|---|---|---|
| Cube | ✓ | ✓ | ✓ |
| Tetrahedron | ✓ | ✓ | ✓ |
| Octahedron | ✓ | ✓ | ✓ |
| Dodecahedron | ✓ | ✓ | ✗ (uses √5) |
| Icosahedron | ✓ | ✓ | ✗ (uses √5) |
| Triakis tetrahedron | ✓ | ✓ | ✓ |
| Pentagonal icositetrahedron | ✓ | ✓ | ✗ (snub-cube dual; uses tribonacci) |
| Snub cube | ✓ | ✓ | ✗ (uses tribonacci) |
| Noperthedron | ✓ | ✓ | ✗ (post-rotation trig expressions) |

The promotion order on the leaderboard becomes ExactRational ≻ IntervalSnap ≻ F64Epsilon. The **headline rank** keeps using just-positive-clearance gating; the cert method is metadata that lets agents compete on "highest cert tier."

## Phase status (updated)

| Phase | Crate(s) | LOC | Risk | Status |
|---|---|---|---|---|
| 1. `Expr` type + `eval_f64` | `rupert-core` | ~300 | Low | ✓ shipped |
| 2. `ExactVec3` + `Polyhedron::exact_vertices` | `rupert-core` | ~150 | Low | ✓ shipped |
| 3. Migrate builtin shapes to exact tables | `rupert-shapes` | ~200 | Low | ✓ shipped (all 9 shapes use `Polyhedron::with_exact`) |
| 4. `eval_interval` (inari arithmetic + tabulated primitives) | `rupert-core` | ~400 | Medium | ✓ shipped (non-optional inari dep with `libm` backend; `safe_sin/safe_cos` for narrow intervals since gmp needs m4) |
| 5. `eval_rational` (malachite arithmetic) | `rupert-core` | ~150 | Low | ✓ shipped |
| 6. Combinatorial-precommit interval hull | `rupert-core` | ~500 | High | ✓ shipped (`hull2d_interval` module with `point_in_interval_polygon_strict` and `convex_hull_interval_certified`) |
| 7. `certify_interval` for IntervalSnap | `rupert-verify` | ~200 | Medium | ✓ shipped (`interval_cert.rs`; bench runner attempts it first) |
| 8. `certify_exact` for ExactRational | `rupert-verify` | ~250 | Medium | ✓ shipped |
| 9. Update headline regression to use verifier rejection for noperthedron | `rupert-bench` | ~30 | Low | ✓ shipped |

Phases 1–9 are shipped, with `ExactRational` compiled by default. See [`roadmap.md`](roadmap.md) for prioritized next steps.

## Crate layering implication

`Expr` belongs in `rupert-core`. The interval and rational evaluators are ordinary compiled code in `rupert-core`, because shapes and verifiers both need the same expression semantics. `rupert-verify` owns certification policy and calls into those evaluators.

## What to NOT do during this work

- **Don't** rewrite the f64 hot path. Solvers must stay on the f64 `Polyhedron::vertices`; the exact vertices are verifier-only. Anyone who touches `EvalCounter::evaluate` to use `Expr` instead of f64 has misunderstood the architecture.
- **Don't** introduce a `Real` trait abstracting over `f64` / `Interval` / `Rational`. We tried this in earlier scaffold sketches; it makes every solver generic for no benefit (solvers don't need exact math). Concrete f64 in solvers, concrete `Expr` in shapes/verifier — separate lanes.
- **Don't** add cube-roots / general algebraic-number arithmetic. Stay scoped to the closed primitive set above.

## Open design questions

1. **Should `Expr` be canonicalized?** If a shape says `Mul(Two, GoldenRatio)` and another says `Add(GoldenRatio, GoldenRatio)`, are they equal expressions? For verification we don't need equality; for human readability we might. v2 punt: no canonicalization, document the convention in `crates/rupert-core/src/expr.rs`.
2. **Hash invariance under expression equality.** `PolyId` currently hashes f64 vertex bytes. After v2, equivalent expressions with bit-different f64 evaluations would hash differently. Either canonicalize before hashing, or hash the expression tree (more robust but more complex). My lean: hash the expression tree's *canonical f64 evaluation*, document that two expressions producing the same vertices yield the same `PolyId`.
3. **Tribonacci as primitive vs. computed root.** We hardcode tribonacci to a known-good interval. An alternative is to store it as `RootOfPolynomial(coefficients)` and use a generic root-isolation algorithm. The latter is more flexible but introduces serious algebraic-number machinery. v2 stays with the hardcoded primitive.
4. **Performance budget for verification.** Currently F64Epsilon recomputation is microseconds. Interval-arithmetic recomputation is ~10× slower; exact-rational is ~100× slower. Should we cache results or just live with the slower verifier? My lean: live with it; verification is per-result, not per-iteration.

## Future triggers

- A new shape needs algebraic-number coordinates beyond the closed primitive set.
- The verifier needs tighter intervals for a primitive than the current tabulated bounds provide.
- The leaderboard begins to stratify by cert method beyond the current display column.
- A refutation-track implementation needs expression export into a proof/certificate format.
