use super::*;
use crate::snapshot::Selection;
use serde_json::json;
use std::path::Path;

fn options(root: &Path) -> CheckOptions {
    CheckOptions {
        root: root.into(),
        config: "qualitygate.yaml".into(),
        selection: Selection::Worktree {
            base: "HEAD".into(),
        },
        snapshot_options: snapshot::CaptureOptions {
            scope: check_scope::CheckScope::Delivery,
            ..Default::default()
        },
        profile: "full".into(),
        task: None,
        policy_ref: None,
        output_dir: None,
        trust_store: None,
        evidence_dir: None,
    }
}

#[test]
fn replay_preserves_selector_trust_task_and_budgets_and_pins_resolved_base() {
    let mut options = options(Path::new("/repository with spaces"));
    options.task = Some("tasks/test.yaml".into());
    options.policy_ref = Some("mutable-ref".into());
    options.trust_store = Some("/trust/store.json".into());
    options.evidence_dir = Some("/evidence".into());
    options.snapshot_options.max_file_bytes = 1024 * 1024 + 1;
    for (selection, expected) in [
        (
            Selection::Worktree {
                base: "HEAD".into(),
            },
            vec!["--worktree", "--base", "resolved-base"],
        ),
        (Selection::Staged, vec!["--staged"]),
        (
            Selection::Diff {
                base: "old".into(),
                head: "new".into(),
            },
            vec!["--diff", "resolved-base..new"],
        ),
        (
            Selection::Path {
                path: "src/a.rs".into(),
                base: "HEAD".into(),
            },
            vec!["--path", "src/a.rs", "--base", "resolved-base"],
        ),
        (
            Selection::MergeRequest {
                url: "https://example.test/pull/1".into(),
                api_base: Some("https://api.example.test".into()),
            },
            vec![
                "--mr",
                "https://example.test/pull/1",
                "--mr-api-base",
                "https://api.example.test",
            ],
        ),
    ] {
        options.selection = selection;
        let argv = recheck(&options, "resolved-base", Some("resolved-policy"));
        assert!(argv.windows(expected.len()).any(|slice| {
            slice
                .iter()
                .map(String::as_str)
                .eq(expected.iter().copied())
        }));
        for pair in [
            ["--policy-ref", "resolved-policy"],
            ["--expect-base", "resolved-base"],
            ["--task", "tasks/test.yaml"],
            ["--snapshot-max-file-mib", "2"],
            ["--trust-store", "/trust/store.json"],
            ["--evidence-dir", "/evidence"],
        ] {
            assert!(argv.windows(2).any(|slice| slice == pair));
        }
        assert!(!argv.iter().any(|arg| arg == "mutable-ref"));
    }
    options.selection = Selection::MergeRequest {
        url: "https://example.test/pull/1".into(),
        api_base: None,
    };
    options.snapshot_options.path_filter = Some("src".into());
    let argv = recheck(&options, "base", None);
    assert!(
        argv.windows(2)
            .any(|pair| pair == ["--policy-ref", "mutable-ref"])
    );
    assert!(argv.windows(2).any(|pair| pair == ["--path", "src"]));
    assert!(!argv.iter().any(|arg| arg == "--mr-api-base"));
    options.snapshot_options.scope = check_scope::CheckScope::Repository;
    assert!(recheck(&options, "base", None).is_empty());
}

