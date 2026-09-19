#![cfg(unix)]

mod common;

use common::{cli, fixture, report};
use qualitygate::{
    domain::{
        Artifact, Report,
        judgment::{
            CalibrationRecord, CalibrationStatus, JudgmentMode, JudgmentPolicy, Question,
            RiskCoveragePoint, provider_binding,
        },
    },
    snapshot,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

fn question() -> Question {
    let mut question = Question {
        id: "qg.warning.validity".into(),
        version: 1,
        text: "Is this warning valid after independent review?".into(),
        criteria: vec!["Inspect the retained diagnostic and its code context".into()],
        choices: vec!["valid".into(), "invalid".into()],
        positive_choice: "valid".into(),
        target_event: "finding_valid_after_independent_review".into(),
        source_digest: String::new(),
    };
    question.source_digest = question.digest().unwrap();
    question
}

fn warning_report(root: &Path) -> (Report, String) {
    fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nchecks:\n  - {id: warning-review, argv: [sh, -c, 'exit 1'], severity: warning}\n").unwrap();
    let output = report(&cli(root, &["check", "--worktree", "--format", "json"]), 0);
    assert_eq!(output["gate"]["decision"], "pass");
    let path = output["context"]["report_path"]
        .as_str()
        .unwrap()
        .to_owned();
    let report = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    (report, path)
}

fn evidence_id(report: &Report, path: &str) -> String {
    let bytes = fs::read(path).unwrap();
    let diagnostic = &report.checks[0].diagnostics[0];
    snapshot::digest(
        &serde_json::to_vec(&(
            &snapshot::digest(&bytes),
            &report.checks[0].id,
            &diagnostic.fingerprint,
            &diagnostic.evidence,
        ))
        .unwrap(),
    )
}

fn provider_script(root: &Path, response: &Value) -> Vec<u8> {
    let body = format!(
        "if [ \"$1\" = '--version' ]; then printf 'fixture-v1\\n'; else cat >/dev/null; printf '%s' '{}'; fi\n",
        response
    );
    fs::write(root.join("provider.sh"), &body).unwrap();
    body.into_bytes()
}

fn policy(root: &Path, script: &[u8], mode: JudgmentMode, model_digest: &str) -> JudgmentPolicy {
    let canonical_root = root.canonicalize().unwrap();
    let executable_bytes = fs::read("/bin/sh").unwrap();
    let executable = Artifact {
        path: "/bin/sh".into(),
        digest: snapshot::digest(&executable_bytes),
        bytes: executable_bytes.len() as u64,
    };
    let input = Artifact {
        path: canonical_root.join("provider.sh").display().to_string(),
        digest: snapshot::digest(script),
        bytes: script.len() as u64,
    };
    let question = question();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    JudgmentPolicy {
        schema_version: 1,
        mode,
        argv: vec!["/bin/sh".into(), "provider.sh".into()],
        version_argv: vec!["/bin/sh".into(), "provider.sh".into(), "--version".into()],
        provider_inputs: vec!["provider.sh".into()],
        timeout_seconds: 5,
        calibration: Some(CalibrationRecord {
            id: "cal-v1".into(),
            declared_status: CalibrationStatus::Valid,
            provider_digest: provider_binding(
                &executable,
                &snapshot::digest(b"fixture-v1\n"),
                &[input],
            )
            .unwrap(),
            model_digest: model_digest.into(),
            calibrator_digest: snapshot::digest(b"calibrator-v1"),
            label_source_digest: snapshot::digest(b"independent-labels-v1"),
            question_digest: question.source_digest.clone(),
            target_event: question.target_event.clone(),
            repository_digest: snapshot::digest(canonical_root.to_string_lossy().as_bytes()),
            language: "unknown".into(),
            rule_id: "warning-review".into(),
            labeled_from_unix: now - 1000,
            labeled_until_unix: now - 100,
            expires_unix: now + 1000,
            revoked: false,
            sample_count: 40,
            independent_label_count: 40,
            brier_score: 0.11,
            log_loss: 0.32,
            near_threshold_error: 0.08,
            risk_coverage: vec![RiskCoveragePoint {
                coverage: 0.8,
                risk: 0.1,
            }],
            known_limits: vec!["Local fixture only".into()],
        }),
        question,
        review_priority_threshold: Some(0.7),
    }
}

fn run(root: &Path, report_path: &str, expected: i32) -> Value {
    report(
        &cli(
            root,
            &[
                "judgment",
                "run",
                "--report",
                report_path,
                "--policy",
                "judgment.yaml",
                "--output-dir",
                "judgment-artifacts",
                "--format",
                "json",
            ],
        ),
        expected,
    )
}

#[test]
fn advisory_provider_retains_original_gate_and_binds_calibrated_warning_evidence() {
    let repository = fixture();
    let root = repository.path();
    let (report_data, report_path) = warning_report(root);
    let evidence = evidence_id(&report_data, &report_path);
    let model = snapshot::digest(b"model-v1");
    let response = json!({"kind":"probabilistic","question_id":"qg.warning.validity","primitive":"choice","selected":"valid","distribution":{"valid":0.8,"invalid":0.2},"target_event":"finding_valid_after_independent_review","applicability":"in_domain","calibration_ref":"cal-v1","model_digest":model,"evidence_refs":[evidence]});
    let script = provider_script(root, &response);
    let policy = policy(root, &script, JudgmentMode::Advisory, &model);
    let mut question = policy.question.clone();
    question.source_digest.clear();
    fs::write(
        root.join("question.yaml"),
        serde_norway::to_string(&question).unwrap(),
    )
    .unwrap();
    let computed = report(
        &cli(
            root,
            &[
                "judgment",
                "question-digest",
                "--input",
                "question.yaml",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(computed["source_digest"], policy.question.source_digest);
    fs::write(
        root.join("judgment.yaml"),
        serde_norway::to_string(&policy).unwrap(),
    )
    .unwrap();
    let result = run(root, &report_path, 0);
    assert_eq!(result["complete"], true);
    assert_eq!(result["route"], "prioritize_review");
    assert_eq!(result["original_gate"]["decision"], "pass");
    assert_eq!(result["calibration_status"], "valid");
    assert_eq!(result["audit"]["automatic_gate_change"], false);
    for item in result["artifacts"].as_array().unwrap() {
        let bytes = fs::read(item["path"].as_str().unwrap()).unwrap();
        assert_eq!(snapshot::digest(&bytes), item["digest"]);
    }
    let decision = root
        .join("judgment-artifacts")
        .join(
            result["decision_id"]
                .as_str()
                .unwrap()
                .trim_start_matches("sha256:"),
        )
        .join("decision.json");
    assert!(decision.is_file());
    let persisted: Value = serde_json::from_slice(&fs::read(decision).unwrap()).unwrap();
    assert_eq!(persisted["evidence_id"], result["evidence_id"]);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut labels = json!({"schema_version":1,"repository_digest":result["repository_digest"],"question_digest":result["question_digest"],"provider_binding_digest":result["provider_binding_digest"],"model_digest":model,"calibrator_digest":policy.calibration.as_ref().unwrap().calibrator_digest,"label_source_digest":policy.calibration.as_ref().unwrap().label_source_digest,"target_event":"finding_valid_after_independent_review","validation_from_unix":now-10,"validation_until_unix":now+1000,"audit_fraction":1.0,"labels":[{"decision_id":result["decision_id"],"finding_valid":true,"reviewer":"independent-reviewer","source_digest":snapshot::digest(b"independent-label"),"reviewed_unix":now,"independent":true}]});
    fs::write(
        root.join("labels.json"),
        serde_json::to_vec(&labels).unwrap(),
    )
    .unwrap();
    let pilot = report(
        &cli(
            root,
            &[
                "judgment",
                "pilot",
                "--runs",
                "judgment-artifacts",
                "--labels",
                "labels.json",
                "--audit-seed",
                "independent-seed-123",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(pilot["complete"], true);
    assert_eq!(pilot["counts"]["manual_review_recommendations"], 1);
    assert_eq!(pilot["counts"]["probabilistic_labeled"], 1);
    assert!((pilot["calibration"]["brier_score"].as_f64().unwrap() - 0.04).abs() < 0.000001);
    assert_eq!(
        pilot["audit"]["sampled_decision_ids"][0],
        result["decision_id"]
    );
    let wrapped = report(
        &cli(
            root,
            &[
                "judgment",
                "pilot",
                "--runs",
                "judgment-artifacts",
                "--labels",
                "labels.json",
                "--audit-seed",
                "independent-seed-123",
                "--envelope",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(wrapped["command_kind"], "pilot");
    assert_eq!(wrapped["payload"]["kind"], "judgment_pilot");
    qualitygate::domain::decision_envelope::DecisionEnvelope::parse(
        &serde_json::to_vec(&wrapped).unwrap(),
    )
    .unwrap();
    labels["labels"] = json!([]);
    fs::write(
        root.join("labels.json"),
        serde_json::to_vec(&labels).unwrap(),
    )
    .unwrap();
    let incomplete = report(
        &cli(
            root,
            &[
                "judgment",
                "pilot",
                "--runs",
                "judgment-artifacts",
                "--labels",
                "labels.json",
                "--audit-seed",
                "independent-seed-123",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(incomplete["complete"], false);
    assert_eq!(incomplete["audit"]["missing_labels"], 1);
    assert!(incomplete["calibration"]["brier_score"].is_null());
    let wrapped_gap = report(
        &cli(
            root,
            &[
                "judgment",
                "pilot",
                "--runs",
                "judgment-artifacts",
                "--labels",
                "labels.json",
                "--audit-seed",
                "independent-seed-123",
                "--envelope",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(wrapped_gap["gate"]["outcome"], "incomplete");
    assert_eq!(wrapped_gap["gate"]["route"], "inspect_gap");
}

#[test]
fn malformed_distribution_stale_calibration_and_timeout_are_typed_gaps() {
    let repository = fixture();
    let root = repository.path();
    let (report_data, report_path) = warning_report(root);
    let evidence = evidence_id(&report_data, &report_path);
    let model = snapshot::digest(b"model-v1");
    let malformed = json!({"kind":"probabilistic","question_id":"qg.warning.validity","primitive":"choice","selected":"valid","distribution":{"valid":0.9,"invalid":0.2},"target_event":"finding_valid_after_independent_review","applicability":"in_domain","calibration_ref":"cal-v1","model_digest":model,"evidence_refs":[evidence]});
    let script = provider_script(root, &malformed);
    let initial_policy = policy(root, &script, JudgmentMode::Shadow, &model);
    fs::write(
        root.join("judgment.yaml"),
        serde_norway::to_string(&initial_policy).unwrap(),
    )
    .unwrap();
    let invalid = run(root, &report_path, 2);
    assert_eq!(invalid["assessment"]["kind"], "execution_gap");
    assert_eq!(invalid["original_gate"]["decision"], "pass");
    let repaired = json!({"kind":"probabilistic","question_id":"qg.warning.validity","primitive":"choice","selected":"valid","distribution":{"valid":0.8,"invalid":0.2},"target_event":"finding_valid_after_independent_review","applicability":"in_domain","calibration_ref":"cal-v1","model_digest":model,"evidence_refs":[evidence]});
    provider_script(root, &repaired);
    let stale = run(root, &report_path, 2);
    assert!(
        stale["assessment"]["reason"]
            .as_str()
            .unwrap()
            .contains("OutOfDomain")
    );
    let mut expired_policy = policy(
        root,
        &fs::read(root.join("provider.sh")).unwrap(),
        JudgmentMode::Shadow,
        &model,
    );
    expired_policy.calibration.as_mut().unwrap().expires_unix = 1;
    fs::write(
        root.join("judgment.yaml"),
        serde_norway::to_string(&expired_policy).unwrap(),
    )
    .unwrap();
    let expired = run(root, &report_path, 2);
    assert!(
        expired["assessment"]["reason"]
            .as_str()
            .unwrap()
            .contains("Stale")
    );
    fs::write(
        root.join("provider.sh"),
        "if [ \"$1\" = '--version' ]; then printf 'fixture-v1\\n'; else sleep 2; fi\n",
    )
    .unwrap();
    let mut timeout_policy = policy(
        root,
        &fs::read(root.join("provider.sh")).unwrap(),
        JudgmentMode::Shadow,
        &model,
    );
    timeout_policy.timeout_seconds = 1;
    fs::write(
        root.join("judgment.yaml"),
        serde_norway::to_string(&timeout_policy).unwrap(),
    )
    .unwrap();
    let timed_out = run(root, &report_path, 2);
    assert!(
        timed_out["assessment"]["reason"]
            .as_str()
            .unwrap()
            .contains("timed out")
    );
    let mut missing_policy = timeout_policy;
    missing_policy.argv[0] = "/qualitygate/missing/provider".into();
    missing_policy.version_argv[0] = "/qualitygate/missing/provider".into();
    missing_policy.provider_inputs.clear();
    fs::write(
        root.join("judgment.yaml"),
        serde_norway::to_string(&missing_policy).unwrap(),
    )
    .unwrap();
    let missing = run(root, &report_path, 2);
    assert_eq!(missing["assessment"]["kind"], "execution_gap");
    assert_eq!(missing["original_gate"]["decision"], "pass");
}

#[test]
fn abstention_is_complete_but_cannot_change_the_gate_or_claim_probability() {
    let repository = fixture();
    let root = repository.path();
    let (report_data, report_path) = warning_report(root);
    let evidence = evidence_id(&report_data, &report_path);
    let response = json!({"kind":"abstained","question_id":"qg.warning.validity","reason":"insufficient_evidence","evidence_refs":[evidence]});
    let script = provider_script(root, &response);
    let policy = policy(
        root,
        &script,
        JudgmentMode::Shadow,
        &snapshot::digest(b"model-v1"),
    );
    fs::write(
        root.join("judgment.yaml"),
        serde_norway::to_string(&policy).unwrap(),
    )
    .unwrap();
    let outcome = run(root, &report_path, 0);
    assert_eq!(outcome["assessment"]["kind"], "abstained");
    assert_eq!(outcome["complete"], true);
    assert_eq!(outcome["route"], "shadow_only");
    assert_eq!(outcome["original_gate"]["decision"], "pass");
    assert!(outcome["calibration_status"].is_null());
    let mut incomplete = serde_json::to_value(report_data).unwrap();
    incomplete["gate"]["complete"] = json!(false);
    incomplete["gate"]["decision"] = json!("incomplete");
    fs::write(&report_path, serde_json::to_vec(&incomplete).unwrap()).unwrap();
    let rejected = cli(
        root,
        &[
            "judgment",
            "run",
            "--report",
            &report_path,
            "--policy",
            "judgment.yaml",
            "--output-dir",
            "judgment-artifacts",
            "--format",
            "json",
        ],
    );
    assert_eq!(rejected.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&rejected.stdout).contains("execution is incomplete"),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&rejected.stdout),
        String::from_utf8_lossy(&rejected.stderr)
    );
}
