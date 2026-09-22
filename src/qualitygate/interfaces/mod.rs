//! Command-line parsing and deterministic report presentation.

pub mod cli;
mod pilot;
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

/// Early errors follow the requested format, retaining the same incomplete gate.
pub fn render_incomplete(error: &str, format: cli::Format) -> String {
    let report = incomplete_report(error);
    if format == cli::Format::Json {
        return report.to_string();
    }
    let verification = &report["verification"];
    let mut output = format!(
        "{}\nGate code: 2 | complete: false\nBlocker: {}\n",
        verification["conclusion"].as_str().unwrap_or_default(),
        render::escape_controls(error)
    );
    if error.contains("run qualitygate init first") || error.contains("run init and stage/commit") {
        output.push_str("Run: qualitygate init (use the same --root and --config). For staged/diff checks, stage/commit the candidate after review.\n");
    }
    for key in ["verified_shapes", "known_limits", "unverified_assumptions"] {
        output.push_str(&format!("{key}: {}\n", verification[key]));
    }
    output
}
