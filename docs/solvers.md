# Solvers

Each solver lives in `crates/rupert-solvers/src/<name>.rs`, implements `rupert_core::Solver`, and is registered with one line in `lib.rs::registered_solvers()`. To add a new one, see [`adding-a-solver.md`](adding-a-solver.md).

## v1 baseline lineup

### `random_quat` (v0.1.0)

**Algorithm.** Uniform random unit-quaternion sampling via Shoemake's method. Translation fixed at `(0, 0)`. Stops on first positive clearance, or budget exhaustion.

**Excels on.** Cube, dodecahedron, octahedron — solids with wide passage basins. ≥ 95% of seeds find a cube passage in ≤ 10 000 evals.

**Fails on.** Triakis tetrahedron (basin too narrow for random sampling), noperthedron (the v1 placeholder isn't actually a proven nopert; the snub cube is the more meaningful "should be exhausted" target).

**Cube to first solution.** ≈ 30–300 evals on average.

### `face_normal_pairs` (v0.1.0)

**Algorithm.** Deterministic. Enumerate every "principal direction" of the polyhedron — vertices, face centroids, edge midpoints. Align each direction to `+z`. Cross-product two enumerations: `(outer × inner)`. For each pair, also try `IN_PLANE_STEPS = 12` post-multiplied rotations around z (the cube needs an in-plane twist to pass through itself). Translation `(0, 0)`.

**Excels on.** Cube (≤ ~110k evals worst-case order), dodecahedron, octahedron — anywhere the canonical Rupert passage sits at a principal-direction-aligned configuration.

**Fails on.** Snub cube (chiral, no canonical principal direction yields a passage), triakis tetrahedron (off-axis solution required), noperthedron-placeholder (no faces in v1).

**Cube to first solution.** Deterministic. Order of ~hundreds of evals if the principal directions hit early.

### `nelder_mead` (v0.1.0)

**Algorithm.** Custom 8-dimensional Nelder-Mead simplex search over `(outer_tangent_xyz, inner_tangent_xyz, t_x, t_y)`. Tangent perturbations to two unit quaternions composed left-to-right via small-angle axis-angle. Random restarts, ~80 LOC pure Rust.

**Excels on.** Refining a candidate already inside a passage basin (improves clearance margin from positive to strongly positive). Less reliable as a from-scratch solver because of basin-finding cost.

**Fails on.** Standalone start on shapes whose passage basins are isolated; restarts pay off only with budget headroom.

**Cube to first solution.** ~50 000 evals standalone, ~500 evals seeded inside a basin.

### `random_then_refine` (v0.1.0)

**Algorithm.** Two phases. Phase 1: random rotations at translation `(0, 0)`, like `random_quat`. Phase 2: when clearance ≥ `-0.05` (close-to-basin), run a coordinate-descent local search over the 2D translation only. Returns on first strictly positive clearance.

**Excels on.** Workhorse for regular polyhedra. ≥ 90% cube hit rate within 5 000 evals; first-solve typically in 50–200 evals.

**Fails on.** Same-orientation degenerate near-misses (the threshold filter for "near a basin" can fire on translation-recoverable rotations that aren't actually Rupert).

**Cube to first solution.** ≈ 50–200 evals.

### `hopf_grid` (v0.1.0)

**Algorithm.** Deterministic. Generate `RESOLUTION_S2 = 32` Fibonacci-lattice points on `S²`, twist by `RESOLUTION_S1 = 8` evenly-spaced angles around the fiber. 256 quaternions per copy → 65 536 candidate pairs. Translation `(0, 0)`.

**Excels on.** Shapes with narrow passage basins missed by random sampling. Deterministic search over a uniform grid.

**Fails on.** Noperthedron and snub cube (basins are either nonexistent or smaller than the grid resolution). 65k evals is the budget ceiling for this solver to terminate without `Exhausted`.

**Cube to first solution.** Deterministic. Few hundred to ~10k evals depending on grid order.

### `patch_aware` (v0.1.0)

**Algorithm.** Tom 7's patch-decomposition idea (SIGBOVIK 2025), v0.1.0 implementation. For each polyhedron: enumerate the patches of `SO(3)` — connected open regions where the face-front/back assignment is constant. Reduce by the polyhedron's rotation symmetry group. Within each `(outer_patch, inner_patch)` pair, run a Nelder-Mead in the 10-DOF delta box anchored at the patch's representative quaternion, with a soft Hamming-distance penalty that biases the simplex back inside the patch.

**Per-shape patch table** is cached behind a `OnceLock<Mutex<HashMap<PolyId, Arc<PatchTable>>>>` keyed by `Polyhedron::id()`. Enumeration RNG is seeded from a hash of the shape name, so the patch table is deterministic per shape (independent of `Budget::seed`). Symmetry group lookup hand-tabled: tetrahedral / octahedral covered; icosahedral (dodec/icos) is a v0.2.0 stub returning identity-only (no symmetry reduction; full O(F²) brute force still works correctly, just ~5× slower).

**Excels on.** Shapes whose Rupert passage requires off-axis configurations that don't sit at any face-aligned principal direction — exactly where `face_normal_pairs` fails. The snub cube (open since arXiv:2112.13754) is the headline target; with its 38 faces and 24 chiral cubic symmetries, it has hundreds of patches to scan.

**Fails on.** v0.1.0 brute force isn't enough on its own — even with patches, the inner Nelder-Mead per cell needs more sample budget for small basins. v0.2.0 adds branch-and-bound across cells via interval-arithmetic upper bounds (uses [`rupert_core::hull2d_interval`]), which is how this solver becomes a real contender on the snub cube.

**Cube to first solution.** ~10 000–50 000 evals depending on the canonical-patch count after symmetry reduction.

### `imperts` (v0.1.0)

**Algorithm.** Port of Tom 7's `imperts.cc` from his SIGBOVIK 2025 SourceForge tree. Self-bootstrapping refiner: random-quaternion search until a positive-clearance seed is found, then an outer loop that perturbs the seed by uniform-random deltas in a `(Δq_outer, Δq_inner, Δt)` box and accepts strictly-improving steps. Adaptive box-shrink (×0.7 per flat outer iter) — explicitly NOT in upstream, where Tom's smart DFO (`cc-lib::Opt::Minimize`) handles concentration without shrinking.

**v1 ↔ upstream diffs.** The upstream uses (a) Tom's cc-lib `Opt::Minimize` derivative-free black-box minimizer at `iters=2000, depth=2, attempts=100` per outer iteration (here: simple uniform random sampling, no smart DFO); (b) static box bounds Q=1.0 / T=0.5 (here: shrunk to 0.3 / 0.2 with adaptive ×0.7 decay since uniform sampling doesn't concentrate); (c) SQLite-backed seed selection from a global database (here: random-quat bootstrap each call). All deviations documented in `crates/rupert-solvers/src/imperts.rs` module header. v2 work: port (or wrap) cc-lib's `Opt::Minimize`, restore Tom's static bounds.

**Excels on.** Clearance margin maximization on shapes that already have a discoverable basin: cube, octahedron, dodecahedron, icosahedron. On a fresh leaderboard run, `imperts` is the clearance leader on each of these even though it spends more evals than the basic search solvers.

**Fails on.** Triakis tetrahedron, snub cube, noperthedron — same as the others. The bootstrap phase needs a positive-clearance seed; without one, refinement never starts.

**Cube to first solution.** ~30 000–50 000 evals (most of which is refinement, not bootstrap). Don't compare against `random_quat`'s 30 evals; this solver is for the *clearance column*, not the *eval-count column*.

### `gosain_grimmer` (v0.1.0)

**Algorithm.** Port of Gosain & Grimmer 2025 ([arXiv:2509.08190](https://arxiv.org/abs/2509.08190)). 7-parameter parametrization `x = (u, v, θ_p, φ_p, α, θ_q, φ_q)` — translation, inner spherical view + in-plane twist, outer spherical view. Steepest ascent on the clearance objective with backtracking line search; finite-difference gradient (~8 evals per gradient step). Random restarts.

**v1 ↔ paper diffs.** The paper uses (a) symbolic gradients via sympy (here: finite difference), (b) the trust-region linearized-min-of-smooth direction over ε-active constraints (here: plain steepest ascent), and (c) the Nieuwland-scale objective μ (here: signed clearance, monotonically related). All three are documented as v2 enhancements in `crates/rupert-solvers/src/gosain_grimmer.rs` module docs.

**Excels on.** Smooth-basin shapes — cube, octahedron, dodecahedron, icosahedron — with comparable or slightly worse eval counts than `random_then_refine` because of the FD gradient overhead. The point of this solver in v1 is the *parametrization*, which is what enables the paper's high-precision clearance maximization once symbolic gradients land.

**Fails on.** Triakis tetrahedron (paper-quoted clearance ~4×10⁻⁶ requires either symbolic gradients or much higher trust-region depth than v1's backtracking line search reaches). Snub cube, noperthedron — same as the other solvers.

**Cube to first solution.** ~200–500 evals once a restart hits a favorable initialization.

## Goal hierarchy for new solvers

If you're contributing a new solver, here's a rough difficulty ladder — pick a target before you start:

1. **Beat existing baselines on the cube.** Lowest bar; mostly a sanity check that your solver works.
2. **Solve every regular polyhedron** (cube, tetra, octa, dodec, icos) within the same budget.
3. **Solve the triakis tetrahedron.** Famous for its tiny clearance margin (~4×10⁻⁶). Requires a refinement phase that can converge to high precision.
4. **Beat Gosain & Grimmer's clearance numbers** ([arXiv:2509.08190](https://arxiv.org/abs/2509.08190)). Their trust-region linearized-min-of-smooth subgradient hits 10⁻¹² accuracy. This is the SOTA target.
5. **Find a Rupert passage for a Johnson solid** not yet in the literature. Volume-game; need broad fleet performance.
6. **Find a snub cube passage.** Open since arXiv:2112.13754. Likely requires either a novel decomposition (patch-aware), a verified non-existence proof (refutation track, v2+), or the elusive missing solver insight.

The leaderboard ranks solvers by `(shape, solver) → best eval count`. Use `cargo run --release -p rupert-cli -- run --all --budget-evals 50000` to see how a new solver compares across the fleet.
