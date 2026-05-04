//! Error types for the foundation crate.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("polyhedron has no vertices")]
    EmptyPolyhedron,
    #[error("face {face_index} has fewer than 3 vertices")]
    DegenerateFace { face_index: usize },
    #[error(
        "face {face_index} references vertex {vertex_index}, but only {vertex_count} vertices exist"
    )]
    FaceIndexOutOfRange {
        face_index: usize,
        vertex_index: usize,
        vertex_count: usize,
    },
    #[error("hull2d: fewer than 3 distinct points")]
    InsufficientPoints,
    #[error("eval count drift: solver reported {reported}, observed {observed}")]
    EvalCountDrift { reported: u64, observed: u64 },
}
