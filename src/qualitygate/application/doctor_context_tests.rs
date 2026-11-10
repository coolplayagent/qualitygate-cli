use super::*;
use serde_json::json;

async fn report() -> Report {
    let mut report =
        super::super::doctor::invalid_request("full".into(), true, "unit fixture").await;
    report.diagnostics.clear();
    report.checks.clear();
    report
}

fn plan(commands: serde_json::Value) -> Plan {
    let config = crate::config::parse(b"schema_version: 1\n").unwrap();
    let mut plan = Plan::build(&config, None, "full").unwrap();
    plan.commands = serde_json::from_value(commands).unwrap();
    plan.order = plan.commands.iter().map(|c| c.id.clone()).collect();
    plan
}

#[tokio::test]
async fn static_inspection_collects_all_missing_inputs_without_executing_commands() {
    let root = tempfile::tempdir().unwrap();
    let mut snapshot = super::super::test_support::snapshot(root.path());
    snapshot.files.insert(
        "report.json".into(),
        snapshot::File {
            bytes: b"source".to_vec(),
            executable: false,
        },
    );
    snapshot.base_files = snapshot.files.clone();
    let plan = plan(json!([
        {"id":"bad-cwd","argv":["git"],"cwd":"missing"},
        {"id":"bad-tool","argv":["missing-unit-command"],"required_args":["--must-exist"],"required_env":["QUALITYGATE_UNIT=ABSENT"],
            "reports":[{"path":"report.json","format":"diagnostics","mode":"new_diagnostics"}]},
        {"id":"probe","argv":["git","--invalid-must-not-run"],"tools":[{"id":"missing","argv":["missing-unit-probe"],"inputs":["missing-input"]}]},
        {"id":"manual","kind":"manual","evidence_file":"review.json"}
    ]));
    let mut report = report().await;
    inspect(&plan, root.path(), &snapshot, &mut report).await;
    let codes: std::collections::BTreeSet<_> =
        report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    for expected in [
        "command.cwd_missing",
        "command.executable_missing",
        "command.argument_missing",
        "environment.missing",
        "probe.declaration_missing",
        "command.output_overwrites_input",
        "command.baseline_output_overwrites_input",
        "probe.input_missing",
        "probe.executable_missing",
    ] {
        assert!(codes.contains(expected), "missing {expected}: {codes:?}");
    }
    assert_eq!(report.checks.len(), 3);
    assert!(
        report
            .checks
            .iter()
            .flat_map(|c| &c.probes)
            .all(|p| !p.attempted)
    );
}

#[tokio::test]
async fn probe_deadline_and_changed_snapshot_prevent_version_execution() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.rs"), b"original").unwrap();
    let mut snapshot = super::super::test_support::snapshot(root.path());
    snapshot.files.insert(
        "source.rs".into(),
        snapshot::File {
            bytes: b"original".to_vec(),
            executable: false,
        },
    );
    let snapshot = Arc::new(snapshot);
    let guard = snapshot::InputGuard::for_snapshot(root.path(), snapshot.clone())
        .await
        .unwrap();
    let plan = plan(
        json!([{"id":"check","argv":["git","--version"],"tools":[{"id":"git","argv":["git","--version"]}]}]),
    );
    let mut first = report().await;
    inspect(&plan, root.path(), &snapshot, &mut first).await;
    probe_until(
        &plan,
        root.path(),
        &snapshot,
        &guard,
        &mut first,
        Instant::now() - Duration::from_secs(1),
    )
    .await
    .unwrap();
    assert!(
        first
            .diagnostics
            .iter()
            .any(|d| d.code == "probe.budget_exhausted")
    );
    assert!(!first.checks[0].probes[0].attempted);
    let mut changed = report().await;
    inspect(&plan, root.path(), &snapshot, &mut changed).await;
    std::fs::write(root.path().join("source.rs"), b"changed").unwrap();
    probe_until(
        &plan,
        root.path(),
        &snapshot,
        &guard,
        &mut changed,
        Instant::now() + Duration::from_secs(10),
    )
    .await
    .unwrap();
    assert!(
        changed
            .diagnostics
            .iter()
            .any(|d| d.code == "snapshot.inputs_changed")
    );
    assert!(!changed.checks[0].probes[0].attempted);
}

#[cfg(unix)]
#[tokio::test]
async fn probe_failure_mutation_and_expired_execution_have_distinct_readiness_diagnostics() {
    for (script, expected, timeout) in [
        ("printf version; exit 1", "probe.exit_code", false),
        (
            "printf changed > source.rs; printf version",
            "snapshot.inputs_changed",
            false,
        ),
        ("sleep 1; printf version", "probe.budget_exhausted", true),
    ] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("source.rs"), "original").unwrap();
        let mut snapshot = super::super::test_support::snapshot(root.path());
        snapshot.files.insert(
            "source.rs".into(),
            snapshot::File {
                bytes: b"original".to_vec(),
                executable: false,
            },
        );
        let snapshot = Arc::new(snapshot);
        let guard = snapshot::InputGuard::for_snapshot(root.path(), snapshot.clone())
            .await
            .unwrap();
        let plan = plan(
            json!([{"id":"unit","argv":["sh","-c","exit 0"],"tools":[{"id":"version","argv":["sh","-c",script]}]}]),
        );
        let mut report = report().await;
        inspect(&plan, root.path(), &snapshot, &mut report).await;
        probe_until(
            &plan,
            root.path(),
            &snapshot,
            &guard,
            &mut report,
            Instant::now()
                + if timeout {
                    Duration::from_millis(50)
                } else {
                    Duration::from_secs(5)
                },
        )
        .await
        .unwrap();
        if !timeout {
            assert!(report.checks[0].probes[0].attempted);
        }
        assert!(!report.checks[0].probes[0].complete);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == expected || (timeout && d.code == "probe.timeout")),
            "{:#?}",
            report.diagnostics
        );
    }
}

#[tokio::test]
async fn changed_tool_identity_is_rejected_before_any_probe_is_started() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = Arc::new(super::super::test_support::snapshot(root.path()));
    let guard = snapshot::InputGuard::for_snapshot(root.path(), snapshot.clone())
        .await
        .unwrap();
    let plan = plan(
        json!([{"id":"unit","argv":["git","--version"],"tools":[{"id":"git","argv":["git","--version"]}]}]),
    );
    let mut report = report().await;
    inspect(&plan, root.path(), &snapshot, &mut report).await;
    report.checks[0].probes[0]
        .executable
        .as_mut()
        .unwrap()
        .digest = "old tool".into();
    probe(&plan, root.path(), &snapshot, &guard, &mut report)
        .await
        .unwrap();
    assert!(!report.checks[0].probes[0].attempted);
    assert!(!report.checks[0].probes[0].complete);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "probe.executable_changed")
    );
}
