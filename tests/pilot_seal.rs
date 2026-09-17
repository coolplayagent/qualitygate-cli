mod common;
use common::*;
use qualitygate::domain::{
    CheckResult, Report, Severity, Summary, evaluate, pilot::tool_inventory_digest,
};
use qualitygate::snapshot::digest;
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

fn plan() -> Value {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    json!({
        "schema_version":1,"id":"sealed-pilot","protocol":{
            "project":"qualitygate-cli","sampling":"one controlled task","owner":"owner",
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

fn two_task_v3_plan() -> Value {
    let mut candidate = plan();
    candidate["schema_version"] = json!(3);
    candidate["protocol"]["task_count"] = json!(2);
    candidate["protocol"]["task_mix"] = json!({"bug_fix":1,"refactor":1});
    candidate["protocol"]["monetary_cap"] = Value::Null;
    candidate["protocol"]["budget"] = json!({
        "currency":"USD","priced_at":candidate["protocol"]["sealed_at"],
        "source":"fixture rate card","max_total_micros":1000000,
        "human_hourly_micros":1000000
    });
    let originals = candidate["assignments"].as_array().unwrap().clone();
    for mut assignment in originals {
        assignment["id"] = json!(format!("{}-refactor", assignment["id"].as_str().unwrap()));
        assignment["input_id"] = json!("task-2");
        assignment["task_id"] = json!("task-2");
        assignment["task_kind"] = json!("refactor");
        assignment["task_digest"] = json!(digest(b"refactor-task"));
        assignment["initial_snapshot"] = json!(digest(b"refactor-initial"));
        candidate["assignments"]
            .as_array_mut()
            .unwrap()
            .push(assignment);
    }
    candidate
}

fn write(root: &std::path::Path, name: &str, value: &Value) {
    std::fs::write(root.join(name), serde_json::to_vec(value).unwrap()).unwrap();
}

#[test]
fn cli_seals_before_observation_and_summary_rejects_later_plan_changes() {
    let repo = fixture();
    let root = repo.path();
    write(root, "plan.json", &plan());
    let sealed = report(
        &cli(
            root,
            &["pilot", "seal", "--input", "plan.json", "--format", "json"],
        ),
        0,
    );
    assert_eq!(sealed["plan_seal"]["algorithm"], "sha256");
    assert!(
        sealed["plan_seal"]["digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );

    write(root, "sealed.json", &sealed);
    let partial = report(
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
    assert_eq!(partial["plan_seal"], sealed["plan_seal"]);
    assert!(
        !partial["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str().unwrap().contains("no digest-verified"))
    );

    let mut changed = sealed;
    changed["assignments"][0]["cohort"]["permissions"] = json!("read-only");
    write(root, "changed.json", &changed);
    let rejected = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "changed.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        rejected["gate"]["blockers"][0]
            .as_str()
            .unwrap()
            .contains("differs from its pre-observation seal")
    );
}

#[test]
fn cli_refuses_to_seal_after_observation_or_without_complete_governance() {
    let repo = fixture();
    let root = repo.path();
    let mut value = plan();
    value["protocol"]["archive"] = Value::Null;
    write(root, "unready.json", &value);
    let unready = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "unready.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        unready["gate"]["blockers"][0]
            .as_str()
            .unwrap()
            .contains("durable archive")
    );

    let mut value = plan();
    value["observations"] = json!([{"assignment_id":"medium-existing","observed_at":1,
        "attempts":[],"findings":[],"review_active_ms":null,"review_comments":null,"rework_rounds":null}]);
    write(root, "late.json", &value);
    let late = report(
        &cli(
            root,
            &["pilot", "seal", "--input", "late.json", "--format", "json"],
        ),
        2,
    );
    assert!(
        late["gate"]["blockers"][0]
            .as_str()
            .unwrap()
            .contains("before observations")
    );
}

#[test]
fn cli_v3_seal_audits_task_mix_and_rejects_repeated_task_contracts() {
    let repo = fixture();
    let root = repo.path();
    let mut candidate = two_task_v3_plan();
    write(root, "candidate-v3.json", &candidate);
    let sealed = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "candidate-v3.json",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(sealed["schema_version"], 3);
    write(root, "sealed-v3.json", &sealed);
    let summary = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed-v3.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(summary["sampling_audit"]["distinct_inputs"], 2);
    assert_eq!(summary["sampling_audit"]["declared"]["refactor"], 1);

    for assignment in &mut candidate["assignments"].as_array_mut().unwrap()[4..] {
        assignment["task_digest"] = json!(digest(b"task"));
    }
    write(root, "duplicate-v3.json", &candidate);
    let rejected = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "duplicate-v3.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        rejected
            .to_string()
            .contains("reuse a task contract digest")
    );
}

#[test]
fn cli_v4_rechecks_confined_source_artifacts_before_sealing_and_summary() {
    let repo = fixture();
    let root = repo.path();
    let mut candidate = two_task_v3_plan();
    candidate["schema_version"] = json!(4);
    let source_one = b"Reviewed issue one\n";
    let source_two = b"Reviewed commit two\n";
    std::fs::write(root.join("source-one.txt"), source_one).unwrap();
    std::fs::write(root.join("source-two.txt"), source_two).unwrap();
    let selected_at = candidate["protocol"]["sealed_at"].clone();
    candidate["sources"] = json!([
        {"input_id":"task-1","kind":"issue","source_id":"fixture/issues/1",
            "path":"source-one.txt","digest":digest(source_one),"bytes":source_one.len(),
            "selected_at":selected_at},
        {"input_id":"task-2","kind":"commit","source_id":"fixture/commits/2",
            "path":"source-two.txt","digest":digest(source_two),"bytes":source_two.len(),
            "selected_at":selected_at}
    ]);
    write(root, "candidate-v4.json", &candidate);
    let sealed = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "candidate-v4.json",
                "--format",
                "json",
            ],
        ),
        0,
    );
    write(root, "sealed-v4.json", &sealed);
    let summary = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed-v4.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(summary["source_audit"]["distinct_inputs"], 2);
    assert_eq!(summary["source_audit"]["sources"][1]["kind"], "commit");

    std::fs::write(root.join("source-one.txt"), b"Changed issue one\n").unwrap();
    let changed = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed-v4.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        changed
            .to_string()
            .contains("Task source artifact length or digest differs")
    );
    std::fs::write(root.join("source-one.txt"), source_one).unwrap();

    let mut traversal = candidate.clone();
    traversal["sources"][0]["path"] = json!("../outside.txt");
    write(root, "traversal-v4.json", &traversal);
    let escaped = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "traversal-v4.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(escaped.to_string().contains("inside the repository"));

    #[cfg(unix)]
    {
        std::fs::remove_file(root.join("source-one.txt")).unwrap();
        std::os::unix::fs::symlink("source-two.txt", root.join("source-one.txt")).unwrap();
        let linked = report(
            &cli(
                root,
                &[
                    "pilot",
                    "seal",
                    "--input",
                    "candidate-v4.json",
                    "--format",
                    "json",
                ],
            ),
            2,
        );
        assert!(
            linked
                .to_string()
                .contains("Symlinks are not valid checked inputs")
        );
        std::fs::remove_file(root.join("source-one.txt")).unwrap();
    }
    std::fs::write(root.join("source-one.txt"), source_one).unwrap();
    let authorized = report(
        &cli(
            root,
            &[
                "pilot",
                "authorization-subject",
                "--input",
                "sealed-v4.json",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(authorized["subject"]["task_count"], 2);

    std::fs::remove_file(root.join("source-two.txt")).unwrap();
    let missing = report(
        &cli(
            root,
            &[
                "pilot",
                "authorization-subject",
                "--input",
                "sealed-v4.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        missing
            .to_string()
            .contains("Task source artifact is unavailable")
    );
}

#[test]
fn cli_v5_seals_schedule_and_blocks_observed_order_deviation() {
    let repo = fixture();
    let root = repo.path();
    let mut candidate = two_task_v3_plan();
    candidate["schema_version"] = json!(5);
    let source_one = b"Reviewed issue one\n";
    let source_two = b"Reviewed commit two\n";
    std::fs::write(root.join("source-one.txt"), source_one).unwrap();
    std::fs::write(root.join("source-two.txt"), source_two).unwrap();
    let selected_at = candidate["protocol"]["sealed_at"].clone();
    candidate["sources"] = json!([
        {"input_id":"task-1","kind":"issue","source_id":"fixture/issues/1",
            "path":"source-one.txt","digest":digest(source_one),"bytes":source_one.len(),
            "selected_at":selected_at},
        {"input_id":"task-2","kind":"commit","source_id":"fixture/commits/2",
            "path":"source-two.txt","digest":digest(source_two),"bytes":source_two.len(),
            "selected_at":selected_at}
    ]);
    candidate["run_order"] = Value::Array(
        candidate["assignments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|assignment| assignment["id"].clone())
            .collect(),
    );
    write(root, "candidate-v5.json", &candidate);
    let sealed = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "candidate-v5.json",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(sealed["run_order"].as_array().unwrap().len(), 8);
    write(root, "sealed-v5.json", &sealed);
    let partial = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed-v5.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(partial["schedule_audit"]["status"], "incomplete");

    let mut deviated = sealed.clone();
    deviated["observations"] = json!([{
        "assignment_id":sealed["run_order"][0],
        "observed_at":sealed["protocol"]["start_at"],
        "start_sequence":2,"attempts":[],"findings":[],"review_active_ms":null,
        "review_comments":null,"rework_rounds":null
    }]);
    write(root, "deviated-v5.json", &deviated);
    let summary = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "deviated-v5.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(summary["schedule_audit"]["status"], "deviated");
    assert_eq!(summary["protocol_ready"], false);

    let mut changed = sealed;
    changed["run_order"].as_array_mut().unwrap().swap(0, 2);
    write(root, "changed-v5.json", &changed);
    let rejected = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "changed-v5.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(rejected.to_string().contains("pre-observation seal"));
}

fn v6_initial_report(assignment: &Value, index: usize) -> Report {
    let mut check = CheckResult::pending("task-test", true, Severity::Error);
    check.complete();
    serde_json::from_value(json!({
        "schema_version":1,"run_id":format!("initial-{index}"),"scope":"task","profile":"full",
        "environment_digest":assignment["cohort"]["environment_digest"],
        "snapshot":{"mode":"worktree","base":assignment["base"],"head":"WORKTREE",
            "content_digest":assignment["initial_snapshot"]},
        "policy":{"source":"fixture","config_digest":assignment["config_digest"],
            "rules_digest":digest(b"rules"),"task_contract_digest":assignment["task_digest"],
            "trust":"selected","changes":[]},
        "plan":{"task_id":assignment["task_id"],"required_checks":assignment["required_checks"],
            "pending_delivery_checks":[],"acceptance":{}},
        "gate":evaluate(&[check.clone()], &["task-test".into()], &[]),
        "checks":[check],"summary":Summary::default()
    }))
    .unwrap()
}

fn v6_candidate(root: &std::path::Path) -> Value {
    let mut candidate = two_task_v3_plan();
    candidate["schema_version"] = json!(6);
    candidate["protocol"]["no_progress_limit"] = json!(2);
    candidate["run_order"] = Value::Array(
        candidate["assignments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["id"].clone())
            .collect(),
    );
    let source_one = b"Reviewed issue one\n";
    let source_two = b"Reviewed commit two\n";
    std::fs::write(root.join("source-one.txt"), source_one).unwrap();
    std::fs::write(root.join("source-two.txt"), source_two).unwrap();
    let selected_at = candidate["protocol"]["sealed_at"].clone();
    candidate["sources"] = json!([
        {"input_id":"task-1","kind":"issue","source_id":"fixture/issues/1",
            "path":"source-one.txt","digest":digest(source_one),"bytes":source_one.len(),
            "selected_at":selected_at},
        {"input_id":"task-2","kind":"commit","source_id":"fixture/commits/2",
            "path":"source-two.txt","digest":digest(source_two),"bytes":source_two.len(),
            "selected_at":selected_at}
    ]);
    for (index, assignment) in candidate["assignments"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        let full = v6_initial_report(assignment, index);
        assignment["cohort"]["tools_digest"] = json!(tool_inventory_digest(&full));
        let bytes = serde_json::to_vec(&full).unwrap();
        let path = format!("initial-{index}.json");
        std::fs::write(root.join(&path), &bytes).unwrap();
        assignment["initial_report"] =
            json!({"path":path,"digest":digest(&bytes),"bytes":bytes.len()});
    }
    candidate
}

#[test]
fn cli_v6_rechecks_initial_reports_and_audits_stopping_after_two_no_progress_attempts() {
    let repo = fixture();
    let root = repo.path();
    let candidate = v6_candidate(root);
    write(root, "candidate-v6.json", &candidate);
    let sealed = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "candidate-v6.json",
                "--format",
                "json",
            ],
        ),
        0,
    );
    write(root, "sealed-v6.json", &sealed);
    let partial = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed-v6.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(
        partial["attempt_audit"]["assignments"][0]["baseline_verified"],
        true
    );

    let original = std::fs::read(root.join("initial-0.json")).unwrap();
    std::fs::remove_file(root.join("initial-0.json")).unwrap();
    let missing = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed-v6.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        missing
            .to_string()
            .contains("Initial report artifact is unavailable")
    );
    std::fs::write(root.join("initial-0.json"), b"changed").unwrap();
    let changed = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "sealed-v6.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        changed
            .to_string()
            .contains("Initial report length or digest differs")
    );
    std::fs::write(root.join("initial-0.json"), original).unwrap();

    let mut traversal = candidate.clone();
    traversal["assignments"][0]["initial_report"]["path"] = json!("../outside.json");
    write(root, "traversal-v6.json", &traversal);
    let rejected = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "traversal-v6.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(rejected.to_string().contains("Path must stay inside"));

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("initial-0.json"), root.join("initial-link.json"))
            .unwrap();
        let mut linked = candidate.clone();
        linked["assignments"][0]["initial_report"]["path"] = json!("initial-link.json");
        write(root, "linked-v6.json", &linked);
        let rejected = report(
            &cli(
                root,
                &[
                    "pilot",
                    "seal",
                    "--input",
                    "linked-v6.json",
                    "--format",
                    "json",
                ],
            ),
            2,
        );
        assert!(rejected.to_string().contains("Symlinks are not valid"));
    }

    let mut overrun = sealed;
    overrun["observations"] = json!([{"assignment_id":overrun["run_order"][0],
        "observed_at":overrun["protocol"]["start_at"],"start_sequence":1,
        "attempts":(1..=3).map(|number|json!({"number":number,"status":"failed",
            "elapsed_ms":1000,"check_elapsed_ms":null,"profile":"full",
            "snapshot_digest":digest(format!("snapshot-{number}").as_bytes()),
            "report":null,"cost":null,"usage":null})).collect::<Vec<_>>(),
        "findings":[],"review_active_ms":null,"review_comments":null,"rework_rounds":null}]);
    write(root, "overrun-v6.json", &overrun);
    let summary = report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "overrun-v6.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(
        summary["attempt_audit"]["assignments"][0]["stop_after_attempt"],
        2
    );
    assert_eq!(
        summary["attempt_audit"]["assignments"][0]["deviations"][0]["reason"],
        "continued_after_no_progress"
    );
    assert_eq!(summary["protocol_ready"], false);
}

