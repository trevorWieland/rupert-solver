# Roadmap

Two parallel tracks: **code** (engineering work to broaden what the harness can certify and accelerate) and **experiments** (large-budget compute runs that don't require new code, just patience and CPU). The former unblocks the latter.

The headline goals — find a snub cube passage, formally certify the noperthedron rejection, prove non-Rupert-ness for a *new* polyhedron — sit at the top of both tracks.

---

## Code track

Ordered by leverage (impact ÷ effort), with explicit dependencies.

### v0.3.0 — Branch-and-bound across patch_aware cells

**Status**: shipped as the first hunt-ready pass. v0.3.0 ships deterministic patch tables, full-cell telemetry, adaptive refinement of top near-miss cells, and deterministic upper-bound pruning.

**What.** `patch_aware` v0.3.0 reconnaissance-first scans cells in score order, records cell telemetry, and reallocates remaining budget to top positive/near-miss cells. It skips cells using the slack heuristic plus a deterministic branch bound:

- For each canonical (outer, inner) cell, bound the projection movement induced by quaternion delta boxes.
- Recursively subdivide the quaternion box into deterministic subcells.
- Compute support-width containment upper bounds across fixed projection directions.
- If the upper bound is ≤ `best_so_far`, skip the cell without spending a Nelder-Mead step.
- Record bound attempts, ambiguous bounds, pruned cells, and upper-bound gap distribution in patch-aware telemetry.

**Why.** On the snub cube (1296 cells × ~1500 evals/cell ≈ 2M evals to fully scan), even cutting 50% of cells via rigorous bounds gets us to 1M evals on cells that *might* yield a passage. Combined with adaptive per-cell budgets (give pruned-but-not-skipped cells more evals), this is the realistic path to either finding a snub cube passage *or* proving none exists in some patch product.

**Crate(s)**: `rupert-solvers`.

**Follow-up.** If pilot data shows most upper bounds remain ambiguous, the next refinement is to replace the Lipschitz support-width bound with a true interval projection over each quat-delta subcell and reuse `convex_hull_interval_certified` for tighter combinatorial hull reasoning.

**Risk**: medium. The bound is safe and deterministic, but may still be too wide on 0.15-wide deltas. The telemetry is explicitly designed to tell us whether to tighten the bound, narrow the delta box, or spend more budget on adaptive refinement.

### v0.3.0 — `certify_exact` via malachite

**Status**: shipped by default for rational-coordinate shapes.

**What.** For shapes whose vertices are pure rationals (cube, tetrahedron, octahedron, triakis tetrahedron), recompute the projection-hull-containment pipeline in `malachite::Rational` after snapping the f64 candidate transform to dyadic rationals. No floating-point containment predicate remains in this tier. Shapes with golden-ratio, tribonacci, or trigonometric coordinates fall back to `IntervalSnap`.

The candidate quaternion is f64, so this path uses **snap-to-rational**: round each rotation-matrix coefficient and translation component to a denominator 2³² rational, then certify strict containment against the snapped transform. The candidate is rejected unless the snapped containment is also strictly positive.

**Why.** For cube/tet/octa/triakis the leaderboard's `cert` column can show `exact` for the strongest tier — a meaningful integrity signal. The noperthedron itself has trigonometric rotated vertices, so it remains an `IntervalSnap` verifier target.

**Crate(s)**: `rupert-verify` and `rupert-core`.

**Estimated effort**: 400 LOC.

**Risk**: low. The remaining hardening item is adding an interval-arithmetic guard that the snap doesn't change combinatorial structure of the hull.

### v0.3.0 — Update noperthedron headline regression

**Status**: shipped semantically through the runner/verifier path: solver-reported candidates only become `Solved` if the strongest available verifier accepts them.

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

**What.** Run `patch_aware` v0.3.0 with `--budget-evals 10_000_000` per seed. Stratify across 16 seeds. Total: ~160M evaluations. Estimated wall time: 12–24 hours on a 32-core machine.

**Goal.** Find a positive-clearance candidate for the snub cube. Open since arXiv:2112.13754.

**Risk.** If patch_aware v0.3.0 still resists at 10M evals/seed, that's a strong empirical lower bound on the difficulty — and motivation to tighten the interval branch-and-bound hook.

**What to record.** Best clearance per cell across all seeds; histogram of best-clearance-per-cell distribution; identify any cell whose best clearance is consistently within 0.01 of zero (those are the "near-miss" candidates worth refining).

### E2 — Snub cube high-budget gosain-grimmer

**What.** Run `gosain_grimmer` with `--budget-evals 100_000_000` and many random restarts. Single-seed at this scale.

**Goal.** Independent attack on the snub cube using a fundamentally different parametrization (Gosain-Grimmer's 7-parameter form vs. patch_aware's 10-DOF delta). If both fail, that's evidence; if one succeeds, that's a result.

### E3 — Triakis tetrahedron precision boundary

**What.** The triakis tetrahedron has the smallest known positive clearance among Rupert solids (~4×10⁻⁶). Run `imperts` (or `gosain_grimmer` with autodiff once v0.4.0 lands) at 10M evals/seed × 32 seeds, recording the *highest clearance* achieved.

**Goal.** Push the published margin upward. The previous SOTA is Fredriksson 2022.

**Why this matters.** A higher certified clearance for triakis is a leaderboard entry that can stay valid forever — clearance only increases when refined.

### E4 — Noperthedron formal cover

**What.** Run a sweep of N=1M random rotation pairs against the noperthedron and verify every single one is rejected by IntervalSnap.

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

- **Tight interval branch-and-bound**: trigger when patch_aware v0.3.0 stalls on the snub cube at 10M+ evals (E1 result). The branch-and-bound work pays off only if there's a real reduction signal to chase.
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
