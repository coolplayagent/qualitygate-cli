use super::*;

fn fixture() -> (tempfile::TempDir, Value, Value) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("runs")).unwrap();
    let digest = snapshot::digest(b"scope");
    let decision = snapshot::digest(b"decision");
    let labels = json!({"schema_version":1,"repository_digest":digest,"question_digest":digest,
        "provider_binding_digest":digest,"model_digest":digest,"calibrator_digest":digest,"label_source_digest":digest,
        "target_event":"finding_valid_after_independent_review","validation_from_unix":100,"validation_until_unix":200,
        "audit_fraction":1.0,"labels":[{"decision_id":decision,"finding_valid":true,"reviewer":"independent",
            "source_digest":digest,"reviewed_unix":160,"independent":true}]});
    let calibration = json!({"id":"calibration","declared_status":"valid","provider_digest":digest,
        "model_digest":digest,"calibrator_digest":digest,"label_source_digest":digest,"question_digest":digest,
        "target_event":"finding_valid_after_independent_review","repository_digest":digest,"language":"rust","rule_id":"lint",
        "labeled_from_unix":1,"labeled_until_unix":99,"expires_unix":201,"revoked":false,"sample_count":30,
        "independent_label_count":30,"brier_score":0.1,"log_loss":0.2,"near_threshold_error":0.1,
        "risk_coverage":[],"known_limits":[]});
    let run = json!({"schema_version":1,"decision_id":decision,"mode":"advisory",
        "original_gate":{"complete":true,"decision":"pass","blockers":[]},"report_digest":digest,"policy_digest":digest,
        "repository_digest":digest,"question_digest":digest,"rule_id":"lint","language":"rust","evidence_id":"warning",
        "assessment":{"kind":"probabilistic","question_id":"review", "primitive":"choice", "selected":"valid", "applicability":"in_domain", "calibration_ref":"calibration", "evidence_refs":[], "distribution":{"valid":0.8,"invalid":0.2},
            "target_event":"finding_valid_after_independent_review","model_digest":digest},
        "complete":true,"route":"prioritize_review","calibration_status":"valid","calibration":calibration,
        "review_priority_threshold":0.8,"provider":null,"provider_version_digest":digest,"provider_binding_digest":digest,
        "artifacts":[],"audit":{"started_unix_nanos":150_000_000_000u64}});
    (root, labels, run)
}

fn save(root: &Path, labels: &Value, run: Option<&Value>) -> Options {
    fs::write(
        root.join("labels.json"),
        serde_json::to_vec(labels).unwrap(),
    )
    .unwrap();
    if let Some(run) = run {
        let directory = root.join("runs").join(
            run["decision_id"]
                .as_str()
                .unwrap()
                .strip_prefix("sha256:")
                .unwrap(),
        );
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("decision.json"),
            serde_json::to_vec(run).unwrap(),
        )
        .unwrap();
    }
    Options {
        runs: root.join("runs"),
        labels: root.join("labels.json"),
        audit_seed: "independent-audit-seed".into(),
    }
}

#[test]
fn calibrated_label_produces_known_scoring_and_audit_values_without_changing_gate() {
    let (root, labels, run) = fixture();
    let (summary, code) = summarize_sync(save(root.path(), &labels, Some(&run))).unwrap();
    assert_eq!(code, 0, "{summary}");
    assert_eq!(summary["complete"], true);
    assert_eq!(summary["gate_effect"], "none");
    assert_eq!(summary["counts"]["probabilistic_labeled"], 1);
    assert_eq!(summary["counts"]["manual_review_recommendations"], 1);
    assert!((summary["calibration"]["brier_score"].as_f64().unwrap() - 0.04).abs() < 1e-12);
    assert!((summary["calibration"]["log_loss"].as_f64().unwrap() + 0.8f64.ln()).abs() < 1e-12);
    assert!(
        (summary["calibration"]["near_threshold_error"]
            .as_f64()
            .unwrap()
            - 0.2)
            .abs()
            < 1e-12
    );
    assert_eq!(summary["calibration"]["reliability"][4]["count"], 1);
    assert_eq!(
        summary["calibration"]["reliability"][0]["mean_probability"],
        Value::Null
    );
    assert_eq!(
        summary["calibration"]["risk_coverage"][3]["empirical_risk"],
        0.0
    );
    assert_eq!(summary["calibration"]["slices"][0]["rule"], "lint");
    assert_eq!(
        summary["audit"]["sampled_decision_ids"],
        json!([run["decision_id"]])
    );
    let (again, _) = summarize_sync(save(root.path(), &labels, Some(&run))).unwrap();
    assert_eq!(summary, again);
}

