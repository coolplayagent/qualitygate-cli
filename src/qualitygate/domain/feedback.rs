//! A bounded, pure projection. The input report remains the sole gate authority.
use super::{Artifact, ExecutionStatus, Report, Severity, Verdict};
use anyhow::{Result, bail};
use serde_json::{Value, json};

pub const MIN_BYTES: usize = 4096;
pub const MAX_BYTES: usize = 256 * 1024;

fn text(value: &str) -> Value {
    let mut end = value.len().min(256);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    json!({"text":&value[..end],"truncated":end < value.len()})
}

fn section(total: usize, filtered: usize, reference: &str) -> Value {
    json!({"total":total,"filtered":filtered,"omitted":total-filtered,
        "report_pointer":reference,"items":[]})
}

fn append(section: &mut Value, item: Value, remaining: &mut usize) -> Result<()> {
    let cost = serde_json::to_vec(&item)?.len() + 1;
    if cost <= *remaining {
        *remaining -= cost;
        section["items"].as_array_mut().unwrap().push(item);
        section["omitted"] = json!(section["omitted"].as_u64().unwrap() - 1);
    }
    Ok(())
}

fn has_truncated_text(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            map.get("truncated") == Some(&Value::Bool(true)) || map.values().any(has_truncated_text)
        }
        Value::Array(values) => values.iter().any(has_truncated_text),
        _ => false,
    }
}

