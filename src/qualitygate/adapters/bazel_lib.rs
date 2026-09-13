//! Bazel-only entry point for language and report adapters.

pub use qualitygate_config::config;
pub use qualitygate_domain::domain;
pub use qualitygate_paths::paths;
pub use qualitygate_snapshot::snapshot;

#[path = "mod.rs"]
pub mod adapters;
