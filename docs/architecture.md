# Architecture

## Why this crate graph

We split foundation, capability-math, capability-orchestration, and binary into separate crates so each can be tested, swapped, and reasoned about independently. The `xtask check-deps` gate enforces dependency direction.

```
L0  rupert-core         — Vec3, Quat, Polyhedron, projection, hull2d, clearance,
                          Solver trait, EvalCounter, RunResult schema. Pure math,
                          no I/O. Stable; new solvers shouldn't change this layer.
L1  rupert-shapes       — 8 builtin polyhedra + JSON I/O. Adding a new builtin
                          shape touches this crate only.
L1  rupert-solvers      — Solver implementations. One module per algorithm.
                          The leaderboard target. New solver = new module + one
                          line in `registered_solvers()`.
L1  rupert-leaderboard  — Aggregate JSONL run results into LEADERBOARD.md.
L2  rupert-verify       — Snap-and-certify verifier. v1 ships F64Epsilon;
                          interval (`inari`) and exact (`malachite`) paths
                          feature-gated.
L3  rupert-bench        — Single-cell run + parallel sweep. Owns the JSONL
                          writer and the rayon parallelism boundary.
L4  bin/rupert-cli      — Thin clap dispatch. ≤100 LOC main + small per-cmd
                          modules.
```

Tooling: `xtask` (workspace gates) sits at layer 99 — it depends on whatever it needs and isn't part of the run-time graph.

## Test surface

- Numerical crates use `#[cfg(test)] mod tests` + `proptest` + `insta` (declarative invariants, snapshot tests of vertex tables, etc.). 110+ tests today.
- Orchestration crates use BDD scenarios via `cucumber-rs` under `tests/bdd/features/`. Behavior IDs `B-XXXX` tag scenarios. The `xtask check-rust-test-surface` gate enforces this distinction (`xtask/test-surface.toml`).

## Determinism story

A `RunResult` is fully reproducible from `(solver_name, solver_version, polyhedron_id, seed, budget)`. The harness wires:

1. `Budget::seed` is the only randomness input. Solvers seed `rand_xoshiro::Xoshiro256PlusPlus` from it; no `OsRng`, no thread-locals, no clock seeds.
2. `EvalCounter` is the only path from `Candidate` to clearance — solvers can't bypass and lie about eval counts. (xtask gate could be tightened to forbid the `evaluate_clearance` import in solver modules.)
3. `EvalCounter: !Sync`. Solvers can't use it across threads. Parallelism happens only at the `(shape, solver, seed)` triple level in `rupert-bench::sweep`.

## Future (NOT in v1)

Captured here so future contributors don't accidentally rebuild things or extend the wrong layer:

- **GPU kernels** (CUDA/Metal/wgpu): would slot in as a sibling of `rupert-solvers` at L1, with its own crate to keep heavy deps out.
- **SDP / Lasserre hierarchies**: `clarabel` integration; new crate `rupert-solvers-sdp`.
- **Autodiff**: `burn` or `candle` for differentiable silhouettes; new crate, plumb gradient hooks via a trait extension.
- **LP scale-oracle** (Zeng 2026): per-rotation-pair, the optimal translation is the Chebyshev center of `outer_hull ⊖ inner_hull` — an LP. Currently translation is a free search variable.
- **Patch decomposition**: Tom 7's combinatorial-class enumeration. Would add a new sub-crate for the patch index; combine with branch-and-bound.
- **Refutation track**: interval-arithmetic non-existence proofs (Steininger-Yurkevich style). A whole separate evaluation criterion (cells eliminated per CPU-hour) and a new module under `rupert-verify`.
- **Algebraic-number coord DSL**: `Sqrt(N)`, `Tribonacci`, `GoldenRatio` symbolic expressions over rationals, with `f64` and `inari` evaluators. Replaces v1's hardcoded f64 approximations for the snub cube and noperthedron.
- **Lean export**: emit `.lean` certificates from certified solutions for formal verification.
- **External / subprocess solvers**: deliberately excluded. The contribution flow is "port to Rust" — see `docs/porting.md`.