async fn prepared(root: &Path, yaml: &str, invalid: Vec<String>, external: bool) -> Report {
    let mut options = options(root);
    let output = tempfile::tempdir().unwrap();
    options.output_dir = Some(output.path().into());
    if external {
        options.trust_store = Some(root.join("missing-trust.json"));
    }
    let mut snapshot = test_support::snapshot(root);
    snapshot.scope_evidence.mode = check_scope::CheckScope::Delivery;
    snapshot.files.insert(
        "qualitygate.yaml".into(),
        snapshot::File {
            bytes: yaml.as_bytes().to_vec(),
            executable: false,
        },
    );
    snapshot.files.insert(
        "src/a.rs".into(),
        snapshot::File {
            bytes: b"new\r\n".to_vec(),
            executable: false,
        },
    );
    snapshot.changes.insert(
        "src/a.rs".into(),
        snapshot::Change {
            kind: "added".into(),
            old_path: None,
            added_lines: [1].into(),
            removed_lines: Default::default(),
        },
    );
    let snapshot = Arc::new(snapshot);
    let loaded = policy::load_plan(&snapshot, &options).await.unwrap();
    let report = check_prepared(options, snapshot, loaded, invalid, false, None, None)
        .await
        .unwrap();
    let persisted: Report = serde_json::from_slice(
        &std::fs::read(&report.context.as_ref().unwrap().report_path).unwrap(),
    )
    .unwrap();
    assert_eq!(persisted.run_id, report.run_id);
    assert_eq!(
        persisted.snapshot.content_digest,
        report.snapshot.content_digest
    );
    assert_eq!(persisted.gate.decision, report.gate.decision);
    report
}

#[tokio::test]
async fn prepared_rule_violation_blocks_dependent_command_without_starting_it() {
    let root = tempfile::tempdir().unwrap();
    let report = prepared(root.path(), "schema_version: 1\nrules:\n  line-ending: {}\nchecks:\n  - id: followup\n    kind: command\n    argv: [git, --version]\n    depends_on: [line-ending]\n", vec![], false).await;
    assert_eq!(report.gate.decision, Decision::Incomplete);
    let rule = report
        .checks
        .iter()
        .find(|r| r.id == "line-ending")
        .unwrap();
    assert_eq!(rule.verdict, Some(Verdict::Fail));
    assert_eq!(rule.diagnostics[0].file.as_deref(), Some("src/a.rs"));
    assert!(!rule.diagnostics[0].recheck.argv.is_empty());
    let command = report.checks.iter().find(|r| r.id == "followup").unwrap();
    assert_eq!(command.execution.status, ExecutionStatus::Blocked);
    assert!(command.execution.started_at_ms.is_none());
    assert_eq!(command.metadata["depends_on"], json!(["line-ending"]));
    assert!(
        command
            .execution
            .reason
            .as_deref()
            .unwrap()
            .contains("Prerequisite line-ending")
    );
}

#[tokio::test]
async fn invalid_policy_or_half_supplied_external_acceptance_never_runs_commands() {
    let root = tempfile::tempdir().unwrap();
    let yaml = "schema_version: 1\nchecks:\n  - id: command\n    kind: command\n    argv: [git, --version]\n";
    for (invalid, external) in [(vec!["policy changed".into()], false), (vec![], true)] {
        let report = prepared(root.path(), yaml, invalid, external).await;
        assert_eq!(report.gate.decision, Decision::Incomplete);
        let command = report.checks.iter().find(|r| r.id == "command").unwrap();
        assert_eq!(command.execution.status, ExecutionStatus::Blocked);
        assert!(command.execution.started_at_ms.is_none());
        assert!(report.gate.blockers.iter().any(|s| if external {
            s.contains("both trust_store")
        } else {
            s.contains("policy changed")
        }));
    }
}

#[tokio::test]
async fn successful_prepared_command_produces_snapshot_bound_report_and_full_replay() {
    let root = tempfile::tempdir().unwrap();
    let report = prepared(root.path(), "schema_version: 1\nchecks:\n  - id: command\n    kind: command\n    argv: [git, --version]\n", vec![], false).await;
    assert_eq!(report.gate.decision, Decision::Pass);
    assert!(report.gate.complete);
    assert!(report.plan.pending_delivery_checks.is_empty());
    let command = report.checks.iter().find(|r| r.id == "command").unwrap();
    assert_eq!(command.verdict, Some(Verdict::Pass));
    assert_eq!(
        command.metadata["execution_snapshot"]["content_digest"],
        report.snapshot.content_digest
    );
    assert!(
        report
            .context
            .as_ref()
            .unwrap()
            .delivery_recheck
            .argv
            .windows(2)
            .any(|pair| pair == ["--profile", "full"])
    );
    assert_eq!(report.policy.trust, "local_candidate");
}

