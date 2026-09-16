use super::*;
use crate::domain::*;

fn report() -> Report {
    serde_json::from_value(json!({"schema_version":1,"run_id":"run-test","scope":"task","profile":"full",
        "snapshot":{"mode":"worktree","base":"base","head":"head","content_digest":"snapshot"},
        "policy":{"source":"policy","resolved_commit":"fixed-policy","trust":"caller_supplied_ref",
            "config_digest":"config","rules_digest":"rules","task_contract_digest":"task","changes":[]},
        "plan":{"task_id":"task","required_checks":[],"pending_delivery_checks":[],
            "acceptance":{"behavior":"tests"},"acceptance_descriptions":{"behavior":"Actual behavior"}},
        "gate":{"complete":true,"decision":"pass","blockers":[]},"checks":[],"summary":Summary::default(),
        "context":{"report_path":"/evidence/report.json","recheck":{"argv":["qualitygate","check","--policy-ref","fixed-policy"]},
            "delivery_recheck":{"argv":["qualitygate","check","--profile","full","--policy-ref","fixed-policy"]}}
    })).unwrap()
}

fn artifact() -> Artifact {
    Artifact {
        path: "/evidence/report.json".into(),
        digest: "digest".into(),
        bytes: 42,
    }
}

fn diagnostic(index: usize) -> Diagnostic {
    Diagnostic {
        id: format!("issue-{index}"),
        fingerprint: "same-fingerprint".into(),
        file: Some("source.rs".into()),
        range: Some(Range {
            start_line: 1,
            end_line: 1,
        }),
        message: "behavior failed".into(),
        evidence: json!({"private_large_report":"do not inline"}),
        fix: "Repair implementation".into(),
        recheck: Recheck::default(),
    }
}

fn decode(report: &Report, bytes: usize, severity: Option<Severity>) -> Value {
    let output = render(report, artifact(), bytes, severity).unwrap();
    assert!(output.len() < bytes);
    serde_json::from_str(&output).unwrap()
}

#[test]
fn findings_gaps_and_pending_delivery_are_independent_and_gate_is_unchanged() {
    let mut report = report();
    let mut failed = CheckResult::pending("tests", true, Severity::Error);
    failed.diagnostics.push(diagnostic(1));
    failed.complete();
    let mut timeout = CheckResult::pending("slow", true, Severity::Warning);
    timeout.diagnostics.push(diagnostic(2));
    timeout.block(ExecutionStatus::TimedOut, "deadline");
    let mut skipped = CheckResult::pending("irrelevant", false, Severity::Info);
    skipped.skip("No matching input");
    skipped.diagnostics.push(diagnostic(3));
    report.checks = vec![failed, timeout, skipped];
    report.gate = evaluate(&report.checks, &[], &[]);
    report.profile = "quick".into();
    report.plan.pending_delivery_checks.push("delivery".into());
    let output = decode(&report, 32768, None);
    assert_eq!(output["gate"]["decision"], "incomplete");
    assert_eq!(output["findings"]["total"], 1);
    assert_eq!(output["execution_gaps"]["total"], 1);
    assert_eq!(
        output["execution_gaps"]["items"][0]["unverified_diagnostics"],
        1
    );
    assert_eq!(
        output["pending_delivery_checks"]["items"][0]["check_id"]["text"],
        "delivery"
    );
    assert_eq!(output["delivery_ready"], false);
    assert_eq!(
        output["findings"]["items"][0]["evidence_pointer"],
        "/checks/0/diagnostics/0/evidence"
    );
    assert_eq!(
        output["findings"]["items"][0]["acceptance_link_known"],
        true
    );
    assert!(!output.to_string().contains("do not inline"));
    assert_eq!(report.checks[1].diagnostics.len(), 1);
    let filtered = decode(&report, 32768, Some(Severity::Info));
    assert_eq!(filtered["gate"], output["gate"]);
    assert_eq!(filtered["findings"]["filtered"], 1);
    assert_eq!(filtered["execution_gaps"]["total"], 1);
}

#[test]
fn bounded_output_retains_member_references_and_reports_utf8_and_inventory_truncation() {
    let mut report = report();
    let mut failed = CheckResult::pending("unmapped", true, Severity::Error);
    failed.diagnostics = (0..400).map(diagnostic).collect();
    failed.diagnostics[0].message = "边界🚀".repeat(200);
    failed.complete();
    report.checks.push(failed);
    report.gate = evaluate(&report.checks, &[], &[]);
    let output = decode(&report, 32768, None);
    let items = output["findings"]["items"].as_array().unwrap();
    assert!(items.len() > 1 && items.len() < 400);
    assert_eq!(output["findings"]["omitted"], 400 - items.len());
    assert_eq!(items[0]["message"]["truncated"], true);
    assert_eq!(items[0]["acceptance_link_known"], false);
    assert_ne!(items[0]["report_pointer"], items[1]["report_pointer"]);
    assert_eq!(items[0]["fingerprint"], items[1]["fingerprint"]);
    assert_eq!(output["truncated"], true);
    for bytes in [4096, 8192, 65536, MAX_BYTES] {
        decode(&report, bytes, None);
    }
    report.context.as_mut().unwrap().recheck.argv = vec!["unsafe-partial-command".repeat(5000)];
    let output = decode(&report, 4096, None);
    assert!(output["recheck"]["argv"].is_null());
    assert_eq!(output["recheck"]["omitted"], true);
}

#[test]
fn legacy_reports_invalid_states_limits_and_delivery_boundaries_remain_explicit() {
    let mut report = report();
    let output = decode(&report, 32768, None);
    assert_eq!(output["delivery_ready"], true);
    assert_eq!(output["truncated"], false);
    assert_eq!(output["recheck"]["argv"][3], "fixed-policy");
    report.scope = "repository".into();
    report.plan.task_id = None;
    report.context = None;
    let output = decode(&report, 4096, None);
    assert_eq!(output["delivery_ready"], false);
    assert!(output["task"]["id"].is_null());
    assert!(output["recheck"]["argv"].is_null());
    let mut invalid = CheckResult::pending("invalid", true, Severity::Error);
    invalid.execution.status = ExecutionStatus::Completed;
    invalid.diagnostics.push(diagnostic(1));
    report.checks.push(invalid);
    report.gate = evaluate(&report.checks, &[], &[]);
    let output = decode(&report, 4096, None);
    assert_eq!(output["findings"]["total"], 0);
    assert_eq!(output["execution_gaps"]["total"], 1);
    assert!(render(&report, artifact(), 4095, None).is_err());
    assert!(render(&report, artifact(), MAX_BYTES + 1, None).is_err());
    report.run_id = "x".repeat(MAX_BYTES);
    assert!(render(&report, artifact(), 4096, None).is_err());
}
