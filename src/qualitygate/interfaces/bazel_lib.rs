//! Bazel-only entry point for CLI parsing and rendering.

pub use qualitygate_application::application;
pub use qualitygate_config::config;
pub use qualitygate_domain::domain;
pub use qualitygate_snapshot::snapshot;

#[path = "mod.rs"]
pub mod interfaces;
