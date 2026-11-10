use super::*;
use crate::domain::pilot::Manifest;
use base64::Engine;
use ed25519_dalek::Signer;
use serde_json::json;

fn plan(now: u64) -> Manifest {
    let d = crate::snapshot::digest(b"unit identity");
    let mut manifest: Value = serde_json::from_slice(br#"{"schema_version":1,"id":"qualitygate-eight-task-seven-day-pilot","protocol":{"project":"qualitygate-cli","sampling":"Before execution select four bug fixes and four refactors; assign each to both model groups and both workflows","owner":null,"reviewer":null,"archive":null,"sealed_at":null,"start_at":null,"end_at":null,"task_count":8,"days":7,"max_attempts":3,"max_seconds":1800,"review_fraction_min":1.0,"monetary_cap":null,"thresholds":{"detection_min":0.9,"false_positive_max":0.05,"repair_min":0.8,"completion_min":0.95,"review_reduction_min":0.1,"full_p95_ratio_max":1.2,"cost_ratio_max":1.0}},"assignments":[],"observations":[]}"#)
    .unwrap();
    manifest["id"] = json!("unit-pilot");
    for (key, value) in [
        ("project", json!("unit/repository")),
        ("owner", json!("owner")),
        ("reviewer", json!("reviewer")),
        ("archive", json!("unit retained archive")),
        ("sealed_at", json!(now)),
        ("start_at", json!(now + 60)),
        ("end_at", json!(now + 60 + 604800)),
        ("task_count", json!(1)),
        ("monetary_cap", json!("USD 5")),
    ] {
        manifest["protocol"][key] = value;
    }
    let assignments: Vec<_> = (0..4).map(|index| json!({"id":format!("run-{index}"),"input_id":"input","task_id":"task","task_kind":"bug_fix","origin":"real",
        "cohort":{"agent_version":"unit-agent","harness_digest":d,"requested_model":if index < 2 {"model-a"} else {"model-b"},
            "actual_model":null,"reasoning_effort":"medium","workflow":if index%2 == 0 {"existing_tools"} else {"qualitygate"},
            "environment_digest":d,"tools_digest":d,"cache":"cold","permissions":"read-only"},
        "base":"a".repeat(40),"initial_snapshot":d,"config_digest":d,"task_digest":d,
        "required_checks":["behavior"],"expected_issues":[],"eligible_repair":true,"exclusion":null})).collect();
    manifest["assignments"] = json!(assignments);
    serde_json::from_value(manifest).unwrap()
}

fn sign(path: &Path, payload_type: &str, record: &Value) {
    let key = ed25519_dalek::SigningKey::from_bytes(&[8; 32]);
    let payload = serde_json::to_vec(record).unwrap();
    let signature = key.sign(&crate::adapters::attestation::pae(payload_type, &payload));
    let base64 = base64::engine::general_purpose::STANDARD;
    std::fs::write(
        path,
        serde_json::to_vec(
            &json!({"payloadType":payload_type,"payload":base64.encode(payload),
        "signatures":[{"keyid":"owner","sig":base64.encode(signature.to_bytes())}]}),
        )
        .unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn sealing_is_read_only_and_external_owner_authority_remains_distinct_from_completion() {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let now = crate::config::policy_store::now().unwrap();
    let path = root.path().join("plan.json");
    let original = serde_json::to_vec(&plan(now)).unwrap();
    std::fs::write(&path, &original).unwrap();
    let (sealed, code) = seal(path.clone()).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(
        sealed["plan_seal"]["digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    std::fs::write(&path, serde_json::to_vec(&sealed).unwrap()).unwrap();
    let (request, code) = authorization_subject(path.clone()).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(request["subject"]["assignment_count"], 4);
    let key = ed25519_dalek::SigningKey::from_bytes(&[8; 32]);
    let trust = external.path().join("trust.json");
    std::fs::write(&trust, serde_json::to_vec(&json!({"schema_version":1,"repository":"unit/repository","max_age_seconds":31536000,
        "keys":[{"id":"owner","public_key":base64::engine::general_purpose::STANDARD.encode(key.verifying_key().as_bytes()),
        "checks":["pilot-plan-authorization"],"allow_repository_checks":true}]})).unwrap()).unwrap();
    let record = external.path().join("authorization.json");
    sign(
        &record,
        request["payload_type"].as_str().unwrap(),
        &json!({"schema_version":1,"record_id":"start-1","subject":request["subject"],
        "authorizer":{"id":"owner","kind":"human"},"issued_at":now,"expires_at":now+604900,"reason":"Authorize declared pilot only"}),
    );
    let (summary, code) = summarize(
        root.path().into(),
        path.clone(),
        Some(trust.clone()),
        Some(record.clone()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(code, 2);
    assert_eq!(summary["complete"], false);
    assert_eq!(
        summary["plan_authorization"]["trust_source"],
        trust.to_str().unwrap()
    );
    assert_eq!(
        summary["plan_authorization"]["authorization_digest"],
        crate::snapshot::digest(&std::fs::read(&record).unwrap())
    );
    assert!(
        acceptance_subject(
            root.path().into(),
            path.clone(),
            trust.clone(),
            record.clone()
        )
        .await
        .is_err()
    );
    assert!(
        summarize(
            root.path().into(),
            path.clone(),
            Some(trust.clone()),
            None,
            None
        )
        .await
        .is_err()
    );
    assert!(
        summarize(
            root.path().into(),
            path.clone(),
            None,
            None,
            Some(record.clone())
        )
        .await
        .is_err()
    );
    std::fs::write(&record, b"tampered").unwrap();
    assert!(
        summarize(root.path().into(), path, Some(trust), Some(record), None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn complete_archived_trial_still_requires_independent_acceptance_and_detects_report_substitution()
 {
    use crate::domain::{
        CheckResult, Report, Severity, Summary, evaluate,
        pilot::{Observation, tool_inventory_digest},
    };
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let now = crate::config::policy_store::now().unwrap();
    let started = now - 605000;
    let mut manifest = plan(started);
    let mut observations = Vec::new();
    for (index, a) in manifest.assignments.iter_mut().enumerate() {
        let mut check = CheckResult::pending("behavior", true, Severity::Error);
        check.complete();
        let snapshot = crate::snapshot::digest(format!("observed-{index}").as_bytes());
        let report:Report=serde_json::from_value(json!({"schema_version":1,"run_id":format!("run-{index}"),"scope":"task","profile":"full","environment_digest":a.cohort.environment_digest,
            "snapshot":{"mode":"worktree","base":a.base,"head":"WORKTREE","content_digest":snapshot},
            "policy":{"source":"selected","config_digest":a.config_digest,"rules_digest":crate::snapshot::digest(b"rules"),"task_contract_digest":a.task_digest,"trust":"selected","changes":[]},
            "plan":{"task_id":a.task_id,"required_checks":a.required_checks,"pending_delivery_checks":[],"acceptance":{}},"gate":evaluate(&[check.clone()],&a.required_checks,&[]),"checks":[check],"summary":Summary::default()})).unwrap();
        a.cohort.tools_digest = tool_inventory_digest(&report);
        let bytes = serde_json::to_vec(&report).unwrap();
        let name = format!("report-{index}.json");
        std::fs::write(root.path().join(&name), &bytes).unwrap();
        let artifact = crate::domain::Artifact {
            path: name,
            digest: crate::snapshot::digest(&bytes),
            bytes: bytes.len() as u64,
        };
        observations.push(serde_json::from_value::<Observation>(json!({"assignment_id":a.id,"observed_at":started+100,"attempts":[{"number":1,"status":"completed","elapsed_ms":1000,"check_elapsed_ms":100,"profile":"full","snapshot_digest":snapshot,"report":artifact,"cost":{"currency":"USD","priced_at":started,"source":"unit tariff","model_micros":10,"infrastructure_micros":5}}],"findings":[],"review_active_ms":100,"review_comments":0,"rework_rounds":0})).unwrap());
    }
    let mut manifest = crate::domain::pilot::seal(manifest, started).unwrap();
    for a in &mut manifest.assignments {
        a.cohort.actual_model = Some(a.cohort.requested_model.clone());
    }
    let subject = crate::domain::pilot::authorization_subject(&manifest).unwrap();
    manifest.observations = observations;
    let path = root.path().join("trial.json");
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&[8; 32]);
    let reviewer_key = ed25519_dalek::SigningKey::from_bytes(&[9; 32]);
    let trust = outside.path().join("trust.json");
    std::fs::write(&trust,serde_json::to_vec(&json!({"schema_version":1,"repository":"unit/repository","max_age_seconds":31536000,"keys":[
        {"id":"owner","public_key":base64::engine::general_purpose::STANDARD.encode(key.verifying_key().as_bytes()),"checks":["pilot-plan-authorization"],"allow_repository_checks":true},
        {"id":"reviewer","public_key":base64::engine::general_purpose::STANDARD.encode(reviewer_key.verifying_key().as_bytes()),"checks":["pilot-trial-acceptance"],"allow_repository_checks":true}]})).unwrap()).unwrap();
    let authorization = outside.path().join("authorization.json");
    sign(
        &authorization,
        crate::adapters::pilot_authorization::PAYLOAD_TYPE,
        &json!({"schema_version":1,"record_id":"start","subject":subject,"authorizer":{"id":"owner","kind":"human"},"reason":"Start selected observation","issued_at":started,"expires_at":now+300}),
    );
    let (summary, code) = summarize(
        root.path().into(),
        path.clone(),
        Some(trust.clone()),
        Some(authorization.clone()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(code, 0, "{summary}");
    assert_eq!(summary["protocol_ready"], true, "{summary}");
    assert_eq!(summary["trial_acceptance"], "requires_external_review");
    let (request, _) = acceptance_subject(
        root.path().into(),
        path.clone(),
        trust.clone(),
        authorization.clone(),
    )
    .await
    .unwrap();
    let record = json!({"schema_version":1,"record_id":"review","subject":request["subject"],"reviewer":{"id":"reviewer","kind":"human"},"decision":"rejected","reason":"Insufficient demonstrated detection benefit","issued_at":now,"expires_at":now+300});
    let payload = serde_json::to_vec(&record).unwrap();
    let payload_type = crate::adapters::pilot_acceptance::PAYLOAD_TYPE;
    let signature = reviewer_key.sign(&crate::adapters::attestation::pae(payload_type, &payload));
    let base64 = base64::engine::general_purpose::STANDARD;
    let acceptance = outside.path().join("acceptance.json");
    std::fs::write(&acceptance,serde_json::to_vec(&json!({"payloadType":payload_type,"payload":base64.encode(payload),"signatures":[{"keyid":"reviewer","sig":base64.encode(signature.to_bytes())}]})).unwrap()).unwrap();
    let (summary, code) = summarize(
        root.path().into(),
        path.clone(),
        Some(trust.clone()),
        Some(authorization.clone()),
        Some(acceptance),
    )
    .await
    .unwrap();
    assert_eq!(code, 1);
    assert_eq!(summary["trial_acceptance"], "rejected");
    assert!(summary["trial_acceptance_evidence"]["acceptance_digest"].is_string());
    std::fs::write(root.path().join("report-0.json"), b"{}").unwrap();
    let (summary, code) = summarize(
        root.path().into(),
        path,
        Some(trust),
        Some(authorization),
        None,
    )
    .await
    .unwrap();
    assert_eq!(code, 2);
    assert_eq!(summary["complete"], false);
}
