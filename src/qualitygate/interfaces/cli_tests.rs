use super::*;
use crate::application::policy_guidance::SelectionKind;

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("qualitygate").chain(args.iter().copied())).unwrap()
}

#[test]
fn feedback_forces_json_and_failure_guidance_keeps_selected_snapshot_and_policy() {
    for (args, expected) in [
        (vec!["check"], "Worktree"),
        (vec!["check", "--worktree", "--path", "src"], "Worktree"),
        (vec!["check", "--staged"], "Staged"),
        (vec!["check", "--diff", "base..head"], "Diff"),
        (
            vec!["check", "--mr", "https://example.test/pull/1"],
            "MergeRequest",
        ),
        (vec!["check", "--path", "src"], "Path"),
    ] {
        let mut argv = vec!["--root", "/work tree", "--config", "custom.yaml"];
        argv.extend(args);
        argv.extend(["--feedback", "--policy-ref", "trusted"]);
        let cli = parse(&argv);
        assert_eq!(cli.output_format(), Format::Json);
        let context = cli.failure_context();
        assert_eq!(format!("{:?}", context.selection.unwrap()), expected);
        assert!(context.policy_ref);
        assert_eq!(context.root, PathBuf::from("/work tree"));
        assert_eq!(context.config, "custom.yaml");
    }
    let cli = parse(&["--format", "markdown", "config", "--show"]);
    assert_eq!(cli.output_format(), Format::Markdown);
    assert!(cli.failure_context().selection.is_none());
    assert!(!cli.failure_context().policy_ref);
    assert!(matches!(
        parse(&["check"]).failure_context().selection,
        Some(SelectionKind::Worktree)
    ));
}

#[test]
fn conflicting_selectors_and_unbounded_or_incomplete_arguments_are_rejected() {
    for args in [
        vec!["check", "--staged", "--worktree"],
        vec!["check", "--diff", "a..b", "--base", "a"],
        vec!["check", "--mr-api-base", "https://example.test"],
        vec!["check", "--feedback-max-bytes", "4096"],
        vec!["check", "--feedback", "--feedback-max-bytes", "4095"],
        vec!["check", "--feedback", "--feedback-max-bytes", "262145"],
        vec!["check", "--export-delivery", "/output.zip", "--feedback"],
        vec!["check", "--trust-store", "/trust.json"],
        vec!["check", "--evidence-dir", "/evidence"],
        vec!["check", "--snapshot-jobs", "0"],
        vec!["check", "--snapshot-max-mib", "1025"],
        vec!["check", "--snapshot-max-file-mib", "9"],
        vec!["check", "--snapshot-timeout-secs", "3601"],
        vec!["config"],
        vec!["rules", "list", "--source", "unknown"],
        vec!["rules", "history", "line-ending", "--limit", "257"],
    ] {
        assert!(
            Cli::try_parse_from(std::iter::once("qualitygate").chain(args.iter().copied()))
                .is_err(),
            "{args:?}"
        );
    }
}

#[test]
fn doctor_options_preserve_explicit_endpoints_and_delivery_resource_budgets() {
    for flags in [
        vec!["--worktree", "--base", "base"],
        vec!["--staged"],
        vec!["--diff", "base..head"],
    ] {
        let mut argv = vec![
            "doctor",
            "--snapshot-max-mib",
            "9",
            "--snapshot-max-file-mib",
            "3",
            "--snapshot-jobs",
            "2",
            "--snapshot-timeout-secs",
            "7",
        ];
        argv.extend(flags.clone());
        let Command::Doctor(args) = parse(&argv).command else {
            panic!("doctor expected")
        };
        let options = args.options("/repo".into(), "custom.yaml".into()).unwrap();
        assert_eq!(options.snapshot_options.max_bytes, 9 * 1024 * 1024);
        assert_eq!(options.snapshot_options.max_file_bytes, 3 * 1024 * 1024);
        assert_eq!(options.snapshot_options.jobs, 2);
        assert_eq!(options.snapshot_options.timeout.as_secs(), 7);
        assert_eq!(
            options.snapshot_options.scope,
            crate::domain::check_scope::CheckScope::Delivery
        );
        assert_eq!(options.config, "custom.yaml");
        match flags[0] {
            "--worktree" => assert!(
                matches!(options.selection, crate::snapshot::Selection::Worktree { base } if base == "base")
            ),
            "--staged" => assert!(matches!(
                options.selection,
                crate::snapshot::Selection::Staged
            )),
            _ => assert!(
                matches!(options.selection, crate::snapshot::Selection::Diff { base, head } if base == "base" && head == "head")
            ),
        }
    }
    for diff in ["head", "..head", "base..", "base...head"] {
        assert!(
            super::super::doctor::diff_selection(diff).is_err(),
            "{diff}"
        );
    }
}

