use super::*;
use crate::domain::{Artifact, CheckResult, PolicyEvidence, Severity, SnapshotIdentity, Verdict};
use serde_json::json;

fn digest(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}
fn fixture() -> (Projection, Receipt, Report, Delivery) {
    let snapshot: SnapshotIdentity =
        serde_json::from_value(json!({"mode":"worktree","base":"base","head":"head",
        "content_digest":digest('a'),"verification_digest":digest('b')}))
        .unwrap();
    let policy: PolicyEvidence =
        serde_json::from_value(json!({"source":"policy","resolved_commit":"head",
        "config_digest":digest('c'),"rules_digest":digest('d'),"task_contract_digest":digest('e'),
        "task_contract_source":"task.yaml","trust":"caller_supplied_ref","changes":[]}))
        .unwrap();
    let obligation: Obligation = serde_json::from_value(json!({"schema_version":1,"id":"delivery","version":1,
        "source":{"document":"AGENTS.md","section":"Delivery","content_hash":digest('f')},"required":true,
        "scope":{"roles":[],"tasks":[]},"triggers":["before_delivery"],"must":["Deliver verified content"],"must_not":[],
        "enforcement":{"kind":"host","control":"immutable_delivery"}})).unwrap();
    let projection = Projection {
        schema_version: 1,
        kind: "agent_contract_projection".into(),
        repository: "fixture".into(),
        role: "implementer".into(),
        task_id: Some("task".into()),
        snapshot: snapshot.clone(),
        policy: policy.clone(),
        contract_digest: digest('0'),
        prompt: "fixture instructions".into(),
        prompt_digest: digest('1'),
        ready: true,
        obligations: vec![BoundObligation {
            obligation,
            binding_digest: digest('2'),
            status: Status::Ready,
            reasons: vec![],
        }],
        gaps: vec![],
        known_limits: vec![],
    };
    let receipt = Receipt {
        session: "session".into(),
        attempt: 1,
        event: Event::Start,
        contract_digest: projection.contract_digest.clone(),
        prompt_digest: projection.prompt_digest.clone(),
        input_digest: digest('3'),
        role: projection.role.clone(),
        task_id: projection.task_id.clone(),
    };
    let mut check = CheckResult::pending("check", true, Severity::Error);
    check.complete();
    let report: Report = serde_json::from_value(json!({"schema_version":1,"run_id":"run","scope":"task","profile":"full",
        "snapshot":snapshot,"policy":policy,"plan":{"task_id":"task","required_checks":["check"],"pending_delivery_checks":[],"acceptance":{}},
        "gate":{"complete":true,"decision":"pass","blockers":[]},"checks":[check],"summary":crate::domain::Summary::default()})).unwrap();
    let delivery = Delivery {
        schema_version: 1,
        format: "qualitygate-snapshot-zip-v1".into(),
        snapshot,
        artifact: Artifact {
            path: "object.zip".into(),
            digest: digest('4'),
            bytes: 100,
        },
        files: 1,
    };
    (projection, receipt, report, delivery)
}

#[test]
fn delivery_requires_current_receipt_policy_scope_snapshot_and_recomputed_complete_checks() {
    let (projection, receipt, report, delivery) = fixture();
    assert!(authorize(&projection, &projection, &receipt, &report, &delivery).allowed);
    for case in 0..12 {
        let (mut current, mut receipt, mut report, mut delivery) = fixture();
        match case {
            0 => receipt.prompt_digest = digest('5'),
            1 => current.contract_digest = digest('5'),
            2 => current.snapshot.content_digest = digest('5'),
            3 => delivery.snapshot.verification_digest = None,
            4 => report.scope = "path".into(),
            5 => report.policy.changes.push("qualitygate.yaml".into()),
            6 => report.checks[0].block(crate::domain::ExecutionStatus::TimedOut, "timeout"),
            7 => report.schema_version = 99,
            8 => report.plan.pending_delivery_checks.push("missing".into()),
            9 => report.policy.rules_digest = digest('5'),
            10 => receipt.input_digest.clear(),
            11 => delivery.artifact.bytes = 0,
            _ => unreachable!(),
        }
        let result = authorize(&current, &projection, &receipt, &report, &delivery);
        assert!(!result.allowed, "case {case}");
        assert!(!result.gaps.is_empty());
        assert!(
            result
                .obligations
                .iter()
                .all(|o| o.status != "host_control_satisfied")
        );
    }
}

#[test]
fn unsupported_controls_and_required_warning_failures_cannot_be_silently_downgraded() {
    let (mut projection, receipt, mut report, delivery) = fixture();
    for control in [
        Control::ToolAuthorization,
        Control::FinalResponse,
        Control::InProcessContextRecovery,
    ] {
        projection.obligations[0].obligation.enforcement = Enforcement::Host { control };
        assert!(!preflight(&projection).is_empty());
        assert!(!authorize(&projection, &projection, &receipt, &report, &delivery).allowed);
    }
    projection.obligations[0].obligation.required = false;
    let optional = authorize(&projection, &projection, &receipt, &report, &delivery);
    assert!(optional.allowed);
    assert_eq!(optional.obligations[0].status, "unsupported_host_control");
    projection.obligations[0].obligation.required = true;
    projection.obligations[0].obligation.enforcement = Enforcement::Checks {
        checks: vec!["check".into()],
    };
    report.checks[0].severity = Severity::Warning;
    report.checks[0].verdict = Some(Verdict::Fail);
    assert!(!authorize(&projection, &projection, &receipt, &report, &delivery).allowed);
    report.checks[0].complete();
    projection.obligations[0].obligation.enforcement = Enforcement::Instruction;
    let instruction = authorize(&projection, &projection, &receipt, &report, &delivery);
    assert!(instruction.allowed);
    assert_eq!(instruction.obligations[0].status, "delivered_unverified");
    projection.obligations[0].obligation.enforcement = Enforcement::Host {
        control: Control::PolicyIntegrity,
    };
    projection.obligations[0]
        .obligation
        .triggers
        .push(Event::BeforeTool);
    assert!(!preflight(&projection).is_empty());
}
