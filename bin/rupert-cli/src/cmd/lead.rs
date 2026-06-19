//! `rupert lead build` — aggregate JSONL results into LEADERBOARD.md.

use std::io::Write as _;
use std::path::PathBuf;

use anyhow::Result;

#[derive(clap::Args, Debug)]
pub(crate) struct LeadArgs {
    #[command(subcommand)]
    cmd: LeadCommand,
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum LeadCommand {
    /// Read curated baseline JSONL results and write `LEADERBOARD.md`.
    Build {
        /// Source results directory.
        #[arg(long, default_value = "results/baseline")]
        results_dir: PathBuf,
        /// Output Markdown file.
        #[arg(long, default_value = "LEADERBOARD.md")]
        out: PathBuf,
    },
}

pub(crate) fn run(args: &LeadArgs) -> Result<()> {
    match &args.cmd {
        LeadCommand::Build { results_dir, out } => {
            let md = rupert_leaderboard::build_markdown_from_dir(results_dir)?;
            std::fs::write(out, &md)?;
            let mut so = std::io::stdout().lock();
            writeln!(so, "wrote {} bytes -> {}", md.len(), out.display())?;
        }
    }
    Ok(())
}
