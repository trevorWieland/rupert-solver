//! rupert CLI entrypoint. Thin wiring; all logic lives in library crates.

mod cmd;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "rupert",
    version,
    about = "rupert-solver — Rupert-property benchmark harness"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    /// Analyze stored experiment results.
    Analyze(cmd::analyze::AnalyzeArgs),
    /// List shapes or solvers.
    List(cmd::list::ListArgs),
    /// Run one or more solver/shape/seed triples.
    Run(cmd::run::RunArgs),
    /// Re-verify stored results.
    Verify(cmd::verify::VerifyArgs),
    /// Aggregate results into LEADERBOARD.md.
    Lead(cmd::lead::LeadArgs),
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    match cli.command {
        Command::Analyze(args) => cmd::analyze::run(&args),
        Command::List(args) => cmd::list::run(&args),
        Command::Run(args) => cmd::run::run(&args),
        Command::Verify(args) => cmd::verify::run(&args),
        Command::Lead(args) => cmd::lead::run(&args),
    }
}
