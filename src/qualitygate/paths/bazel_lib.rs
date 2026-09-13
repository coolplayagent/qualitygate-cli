//! Bazel-only entry point for the confined-path domain.

pub use qualitygate_env::env;

#[path = "mod.rs"]
pub mod paths;
