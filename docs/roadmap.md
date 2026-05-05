# Roadmap

Two parallel tracks: **code** (engineering work to broaden what the harness can certify and accelerate) and **experiments** (large-budget compute runs that don't require new code, just patience and CPU). The former unblocks the latter.

The headline goals — find a snub cube passage, formally certify the noperthedron rejection, prove non-Rupert-ness for a *new* polyhedron — sit at the top of both tracks.

---

## Code track

Ordered by leverage (impact ÷ effort), with explicit dependencies.

### v0.3.0 — Branch-and-bound across patch_aware cells

**Status**: not started. Highest leverage item on the list.

**What.** Today `patch_aware` v0.2.0 reconnaissance-first scans cells in score order and skips those whose recon score lags `best_so_far` by more than `SKIP_SLACK = 0.5`. That heuristic prunes by the *anchor sample alone*. The next step is to replace the heuristic with an **interval upper bound** on per-cell clearance:

- For each canonical (outer, inner) cell, build an `IntervalP2` shadow projection where the rotation is allowed to vary across the entire 0.15-wide quat-delta box around the patch anchor. The result is a polygon whose vertices are wider intervals than the IntervalSnap path (which uses singleton f64 quaternions).
- Compute an interval upper bound on signed clearance over that box (via `convex_hull_interval_certified` and the interval signed-distance pipeline already in `rupert-verify::interval_cert`).
- If the upper bound is ≤ `best_so_far`, **skip the cell** without spending a single Nelder-Mead step. This is exact pruning, not a heuristic.

**Why.** On the snub cube (1296 cells × ~1500 evals/cell ≈ 2M evals to fully scan), even cutting 50% of cells via rigorous bounds gets us to 1M evals on cells that *might* yield a passage. Combined with adaptive per-cell budgets (give pruned-but-not-skipped cells more evals), this is the realistic path to either finding a snub cube passage *or* proving none exists in some patch product.

**Crate(s)**: `rupert-solvers` (uses `rupert-verify`'s interval primitives).

**Estimated effort**: 800–1200 LOC. The interval projection over a quat-delta box (rather than a singleton quat) is the new piece; the rest reuses existing machinery.

**Risk**: medium. The bounds might be too loose on cells with 0.15 wide deltas — empirically open. If too loose, narrow the delta box at the cost of more cells (subdivide patches into sub-cells).

### v0.3.0 — `certify_exact` via malachite

**Status**: pending. `Cargo.toml` already gates `malachite` behind `exact`.

**What.** For shapes whose vertices are pure rationals (cube, tetrahedron, octahedron, triakis tetrahedron, noperthedron seeds), recompute the projection-hull-clearance pipeline in `malachite::Rational`. No floating point, no rounding error. The result is a Boolean: rational clearance is either > 0 or ≤ 0; we don't get an approximation, we get the truth.

The challenge: the candidate quaternion is f64. Two options:

- **Snap-to-rational.** Round each quaternion component to a denominator ≤ 2³² rational (continued fraction). Then certify in exact arithmetic against the *snapped* quaternion. The candidate is rejected unless the snapped clearance is also strictly positive — strictly stronger than IntervalSnap.
- **Range-rational.** Represent each quaternion component as the rational interval `[next_down(f), next_up(f)]` and propagate. Loses the "exact" promise but avoids rounding errors at the snap step.

**Why.** Two payoffs. (1) The headline noperthedron regression — currently empirical at 50 000 evals — would graduate to "no candidate (after f64-to-rational snap) certifies a passage in exact rational arithmetic". The snap step has provable error bounds: a candidate that's *truly* a passage is preserved, a candidate that's an f64 rounding artifact is rejected. (2) For cube/tet/octa the leaderboard's `cert` column would show `exact` for the strongest tier — a meaningful integrity signal.

**Crate(s)**: `rupert-verify` (+ feature-gate plumbing in `rupert-core` to expose `Expr::eval_rational`).

**Estimated effort**: 400 LOC.

**Risk**: low. Algorithm is straightforward; the only design choice is snap-vs-range. Recommendation: snap, with an interval-arithmetic guard that the snap doesn't change combinatorial structure of the hull.

### v0.3.0 — Update noperthedron headline regression

**Status**: pending. Depends on `certify_exact` *or* the existing `certify_interval` (whichever lands first).

**What.** The current `noperthedron_resists_every_v1_solver` test in `rupert-bench` runs every solver × 3 seeds × 50k evals and asserts `RunOutcome::Solved` never appears. Replace the assertion with: "if any solver returns Found, certify_interval (or certify_exact) MUST reject it."

**Why.** Today: empirically robust at 50k evals. Tomorrow: formally robust at any candidate. The verifier integrity proof matches the paper's SageMath certificate.

**Crate(s)**: `rupert-bench`.

**Estimated effort**: 30 LOC.

**Risk**: low.

### v0.3.0 — Snub cube graduation: confirm IntervalSnap

**Status**: framework shipped, not exercised because no passage has been found.

**What.** When (if) a solver finds a snub cube passage, the bench runner's IntervalSnap-first logic will attempt rigorous certification automatically. We should add a unit test that *fakes* a snub cube passage candidate (e.g. via `face_normal_pairs` on a heavily-perturbed snub cube where we know a passage exists) to confirm the IntervalSnap path's tribonacci-interval propagation is tight enough.

**Why.** Gold-plating: if patch_aware v0.3.0 ever finds a snub cube candidate, we want zero certification surprises.

**Estimated effort**: 100 LOC test code.

### v0.4.0 — LP scale oracle (Zeng 2026)

**What.** Per (q_outer, q_inner) pair, the optimal translation `t` is the 2D **Chebyshev center** of `outer_hull ⊖ inner_hull` (Minkowski difference). This is an LP. Currently the harness treats translation as a free search variable.

**Why.** Eliminates 2 of the 10 search dimensions for every solver. For deterministic solvers (`face_normal_pairs`, `hopf_grid`) this is a strict speedup; for stochastic ones it improves convergence. Zeng uses this as the basis of a stellated-tetrahedron non-Rupert proof.

**Crate(s)**: new `rupert-core::scale_oracle` module + `clarabel` (or `microlp`) workspace dep.

**Estimated effort**: 400 LOC.

**Risk**: low (LP solvers are well-understood).

### v0.4.0 — Differentiable silhouette (autodiff)

**What.** Wrap the projection-hull-clearance pipeline in `burn` or `candle` to obtain f64 gradients of clearance with respect to the 10-DOF candidate. Plumb a gradient hook into `Solver` via a trait extension.

**Why.** Gosain & Grimmer's published clearance numbers depend on symbolic gradients. Our `gosain_grimmer` solver currently uses finite differences (~8 evals per gradient step). Autodiff would let it match the paper.

**Crate(s)**: new `rupert-solvers-autodiff` (heavy dep, kept isolated).

**Estimated effort**: 1500 LOC.

**Risk**: medium. Convex hull is non-smooth at combinatorial transitions; gradient handling at those points needs care.

### v0.4.0 — Symmetry discovery for unannotated shapes

**What.** Today `rotation_group_for(name)` is hand-tabulated. Adding a new shape to the harness means writing a rotation group by hand. v0.4.0 should attempt to discover the rotation group automatically by sampling rotations and testing whether they permute the vertex set.

**Why.** Lowers the barrier for adding Johnson solids, Catalan solids, or one-off shapes proposed by users.

**Estimated effort**: 300 LOC.

**Risk**: low.

### v0.5.0 — GPU kernel for the projection-hull-clearance pipeline

**What.** Port `evaluate_clearance` to wgpu/CUDA. The inner loop is a per-vertex matrix-vector product + 2D hull + signed distance, all suitable for SIMT.

**Why.** Throughput. Current f64 evals are ~5–8 µs each on a single core; a GPU could do 10× more. Long-run experiments (1B+ evals on a single shape) become realistic.

**Crate(s)**: new `rupert-solvers-gpu`.

**Estimated effort**: 2000+ LOC.

**Risk**: high. Convex hull on GPU is tricky; might need a CPU fallback for the hull step with a GPU prefilter.

### v0.5.0 — SDP / Lasserre hierarchy

**What.** Formulate the Rupert search as a polynomial optimization problem with the Lasserre hierarchy and solve via SDP (e.g. `clarabel`). At sufficient hierarchy depth, this provides *certifiably optimal* clearance for a given polyhedron — and a non-Rupert proof for any shape it returns clearance ≤ 0 on.

**Why.** Currently no solver in the fleet provides global guarantees; they're all local (Nelder-Mead, gradient ascent, restart-augmented). SDP would be the first algorithm in the harness with the structure to actually *prove* non-Rupert-ness for a new shape (without an interval-arithmetic certificate).

**Crate(s)**: `rupert-solvers-sdp`.

**Estimated effort**: 4000+ LOC. Polynomial encoding alone is non-trivial.

**Risk**: very high. Lasserre depth scales rapidly; tractable depth might be too shallow to be informative on real shapes.

### v0.5.0 — Refutation track

**What.** A separate track from "find a passage": **prove no passage exists**. Steininger-Yurkevich's noperthedron proof is the model. Slot a `rupert_refute::cover_so3` algorithm that subdivides `SO(3)² × ℝ²` into cells and uses interval arithmetic to prove each cell admits no passage. Aggregates as "cells eliminated per CPU-hour".

**Why.** This is the formal flip side of the leaderboard. A noperthedron-shape candidate generated by us (via, say, perturbation of a known shape) would be the headline result.

**Crate(s)**: new `rupert-refute`.

**Estimated effort**: 5000+ LOC. Effectively a small certificate-generating proof assistant.

**Risk**: very high.

### v0.5.0 — Lean export

**What.** For an `IntervalSnap`-certified passage, emit a `.lean` file with a manually-checkable statement of the certificate. Mathlib has the requisite interval-arithmetic and convex-hull machinery.

**Why.** Gold-plates the soundness story end-to-end. A reviewer can compile the Lean and trust the result without trusting our Rust pipeline.

**Estimated effort**: 600 LOC + Lean expertise.

**Risk**: medium (depends on Mathlib API stability for the relevant lemmas).

---

## Experiment track

These don't require new code — just patience, compute, and willingness to commit results. Each is a separately-fundable run that could yield a publishable result.

### E1 — Snub cube blitz

**What.** Run `patch_aware` v0.2.0 with `--budget-evals 10_000_000` per (seed, cell-priority-order). Stratify across 16 seeds. Total: ~160M evaluations. Estimated wall time: 12–24 hours on a 32-core machine.

**Goal.** Find a positive-clearance candidate for the snub cube. Open since arXiv:2112.13754.

**Risk.** If patch_aware v0.2.0 still resists at 10M evals/seed, that's a strong empirical lower bound on the difficulty — and motivation to land v0.3.0's branch-and-bound.

**What to record.** Best clearance per cell across all seeds; histogram of best-clearance-per-cell distribution; identify any cell whose best clearance is consistently within 0.01 of zero (those are the "near-miss" candidates worth refining).

### E2 — Snub cube high-budget gosain-grimmer

**What.** Run `gosain_grimmer` with `--budget-evals 100_000_000` and many random restarts. Single-seed at this scale.

**Goal.** Independent attack on the snub cube using a fundamentally different parametrization (Gosain-Grimmer's 7-parameter form vs. patch_aware's 10-DOF delta). If both fail, that's evidence; if one succeeds, that's a result.

### E3 — Triakis tetrahedron precision boundary

**What.** The triakis tetrahedron has the smallest known positive clearance among Rupert solids (~4×10⁻⁶). Run `imperts` (or `gosain_grimmer` with autodiff once v0.4.0 lands) at 10M evals/seed × 32 seeds, recording the *highest clearance* achieved.

**Goal.** Push the published margin upward. The previous SOTA is Fredriksson 2022.

**Why this matters.** A higher certified clearance for triakis is a leaderboard entry that can stay valid forever — clearance only increases when refined.

### E4 — Noperthedron formal cover

**What.** Once `certify_exact` lands (v0.3.0), run a sweep of N=1M random rotation pairs against the noperthedron and verify every single one is rejected by IntervalSnap (and ExactRational where applicable).

**Goal.** Empirical confidence margin: at 1M random candidates, the false-positive rate is < 1ppm. Combined with the formal interval-arithmetic certificate from the paper, this is the strongest possible robustness story.

### E5 — Snub dodecahedron exploratory

**What.** Add the snub dodecahedron as a new shape (vertices use both `Tribonacci` and the `(1+√5)/2`-related constants). Run all solvers at 1M evals/seed.

**Goal.** Catalog open. The snub dodecahedron is also conjectured non-Rupert; finding any positive-clearance candidate would be a direct counterexample.

### E6 — Catalan dual sweep

**What.** Add the four Catalan duals of icosahedral Archimedeans (deltoidal hexecontahedron, disdyakis triacontahedron, pentagonal hexecontahedron, pentakis dodecahedron). Run patch_aware at 5M evals/seed.

**Goal.** Catalog. These shapes have rich face structure (60 to 120 faces) and the patch_aware solver should excel on them. Several are open in the literature.

### E7 — Custom-shape noperthedron candidate

**What.** Pick a small perturbation of the noperthedron seeds and run all solvers + (when shipped) the refutation track. The hypothesis: there's a neighborhood in seed-space around the noperthedron where the polyhedron remains non-Rupert.

**Goal.** Demonstrate the refutation track on a *new* shape — not just reproducing the paper's result.

### E8 — Determinism stress test

**What.** Run the full sweep on 16 different machines (or 16 different OSes via Docker). Compare JSONL outputs byte-for-byte.

**Goal.** Confirm the determinism story holds across platforms. Floating-point reductions (FMA ordering, etc.) are the most likely source of drift; if any rows differ, that's a P0 bug.

### E9 — Profile-guided optimization sweep

**What.** Build with `RUSTFLAGS="-C profile-generate=/tmp/pgo"`, run the full sweep, then rebuild with `-C profile-use=/tmp/pgo`. Re-run the bench.

**Goal.** Lower per-eval cost in the inner loop. Even a 2× speedup is significant for 100M+ eval experiments.

### E10 — Tom 7 imperts upstream port

**What.** Port the missing pieces of Tom 7's `imperts.cc` — specifically `cc-lib::Opt::Minimize`, the smart DFO black-box minimizer that handles concentration without our adaptive box-shrink hack.

**Goal.** Match (or beat) Tom 7's published clearance numbers for cube/octa/dodec/icos. Removes the v1 hack from the `imperts.rs` module header.

### E11 — Random shape catalog

**What.** Build a generator that produces random convex polyhedra (sample N points on `S²`, take convex hull). Run all solvers at 50k evals on M=1000 such shapes. Measure: what fraction certify a passage?

**Goal.** Empirical baseline for "what does a typical convex polyhedron look like?". Rough hypothesis: most convex polyhedra are Rupert (have wide passage basins); the noperthedron is exceptional.

---

## Triggers for starting each track

- **v0.3.0 branch-and-bound**: trigger when patch_aware v0.2.0 stalls on the snub cube at 10M+ evals (E1 result). The branch-and-bound work pays off only if there's a real reduction signal to chase.
- **`certify_exact`**: trigger when (a) someone proposes a shape with rational vertices that we want to certify rigorously, OR (b) the headline noperthedron regression starts to feel inadequate (e.g. someone publishes a near-miss attack).
- **LP scale oracle**: trigger when any solver's translation search dominates its eval cost (visible in profiler).
- **GPU**: trigger when any experiment in this list takes > 1 week wall time on commodity hardware.
- **SDP / Lasserre**: trigger when there's appetite for a formal global-optimum result on a small shape.
- **Refutation track**: trigger when (a) someone wants to publish a new non-Rupert proof, OR (b) the snub cube refuses to yield to direct-search solvers and we need an alternative attack.
- **Lean export**: trigger when there's an external collaborator who needs Lean-checkable certificates.

---

## How to contribute to the roadmap

1. **Code track**: pick an item, open a PR. Keep diffs surgical; the architecture (`docs/architecture.md`) limits where new dependencies can land.
2. **Experiment track**: pick an item, run it, commit results to `results/<your-name>/`. The CLI's `verify` and `lead build` will incorporate results into the leaderboard automatically. If your run takes > 1 hour, document the wall time and CPU/GPU specs in a sibling `results/<your-name>/README.md`.
3. **New roadmap items**: open a PR adding a section. Format: what / why / crate / effort / risk for code; what / goal / what-to-record for experiments.

The bench runs are *cheap* relative to writing solvers from scratch, but every experiment fills in a row of the leaderboard that the next contributor can build on.
