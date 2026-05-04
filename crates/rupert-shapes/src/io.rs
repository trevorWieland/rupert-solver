//! JSON I/O for `Polyhedron`.

use std::path::Path;

use rupert_core::Polyhedron;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IoError {
    #[error("read {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("write {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("parse {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("serialize: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub fn load_json(path: impl AsRef<Path>) -> Result<Polyhedron, IoError> {
    let path_ref = path.as_ref();
    let bytes = std::fs::read(path_ref).map_err(|e| IoError::Read {
        path: path_ref.display().to_string(),
        source: e,
    })?;
    serde_json::from_slice(&bytes).map_err(|e| IoError::Parse {
        path: path_ref.display().to_string(),
        source: e,
    })
}

pub fn save_json(poly: &Polyhedron, path: impl AsRef<Path>) -> Result<(), IoError> {
    let path_ref = path.as_ref();
    let bytes = serde_json::to_vec_pretty(poly)?;
    std::fs::write(path_ref, bytes).map_err(|e| IoError::Write {
        path: path_ref.display().to_string(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cube::cube;

    #[test]
    fn round_trip_through_json() {
        let p = cube();
        let dir = std::env::temp_dir().join("rupert_shapes_io_test");
        std::fs::create_dir_all(&dir).expect("create tmp dir");
        let path = dir.join("cube.json");
        save_json(&p, &path).expect("save");
        let loaded = load_json(&path).expect("load");
        assert_eq!(loaded.name, "cube");
        assert_eq!(loaded.vertex_count(), p.vertex_count());
        assert_eq!(loaded.face_count(), p.face_count());
        // The PolyId is computed canonically — should match.
        assert_eq!(loaded.id(), p.id());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_file_produces_read_error() {
        let r = load_json("/definitely/does/not/exist.json");
        assert!(matches!(r, Err(IoError::Read { .. })));
    }
}
