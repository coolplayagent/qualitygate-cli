//! Bazel-only entry point for application orchestration.

pub use qualitygate_adapters::adapters;
pub use qualitygate_config::config;
pub use qualitygate_domain::domain;
pub use qualitygate_env::env;
pub use qualitygate_net::net;
pub use qualitygate_paths::paths;
pub use qualitygate_runner::runner;
pub use qualitygate_snapshot::snapshot;

#[path = "mod.rs"]
pub mod application;
