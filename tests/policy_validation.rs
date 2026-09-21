mod common;
#[path = "common/evolution.rs"]
mod evolution;

use evolution::*;
use serde_json::json;

#[test]
fn protected_file_capacity_is_bounded_authorized_and_used_for_both_policies() {
    use qualitygate::{
        config::policy_acceptance::{ValidationSuite, validate_suite},
        domain::policy_evaluation::CaseKind,
    };
    let mut fixture = Fixture::new();
    let mut legacy = serde_json::to_value(&fixture.suite).unwrap();
    legacy["budget"]
        .as_object_mut()
        .unwrap()
        .remove("snapshot_max_file_mib");
    let legacy: ValidationSuite = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.budget.snapshot_max_file_mib, 2);
    for invalid in [0, 9] {
        fixture.suite.budget.snapshot_max_file_mib = invalid;
        assert!(validate_suite(&fixture.suite).is_err());
    }
    fixture.suite.budget.snapshot_max_file_mib = 2;
    std::fs::write(
        fixture.root.path().join("legacy.bin"),
        vec![b'x'; 3 * 1024 * 1024],
    )
    .unwrap();
    common::git(fixture.root.path(), &["add", "."]);
    common::git(
        fixture.root.path(),
        &["commit", "-qm", "unchanged historical resource"],
    );
    for case in &mut fixture.suite.cases {
        case.base = head(fixture.root.path());
        let content = if case.kind == CaseKind::Replay {
            "bad\r\n"
        } else {
            "good\n"
        };
        std::fs::write(
            fixture
                .root
                .path()
                .join(format!("capacity-{}.txt", case.id)),
            content,
        )
        .unwrap();
        common::git(fixture.root.path(), &["add", "."]);
        common::git(
            fixture.root.path(),
            &["commit", "-qm", "independent capacity case"],
        );
        case.head = head(fixture.root.path());
    }
    fixture.write_inputs();
    fixture.enable();
    let blocked = fixture.validate("1", 2);
    assert!(
        blocked["evaluation"]["reasons"]
            .to_string()
            .contains("File exceeds")
    );
    fixture.suite.budget.snapshot_max_file_mib = 3;
    // Changing the protected capacity without updating its external authorization is rejected.
    std::fs::write(
        fixture.external.path().join("suite.json"),
        serde_json::to_vec(&fixture.suite).unwrap(),
    )
    .unwrap();
    assert_eq!(fixture.validate("1", 2)["gate"]["complete"], false);
    fixture.write_inputs();
    let passed = fixture.validate("1", 0);
    assert_eq!(passed["evaluation"]["budget"]["snapshot_max_file_mib"], 3);
    assert!(
        passed["evaluation"]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case| {
                case["baseline"]["complete"] == true && case["candidate"]["complete"] == true
            })
    );
}

fn probe(directory: &std::path::Path) -> String {
    let source = directory.join("probe.rs");
    std::fs::write(&source, include_str!("common/policy_probe.rs")).unwrap();
    let executable = directory.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let argv = vec![
        "rustc".into(),
        "--edition=2024".into(),
        source.to_str().unwrap().into(),
        "-o".into(),
        executable.to_str().unwrap().into(),
    ];
    let output = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(qualitygate::runner::capture(
            &argv,
            directory,
            None,
            std::time::Duration::from_secs(30),
        ))
        .unwrap();
    assert_eq!(
        output.exit_code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable.to_str().unwrap().into()
}

#[test]
fn protected_library_growth_blocks_complete_execution_and_abandoned_attempts_stay_archived() {
    let mut fixture = Fixture::new();
    fixture.enable();
    run(
        fixture.root.path(),
        &[
            "policy",
            "candidate",
            "rules",
            "enable",
            &fixture.id,
            "test-naming",
            "--actor",
            "generator",
        ],
        0,
    );
    fixture.suite.max_rules = 1;
    fixture.suite.max_rule_growth = 1;
    for case in &mut fixture.suite.cases {
        case.expectations.insert(
            "test-naming".into(),
            qualitygate::domain::policy_evaluation::Expected::Skipped,
        );
    }
    fixture.write_inputs();
    let blocked = fixture.validate("4", 1);
    assert!(
        blocked["evaluation"]["reasons"]
            .to_string()
            .contains("contribution budget")
    );
    assert!(
        blocked["evaluation"]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case| case["candidate"]["complete"] == true)
    );
    fixture.new_candidate();
    let abandoned = run(
        fixture.root.path(),
        &[
            "policy",
            "candidate",
            "abandon",
            &fixture.id,
            "--actor",
            "generator",
            "--reason",
            "Superseded experiment",
        ],
        0,
    );
    assert_eq!(abandoned["revision"]["status"], "abandoned");
    let invalid = fixture.validate("4", 2);
    assert!(
        invalid["gate"]["blockers"]
            .to_string()
            .contains("immutable parent")
    );
    let listed = run(fixture.root.path(), &["policy", "candidate", "list"], 0);
    assert!(listed.to_string().contains("abandoned") && listed.to_string().contains("rejected"));
}

