# Experiment Playbook

This repo is optimized for certified positive discovery, not for proving
non-Rupert-ness by search exhaustion. Treat an exhausted run as telemetry:
near-misses, boundary contacts, pruning behavior, and candidate basins.

## Result Hygiene

- `results/baseline/` is the only tracked result directory.
- Ad hoc runs belong directly under `results/` or another ignored scratch
  directory.
- Any tracked run over 1 hour must include CPU model, thread count, wall time,
  commit SHA, exact command, and interpretation notes.
- Rebuild `LEADERBOARD.md` only from curated results. `rupert lead build`
  defaults to `results/baseline/`; pass `--results-dir` only when you
  intentionally want a scratch leaderboard.

## Calibration Targets

| shape | status | difficulty | recommended recipe | acceptance criteria |
|-------|--------|------------|--------------------|---------------------|
| cube | known Rupert | easy positive | `calibrate-easy` | at least one certified row |
| octahedron | known Rupert | easy positive | `calibrate-easy` | at least one certified row |
| dodecahedron | known Rupert | easy positive | `calibrate-easy` | at least one certified row |
| icosahedron | known Rupert | easy positive | `calibrate-easy` | at least one certified row |
| triakis_tetrahedron | known Rupert | hard positive | `calibrate-hard` | at least one certified row before hunts |
| pentagonal_icositetrahedron | known Rupert, exact snub-cube dual | hard positive | `calibrate-hard` | at least one interval-certified row |
| noperthedron | proven non-Rupert control | negative control | `nopert-control` | no verified `Solved` rows |
| snub_cube | open target | search-only | `snub-pilot`, then `snub-hunt` | near-miss, adaptive, and bound/prune telemetry, not proof |

## CLI Analysis

```bash
cargo run --release -p rupert-cli -- analyze summary --results-dir results
cargo run --release -p rupert-cli -- analyze patch-aware \
  --results-dir results --shape snub_cube --top 25
```

Add `--json` to either command for scripts.

## Recipes

The `justfile` exposes these hunt-loop targets:

- `just sweep` / `just baseline-sweep`
- `just calibrate-easy`
- `just calibrate-hard`
- `just snub-pilot`
- `just snub-hunt`
- `just nopert-control`

Run `rupert verify` after each batch. For open targets, inspect
`best_near_miss` first; `best_boundary` is useful for detecting touching or
degenerate solver behavior but must not dominate open-problem rankings. For
patch-aware runs, inspect `bound_cells_ambiguous`,
`cells_skipped_by_interval_bound`, and the upper-bound gap histogram before
raising budgets.
