mod common;
use common::*;
use qualitygate::{
    domain::{Artifact, Report, pilot::tool_inventory_digest},
    snapshot::digest,
};
use serde_json::{Value, json};
use std::path::Path;

fn prepare(root: &Path) -> (Value, Artifact) {
    std::fs::write(root.join("qualitygate.yaml"), "schema_version: 1\n").unwrap();
    std::fs::write(root.join("task.yaml"),"schema_version: 1\ntask_id: observation-test\nacceptance:\n  - id: producer\n    description: An actual producer is invoked for this fixture\n    verification:\n      check_id: probe\n      argv: [git, --version]\n      timeout_seconds: 10\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "trusted task"]);
    std::fs::write(root.join("hello.txt"), "changed\n").unwrap();
    let value = report(
        &cli(
            root,
            &[
                "check",
                "--task",
                "task.yaml",
                "--policy-ref",
                "HEAD",
                "--format",
                "json",
            ],
        ),
        0,
    );
    let full: Report = serde_json::from_value(value.clone()).unwrap();
    let bytes = serde_json::to_vec(&value).unwrap();
    std::fs::write(root.join("full.json"), &bytes).unwrap();
    let artifact = Artifact {
        path: "full.json".into(),
        digest: digest(&bytes),
        bytes: bytes.len() as u64,
    };
    let mut m: Value =
        serde_json::from_str(include_str!("../templates/pilot/observation-v1.json")).unwrap();
    m["assignments"] = json!([{"id":"run-1","input_id":"input-1","task_id":"observation-test","task_kind":"bug_fix","origin":"injected",
        "cohort":{"agent_version":"fixture","harness_digest":digest(b"fixture"),"requested_model":"fixture-model","actual_model":null,
            "reasoning_effort":"medium","workflow":"qualitygate","environment_digest":full.environment_digest,"tools_digest":tool_inventory_digest(&full),
            "cache":"unknown","permissions":"fixture"},"base":full.snapshot.base,"initial_snapshot":digest(b"prior"),
        "config_digest":full.policy.config_digest,"task_digest":full.policy.task_contract_digest,"required_checks":full.plan.required_checks,
        "expected_issues":null,"eligible_repair":true,"exclusion":null}]);
    m["observations"] = json!([{"assignment_id":"run-1","observed_at":1,"attempts":[{"number":1,"status":"completed","elapsed_ms":1000,
        "check_elapsed_ms":100,"profile":"full","snapshot_digest":full.snapshot.content_digest,"report":artifact,"cost":null}],
        "findings":[],"review_active_ms":null,"review_comments":null,"rework_rounds":null}]);
    (m, artifact)
}
fn run(root: &Path, m: &Value, code: i32) -> Value {
    std::fs::write(
        root.join("observations.json"),
        serde_json::to_vec(m).unwrap(),
    )
    .unwrap();
    report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "observations.json",
                "--format",
                "json",
            ],
        ),
        code,
    )
}

#[test]
fn cli_verifies_actual_report_bytes_and_keeps_missing_assigned_runs() {
    let repo = fixture();
    let root = repo.path();
    let (mut m, artifact) = prepare(root);
    let s = run(root, &m, 0);
    assert_eq!(s["complete"], true);
    assert_eq!(s["protocol_ready"], false);
    assert_eq!(s["groups"][0]["accepted"], 1);
    assert!(s["groups"][0]["cost"]["total_micros"].is_null());
    let mut second = m["assignments"][0].clone();
    second["id"] = json!("not-observed");
    m["assignments"].as_array_mut().unwrap().push(second);
    let s = run(root, &m, 2);
    assert_eq!(s["groups"][0]["assigned"], 2);
    assert_eq!(s["groups"][0]["repair"]["denominator"], 2);
    m["assignments"].as_array_mut().unwrap().pop();
    std::fs::write(root.join(&artifact.path), b"{}").unwrap();
    let s = run(root, &m, 2);
    assert!(s["groups"][0]["repair"]["value"].is_null());
    assert!(s.to_string().contains("digest differs"));
    std::fs::remove_file(root.join(&artifact.path)).unwrap();
    assert_eq!(run(root, &m, 2)["complete"], false);
    m["observations"][0]["attempts"][0]["report"]["path"] = json!("../outside.json");
    assert!(
        run(root, &m, 2)
            .to_string()
            .contains("Path must stay inside")
    );
}

#[test]
fn strict_loader_bounds_inventory_and_rejects_malformed_artifacts_and_claims() {
    let repo = fixture();
    let root = repo.path();
    let (mut m, _) = prepare(root);
    m["unexpected"] = json!(true);
    let output = run(root, &m, 2);
    assert_eq!(output["gate"]["decision"], "incomplete");
    m.as_object_mut().unwrap().remove("unexpected");
    std::fs::write(root.join("bad.json"), b"{}").unwrap();
    m["observations"][0]["attempts"][0]["report"] =
        json!({"path":"bad.json","digest":digest(b"{}"),"bytes":2});
    assert!(run(root, &m, 2).to_string().contains("Invalid full report"));
    for i in 0..5 {
        let mut attempt = m["observations"][0]["attempts"][0].clone();
        attempt["number"] = json!(i + 1);
        attempt["report"] =
            json!({"path":"missing.json","digest":format!("sha256:{i:064x}"),"bytes":16*1024*1024});
        if i == 0 {
            m["observations"][0]["attempts"] = json!([attempt]);
        } else {
            m["observations"][0]["attempts"]
                .as_array_mut()
                .unwrap()
                .push(attempt);
        }
    }
    assert_eq!(run(root, &m, 2)["gate"]["decision"], "incomplete");
    std::fs::write(root.join("oversize.json"), vec![b' '; 1024 * 1024 + 1]).unwrap();
    report(
        &cli(
            root,
            &[
                "pilot",
                "summarize",
                "--input",
                "oversize.json",
                "--format",
                "json",
            ],
        ),
        2,
    );
}

#[cfg(unix)]
#[test]
fn report_symlinks_are_incomplete_instead_of_followed() {
    let repo = fixture();
    let root = repo.path();
    let (mut m, _) = prepare(root);
    std::os::unix::fs::symlink(root.join("full.json"), root.join("alias.json")).unwrap();
    m["observations"][0]["attempts"][0]["report"]["path"] = json!("alias.json");
    let s = run(root, &m, 2);
    assert!(s.to_string().contains("Symlinks are not valid"));
}
