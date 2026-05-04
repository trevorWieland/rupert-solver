//! rupert-shapes — builtin convex polyhedra and JSON I/O.
//!
//! Add a new builtin:
//! 1. Drop a new module `crates/rupert-shapes/src/<name>.rs` exposing a
//!    `pub fn <name>() -> Polyhedron`.
//! 2. Append to [`builtins`] below.
//!
//! Adding a builtin should NOT require touching anything else in the
//! workspace — the CLI iterates this list and the leaderboard sorts by
//! polyhedron name automatically.

pub mod cube;
pub mod dodecahedron;
pub mod icosahedron;
pub mod io;
pub mod noperthedron;
pub mod octahedron;
pub mod snub_cube;
pub mod tetrahedron;
pub mod triakis_tetrahedron;

pub use cube::cube;
pub use dodecahedron::dodecahedron;
pub use icosahedron::icosahedron;
pub use io::{IoError, load_json, save_json};
pub use noperthedron::noperthedron;
pub use octahedron::octahedron;
pub use snub_cube::snub_cube;
pub use tetrahedron::tetrahedron;
pub use triakis_tetrahedron::triakis_tetrahedron;

use rupert_core::Polyhedron;

/// All builtin polyhedra in canonical order. The CLI's `rupert list shapes`
/// reads this list verbatim.
pub fn builtins() -> Vec<Polyhedron> {
    vec![
        tetrahedron(),
        cube(),
        octahedron(),
        dodecahedron(),
        icosahedron(),
        triakis_tetrahedron(),
        snub_cube(),
        noperthedron(),
    ]
}

/// Look up a builtin polyhedron by name; `None` if unknown.
pub fn lookup(name: &str) -> Option<Polyhedron> {
    builtins().into_iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_builtins() {
        assert_eq!(builtins().len(), 8);
    }

    #[test]
    fn names_are_unique() {
        let polyhedra = builtins();
        let names: Vec<&str> = polyhedra.iter().map(|p| p.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "duplicate shape names: {names:?}");
    }

    #[test]
    fn lookup_finds_each_builtin() {
        for p in builtins() {
            let found = lookup(&p.name);
            assert!(found.is_some(), "lookup failed for {}", p.name);
        }
    }

    #[test]
    fn lookup_unknown_returns_none() {
        assert!(lookup("not_a_real_shape").is_none());
    }
}
