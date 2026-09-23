//! Command-line parsing and deterministic report presentation.

pub mod cli;
pub mod errors;
mod pilot;
mod policy;
mod render;
mod rule_lifecycle;

/// Early discovery/execution failures still carry the report's claim boundary.
pub fn incomplete_report(error: &str) -> serde_json::Value {
    serde_json::to_value(crate::domain::prerequisites::CommandError::new(
        crate::domain::prerequisites::PrerequisiteIssue::from_error(&anyhow::anyhow!(
            error.to_owned()
        )),
    ))
    .expect("serializable command error")
}

/// Early errors follow the requested format, retaining the same incomplete gate.
pub fn render_incomplete(error: &str, format: cli::Format) -> String {
    errors::ErrorContext {
        root: ".".into(),
        config: "qualitygate.yaml".into(),
        format,
        max_bytes: 65_536,
    }
    .render(&anyhow::anyhow!(error.to_owned()))
}
