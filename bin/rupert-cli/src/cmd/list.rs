//! `rupert list shapes` and `rupert list solvers`.

use std::io::Write as _;

use anyhow::Result;

#[derive(clap::Args, Debug)]
pub(crate) struct ListArgs {
    #[command(subcommand)]
    target: ListTarget,
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum ListTarget {
    /// List all built-in polyhedra.
    Shapes,
    /// List all registered solvers.
    Solvers,
}

pub(crate) fn run(args: &ListArgs) -> Result<()> {
    let mut out = std::io::stdout().lock();
    match &args.target {
        ListTarget::Shapes => {
            for poly in rupert_shapes::builtins() {
                writeln!(
                    out,
                    "{}\tvertices={}\tfaces={}",
                    poly.name,
                    poly.vertex_count(),
                    poly.face_count()
                )?;
            }
        }
        ListTarget::Solvers => {
            for solver in rupert_solvers::registered_solvers() {
                writeln!(out, "{}\tv{}", solver.name(), solver.version())?;
            }
        }
    }
    Ok(())
}
