mod common;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

fn write(root: &Path, name: &str, value: &Value) {
    std::fs::write(root.join(name), serde_norway::to_string(value).unwrap()).unwrap();
}

fn selected(root: &Path, code: i32) -> Value {
    report(
        &cli(
            root,
            &[
                "check",
                "--policy-ref",
                "approved",
                "--task",
                "task.yaml",
                "--format",
                "json",
            ],
        ),
        code,
    )
}

#[test]
fn selected_policy_preserves_the_original_task_plan_after_candidate_deletion_or_corruption() {
    let root = fixture();
    let root = root.path();
    let config = json!({"schema_version":1,"rules":{"line-ending":{}},"verification_assets":["qualitygate.yaml", "task.yaml"]});
    let task = json!({"schema_version":1,"task_id":"retained-task","acceptance":[
        {"id":"first-condition","description":"First condition must hold","verification":{"check_id":"first-check","argv":["git","--version"]}},
        {"id":"second-condition","description":"Second condition must hold","verification":{"check_id":"second-check","argv":["git","--version"]}}
    ]});
    write(root, "qualitygate.yaml", &config);
    write(root, "task.yaml", &task);
    git(root, &["add", "."]);
    git(
        root,
        &["commit", "-qm", "approved policy and complete task"],
    );
    git(root, &["branch", "approved"]);
    let initial = selected(root, 0);
    assert_eq!(initial["plan"]["task_id"], "retained-task");
    assert_eq!(initial["policy"]["task_contract_source"], "task.yaml");
    assert_eq!(
        initial["policy"]["resolved_commit"],
        initial["snapshot"]["head"]
    );
    assert_eq!(
        initial["plan"]["acceptance_descriptions"]["second-condition"],
        "Second condition must hold"
    );
    let check_original = |changed: Value| {
        assert_eq!(changed["gate"]["decision"], "incomplete");
        assert_eq!(changed["plan"], initial["plan"]);
        assert_eq!(
            changed["policy"]["task_contract_digest"],
            initial["policy"]["task_contract_digest"]
        );
        assert_eq!(changed["checks"].as_array().unwrap().len(), 3);
        assert_eq!(
            changed["checks"][2]["metadata"]["command_definition"],
            initial["checks"][2]["metadata"]["command_definition"]
        );
        assert_eq!(
            changed["checks"][2]["metadata"]["command_definition"]["expected_exit_code"],
            0
        );
        assert!(
            changed["checks"]
                .as_array()
                .unwrap()
                .iter()
                .skip(1)
                .all(|check| check["execution"]["status"] == "blocked")
        );
        let paths = changed["policy"]["changes"].as_array().unwrap();
        assert_eq!(paths.len(), 1, "Each changed asset is reported once");
    };
    let mut shortened = task.clone();
    shortened["acceptance"].as_array_mut().unwrap().pop();
    write(root, "task.yaml", &shortened);
    check_original(selected(root, 2));
    for format in ["table", "markdown"] {
        let output = cli(
            root,
            &[
                "check",
                "--policy-ref",
                "approved",
                "--task",
                "task.yaml",
                "--format",
                format,
            ],
        );
        assert_eq!(output.status.code(), Some(2));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Task: retained-task"));
        assert!(text.contains("Second condition must hold"));
    }
    std::fs::write(root.join("task.yaml"), "invalid: [").unwrap();
    check_original(selected(root, 2));
    std::fs::remove_file(root.join("task.yaml")).unwrap();
    check_original(selected(root, 2));
    write(root, "task.yaml", &task);
    std::fs::remove_file(root.join("qualitygate.yaml")).unwrap();
    check_original(selected(root, 2));
    write(root, "qualitygate.yaml", &config);
    selected(root, 0);
}

