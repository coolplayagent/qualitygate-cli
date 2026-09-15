mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{collections::BTreeSet, path::Path, process::Command};

#[test]
fn full_corpus_runs_real_evaluators_and_keeps_negative_cases_green_only_on_golden_agreement() {
    let root = tempfile::tempdir().unwrap();
    let result = report(&cli(root.path(), &["selfcheck", "--format", "json"]), 0);
    assert_eq!(result["complete"], true);
    assert_eq!(result["decision"], "pass");
    assert!(result["errors"].as_array().unwrap().is_empty());
    let cases = result["fixtures"].as_array().unwrap();
    assert_eq!(cases.len(), 338, "Shipped fixture inventory changed");
    let evolution: Vec<_> = cases
        .iter()
        .filter(|case| case["rule"] == "policy-evolution")
        .collect();
    assert_eq!(evolution.len(), 85);
    let observed =
        |id: &str| &evolution.iter().find(|case| case["fixture"] == id).unwrap()["observed"];
    assert_eq!(
        observed("evolution-negative-oracle")["gates"][0]["candidate"]["decision"],
        "fail"
    );
    assert_eq!(
        observed("evolution-held-out-regression")["conclusion"],
        "block"
    );
    assert_eq!(
        observed("evolution-workflow-timeout-4")["probe_statuses"],
        serde_json::json!(["timed_out"])
    );
    assert_eq!(
        observed("evolution-workflow-missing-tool-2")["probe_statuses"],
        serde_json::json!(["tool_error"])
    );
    let pairs = |id| {
        observed(id)["validation"]["evaluation"]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| {
                serde_json::json!([
                    case["kind"],
                    case["snapshot_digest"],
                    case["baseline"]["mismatches"],
                    case["candidate"]["mismatches"]
                ])
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        pairs("evolution-workflow-promote-1"),
        pairs("evolution-workflow-promote-4")
    );
    let suites: BTreeSet<_> = cases
        .iter()
        .map(|case| case["suite"].as_str().unwrap())
        .collect();
    assert_eq!(suites, BTreeSet::from(["minimal", "typical", "stress"]));
    assert!(cases.iter().all(
        |case| case["decision"] == "pass" && case["mismatches"].as_array().unwrap().is_empty()
    ));
    assert!(
        cases
            .iter()
            .any(|case| case["rule"] == "commit-message" && case["observed"]["verdict"] == "fail")
    );
    assert!(
        cases.iter().any(
            |case| case["fixture"] == "runner-timeout" && case["observed"]["timed_out"] == true
        )
    );
    assert!(cases.iter().any(|case| case["fixture"] == "snapshot-staged"
        && case["observed"]["files"]["fixture.txt"] == "staged\n"));
    assert!(
        result["corpus_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        result["rules_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    for key in ["verified_shapes", "known_limits", "unverified_assumptions"] {
        assert!(!result["verification"][key].as_array().unwrap().is_empty());
    }
}

#[test]
fn filters_are_composable_and_unknown_rules_cannot_pass_empty_regression() {
    let root = tempfile::tempdir().unwrap();
    for args in [
        vec!["selfcheck", "--fixture", "minimal", "--format", "json"],
        vec!["selfcheck", "--rule", "commit-message", "--format", "json"],
        vec![
            "selfcheck",
            "--fixture",
            "stress",
            "--rule",
            "commit-message",
            "--format",
            "json",
        ],
    ] {
        let result = report(&cli(root.path(), &args), 0);
        for case in result["fixtures"].as_array().unwrap() {
            if let Some(index) = args.iter().position(|arg| *arg == "--fixture") {
                assert_eq!(case["suite"], args[index + 1]);
            }
            if let Some(index) = args.iter().position(|arg| *arg == "--rule") {
                assert_eq!(case["rule"], args[index + 1]);
            }
        }
    }
    let missing = report(
        &cli(
            root.path(),
            &["selfcheck", "--rule", "unknown", "--format", "json"],
        ),
        2,
    );
    assert_eq!(missing["decision"], "incomplete");
    assert!(
        missing["errors"][0]
            .as_str()
            .unwrap()
            .contains("No fixtures match")
    );
    assert_eq!(
        cli(root.path(), &["selfcheck", "--fixture", "unknown"])
            .status
            .code(),
        Some(2)
    );
}

fn rule_assets(destination: &Path) {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/qualitygate-cli/references/rules");
    for package in std::fs::read_dir(source).unwrap() {
        let package = package.unwrap();
        let target = destination.join(package.file_name());
        std::fs::create_dir_all(&target).unwrap();
        for entry in std::fs::read_dir(package.path()).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}

fn with_assets(root: &Path, assets: &Path, format: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .env("QUALITYGATE_BUILTIN_RULES_DIR", assets)
        .current_dir(root)
        .args(["selfcheck", "--rule", "commit-message", "--format", format])
        .output()
        .unwrap()
}

#[test]
fn weakening_commit_rule_is_falsified_with_fixture_assertion_and_input_evidence() {
    let root = tempfile::tempdir().unwrap();
    let assets = root.path().join("rules");
    rule_assets(&assets);
    let original = report(&with_assets(root.path(), &assets, "json"), 0);
    let path = assets.join("core/commit-message.yaml");
    let mut rule: Value = serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    rule["defaults"]["parameters"] = serde_json::json!({"pattern":"(?s).*"});
    std::fs::write(path, serde_norway::to_string(&rule).unwrap()).unwrap();
    let mutated = report(&with_assets(root.path(), &assets, "json"), 1);
    assert_ne!(original["rules_digest"], mutated["rules_digest"]);
    assert_eq!(original["corpus_digest"], mutated["corpus_digest"]);
    let empty = mutated["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["fixture"] == "commit-empty")
        .unwrap();
    assert_eq!(empty["decision"], "fail");
    assert_eq!(empty["observed"]["verdict"], "pass");
    assert!(
        empty["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mismatch| mismatch["assertion"] == "/verdict"
                && mismatch["expected"] == "fail"
                && mismatch["actual"] == "pass")
    );
    assert!(
        empty["input"]
            .as_str()
            .unwrap()
            .starts_with("fixtures/minimal/cases.json#/")
    );
    for format in ["table", "markdown"] {
        let rendered = with_assets(root.path(), &assets, format);
        assert_eq!(rendered.status.code(), Some(1));
        let text = String::from_utf8(rendered.stdout).unwrap();
        assert!(text.contains("commit-empty"));
        assert!(text.contains("assertion /verdict"));
        assert!(text.contains("已知边界"));
    }
}

#[test]
fn missing_rule_assets_are_structured_incomplete_evidence() {
    let root = tempfile::tempdir().unwrap();
    let result = report(
        &with_assets(root.path(), &root.path().join("missing"), "json"),
        2,
    );
    assert_eq!(result["complete"], false);
    assert!(
        result["errors"][0]
            .as_str()
            .unwrap()
            .contains("Cannot load active rule assets")
    );
}

#[test]
fn weakening_the_policy_workflow_evaluator_is_falsified_by_unchanged_independent_goldens() {
    let root = tempfile::tempdir().unwrap();
    let assets = root.path().join("rules");
    rule_assets(&assets);
    let path = assets.join("core/line-ending.yaml");
    let mut rule: Value = serde_norway::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    // A different real evaluator accepts the CRLF replay. The acceptance oracle
    // and bundled fixture goldens must continue to demand its rejection.
    rule["implementation"] = serde_json::json!("diff-size");
    std::fs::write(path, serde_norway::to_string(&rule).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .env("QUALITYGATE_BUILTIN_RULES_DIR", &assets)
        .current_dir(root.path())
        .args([
            "selfcheck",
            "--fixture",
            "typical",
            "--rule",
            "policy-evolution",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let result = report(&output, 1);
    for id in [
        "evolution-workflow-promote-1",
        "evolution-workflow-promote-4",
    ] {
        let case = result["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["fixture"] == id)
            .unwrap();
        assert_eq!(case["decision"], "fail");
        assert_eq!(
            case["observed"]["validation"]["evaluation"]["conclusion"],
            "block"
        );
        assert_eq!(case["observed"]["has_active"], false);
        assert!(case["mismatches"].as_array().unwrap().iter().any(
            |mismatch| mismatch["assertion"] == "/validation/evaluation/conclusion"
                && mismatch["expected"] == "pass"
                && mismatch["actual"] == "block"
        ));
        assert!(
            case["input_digest"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            case["golden_digest"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
    }
}

#[test]
fn native_policy_workflows_reject_inherited_git_redirection_before_setup() {
    let protected = fixture();
    let before = std::fs::read(protected.path().join(".git/index")).unwrap();
    let run_root = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .env("GIT_DIR", protected.path().join(".git"))
        .current_dir(run_root.path())
        .args([
            "selfcheck",
            "--fixture",
            "typical",
            "--rule",
            "policy-evolution",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let result = report(&output, 2);
    let workflows: Vec<_> = result["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| {
            case["fixture"]
                .as_str()
                .unwrap()
                .starts_with("evolution-workflow-")
        })
        .collect();
    assert_eq!(workflows.len(), 3);
    assert!(workflows.iter().all(|case| case["decision"] == "incomplete"
        && case["error"].as_str().unwrap().contains("GIT_DIR")));
    assert_eq!(
        std::fs::read(protected.path().join(".git/index")).unwrap(),
        before
    );
    assert_eq!(std::fs::read_dir(run_root.path()).unwrap().count(), 0);
}

#[test]
fn selfcheck_needs_no_repository_and_never_executes_candidate_commands() {
    let root = tempfile::tempdir().unwrap();
    let output = cli(
        &root.path().join("absent-root"),
        &[
            "selfcheck",
            "--fixture",
            "minimal",
            "--rule",
            "commit-message",
            "--format",
            "json",
        ],
    );
    report(&output, 0);
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "invalid candidate policy; must not be read",
    )
    .unwrap();
    report(
        &cli(
            root.path(),
            &[
                "selfcheck",
                "--fixture",
                "minimal",
                "--rule",
                "commit-message",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn inherited_git_overrides_are_incomplete_before_any_repository_write() {
    let protected = fixture();
    let before = std::fs::read(protected.path().join(".git/index")).unwrap();
    let run_root = tempfile::tempdir().unwrap();
    for variable in ["GIT_DIR", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY"] {
        let output = Command::new(env!("CARGO_BIN_EXE_qualitygate"))
            .env(variable, protected.path().join(".git"))
            .current_dir(run_root.path())
            .args(["selfcheck", "--rule", "snapshot", "--format", "json"])
            .output()
            .unwrap();
        let result = report(&output, 2);
        assert!(
            result["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .all(|case| case["error"].as_str().unwrap().contains(variable))
        );
        assert_eq!(
            std::fs::read(protected.path().join(".git/index")).unwrap(),
            before
        );
        assert_eq!(std::fs::read_dir(run_root.path()).unwrap().count(), 0);
    }
}

#[test]
fn check_reports_preserve_gate_semantics_and_expose_boundaries_in_all_formats() {
    let root = fixture();
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "schema_version: 1\nrules:\n  line-ending: {}\n",
    )
    .unwrap();
    std::fs::write(root.path().join("hello.txt"), "updated\n").unwrap();
    let result = report(
        &cli(
            root.path(),
            &[
                "check",
                "--worktree",
                "--profile",
                "quick",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(result["gate"]["decision"], "pass");
    assert!(
        result["verification"]["conclusion"]
            .as_str()
            .unwrap()
            .contains("已验证形态")
    );
    for format in ["table", "markdown"] {
        let output = cli(root.path(), &["check", "--worktree", "--format", format]);
        assert_eq!(output.status.code(), Some(0));
        let text = String::from_utf8(output.stdout).unwrap();
        for label in ["已验证形态", "已知边界", "未验证假设"] {
            assert!(text.contains(label));
        }
    }
}
