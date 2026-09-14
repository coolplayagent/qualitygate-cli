//! Pure, I/O-free gate contracts, shared by CLI, rules and execution adapters.

mod attestation;
mod gate;
pub mod language;
mod project;
mod provenance;
mod report;
pub mod selfcheck;
mod source_review;
mod verification;

pub use attestation::*;
pub use gate::{Decision, Gate, evaluate};
pub use project::*;
pub use provenance::*;
pub use report::*;
pub use source_review::*;
pub use verification::*;
