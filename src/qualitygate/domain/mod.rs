//! Pure, I/O-free gate contracts, shared by CLI, rules and execution adapters.

mod attestation;
mod category;
pub mod evolution;
mod gate;
pub mod language;
pub mod normative;
pub mod policy_approval;
pub mod policy_effectiveness;
pub mod policy_evaluation;
pub mod policy_rollback;
mod project;
mod provenance;
pub mod ratchet;
mod report;
pub mod rule_lifecycle;
mod rule_validation;
pub mod selfcheck;
mod source_review;
mod verification;

pub use attestation::*;
pub use category::*;
pub use gate::{Decision, Gate, evaluate};
pub use project::*;
pub use provenance::*;
pub use report::*;
pub use rule_validation::*;
pub use source_review::*;
pub use verification::*;
