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
    render_incomplete_with_steps(error, format, &[])
}

/// Adds reliable policy guidance without exposing application ownership to the executable.
pub async fn render_failure(
    error: &anyhow::Error,
    format: cli::Format,
    context: crate::application::policy_guidance::Context,
) -> String {
    let steps = match crate::application::policy_guidance::next_steps(error, context).await {
        Ok(steps) => steps,
        Err(guidance_error) => {
            eprintln!("Cannot prepare next steps: {guidance_error:#}");
            Vec::new()
        }
    };
    render_incomplete_with_steps(&format!("{error:#}"), format, &steps)
}

pub fn render_incomplete_with_steps(
    error: &str,
    format: cli::Format,
    steps: &[crate::domain::NextStep],
) -> String {
    let mut report = incomplete_report(error);
    if !steps.is_empty() {
        report["next_steps"] = serde_json::to_value(steps)
            .expect("serializing static next-step contracts cannot fail");
    }
    if format == cli::Format::Json {
        return report.to_string();
    }
    let verification = &report["verification"];
    let mut output = format!(
        "{}\nGate code: 2 | complete: false\nBlocker: {}\n",
        verification["conclusion"].as_str().unwrap_or_default(),
        render::escape_controls(error)
    );
    for step in steps {
        if let Some(command) = &step.command {
            let (shell, command) = guidance_command(command);
            output.push_str(&format!(
                "Run ({shell}): {}\n",
                render::escape_controls(&command)
            ));
        }
        output.push_str(&format!(
            "Next step: {}\n",
            render::escape_controls(&step.message)
        ));
    }
    for key in ["verified_shapes", "known_limits", "unverified_assumptions"] {
        output.push_str(&format!("{key}: {}\n", verification[key]));
    }
    output
}

fn guidance_command(argv: &[String]) -> (&'static str, String) {
    #[cfg(windows)]
    let (shell, prefix, escaped_quote) = ("PowerShell", "& ", "''");
    #[cfg(not(windows))]
    let (shell, prefix, escaped_quote) = ("POSIX shell", "", "'\\''");
    let command = argv
        .iter()
        .map(|arg| format!("'{}'", arg.replace('\'', escaped_quote)))
        .collect::<Vec<_>>()
        .join(" ");
    (shell, format!("{prefix}{command}"))
}