#[tokio::test]
async fn schema_exports_are_json_without_repository_or_configuration_discovery() {
    for schema in [
        "agent-obligation",
        "operator-rule",
        "operator-rule-v3",
        "project-rule-family",
        "constraint",
        "capabilities",
        "doctor",
        "doctor-summary",
        "decision",
        "feedback",
        "project-rule",
    ] {
        let (output, code) = parse(&[
            "--root",
            "/qualitygate-nonexistent-unit-root",
            "schema",
            schema,
        ])
        .run()
        .await
        .unwrap();
        assert_eq!(code, 0);
        let document: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(document["$schema"].is_string(), "{schema}");
        assert!(document["$id"].is_string(), "{schema}");
    }
    let (output, code) = parse(&[
        "--root",
        "/qualitygate-nonexistent-unit-root",
        "rules",
        "schema",
    ])
    .run()
    .await
    .unwrap();
    assert_eq!(code, 0);
    assert!(serde_json::from_str::<serde_json::Value>(&output).unwrap()["$id"].is_string());
}

#[tokio::test]
async fn unsupported_envelopes_fail_before_repository_work_and_preflight_cannot_claim_delivery() {
    for args in [
        vec!["--envelope", "capabilities"],
        vec!["--envelope", "selfcheck"],
        vec!["--envelope", "check"],
    ] {
        let error = parse(&args).run().await.unwrap_err();
        assert!(error.to_string().contains("envelope"));
    }
    let (output, code) = parse(&["--format", "json", "--envelope", "doctor"])
        .run()
        .await
        .unwrap();
    assert_eq!(code, 2);
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(report["complete"], false);
    assert!(output.contains("does not produce a delivery decision envelope"));
    let (output, code) = parse(&["--format", "json", "capabilities"])
        .run()
        .await
        .unwrap();
    assert_eq!(code, 0);
    let capabilities: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(capabilities["schema_version"], 1);
    crate::config::preflight_schema::validate_capabilities(&capabilities).unwrap();
    assert!(
        crate::config::preflight_schema::validate_capabilities(&serde_json::json!({})).is_err()
    );
    assert!(
        capabilities["executable"]["digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
}

#[tokio::test]
async fn selected_selfcheck_envelope_binds_rule_catalog_and_corpus_without_repository() {
    let (output, code) = parse(&[
        "--format",
        "json",
        "--envelope",
        "selfcheck",
        "--fixture",
        "minimal",
        "--rule",
        "line-ending",
    ])
    .run()
    .await
    .unwrap();
    assert_eq!(code, 0);
    let envelope =
        crate::domain::decision_envelope::DecisionEnvelope::parse(output.as_bytes()).unwrap();
    assert_eq!(
        envelope.command_kind,
        crate::domain::decision_envelope::CommandKind::Selfcheck
    );
    assert_eq!(
        envelope.policy.digest.as_deref(),
        envelope.payload["rules_digest"].as_str()
    );
    assert_eq!(
        envelope.subject.source_digest.as_deref(),
        envelope.payload["corpus_digest"].as_str()
    );
}

async fn dispatch(
    root: &std::path::Path,
    args: &[&str],
) -> anyhow::Result<(serde_json::Value, u8)> {
    let mut argv = vec!["--root", root.to_str().unwrap(), "--format", "json"];
    argv.extend_from_slice(args);
    let (output, code) = parse(&argv).run().await?;
    Ok((serde_json::from_str(&output)?, code))
}

#[tokio::test]
async fn rule_commands_edit_only_selected_candidate_and_preserve_category_membership() {
    let root = tempfile::tempdir().unwrap();
    let (initialized, code) = dispatch(root.path(), &["init"]).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(initialized["status"], "candidate");
    assert_eq!(initialized["snapshot_preflight"]["complete"], false);
    for args in [
        vec![
            "rules",
            "categories",
            "create",
            "unit",
            "--description",
            "Scenario checks",
        ],
        vec!["rules", "enable", "diff-size"],
        vec!["rules", "assign", "diff-size", "--category", "unit"],
        vec![
            "rules",
            "configure",
            "diff-size",
            "--param",
            "max_added_lines=17",
            "--severity",
            "warning",
            "--required",
            "false",
        ],
        vec!["rules", "categories", "rename", "unit", "scenarios"],
    ] {
        assert_eq!(dispatch(root.path(), &args).await.unwrap().1, 0, "{args:?}");
    }
    let (shown, _) = dispatch(root.path(), &["config", "--show"]).await.unwrap();
    assert_eq!(shown["config"]["rules"]["diff-size"]["severity"], "warning");
    assert_eq!(shown["config"]["rules"]["diff-size"]["required"], false);
    assert_eq!(
        shown["config"]["rules"]["diff-size"]["parameters"]["max_added_lines"],
        17
    );
    for args in [
        vec!["rules", "list", "--source", "all"],
        vec!["rules", "describe", "diff-size"],
        vec!["rules", "categories"],
        vec!["rules", "categories", "list"],
        vec!["rules", "context"],
    ] {
        assert_eq!(dispatch(root.path(), &args).await.unwrap().1, 0);
    }
    assert!(
        dispatch(root.path(), &["rules", "categories", "delete", "scenarios"])
            .await
            .is_err()
    );
    for args in [
        vec!["rules", "unassign", "diff-size", "--category", "scenarios"],
        vec!["rules", "categories", "delete", "scenarios"],
        vec!["rules", "disable", "diff-size"],
    ] {
        assert_eq!(dispatch(root.path(), &args).await.unwrap().1, 0);
    }
    let (again, _) = dispatch(root.path(), &["init"]).await.unwrap();
    assert_eq!(again["created"], false);
}

#[tokio::test]
async fn read_only_pilot_dispatch_retains_incomplete_draft_and_rejects_unsupported_envelopes() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("pilot.json"),
        br#"{"schema_version":1,"id":"qualitygate-eight-task-seven-day-pilot","protocol":{"project":"qualitygate-cli","sampling":"Before execution select four bug fixes and four refactors; assign each to both model groups and both workflows","owner":null,"reviewer":null,"archive":null,"sealed_at":null,"start_at":null,"end_at":null,"task_count":8,"days":7,"max_attempts":3,"max_seconds":1800,"review_fraction_min":1.0,"monetary_cap":null,"thresholds":{"detection_min":0.9,"false_positive_max":0.05,"repair_min":0.8,"completion_min":0.95,"review_reduction_min":0.1,"full_p95_ratio_max":1.2,"cost_ratio_max":1.0}},"assignments":[],"observations":[]}"#,
    )
    .unwrap();
    let (summary, code) = dispatch(
        root.path(),
        &["pilot", "summarize", "--input", "pilot.json"],
    )
    .await
    .unwrap();
    assert_ne!(code, 0);
    assert_eq!(summary["complete"], false);
    let (_, code) = dispatch(
        root.path(),
        &["--envelope", "pilot", "summarize", "--input", "pilot.json"],
    )
    .await
    .unwrap();
    assert_ne!(code, 0);
    for args in [
        vec!["--envelope", "pilot", "seal", "--input", "pilot.json"],
        vec!["--envelope", "rules", "list"],
        vec!["--envelope", "policy", "evaluator"],
        vec!["pilot", "seal", "--input", "pilot.json"],
        vec!["pilot", "authorization-subject", "--input", "pilot.json"],
        vec![
            "pilot",
            "acceptance-subject",
            "--input",
            "pilot.json",
            "--trust-store",
            "missing-trust.json",
            "--authorization",
            "missing-authorization.json",
        ],
    ] {
        assert!(dispatch(root.path(), &args).await.is_err(), "{args:?}");
    }
    assert_eq!(
        dispatch(root.path(), &["policy", "evaluator"])
            .await
            .unwrap()
            .1,
        0
    );
    for args in [
        vec!["policy", "evidence", "list"],
        vec!["policy", "candidate", "list"],
        vec!["policy", "history"],
        vec!["policy", "effectiveness"],
    ] {
        assert!(dispatch(root.path(), &args).await.is_err(), "{args:?}");
    }
}

