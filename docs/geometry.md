# Geometry

## What "Rupert property" means precisely

A convex polyhedron `P ⊂ ℝ³` has the **Rupert property** iff there exist rotations `R_outer, R_inner ∈ SO(3)` and a 2D translation `t ∈ ℝ²` such that:

1. `outer_shadow := π_z(R_outer · P)` (the orthographic projection of the rotated outer copy onto the xy plane), and
2. `inner_shadow := π_z(R_inner · P) + t` (the rotated, translated inner copy's shadow),

satisfy

> every vertex of `inner_shadow` lies *strictly* inside the convex hull of `outer_shadow`.

"Strictly" matters. If we allowed boundary touching, every polyhedron would trivially be Rupert (set `R_outer = R_inner`). The strict inequality is what makes the problem interesting and is why the v1 verifier rejects clearance ≤ `F64_EPS = 1e-9`.

## Convention

- Projection axis: **z**. Drop the z component after rotation.
- Inner translation lives in the **xy plane** of the *projection*, not in 3D — the inner copy slides through along `+z`.
- Both copies are the **same size**. The classical Prince Rupert problem is "same-size cubes".

The `Candidate` type carries `outer: Quat`, `inner: Quat`, `translation: [f64; 2]`. v1 keeps translation as a free search variable; v2 may add a per-rotation-pair LP that recovers the optimal translation as the 2D Chebyshev center of `outer_hull ⊖ inner_hull` (Zeng 2026).

## Clearance metric

`rupert_core::clearance(inner_pts, outer_pts)` computes:

> `min over each inner point p` of (signed perpendicular distance from p to each edge of the outer hull).

Positive ⇒ p is strictly inside; negative ⇒ p is outside; zero ⇒ on boundary. The minimum across all inner points gives the **clearance margin** — the worst-case distance to the boundary.

A `Solution` is "found" if the f64 clearance is `> 0`. A solution is "certified" if `rupert_verify::certify` recomputes the clearance and finds it `> F64_EPS`.

## Convex hull and silhouettes

For a convex polyhedron, the orthographic shadow's boundary is the convex hull of the projected vertices. So `clearance` only needs the outer's projected vertices (no face data); the inner just contributes its projected vertex set.

This means **face data is optional** in the Polyhedron struct. Builtin shapes that ship without faces (snub cube, noperthedron in v1) still work for clearance evaluation. They're only restricted from solvers that genuinely need face data (`face_normal_pairs` enumerates face centroids from the face list — when faces are empty, only vertex and edge-midpoint enumerations contribute).

## Why the cube can pass through itself

Looking down a body diagonal of the cube, the silhouette is a regular hexagon with apothem `√2`. A unit square has corners at distance `√2` from origin — *exactly* on the hexagon's apothem. So the cube doesn't fit at zero in-plane rotation: corners touch the hex edges. Rotating the inner cube by a small angle around z brings the corners off the apothem and inside the edges. That's the classical Prince Rupert solution: same-size cubes pass through each other with an inner in-plane rotation, and the maximum scale factor (Nieuwland constant) is `3√2/4 ≈ 1.0607` for the cube.

This is why `face_normal_pairs` enumerates not just vertex/face/edge directions but also `IN_PLANE_STEPS = 12` post-multiplied rotations around z per direction.

## Symmetry and search

For the centrally-symmetric builtins we ship in v1 (cube, tetra, octa, dodec, icos), the optimal translation is near origin. Random sampling of translation hurts more than it helps, so `random_quat` uses translation `(0, 0)` and lets `random_then_refine`'s phase-2 coordinate-descent recover translation when needed.

## References

- Steininger & Yurkevich, *An algorithmic approach to Rupert's problem*, arXiv:2112.13754 (2021).
- Steininger & Yurkevich, *A convex polyhedron without Rupert's property*, arXiv:2508.18475 (2025) — the noperthedron paper.
- Gosain & Grimmer, *Some new insights from highly optimized polyhedral passages*, arXiv:2509.08190 (2025) — current SOTA on clearance maximization, including the conjectured Nieuwland constant for the regular tetrahedron `√6/(1+√2) ≈ 1.0146`.
- Zeng, *A stellated tetrahedron that is probably not Rupert*, arXiv:2604.26531 (2026) — LP formulation of the scale oracle.
