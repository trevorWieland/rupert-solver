# rupert-solver task runner. `just ci` is the single gate.

cargo := "cargo"

# Default recipe — show help.
default:
    @just --list

# One-shot first-time setup. Installs dev tools via cargo-binstall.
bootstrap:
    #!/usr/bin/env bash
    set -euo pipefail
    rustup show
    if ! command -v cargo-binstall >/dev/null 2>&1; then
        cargo install cargo-binstall --locked
    fi
    cargo binstall --no-confirm cargo-nextest cargo-deny cargo-llvm-cov cargo-machete taplo-cli lefthook
    lefthook install
    @echo "bootstrap: complete"

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

# Cargo check.
check:
    {{cargo}} check --workspace --all-targets

# Run tests via nextest.
test:
    {{cargo}} nextest run --workspace

# All workspace gates from xtask.
xtask-all:
    {{cargo}} run -p xtask -- check-suppression
    {{cargo}} run -p xtask -- check-rust-test-surface
    {{cargo}} run -p xtask -- check-deps
    {{cargo}} run -p xtask -- check-lines
    {{cargo}} run -p xtask -- check-cargo-fields

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
ci: fmt-check clippy xtask-all deny machete test

# Run every solver against every shape, then rebuild the leaderboard.
sweep:
    {{cargo}} run --release -p rupert-cli -- run --all --budget-evals 50000
    {{cargo}} run --release -p rupert-cli -- verify results
    {{cargo}} run --release -p rupert-cli -- lead build
