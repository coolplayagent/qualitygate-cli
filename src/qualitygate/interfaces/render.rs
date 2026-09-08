use crate::domain::{Report, Severity};

pub(super) fn report(
    report: &Report,
    format: super::cli::Format,
    severity: Option<Severity>,
) -> anyhow::Result<String> {
    let mut report = report.clone();
    if let Some(severity) = severity {
        for check in &mut report.checks {
            if check.severity != severity {
                report.summary.diagnostics_filtered += check.diagnostics.len();
                check.diagnostics.clear();
            }
        }
    }
    if format == super::cli::Format::Json {
        return Ok(serde_json::to_string_pretty(&report)?);
    }
    let mut out = format!(
        "Gate: {:?} | complete: {} | scope: {} | profile: {}\n",
        report.gate.decision, report.gate.complete, report.scope, report.profile
    );
    if format == super::cli::Format::Markdown {
        out.push_str("\n| Check | Execution | Verdict | Details |\n|---|---|---|---|\n");
    }
    for check in &report.checks {
        let mut details = check.execution.reason.clone().unwrap_or_default();
        for diagnostic in &check.diagnostics {
            details.push_str(&format!(
                " {}:{} {}. {}",
                diagnostic.file.as_deref().unwrap_or("-"),
                diagnostic
                    .range
                    .as_ref()
                    .map_or(0, |range| range.start_line),
                diagnostic.message,
                diagnostic.fix
            ));
        }
        let details = details.replace('|', "\\|").replace(['\n', '\r'], " ");
        if format == super::cli::Format::Markdown {
            out.push_str(&format!(
                "| {} | {:?} | {:?} | {} |\n",
                check.id, check.execution.status, check.verdict, details
            ));
        } else {
            out.push_str(&format!(
                "{}\t{:?}\t{:?}\t{}\n",
                check.id, check.execution.status, check.verdict, details
            ));
        }
    }
    for blocker in &report.gate.blockers {
        out.push_str(&format!("Blocker: {blocker}\n"));
    }
    if !report.plan.pending_delivery_checks.is_empty() {
        out.push_str(&format!(
            "Pending delivery checks: {}\n",
            report.plan.pending_delivery_checks.join(", ")
        ));
    }
    out.push_str(&format!(
        "Snapshot: {}\nFiltered diagnostics: {}\n",
        report.snapshot.content_digest, report.summary.diagnostics_filtered
    ));
    Ok(out)
}
