# Architecture

## Why this crate graph

We split foundation, capability-math, capability-orchestration, and binary into separate crates so each can be tested, swapped, and reasoned about independently. The `xtask check-deps` gate enforces dependency direction.

```
L0  rupert-core         — Vec3, Quat, Polyhedron, projection, hull2d, clearance,
                          Solver trait, EvalCounter, RunResult schema. Algebraic
                          DSL: Expr, ExactVec3, eval_f64/eval_interval. Interval
                          hull (combinatorial-precommit). Symmetry groups
                          (T/O/I/dodec). Pure math; no I/O. New solvers shouldn't
                          change this layer.
L1  rupert-shapes       — 8 builtin polyhedra (with exact vertex tables) + JSON
                          I/O. Adding a new builtin shape touches this crate
                          only. Exposes rotation_group_for(name) for solvers
                          that exploit symmetry.
L1  rupert-solvers      — Solver implementations. One module per algorithm.
                          The leaderboard target. New solver = new module + one
                          line in `registered_solvers()`.
L1  rupert-leaderboard  — Aggregate JSONL run results into LEADERBOARD.md;
                          tracks per-row CertMethod (F64Epsilon / IntervalSnap).
L2  rupert-verify       — Snap-and-certify verifier. Shipped paths:
                          `certify` (F64Epsilon) and `certify_interval`
                          (IntervalSnap, requires exact_vertices). The exact
                          (`malachite`-Rational) path is roadmapped, not yet
                          shipped.
L3  rupert-bench        — Single-cell run + parallel sweep. Owns the JSONL
                          writer and the rayon parallelism boundary. The
                          run_one harness attempts IntervalSnap first, falls
                          back to F64Epsilon.
L4  bin/rupert-cli      — Thin clap dispatch. ≤100 LOC main + small per-cmd
                          modules.
```

Tooling: `xtask` (workspace gates) sits at layer 99 — it depends on whatever it needs and isn't part of the run-time graph.

## Test surface

- Numerical crates use `#[cfg(test)] mod tests` + `proptest` + `insta` (declarative invariants, snapshot tests of vertex tables, etc.). 140+ tests today.
- Orchestration crates use BDD scenarios via `cucumber-rs` under `tests/bdd/features/`. Behavior IDs `B-XXXX` tag scenarios. The `xtask check-rust-test-surface` gate enforces this distinction (`xtask/test-surface.toml`).

## Determinism story

A `RunResult` is fully reproducible from `(solver_name, solver_version, polyhedron_id, seed, budget)`. The harness wires:

1. `Budget::seed` is the only randomness input. Solvers seed `rand_xoshiro::Xoshiro256PlusPlus` from it; no `OsRng`, no thread-locals, no clock seeds.
2. `EvalCounter` is the only path from `Candidate` to clearance — solvers can't bypass and lie about eval counts. (xtask gate could be tightened to forbid the `evaluate_clearance` import in solver modules.)
3. `EvalCounter: !Sync`. Solvers can't use it across threads. Parallelism happens only at the `(shape, solver, seed)` triple level in `rupert-bench::sweep`.
4. The patch_aware solver caches its per-shape patch table behind a `OnceLock<Mutex<HashMap<PolyId, Arc<PatchTable>>>>`. Patch enumeration RNG is seeded from `blake3(shape_name)`, NOT from `Budget::seed` — so the patch table is deterministic per shape and reused across all seeds.

## The algebraic coordinate layer

`rupert-core` ships an `Expr` enum (closed under +, −, ×, ÷, plus the primitives `Rational`, `Sqrt`, `GoldenRatio`, `Tribonacci`, `Cos`, `Sin`, `Pi`) and an `ExactVec3` triple-of-Expr. Each builtin shape stores its vertices as `Vec<ExactVec3>` and derives the f64 vertex array via `eval_f64()`. The interval evaluator (`eval_interval`) lifts every primitive into `inari::Interval` with rigorous enclosure — tabulated for the algebraic constants, propagated through arithmetic, and computed via Taylor series with rigorous remainder bounds for trig.

Solvers consume only the f64 `Polyhedron::vertices` field — they're unaware of the algebraic layer. The verifier uses `exact_vertices` for `IntervalSnap`. This separation is enforced architecturally: the solver hot path stays in f64 (microseconds per evaluation) while certification pays the ~10× interval-arithmetic cost only when promoting a candidate.

See [`v2-algebraic-coords.md`](v2-algebraic-coords.md) for the design rationale.

## Future (NOT shipped)

Captured here so future contributors don't accidentally rebuild things or extend the wrong layer. See [`roadmap.md`](roadmap.md) for the prioritized version with code-vs-experiments split.

- **GPU kernels** (CUDA/Metal/wgpu): would slot in as a sibling of `rupert-solvers` at L1, with its own crate to keep heavy deps out.
- **SDP / Lasserre hierarchies**: `clarabel` integration; new crate `rupert-solvers-sdp`.
- **Autodiff**: `burn` or `candle` for differentiable silhouettes; new crate, plumb gradient hooks via a trait extension.
- **LP scale-oracle** (Zeng 2026): per-rotation-pair, the optimal translation is the Chebyshev center of `outer_hull ⊖ inner_hull` — an LP. Currently translation is a free search variable.
- **Branch-and-bound across patch_aware cells**: uses `convex_hull_interval_certified` to derive interval upper bounds on per-patch clearance, pruning hopeless cells without a single Nelder-Mead step. This is the single highest-impact algorithmic improvement still on the table.
- **`certify_exact` via malachite**: rational-arithmetic certification for the rational-vertex shapes (cube, tet, octa, triakis tet). Roadmapped as v0.3.0.
- **Refutation track**: interval-arithmetic non-existence proofs (Steininger-Yurkevich style). A whole separate evaluation criterion (cells eliminated per CPU-hour).
- **Lean export**: emit `.lean` certificates from certified solutions for formal verification.
- **External / subprocess solvers**: deliberately excluded. The contribution flow is "port to Rust" — see `docs/porting.md`.
