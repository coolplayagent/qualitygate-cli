mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use ed25519_dalek::{Signer, SigningKey};
use qualitygate::{
    adapters::{pilot_acceptance, pilot_authorization},
    domain::{
        CheckResult, Diagnostic, Report, Severity, Summary, evaluate,
        pilot::{Manifest, seal, tool_inventory_digest},
    },
    snapshot::digest,
};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

fn d(value: &str) -> String {
    digest(value.as_bytes())
}

fn full_report(run_id: &str, base: &str, snapshot: &str, environment: &str, pass: bool) -> Report {
    let mut check = CheckResult::pending("task-test", true, Severity::Error);
    if !pass {
        check.diagnostics.push(Diagnostic {
            id: "known-bug".into(),
            fingerprint: "bug-fingerprint".into(),
            file: Some("src/lib.rs".into()),
            range: None,
            message: "retained known issue".into(),
            evidence: json!({"fixture":true}),
            fix: "repair the known issue".into(),
            recheck: Default::default(),
        });
    }
    check.complete();
    serde_json::from_value(json!({
        "schema_version":1,"run_id":run_id,"scope":"task","profile":"full",
        "environment_digest":environment,
        "snapshot":{"mode":"worktree","base":base,"head":"WORKTREE","content_digest":snapshot},
        "policy":{"source":"fixture","config_digest":d("config"),"rules_digest":d("rules"),
            "task_contract_digest":d("task"),"trust":"selected","changes":[]},
        "plan":{"task_id":"task-1","required_checks":["task-test"],
            "pending_delivery_checks":[],"acceptance":{}},
        "gate":evaluate(std::slice::from_ref(&check), &["task-test".into()], &[]),
        "checks":[check],"summary":Summary::default()
    }))
    .unwrap()
}

fn write_report(root: &Path, name: &str, report: &Report) -> Value {
    let bytes = serde_json::to_vec(report).unwrap();
    std::fs::write(root.join(name), &bytes).unwrap();
    json!({"path":name,"digest":digest(&bytes),"bytes":bytes.len()})
}

