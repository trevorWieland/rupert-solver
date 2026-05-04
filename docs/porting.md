# Porting an External Solver

We deliberately don't support subprocess/external solvers. To add an external algorithm to the leaderboard, you must **port it into Rust** as a new module under `crates/rupert-solvers/src/`. This keeps everything reproducible, single-language, and benchmark-comparable.

## When this applies

You found a solver in Python, C++, Julia, Lisp, or any other language — typically as part of a published paper or hobbyist project. The literature points: [Gosain & Grimmer 2025](https://github.com/RajGosain13/RupertResults), [Steininger & Yurkevich's repo](https://github.com/Jakob256/Rupert), [Tom 7's SVN](https://sourceforge.net/p/tom7misc/svn/HEAD/tree/trunk/ruperts/), and so on.

## Step-by-step

### 1. Read the source — find the inner objective

Every Rupert solver has the same shape: it explores some parameter space (rotations + translation) and minimizes the same kind of clearance objective. Find:

- **What parametrization?** Quaternions, Euler angles, axis-angle, "log-quaternion tangent vectors near a base point"? Translate to quaternion + 2D translation when calling `EvalCounter::evaluate`.
- **What optimizer?** Nelder-Mead, CMA-ES, BFGS, custom subgradient, simulated annealing, genetic, gradient via finite difference, …? Find a Rust crate (`argmin` covers many) or implement directly. Check workspace dependencies first; consider whether the new dep is worth the compile-time cost.
- **How is clearance computed in the source?** Should match our pipeline (rotate → project → 2D hull → signed point-to-polygon). If their definition differs (e.g. different sign convention, scale parameter), reconcile before porting — adjust their objective to match ours, not the other way around.
- **Determinism?** The source might use `np.random.rand()` (numpy default RNG, non-deterministic) or `rand()` (libc) — replace with `Xoshiro256PlusPlus` seeded from `Budget::seed`.

### 2. Write the Rust translation

Follow [`adding-a-solver.md`](adding-a-solver.md). For non-trivial algorithms, the line limit (500/file, 100/fn) means you'll likely split into a directory module:

```
crates/rupert-solvers/src/cma_es/
    mod.rs          — Solver impl + entry point
    sigma.rs        — covariance matrix update
    sample.rs       — population sampling
    selection.rs    — fitness ranking
```

Match the name closely to the source: `cma_es` if porting CMA-ES, `gosain_grimmer` if porting their specific subgradient method, etc.

### 3. Document provenance

In the module header:

```rust
//! Ported from <author>, "<paper title>", <venue> <year>, arXiv:NNNN.NNNNN.
//! Original source: <URL>. License: <theirs>.
//!
//! Notable adaptations:
//! - Replaced numpy random_state with Xoshiro256PlusPlus (deterministic).
//! - Reformulated their objective `f(theta)` as `-clearance(candidate)` so
//!   our maximization aligns with their minimization.
//! - <other adaptations>
```

Provenance is non-optional. The leaderboard's integrity depends on tracing every solver back to a verifiable source.

### 4. License compliance

Workspace allows MIT, Apache-2.0, BSD-{2,3}-Clause, ISC, Zlib, BSL-1.0, CC0-1.0, Unicode-3.0, BlueOak-1.0.0, CDLA-Permissive-2.0, OpenSSL. If the upstream license is **GPL/LGPL/AGPL/MPL-2.0**, do NOT port — `cargo deny check` will reject any added dep with a copyleft license, and we don't want to relicense our own crate.

For ports of *algorithms* (rather than copy-pasted code), the algorithmic ideas themselves are not copyrighted — re-implementing them from a paper description is fine and is the preferred path.

### 5. Verify it works

Run the bench. Beat the existing baselines on at least one shape, ideally on multiple. Submit the leaderboard delta.

## Why we do it this way

Subprocess contracts are tempting but break:

- **Reproducibility.** A subprocess solver is parameterized by an external interpreter version, OS-specific syscalls, `numpy` ABI, etc. Rust ports run on a single pinned toolchain.
- **Determinism.** The deterministic RNG story breaks across language boundaries.
- **Benchmark fairness.** Subprocess overhead dominates `eval_count`-based budgets for cheap inner loops.
- **Single point of contribution.** "Port to Rust" is the contribution; passing through a wrapper trivializes the work and weakens the project's central artifact (a clean, in-process Rust harness). The whole leaderboard is one `cargo run` away on a fresh clone.

## Examples to port (priority order)

1. **Gosain & Grimmer 2025** — trust-region linearized-min-of-smooth subgradient. Currently SOTA on clearance margins. Python source is public.
2. **Steininger & Yurkevich 2021** — probabilistic Rupertness algorithm. R/Python source.
3. **Fredriksson 2022** — nonlinear optimization that found the triakis tetrahedron passage. Source is unpublished as of writing.
4. **Tom 7's `imperts.cc`** — refinement loop that converged on highly-optimized passages. C++ source on SourceForge.

If you port any of these, add a row to [`solvers.md`](solvers.md) describing the algorithm, its strengths, and its known failure modes.
