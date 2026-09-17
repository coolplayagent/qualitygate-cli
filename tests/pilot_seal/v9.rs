use super::*;

fn execution_capture(assignment: &Value, attempt: &Value, started_at_ms: u64) -> Value {
    json!({
        "assignment_id":assignment["id"],
        "attempt_number":attempt["number"],
        "started_at_ms":started_at_ms,
        "ended_at_ms":started_at_ms + attempt["elapsed_ms"].as_u64().unwrap(),
        "harness_digest":assignment["cohort"]["harness_digest"],
        "status":attempt["status"],
        "snapshot_digest":attempt["snapshot_digest"],
        "report_digest":attempt["report"]["digest"]
    })
}

fn evidence(root: &std::path::Path, name: &str, capture: &Value) -> Value {
    let bytes = serde_json::to_vec(capture).unwrap();
    std::fs::write(root.join(name), &bytes).unwrap();
    json!({"artifact":{"path":name,"digest":digest(&bytes),"bytes":bytes.len()},
        "capture":capture})
}

fn summarized(root: &std::path::Path, name: &str, manifest: &Value) -> Value {
    write(root, name, manifest);
    report(
        &cli(
            root,
            &["pilot", "summarize", "--input", name, "--format", "json"],
        ),
        2,
    )
}

fn sealed(root: &std::path::Path) -> Value {
    let mut candidate = v6_candidate(root);
    candidate["schema_version"] = json!(9);
    write(root, "candidate-v9.json", &candidate);
    report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "candidate-v9.json",
                "--format",
                "json",
            ],
        ),
        0,
    )
}

fn add_observation(root: &std::path::Path, manifest: &mut Value, sequence: usize, start_ms: u64) {
    let id = manifest["run_order"][sequence - 1]
        .as_str()
        .unwrap()
        .to_owned();
    let assignment = manifest["assignments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|assignment| assignment["id"] == id)
        .unwrap()
        .clone();
    manifest["assignments"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|assignment| assignment["id"] == id)
        .unwrap()["cohort"]["actual_model"] = json!("routed-A");
    let at = start_ms / 1000;
    let model_capture = super::v8::capture(&assignment, 1, at + 1, Some("routed-A"));
    let model = super::v8::evidence(root, &format!("model-{sequence}.json"), &model_capture);
    let mut attempt = json!({"number":1,"status":"failed","elapsed_ms":1000,
        "check_elapsed_ms":null,"profile":"full","snapshot_digest":assignment["initial_snapshot"],
        "report":null,"cost":null,"usage":null,"model_evidence":model});
    let capture = execution_capture(&assignment, &attempt, start_ms);
    attempt["execution_evidence"] = evidence(root, &format!("execution-{sequence}.json"), &capture);
    manifest["observations"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "assignment_id":id,"observed_at":at + 2,"start_sequence":sequence,
            "attempts":[attempt],"findings":[],"review_active_ms":null,
            "review_comments":null,"rework_rounds":null
        }));
}

#[test]
fn cli_v9_rereads_receipts_and_rejects_drift_and_unsafe_files() {
    let repo = fixture();
    let root = repo.path();
    let mut manifest = sealed(root);
    let at = manifest["protocol"]["start_at"].as_u64().unwrap();
    add_observation(root, &mut manifest, 1, at * 1000);
    let summary = summarized(root, "observed-v9.json", &manifest);
    assert!(summary.get("execution_audit").is_some(), "{summary}");
    assert_eq!(summary["execution_audit"]["records"][0]["status"], "failed");
    assert_eq!(summary["execution_audit"]["missing_attempts"], 0);

    let mut missing = manifest.clone();
    missing["observations"][0]["attempts"][0]
        .as_object_mut()
        .unwrap()
        .remove("execution_evidence");
    let summary = summarized(root, "missing-v9.json", &missing);
    assert_eq!(summary["execution_audit"]["missing_attempts"], 1);
    assert_eq!(summary["complete"], false);

    let path = root.join("execution-1.json");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{}").unwrap();
    assert!(
        summarized(root, "changed-v9.json", &manifest)
            .to_string()
            .contains("length or digest differs")
    );
    std::fs::write(&path, &original).unwrap();

    let mut wrong = manifest.clone();
    let bad_capture = execution_capture(
        &wrong["assignments"][0],
        &wrong["observations"][0]["attempts"][0],
        at * 1000 + 1,
    );
    wrong["observations"][0]["attempts"][0]["execution_evidence"]["artifact"] =
        evidence(root, "execution-1.json", &bad_capture)["artifact"].clone();
    assert!(
        summarized(root, "wrong-v9.json", &wrong)
            .to_string()
            .contains("differs from the manifest claim")
    );
    std::fs::write(&path, &original).unwrap();

    let mut extra = manifest.clone();
    let mut extra_capture =
        extra["observations"][0]["attempts"][0]["execution_evidence"]["capture"].clone();
    extra_capture["unrecognized"] = json!(true);
    extra["observations"][0]["attempts"][0]["execution_evidence"]["artifact"] =
        evidence(root, "execution-1.json", &extra_capture)["artifact"].clone();
    assert!(
        summarized(root, "extra-v9.json", &extra)
            .to_string()
            .contains("Invalid execution evidence JSON")
    );
    std::fs::write(&path, &original).unwrap();

    let mut traversal = manifest.clone();
    traversal["observations"][0]["attempts"][0]["execution_evidence"]["artifact"]["path"] =
        json!("../outside.json");
    assert!(
        summarized(root, "traversal-v9.json", &traversal)
            .to_string()
            .contains("Path must stay inside")
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&path, root.join("execution-link.json")).unwrap();
        let mut linked = manifest.clone();
        linked["observations"][0]["attempts"][0]["execution_evidence"]["artifact"]["path"] =
            json!("execution-link.json");
        assert!(
            summarized(root, "linked-v9.json", &linked)
                .to_string()
                .contains("Symlinks are not valid")
        );
    }
    let mut oversized = manifest;
    oversized["observations"][0]["attempts"][0]["execution_evidence"]["artifact"]["bytes"] =
        json!(64 * 1024 + 1);
    assert!(
        summarized(root, "oversized-v9.json", &oversized)
            .to_string()
            .contains("execution capture")
    );
}

#[test]
fn cli_v9_detects_start_sequence_conflict_with_archived_clock() {
    let repo = fixture();
    let root = repo.path();
    let mut manifest = sealed(root);
    let at = manifest["protocol"]["start_at"].as_u64().unwrap();
    add_observation(root, &mut manifest, 1, at * 1000 + 2000);
    add_observation(root, &mut manifest, 2, at * 1000);
    let summary = summarized(root, "reversed-v9.json", &manifest);
    assert!(summary.get("execution_audit").is_some(), "{summary}");
    assert_eq!(summary["execution_audit"]["status"], "deviated");
    assert_eq!(
        summary["execution_audit"]["deviations"][0]["reason"],
        "start_order_differs_from_sequence"
    );
    assert_eq!(summary["protocol_ready"], false);
}
