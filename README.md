# rupert-solver

A Rust workspace + leaderboard for the **Rupert property** of convex polyhedra: can a copy of a polyhedron pass through a hole cut in a same-size copy of itself? Some can (cube, dodecahedron); some can't (the [noperthedron](https://arxiv.org/abs/2508.18475), proven non-Rupert in 2025); and several common solids — snub cube, snub dodecahedron, rhombicosidodecahedron, deltoidal hexecontahedron, pentagonal hexecontahedron — remain open.

This repo is a **benchmarking harness** for solver implementations, designed for AI-driven iteration. Headless agents read the leaderboard, port or invent a solver, run the bench, and produce certified results. Over time the leaderboard tightens; novel solvers can attack open problems.

## Quickstart

```bash
git clone https://github.com/trevorWieland/rupert-solver
cd rupert-solver
just bootstrap                          # one-time, ~5 min: rust toolchain + dev tools
just ci                                 # sanity gate: fmt, clippy, xtask, deny, machete, test
cargo run --release -p rupert-cli -- list shapes
cargo run --release -p rupert-cli -- list solvers
cargo run --release -p rupert-cli -- run \
    --shape cube --solver random_then_refine --seed 0 --budget-evals 50000
cargo run --release -p rupert-cli -- verify results
cargo run --release -p rupert-cli -- lead build
cat LEADERBOARD.md
```

`just sweep` runs every shape × every solver and rebuilds the leaderboard.

## Adding a solver

See [`docs/adding-a-solver.md`](docs/adding-a-solver.md). One-liner: drop `crates/rupert-solvers/src/<name>.rs`, append one line to `registered_solvers()` in `lib.rs`. Run the bench, run verify, run lead.

## What's shipped (v1)

- 7 crates + 1 binary; flat workspace; edition 2024; MSRV 1.85.
- 8 builtin polyhedra (cube, tetra, octa, dodec, icos, triakis tetra, snub cube, noperthedron-placeholder).
- 5 baseline solvers (`random_quat`, `face_normal_pairs`, `nelder_mead`, `random_then_refine`, `hopf_grid`).
- F64Epsilon certifier with `inari` interval / `malachite` exact paths feature-gated for v2.
- BDD scenarios on the orchestration layer (cucumber-rs).
- xtask gates: suppression, test-surface, deps-layering, file-line-count, cargo-fields-inheritance.
- ~110 unit/integration tests; clippy clean under workspace pedantic lints; `just ci` is the single gate.

## References

- Steininger & Yurkevich, *A convex polyhedron without Rupert's property*, [arXiv:2508.18475](https://arxiv.org/abs/2508.18475) (2025).
- Gosain & Grimmer, *Some new insights from highly optimized polyhedral passages*, [arXiv:2509.08190](https://arxiv.org/abs/2509.08190) (2025).
- Fredriksson, *Optimizing for the Rupert property*, [arXiv:2210.00601](https://arxiv.org/abs/2210.00601) (2022).
- Tom 7 / suckerpinch, *Rupert's snub cube and other math holes*, [SIGBOVIK 2025](http://tom7.org/ruperts/).

## License

Dual MIT-or-Apache. See `LICENSE-MIT`, `LICENSE-APACHE`.
