//! Bazel-only entry point for bounded process execution.

pub use qualitygate_domain::domain;
pub use qualitygate_env::env;

#[path = "mod.rs"]
pub mod runner;