#[tokio::test]
async fn check_dispatch_preserves_failed_gate_across_full_feedback_and_envelope_formats() {
    let root = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    crate::snapshot::run_git(root.path(), &["init", "--quiet", "--template="], None)
        .await
        .unwrap();
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "schema_version: 1\nrules: {line-ending: {enabled: true}}\n",
    )
    .unwrap();
    std::fs::write(root.path().join("source.rs"), "fn original() {}\n").unwrap();
    crate::snapshot::run_git(root.path(), &["add", "."], None)
        .await
        .unwrap();
    crate::snapshot::run_git(
        root.path(),
        &[
            "-c",
            "user.name=Unit Test",
            "-c",
            "user.email=unit@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
        None,
    )
    .await
    .unwrap();
    std::fs::write(root.path().join("source.rs"), "fn changed() {}\r\n").unwrap();
    for flags in [
        vec![],
        vec!["--feedback"],
        vec!["--envelope"],
        vec!["--feedback", "--envelope"],
        vec!["--path", "source.rs"],
    ] {
        let mut args = vec![
            "check",
            "--worktree",
            "--output-dir",
            output.path().to_str().unwrap(),
        ];
        args.extend(flags.clone());
        let (result, code) = dispatch(root.path(), &args).await.unwrap();
        assert_eq!(code, 1, "{flags:?}: {result}");
        if flags.contains(&"--envelope") {
            assert_eq!(result["gate"]["route"], "block");
        } else {
            assert_eq!(result["gate"]["decision"], "fail");
        }
    }
    crate::snapshot::run_git(root.path(), &["add", "source.rs"], None)
        .await
        .unwrap();
    assert_eq!(
        dispatch(
            root.path(),
            &[
                "check",
                "--staged",
                "--output-dir",
                output.path().to_str().unwrap()
            ]
        )
        .await
        .unwrap()
        .1,
        1
    );
    std::fs::write(root.path().join("source.rs"), "fn repaired() {}\n").unwrap();
    let (passed, code) = dispatch(
        root.path(),
        &[
            "check",
            "--path",
            "source.rs",
            "--output-dir",
            output.path().to_str().unwrap(),
        ],
    )
    .await
    .unwrap();
    assert_eq!(code, 0, "{passed}");
    assert_eq!(passed["scope"], "path");
    let (export, code) = dispatch(
        root.path(),
        &[
            "check",
            "--worktree",
            "--output-dir",
            output.path().to_str().unwrap(),
            "--export-delivery",
            output.path().to_str().unwrap(),
        ],
    )
    .await
    .unwrap();
    assert_eq!(code, 0, "{export}");
    assert_eq!(export["kind"], "delivery_check");
    assert!(
        std::path::Path::new(export["delivery"]["artifact"]["path"].as_str().unwrap()).is_file()
    );
    assert!(
        dispatch(root.path(), &["check", "--expect-base", "wrong-base"])
            .await
            .unwrap_err()
            .to_string()
            .contains("expected base")
    );
    let (_, code) = dispatch(
        root.path(),
        &[
            "check",
            "--diff",
            "HEAD..HEAD",
            "--output-dir",
            output.path().to_str().unwrap(),
        ],
    )
    .await
    .unwrap();
    assert_eq!(code, 0);
}

