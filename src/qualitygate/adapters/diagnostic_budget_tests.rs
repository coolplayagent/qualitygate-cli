use super::*;
use crate::{
    adapters::rules::diagnostic,
    domain::{Severity, Verdict},
};

fn finding(fix: &str) -> Diagnostic {
    diagnostic(
        "rule",
        Some("source.rs"),
        None,
        "Repair this finding".into(),
        serde_json::json!({"text":"\"quoted\"\n界\u{0}"}),
        fix,
        "finding",
    )
}
fn result() -> CheckResult {
    CheckResult::pending("rule", true, Severity::Error)
}

#[test]
fn diagnostic_bytes_include_json_escaping_and_failed_reservations_are_atomic() {
    let first = finding("repair\n\"界\"");
    let exact = serde_json::to_vec(&vec![&first, &first]).unwrap().len();
    let mut budget = Budget {
        max_bytes: exact,
        ..Budget::default()
    };
    let mut result = result();
    budget.push(&mut result, first.clone()).unwrap();
    budget.push(&mut result, first.clone()).unwrap();
    assert_eq!(budget.bytes, exact);
    assert!(
        budget
            .push(&mut result, first)
            .unwrap_err()
            .to_string()
            .contains("serialized JSON bytes")
    );
    assert_eq!((budget.count, budget.bytes), (2, exact));
    assert_eq!(result.diagnostics.len(), 2);
    assert_eq!(result.metadata["diagnostic_output"]["complete"], false);
    assert_eq!(
        serde_json::to_vec(&result.diagnostics).unwrap().len(),
        exact
    );
}

#[test]
fn count_exhaustion_retains_findings_without_claiming_a_complete_omission_count() {
    let mut budget = Budget {
        max_count: 1,
        ..Budget::default()
    };
    let mut result = result();
    budget.push(&mut result, finding("repair")).unwrap();
    assert!(
        budget
            .push(&mut result, finding("repair"))
            .unwrap_err()
            .to_string()
            .contains("records")
    );
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.metadata["diagnostic_output"]["omitted_count_known"],
        false
    );
}

#[test]
fn recheck_expansion_is_bounded_and_preserves_prior_execution_failure() {
    let argv = vec!["qualitygate".into(), "--root".into(), "r".repeat(500)];
    let mut first = finding("repair");
    first.recheck.argv = argv.clone();
    let exact = serde_json::to_vec(&vec![first]).unwrap().len();
    for timed_out in [false, true] {
        let mut result = result();
        result.diagnostics = vec![finding("repair"), finding("repair")];
        result.complete();
        assert_eq!(result.verdict, Some(Verdict::Fail));
        if timed_out {
            result.block(ExecutionStatus::TimedOut, "original timeout");
        }
        attach(
            &mut result,
            &argv,
            Budget {
                max_bytes: exact,
                ..Budget::default()
            },
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].recheck.argv, argv);
        assert_eq!(
            serde_json::to_vec(&result.diagnostics).unwrap().len(),
            exact
        );
        assert!(result.verdict.is_none());
        assert_eq!(
            result.metadata["diagnostic_output"]["phase"],
            "recheck_binding"
        );
        assert_eq!(
            result.execution.status,
            if timed_out {
                ExecutionStatus::TimedOut
            } else {
                ExecutionStatus::Blocked
            }
        );
        if timed_out {
            assert!(
                result
                    .execution
                    .reason
                    .as_ref()
                    .unwrap()
                    .starts_with("original timeout;")
            );
        }
    }
    let mut result = result();
    result.diagnostics.push(finding("repair"));
    result.complete();
    attach_recheck(&mut result, &argv);
    assert_eq!(result.verdict, Some(Verdict::Fail));
    assert!(!result.metadata.contains_key("diagnostic_output"));
}

#[test]
fn a_single_oversized_finding_cannot_enter_the_report() {
    let mut result = result();
    let mut budget = Budget {
        max_bytes: 32,
        ..Budget::default()
    };
    assert!(budget.push(&mut result, finding("repair")).is_err());
    assert!(result.diagnostics.is_empty());
    assert_eq!((budget.count, budget.bytes), (0, 2));
}

#[test]
fn truncation_cannot_turn_an_invalid_skip_into_usable_evidence() {
    let mut result = result();
    result.skip("Original non-applicability explanation");
    result.diagnostics.push(finding("repair"));
    attach(
        &mut result,
        &[],
        Budget {
            max_bytes: 32,
            ..Budget::default()
        },
    );
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.execution.status, ExecutionStatus::Blocked);
    assert!(result.verdict.is_none());
    assert!(
        result
            .execution
            .reason
            .as_ref()
            .unwrap()
            .starts_with("Original non-applicability explanation;")
    );
    let mut passed = CheckResult::pending("other", true, Severity::Error);
    passed.complete();
    let gate = crate::domain::evaluate(&[result, passed], &["rule".into()], &[]);
    assert_eq!(gate.decision, crate::domain::Decision::Incomplete);
}

#[test]
fn every_project_rule_version_bounds_its_actual_emitted_record_count() {
    use crate::{
        adapters::custom_rules,
        config::{CustomRule, RuleSetting},
        snapshot::{File, Identity, Snapshot},
    };
    let snapshot = Snapshot {
        scope_evidence: Default::default(),
        root: "/repo".into(),
        identity: Identity {
            verification_digest: None,
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
            merge_request: None,
        },
        files: (0..=MAX_DIAGNOSTICS)
            .map(|index| {
                (
                    format!("{index}.txt"),
                    File {
                        bytes: b"bad".to_vec(),
                        executable: false,
                    },
                )
            })
            .collect(),
        base_files: Default::default(),
        changes: Default::default(),
        path_filter: None,
        commits: vec![],
    };
    for version in [1, 2, 3] {
        let mut value = serde_json::json!({
            "schema_version":version,"id":"bounded","version":1,
            "source":{"document":"AGENTS.md","section":"Files","content_hash":format!("sha256:{}", "0".repeat(64))},
            "requires_capabilities":["files"],"when":{"entity":"file","change":"all"},
            "then":{"forbid_pattern":"bad"},"fix":"Repair the matching files"
        });
        if version >= 2 {
            value["expression"] = serde_json::json!({"op":"bool","value":true});
        }
        let rule: CustomRule = serde_json::from_value(value).unwrap();
        let result = custom_rules::evaluate(&rule, &RuleSetting::default(), &snapshot);
        assert_eq!(result.execution.status, ExecutionStatus::Blocked);
        assert!(result.verdict.is_none());
        assert_eq!(result.diagnostics.len(), MAX_DIAGNOSTICS);
        assert!(result.execution.reason.unwrap().contains("10000 records"));
    }
}