#[test]
fn cli_v7_binds_model_capture_and_preserves_explicit_unknown_identity() {
    let repo = fixture();
    let root = repo.path();
    let mut candidate = v6_candidate(root);
    candidate["schema_version"] = json!(7);
    write(root, "candidate-v7.json", &candidate);
    let mut sealed = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "candidate-v7.json",
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
        .find(|a| a["id"] == id)
        .unwrap();
    let captured_at = sealed["protocol"]["start_at"].as_u64().unwrap();
    let capture = json!({
        "assignment_id":id,"captured_at":captured_at,
        "agent_version":assignment["cohort"]["agent_version"],
        "harness_digest":assignment["cohort"]["harness_digest"],
        "requested_model":assignment["cohort"]["requested_model"],
        "reasoning_effort":assignment["cohort"]["reasoning_effort"],
        "actual_model":null,"status":"unknown",
        "unknown_reason":"Provider response did not expose a routed model identifier"
    });
    let bytes = serde_json::to_vec(&capture).unwrap();
    std::fs::write(root.join("model-0.json"), &bytes).unwrap();
    sealed["observations"] = json!([{
        "assignment_id":id,"observed_at":captured_at,"start_sequence":1,
        "model_evidence":{"artifact":{"path":"model-0.json","digest":digest(&bytes),
            "bytes":bytes.len()},"capture":capture},
        "attempts":[],"findings":[],"review_active_ms":null,
        "review_comments":null,"rework_rounds":null
    }]);
    let run = |name: &str, value: &Value| {
        write(root, name, value);
        report(
            &cli(
                root,
                &["pilot", "summarize", "--input", name, "--format", "json"],
            ),
            2,
        )
    };
    let summary = run("observed-v7.json", &sealed);
    assert_eq!(summary["model_audit"]["records"][0]["status"], "unknown");
    assert_eq!(summary["model_audit"]["actual_identity_unknown"], 1);
    assert_eq!(summary["model_audit"]["unobserved"], 7);
    assert!(
        !summary["limitations"]
            .to_string()
            .contains("Actual model identity is unknown")
    );
    let mut missing = sealed.clone();
    missing["observations"][0]
        .as_object_mut()
        .unwrap()
        .remove("model_evidence");
    let summary = run("missing-model-v7.json", &missing);
    assert_eq!(summary["model_audit"]["records"][0]["status"], "missing");
    assert!(
        summary["limitations"]
            .to_string()
            .contains("lack archived model capture")
    );

    let mut changed = sealed.clone();
    changed["observations"][0]["model_evidence"]["capture"]["requested_model"] = json!("different");
    assert!(
        run("changed-model-v7.json", &changed)
            .to_string()
            .contains("sealed assignment")
    );
    std::fs::write(root.join("model-0.json"), b"changed").unwrap();
    assert!(
        run("drift-model-v7.json", &sealed)
            .to_string()
            .contains("length or digest differs")
    );
    std::fs::write(root.join("model-0.json"), &bytes).unwrap();

    let mut mismatched = sealed.clone();
    let different = serde_json::to_vec(&json!({"assignment_id":"other","captured_at":captured_at,
        "agent_version":"other","harness_digest":digest(b"other"),
        "requested_model":"other","reasoning_effort":"low","actual_model":null,
        "status":"unknown","unknown_reason":"unavailable"}))
    .unwrap();
    std::fs::write(root.join("model-0.json"), &different).unwrap();
    mismatched["observations"][0]["model_evidence"]["artifact"]["digest"] =
        json!(digest(&different));
    mismatched["observations"][0]["model_evidence"]["artifact"]["bytes"] = json!(different.len());
    assert!(
        run("mismatched-model-v7.json", &mismatched)
            .to_string()
            .contains("differs from the manifest claim")
    );
    std::fs::write(root.join("model-0.json"), &bytes).unwrap();

    let mut traversal = sealed.clone();
    traversal["observations"][0]["model_evidence"]["artifact"]["path"] = json!("../outside.json");
    assert!(
        run("traversal-model-v7.json", &traversal)
            .to_string()
            .contains("Path must stay inside")
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("model-0.json"), root.join("model-link.json"))
            .unwrap();
        let mut linked = sealed.clone();
        linked["observations"][0]["model_evidence"]["artifact"]["path"] = json!("model-link.json");
        assert!(
            run("linked-model-v7.json", &linked)
                .to_string()
                .contains("Symlinks are not valid")
        );
    }
    let mut oversized = sealed;
    oversized["observations"][0]["model_evidence"]["artifact"]["bytes"] = json!(64 * 1024 + 1);
    assert!(
        run("oversized-model-v7.json", &oversized)
            .to_string()
            .contains("model capture artifact")
    );
}
