# AGENTS.md

This file is the brief for headless coding agents (Claude Code, Codex, OpenCode) working on this repo. **Read all of it.** It supersedes whatever you think you know about the project.

## What this project is

A Rust workspace that benchmarks solver implementations for the **Rupert property** of convex polyhedra: can a copy of a polyhedron pass through a hole cut in a same-size copy of itself? See `docs/geometry.md` for the math.

The edge isn't a single new solver. It's the **leaderboard-driven AI-iteration loop**. You read `LEADERBOARD.md`, port or invent a solver, drop it in `crates/rupert-solvers/src/<name>.rs`, run the bench, produce a new certified result. Repeat.

## How to add a new solver (the only contribution flow that matters)

```bash
# 1. Pick a name and a parent algorithm. Read docs/solvers.md for what
#    we already have. Read docs/porting.md if you're translating from
#    Python / C++ / Lisp / Julia.

# 2. Drop a new file:
#    crates/rupert-solvers/src/<name>.rs
#    implementing rupert_core::Solver. Look at random_quat.rs as the
#    minimal template; nelder_mead.rs as the more involved one.

# 3. Register it. ONE line in crates/rupert-solvers/src/lib.rs:
#       Box::new(MyNewSolver),
#    inside `registered_solvers()`. Nothing else changes.

# 4. Iterate.
just c                    # cargo clippy --workspace --all-targets
just t                    # cargo nextest run --workspace
just ci                   # full gate (fmt + clippy + xtask + deny + machete + test)
cargo run --release -p rupert-cli -- run \
    --shape cube --solver <name> --seed 0 --budget-evals 50000
cargo run --release -p rupert-cli -- verify results
cargo run --release -p rupert-cli -- lead build
```

The leaderboard updates from the JSONL files in `results/`. Commit your solver + an entry pinning the seed/budget you used.

## Hard rules (workspace lints enforce these — see Cargo.toml)

- **No `unwrap` / `panic!` / `todo!` / `unimplemented!` / `println!` / `eprintln!` / `dbg!`** anywhere. Use `?`, `expect("clear message")` (in tests only), `tracing::info!` for logging, `writeln!(stdout, ...)` for CLI output.
- **No `unsafe` code.** Forbidden, not warn-able.
- **No inline `#[allow(...)]` / `#[expect(...)]`.** Lint relaxations live in workspace `[lints]` only. The `xtask check-suppression` gate enforces.
- **500 lines/file, 100 lines/fn.** `xtask check-lines` and clippy's `too_many_lines` enforce. Split into submodules when you hit the limit.
- **`thiserror` in libs, `anyhow` in bins.**
- **Determinism.** All RNG via `rand_xoshiro::Xoshiro256PlusPlus` seeded from `Budget::seed`. No `OsRng`, no thread-locals, no clock seeds.
- **EvalCounter is the only path** from a `Candidate` to a clearance number. Never call `rupert_core::evaluate_clearance` directly from a solver — it bypasses the counter and would falsify the leaderboard. (The xtask gate could be tightened later to forbid that import in `crates/rupert-solvers/src/*.rs`.)

## Crate graph (don't break the layering — `xtask check-deps` enforces)

```
L0 rupert-core         (foundation: geometry, projection, hull, clearance, Solver trait)
L1 rupert-shapes       (8 builtin polyhedra + JSON I/O; deps: core)
L1 rupert-solvers      (5 baseline solvers; deps: core)
L1 rupert-leaderboard  (aggregate JSONL → LEADERBOARD.md; deps: core)
L2 rupert-verify       (snap-and-certify verifier; deps: core, shapes)
L3 rupert-bench        (single-run + parallel sweep; deps: core, shapes, solvers, verify)
L4 bin/rupert-cli      (the user-facing binary; deps: all of the above)
```

`xtask` is layer 99 (out-of-band tooling).

## Tests

- **Numerical crates** (`rupert-core`, `rupert-shapes`, `rupert-solvers`, `rupert-verify`, `xtask`) use plain `#[cfg(test)] mod tests` with `proptest` and `insta`.
- **Orchestration crates** (`rupert-bench`, `rupert-leaderboard`, `bin/rupert-cli`) MUST have BDD scenarios under `tests/bdd/features/*.feature` driven by cucumber-rs. The `xtask check-rust-test-surface` gate enforces this distinction (config: `xtask/test-surface.toml`).
- Behavior IDs `B-XXXX` go in feature tags. The CLI BDD test drives the *built binary* via subprocess — that's the integration boundary.
- Run `just t` (nextest) for plain tests, `just ci` for the full gate.

## Verification semantics (read this before touching the verifier)

A run produces a `RunResult` with `outcome ∈ {Solved, Exhausted, Error, Disqualified}`. Only `Solved` rows with a non-`None` `solution.certification` are eligible for the leaderboard headline. `rupert verify` re-runs `rupert_verify::certify` on each result; if the recomputed clearance ≤ `F64_EPS = 1e-9`, the row becomes `Disqualified`.

v1 ships `CertMethod::F64Epsilon` only. The interval (`inari`) and exact (`malachite`) paths are feature-gated and arrive in v2 along with the algebraic-number coordinate DSL needed for the snub cube and the noperthedron's exact-paper construction.

## What is NOT in v1 (do not start these without explicit user approval)

GPU kernels, SDP / Lasserre hierarchies, autodiff, LP scale-oracle (translation is a free search variable in v1), patch decomposition, refutation-track infrastructure, algebraic-number coord DSL, snub cube proof attempt, Lean / Coq export, external/subprocess solvers, wasm builds, hosted leaderboard, multi-precision floats. These are tracked under `docs/architecture.md#future`.

## When in doubt

- `docs/geometry.md` — what the Rupert property actually means + projection convention.
- `docs/solvers.md` — what each baseline solver does, when it excels, when it fails.
- `docs/verification.md` — snap-and-certify pattern + the noperthedron caveat.
- `docs/porting.md` — translating an external solver into `rupert-solvers/src/<name>.rs`.
- `docs/adding-a-solver.md` — minimal new-solver template.
- `docs/architecture.md` — crate graph rationale + future-roadmap.
- `docs/v2-algebraic-coords.md` — design notes for the algebraic-coordinate DSL that unlocks the snub cube + dodecahedron + noperthedron interval-verification paths. Read before starting any v2 verifier work.

The plan that birthed this scaffold lives at `~/.claude/plans/virtual-booping-graham.md` (read-only context). The arXiv references are in the project README.
