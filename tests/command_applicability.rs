mod common;
#[path = "common/repository.rs"]
mod repository;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::path::Path;

fn write(root: &Path, name: &str, content: impl AsRef<[u8]>) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn commit(root: &Path) {
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "applicability fixture"]);
}

fn command(id: &str, when: Value) -> Value {
    json!({"id":id,"argv":["git","--version"],"when_changed":when})
}

fn policy(root: &Path, checks: Vec<Value>) -> Value {
    let mut all = vec![json!({"id":"unconditional","argv":["git","--version"]})];
    all.extend(checks);
    let policy = json!({"schema_version":1,"checks":all});
    write_policy(root, &policy);
    policy
}

fn write_policy(root: &Path, policy: &Value) {
    write(
        root,
        "qualitygate.yaml",
        serde_json::to_vec(policy).unwrap(),
    );
}

fn checked(root: &Path, args: &[&str], code: i32) -> Value {
    let mut command = vec!["check", "--format", "json"];
    command.extend_from_slice(args);
    report(&cli(root, &command), code)
}

fn check<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap()
}

fn ran(report: &Value, id: &str) {
    let check = check(report, id);
    assert_eq!(check["execution"]["status"], "completed", "{check}");
    assert_eq!(check["applicability"], "applicable", "{check}");
    assert_eq!(check["verdict"], "pass", "{check}");
    assert_eq!(check["execution"]["argv"], json!(["git", "--version"]));
    assert_eq!(check["metadata"]["change_selection"]["execute"], true);
}

