//! Pure, I/O-free gate contracts, shared by CLI, rules and execution adapters.

mod attestation;
pub mod case_provenance;
mod category;
pub mod decision_envelope;
pub mod evolution;
pub mod feedback;
mod gate;
pub mod judgment;
pub mod language;
pub mod normative;
pub mod pilot;
pub mod policy_approval;
pub mod policy_effectiveness;
pub mod policy_evaluation;
mod policy_input;
pub mod policy_rollback;
mod project;
mod provenance;
pub mod ratchet;
mod report;
pub mod rule_lifecycle;
mod rule_validation;
pub mod selfcheck;
pub mod snapshot_budget;
mod source_review;
mod verification;

pub use attestation::*;
pub use category::*;
pub use gate::{Decision, Gate, evaluate};
pub use policy_input::{NextStep, PolicyInputError};
pub use project::*;
pub use provenance::*;
pub use report::*;
pub use rule_validation::*;
pub use source_review::*;
pub use verification::*;

pub mod check_scope;
pub mod test_effectiveness;
