use crate::domain::{Artifact, doctor::Report};
use serde_json::json;

const DETAIL_LIMIT: usize = 5;
const TEXT_LIMIT: usize = 240;

pub(super) fn render(
    report: &Report,
    artifact: Option<Artifact>,
    format: super::cli::Format,
) -> anyhow::Result<String> {
    let mut truncated =
        report.diagnostics.len() > DETAIL_LIMIT || report.next_steps.len() > DETAIL_LIMIT;
    let diagnostics: Vec<_> = report
        .diagnostics
        .iter()
        .take(DETAIL_LIMIT)
        .map(|item| {
            json!({
                "code": brief(&item.code, &mut truncated),
                "message": brief(&item.message, &mut truncated),
                "field": item.field.as_ref().map(|field| brief(field, &mut truncated)),
                "line": item.line,
                "column": item.column,
            })
        })
        .collect();
    let next_steps: Vec<_> = report.next_steps.iter().take(DETAIL_LIMIT).map(|item| {
        json!({"action": brief(&item.action, &mut truncated), "message": brief(&item.message, &mut truncated)})
    }).collect();
    super::render::metadata(
        &json!({
            "schema_version": 1,
            "kind": "doctor_summary",
            "scope": "preflight",
            "profile": report.profile,
            "probe_tools": report.probe_tools,
            "complete": report.complete,
            "exit_code": report.exit_code(),
            "report": artifact,
            "counts": {
                "diagnostics": report.diagnostics.len(),
                "next_steps": report.next_steps.len(),
                "planned_checks": report.planned_checks.len(),
                "pending_delivery_checks": report.pending_delivery_checks.len(),
                "not_executed": report.not_executed.len(),
            },
            "diagnostics": diagnostics,
            "next_steps": next_steps,
            "details_truncated": truncated,
            "notice": "Preflight is not a delivery verdict. Read report.path for full diagnostics, identities, budgets and executable next-step argv; remove its temporary directory when no longer needed.",
        }),
        format,
    )
}

fn brief(text: &str, truncated: &mut bool) -> String {
    let mut characters = text.chars();
    let mut result: String = characters.by_ref().take(TEXT_LIMIT).collect();
    if characters.next().is_some() {
        *truncated = true;
        result.push('…');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{NextStep, doctor::Diagnostic};

    #[test]
    fn bounded_summary_preserves_counts_unicode_and_preflight_status() {
        let mut report = Report::new("full".into(), false);
        for _ in 0..100 {
            let mut diagnostic =
                Diagnostic::new("坏".repeat(300).as_str(), "证据".repeat(500), vec![]);
            diagnostic.field = Some("长".repeat(500));
            diagnostic.line = Some(7);
            report.diagnostics.push(diagnostic);
            report.next_steps.push(NextStep {
                action: "修".repeat(500),
                message: "复".repeat(500),
                command: None,
            });
        }
        let output = render(&report, None, super::super::cli::Format::Json).unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["counts"]["diagnostics"], 100);
        assert_eq!(value["counts"]["next_steps"], 100);
        assert_eq!(value["diagnostics"].as_array().unwrap().len(), 5);
        assert_eq!(value["next_steps"].as_array().unwrap().len(), 5);
        assert_eq!(value["diagnostics"][0]["line"], 7);
        assert_eq!(
            value["diagnostics"][0]["message"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            241
        );
        assert_eq!(value["details_truncated"], true);
        assert_eq!(value["exit_code"], 2);
        assert!(value["report"].is_null());
        assert!(output.len() < 22_000);
    }
}
