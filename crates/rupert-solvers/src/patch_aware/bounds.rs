//! Deterministic upper bounds for patch-aware cells.
//!
//! The bound is intentionally conservative but no longer shape-diameter
//! loose. It combines two safe estimates:
//!
//! - clearance at the cell anchor plus a quaternion/translation Lipschitz
//!   radius;
//! - a support-width containment bound over several fixed projection
//!   directions, recursively subdividing quaternion delta dimensions.

use rupert_core::{Candidate, Quat, Vec3};

use super::table::{CanonicalPatch, PatchTable};
use crate::dfo::apply_quat_delta;

const DIRECTIONS: usize = 16;
const SUBDIVISION_LEAVES: usize = 16;
const QUAT_DIMENSIONS: usize = 8;
const TRANSLATION_DIMENSIONS: usize = 2;
const TOTAL_DIMENSIONS: usize = QUAT_DIMENSIONS + TRANSLATION_DIMENSIONS;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum BoundOutcome {
    Prunable { upper_bound: f64 },
    Ambiguous { upper_bound: f64 },
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CellBounds {
    center: [f64; TOTAL_DIMENSIONS],
    half_width: [f64; TOTAL_DIMENSIONS],
}

impl CellBounds {
    pub(super) fn root(q_box: f64, t_box: f64) -> Self {
        let center = [0.0; TOTAL_DIMENSIONS];
        let mut half_width = [0.0; TOTAL_DIMENSIONS];
        half_width[..QUAT_DIMENSIONS].fill(q_box);
        half_width[QUAT_DIMENSIONS..].fill(t_box);
        Self { center, half_width }
    }

    fn split(self) -> (Self, Self) {
        let dim = widest_dimension(&self.half_width);
        let mut lo = self;
        let mut hi = self;
        let offset = self.half_width[dim] * 0.5;
        lo.center[dim] -= offset;
        hi.center[dim] += offset;
        lo.half_width[dim] = offset;
        hi.half_width[dim] = offset;
        (lo, hi)
    }
}

pub(super) fn bound_cell(
    table: &PatchTable,
    outer: &CanonicalPatch,
    inner: &CanonicalPatch,
    recon_clearance: f64,
    current_best: f64,
    q_box: f64,
    t_box: f64,
) -> BoundOutcome {
    if !current_best.is_finite() {
        return BoundOutcome::Ambiguous {
            upper_bound: f64::INFINITY,
        };
    }
    let root = CellBounds::root(q_box, t_box);
    let anchor = Candidate {
        outer: outer.q_rep,
        inner: inner.q_rep,
        translation: [0.0, 0.0],
    };
    let upper_bound = subdivided_upper_bound(table, &anchor, recon_clearance, root);
    if upper_bound <= current_best {
        BoundOutcome::Prunable { upper_bound }
    } else {
        BoundOutcome::Ambiguous { upper_bound }
    }
}

fn subdivided_upper_bound(
    table: &PatchTable,
    anchor: &Candidate,
    recon_clearance: f64,
    root: CellBounds,
) -> f64 {
    let mut leaves = vec![root];
    while leaves.len() < SUBDIVISION_LEAVES {
        let Some(cell) = leaves.pop() else {
            break;
        };
        let (lo, hi) = cell.split();
        leaves.push(lo);
        leaves.push(hi);
    }
    leaves
        .iter()
        .map(|cell| leaf_upper_bound(table, anchor, recon_clearance, *cell, leaves.len() == 1))
        .fold(f64::NEG_INFINITY, f64::max)
}

fn leaf_upper_bound(
    table: &PatchTable,
    anchor: &Candidate,
    recon_clearance: f64,
    cell: CellBounds,
    is_root: bool,
) -> f64 {
    let center = apply_quat_delta(anchor, &cell.center);
    let outer_d =
        quat_normalization_radius(anchor.outer, &cell.center[0..4], &cell.half_width[0..4]);
    let inner_d =
        quat_normalization_radius(anchor.inner, &cell.center[4..8], &cell.half_width[4..8]);
    let outer_move = 2.0 * table.max_vertex_norm * outer_d;
    let inner_move = 2.0 * table.max_vertex_norm * inner_d;
    let trans_move = translation_radius(&cell.half_width[8..10]);
    let clearance_bound = if is_root {
        recon_clearance + outer_move + inner_move + trans_move
    } else {
        f64::INFINITY
    };
    clearance_bound.min(width_upper_bound(
        &table.vertices,
        center.outer,
        center.inner,
        outer_move,
        inner_move,
    ))
}

fn quat_normalization_radius(seed: Quat, center_delta: &[f64], half_width: &[f64]) -> f64 {
    let raw_center = [
        seed.w + center_delta[0],
        seed.x + center_delta[1],
        seed.y + center_delta[2],
        seed.z + center_delta[3],
    ];
    let center_norm = norm4(&raw_center);
    let radius = norm4(half_width);
    if center_norm <= radius {
        return 2.0;
    }
    (2.0 * radius / (center_norm - radius)).min(2.0)
}

fn width_upper_bound(
    vertices: &[Vec3],
    outer: Quat,
    inner: Quat,
    outer_move: f64,
    inner_move: f64,
) -> f64 {
    let mut best = f64::INFINITY;
    for i in 0..DIRECTIONS {
        let theta = std::f64::consts::TAU * (i as f64) / (DIRECTIONS as f64);
        let axis = Vec3::new(theta.cos(), theta.sin(), 0.0);
        let outer_width = projected_width(vertices, outer, axis);
        let inner_width = projected_width(vertices, inner, axis);
        let upper = (outer_width - inner_width) * 0.5 + outer_move + inner_move;
        best = best.min(upper);
    }
    best
}

fn projected_width(vertices: &[Vec3], q: Quat, axis: Vec3) -> f64 {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &v in vertices {
        let p = q.rotate(v);
        let d = p.x * axis.x + p.y * axis.y;
        lo = lo.min(d);
        hi = hi.max(d);
    }
    hi - lo
}

fn widest_dimension(half_width: &[f64; TOTAL_DIMENSIONS]) -> usize {
    let mut best_dim = 0_usize;
    let mut best = f64::NEG_INFINITY;
    for (dim, width) in half_width.iter().take(QUAT_DIMENSIONS).enumerate() {
        if *width > best {
            best = *width;
            best_dim = dim;
        }
    }
    best_dim
}

fn translation_radius(half_width: &[f64]) -> f64 {
    half_width.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn norm4(values: &[f64]) -> f64 {
    values.iter().map(|x| x * x).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_bounds_cover_configured_boxes() {
        let root = CellBounds::root(0.15, 0.5);
        assert_eq!(root.center, [0.0; TOTAL_DIMENSIONS]);
        assert_eq!(root.half_width[0], 0.15);
        assert_eq!(root.half_width[7], 0.15);
        assert_eq!(root.half_width[8], 0.5);
        assert_eq!(root.half_width[9], 0.5);
    }

    #[test]
    fn split_is_deterministic_and_halves_widest_quat_dimension() {
        let root = CellBounds::root(0.15, 0.5);
        let (lo, hi) = root.split();
        assert_eq!(lo.half_width[0], 0.075);
        assert_eq!(hi.half_width[0], 0.075);
        assert!(lo.center[0] < 0.0);
        assert!(hi.center[0] > 0.0);
        assert_eq!(lo.half_width[8], 0.5);
    }

    #[test]
    fn quaternion_radius_is_finite_for_patch_box() {
        let radius = quat_normalization_radius(Quat::IDENTITY, &[0.0; 4], &[0.15; 4]);
        assert!(radius.is_finite());
        assert!(radius > 0.0);
    }

    #[test]
    fn width_bound_is_finite_for_cube_identity() {
        let cube = rupert_shapes::cube();
        let bound = width_upper_bound(&cube.vertices, Quat::IDENTITY, Quat::IDENTITY, 0.0, 0.0);
        assert!(bound.abs() < 1.0e-12);
    }
}
