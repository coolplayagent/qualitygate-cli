//! Bazel-only entry point for bounded network access.

pub use qualitygate_domain::domain;
pub use qualitygate_env::env;

#[path = "mod.rs"]
pub mod net;
