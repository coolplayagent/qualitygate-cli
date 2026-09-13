use crate::domain::{Report, Severity};

pub(super) fn metadata(
    value: &serde_json::Value,
    format: super::cli::Format,
) -> anyhow::Result<String> {
    use super::cli::Format;
    if format == Format::Json {
        return Ok(serde_json::to_string_pretty(value)?);
    }
    if let Some(rules) = value["rules"].as_array() {
        let mut out: String = if format == Format::Markdown {
            "| Rule | Enabled | Origin | Capabilities |\n|---|---|---|---|\n".into()
        } else {
            "Rule\tEnabled\tOrigin\tCapabilities\n".into()
        };
        for rule in rules {
            let definition = &rule["definition"];
            let details = if definition["custom"].is_null() {
                &definition["builtin"]
            } else {
                &definition["custom"]
            };
            let fields = [
                rule["id"].as_str().unwrap_or_default().to_owned(),
                rule["enabled"].to_string(),
                definition["origin"].as_str().unwrap_or_default().to_owned(),
                details["requires_capabilities"].to_string(),
            ];
            let fields: Vec<_> = fields
                .into_iter()
                .map(|field| field.replace('|', "\\|").replace(['\n', '\r', '\t'], " "))
                .collect();
            if format == Format::Markdown {
                out.push_str(&format!("| {} |\n", fields.join(" | ")));
            } else {
                out.push_str(&format!("{}\n", fields.join("\t")));
            }
        }
        return Ok(out);
    }
    let yaml = serde_norway::to_string(value)?;
    Ok(if format == Format::Markdown {
        format!("```yaml\n{yaml}```\n")
    } else {
        yaml
    })
}

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
    if let Some(task) = &report.plan.task_id {
        out.push_str(&format!(
            "Task: {}\n",
            task.replace(['\n', '\r', '\t'], " ")
        ));
        for (id, check) in &report.plan.acceptance {
            let description = report
                .plan
                .acceptance_descriptions
                .get(id)
                .map(String::as_str)
                .unwrap_or_default();
            out.push_str(&format!(
                "Acceptance {id} ({check}): {}\n",
                description.replace(['\n', '\r', '\t'], " ")
            ));
        }
    }
    if let Some(comparison) = &report.snapshot.merge_request {
        out.push_str(&format!(
            "MR: {}\nTarget: {}\nSource: {}\nMerge base: {}\n",
            comparison.url, comparison.target_head, comparison.source_head, comparison.merge_base
        ));
    }
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
