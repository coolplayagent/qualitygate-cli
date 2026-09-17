mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use ed25519_dalek::{Signer, SigningKey};
use qualitygate::{adapters::pilot_authorization, snapshot::digest};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

fn assignment(id: &str, model: &str, workflow: &str) -> Value {
    json!({
        "input_id":"task-1","id":id,"task_id":"task-1","task_kind":"bug_fix","origin":"real",
        "cohort":{"agent_version":"codex-cli","harness_digest":digest(b"harness"),
            "requested_model":model,"actual_model":null,"reasoning_effort":"medium","workflow":workflow,
            "environment_digest":digest(b"environment"),"tools_digest":digest(b"tools"),
            "cache":"cold","permissions":"workspace-write"},
        "base":"a".repeat(40),"initial_snapshot":digest(b"initial"),
        "config_digest":digest(b"config"),"task_digest":digest(b"task"),
        "required_checks":["task-test"],"expected_issues":[],"eligible_repair":true,"exclusion":null
    })
}

fn plan(now: u64) -> Value {
    json!({
        "schema_version":1,"id":"authorized-pilot","protocol":{
            "project":"owner/qualitygate-cli","sampling":"one controlled task","owner":"pilot-owner",
            "reviewer":"independent-reviewer","archive":"durable://pilot","sealed_at":now,
            "start_at":now + 60,"end_at":now + 60 + 7 * 86_400,"task_count":1,"days":7,
            "max_attempts":3,"max_seconds":1800,"review_fraction_min":1.0,"monetary_cap":"USD 10",
            "thresholds":{"detection_min":0.9,"false_positive_max":0.05,"repair_min":0.8,
                "completion_min":0.95,"review_reduction_min":0.1,"full_p95_ratio_max":1.2,
                "cost_ratio_max":1.0}},
        "assignments":[
            assignment("medium-existing","medium-model","existing_tools"),
            assignment("medium-qualitygate","medium-model","qualitygate"),
            assignment("lower-existing","lower-model","existing_tools"),
            assignment("lower-qualitygate","lower-model","qualitygate")
        ],
        "observations":[]
    })
}

fn write(root: &std::path::Path, name: &str, value: &Value) {
    std::fs::write(root.join(name), serde_json::to_vec(value).unwrap()).unwrap();
}

fn publish(key: &SigningKey, external: &std::path::Path, record: &Value) {
    let payload = serde_json::to_vec(record).unwrap();
    let signature = key.sign(&qualitygate::adapters::attestation::pae(
        pilot_authorization::PAYLOAD_TYPE,
        &payload,
    ));
    std::fs::write(
        external.join("authorization.json"),
        serde_json::to_vec(&json!({
            "payloadType":pilot_authorization::PAYLOAD_TYPE,
            "payload":STANDARD.encode(payload),
            "signatures":[{"keyid":"pilot-owner","sig":STANDARD.encode(signature.to_bytes())}]
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn cli_authenticates_the_owner_and_exact_sealed_start_subject() {
    let repo = fixture();
    let root = repo.path();
    let external = tempfile::tempdir().unwrap();
    let key = SigningKey::from_bytes(&[51; 32]);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    write(root, "plan.json", &plan(now));
    let sealed = report(
        &cli(
            root,
            &["pilot", "seal", "--input", "plan.json", "--format", "json"],
        ),
        0,
    );
    write(root, "sealed.json", &sealed);
    let subject = report(
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
    assert_eq!(subject["payload_type"], pilot_authorization::PAYLOAD_TYPE);
    assert_eq!(subject["subject"]["assignment_count"], 4);
    let mut observed = sealed.clone();
    observed["observations"] = json!([{
        "assignment_id":"medium-existing","observed_at":now + 60,
        "attempts":[],"findings":[],"review_active_ms":null,
        "review_comments":null,"rework_rounds":null
    }]);
    write(root, "observed.json", &observed);
    let refused = report(
        &cli(
            root,
            &[
                "pilot",
                "authorization-subject",
                "--input",
                "observed.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(refused.to_string().contains("requires no observations"));

    let trust = json!({
        "schema_version":1,"repository":"owner/qualitygate-cli","max_age_seconds":700_000,
        "keys":[{"id":"pilot-owner","public_key":STANDARD.encode(key.verifying_key().as_bytes()),
            "checks":[pilot_authorization::CHECK_SCOPE],"allow_repository_checks":true}],
        "revoked_records":[]
    });
    std::fs::write(
        external.path().join("trust.json"),
        serde_json::to_vec(&trust).unwrap(),
    )
    .unwrap();
    let record = json!({
        "schema_version":1,"record_id":"authorized-pilot-start","subject":subject["subject"],
        "authorizer":{"id":"pilot-owner","kind":"human"},"reason":"Approve the sealed pilot",
        "issued_at":now,"expires_at":now + 60 + 7 * 86_400 + 60
    });
    publish(&key, external.path(), &record);

    let authenticated = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed.json",
                "--trust-store",
                external.path().join("trust.json").to_str().unwrap(),
                "--authorization",
                external.path().join("authorization.json").to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(
        authenticated["plan_authorization"]["status"],
        "authenticated"
    );
    assert_eq!(
        authenticated["plan_authorization"]["evidence"]["signer_key_id"],
        "pilot-owner"
    );
    assert!(
        authenticated["plan_authorization"]["trust_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        !authenticated["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str().unwrap().contains("owner authorization"))
    );

    let absent = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(absent["plan_authorization"]["status"], "absent");
}

#[test]
fn cli_rejects_repository_owned_or_foreign_authorization_inputs() {
    let repo = fixture();
    let root = repo.path();
    let external = tempfile::tempdir().unwrap();
    let key = SigningKey::from_bytes(&[52; 32]);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    write(root, "plan.json", &plan(now));
    let sealed = report(
        &cli(
            root,
            &["pilot", "seal", "--input", "plan.json", "--format", "json"],
        ),
        0,
    );
    write(root, "sealed.json", &sealed);
    let subject = report(
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
    let trust = json!({
        "schema_version":1,"repository":"owner/qualitygate-cli","max_age_seconds":700_000,
        "keys":[{"id":"pilot-owner","public_key":STANDARD.encode(key.verifying_key().as_bytes()),
            "checks":[pilot_authorization::CHECK_SCOPE],"allow_repository_checks":true}],
        "revoked_records":[]
    });
    write(root, "trust.json", &trust);
    let mut record = json!({
        "schema_version":1,"record_id":"foreign-start","subject":subject["subject"],
        "authorizer":{"id":"pilot-owner","kind":"human"},"reason":"foreign subject",
        "issued_at":now,"expires_at":now + 60 + 7 * 86_400 + 60
    });
    record["subject"]["pilot_id"] = json!("other-pilot");
    publish(&key, external.path(), &record);
    let failed = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed.json",
                "--trust-store",
                root.join("trust.json").to_str().unwrap(),
                "--authorization",
                external.path().join("authorization.json").to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        failed
            .to_string()
            .contains("outside the checked repository")
    );

    std::fs::write(
        external.path().join("trust.json"),
        serde_json::to_vec(&trust).unwrap(),
    )
    .unwrap();
    let foreign = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed.json",
                "--trust-store",
                external.path().join("trust.json").to_str().unwrap(),
                "--authorization",
                external.path().join("authorization.json").to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(foreign.to_string().contains("sealed plan subject"));
}
