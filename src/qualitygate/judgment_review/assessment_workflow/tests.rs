use super::*;
use crate::domain::judgment::{JudgmentPolicy, Question};
use crate::domain::{CheckResult, Diagnostic, Summary, evaluate};

fn fixture(root: &Path, response: &str) -> (Options, Report) {
    let mut check = CheckResult::pending("review", false, Severity::Warning);
    check.diagnostics.push(Diagnostic {
        id: "review-1".into(),
        fingerprint: "fp-1".into(),
        file: Some("src/lib.rs".into()),
        range: None,
        message: "Review this behavior".into(),
        evidence: json!({"language":"rust"}),
        fix: "Inspect the source".into(),
        recheck: Default::default(),
    });
    check.complete();
    let report: Report = serde_json::from_value(json!({"schema_version":1,"run_id":"run-1","scope":"delivery","profile":"full",
        "snapshot":{"mode":"worktree","base":"base","head":"head","content_digest":snapshot::digest(b"snapshot")},
        "policy":crate::application::test_support::policy(), "plan":{"required_checks":[],"pending_delivery_checks":[],"acceptance":{}},
        "gate":evaluate(&[check.clone()], &[], &[]),"checks":[check],"summary":Summary::default()})).unwrap();
    let bytes = serde_json::to_vec(&report).unwrap();
    std::fs::write(root.join("report.json"), &bytes).unwrap();
    let evidence = snapshot::digest(
        &serde_json::to_vec(&(
            snapshot::digest(&bytes),
            "review",
            "fp-1",
            json!({"language":"rust"}),
        ))
        .unwrap(),
    );
    let response = response.replace("EVIDENCE", &evidence);
    std::fs::write(root.join("response-input.json"), response).unwrap();
    let mut question = Question {
        id: "review".into(),
        version: 1,
        text: "Is the finding valid?".into(),
        criteria: vec!["Independent review".into()],
        choices: vec!["true".into(), "false".into()],
        positive_choice: "true".into(),
        target_event: "finding_valid".into(),
        source_digest: String::new(),
    };
    question.source_digest = question.digest().unwrap();
    let policy = JudgmentPolicy {
        schema_version: 1,
        mode: JudgmentMode::Shadow,
        argv: vec![
            "sh".into(),
            "-c".into(),
            "cat >/dev/null; cat response-input.json".into(),
        ],
        version_argv: vec![
            "sh".into(),
            "-c".into(),
            "printf fixture-provider-v1".into(),
        ],
        provider_inputs: vec!["response-input.json".into()],
        timeout_seconds: 1,
        question,
        calibration: None,
        review_priority_threshold: None,
    };
    std::fs::write(
        root.join("policy.json"),
        serde_json::to_vec(&policy).unwrap(),
    )
    .unwrap();
    (
        Options {
            root: root.into(),
            report: root.join("report.json"),
            policy: root.join("policy.json"),
            output_dir: root.join("decisions"),
            fingerprint: Some("fp-1".into()),
        },
        report,
    )
}

