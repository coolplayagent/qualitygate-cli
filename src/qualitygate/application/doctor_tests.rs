use super::super::test_support;
use super::*;

fn budget() -> SnapshotBudget {
    SnapshotBudget {
        side: "worktree".into(),
        reference: "head".into(),
        inventory_digest: "inventory".into(),
        file_count: 1,
        total_bytes: 1,
        max_files: 100,
        max_bytes: 1024 * 1024,
        max_file_bytes: 1024 * 1024,
        oversized_file_count: 0,
        oversized_files: vec![],
        unsupported_entry_count: 0,
        unsupported_entries: vec![],
        excluded_file_count: 0,
        excluded_paths: vec![],
        resource_exclusions: Default::default(),
        protected_exclusion_count: 0,
        protected_exclusions: vec![],
        details_truncated: false,
        recommended_max_file_mib: None,
    }
}

#[test]
fn budget_diagnostics_keep_independent_limits_and_offer_only_supported_increases() {
    let options = test_support::options(std::path::Path::new("/unit"));
    let mut report = Report::new("full".into(), true);
    report.planned_checks = vec!["tests".into()];
    report.budgets.push(budget());
    budget_diagnostics(&options, &mut report);
    assert!(report.diagnostics.is_empty());
    assert!(report.next_steps.is_empty());
    let over = &mut report.budgets[0];
    over.oversized_file_count = 1;
    over.recommended_max_file_mib = Some(3);
    over.total_bytes = 4 * 1024 * 1024 + 1;
    over.file_count = 101;
    over.unsupported_entry_count = 1;
    over.protected_exclusion_count = 1;
    budget_diagnostics(&options, &mut report);
    let codes: Vec<_> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(
        codes,
        [
            "snapshot.file_limit",
            "snapshot.file_count",
            "snapshot.total_limit",
            "snapshot.unsupported_entry",
            "snapshot.protected_excluded"
        ]
    );
    for (action, flag, value) in [
        ("increase_total_budget", "--snapshot-max-mib", "5"),
        ("increase_file_budget", "--snapshot-max-file-mib", "3"),
    ] {
        let step = report
            .next_steps
            .iter()
            .find(|s| s.action == action)
            .unwrap();
        let command = step.command.as_ref().unwrap();
        assert!(command.windows(2).any(|p| p == [flag, value]));
        assert!(command.iter().any(|s| s == "--probe-tools"));
    }
    report.next_steps.clear();
    report.budgets[0].recommended_max_file_mib = None;
    report.budgets[0].total_bytes = 1024 * 1024 * 1024 + 1;
    budget_diagnostics(&options, &mut report);
    assert_eq!(report.next_steps.len(), 1);
    assert_eq!(report.next_steps[0].action, "review_exclusions");
    assert!(report.next_steps[0].message.contains("8 MiB"));
}

#[test]
fn remediation_deduplicates_missing_variable_names_and_preserves_structured_errors() {
    let mut report = Report::new("full".into(), false);
    for id in ["a", "b"] {
        report.checks.push(Check {
            id: id.into(),
            required_env: vec![
                Environment {
                    name: "TOKEN".into(),
                    present: false,
                },
                Environment {
                    name: "PRESENT".into(),
                    present: true,
                },
            ],
            executable: None,
            probes: vec![],
        });
    }
    report.diagnostics.push(Diagnostic::new(
        "command.executable_missing",
        "missing",
        vec![],
    ));
    remediation(&mut report);
    assert_eq!(report.next_steps.len(), 2);
    assert_eq!(report.next_steps[0].message.matches("TOKEN").count(), 1);
    assert!(!report.next_steps[0].message.contains("PRESENT"));
    let input = anyhow::Error::new(PolicyInputError::SnapshotConfigurationMissing {
        path: "qualitygate.yaml".into(),
    });
    assert_eq!(
        diagnostic("fallback", &input, vec!["unit".into()]).code,
        "config.missing"
    );
    let input = anyhow::anyhow!("outer").context("context");
    let result = diagnostic("fallback", &input, vec!["unit".into()]);
    assert_eq!(result.code, "fallback");
    assert!(result.message.contains("outer"));
    assert_eq!(result.checks, ["unit"]);
}

#[tokio::test]
async fn static_preflight_never_runs_commands_and_explicit_probing_only_runs_declared_versions() {
    let policy = "schema_version: 1\nchecks:\n  - id: tests\n    kind: command\n    argv: [git, --qualitygate-invalid-option]\n    tools:\n      - {id: git, argv: [git, --version]}\n";
    let root = test_support::repository(policy).await;
    for probe in [false, true] {
        let report = run(test_support::options(root.path()), probe).await;
        assert!(report.complete, "{:?}", report.diagnostics);
        assert_eq!(report.exit_code(), 0);
        assert_eq!(report.scope, "preflight");
        assert!(
            report
                .not_executed
                .iter()
                .any(|n| n.check == "tests" && n.operation == "check")
        );
        assert_eq!(
            report
                .not_executed
                .iter()
                .any(|n| n.operation == "tool:git"),
            !probe
        );
        assert_eq!(report.checks[0].probes[0].attempted, probe);
        assert_eq!(report.checks[0].probes[0].complete, probe);
        assert!(report.next_steps.iter().any(|s| s.action == "run_check"));
        assert!(report.snapshot.is_some());
    }
}

