//! v1 fallback verifier path. Currently a stub — the F64Epsilon logic
//! lives in the parent `lib.rs`. v2 will use this module for the
//! interval-snap and exact-rational paths.
//!
//! Documenting the structure here so the v2 PR has a clear home.

use rupert_core::{Polyhedron, Solution};

/// Future entry point for the interval-arithmetic verifier (v2).
/// Currently always returns `None`; the orchestrator falls back to the
/// `F64Epsilon` path.
#[must_use]
pub fn try_interval(_solution: &Solution, _poly: &Polyhedron) -> Option<()> {
    None
}
