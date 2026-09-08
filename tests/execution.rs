mod common;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

fn policy(root: &Path, checks: Value) {
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&json!({"schema_version":1,"checks":checks})).unwrap(),
    )
    .unwrap();
}

#[test]
fn executed_tools_lock_inputs_timestamps_and_distinct_artifacts_are_reported() {
    let root = fixture();
    std::fs::write(root.path().join("Cargo.lock"), "fixture lock input\n").unwrap();
    policy(
        root.path(),
        json!([
            {"id":"A","argv":["git","--version"],"tools":[{"id":"git","argv":["git","--version"]}]},
            {"id":"a","argv":["git","--exec-path"],"tools":[{"id":"git","argv":["git","--version"]}]}
        ]),
    );
    let value = report(&cli(root.path(), &["check", "--format", "json"]), 0);
    let first = &value["checks"][0];
    assert!(
        first["metadata"]["tools"][0]["version"]
            .as_str()
            .unwrap()
            .starts_with("git version ")
    );
    assert_eq!(
        first["metadata"]["tools"][0]["snapshot_digest"],
        value["snapshot"]["content_digest"]
    );
    assert_eq!(
        first["metadata"]["command_executable"]["digest"],
        first["metadata"]["tools"][0]["executable"]["digest"]
    );
    assert_eq!(
        first["metadata"]["environment"]["dependency_inputs"]["Cargo.lock"],
        qualitygate::snapshot::digest(b"fixture lock input\n")
    );
    assert!(first["metadata"]["environment"]["os"].is_string());
    assert!(
        first["execution"]["ended_at_ms"].as_u64().unwrap()
            >= first["execution"]["started_at_ms"].as_u64().unwrap()
    );
    let first_path = first["execution"]["artifacts"][0]["path"].as_str().unwrap();
    let second_path = value["checks"][1]["execution"]["artifacts"][0]["path"]
        .as_str()
        .unwrap();
    assert_ne!(
        first_path.to_ascii_lowercase(),
        second_path.to_ascii_lowercase()
    );
    assert_ne!(
        std::fs::read(first_path).unwrap(),
        std::fs::read(second_path).unwrap()
    );
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn command(id: &str, script: &str) -> Value {
        json!({"id":id,"argv":["sh","-c",script]})
    }

    #[test]
    fn absent_report_versions_are_visible_and_follow_check_requiredness() {
        let root = fixture();
        for (required, code) in [(true, 2), (false, 0)] {
            let mut check = command("report", "printf should-not-run");
            check["required"] = json!(required);
            check["reports"] = json!([{"path":"report.json","format":"diagnostics"}]);
            policy(root.path(), json!([command("valid", "true"), check]));
            let value = report(&cli(root.path(), &["check", "--format", "json"]), code);
            assert_eq!(value["checks"][0]["verdict"], "pass");
            assert_eq!(value["checks"][1]["execution"]["status"], "blocked");
            assert!(value["checks"][1]["execution"]["started_at_ms"].is_null());
            assert!(
                value["checks"][1]["execution"]["reason"]
                    .as_str()
                    .unwrap()
                    .contains("version probes")
            );
        }
    }

    fn quote(text: &str) -> String {
        format!("'{}'", text.replace('\'', "'\\''"))
    }

    fn analyzer(root: &Path, script: &str, findings: &[i32]) {
        std::fs::write(root.join("analyze.sh"), script).unwrap();
        policy(
            root,
            json!([{
                "id":"analyze","argv":["sh","analyze.sh"],"findings_exit_codes":findings,
                "tools":[{"id":"fixture","argv":["sh","-c","printf fixture-v1"],"inputs":["analyze.sh"]}],
                "reports":[{"path":"report.json","format":"diagnostics","mode":"new_diagnostics","baseline":"report.json"}]
            }]),
        );
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "analyzer baseline"]);
        std::fs::write(root.join("hello.txt"), "current\n").unwrap();
    }

    #[test]
    fn mutating_commands_invalidate_evidence_and_cannot_be_repaired_by_later_commands() {
        let root = fixture();
        policy(
            root.path(),
            json!([
                command("mutate","printf changed > hello.txt"),
                command("restore","printf 'initial\n' > hello.txt"),
                {"id":"dependent","argv":["sh","-c","printf ran"],"depends_on":["mutate"]}
            ]),
        );
        let value = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        assert_eq!(
            value["checks"][0]["metadata"]["input_integrity"]["after"],
            "invalid"
        );
        assert_eq!(
            value["checks"][1]["metadata"]["input_integrity"]["before"],
            "invalid"
        );
        assert!(value["checks"][1]["execution"]["started_at_ms"].is_null());
        assert!(
            value["checks"][2]["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains("Prerequisite")
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("hello.txt")).unwrap(),
            "initial\n"
        );
    }

    #[test]
    fn restored_bytes_mode_changes_and_symlinks_do_not_hide_input_mutations() {
        for script in [
            "cp hello.txt saved; printf changed > hello.txt; cp saved hello.txt",
            "chmod +x hello.txt",
            "mv hello.txt saved; ln -s saved hello.txt",
        ] {
            let root = fixture();
            policy(root.path(), json!([command("mutate", script)]));
            let value = report(&cli(root.path(), &["check", "--format", "json"]), 2);
            assert_eq!(
                value["checks"][0]["metadata"]["input_integrity"]["after"],
                "invalid"
            );
            assert_eq!(value["checks"][0]["verdict"], Value::Null);
        }
    }

    #[test]
    fn baseline_input_mutation_and_unexpected_exit_codes_cannot_pass_with_valid_reports() {
        for (effect, expected) in [
            ("printf changed > hello.txt", "Baseline command changed"),
            ("exit 7", "unexpected exit code"),
        ] {
            let root = fixture();
            analyzer(
                root.path(),
                &format!(
                    "printf '%s' '{{\"issues\":[]}}' > report.json\nif test \"$(cat hello.txt)\" = initial; then {effect}; fi\n"
                ),
                &[],
            );
            let value = report(&cli(root.path(), &["check", "--format", "json"]), 2);
            let check = &value["checks"][0];
            assert!(
                check["execution"]["reason"]
                    .as_str()
                    .unwrap()
                    .contains(expected),
                "{value}"
            );
            assert!(check["metadata"]["baseline_execution"]["ended_at_ms"].is_number());
            assert_eq!(check["metadata"]["input_integrity"]["after"], "verified");
        }
    }

    #[test]
    fn declared_findings_exit_codes_allow_comparable_historical_diagnostics_to_be_filtered() {
        let root = fixture();
        analyzer(
            root.path(),
            "printf '%s' '{\"issues\":[{\"rule\":\"old\",\"file\":\"hello.txt\",\"line\":1,\"message\":\"historical\"}]}' > report.json\nexit 1\n",
            &[1],
        );
        let value = report(&cli(root.path(), &["check", "--format", "json"]), 0);
        assert_eq!(value["checks"][0]["execution"]["exit_code"], 1);
        assert_eq!(
            value["checks"][0]["metadata"]["baseline_execution"]["exit_code"],
            1
        );
        assert_eq!(value["checks"][0]["metadata"]["report.json:filtered"], 1);
        assert_eq!(
            value["checks"][0]["metadata"]["baseline_input_integrity"]["status"],
            "verified"
        );
    }

    #[test]
    fn analyzer_exit_codes_need_consistent_report_evidence() {
        for (issues, exit, expected) in [
            (json!([]), 1, 2),
            (
                json!([{"rule":"issue","file":"hello.txt","line":1,"message":"violation"}]),
                1,
                1,
            ),
            (
                json!([{"rule":"issue","file":"hello.txt","line":1,"message":"violation"}]),
                7,
                2,
            ),
        ] {
            let root = fixture();
            let data = json!({"issues":issues}).to_string();
            let script = format!("printf '%s' {} > report.json; exit {exit}", quote(&data));
            let mut check = command("analyze", &script);
            check["findings_exit_codes"] = json!([1]);
            check["tools"] = json!([{"id":"fixture","argv":["sh","-c","printf fixture-v1"]}]);
            check["reports"] = json!([{"path":"report.json","format":"diagnostics"}]);
            policy(root.path(), json!([check]));
            let value = report(&cli(root.path(), &["check", "--format", "json"]), expected);
            if expected == 2 {
                assert_eq!(value["checks"][0]["execution"]["status"], "tool_error");
            }
        }
    }

    #[test]
    fn baseline_versions_and_declared_analyzer_inputs_must_match() {
        let root = fixture();
        analyzer(
            root.path(),
            "printf '%s' '{\"issues\":[]}' > report.json\n",
            &[],
        );
        std::fs::write(
            root.path().join("analyze.sh"),
            "printf '%s' '{\"issues\":[]}' > report.json\n# different analyzer\n",
        )
        .unwrap();
        let value = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        assert!(
            value["checks"][0]["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains("tool inputs differ")
        );

        let root = fixture();
        std::fs::write(root.path().join("version.txt"), "v1\n").unwrap();
        analyzer(
            root.path(),
            "printf '%s' '{\"issues\":[]}' > report.json\n",
            &[],
        );
        let mut config: Value = serde_norway::from_str(
            &std::fs::read_to_string(root.path().join("qualitygate.yaml")).unwrap(),
        )
        .unwrap();
        config["checks"][0]["tools"][0]["argv"] = json!(["cat", "version.txt"]);
        std::fs::write(
            root.path().join("qualitygate.yaml"),
            serde_norway::to_string(&config).unwrap(),
        )
        .unwrap();
        std::fs::write(root.path().join("version.txt"), "v2\n").unwrap();
        let value = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        assert_eq!(value["checks"][0]["metadata"]["tools"][0]["version"], "v2");
        assert_eq!(
            value["checks"][0]["metadata"]["baseline_tools"][0]["version"],
            "v1"
        );
    }

    #[test]
    fn invalid_or_timed_out_version_probes_keep_logs_and_do_not_execute_the_check() {
        for version in [
            "true",
            "printf version; exit 7",
            "printf '\\377'",
            "head -c 65537 /dev/zero",
            "printf partial; sleep 30",
        ] {
            let root = fixture();
            let mut check = command("build", "printf should-not-run");
            check["tools"] =
                json!([{"id":"version","argv":["sh","-c",version],"timeout_seconds":1}]);
            policy(root.path(), json!([check]));
            let value = report(&cli(root.path(), &["check", "--format", "json"]), 2);
            let check = &value["checks"][0];
            assert_eq!(check["execution"]["status"], "tool_error", "{value}");
            assert!(check["execution"]["started_at_ms"].is_null());
            assert!(
                Path::new(
                    check["metadata"]["tools"][0]["stdout"]["path"]
                        .as_str()
                        .unwrap()
                )
                .is_file()
            );
        }
    }

    #[test]
    fn missing_tool_inputs_and_mutating_version_probes_are_incomplete() {
        let root = fixture();
        let mut check = command("build", "printf should-not-run");
        check["tools"] =
            json!([{"id":"version","argv":["sh","-c","printf version"],"inputs":["absent.jar"]}]);
        policy(root.path(), json!([check]));
        let missing = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        assert!(
            missing["checks"][0]["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains("missing from checked snapshot")
        );
        let mut check = command("build", "printf should-not-run");
        check["tools"] = json!([{"id":"version","argv":["sh","-c","printf changed > hello.txt; printf version"]}]);
        policy(root.path(), json!([check]));
        let changed = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        assert!(changed["checks"][0]["execution"]["started_at_ms"].is_null());
        assert_eq!(
            changed["checks"][0]["metadata"]["input_integrity"]["after"],
            "invalid"
        );
    }

    #[test]
    fn changed_external_tools_and_oversized_executables_are_rejected() {
        let root = fixture();
        let external = tempfile::tempdir().unwrap();
        let tool = external.path().join("tool");
        std::fs::write(&tool, "#!/bin/sh\nprintf version\n").unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut check = command(
            "build",
            &format!("printf changed > {}", quote(tool.to_str().unwrap())),
        );
        check["tools"] = json!([{"id":"tool","argv":[tool.to_str().unwrap()]}]);
        policy(root.path(), json!([check]));
        let changed = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        assert!(
            changed["checks"][0]["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains("Tool executable changed"),
            "{changed}"
        );

        std::fs::File::create(&tool)
            .unwrap()
            .set_len(256 * 1024 * 1024 + 1)
            .unwrap();
        policy(
            root.path(),
            json!([{"id":"huge","argv":[tool.to_str().unwrap()]}]),
        );
        let huge = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        assert!(
            huge["checks"][0]["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains("256 MiB"),
            "{huge}"
        );
    }

    #[test]
    fn signal_termination_and_failed_source_revalidation_retain_completed_evidence() {
        let root = fixture();
        policy(root.path(), json!([command("signal", "kill -TERM $$")]));
        let signal = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        assert!(
            signal["checks"][0]["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains("without an exit code")
        );
        let script = format!(
            "printf completed; rm {}",
            quote(root.path().join(".git/HEAD").to_str().unwrap())
        );
        policy(root.path(), json!([command("source", &script)]));
        let value = report(&cli(root.path(), &["check", "--format", "json"]), 2);
        assert_eq!(value["checks"][0]["verdict"], "pass");
        assert!(value["run_id"].is_string());
        assert!(
            value["gate"]
                .to_string()
                .contains("Cannot revalidate the source snapshot")
        );
        assert_eq!(
            std::fs::read_to_string(
                value["checks"][0]["execution"]["artifacts"][0]["path"]
                    .as_str()
                    .unwrap()
            )
            .unwrap(),
            "completed"
        );
    }

    #[test]
    fn task_contracts_preserve_expected_exit_codes_and_tool_evidence() {
        let root = fixture();
        policy(root.path(), json!([]));
        let task = json!({"schema_version":1,"task_id":"behavior","acceptance":[{
            "id":"expected","description":"The fixture returns its documented code",
            "verification":{"check_id":"expected","argv":["sh","-c","exit 7"],"expected_exit_code":7,"tools":[{"id":"fixture","argv":["sh","-c","printf fixture-v1"]}]}
        }]});
        std::fs::write(
            root.path().join("task.yaml"),
            serde_norway::to_string(&task).unwrap(),
        )
        .unwrap();
        let value = report(
            &cli(
                root.path(),
                &["check", "--task", "task.yaml", "--format", "json"],
            ),
            0,
        );
        assert_eq!(value["checks"][0]["execution"]["exit_code"], 7);
        assert_eq!(
            value["checks"][0]["metadata"]["tools"][0]["version"],
            "fixture-v1"
        );
    }
}
