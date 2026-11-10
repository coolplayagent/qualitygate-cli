use super::super::test_support;
use super::*;
use serde_json::json;

#[test]
fn source_adoption_and_execution_are_separate_requirements_for_verified_coverage() {
    let mut snapshot = test_support::snapshot(std::path::Path::new("/unit"));
    let source = test_support::source(&mut snapshot);
    let binding = crate::snapshot::digest(b"binding");
    let base = constraint::Constraint {
        schema_version: 1,
        id: "tests".into(),
        version: 1,
        source,
        statement: "Keep meaningful tests".into(),
        required: true,
        realization: Realization::Checks {
            mechanism: constraint::Mechanism::Tool,
            checks: vec!["unit".into()],
            positive_examples: vec!["pass".into()],
            negative_examples: vec!["fail".into()],
        },
        adoption: Some(test_support::adoption(&binding)),
    };
    let bindings = [("tests".into(), binding)].into();
    for (scenario, status, execution, verdict) in [
        (
            "pass",
            "verified",
            ExecutionStatus::Completed,
            Some(Verdict::Pass),
        ),
        (
            "failure",
            "failed",
            ExecutionStatus::Completed,
            Some(Verdict::Fail),
        ),
        ("timeout", "pending", ExecutionStatus::Blocked, None),
        (
            "skipped",
            "not_applicable",
            ExecutionStatus::Completed,
            Some(Verdict::Pass),
        ),
        ("missing", "pending", ExecutionStatus::Blocked, None),
        ("unadopted", "unadopted", ExecutionStatus::Blocked, None),
        ("stale", "stale_adoption", ExecutionStatus::Blocked, None),
        ("source", "source_invalid", ExecutionStatus::Blocked, None),
        ("unsupported", "unsupported", ExecutionStatus::Blocked, None),
    ] {
        let mut clause = base.clone();
        let mut check = CheckResult::pending("unit", true, Severity::Error);
        check.complete();
        match scenario {
            "failure" => check.verdict = Some(Verdict::Fail),
            "timeout" => check.block(ExecutionStatus::TimedOut, "deadline"),
            "skipped" => check.skip("No matching sources"),
            "unadopted" => clause.adoption = None,
            "stale" => clause.adoption.as_mut().unwrap().binding_digest = "old".into(),
            "source" => clause.source.content_hash = "old".into(),
            "unsupported" => {
                clause.realization = Realization::Unsupported {
                    reason: "Needs human review".into(),
                    evidence_owner: "reviewer".into(),
                }
            }
            _ => {}
        }
        let checks = if scenario == "missing" {
            vec![]
        } else {
            vec![check]
        };
        let result = evaluate(
            &[clause],
            &bindings,
            &snapshot,
            &test_support::policy(),
            &checks,
        )
        .unwrap();
        assert_eq!(result.execution.status, execution, "{scenario}");
        assert_eq!(result.verdict, verdict, "{scenario}");
        assert_eq!(result.matched_entities, 1);
        let evidence = &result.metadata["constraint_coverage"];
        assert_eq!(evidence["clauses"][0]["status"], status, "{scenario}");
        assert_eq!(
            evidence["snapshot"]["content_digest"],
            snapshot.identity.content_digest
        );
        assert_eq!(
            evidence["policy_config_digest"],
            test_support::policy().config_digest
        );
        if scenario == "failure" {
            assert_eq!(result.diagnostics.len(), 1);
            assert_eq!(result.diagnostics[0].file.as_deref(), Some("AGENTS.md"));
        } else {
            assert!(result.diagnostics.is_empty());
        }
    }
    let mut optional = base;
    optional.required = false;
    optional.adoption = None;
    let result = evaluate(
        &[optional],
        &bindings,
        &snapshot,
        &test_support::policy(),
        &[],
    )
    .unwrap();
    assert!(!result.required);
    assert_eq!(result.verdict, Some(Verdict::Pass));
    assert_eq!(
        result.metadata["constraint_coverage"]["clauses"][0]["status"],
        json!("unadopted")
    );
}