#[test]
fn paired_replay_and_held_out_oracles_preserve_negative_cases_and_reject_weakening() {
    let mut fixture = Fixture::new();
    let rejected = fixture.validate("2", 1);
    assert_eq!(rejected["evaluation"]["conclusion"], "block");
    assert_eq!(
        run(
            fixture.root.path(),
            &["policy", "candidate", "show", &fixture.id],
            0
        )["revision"]["status"],
        "rejected"
    );
    fixture.new_candidate();
    fixture.enable();
    let serial = fixture.validate("1", 0);
    assert_eq!(serial["evaluation"]["conclusion"], "pass");
    let parallel = fixture.validate("4", 0);
    assert_eq!(parallel["evaluation"]["conclusion"], "pass");
    assert_eq!(
        parallel["evaluation"]["budget"],
        serial["evaluation"]["budget"]
    );
    for (before, after) in serial["evaluation"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(parallel["evaluation"]["cases"].as_array().unwrap())
    {
        for field in ["id", "kind", "snapshot_digest", "task_digest"] {
            assert_eq!(before[field], after[field]);
        }
        assert_eq!(before["candidate"]["mismatches"], json!([]));
        assert_eq!(after["candidate"]["mismatches"], json!([]));
        assert!(
            !before["baseline"]["mismatches"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    assert_ne!(serial["evaluation_ref"], parallel["evaluation_ref"]);
    let effectiveness = run(fixture.root.path(), &["policy", "effectiveness"], 0);
    assert_eq!(effectiveness["groups"].as_array().unwrap().len(), 3);
    let parallel_group = effectiveness["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["inputs"]["jobs"] == 4)
        .unwrap();
    let measures = &parallel_group["observations"][0];
    assert_eq!(measures["update_quality"], "pass");
    assert_eq!(measures["oracle_improvement"], 3);
    assert!(measures["downstream_benefit"].is_null() && measures["context_tokens"].is_null());
    let page = run(
        fixture.root.path(),
        &["policy", "effectiveness", "--limit", "1"],
        0,
    );
    assert_eq!(page["next_offset"], 1);
    let archived = run(
        fixture.root.path(),
        &[
            "policy",
            "evaluation",
            parallel["evaluation_ref"].as_str().unwrap(),
        ],
        0,
    );
    assert_eq!(archived["evaluation"], parallel["evaluation"]);
    let store = qualitygate::config::policy_store::Store::open(fixture.root.path()).unwrap();
    for case in parallel["evaluation"]["cases"].as_array().unwrap() {
        let baseline: qualitygate::domain::Report = store
            .record(
                case["baseline"]["report_ref"].as_str().unwrap(),
                "check_report",
            )
            .unwrap();
        let candidate: qualitygate::domain::Report = store
            .record(
                case["candidate"]["report_ref"].as_str().unwrap(),
                "check_report",
            )
            .unwrap();
        assert_eq!(
            baseline.snapshot.content_digest,
            candidate.snapshot.content_digest
        );
        assert_eq!(
            baseline.policy.task_contract_digest,
            candidate.policy.task_contract_digest
        );
        assert_eq!(baseline.evaluator_digest, candidate.evaluator_digest);
        assert_eq!(baseline.environment_digest, candidate.environment_digest);
        if case["kind"] == "replay" {
            assert_eq!(candidate.gate.decision, qualitygate::domain::Decision::Fail);
        }
    }
}

#[test]
fn missing_tools_timeout_and_invalid_protected_suites_stay_incomplete() {
    let mut fixture = Fixture::new();
    fixture.enable();
    fixture.suite.cases[0].task.acceptance[0].verification.argv =
        vec!["qualitygate-deliberately-missing-producer".into()];
    fixture.write_inputs();
    let incomplete = fixture.validate("4", 2);
    assert_eq!(incomplete["evaluation"]["conclusion"], "incomplete");
    assert_eq!(
        incomplete["evaluation"]["cases"].as_array().unwrap().len(),
        3
    );
    assert!(
        incomplete["evaluation"]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|case| case["candidate"]["complete"] == false)
    );
    let native = probe(fixture.external.path());
    fixture.suite.cases[0].task.acceptance[0].verification.argv =
        vec![native, "sleep".into(), "3000".into()];
    fixture.suite.cases[0].task.acceptance[0]
        .verification
        .timeout_seconds = 1;
    fixture.write_inputs();
    let timeout = fixture.validate("1", 2);
    let store = qualitygate::config::policy_store::Store::open(fixture.root.path()).unwrap();
    let replay = timeout["evaluation"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "replay")
        .unwrap();
    let report: qualitygate::domain::Report = store
        .record(
            replay["candidate"]["report_ref"].as_str().unwrap(),
            "check_report",
        )
        .unwrap();
    assert_eq!(
        report
            .checks
            .iter()
            .find(|check| check.id == "probe")
            .unwrap()
            .execution
            .status,
        qualitygate::domain::ExecutionStatus::TimedOut
    );
    fixture.suite.cases[1].head = fixture.suite.cases[0].head.clone();
    fixture.suite.cases[1].base = fixture.suite.cases[0].base.clone();
    fixture.write_inputs();
    let rejected = fixture.validate("1", 2);
    assert!(
        rejected["gate"]["blockers"]
            .to_string()
            .contains("distinct")
    );
}

#[test]
fn native_barriers_prove_actual_parallel_execution_within_the_configured_job_limit() {
    let mut fixture = Fixture::new();
    fixture.enable();
    let native = probe(fixture.external.path());
    for case in &mut fixture.suite.cases {
        case.task.acceptance[0].verification.argv = vec![
            native.clone(),
            "barrier".into(),
            fixture
                .external
                .path()
                .join(&case.id)
                .to_str()
                .unwrap()
                .into(),
        ];
    }
    fixture.write_inputs();
    let parallel = fixture.validate("4", 0);
    assert_eq!(parallel["evaluation"]["jobs"], 4);
    let store = qualitygate::config::policy_store::Store::open(fixture.root.path()).unwrap();
    let mut events = Vec::new();
    for case in parallel["evaluation"]["cases"].as_array().unwrap() {
        for side in ["baseline", "candidate"] {
            let report: qualitygate::domain::Report = store
                .record(case[side]["report_ref"].as_str().unwrap(), "check_report")
                .unwrap();
            let command = &report
                .checks
                .iter()
                .find(|check| check.id == "probe")
                .unwrap()
                .execution;
            events.push((command.started_at_ms.unwrap(), 1_i32));
            events.push((command.ended_at_ms.unwrap(), -1));
        }
    }
    events.sort();
    let mut live = 0;
    let mut maximum = 0;
    for (_, delta) in events {
        live += delta;
        maximum = maximum.max(live);
    }
    assert!(
        (2..=4).contains(&maximum),
        "Observed concurrent commands: {maximum}"
    );
    assert_eq!(live, 0);
}

#[test]
fn modified_and_restored_protected_inputs_and_total_timeouts_cannot_be_approved() {
    let mut fixture = Fixture::new();
    fixture.enable();
    let native = probe(fixture.external.path());
    fixture.suite.cases[0].task.acceptance[0].verification.argv = vec![
        native.clone(),
        "restore".into(),
        fixture
            .external
            .path()
            .join("suite.json")
            .to_str()
            .unwrap()
            .into(),
    ];
    fixture.write_inputs();
    let modified = fixture.validate("1", 2);
    assert!(
        modified["evaluation"]["reasons"]
            .to_string()
            .contains("changed after validation began")
    );
    for case in &mut fixture.suite.cases {
        case.task.acceptance[0].verification.argv =
            vec![native.clone(), "sleep".into(), "3000".into()];
    }
    fixture.suite.budget.total_timeout_seconds = 1;
    fixture.write_inputs();
    let timeout = fixture.validate("4", 2);
    assert!(
        timeout["evaluation"]["reasons"]
            .to_string()
            .contains("total time budget")
    );
    assert!(
        timeout["evaluation"]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case| case["candidate"]["complete"] == false)
    );
}