/// `max_bytes` includes the CLI's final newline. Identity/locator overflow is an
/// explicit presentation error; it cannot produce a shortened executable argv.
pub fn render(
    report: &Report,
    artifact: Artifact,
    max_bytes: usize,
    severity: Option<Severity>,
) -> Result<String> {
    if !(MIN_BYTES..=MAX_BYTES).contains(&max_bytes) {
        bail!("Feedback byte limit must be 4096..=262144");
    }
    let completed = |check: &super::CheckResult| {
        check.execution.status == ExecutionStatus::Completed
            && check.applicability == super::Applicability::Applicable
            && matches!(check.verdict, Some(Verdict::Pass | Verdict::Fail))
    };
    let selected =
        |check: &super::CheckResult| severity.is_none_or(|value| check.severity == value);
    let findings = report
        .checks
        .iter()
        .filter(|c| completed(c))
        .map(|c| c.diagnostics.len())
        .sum();
    let filtered = report
        .checks
        .iter()
        .filter(|c| completed(c) && !selected(c))
        .map(|c| c.diagnostics.len())
        .sum();
    let gaps = report
        .checks
        .iter()
        .filter(|c| !completed(c) && c.verdict != Some(Verdict::Skipped))
        .count();
    let pending = &report.plan.pending_delivery_checks;
    let mut result = json!({
        "schema_version":1,"kind":"agent_feedback","run_id":report.run_id,
        "full_report":artifact,"max_bytes":max_bytes,"truncated":false,
        "gate":{"complete":report.gate.complete,"decision":report.gate.decision},
        "scope":report.scope,"profile":report.profile,
        "snapshot":{"mode":report.snapshot.mode,"base":report.snapshot.base,"head":report.snapshot.head,
            "content_digest":report.snapshot.content_digest,"verification_digest":report.snapshot.verification_digest,"report_pointer":"/snapshot"},
        "policy":{"resolved_commit":report.policy.resolved_commit,"trust":report.policy.trust,
            "config_digest":report.policy.config_digest,"rules_digest":report.policy.rules_digest,
            "task_contract_digest":report.policy.task_contract_digest,"changes":report.policy.changes.len(),
            "report_pointer":"/policy"},
        "task":{"id":report.plan.task_id.as_deref().map(text),"report_pointer":"/plan"},
        "delivery_ready":report.gate.decision == super::Decision::Pass && report.gate.complete
            && report.profile == "full" && report.scope == "task" && pending.is_empty(),
        "findings":section(findings,filtered,"/checks"),
        "execution_gaps":section(gaps,0,"/checks"),
        "blockers":section(report.gate.blockers.len(),0,"/gate/blockers"),
        "pending_delivery_checks":section(pending.len(),0,"/plan/pending_delivery_checks"),
        "acceptance":section(report.plan.acceptance.len(),0,"/plan/acceptance"),
        "recheck":{"argv":null,"report_pointer":"/context/recheck","omitted":true},
        "delivery_recheck":{"argv":null,"report_pointer":"/context/delivery_recheck","omitted":true},
        "verification":{"conclusion":text(&report.verification.conclusion),"report_pointer":"/verification"},
        "evidence_availability":"not_rechecked; resolve original artifact paths and verify digests",
        "diagnostic_evidence":"referenced in the full report; not replaced by this view"
    });
    let initial = serde_json::to_vec(&result)?.len();
    // Space for changed booleans and the final newline. Array count widths only shrink.
    let Some(mut remaining) = max_bytes.checked_sub(initial + 16) else {
        bail!(
            "Feedback identity and full report location exceed byte limit; full report: {}",
            result["full_report"]["path"]
        );
    };
    // Preserve small, executable commands in full. Missing context on imported
    // legacy reports is visible through null argv and explicit references.
    if let Some(context) = &report.context {
        for (key, command) in [
            ("recheck", &context.recheck),
            ("delivery_recheck", &context.delivery_recheck),
        ] {
            let bytes = serde_json::to_vec(&command.argv)?.len();
            if bytes <= remaining / 4 {
                result[key]["argv"] = json!(command.argv);
                result[key]["omitted"] = json!(false);
                remaining -= bytes + 1;
            }
        }
    }
    // Every category gets an independent share, so many findings cannot hide gaps.
    let mut share = remaining / 5;
    for (index, value) in pending.iter().enumerate() {
        append(
            &mut result["pending_delivery_checks"],
            json!({"check_id":text(value),
            "report_pointer":format!("/plan/pending_delivery_checks/{index}")}),
            &mut share,
        )?;
    }
    let mut share = remaining / 5;
    for (index, value) in report.gate.blockers.iter().enumerate() {
        append(
            &mut result["blockers"],
            json!({"message":text(value),
            "report_pointer":format!("/gate/blockers/{index}")}),
            &mut share,
        )?;
    }
    let mut share = remaining / 5;
    for (index, (id, check)) in report.plan.acceptance.iter().enumerate() {
        append(
            &mut result["acceptance"],
            json!({"id":text(id),"check_id":text(check),
            "description":report.plan.acceptance_descriptions.get(id).map(|value|text(value)),
            "ordinal":index,"report_pointer":"/plan/acceptance"}),
            &mut share,
        )?;
    }
    let mut gap_budget = remaining / 5;
    let mut finding_budget = remaining / 5;
    for (index, check) in report.checks.iter().enumerate() {
        let reference = format!("/checks/{index}");
        if !completed(check) && check.verdict != Some(Verdict::Skipped) {
            append(
                &mut result["execution_gaps"],
                json!({"check_id":text(&check.id),
                "status":check.execution.status,"required":check.required,"severity":check.severity,
                "reason":check.execution.reason.as_deref().map(text),"report_pointer":reference,
                "unverified_diagnostics":check.diagnostics.len(),
                "artifacts":{"count":check.execution.artifacts.len(),"report_pointer":format!("{reference}/execution/artifacts")}}),
                &mut gap_budget,
            )?;
        } else if completed(check) && selected(check) {
            for (diagnostic_index, diagnostic) in check.diagnostics.iter().enumerate() {
                let pointer = format!("{reference}/diagnostics/{diagnostic_index}");
                append(
                    &mut result["findings"],
                    json!({"check_id":text(&check.id),"severity":check.severity,
                    "id":text(&diagnostic.id),"fingerprint":text(&diagnostic.fingerprint),
                    "file":diagnostic.file.as_deref().map(text),"range":diagnostic.range,
                    "message":text(&diagnostic.message),"fix":text(&diagnostic.fix),
                    "report_pointer":pointer,"evidence_pointer":format!("{pointer}/evidence"),
                    "acceptance_mapping_pointer":"/plan/acceptance",
                    "acceptance_link_known":report.plan.acceptance.values().any(|id|id == &check.id)}),
                    &mut finding_budget,
                )?;
            }
        }
    }
    result["truncated"] = json!(
        has_truncated_text(&result)
            || [
                "findings",
                "execution_gaps",
                "blockers",
                "pending_delivery_checks",
                "acceptance"
            ]
            .iter()
            .any(|key| result[*key]["omitted"].as_u64().unwrap() > 0)
            || result["recheck"]["omitted"] == true
            || result["delivery_recheck"]["omitted"] == true
    );
    let output = serde_json::to_string(&result)?;
    if output.len() + 1 > max_bytes {
        bail!("Feedback exceeded its serialization budget");
    }
    Ok(output)
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