fn plan(now: u64, root: &Path) -> (Manifest, Vec<Value>) {
    let end = now - 60;
    let start = end - 7 * 86_400;
    let sealed_at = start - 60;
    let base = "a".repeat(40);
    let environment = d("environment");
    let sample = full_report("sample", &base, &d("sample"), &environment, true);
    let tools = tool_inventory_digest(&sample);
    let mut assignments = Vec::new();
    let mut observations = Vec::new();
    for (index, (model, workflow)) in [
        ("medium-model", "existing_tools"),
        ("medium-model", "qualitygate"),
        ("lower-model", "existing_tools"),
        ("lower-model", "qualitygate"),
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("run-{index}");
        let failed_snapshot = d(&format!("{id}-failed"));
        let passed_snapshot = d(&format!("{id}-passed"));
        let failed = full_report(
            &format!("{id}-failed"),
            &base,
            &failed_snapshot,
            &environment,
            false,
        );
        let passed = full_report(
            &format!("{id}-passed"),
            &base,
            &passed_snapshot,
            &environment,
            true,
        );
        let failed_artifact = write_report(root, &format!("{id}-failed.json"), &failed);
        let passed_artifact = write_report(root, &format!("{id}-passed.json"), &passed);
        assignments.push(json!({
            "input_id":"input-1","id":id,"task_id":"task-1","task_kind":"bug_fix","origin":"real",
            "cohort":{"agent_version":"codex-cli","harness_digest":d("harness"),
                "requested_model":model,"actual_model":null,"reasoning_effort":"medium","workflow":workflow,
                "environment_digest":environment,"tools_digest":tools,"cache":"cold","permissions":"workspace-write"},
            "base":base,"initial_snapshot":d("initial"),"config_digest":d("config"),
            "task_digest":d("task"),"required_checks":["task-test"],"expected_issues":["bug"],
            "eligible_repair":true,"exclusion":null
        }));
        let qualitygate = workflow == "qualitygate";
        let elapsed = if qualitygate { 110 } else { 100 };
        let cost = if qualitygate { 80 } else { 100 };
        let attribution = if qualitygate {
            "qualitygate_rule"
        } else {
            "existing_tool"
        };
        observations.push(json!({
            "assignment_id":id,"observed_at":start + 3600,
            "attempts":[
                {"number":1,"status":"completed","elapsed_ms":1000,"check_elapsed_ms":elapsed,
                    "profile":"full","snapshot_digest":failed_snapshot,"report":failed_artifact,
                    "cost":{"currency":"USD","priced_at":start,"source":"fixture",
                        "model_micros":cost,"infrastructure_micros":0},"usage":null},
                {"number":2,"status":"completed","elapsed_ms":1000,"check_elapsed_ms":elapsed,
                    "profile":"full","snapshot_digest":passed_snapshot,"report":passed_artifact,
                    "cost":{"currency":"USD","priced_at":start,"source":"fixture",
                        "model_micros":cost,"infrastructure_micros":0},"usage":null}
            ],
            "findings":[{"diagnostic_id":"known-bug","issue_id":"bug",
                "report_digest":failed_artifact["digest"],"check_id":"task-test",
                "fingerprint":"bug-fingerprint","attribution":attribution,
                "review":{"actor":{"id":"pilot-reviewer","kind":"human"},"label":"confirmed"}}],
            "review_active_ms":if qualitygate {800} else {1000},
            "review_comments":0,"rework_rounds":0
        }));
    }
    let value = json!({
        "schema_version":2,"id":"completed-pilot","protocol":{
            "project":"owner/qualitygate-cli","sampling":"fixed fixture","owner":"pilot-owner",
            "reviewer":"pilot-reviewer","archive":"durable://pilot","sealed_at":sealed_at,
            "start_at":start,"end_at":end,"task_count":1,"days":7,"max_attempts":3,
            "max_seconds":1800,"review_fraction_min":1.0,"monetary_cap":null,
            "budget":{"currency":"USD","priced_at":start,"source":"fixture tariff",
                "max_total_micros":10000,"human_hourly_micros":3600000},
            "thresholds":{"detection_min":0.9,"false_positive_max":0.05,"repair_min":0.8,
                "completion_min":0.95,"review_reduction_min":0.1,"full_p95_ratio_max":1.2,
                "cost_ratio_max":1.0}},
        "assignments":assignments,"observations":[]
    });
    let sealed = seal(serde_json::from_value(value).unwrap(), sealed_at).unwrap();
    (sealed, observations)
}

fn publish(path: &Path, key_id: &str, key: &SigningKey, payload_type: &str, record: &Value) {
    let payload = serde_json::to_vec(record).unwrap();
    let signature = key.sign(&qualitygate::adapters::attestation::pae(
        payload_type,
        &payload,
    ));
    std::fs::write(
        path,
        serde_json::to_vec(&json!({
            "payloadType":payload_type,"payload":STANDARD.encode(payload),
            "signatures":[{"keyid":key_id,"sig":STANDARD.encode(signature.to_bytes())}]
        }))
        .unwrap(),
    )
    .unwrap();
}

fn write(root: &Path, name: &str, value: &impl serde::Serialize) {
    std::fs::write(root.join(name), serde_json::to_vec(value).unwrap()).unwrap();
}

