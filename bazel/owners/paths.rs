//! Bazel-only entry point for the paths capability boundary.

pub use qualitygate_env::env;

#[path = "../../src/qualitygate/paths.rs"]
pub mod paths;