fn skipped(report: &Value, id: &str) {
    let check = check(report, id);
    assert_eq!(check["execution"]["status"], "not_run", "{check}");
    assert_eq!(check["applicability"], "not_applicable", "{check}");
    assert_eq!(check["verdict"], "skipped", "{check}");
    assert!(
        !check["execution"]["reason"]
            .as_str()
            .unwrap()
            .trim()
            .is_empty()
    );
    assert!(
        check["execution"]["artifacts"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(check["metadata"]["change_selection"].is_object(), "{check}");
    assert_eq!(check["metadata"]["change_selection"]["execute"], false);
}

#[test]
fn languages_and_paths_intersect_while_explicit_inputs_trigger_independently() {
    let root = fixture();
    let root = root.path();
    policy(
        root,
        vec![
            command(
                "backend",
                json!({"languages":["rust"],"paths":["backend/**"]}),
            ),
            command(
                "frontend",
                json!({"languages":["typescript"],"paths":["web/**"]}),
            ),
            command(
                "wrong-project",
                json!({"languages":["rust"],"paths":["other/**"]}),
            ),
            command("schema", json!({"inputs":["schema/*.json"]})),
            command(
                "schema-consumer",
                json!({"languages":["python"],"paths":["worker/**"],"inputs":["schema/*.json"]}),
            ),
        ],
    );
    write(root, "backend/lib.rs", "fn original() {}\n");
    write(root, "web/README.md", "original\n");
    write(root, "schema/api.json", "{}\n");
    commit(root);
    write(root, "backend/lib.rs", "fn changed() {}\n");
    write(root, "web/README.md", "changed\n");
    let rust = checked(root, &[], 0);
    ran(&rust, "unconditional");
    ran(&rust, "backend");
    for id in ["frontend", "wrong-project", "schema", "schema-consumer"] {
        skipped(&rust, id);
    }
    git(root, &["restore", "backend/lib.rs"]);
    write(root, "schema/api.json", "{\"changed\":true}\n");
    let schema = checked(root, &[], 0);
    ran(&schema, "schema");
    ran(&schema, "schema-consumer");
    for id in ["backend", "frontend", "wrong-project"] {
        skipped(&schema, id);
    }
}

#[test]
fn skipping_precedes_environment_tool_and_cwd_requirements() {
    let root = fixture();
    let root = root.path();
    let mut unavailable = command("python", json!({"languages":["python"]}));
    unavailable["argv"] = json!(["qualitygate-applicability-no-such-executable"]);
    unavailable["cwd"] = json!("absent-project");
    unavailable["required_env"] = json!(["QUALITYGATE_APPLICABILITY_TEST_ABSENT_50DFB87C"]);
    unavailable["tools"] =
        json!([{"id":"missing","argv":["qualitygate-applicability-no-such-tool"]}]);
    policy(root, vec![unavailable]);
    commit(root);
    write(root, "readme.md", "documentation only\n");
    let checked = checked(root, &[], 0);
    skipped(&checked, "python");
    assert_eq!(checked["gate"]["complete"], true);
    let doctor = report(
        &cli(
            root,
            &["doctor", "--worktree", "--probe-tools", "--format", "json"],
        ),
        0,
    );
    assert_eq!(doctor["complete"], true);
    assert!(
        doctor["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["id"] != "python")
    );
    assert!(
        doctor["not_executed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["check"] == "python"
                    && item["reason"].as_str().unwrap().contains("when_changed")
            })
    );
}

#[test]
fn deletion_rename_and_previous_shebang_still_trigger_the_original_language() {
    let root = fixture();
    let root = root.path();
    policy(
        root,
        vec![
            command("old-rust", json!({"languages":["rust"],"paths":["old/**"]})),
            command("destination", json!({"paths":["new/**"]})),
            command("deleted-python", json!({"languages":["python"]})),
            command("shell", json!({"languages":["shell"]})),
        ],
    );
    write(root, "old/lib.rs", "fn moved() {}\n");
    write(root, "worker.py", "print('old')\n");
    write(root, "bin/run", "#!/bin/sh\necho old\n");
    commit(root);
    std::fs::create_dir(root.join("new")).unwrap();
    std::fs::rename(root.join("old/lib.rs"), root.join("new/copied.txt")).unwrap();
    std::fs::remove_file(root.join("worker.py")).unwrap();
    write(root, "bin/run", "plain text now\n");
    let checked = checked(root, &[], 0);
    for id in ["old-rust", "destination", "deleted-python", "shell"] {
        ran(&checked, id);
    }
}

#[test]
fn c_headers_activate_both_c_and_cpp_consumers() {
    let root = fixture();
    let root = root.path();
    policy(
        root,
        vec![
            command("c", json!({"languages":["c"]})),
            command("cpp", json!({"languages":["cpp"]})),
            command("rust", json!({"languages":["rust"]})),
        ],
    );
    write(root, "include/shared.h", "int original(void);\n");
    commit(root);
    write(root, "include/shared.h", "int changed(void);\n");
    let checked = checked(root, &[], 0);
    ran(&checked, "c");
    ran(&checked, "cpp");
    skipped(&checked, "rust");
}

#[test]
fn binary_source_changes_trigger_commands_without_added_lines() {
    let root = fixture();
    let root = root.path();
    policy(root, vec![command("rust", json!({"languages":["rust"]}))]);
    write(root, "lib.rs", b"old\0content\n");
    commit(root);
    write(root, "lib.rs", b"new\0content\n");
    let checked = checked(root, &[], 0);
    assert_eq!(checked["selection"]["changed_lines"], 0);
    assert_eq!(checked["selection"]["changed_files"], json!(["lib.rs"]));
    ran(&checked, "rust");
}

#[cfg(unix)]
#[test]
fn permission_only_source_changes_trigger_commands_without_added_lines() {
    use std::os::unix::fs::PermissionsExt;

    let root = fixture();
    let root = root.path();
    policy(root, vec![command("rust", json!({"languages":["rust"]}))]);
    write(root, "lib.rs", "fn unchanged() {}\n");
    std::fs::set_permissions(root.join("lib.rs"), std::fs::Permissions::from_mode(0o644)).unwrap();
    git(root, &["config", "core.filemode", "true"]);
    commit(root);
    std::fs::set_permissions(root.join("lib.rs"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let checked = checked(root, &[], 0);
    assert_eq!(checked["selection"]["changed_lines"], 0);
    assert_eq!(checked["selection"]["changed_files"], json!(["lib.rs"]));
    ran(&checked, "rust");
}

#[test]
fn staged_and_diff_scheduling_follow_selected_bytes_and_path_only_filters_feedback() {
    let root = fixture();
    let root = root.path();
    policy(
        root,
        vec![
            command("rust", json!({"languages":["rust"]})),
            command("python", json!({"languages":["python"]})),
        ],
    );
    write(root, "lib.rs", "fn original() {}\n");
    write(root, "worker.py", "print('original')\n");
    commit(root);
    write(root, "lib.rs", "fn staged() {}\n");
    git(root, &["add", "lib.rs"]);
    write(root, "lib.rs", "fn original() {}\n");
    write(root, "worker.py", "print('changed')\n");
    let staged = checked(root, &["--staged"], 0);
    ran(&staged, "rust");
    skipped(&staged, "python");
    let worktree = checked(root, &["--worktree", "--path", "lib.rs"], 0);
    skipped(&worktree, "rust");
    ran(&worktree, "python");
    commit(root);
    let diff = checked(root, &["--diff", "HEAD~1..HEAD"], 0);
    skipped(&diff, "rust");
    ran(&diff, "python");
}

#[test]
fn active_commands_and_rules_force_their_transitive_command_dependencies() {
    let root = fixture();
    let root = root.path();
    let leaf = command("leaf", json!({"languages":["rust"]}));
    let mut middle = command("middle", json!({"languages":["python"]}));
    middle["depends_on"] = json!(["leaf"]);
    let mut consumer = command("consumer", json!({"paths":["hello.txt"]}));
    consumer["depends_on"] = json!(["middle"]);
    let producer = command("rule-producer", json!({"languages":["go"]}));
    let mut config = policy(root, vec![leaf, middle, consumer, producer]);
    config["rules"] = json!({"line-ending":{"depends_on":["rule-producer"]}});
    write_policy(root, &config);
    commit(root);
    write(root, "hello.txt", "changed\n");
    let checked = checked(root, &[], 0);
    for id in ["leaf", "middle", "consumer", "rule-producer"] {
        ran(&checked, id);
    }
    assert_eq!(check(&checked, "line-ending")["verdict"], "pass");
}

#[test]
fn task_acceptance_always_executes_and_forces_its_scoped_prerequisites() {
    let root = fixture();
    let root = root.path();
    policy(
        root,
        vec![command("producer", json!({"languages":["rust"]}))],
    );
    write(
        root,
        "task.yaml",
        json!({"schema_version":1,"task_id":"task","acceptance":[{
            "id":"requirement","description":"Task acceptance always executes","verification":{
                "check_id":"acceptance","argv":["git","--version"],"depends_on":["producer"]
            }
        }]})
        .to_string(),
    );
    commit(root);
    write(root, "readme.md", "documentation only\n");
    let checked = checked(root, &["--task", "task.yaml"], 0);
    ran(&checked, "producer");
    ran(&checked, "acceptance");
    assert_eq!(
        check(&checked, "producer")["metadata"]["change_selection"]["required_by"],
        "acceptance"
    );
}

#[test]
fn policy_manifest_lock_and_verification_changes_force_checks() {
    for path in [
        "qualitygate.yaml",
        "Cargo.toml",
        "subproject/package-lock.json",
        "verification/schema.txt",
        "tools/probe.txt",
    ] {
        let root = fixture();
        let root = root.path();
        let mut selected = command("python", json!({"languages":["python"]}));
        selected["tools"] =
            json!([{"id":"git","argv":["git","--version"],"inputs":["tools/probe.txt"]}]);
        let mut config = policy(root, vec![selected]);
        config["verification_assets"] = json!(["verification/**"]);
        write_policy(root, &config);
        write(root, "Cargo.toml", "[workspace]\n");
        write(root, "subproject/package-lock.json", "{}\n");
        write(root, "verification/schema.txt", "old\n");
        write(root, "tools/probe.txt", "old\n");
        commit(root);
        if path == "qualitygate.yaml" {
            let mut bytes = std::fs::read(root.join(path)).unwrap();
            bytes.push(b'\n');
            write(root, path, bytes);
        } else {
            write(root, path, "changed\n");
        }
        let checked = checked(root, &[], 0);
        ran(&checked, "python");
    }
}

#[test]
fn repository_scope_runs_checks_and_empty_delivery_retains_auditable_skips() {
    let root = fixture();
    let root = root.path();
    policy(root, vec![command("rust", json!({"languages":["rust"]}))]);
    commit(root);
    let delivery = checked(root, &[], 0);
    assert_eq!(delivery["selection"]["empty_delivery"], true);
    ran(&delivery, "unconditional");
    skipped(&delivery, "rust");
    let repository = report(&repository::check(root, &[]), 0);
    ran(&repository, "rust");
}

#[test]
fn all_skipped_checks_do_not_claim_usable_verification() {
    let root = fixture();
    let root = root.path();
    write_policy(
        root,
        &json!({
            "schema_version":1,
            "checks":[command("rust", json!({"languages":["rust"]}))]
        }),
    );
    commit(root);
    write(root, "readme.md", "documentation only\n");
    let checked = checked(root, &[], 2);
    skipped(&checked, "rust");
    assert_eq!(checked["gate"]["complete"], false);
    assert!(
        checked["gate"]["blockers"]
            .as_array()
            .unwrap()
            .contains(&json!("No check produced a usable result"))
    );
}

#[test]
fn malformed_selectors_and_manual_or_task_selectors_are_rejected() {
    for selector in [
        json!({}),
        json!({"languages":["not-a-language"]}),
        json!({"languages":["rust","rust"]}),
        json!({"paths":["../outside"]}),
        json!({"paths":["/absolute"]}),
        json!({"inputs":["[invalid"]}),
        json!({"unknown":["rust"]}),
    ] {
        let root = fixture();
        policy(root.path(), vec![command("invalid", selector)]);
        let invalid = checked(root.path(), &[], 2);
        assert_eq!(invalid["gate"]["complete"], false);
    }
    let root = fixture();
    policy(
        root.path(),
        vec![json!({"id":"manual","kind":"manual","when_changed":{"languages":["rust"]}})],
    );
    assert_eq!(checked(root.path(), &[], 2)["gate"]["complete"], false);
    policy(root.path(), Vec::new());
    write(
        root.path(),
        "task.yaml",
        json!({"schema_version":1,"task_id":"task","acceptance":[{
            "id":"requirement","description":"Task acceptance always executes","verification":{
                "check_id":"acceptance","argv":["git","--version"],"when_changed":{"languages":["rust"]}
            }
        }]}).to_string(),
    );
    assert_eq!(
        checked(root.path(), &["--task", "task.yaml"], 2)["gate"]["complete"],
        false
    );
}

#[test]
fn applicable_failures_missing_tools_and_missing_reports_do_not_become_skips() {
    let root = fixture();
    let root = root.path();
    let mut failing = command("failure", json!({"languages":["rust"]}));
    failing["argv"] = json!(["git", "qualitygate-applicability-no-such-subcommand"]);
    let mut absent = command("absent", json!({"languages":["rust"]}));
    absent["argv"] = json!(["qualitygate-applicability-no-such-executable"]);
    let mut evidence = command("evidence", json!({"languages":["rust"]}));
    evidence["tools"] = json!([{"id":"git","argv":["git","--version"]}]);
    evidence["reports"] = json!([{"path":"missing.json","format":"diagnostics"}]);
    policy(root, vec![failing, absent, evidence]);
    commit(root);
    write(root, "lib.rs", "fn changed() {}\n");
    let checked = checked(root, &[], 2);
    assert_eq!(check(&checked, "failure")["verdict"], "fail");
    assert_eq!(
        check(&checked, "absent")["execution"]["status"],
        "tool_error"
    );
    assert_eq!(check(&checked, "evidence")["verdict"], Value::Null);
    assert_eq!(checked["gate"]["complete"], false);
}

#[test]
fn mismatched_policy_reference_cannot_turn_unrelated_commands_into_successful_skips() {
    let root = fixture();
    let root = root.path();
    let mut config = policy(root, vec![command("rust", json!({"languages":["rust"]}))]);
    commit(root);
    config["checks"][1]["when_changed"] = json!({"languages":["python"]});
    write_policy(root, &config);
    let checked = checked(root, &["--policy-ref", "HEAD"], 2);
    assert_eq!(checked["gate"]["complete"], false);
    assert_ne!(check(&checked, "rust")["verdict"], "skipped");
}

#[cfg(unix)]
#[test]
fn applicable_timeout_stays_incomplete() {
    let root = fixture();
    let root = root.path();
    let mut slow = command("slow", json!({"languages":["rust"]}));
    slow["argv"] = json!(["sh", "-c", "printf started; sleep 30"]);
    slow["timeout_seconds"] = json!(1);
    policy(root, vec![slow]);
    commit(root);
    write(root, "lib.rs", "fn changed() {}\n");
    let checked = checked(root, &[], 2);
    assert_eq!(check(&checked, "slow")["execution"]["status"], "timed_out");
    assert_eq!(check(&checked, "slow")["verdict"], Value::Null);
    assert_eq!(checked["gate"]["complete"], false);
}

#[cfg(unix)]
#[test]
fn source_changes_after_scheduling_invalidate_even_previously_skipped_checks() {
    let root = fixture();
    let root = root.path();
    let mut config = policy(root, vec![command("rust", json!({"languages":["rust"]}))]);
    // This fixture deliberately changes the isolated original repository while
    // the command runs from its immutable materialization, using an argv path.
    config["checks"][0]["argv"] = json!([
        "sh",
        "-c",
        "printf 'fn appeared() {}\\n' > \"$1/lib.rs\"",
        "fixture",
        root
    ]);
    write_policy(root, &config);
    commit(root);
    write(root, "readme.md", "documentation only\n");
    let checked = checked(root, &[], 2);
    skipped(&checked, "rust");
    assert_eq!(checked["gate"]["complete"], false);
    assert!(
        checked["gate"]["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason.as_str().unwrap().contains("Source snapshot changed"))
    );
}