#[test]
fn staged_tasks_and_executable_mode_changes_use_their_selected_inputs() {
    let root = fixture();
    let root = root.path();
    write(
        root,
        "qualitygate.yaml",
        &json!({"schema_version":1,"rules":{"line-ending":{}}}),
    );
    let task = json!({"schema_version":1,"task_id":"staged-task","acceptance":[
        {"id":"behavior","description":"Required behavior","verification":{"check_id":"behavior-test","argv":["git","--version"]}}
    ]});
    write(root, "task.yaml", &task);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "stage task"]);
    git(root, &["branch", "approved"]);
    let initial = selected(root, 0);
    std::fs::write(root.join("task.yaml"), "invalid: [").unwrap();
    let staged = report(
        &cli(
            root,
            &[
                "check",
                "--staged",
                "--task",
                "task.yaml",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(staged["plan"], initial["plan"]);
    assert_eq!(
        staged["policy"]["task_contract_digest"],
        initial["policy"]["task_contract_digest"]
    );
    assert_eq!(staged["policy"]["resolved_commit"], Value::Null);
    report(
        &cli(root, &["check", "--task", "task.yaml", "--format", "json"]),
        2,
    );
    write(root, "task.yaml", &task);
    git(root, &["update-index", "--chmod=+x", "task.yaml"]);
    let mode_changed = report(
        &cli(
            root,
            &[
                "check",
                "--staged",
                "--policy-ref",
                "approved",
                "--task",
                "task.yaml",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(mode_changed["policy"]["changes"], json!(["task.yaml"]));
    assert_eq!(mode_changed["plan"], initial["plan"]);
    git(root, &["update-index", "--chmod=-x", "task.yaml"]);
    git(root, &["update-index", "--chmod=+x", "qualitygate.yaml"]);
    let mode_changed = report(
        &cli(
            root,
            &[
                "check",
                "--staged",
                "--policy-ref",
                "approved",
                "--task",
                "task.yaml",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(
        mode_changed["policy"]["changes"],
        json!(["qualitygate.yaml"])
    );
}

#[test]
fn recheck_pins_the_selected_policy_commit_after_a_branch_moves() {
    let root = fixture();
    let root = root.path();
    let mut config = json!({"schema_version":1,"rules":{"line-ending":{"severity":"error"}}});
    write(root, "qualitygate.yaml", &config);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "strict policy"]);
    git(root, &["branch", "approved"]);
    std::fs::write(root.join("hello.txt"), "mixed\r\nline endings\n").unwrap();
    let initial = report(
        &cli(
            root,
            &["check", "--policy-ref", "approved", "--format", "json"],
        ),
        1,
    );
    let commit = initial["snapshot"]["head"].as_str().unwrap();
    assert_eq!(initial["policy"]["source"], "approved");
    assert_eq!(initial["policy"]["resolved_commit"], commit);
    let argv = initial["checks"][0]["diagnostics"][0]["recheck"]["argv"]
        .as_array()
        .unwrap();
    let argv: Vec<_> = argv.iter().map(|item| item.as_str().unwrap()).collect();
    let position = argv.iter().position(|arg| *arg == "--policy-ref").unwrap();
    assert_eq!(argv[position + 1], commit);
    config["rules"]["line-ending"]["severity"] = json!("warning");
    write(root, "qualitygate.yaml", &config);
    git(root, &["add", "qualitygate.yaml"]);
    git(root, &["commit", "-qm", "next policy"]);
    git(root, &["branch", "-f", "approved", "HEAD"]);
    report(
        &cli(
            root,
            &["check", "--policy-ref", "approved", "--format", "json"],
        ),
        0,
    );
    let repeated = report(&cli(root, &argv[3..]), 2);
    assert_eq!(repeated["policy"]["resolved_commit"], commit);
    assert_eq!(repeated["policy"]["changes"], json!(["qualitygate.yaml"]));
    assert_eq!(repeated["checks"][0]["verdict"], "fail");
    assert_eq!(repeated["checks"][0]["severity"], "error");
    let task = json!({"schema_version":1,"task_id":"unapproved","acceptance":[
        {"id":"new","description":"New condition","verification":{"check_id":"new-test","argv":["git","--version"]}}
    ]});
    write(root, "task.yaml", &task);
    let missing = selected(root, 2);
    assert!(
        missing
            .to_string()
            .contains("Task contract missing from selected policy snapshot")
    );
}