#[test]
fn absent_labels_calibration_and_execution_gaps_never_become_valid_probability_evidence() {
    for (pointer, replacement, expected) in [
        ("/calibration", Value::Null, "Missing calibration"),
        (
            "/calibration_status",
            json!("stale"),
            "Calibration or label cohort is invalid",
        ),
        (
            "/repository_digest",
            json!(snapshot::digest(b"other repo")),
            "binding changed",
        ),
        (
            "/audit/started_unix_nanos",
            json!(99_000_000_000u64),
            "outside the validation period",
        ),
        (
            "/assessment/distribution",
            json!({"invalid":1.0}),
            "Target event choice is absent",
        ),
        (
            "/assessment/distribution/valid",
            json!(1.1),
            "Invalid probability",
        ),
    ] {
        let (root, labels, mut run) = fixture();
        *run.pointer_mut(pointer).unwrap() = replacement;
        let (summary, code) = summarize_sync(save(root.path(), &labels, Some(&run))).unwrap();
        assert_eq!(code, 2);
        assert!(
            summary["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue.as_str().unwrap().contains(expected)),
            "{summary}"
        );
    }
    let (root, mut labels, run) = fixture();
    labels["labels"] = json!([]);
    let (summary, code) = summarize_sync(save(root.path(), &labels, Some(&run))).unwrap();
    assert_eq!(code, 2);
    assert_eq!(summary["audit"]["missing_labels"], 1);
    assert!(summary["calibration"]["brier_score"].is_null());
    let (root, labels, _) = fixture();
    let (summary, code) = summarize_sync(save(root.path(), &labels, None)).unwrap();
    assert_eq!(code, 2);
    assert!(
        summary["issues"]
            .as_array()
            .unwrap()
            .contains(&json!("No retained judgment runs"))
    );
}

#[test]
fn invalid_independent_labels_and_tampered_artifacts_are_rejected() {
    for (key, value) in [
        ("independent", json!(false)),
        ("reviewer", json!("")),
        ("reviewed_unix", json!(149)),
        ("source_digest", json!("invalid")),
    ] {
        let (root, mut labels, run) = fixture();
        labels["labels"][0][key] = value;
        let (summary, code) = summarize_sync(save(root.path(), &labels, Some(&run))).unwrap();
        assert_eq!(code, 2, "{key}: {summary}");
    }
    let (root, labels, mut run) = fixture();
    let options = save(root.path(), &labels, Some(&run));
    let directory = options.runs.join(
        run["decision_id"]
            .as_str()
            .unwrap()
            .trim_start_matches("sha256:"),
    );
    let artifact = directory.join("response.json");
    fs::write(&artifact, b"original").unwrap();
    run["artifacts"] = json!([{"path":artifact,"digest":snapshot::digest(b"original"),"bytes":8}]);
    let options = save(root.path(), &labels, Some(&run));
    load(&options).unwrap();
    fs::write(&artifact, b"tampered").unwrap();
    assert!(
        load(&options)
            .err()
            .unwrap()
            .to_string()
            .contains("digest differs")
    );
    let outside = root.path().join("outside.json");
    fs::write(&outside, b"original").unwrap();
    run["artifacts"][0]["path"] = json!(outside);
    assert!(
        load(&save(root.path(), &labels, Some(&run)))
            .err()
            .unwrap()
            .to_string()
            .contains("escapes")
    );
}

#[test]
fn independent_negative_labels_and_abstentions_keep_scoring_denominators_separate() {
    let (root, mut labels, run) = fixture();
    save(root.path(), &labels, Some(&run));
    let mut negative = run.clone();
    let id = snapshot::digest(b"negative decision");
    negative["decision_id"] = json!(id);
    negative["assessment"]["distribution"] = json!({"valid":0.3,"invalid":0.7});
    labels["labels"].as_array_mut().unwrap().push(json!({"decision_id":id,"finding_valid":false,"reviewer":"independent","source_digest":snapshot::digest(b"negative review"),"reviewed_unix":160,"independent":true}));
    save(root.path(), &labels, Some(&negative));
    for (kind, assessment) in [
        (
            "abstained",
            json!({"kind":"abstained","question_id":"review","reason":"insufficient_evidence","evidence_refs":[]}),
        ),
        (
            "deterministic",
            json!({"kind":"deterministic","question_id":"review","value":true,"evidence_refs":[]}),
        ),
    ] {
        let mut other = run.clone();
        other["decision_id"] = json!(snapshot::digest(kind.as_bytes()));
        other["assessment"] = assessment;
        save(root.path(), &labels, Some(&other));
    }
    let (summary, code) = summarize_sync(save(root.path(), &labels, None)).unwrap();
    assert_eq!(code, 0, "{summary}");
    assert_eq!(summary["counts"]["probabilistic_labeled"], 2);
    assert!((summary["calibration"]["brier_score"].as_f64().unwrap() - 0.065).abs() < 1e-12);
    assert!(
        (summary["calibration"]["log_loss"].as_f64().unwrap() - (-0.8f64.ln() - 0.7f64.ln()) / 2.0)
            .abs()
            < 1e-12
    );
    let mut gap = run;
    gap["decision_id"] = json!(snapshot::digest(b"execution gap"));
    gap["assessment"] = json!({"kind":"execution_gap","reason":"provider unavailable"});
    gap["complete"] = json!(false);
    let (summary, code) = summarize_sync(save(root.path(), &labels, Some(&gap))).unwrap();
    assert_eq!(code, 2);
    assert!(summary["issues"].to_string().contains("Execution gap"));
}

#[test]
fn pilot_inventory_rejects_unbounded_seed_special_entries_and_malformed_decisions() {
    let (root, labels, run) = fixture();
    let mut options = save(root.path(), &labels, Some(&run));
    options.audit_seed = "short".into();
    assert!(
        load(&options)
            .err()
            .expect("invalid inventory")
            .to_string()
            .contains("Audit seed")
    );
    options.audit_seed = "x".repeat(16);
    std::fs::write(options.runs.join("unrelated"), b"not a directory").unwrap();
    assert!(
        load(&options)
            .err()
            .expect("invalid inventory")
            .to_string()
            .contains("non-directory")
    );
    std::fs::remove_file(options.runs.join("unrelated")).unwrap();
    let directory = options.runs.join(
        run["decision_id"]
            .as_str()
            .unwrap()
            .strip_prefix("sha256:")
            .unwrap(),
    );
    std::fs::write(directory.join("decision.json"), b"malformed").unwrap();
    assert!(
        load(&options)
            .err()
            .expect("invalid inventory")
            .to_string()
            .contains("Malformed retained")
    );
    std::fs::File::create(directory.join("decision.json"))
        .unwrap()
        .set_len(256 * 1024 + 1)
        .unwrap();
    assert!(load(&options).is_err());
}
