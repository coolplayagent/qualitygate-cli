//! Pure, I/O-free gate contracts, shared by CLI, rules and execution adapters.

mod gate;
mod project;
mod report;

pub use gate::{Decision, Gate, evaluate};
pub use project::*;
pub use report::*;