#[tokio::test]
async fn authoring_dispatch_keeps_generated_rules_unadopted_and_validates_source_changes() {
    use serde_json::json;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("AGENTS.md"),
        "# Rules\nTests must describe observable behavior.\n",
    )
    .unwrap();
    let (source, code) = dispatch(
        root.path(),
        &[
            "rules",
            "source",
            "--document",
            "AGENTS.md",
            "--section",
            "Rules",
        ],
    )
    .await
    .unwrap();
    assert_eq!(code, 0);
    let rule = json!({"id":"scenario-tests","version":1,"source":source["source"],"language":["rust"],"requires_capabilities":["test_methods"],"when":{"entity":"test_method"},"then":{"name_pattern":"^test_"},"fix":"Name the observed behavior"});
    std::fs::write(
        root.path().join("candidate.json"),
        serde_json::to_vec(&rule).unwrap(),
    )
    .unwrap();
    let (generated, code) = dispatch(
        root.path(),
        &["rules", "generate", "--input", "candidate.json"],
    )
    .await
    .unwrap();
    assert_eq!(code, 0);
    assert_eq!(generated["policy_changed"], false);
    let (validated, code) = dispatch(
        root.path(),
        &["--envelope", "rules", "validate", "qualitygate/rules"],
    )
    .await
    .unwrap();
    assert_eq!(code, 0);
    assert_eq!(validated["gate"]["route"], "accept");
    std::fs::write(
        root.path().join("AGENTS.md"),
        "# Rules\nChanged contract.\n",
    )
    .unwrap();
    let (_, code) = dispatch(root.path(), &["rules", "validate", "qualitygate/rules"])
        .await
        .unwrap();
    assert_eq!(code, 1);
}

