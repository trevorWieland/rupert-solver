# rupert-solver task runner. `just check` is the fast loop; `just ci` is the full gate.

cargo := "cargo"

# Default recipe — show help.
default:
    @just --list

# One-shot first-time setup. Installs dev tools via cargo-binstall.
# Lefthook is NOT a cargo crate — install it separately (apt, brew, npm,
# or download the binary from github.com/evilmartians/lefthook/releases).
bootstrap:
    #!/usr/bin/env bash
    set -euo pipefail
    rustup show
    if ! command -v cargo-binstall >/dev/null 2>&1; then
        cargo install cargo-binstall --locked
    fi
    cargo binstall --no-confirm cargo-nextest cargo-deny cargo-llvm-cov cargo-machete taplo-cli
    if command -v lefthook >/dev/null 2>&1; then
        lefthook install
    else
        echo "lefthook not on PATH — skipping git hook setup."
        echo "Install via: apt install lefthook | brew install lefthook |"
        echo "             curl -sSL https://github.com/evilmartians/lefthook/releases/latest/download/lefthook_Linux_x86_64.gz | gunzip > /usr/local/bin/lefthook && chmod +x /usr/local/bin/lefthook"
    fi
    echo "bootstrap: complete"

# All-formats formatter.
fmt:
    {{cargo}} fmt --all
    taplo fmt Cargo.toml bin/*/Cargo.toml crates/*/Cargo.toml xtask/Cargo.toml .cargo/*.toml rust-toolchain.toml clippy.toml taplo.toml deny.toml rustfmt.toml

# Format check (used by CI).
fmt-check:
    {{cargo}} fmt --all -- --check
    taplo fmt --check Cargo.toml bin/*/Cargo.toml crates/*/Cargo.toml xtask/Cargo.toml .cargo/*.toml rust-toolchain.toml clippy.toml taplo.toml deny.toml rustfmt.toml

# Lint with clippy (deny warnings).
clippy:
    {{cargo}} clippy --workspace --all-targets -- -D warnings

# Cargo check only.
cargo-check:
    {{cargo}} check --workspace --all-targets

# Fast regular gate: formatting, compilation, clippy, and workspace policy checks.
check: fmt-check cargo-check clippy xtask-all

# Run tests: nextest for libtest binaries, plain cargo test for the
# cucumber BDD harnesses (which use harness=false and don't expose
# nextest's --list interface).
test:
    {{cargo}} nextest run --workspace -E 'not binary(bdd)'
    {{cargo}} test --workspace --test bdd

# All workspace gates from xtask.
xtask-all:
    {{cargo}} run -p xtask -- check-suppression
    {{cargo}} run -p xtask -- check-rust-test-surface
    {{cargo}} run -p xtask -- check-deps
    {{cargo}} run -p xtask -- check-lines
    {{cargo}} run -p xtask -- check-cargo-fields
    {{cargo}} run -p xtask -- check-solver-eval

# License + advisory + sources.
deny:
    {{cargo}} deny check

# Unused dependency detection.
machete:
    {{cargo}} machete

# Coverage report.
coverage:
    {{cargo}} llvm-cov nextest --workspace --lcov --output-path lcov.info

# Build docs with warnings as errors.
doc:
    RUSTDOCFLAGS="-D warnings" {{cargo}} doc --workspace --no-deps

# Single CI gate.
ci: check deny machete test doc

# Refresh the curated baseline: run every solver against every shape, verify,
# then rebuild the leaderboard from results/baseline.
baseline-sweep:
    {{cargo}} run --release -p rupert-cli -- run --all --budget-evals 50000 --out-dir results/baseline
    {{cargo}} run --release -p rupert-cli -- verify results/baseline
    {{cargo}} run --release -p rupert-cli -- lead build

# Backward-compatible alias for the curated baseline-producing sweep.
sweep: baseline-sweep

# Easy known-Rupert calibration: all solvers × easy positives × seeds 0..7.
calibrate-easy:
    #!/usr/bin/env bash
    set -euo pipefail
    shapes=(cube octahedron dodecahedron icosahedron)
    solvers=(random_quat face_normal_pairs nelder_mead random_then_refine hopf_grid imperts gosain_grimmer patch_aware)
    for shape in "${shapes[@]}"; do
      for solver in "${solvers[@]}"; do
        {{cargo}} run --release -p rupert-cli -- run --shape "$shape" --solver "$solver" --seed 0 --seed 1 --seed 2 --seed 3 --seed 4 --seed 5 --seed 6 --seed 7 --budget-evals 50000
      done
    done

# Hard known-Rupert calibration: selected solvers × hard positives × seeds 0..15.
calibrate-hard:
    #!/usr/bin/env bash
    set -euo pipefail
    shapes=(triakis_tetrahedron pentagonal_icositetrahedron)
    solvers=(nelder_mead random_then_refine imperts gosain_grimmer patch_aware)
    for shape in "${shapes[@]}"; do
      for solver in "${solvers[@]}"; do
        {{cargo}} run --release -p rupert-cli -- run --shape "$shape" --solver "$solver" --seed 0 --seed 1 --seed 2 --seed 3 --seed 4 --seed 5 --seed 6 --seed 7 --seed 8 --seed 9 --seed 10 --seed 11 --seed 12 --seed 13 --seed 14 --seed 15 --budget-evals 10000000
      done
    done

# Snub cube telemetry pilot: patch-aware × seeds 0..3 × 2M evals.
snub-pilot:
    {{cargo}} run --release -p rupert-cli -- run --shape snub_cube --solver patch_aware --seed 0 --seed 1 --seed 2 --seed 3 --budget-evals 2000000

# Snub cube hunt: patch-aware × seeds 0..15 × 10M evals.
snub-hunt:
    {{cargo}} run --release -p rupert-cli -- run --shape snub_cube --solver patch_aware --seed 0 --seed 1 --seed 2 --seed 3 --seed 4 --seed 5 --seed 6 --seed 7 --seed 8 --seed 9 --seed 10 --seed 11 --seed 12 --seed 13 --seed 14 --seed 15 --budget-evals 10000000

# Proven non-Rupert control: all solvers × noperthedron × seeds 0..7.
nopert-control:
    #!/usr/bin/env bash
    set -euo pipefail
    solvers=(random_quat face_normal_pairs nelder_mead random_then_refine hopf_grid imperts gosain_grimmer patch_aware)
    for solver in "${solvers[@]}"; do
      {{cargo}} run --release -p rupert-cli -- run --shape noperthedron --solver "$solver" --seed 0 --seed 1 --seed 2 --seed 3 --seed 4 --seed 5 --seed 6 --seed 7 --budget-evals 50000
    done
