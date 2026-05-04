//! xtask — workspace gates.

mod check_cargo_fields;
mod check_deps;
mod check_lines;
mod check_suppression;
mod check_test_surface;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "xtask", version, about = "rupert-solver workspace gates")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    /// Reject inline #[allow] / #[expect] / #![allow] in workspace source.
    CheckSuppression,
    /// Enforce per-crate test surface (bdd vs rust) per `xtask/test-surface.toml`.
    CheckRustTestSurface,
    /// Reject dependency edges that violate workspace layering per `xtask/layers.toml`.
    CheckDeps,
    /// Reject any .rs file over 500 lines.
    CheckLines,
    /// Reject Cargo.toml files missing inherited workspace fields.
    CheckCargoFields,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::CheckSuppression => check_suppression::run()?,
        Command::CheckRustTestSurface => check_test_surface::run()?,
        Command::CheckDeps => check_deps::run()?,
        Command::CheckLines => check_lines::run()?,
        Command::CheckCargoFields => check_cargo_fields::run()?,
    }
    Ok(())
}

/// Where the workspace root sits. The xtask binary always runs from the
/// workspace root via `cargo run -p xtask`, so `.` is the right anchor.
pub(crate) fn workspace_root() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

/// Source-code roots scanned by every gate.
pub(crate) const SOURCE_ROOTS: [&str; 3] = ["bin", "crates", "xtask"];