#[test]
fn cli_binds_thresholds_and_reports_authenticated_reviewer_decisions() {
    let repo = fixture();
    let root = repo.path();
    let external = tempfile::tempdir().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let (mut manifest, observations) = plan(now, root);
    write(root, "sealed.json", &manifest);
    let authorization_subject = report(
        &cli(
            root,
            &[
                "pilot",
                "authorization-subject",
                "--input",
                "sealed.json",
                "--format",
                "json",
            ],
        ),
        0,
    );

    let owner = SigningKey::from_bytes(&[81; 32]);
    let reviewer = SigningKey::from_bytes(&[82; 32]);
    write(
        external.path(),
        "trust.json",
        &json!({
            "schema_version":1,"repository":"owner/qualitygate-cli","max_age_seconds":1_000_000,
            "keys":[
                {"id":"pilot-owner","public_key":STANDARD.encode(owner.verifying_key().as_bytes()),
                    "checks":[pilot_authorization::CHECK_SCOPE],"allow_repository_checks":true},
                {"id":"pilot-reviewer","public_key":STANDARD.encode(reviewer.verifying_key().as_bytes()),
                    "checks":[pilot_acceptance::CHECK_SCOPE],"allow_repository_checks":true}
            ],"revoked_records":[]
        }),
    );
    let start = manifest.protocol.start_at.unwrap();
    publish(
        &external.path().join("authorization.json"),
        "pilot-owner",
        &owner,
        pilot_authorization::PAYLOAD_TYPE,
        &json!({
            "schema_version":1,"record_id":"pilot-start","subject":authorization_subject["subject"],
            "authorizer":{"id":"pilot-owner","kind":"human"},"reason":"authorize start",
            "issued_at":start - 1,"expires_at":now + 86_400
        }),
    );
    for assignment in &mut manifest.assignments {
        assignment.cohort.actual_model = Some(assignment.cohort.requested_model.clone());
    }
    manifest.observations = observations
        .into_iter()
        .map(|value| serde_json::from_value(value).unwrap())
        .collect();
    write(root, "observations.json", &manifest);
    let trust = external.path().join("trust.json");
    let authorization = external.path().join("authorization.json");
    let subject = report(
        &cli(
            root,
            &[
                "pilot",
                "acceptance-subject",
                "--input",
                "observations.json",
                "--trust-store",
                trust.to_str().unwrap(),
                "--authorization",
                authorization.to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(subject["subject"]["assessment"]["outcome"], "met");
    assert_eq!(
        subject["subject"]["assessment"]["checks"]
            .as_array()
            .unwrap()
            .len(),
        10
    );

    let accepted = json!({
        "schema_version":1,"record_id":"pilot-review","subject":subject["subject"],
        "reviewer":{"id":"pilot-reviewer","kind":"human"},"decision":"accepted",
        "reason":"all retained thresholds met","issued_at":now,"expires_at":now + 3600
    });
    let acceptance = external.path().join("acceptance.json");
    publish(
        &acceptance,
        "pilot-reviewer",
        &reviewer,
        pilot_acceptance::PAYLOAD_TYPE,
        &accepted,
    );
    let accepted_summary = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "observations.json",
                "--trust-store",
                trust.to_str().unwrap(),
                "--authorization",
                authorization.to_str().unwrap(),
                "--acceptance",
                acceptance.to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(accepted_summary["trial_acceptance"], "accepted");
    assert_eq!(
        accepted_summary["authority"],
        "authenticated_external_review"
    );
    assert_eq!(
        accepted_summary["trial_acceptance_evidence"]["evidence"]["signer_key_id"],
        "pilot-reviewer"
    );

    let mut rejected = accepted.clone();
    rejected["decision"] = json!("rejected");
    rejected["reason"] = json!("independent reviewer declined");
    publish(
        &acceptance,
        "pilot-reviewer",
        &reviewer,
        pilot_acceptance::PAYLOAD_TYPE,
        &rejected,
    );
    let rejected_summary = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "observations.json",
                "--trust-store",
                trust.to_str().unwrap(),
                "--authorization",
                authorization.to_str().unwrap(),
                "--acceptance",
                acceptance.to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        1,
    );
    assert_eq!(rejected_summary["trial_acceptance"], "rejected");

    // Keep each workflow's cost ratio unchanged while total pilot spend exceeds
    // the predeclared cap. A signed acceptance must still fail closed.
    for observation in &mut manifest.observations {
        for attempt in &mut observation.attempts {
            let cost = attempt.cost.as_mut().unwrap();
            cost.model_micros = cost.model_micros.map(|value| value * 100);
        }
    }
    write(root, "observations.json", &manifest);
    let over_budget_subject = report(
        &cli(
            root,
            &[
                "pilot",
                "acceptance-subject",
                "--input",
                "observations.json",
                "--trust-store",
                trust.to_str().unwrap(),
                "--authorization",
                authorization.to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(
        over_budget_subject["subject"]["assessment"]["outcome"],
        "not_met"
    );
    assert_eq!(
        over_budget_subject["subject"]["assessment"]["checks"][9]["metric"],
        "total_cost_micros"
    );
    assert_eq!(
        over_budget_subject["subject"]["assessment"]["checks"][8]["status"],
        "met"
    );
    let mut invalid_acceptance = accepted;
    invalid_acceptance["subject"] = over_budget_subject["subject"].clone();
    publish(
        &acceptance,
        "pilot-reviewer",
        &reviewer,
        pilot_acceptance::PAYLOAD_TYPE,
        &invalid_acceptance,
    );
    let refused = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "observations.json",
                "--trust-store",
                trust.to_str().unwrap(),
                "--authorization",
                authorization.to_str().unwrap(),
                "--acceptance",
                acceptance.to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        refused
            .to_string()
            .contains("failed or unknown required thresholds")
    );
}

#[test]
fn cli_rejects_an_acceptance_after_observation_evidence_changes() {
    let repo = fixture();
    let root = repo.path();
    let external = tempfile::tempdir().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let (mut manifest, observations) = plan(now, root);
    write(root, "sealed.json", &manifest);
    let authorization_subject = report(
        &cli(
            root,
            &[
                "pilot",
                "authorization-subject",
                "--input",
                "sealed.json",
                "--format",
                "json",
            ],
        ),
        0,
    );
    let owner = SigningKey::from_bytes(&[83; 32]);
    let reviewer = SigningKey::from_bytes(&[84; 32]);
    write(
        external.path(),
        "trust.json",
        &json!({
            "schema_version":1,"repository":"owner/qualitygate-cli","max_age_seconds":1_000_000,
            "keys":[
                {"id":"pilot-owner","public_key":STANDARD.encode(owner.verifying_key().as_bytes()),
                    "checks":[pilot_authorization::CHECK_SCOPE],"allow_repository_checks":true},
                {"id":"pilot-reviewer","public_key":STANDARD.encode(reviewer.verifying_key().as_bytes()),
                    "checks":[pilot_acceptance::CHECK_SCOPE],"allow_repository_checks":true}
            ],"revoked_records":[]
        }),
    );
    let start = manifest.protocol.start_at.unwrap();
    publish(
        &external.path().join("authorization.json"),
        "pilot-owner",
        &owner,
        pilot_authorization::PAYLOAD_TYPE,
        &json!({
            "schema_version":1,"record_id":"pilot-start","subject":authorization_subject["subject"],
            "authorizer":{"id":"pilot-owner","kind":"human"},"reason":"authorize start",
            "issued_at":start - 1,"expires_at":now + 86_400
        }),
    );
    for assignment in &mut manifest.assignments {
        assignment.cohort.actual_model = Some(assignment.cohort.requested_model.clone());
    }
    manifest.observations = observations
        .into_iter()
        .map(|value| serde_json::from_value(value).unwrap())
        .collect();
    write(root, "observations.json", &manifest);
    let trust = external.path().join("trust.json");
    let authorization = external.path().join("authorization.json");
    let subject = report(
        &cli(
            root,
            &[
                "pilot",
                "acceptance-subject",
                "--input",
                "observations.json",
                "--trust-store",
                trust.to_str().unwrap(),
                "--authorization",
                authorization.to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        0,
    );
    let acceptance = external.path().join("acceptance.json");
    publish(
        &acceptance,
        "pilot-reviewer",
        &reviewer,
        pilot_acceptance::PAYLOAD_TYPE,
        &json!({
            "schema_version":1,"record_id":"pilot-review","subject":subject["subject"],
            "reviewer":{"id":"pilot-reviewer","kind":"human"},"decision":"accepted",
            "reason":"retained evidence met thresholds","issued_at":now,"expires_at":now + 3600
        }),
    );
    manifest.observations[0].review_active_ms = Some(999);
    write(root, "observations.json", &manifest);
    let stale = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "observations.json",
                "--trust-store",
                trust.to_str().unwrap(),
                "--authorization",
                authorization.to_str().unwrap(),
                "--acceptance",
                acceptance.to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(stale.to_string().contains("evaluated evidence subject"));
}
