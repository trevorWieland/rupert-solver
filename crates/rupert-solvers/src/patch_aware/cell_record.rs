//! Patch-aware per-cell telemetry aggregation and ranking.

use rupert_core::{CLEARANCE_EPS, PatchAwareCellSummary, PatchAwareSkipReason};

const TOP_CELL_LIMIT: usize = 64;

#[derive(Debug)]
pub(super) struct CellRecorder {
    pub best_positive_cell: Option<PatchAwareCellSummary>,
    pub best_near_miss_cell: Option<PatchAwareCellSummary>,
    pub best_boundary_cell: Option<PatchAwareCellSummary>,
    pub cell_summaries: Vec<PatchAwareCellSummary>,
    pub top_cells: Vec<PatchAwareCellSummary>,
}

impl CellRecorder {
    pub(super) fn new(cell_pairs: usize) -> Self {
        Self {
            best_positive_cell: None,
            best_near_miss_cell: None,
            best_boundary_cell: None,
            cell_summaries: Vec::with_capacity(cell_pairs),
            top_cells: Vec::new(),
        }
    }

    pub(super) fn record(&mut self, cell: PatchAwareCellSummary) {
        if let Some(existing) = self.cell_summaries.iter_mut().find(|existing| {
            existing.outer_cell == cell.outer_cell && existing.inner_cell == cell.inner_cell
        }) {
            existing.end_eval = cell.end_eval;
            existing.evals_spent += cell.evals_spent;
            if cell.best_clearance > existing.best_clearance {
                existing.best_clearance = cell.best_clearance;
            }
            if cell.skip_reason != PatchAwareSkipReason::None {
                existing.skip_reason = cell.skip_reason;
            }
            return;
        }
        self.cell_summaries.push(cell);
    }

    pub(super) fn finalize(&mut self) {
        self.rebuild_indexes();
    }

    pub(super) fn ranked_refinement_cells(&self, limit: usize) -> Vec<PatchAwareCellSummary> {
        let mut cells: Vec<PatchAwareCellSummary> = self
            .cell_summaries
            .iter()
            .filter(|cell| cell.skip_reason == PatchAwareSkipReason::None)
            .cloned()
            .collect();
        cells.sort_by(|a, b| {
            class_rank(a.best_clearance)
                .cmp(&class_rank(b.best_clearance))
                .then_with(|| {
                    b.best_clearance
                        .partial_cmp(&a.best_clearance)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.outer_cell.cmp(&b.outer_cell))
                .then_with(|| a.inner_cell.cmp(&b.inner_cell))
        });
        cells.truncate(limit);
        cells
    }

    fn rebuild_indexes(&mut self) {
        self.best_positive_cell = None;
        self.best_near_miss_cell = None;
        self.best_boundary_cell = None;
        for cell in &self.cell_summaries {
            classify_cell(
                &mut self.best_positive_cell,
                &mut self.best_near_miss_cell,
                &mut self.best_boundary_cell,
                cell,
            );
        }
        self.top_cells = self.cell_summaries.clone();
        self.top_cells.sort_by(|a, b| {
            b.best_clearance
                .partial_cmp(&a.best_clearance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.outer_cell.cmp(&b.outer_cell))
                .then_with(|| a.inner_cell.cmp(&b.inner_cell))
        });
        self.top_cells.truncate(TOP_CELL_LIMIT);
    }
}

fn class_rank(clearance: f64) -> u8 {
    if clearance > CLEARANCE_EPS {
        0
    } else if clearance < -CLEARANCE_EPS {
        1
    } else {
        2
    }
}

fn classify_cell(
    best_positive_cell: &mut Option<PatchAwareCellSummary>,
    best_near_miss_cell: &mut Option<PatchAwareCellSummary>,
    best_boundary_cell: &mut Option<PatchAwareCellSummary>,
    cell: &PatchAwareCellSummary,
) {
    if cell.best_clearance > CLEARANCE_EPS {
        replace_if_better(best_positive_cell, cell, |candidate, best| {
            candidate.best_clearance > best.best_clearance
        });
    } else if cell.best_clearance < -CLEARANCE_EPS {
        replace_if_better(best_near_miss_cell, cell, |candidate, best| {
            candidate.best_clearance > best.best_clearance
        });
    } else {
        replace_if_better(best_boundary_cell, cell, |candidate, best| {
            candidate.best_clearance.abs() < best.best_clearance.abs()
        });
    }
}

fn replace_if_better(
    slot: &mut Option<PatchAwareCellSummary>,
    candidate: &PatchAwareCellSummary,
    better: impl Fn(&PatchAwareCellSummary, &PatchAwareCellSummary) -> bool,
) {
    if slot.as_ref().is_none_or(|best| {
        better(candidate, best)
            || (candidate.best_clearance == best.best_clearance
                && (candidate.outer_cell, candidate.inner_cell)
                    < (best.outer_cell, best.inner_cell))
    }) {
        *slot = Some(candidate.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(outer_cell: usize, inner_cell: usize, clearance: f64) -> PatchAwareCellSummary {
        PatchAwareCellSummary {
            outer_cell,
            inner_cell,
            start_eval: 10,
            end_eval: 20,
            evals_spent: 10,
            recon_clearance: clearance,
            best_clearance: clearance,
            skip_reason: PatchAwareSkipReason::None,
        }
    }

    #[test]
    fn classified_best_cells_use_deterministic_tie_breaks() {
        let mut recorder = CellRecorder::new(3);
        recorder.record(cell(2, 2, -0.01));
        recorder.record(cell(1, 1, -0.01));
        recorder.record(cell(3, 3, 0.0));
        recorder.finalize();

        let best_near = recorder.best_near_miss_cell.expect("near miss");
        assert_eq!((best_near.outer_cell, best_near.inner_cell), (1, 1));
        assert!(recorder.best_boundary_cell.is_some());
        assert!(recorder.best_positive_cell.is_none());
    }
}
