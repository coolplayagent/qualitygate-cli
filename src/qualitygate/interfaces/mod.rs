//! Command-line parsing and deterministic report presentation.

pub mod cli;
mod policy;
mod render;
mod rule_lifecycle;

/// Early discovery/execution failures still carry the report's claim boundary.
pub fn incomplete_report(error: &str) -> serde_json::Value {
    let gate = crate::domain::Gate {
        complete: false,
        decision: crate::domain::Decision::Incomplete,
        blockers: vec![error.into()],
    };
    let verification = crate::domain::VerificationBoundary::for_check(&gate, &[], "full", false);
    serde_json::json!({"schema_version":1,"gate":gate,"verification":verification})
}
