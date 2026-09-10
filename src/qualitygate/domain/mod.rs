//! Pure, I/O-free gate contracts, shared by CLI, rules and execution adapters.

mod attestation;
mod gate;
pub mod language;
mod project;
mod report;

pub use attestation::*;
pub use gate::{Decision, Gate, evaluate};
pub use project::*;
pub use report::*;