#[tokio::test]
async fn unavailable_configuration_executable_or_snapshot_scope_remains_incomplete() {
    let root = test_support::repository("schema_version: 1\nchecks:\n  - id: missing\n    kind: command\n    argv: [qualitygate-unit-absent-493bf]\n").await;
    let report = run(test_support::options(root.path()), false).await;
    assert!(!report.complete);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code.ends_with("executable_missing"))
    );
    assert!(
        report
            .next_steps
            .iter()
            .any(|s| s.action == "provide_executable")
    );
    let mut options = test_support::options(root.path());
    options.selection = snapshot::Selection::Path {
        path: "source.rs".into(),
        base: "HEAD".into(),
    };
    let report = run(options, false).await;
    assert_eq!(report.exit_code(), 2);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.message.contains("worktree, staged and diff"))
    );
    std::fs::remove_file(root.path().join("qualitygate.yaml")).unwrap();
    let report = run(test_support::options(root.path()), false).await;
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "config.missing")
    );
    assert!(!root.path().join("qualitygate.yaml").exists());
    assert!(report.next_steps.iter().any(|s| s.action == "retry_doctor"));
}

#[tokio::test]
async fn explicit_policy_reference_detects_worktree_policy_change_before_tool_probes() {
    let policy = "schema_version: 1\nrules:\n  line-ending: {}\n";
    let root = test_support::repository(policy).await;
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "schema_version: 1\nrules:\n  line-ending: {enabled: false}\n",
    )
    .unwrap();
    let mut options = test_support::options(root.path());
    options.policy_ref = Some("HEAD".into());
    let report = run(options, true).await;
    assert!(!report.complete);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "policy.snapshot_mismatch"),
        "{:?}",
        report.diagnostics
    );
    assert!(report.checks.is_empty());
    assert_eq!(report.policy.unwrap().changes, ["qualitygate.yaml"]);
}

#[test]
fn repair_commands_preserve_staged_diff_and_task_context_and_parser_locations() {
    let error: anyhow::Error = crate::domain::runtime::ConfigurationError {
        code: "config.type".into(),
        message: "Expected integer".into(),
        field: Some("timeout_seconds".into()),
        line: Some(4),
        column: Some(7),
    }
    .into();
    let diagnostic = diagnostic("fallback", &error, vec!["unit".into()]);
    assert_eq!(diagnostic.code, "config.type");
    assert_eq!(diagnostic.line, Some(4));
    assert_eq!(diagnostic.column, Some(7));
    for selection in [
        snapshot::Selection::Staged,
        snapshot::Selection::Diff {
            base: "base".into(),
            head: "head".into(),
        },
    ] {
        let mut options = test_support::options(std::path::Path::new("/unit"));
        options.selection = selection;
        options.task = Some("tasks/release.yaml".into());
        let mut report = Report::new("full".into(), false);
        let mut over = budget();
        over.total_bytes = 2 * 1024 * 1024;
        over.max_bytes = 1024 * 1024;
        report.budgets.push(over);
        budget_diagnostics(&options, &mut report);
        let argv = report.next_steps[0].command.as_ref().unwrap();
        assert!(
            argv.windows(2)
                .any(|p| p == ["--task", "tasks/release.yaml"])
        );
        assert!(argv.iter().any(|p| p == "--staged" || p == "--diff"));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn probing_detects_both_materialized_input_and_original_policy_mutation() {
    for original in [false, true] {
        let root = test_support::repository("schema_version: 1\n").await;
        let target = if original {
            root.path().join("qualitygate.yaml")
        } else {
            std::path::PathBuf::from("source.rs")
        };
        let script = if original {
            "printf 'schema_version: 1\\nrules: {line-ending: {enabled: false}}\\n' > \"$1\"; printf version"
        } else {
            "printf changed > \"$1\"; printf version"
        };
        let policy = serde_json::json!({"schema_version":1,"checks":[{"id":"unit","argv":["git","--version"],"tools":[{"id":"version","argv":["sh","-c",script,"probe",target]}]}]});
        std::fs::write(
            root.path().join("qualitygate.yaml"),
            serde_norway::to_string(&policy).unwrap(),
        )
        .unwrap();
        let report = run(test_support::options(root.path()), true).await;
        assert!(!report.complete);
        let codes: Vec<_> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
        if original {
            assert!(codes.contains(&"snapshot.source_changed"), "{codes:?}");
            assert!(codes.contains(&"policy.changed"), "{codes:?}");
        } else {
            assert!(codes.contains(&"snapshot.inputs_changed"), "{codes:?}");
        }
    }
}

#[tokio::test]
async fn historical_preflight_preserves_selection_and_reports_default_resource_exclusions() {
    let root = test_support::repository("schema_version: 1\n").await;
    std::fs::write(root.path().join("movie.mp4"), b"excluded media").unwrap();
    let report = run(test_support::options(root.path()), false).await;
    assert!(
        report
            .next_steps
            .iter()
            .any(|s| s.action == "review_default_resource_exclusions")
    );
    for selection in [
        snapshot::Selection::Staged,
        snapshot::Selection::Diff {
            base: "HEAD".into(),
            head: "HEAD".into(),
        },
    ] {
        let mut options = test_support::options(root.path());
        options.selection = selection;
        let report = run(options, false).await;
        assert!(report.complete, "{:?}", report.diagnostics);
        assert!(report.snapshot.is_some());
    }
}
