//! Bazel-only entry point for immutable snapshot ownership.

pub use qualitygate_domain::domain;
pub use qualitygate_net::net;
pub use qualitygate_paths::paths;
pub use qualitygate_runner::runner;

#[path = "mod.rs"]
pub mod snapshot;