#[tokio::test]
async fn final_source_revalidation_blocks_stale_results_even_when_the_command_passed() {
    let root = test_support::repository(
        "schema_version: 1\nchecks:\n  - id: behavior\n    argv: [git, --version]\n",
    )
    .await;
    for missing_repository in [false, true] {
        let mut options = test_support::options(root.path());
        let output = tempfile::tempdir().unwrap();
        options.output_dir = Some(output.path().into());
        let snapshot = Arc::new(
            snapshot::capture_with_options(
                root.path(),
                &options.selection,
                &options.snapshot_options,
            )
            .await
            .unwrap(),
        );
        let loaded = policy::load_plan(&snapshot, &options).await.unwrap();
        std::fs::write(
            root.path().join("source.rs"),
            if missing_repository {
                "second edit"
            } else {
                "first edit"
            },
        )
        .unwrap();
        if missing_repository {
            options.root = root.path().join("missing");
        }
        let report = check_prepared(options, snapshot, loaded, vec![], true, None, None)
            .await
            .unwrap();
        assert_eq!(report.gate.decision, Decision::Incomplete);
        assert_eq!(report.checks[0].verdict, Some(Verdict::Pass));
        assert!(report.gate.blockers.iter().any(|b| if missing_repository {
            b.contains("Cannot revalidate")
        } else {
            b.contains("Source snapshot changed")
        }));
    }
}

#[tokio::test]
async fn absent_environment_and_manual_authority_are_explicit_execution_gaps() {
    let root = tempfile::tempdir().unwrap();
    for yaml in [
        "schema_version: 1\nchecks:\n  - id: behavior\n    argv: [git, --version]\n    required_env: [QUALITYGATE_UNIT_ABSENT_94FC20]\n",
        "schema_version: 1\nchecks:\n  - id: behavior\n    kind: manual\n",
    ] {
        let report = prepared(root.path(), yaml, vec![], false).await;
        assert_eq!(report.gate.decision, Decision::Incomplete);
        assert_eq!(report.checks[0].execution.status, ExecutionStatus::Blocked);
        assert!(report.checks[0].execution.started_at_ms.is_none());
    }
    let options = options(root.path());
    let mut selected = test_support::snapshot(root.path());
    let yaml = "schema_version: 1\nchecks:\n  - id: behavior\n    argv: [git, --version]\n";
    selected.files.insert(
        "qualitygate.yaml".into(),
        snapshot::File {
            bytes: yaml.as_bytes().to_vec(),
            executable: false,
        },
    );
    let selected = Arc::new(selected);
    let loaded = policy::load_plan(&selected, &options).await.unwrap();
    let mut options = options;
    options.trust_store = Some(root.path().join("missing-trust"));
    options.evidence_dir = Some(root.path().join("missing-evidence"));
    let report = check_prepared(options, selected, loaded, vec![], false, None, None)
        .await
        .unwrap();
    assert_eq!(report.gate.decision, Decision::Incomplete);
    assert!(
        report
            .gate
            .blockers
            .iter()
            .any(|s| s.contains("External acceptance inputs"))
    );
}

