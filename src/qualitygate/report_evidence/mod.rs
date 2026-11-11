//! Resolves captured report sources, normalizes gate evidence and renders reports.
//!

pub mod coverage;
pub(crate) mod coverage_gate;
pub mod decision_envelope;
pub mod decision_schema;
pub mod diagnostic_budget;
pub(crate) mod failure;
pub(crate) mod generated_reports;
pub(crate) mod project_reports;
pub mod ratchet;
pub(crate) mod render;
pub(crate) mod report;
pub(crate) mod report_gate;
pub(crate) mod report_sources;
pub(crate) mod test_counts;
pub mod tool_reports;
pub(crate) mod verification;
