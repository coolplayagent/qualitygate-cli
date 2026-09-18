//! Rule and ecosystem adapters produce shared results without CLI dependencies.

pub mod attestation;
mod builtin_conventions;
pub mod compatibility;
pub mod custom_rules;
mod entity_changes;
pub mod facts;
pub mod git_trailers;
mod markers;
pub mod maven;
pub mod maven_usage;
mod parallel;
pub mod pilot_acceptance;
pub mod pilot_authorization;
pub mod policy_approval;
pub mod policy_rollback;
mod project_rules;
pub mod provenance;
pub mod python;
mod python_metadata;
pub mod reports;
pub mod rules;
mod shell_conventions;
mod structure_rules;
pub mod syntax;
