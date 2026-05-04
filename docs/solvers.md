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

## Goal hierarchy for new solvers

If you're contributing a new solver, here's a rough difficulty ladder — pick a target before you start:

1. **Beat existing baselines on the cube.** Lowest bar; mostly a sanity check that your solver works.
2. **Solve every regular polyhedron** (cube, tetra, octa, dodec, icos) within the same budget.
3. **Solve the triakis tetrahedron.** Famous for its tiny clearance margin (~4×10⁻⁶). Requires a refinement phase that can converge to high precision.
4. **Beat Gosain & Grimmer's clearance numbers** ([arXiv:2509.08190](https://arxiv.org/abs/2509.08190)). Their trust-region linearized-min-of-smooth subgradient hits 10⁻¹² accuracy. This is the SOTA target.
5. **Find a Rupert passage for a Johnson solid** not yet in the literature. Volume-game; need broad fleet performance.
6. **Find a snub cube passage.** Open since arXiv:2112.13754. Likely requires either a novel decomposition (patch-aware), a verified non-existence proof (refutation track, v2+), or the elusive missing solver insight.

The leaderboard ranks solvers by `(shape, solver) → best eval count`. Use `cargo run --release -p rupert-cli -- run --all --budget-evals 50000` to see how a new solver compares across the fleet.