#[cfg(unix)]
#[tokio::test]
async fn aggregate_constraints_observe_final_external_revocation_and_preserve_instruction_limits() {
    for tamper in [false, true] {
        let (root, outside, checks) = manual::tests::fixture();
        let mut options = options(root.path());
        options.trust_store = Some(outside.path().join("trust.json"));
        options.evidence_dir = Some(outside.path().into());
        let mut input = test_support::snapshot(root.path());
        let source = test_support::source(&mut input);
        input.scope_evidence.excluded_paths = vec!["media.bin".into()];
        let input = Arc::new(input);
        let config = config::parse(b"schema_version: 1\n").unwrap();
        let mut plan = config::Plan::build(&config, None, "full").unwrap();
        plan.commands = checks;
        plan.order = vec!["manual".into(), constraint::CHECK_ID.into()];
        plan.required = plan.order.clone();
        let binding = snapshot::digest(b"adopted clause");
        plan.constraints = vec![constraint::Constraint {
            schema_version: 1,
            id: "review".into(),
            version: 1,
            source: source.clone(),
            statement: "Require independent review".into(),
            required: true,
            realization: constraint::Realization::Checks {
                mechanism: constraint::Mechanism::Tool,
                checks: vec!["manual".into()],
                positive_examples: vec!["approved".into()],
                negative_examples: vec!["rejected".into()],
            },
            adoption: Some(test_support::adoption(&binding)),
        }];
        plan.agent_obligations=vec![serde_json::from_value(json!({"schema_version":1,"id":"instruction","version":1,"source":source,"required":true,"scope":{"roles":[],"tasks":[]},"triggers":["before_delivery"],"must":["Review evidence"],"must_not":[],"enforcement":{"kind":"instruction"}})).unwrap()];
        if tamper {
            plan.commands.push(serde_json::from_value(json!({"id":"mutate","argv":["sh","-c","printf changed > \"$1\"","mutation",outside.path().join("review.json")]})).unwrap());
            plan.order.insert(1, "mutate".into());
        }
        let policy = test_support::policy();
        let subject = ManualSubject {
            repository: "example/project".into(),
            snapshot: SnapshotBinding::new(&input.identity, None),
            policy: PolicyBinding::from(&policy),
            check_id: "manual".into(),
            task: None,
        };
        std::fs::write(
            outside.path().join("review.json"),
            manual::tests::signed_record(subject, ManualDecision::Approved),
        )
        .unwrap();
        let loaded = policy::Loaded {
            catalog: config::catalog::Catalog::load(&config, []).unwrap(),
            evidence: policy,
            plan,
            protected_paths: vec![],
            constraint_bindings: [("review".into(), binding)].into(),
            agent_bindings: Default::default(),
        };
        let report = check_prepared(options, input, loaded, vec![], false, None, None)
            .await
            .unwrap();
        assert_eq!(
            report.gate.decision,
            if tamper {
                Decision::Incomplete
            } else {
                Decision::Pass
            }
        );
        let review = &report.checks[0];
        assert_eq!(
            review.metadata["manual_acceptance"]["valid_at_completion"],
            !tamper
        );
        assert_eq!(report.checks.last().unwrap().id, constraint::CHECK_ID);
        assert!(
            report
                .verification
                .known_limits
                .iter()
                .any(|s| s.contains("Agent obligations"))
        );
        assert!(
            report
                .verification
                .known_limits
                .iter()
                .any(|s| s.contains("excluded resources"))
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn prepared_plan_dispatches_specialized_evidence_and_skips_unaffected_commands() {
    let root = tempfile::tempdir().unwrap();
    let report=prepared(root.path(),"schema_version: 1\nchecks:\n  - id: docs\n    argv: [missing-unit-command]\n    when_changed: {paths: ['docs/**']}\n",vec![],false).await;
    assert_eq!(report.checks[0].verdict, Some(Verdict::Skipped));
    assert!(report.checks[0].execution.artifacts.is_empty());
    let report=prepared(root.path(),"schema_version: 1\nchecks:\n  - id: effectiveness\n    findings_exit_codes: [1]\n    argv: [sh, -c, 'exit 99']\n    tools: [{id: shell, argv: [sh, --version]}]\n    reports: [{path: results.xml, format: junit, mode: full}]\n    test_effectiveness: {source_paths: ['src/**'], test_paths: ['tests/**'], assertion_failure_types: [AssertionError]}\n",vec![],false).await;
    assert_eq!(report.checks[0].verdict, Some(Verdict::Fail));
    assert!(
        report.checks[0]
            .diagnostics
            .iter()
            .any(|d| d.message.contains("no added or content-modified"))
    );
    let yaml = format!(
        "schema_version: 1\nchecks:\n  - id: api\n    argv: [sh, -c, 'exit 99']\n    tools: [{{id: shell, argv: [sh, --version]}}]\n    compatibility:\n      tool: japicmp\n      analyzer_jar: /nonexistent/qualitygate-unit.jar\n      analyzer_sha256: {}\n      artifacts: [{{baseline: old.jar, current: new.jar}}]\n",
        snapshot::digest(b"absent")
    );
    let report = prepared(root.path(), &yaml, vec![], false).await;
    assert_eq!(report.checks[0].execution.status, ExecutionStatus::Blocked);
    assert_eq!(report.gate.decision, Decision::Incomplete);
}
