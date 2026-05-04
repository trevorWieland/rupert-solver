//! rupert-bench — single-run + parallel sweep harness.

pub mod jsonl;
pub mod runner;
pub mod sweep;

pub use jsonl::{default_filename, read_all_results, write_results};
pub use runner::run_one;
pub use sweep::{SweepConfig, run_full_sweep, run_sweep};