#[tokio::test]
async fn doctor_and_contract_dispatch_report_selected_scope_and_invalid_comparisons() {
    let root = tempfile::tempdir().unwrap();
    crate::snapshot::run_git(root.path(), &["init", "-q", "--template="], None)
        .await
        .unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"), "schema_version: 1\n").unwrap();
    crate::snapshot::run_git(root.path(), &["add", "."], None)
        .await
        .unwrap();
    crate::snapshot::run_git(
        root.path(),
        &[
            "-c",
            "user.name=Unit",
            "-c",
            "user.email=unit@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "base",
        ],
        None,
    )
    .await
    .unwrap();
    let (contract, code) = dispatch(root.path(), &["agent-contract", "--role", "implementer"])
        .await
        .unwrap();
    assert_eq!(code, 0);
    assert_eq!(contract["ready"], true);
    for args in [
        vec!["doctor"],
        vec!["doctor", "--summary"],
        vec!["doctor", "--diff", "invalid"],
    ] {
        let (report, code) = dispatch(root.path(), &args).await.unwrap();
        assert!(report.is_object());
        if args.contains(&"--summary") {
            crate::config::preflight_schema::validate_doctor_summary(&report).unwrap();
        } else {
            crate::config::preflight_schema::validate_doctor(&report).unwrap();
        }
        if args.contains(&"invalid") {
            assert_eq!(code, 2);
        } else {
            assert_eq!(code, 0);
        }
    }
    let (output, code) = parse(&["selfcheck-probe", "success"]).run().await.unwrap();
    assert_eq!(code, 0);
    assert!(!output.is_empty());
    let (_, code) = parse(&[
        "--format",
        "markdown",
        "selfcheck",
        "--fixture",
        "minimal",
        "--rule",
        "line-ending",
    ])
    .run()
    .await
    .unwrap();
    assert_eq!(code, 0);
}

#[cfg(unix)]
#[tokio::test]
async fn agent_dispatch_keeps_invalid_evaluator_protocol_incomplete_with_retained_ledger() {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    for args in [
        vec!["init", "-q", "--template="],
        vec![
            "-c",
            "user.name=Unit",
            "-c",
            "user.email=unit@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-qm",
            "base",
        ],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(root.path())
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let executable = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join("git"))
        .find(|p| p.is_file())
        .unwrap();
    let prompt = external.path().join("prompt.txt");
    let agent = external.path().join("agent.json");
    std::fs::write(&prompt, "Verify the declared task").unwrap();
    std::fs::write(&agent, b"[\"git\",\"--version\"]").unwrap();
    let args = [
        "agent-run",
        "--qualitygate",
        executable.to_str().unwrap(),
        "--base",
        "HEAD",
        "--policy-ref",
        "HEAD",
        "--task",
        "task.yaml",
        "--prompt-file",
        prompt.to_str().unwrap(),
        "--agent-command",
        agent.to_str().unwrap(),
        "--output-dir",
        external.path().to_str().unwrap(),
        "--timeout-seconds",
        "5",
    ];
    let (result, code) = dispatch(root.path(), &args).await.unwrap();
    assert_eq!(code, 2);
    assert_eq!(result["complete"], false);
    assert_eq!(result["termination"], "execution_incomplete");
    let ledger: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(result["evidence"].as_str().unwrap()).join("index.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(ledger["attempts"].as_array().unwrap().is_empty());
    assert!(ledger["error"].is_string());
    let mut envelope = vec!["--envelope"];
    envelope.extend(args);
    assert!(
        dispatch(root.path(), &envelope)
            .await
            .unwrap_err()
            .to_string()
            .contains("lifecycle protocol")
    );
}
