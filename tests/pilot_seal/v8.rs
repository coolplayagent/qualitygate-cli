use super::*;

pub(super) fn capture(assignment: &Value, number: u16, at: u64, actual: Option<&str>) -> Value {
    json!({
        "assignment_id":assignment["id"],"attempt_number":number,"captured_at":at,
        "agent_version":assignment["cohort"]["agent_version"],
        "harness_digest":assignment["cohort"]["harness_digest"],
        "requested_model":assignment["cohort"]["requested_model"],
        "reasoning_effort":assignment["cohort"]["reasoning_effort"],
        "actual_model":actual,
        "status":if actual.is_some() {"reported"} else {"unknown"},
        "unknown_reason":actual.is_none().then_some("Provider did not expose routing")
    })
}

pub(super) fn evidence(root: &std::path::Path, name: &str, capture: &Value) -> Value {
    let bytes = serde_json::to_vec(capture).unwrap();
    std::fs::write(root.join(name), &bytes).unwrap();
    json!({"artifact":{"path":name,"digest":digest(&bytes),"bytes":bytes.len()},
        "capture":capture})
}

fn summarize(root: &std::path::Path, name: &str, manifest: &Value) -> Value {
    write(root, name, manifest);
    report(
        &cli(
            root,
            &["pilot", "summarize", "--input", name, "--format", "json"],
        ),
        2,
    )
}

#[test]
fn cli_v8_verifies_each_attempt_model_file_and_exposes_route_drift() {
    let repo = fixture();
    let root = repo.path();
    let mut candidate = v6_candidate(root);
    candidate["schema_version"] = json!(8);
    write(root, "candidate-v8.json", &candidate);
    let mut sealed = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "candidate-v8.json",
                "--format",
                "json",
            ],
        ),
        0,
    );
    let id = sealed["run_order"][0].as_str().unwrap().to_owned();
    let assignment = sealed["assignments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|assignment| assignment["id"] == id)
        .unwrap()
        .clone();
    let at = sealed["protocol"]["start_at"].as_u64().unwrap();
    let first = capture(&assignment, 1, at, Some("routed-A"));
    let second = capture(&assignment, 2, at + 1, None);
    let first_evidence = evidence(root, "model-attempt-1.json", &first);
    let second_evidence = evidence(root, "model-attempt-2.json", &second);
    sealed["observations"] = json!([{
        "assignment_id":id,"observed_at":at + 1,"start_sequence":1,
        "attempts":[
            {"number":1,"status":"failed","elapsed_ms":1000,"check_elapsed_ms":null,
                "profile":"full","snapshot_digest":assignment["initial_snapshot"],
                "report":null,"cost":null,"usage":null,"model_evidence":first_evidence},
            {"number":2,"status":"timed_out","elapsed_ms":1000,"check_elapsed_ms":null,
                "profile":"full","snapshot_digest":assignment["initial_snapshot"],
                "report":null,"cost":null,"usage":null,"model_evidence":second_evidence}
        ],"findings":[],"review_active_ms":null,"review_comments":null,"rework_rounds":null
    }]);
    let summary = summarize(root, "observed-v8.json", &sealed);
    assert_eq!(
        summary["model_audit"]["records"][0]["attempt_status"],
        "failed"
    );
    assert_eq!(
        summary["model_audit"]["records"][1]["attempt_status"],
        "timed_out"
    );
    assert_eq!(summary["model_audit"]["actual_identity_unknown"], 1);

    let mut missing = sealed.clone();
    missing["observations"][0]["attempts"][1]
        .as_object_mut()
        .unwrap()
        .remove("model_evidence");
    let summary = summarize(root, "missing-v8.json", &missing);
    assert_eq!(summary["model_audit"]["missing_attempts"], 1);
    assert_eq!(summary["complete"], false);

    let mut changed = sealed.clone();
    let changed_capture = capture(&assignment, 2, at + 1, Some("routed-B"));
    changed["observations"][0]["attempts"][1]["model_evidence"] =
        evidence(root, "model-attempt-2.json", &changed_capture);
    let summary = summarize(root, "changed-v8.json", &changed);
    assert_eq!(summary["model_audit"]["drifted_assignments"], json!([id]));
    assert!(
        summary["limitations"]
            .to_string()
            .contains("changed across attempts")
    );
    assert!(
        summarize(root, "drifted-v8.json", &sealed)
            .to_string()
            .contains("length or digest differs")
    );
    evidence(root, "model-attempt-2.json", &second);

    let mut mismatched = sealed.clone();
    let different = capture(&assignment, 2, at + 1, Some("other"));
    let different_evidence = evidence(root, "model-attempt-2.json", &different);
    mismatched["observations"][0]["attempts"][1]["model_evidence"]["artifact"] =
        different_evidence["artifact"].clone();
    assert!(
        summarize(root, "mismatched-v8.json", &mismatched)
            .to_string()
            .contains("differs from the manifest claim")
    );
    evidence(root, "model-attempt-2.json", &second);

    let mut traversal = sealed.clone();
    traversal["observations"][0]["attempts"][1]["model_evidence"]["artifact"]["path"] =
        json!("../outside.json");
    assert!(
        summarize(root, "traversal-v8.json", &traversal)
            .to_string()
            .contains("Path must stay inside")
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            root.join("model-attempt-2.json"),
            root.join("model-link.json"),
        )
        .unwrap();
        let mut linked = sealed.clone();
        linked["observations"][0]["attempts"][1]["model_evidence"]["artifact"]["path"] =
            json!("model-link.json");
        assert!(
            summarize(root, "linked-v8.json", &linked)
                .to_string()
                .contains("Symlinks are not valid")
        );
    }
    let mut oversized = sealed;
    oversized["observations"][0]["attempts"][1]["model_evidence"]["artifact"]["bytes"] =
        json!(64 * 1024 + 1);
    assert!(
        summarize(root, "oversized-v8.json", &oversized)
            .to_string()
            .contains("model capture artifact")
    );
}
