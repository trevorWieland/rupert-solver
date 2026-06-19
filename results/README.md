# Results

`results/baseline/` is tracked and curated. All other result files under
`results/` are ignored scratch output from local experiments.

Before moving a run into `results/baseline/`:

- run `rupert verify` on the JSONL file or directory
- rebuild `LEADERBOARD.md` from curated results only (`rupert lead build`
  defaults to this directory)
- document any run over 1 hour with CPU, thread count, wall time, commit SHA,
  exact command, and interpretation notes

Search exhaustion is not a non-Rupert proof. Use `best_near_miss`,
`best_boundary`, bound/prune counts, and solver telemetry to decide the next
experiment.
