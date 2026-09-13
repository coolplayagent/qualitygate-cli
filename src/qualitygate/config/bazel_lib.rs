//! Bazel-only entry point for configuration ownership.

pub use qualitygate_domain::domain;
pub use qualitygate_paths::paths;

#[path = "mod.rs"]
pub mod config;
