//! rupert-core — foundation crate.
//!
//! Geometry primitives, projection, 2D hull, clearance metric,
//! and the [`Solver`] contract used by every solver registered
//! in `rupert-solvers`.

pub mod clearance;
pub mod error;
pub mod eval;
pub mod exact_vec3;
pub mod expr;
pub mod hull2d;
pub mod poly;
pub mod polyid;
pub mod projection;
pub mod quat;
pub mod runresult;
pub mod solver;
pub mod vec3;

pub use clearance::{clearance, signed_distance_to_polygon};
pub use error::CoreError;
pub use eval::{EvalCounter, evaluate_clearance};
pub use exact_vec3::ExactVec3;
pub use expr::Expr;
pub use hull2d::{convex_hull, point_in_convex_strict};
pub use poly::Polyhedron;
pub use polyid::PolyId;
pub use projection::{P2, project_xy, translate_xy};
pub use quat::Quat;
pub use runresult::{BudgetSnapshot, HostInfo, RunOutcome, RunResult, SCHEMA_VERSION};
pub use solver::{
    Budget, Candidate, CertMethod, Certification, Solution, Solver, SolverError, SolverOutcome,
};
pub use vec3::Vec3;