#[cfg(unix)]
#[tokio::test]
async fn completed_judgment_retains_warning_gate_and_all_raw_evidence() {
    for mode in [JudgmentMode::Shadow, JudgmentMode::Advisory] {
        let root = tempfile::tempdir().unwrap();
        let (options, report) = fixture(
            root.path(),
            r#"{"kind":"deterministic","question_id":"review","value":false,"evidence_refs":["EVIDENCE"]}"#,
        );
        let mut policy: JudgmentPolicy =
            serde_json::from_slice(&std::fs::read(&options.policy).unwrap()).unwrap();
        policy.mode = mode;
        std::fs::write(&options.policy, serde_json::to_vec(&policy).unwrap()).unwrap();
        let (result, code) = run(options).await.unwrap();
        assert_eq!(code, 0);
        assert_eq!(result["complete"], true);
        assert_eq!(
            result["original_gate"],
            serde_json::to_value(report.gate).unwrap()
        );
        assert_eq!(
            result["route"],
            if mode == JudgmentMode::Shadow {
                "shadow_only"
            } else {
                "manual_review"
            }
        );
        assert_eq!(result["audit"]["automatic_gate_change"], false);
        for reference in result["artifacts"].as_array().unwrap() {
            let bytes = std::fs::read(reference["path"].as_str().unwrap()).unwrap();
            assert_eq!(reference["digest"], snapshot::digest(&bytes));
            assert_eq!(reference["bytes"], bytes.len());
        }
        assert_eq!(result["artifacts"].as_array().unwrap().len(), 4);
        assert!(
            result["provider_binding_digest"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn provider_failure_timeout_bad_binding_and_mutation_remain_execution_gaps() {
    for case in 0..7 {
        let root = tempfile::tempdir().unwrap();
        let (options, _) = fixture(
            root.path(),
            r#"{"kind":"deterministic","question_id":"review","value":true,"evidence_refs":["wrong"]}"#,
        );
        let mut policy: JudgmentPolicy =
            serde_json::from_slice(&std::fs::read(&options.policy).unwrap()).unwrap();
        match case {
            0 => policy.argv[2] = "cat >/dev/null; printf malformed".into(),
            1 => policy.argv[2] = "cat >/dev/null; printf error >&2; exit 7".into(),
            2 => policy.argv[2] = "sleep 3".into(),
            3 => policy.version_argv[2] = "exit 0".into(),
            4 => {
                policy.argv[2] =
                    "cat >/dev/null; printf changed >response-input.json; printf '{}'".into()
            }
            5 => policy.argv[2] = "cat >/dev/null; printf changed >report.json; printf '{}'".into(),
            6 => {}
            _ => unreachable!(),
        }
        std::fs::write(&options.policy, serde_json::to_vec(&policy).unwrap()).unwrap();
        let (result, code) = run(options).await.unwrap();
        assert_eq!(code, Decision::Incomplete.exit_code(), "case {case}");
        assert_eq!(result["complete"], false);
        assert_eq!(result["route"], "inspect_gap");
        assert_eq!(result["assessment"]["kind"], "execution_gap");
        assert_eq!(result["original_gate"]["complete"], true);
        assert_eq!(result["artifacts"].as_array().unwrap().len(), 4);
    }
}

#[tokio::test]
async fn incomplete_or_ambiguous_reports_and_unconfined_inputs_are_not_sent() {
    let root = tempfile::tempdir().unwrap();
    let (mut options, report) = fixture(root.path(), "{}");
    options.fingerprint = Some("missing".into());
    assert!(
        run(options)
            .await
            .unwrap_err()
            .to_string()
            .contains("unambiguous")
    );
    for case in 0..3 {
        let (options, _) = fixture(root.path(), "{}");
        let mut changed = report.clone();
        match case {
            0 => changed.gate.complete = false,
            1 => changed.run_id.clear(),
            2 => changed.schema_version = 2,
            _ => unreachable!(),
        }
        std::fs::write(&options.report, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            run(options)
                .await
                .unwrap_err()
                .to_string()
                .contains("incomplete")
        );
    }
    let (options, _) = fixture(root.path(), "{}");
    let mut policy: JudgmentPolicy =
        serde_json::from_slice(&std::fs::read(&options.policy).unwrap()).unwrap();
    policy.provider_inputs = vec!["../outside".into()];
    std::fs::write(&options.policy, serde_json::to_vec(&policy).unwrap()).unwrap();
    assert!(run(options).await.is_err());
    assert!(!root.path().join("decisions").exists());
    assert!(bounded_file(root.path(), 10).await.is_err());
    assert!(
        bounded_file(&root.path().join("report.json"), 1)
            .await
            .is_err()
    );
    let Assessment::ExecutionGap { reason } = gap("界".repeat(1025)) else {
        panic!("expected gap")
    };
    assert_eq!(reason.chars().count(), 1024);
    std::fs::write(
        root.path().join("question.json"),
        serde_json::to_vec(&policy.question).unwrap(),
    )
    .unwrap();
    assert_eq!(
        question_digest(root.path().join("question.json"))
            .await
            .unwrap()["source_digest"],
        policy.question.source_digest
    );
}

#[cfg(unix)]
#[tokio::test]
async fn provider_aggregate_inputs_and_response_size_are_enforced_before_assessment() {
    for scenario in ["inputs", "response", "capture"] {
        let root = tempfile::tempdir().unwrap();
        let (options, _) = fixture(root.path(), "{}");
        let mut policy: JudgmentPolicy =
            serde_json::from_slice(&std::fs::read(&options.policy).unwrap()).unwrap();
        if scenario == "inputs" {
            for name in ["a.bin", "b.bin"] {
                std::fs::write(root.path().join(name), vec![b'x'; 600_000]).unwrap();
            }
            policy.provider_inputs = vec!["a.bin".into(), "b.bin".into()];
        } else {
            policy.argv[2] = format!(
                "cat >/dev/null; head -c {} /dev/zero",
                if scenario == "response" {
                    70_000
                } else {
                    runner::MAX_OUTPUT_BYTES + 1
                }
            );
        }
        std::fs::write(&options.policy, serde_json::to_vec(&policy).unwrap()).unwrap();
        if scenario == "inputs" {
            assert!(
                run(options)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("Provider inputs exceed")
            );
        } else {
            let (result, code) = run(options).await.unwrap();
            assert_eq!(code, 2);
            assert_eq!(result["route"], "inspect_gap");
            assert!(
                result["assessment"]["reason"].as_str().unwrap().contains(
                    if scenario == "response" {
                        "64 KiB"
                    } else {
                        "capture failed"
                    }
                ),
                "{result:#}"
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn calibrated_probability_only_prioritizes_review_and_never_changes_the_warning_gate() {
    for probability in [0.9, 0.4] {
        let root = tempfile::tempdir().unwrap();
        let model = snapshot::digest(b"fixture model");
        let response=json!({"kind":"probabilistic","question_id":"review","primitive":"choice","selected":if probability>=0.5{"true"}else{"false"},"applicability":"in_domain","calibration_ref":"calibration","evidence_refs":["EVIDENCE"],"distribution":{"true":probability,"false":1.0-probability},"target_event":"finding_valid","model_digest":model}).to_string();
        let (options, report) = fixture(root.path(), &response);
        let mut policy: JudgmentPolicy =
            serde_json::from_slice(&std::fs::read(&options.policy).unwrap()).unwrap();
        let executable = runner::identity::executable("sh", root.path())
            .await
            .unwrap();
        let input_path = root.path().join("response-input.json");
        let binding = provider_binding(
            &executable,
            &snapshot::digest(b"fixture-provider-v1"),
            &[artifact(&input_path, &std::fs::read(&input_path).unwrap())],
        )
        .unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        policy.mode = JudgmentMode::Advisory;
        policy.review_priority_threshold = Some(0.8);
        policy.calibration=Some(serde_json::from_value(json!({"id":"calibration","declared_status":"valid","provider_digest":binding,"model_digest":model,"calibrator_digest":snapshot::digest(b"calibrator"),"label_source_digest":snapshot::digest(b"independent labels"),"question_digest":policy.question.source_digest,"target_event":"finding_valid","repository_digest":snapshot::digest(root.path().to_string_lossy().as_bytes()),"language":"rust","rule_id":"review","labeled_from_unix":now-100,"labeled_until_unix":now-1,"expires_unix":now+3600,"revoked":false,"sample_count":30,"independent_label_count":30,"brier_score":0.1,"log_loss":0.2,"near_threshold_error":0.1,"risk_coverage":[{"coverage":0.5,"risk":0.1}],"known_limits":["synthetic unit calibration"]})).unwrap());
        std::fs::write(&options.policy, serde_json::to_vec(&policy).unwrap()).unwrap();
        let (result, code) = run(options).await.unwrap();
        assert_eq!(code, 0, "{result:#}");
        assert_eq!(result["calibration_status"], "valid");
        assert_eq!(
            result["route"],
            if probability >= 0.8 {
                "prioritize_review"
            } else {
                "manual_review"
            }
        );
        assert_eq!(
            result["original_gate"],
            serde_json::to_value(report.gate).unwrap()
        );
        assert_eq!(result["audit"]["automatic_gate_change"], false);
    }
}
