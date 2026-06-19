//! Patch-table construction and per-shape caching.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use rupert_core::symmetry::{
    dodecahedral_rotation_group, icosahedral_rotation_group, octahedral_rotation_group,
    tetrahedral_rotation_group,
};
use rupert_core::{PolyId, Polyhedron, Quat, Vec3};

use crate::sample::random_unit_quat;

pub(super) const MAX_ENUM_SAMPLES: usize = 200_000;
pub(super) const STAGNATION_LIMIT: usize = 2_000;
pub(super) const SIGN_VEC_BITS: usize = 128;

/// One raw patch from enumeration.
#[derive(Debug, Clone, Copy)]
pub(super) struct PatchEntry {
    pub sign_vec: u128,
    pub q_rep: Quat,
}

/// One canonical patch after orbit reduction.
#[derive(Debug, Clone)]
pub(super) struct CanonicalPatch {
    pub sign_vec: u128,
    pub q_rep: Quat,
}

/// Per-shape patch table, cached by `PolyId`.
#[derive(Debug)]
pub(super) struct PatchTable {
    pub canonical: Vec<CanonicalPatch>,
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub max_vertex_norm: f64,
}

/// Internal name → rotation group dispatch. Mirrors
/// `rupert_shapes::rotation_group_for` but lives here so
/// `rupert-solvers` can stay free of its `rupert-shapes` dep
/// (layering: both crates are L1, can't share runtime deps).
pub(super) fn rotation_group_for(name: &str) -> Vec<Quat> {
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

/// Compute outward face normals via Newell's method (robust to
/// non-coplanar quads).
pub(super) fn face_normals(poly: &Polyhedron) -> Vec<Vec3> {
    poly.faces
        .iter()
        .map(|face| {
            let mut nx = 0.0_f64;
            let mut ny = 0.0_f64;
            let mut nz = 0.0_f64;
            for i in 0..face.len() {
                let curr = poly.vertices[face[i]];
                let next = poly.vertices[face[(i + 1) % face.len()]];
                nx += (curr.y - next.y) * (curr.z + next.z);
                ny += (curr.z - next.z) * (curr.x + next.x);
                nz += (curr.x - next.x) * (curr.y + next.y);
            }
            Vec3::new(nx, ny, nz).normalized()
        })
        .collect()
}

/// Compute the face-sign vector for a given rotation. Bit `i` is 1 iff
/// face `i`'s rotated normal has positive z-component.
pub(super) fn face_sign_vector(normals: &[Vec3], q: &Quat) -> u128 {
    let limit = normals.len().min(SIGN_VEC_BITS);
    let mut bits: u128 = 0;
    for (i, n) in normals.iter().take(limit).enumerate() {
        let rotated = q.rotate(*n);
        if rotated.z >= 0.0 {
            bits |= 1u128 << i;
        }
    }
    bits
}

/// Brute-force patch enumeration with stagnation stop.
pub(super) fn enumerate_patches(normals: &[Vec3], shape_seed: u64) -> Vec<PatchEntry> {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(shape_seed);
    let mut table: BTreeMap<u128, Quat> = BTreeMap::new();
    let mut stagnation = 0_usize;
    for _ in 0..MAX_ENUM_SAMPLES {
        let q = random_unit_quat(&mut rng);
        let sv = face_sign_vector(normals, &q);
        if let std::collections::btree_map::Entry::Vacant(e) = table.entry(sv) {
            e.insert(q);
            stagnation = 0;
        } else {
            stagnation += 1;
            if stagnation >= STAGNATION_LIMIT && !table.is_empty() {
                break;
            }
        }
    }
    table
        .into_iter()
        .map(|(sv, q)| PatchEntry {
            sign_vec: sv,
            q_rep: q,
        })
        .collect()
}

/// Reduce raw patches to canonical orbit representatives under the
/// rotation group. Lex-min sign-vector is the canonical key.
pub(super) fn canonicalize_under_symmetry(
    patches: &[PatchEntry],
    group: &[Quat],
    normals: &[Vec3],
) -> Vec<CanonicalPatch> {
    if group.len() <= 1 {
        return patches
            .iter()
            .map(|p| CanonicalPatch {
                sign_vec: p.sign_vec,
                q_rep: p.q_rep,
            })
            .collect();
    }
    let mut canonical_key_for: BTreeMap<u128, u128> = BTreeMap::new();
    for entry in patches {
        let mut min_sv = entry.sign_vec;
        for g in group.iter().skip(1) {
            let rotated = (*g * entry.q_rep).normalized();
            let sv_rot = face_sign_vector(normals, &rotated);
            if sv_rot < min_sv {
                min_sv = sv_rot;
            }
        }
        canonical_key_for.insert(entry.sign_vec, min_sv);
    }
    let mut canonical_map: BTreeMap<u128, Quat> = BTreeMap::new();
    for entry in patches {
        let canonical_sv = *canonical_key_for
            .get(&entry.sign_vec)
            .unwrap_or(&entry.sign_vec);
        let q_slot = canonical_map.entry(canonical_sv).or_insert(entry.q_rep);
        if entry.sign_vec == canonical_sv {
            *q_slot = entry.q_rep;
        }
    }
    canonical_map
        .into_iter()
        .map(|(sv, q_rep)| CanonicalPatch {
            sign_vec: sv,
            q_rep,
        })
        .collect()
}

fn shape_seed(name: &str) -> u64 {
    let hash = blake3::hash(name.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[0..8]);
    u64::from_le_bytes(bytes)
}

fn cache() -> &'static Mutex<HashMap<PolyId, Arc<PatchTable>>> {
    static CACHE: OnceLock<Mutex<HashMap<PolyId, Arc<PatchTable>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn patch_table_for(poly: &Polyhedron) -> Arc<PatchTable> {
    let key = *poly.id();
    {
        let map = cache().lock().expect("cache mutex");
        if let Some(t) = map.get(&key) {
            return Arc::clone(t);
        }
    }
    let normals = face_normals(poly);
    let max_vertex_norm = poly
        .vertices
        .iter()
        .map(|v| v.norm())
        .fold(0.0_f64, f64::max);
    let raw = enumerate_patches(&normals, shape_seed(&poly.name));
    let group = rotation_group_for(&poly.name);
    let canonical = canonicalize_under_symmetry(&raw, &group, &normals);
    let table = Arc::new(PatchTable {
        canonical,
        vertices: poly.vertices.clone(),
        normals,
        max_vertex_norm,
    });
    let mut map = cache().lock().expect("cache mutex");
    Arc::clone(map.entry(key).or_insert(table))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_cube_patches_under_octahedral_symmetry() {
        let cube = rupert_shapes::cube();
        let table = patch_table_for(&cube);
        assert!(!table.canonical.is_empty());
        let group = rotation_group_for("cube");
        assert_eq!(group.len(), 24);
        let raw = enumerate_patches(&table.normals, shape_seed("cube"));
        assert!(table.canonical.len() <= raw.len());
    }

    #[test]
    fn face_sign_vector_is_deterministic() {
        let cube = rupert_shapes::cube();
        let normals = face_normals(&cube);
        let q = Quat::IDENTITY;
        assert_eq!(
            face_sign_vector(&normals, &q),
            face_sign_vector(&normals, &q)
        );
    }
}
