# rupert-solver

A Rust workspace + leaderboard for the **Rupert property** of convex polyhedra: can a copy of a polyhedron pass through a hole cut in a same-size copy of itself? Some can (cube, dodecahedron); some can't (the [noperthedron](https://arxiv.org/abs/2508.18475), proven non-Rupert in 2025); and several common solids — snub cube, snub dodecahedron, rhombicosidodecahedron, deltoidal hexecontahedron, pentagonal hexecontahedron — remain open.

This repo is a **benchmarking harness** for solver implementations, designed for AI-driven iteration. Headless agents read the leaderboard, port or invent a solver, run the bench, and produce certified results. Over time the leaderboard tightens; novel solvers can attack open problems.

## Quickstart

```bash
git clone https://github.com/trevorWieland/rupert-solver
cd rupert-solver
just bootstrap                          # one-time, ~5 min: rust toolchain + dev tools
just check                              # fast gate: fmt-check, cargo check, clippy, xtask
just ci                                 # full gate: check, deny, machete, test, docs
cargo run --release -p rupert-cli -- list shapes
cargo run --release -p rupert-cli -- list solvers
cargo run --release -p rupert-cli -- run \
    --shape cube --solver random_then_refine --seed 0 --budget-evals 50000 \
    --out-dir results/baseline
cargo run --release -p rupert-cli -- verify results/baseline
cargo run --release -p rupert-cli -- lead build
cargo run --release -p rupert-cli -- analyze summary --results-dir results/baseline
cat LEADERBOARD.md
```

`just sweep` writes a curated baseline sweep under `results/baseline/`,
verifies it, and rebuilds the leaderboard. Scratch runs under `results/` are
ignored by `rupert lead build` unless passed explicitly with `--results-dir`.
See
[`docs/experiments.md`](docs/experiments.md) for calibration and hunt recipes.

## Adding a solver

See [`docs/adding-a-solver.md`](docs/adding-a-solver.md). One-liner: drop `crates/rupert-solvers/src/<name>.rs`, append one line to `registered_solvers()` in `lib.rs`. Run the bench, run verify, run lead.

## What's shipped

- 6 crates + 1 binary; flat workspace; edition 2024; MSRV 1.86.
- **9 builtin polyhedra** (cube, tetrahedron, octahedron, dodecahedron, icosahedron, triakis tetrahedron, pentagonal icositetrahedron, snub cube, noperthedron). All builtin shapes carry exact algebraic vertex tables (`ExactVec3`) for interval-arithmetic verification; rational-coordinate shapes can additionally certify through `ExactRational`.
- **8 baseline solvers**: `random_quat`, `face_normal_pairs`, `nelder_mead`, `random_then_refine`, `hopf_grid`, `imperts` (port of Tom 7's SIGBOVIK 2025 refiner), `gosain_grimmer` (port of arXiv:2509.08190 subgradient), `patch_aware` (Tom 7's patch decomposition; v0.3.0 with recon-first adaptive scan, deterministic upper bounds, and experiment telemetry).
- **Three certification tiers**: `ExactRational` (malachite rational recomputation for rational-coordinate shapes), `IntervalSnap` (rigorous lower-bound via `inari` interval arithmetic over algebraic vertex tables), and `F64Epsilon` (f64 self-consistency). The bench runner attempts them in that order.
- **Real symmetry groups**: order-12 tetrahedral, order-24 octahedral, order-60 icosahedral, order-60 dodecahedral, all validated by per-shape vertex-set permutation tests.
- **Combinatorial-precommit interval hull** (`hull2d_interval`), implementing the Steininger–Yurkevich technique for soundly certifying convex hulls under interval arithmetic.
- BDD scenarios (cucumber-rs) on the orchestration layer; `xtask` gates for suppression, test-surface, deps-layering, file-line-count, cargo-fields-inheritance.
- 140+ unit/integration tests; clippy clean under workspace pedantic lints; `just ci` is the single gate.

## Documentation map

- [`docs/architecture.md`](docs/architecture.md) — crate graph, layering, determinism story.
- [`docs/geometry.md`](docs/geometry.md) — Rupert problem definition, clearance metric, projection conventions.
- [`docs/solvers.md`](docs/solvers.md) — per-solver algorithm summaries and known failure modes.
- [`docs/verification.md`](docs/verification.md) — ExactRational + IntervalSnap + F64Epsilon pipeline; the noperthedron soundness gate.
- [`docs/adding-a-solver.md`](docs/adding-a-solver.md) — three-step recipe for new solvers.
- [`docs/porting.md`](docs/porting.md) — porting external (Python/C++) solvers into Rust.
- [`docs/v2-algebraic-coords.md`](docs/v2-algebraic-coords.md) — algebraic coordinate DSL design notes.
- [`docs/experiments.md`](docs/experiments.md) — calibration targets, result hygiene, and hunt recipes.
- [`docs/roadmap.md`](docs/roadmap.md) — code roadmap *and* experiment ladder for large-budget runs.

## References

- Steininger & Yurkevich, *A convex polyhedron without Rupert's property*, [arXiv:2508.18475](https://arxiv.org/abs/2508.18475) (2025).
- Gosain & Grimmer, *Some new insights from highly optimized polyhedral passages*, [arXiv:2509.08190](https://arxiv.org/abs/2509.08190) (2025).
- Fredriksson, *Optimizing for the Rupert property*, [arXiv:2210.00601](https://arxiv.org/abs/2210.00601) (2022).
- Steininger & Yurkevich, *An algorithmic approach to Rupert's problem*, [arXiv:2112.13754](https://arxiv.org/abs/2112.13754) (2021).
- Tom 7 / suckerpinch, *Rupert's snub cube and other math holes*, [SIGBOVIK 2025](http://tom7.org/ruperts/).

## License

Dual MIT-or-Apache. See `LICENSE-MIT`, `LICENSE-APACHE`.
