//! Attach clause coverage to the same immutable execution subject as the checks.
use crate::domain::constraint::{self, Coverage, CoverageStatus as Status, Realization};
use crate::{domain::*, snapshot::Snapshot};
use anyhow::Result;

pub(super) fn evaluate(
    constraints: &[constraint::Constraint],
    bindings: &std::collections::BTreeMap<String, String>,
    snapshot: &Snapshot,
    policy: &PolicyEvidence,
    checks: &[CheckResult],
) -> Result<CheckResult> {
    let mut result = CheckResult::pending(
        constraint::CHECK_ID,
        constraints.iter().any(|c| c.required),
        Severity::Error,
    );
    let mut coverage = Vec::new();
    let mut gaps = Vec::new();
    for c in constraints {
        let expected = bindings[&c.id].clone();
        let (status, reasons) = if let Err(error) =
            super::policy::validate_source(&c.source, &snapshot.files)
        {
            (Status::SourceInvalid, vec![format!("{error:#}")])
        } else if let Realization::Unsupported {
            reason,
            evidence_owner,
        } = &c.realization
        {
            (
                Status::Unsupported,
                vec![format!("{reason}; evidence owner: {evidence_owner}")],
            )
        } else if let Some(adoption) = &c.adoption {
            if adoption.binding_digest != expected {
                (
                    Status::StaleAdoption,
                    vec![
                        "Constraint source, mapping or version differs from adoption binding"
                            .into(),
                    ],
                )
            } else if let Realization::Checks { checks: ids, .. } = &c.realization {
                constraint::execution(ids, checks)
            } else {
                unreachable!("unsupported realization handled above")
            }
        } else {
            (
                Status::Unadopted,
                vec!["Mapping has no source-bound adoption record in the selected policy".into()],
            )
        };
        if c.required {
            match status {
                Status::Failed => result.diagnostics.push(Diagnostic {
                    id: c.id.clone(),
                    fingerprint: expected.clone(),
                    file: Some(c.source.document.clone()),
                    range: None,
                    message: format!("Required constraint {} failed", c.id),
                    evidence: serde_json::json!({"reasons":reasons}),
                    fix: "Repair the referenced checks and recheck the final snapshot".into(),
                    recheck: Recheck::default(),
                }),
                Status::Verified | Status::NotApplicable => {}
                _ => gaps.push(format!("Constraint {}: {}", c.id, reasons.join("; "))),
            }
        }
        coverage.push(Coverage {
            constraint: c.clone(),
            expected_binding_digest: expected,
            status,
            reasons,
        });
    }
    result.matched_entities = coverage.len();
    result.metadata.insert("constraint_coverage".into(), serde_json::json!({
        "schema_version":1,
        "snapshot":snapshot.identity,
        "policy_config_digest":policy.config_digest,
        "rules_digest":policy.rules_digest,
        "task_contract_digest":policy.task_contract_digest,
        "policy_trust":policy.trust,
        "clauses":coverage,
        "limits":["Coverage includes only declared clauses; it cannot prove prose extraction completeness.",
            "Mechanism and example references are reviewed declarations, not executed example evidence or independent approval.",
            "Verified means mapped checks passed in this report's scope; it does not expand their semantics or scope."]
    }));
    result.complete();
    if !gaps.is_empty() {
        result.block(ExecutionStatus::Blocked, gaps.join("; "));
    }
    Ok(result)
}
