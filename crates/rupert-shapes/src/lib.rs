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
pub mod hull3d;
pub mod icosahedron;
pub mod io;
pub mod noperthedron;
pub mod octahedron;
pub mod pentagonal_icositetrahedron;
pub mod snub_cube;
pub mod tetrahedron;
pub mod triakis_tetrahedron;

pub use cube::cube;
pub use dodecahedron::dodecahedron;
pub use icosahedron::icosahedron;
pub use io::{IoError, load_json, save_json};
pub use noperthedron::noperthedron;
pub use octahedron::octahedron;
pub use pentagonal_icositetrahedron::pentagonal_icositetrahedron;
pub use snub_cube::snub_cube;
pub use tetrahedron::tetrahedron;
pub use triakis_tetrahedron::triakis_tetrahedron;

use rupert_core::{Polyhedron, Quat};

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
        pentagonal_icositetrahedron(),
        snub_cube(),
        noperthedron(),
    ]
}

/// Look up a builtin polyhedron by name; `None` if unknown.
pub fn lookup(name: &str) -> Option<Polyhedron> {
    builtins().into_iter().find(|p| p.name == name)
}

/// Rotation symmetry group for a builtin shape, as a list of unit
/// quaternions. Unknown shapes return `vec![Quat::IDENTITY]` (a trivial
/// group); the patch_aware solver gracefully handles this — no symmetry
/// reduction means full O(F²) brute force.
#[must_use]
pub fn rotation_group_for(name: &str) -> Vec<Quat> {
    use rupert_core::symmetry::{
        dodecahedral_rotation_group, icosahedral_rotation_group, octahedral_rotation_group,
        tetrahedral_rotation_group,
    };
    match name {
        "tetrahedron" | "triakis_tetrahedron" => tetrahedral_rotation_group(),
        "cube" | "octahedron" | "snub_cube" | "pentagonal_icositetrahedron" => {
            octahedral_rotation_group()
        }
        "icosahedron" => icosahedral_rotation_group(),
        "dodecahedron" => dodecahedral_rotation_group(),
        _ => vec![Quat::IDENTITY],
    }
}

#[cfg(test)]
mod symmetry_validation {
    use rupert_core::symmetry::{
        dodecahedral_rotation_group, icosahedral_rotation_group, octahedral_rotation_group,
        tetrahedral_rotation_group,
    };
    use rupert_core::{Polyhedron, Quat, Vec3};

    fn permutes(vertices: &[Vec3], q: Quat, tol: f64) -> bool {
        for v in vertices {
            let r = q.rotate(*v);
            if !vertices.iter().any(|o| (*o - r).norm() < tol) {
                return false;
            }
        }
        true
    }

    fn check_group(group: &[Quat], poly: &Polyhedron) {
        for (i, q) in group.iter().enumerate() {
            assert!(
                permutes(&poly.vertices, *q, 1e-9),
                "group element {i} (q = {q:?}) does not permute {} vertex set",
                poly.name
            );
        }
    }

    #[test]
    fn tetrahedral_group_permutes_tetrahedron() {
        check_group(&tetrahedral_rotation_group(), &super::tetrahedron());
    }

    #[test]
    fn octahedral_group_permutes_cube() {
        check_group(&octahedral_rotation_group(), &super::cube());
    }

    #[test]
    fn octahedral_group_permutes_octahedron() {
        check_group(&octahedral_rotation_group(), &super::octahedron());
    }

    #[test]
    fn icosahedral_group_permutes_icosahedron() {
        check_group(&icosahedral_rotation_group(), &super::icosahedron());
    }

    #[test]
    fn dodecahedral_group_permutes_dodecahedron() {
        check_group(&dodecahedral_rotation_group(), &super::dodecahedron());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_builtins() {
        assert_eq!(builtins().len(), 9);
    }

    #[test]
    fn names_are_unique() {
        let polyhedra = builtins();
        let names: Vec<&str> = polyhedra.iter().map(|p| p.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            names.len(),
            "duplicate shape names: {names:?}"
        );
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
