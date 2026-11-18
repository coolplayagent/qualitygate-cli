use super::*;
use crate::{
    domain::{ExecutionStatus, Severity, Verdict},
    snapshot::{self, File},
};
use serde_json::{Value, json};

fn producer(stdout: bool) -> CommandCheck {
    serde_json::from_value(json!({"id":"metrics","argv":["sh","-c",if stdout {"cat raw.json"} else {"cat raw.json > facts.json"}],"timeout_seconds":2,
        "tools":[{"id":"fixture","argv":["sh","-c","printf fixture-v1"]}],
        "tool_facts":{"adapter":"tool_facts","schema":"function_metric","path":"facts.json","from_stdout":stdout,"tool_id":"fixture","tool_version":"fixture-v1","scope":{"project":"sample"},"paths":["src/lib.rs"],"crate_name":"sample","metrics":[{"metric":"complexity","unit":"count","measurement":"fixture-v1"}],"context_path":"run/context.json"}
    })).unwrap()
}

fn raw(snapshot: &Snapshot) -> Value {
    json!({"protocol_version":1,"schema":"function_metric","schema_version":1,"producer":"metrics","snapshot_digest":snapshot.identity.content_digest,
        "tool_id":"fixture","tool_version":"fixture-v1","artifact_digest":"external-claim-replaced-by-actual-bytes","scope":{"project":"sample"},"complete":true,
        "minimum_lines":1,"minimum_nodes":0,"similarity_floor":{"coefficient":"0","scale":0},"metric_definitions":[{"metric":"complexity","unit":"count","measurement":"fixture-v1"}],
        "inventory":{"files":["src/lib.rs"],"entities":[{"namespace":"sample","name":"sample","location":{"file":"src/lib.rs","start_line":1,"end_line":1,"symbol":"sample::sample"}}]},
        "records":[{"kind":"metric","id":"function-1","location":{"file":"src/lib.rs","start_line":1,"end_line":1,"symbol":"sample::sample"},"metric":"complexity","value":{"coefficient":"99","scale":0},"unit":"count","measurement":"fixture-v1"}]})
}

#[cfg(unix)]
#[tokio::test]
async fn actual_capture_publishes_complete_typed_metrics_without_failing_on_high_values() {
    let workspace = tempfile::tempdir().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut snapshot = crate::application::test_support::snapshot(workspace.path());
    snapshot.files.insert(
        "src/lib.rs".into(),
        File {
            bytes: b"fn sample() {}\n".to_vec(),
            executable: false,
        },
    );
    std::fs::create_dir(workspace.path().join("src")).unwrap();
    std::fs::write(
        workspace.path().join("src/lib.rs"),
        &snapshot.files["src/lib.rs"].bytes,
    )
    .unwrap();
    std::fs::write(
        workspace.path().join("raw.json"),
        serde_json::to_vec(&raw(&snapshot)).unwrap(),
    )
    .unwrap();
    let snapshot = Arc::new(snapshot);
    let guard = snapshot::InputGuard::for_snapshot(workspace.path(), snapshot.clone())
        .await
        .unwrap();
    for stdout in [true, false] {
        let check = producer(stdout);
        let outcome = super::super::commands::execute_outcome(
            &check,
            workspace.path(),
            directory.path(),
            &snapshot,
            &guard,
            None,
            None,
        )
        .await;
        assert_eq!(
            outcome.check_result.verdict,
            Some(Verdict::Pass),
            "{:?}",
            outcome.check_result.execution.reason
        );
        let facts = outcome.evidence.tool_facts.as_ref().unwrap();
        assert!(facts.complete);
        assert_eq!(facts.records.len(), 1);
        assert_eq!(
            facts.artifact_digest,
            snapshot::digest(&serde_json::to_vec(&raw(&snapshot)).unwrap())
        );
        assert!(outcome.evidence.receipt.is_some());
        let context: Value = serde_json::from_slice(
            &std::fs::read(workspace.path().join("run/context.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(context["snapshot_digest"], snapshot.identity.content_digest);
        let consumer: CommandCheck = serde_json::from_value(
            json!({"id":"consumer","argv":["git","--version"],"depends_on":["metrics"]}),
        )
        .unwrap();
        let config: crate::config::Config =
            serde_json::from_value(json!({"schema_version":1,"checks":[check, consumer]})).unwrap();
        let plan = crate::config::Plan::build(&config, None, "full").unwrap();
        let mut consumer = CheckResult::pending("consumer", true, Severity::Error);
        consumer.complete();
        let mut results = vec![outcome.check_result.clone(), consumer];
        let mut context = super::super::rule_dispatch::Context::new(snapshot.clone());
        context.publish("metrics", outcome.evidence);
        super::super::fact_revalidation::finalize(
            &plan,
            &context,
            Some(workspace.path()),
            &mut results,
            false,
        )
        .await;
        assert!(
            results
                .iter()
                .all(|result| result.verdict == Some(Verdict::Pass))
        );
        std::fs::write(&context.receipts["metrics"].artifacts[0].path, "tampered").unwrap();
        super::super::fact_revalidation::finalize(
            &plan,
            &context,
            Some(workspace.path()),
            &mut results,
            false,
        )
        .await;
        assert!(
            results
                .iter()
                .all(|result| result.execution.status == ExecutionStatus::Blocked
                    && result.verdict.is_none())
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn declared_findings_exit_can_publish_facts_but_error_timeout_or_stale_evidence_cannot() {
    let workspace = tempfile::tempdir().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut snapshot = crate::application::test_support::snapshot(workspace.path());
    snapshot.files.insert(
        "src/lib.rs".into(),
        File {
            bytes: b"fn sample() {}\n".to_vec(),
            executable: false,
        },
    );
    std::fs::create_dir(workspace.path().join("src")).unwrap();
    std::fs::write(
        workspace.path().join("src/lib.rs"),
        &snapshot.files["src/lib.rs"].bytes,
    )
    .unwrap();
    let snapshot = Arc::new(snapshot);
    let guard = snapshot::InputGuard::for_snapshot(workspace.path(), snapshot.clone())
        .await
        .unwrap();
    for (script, expected, stale) in [
        ("cat raw.json; exit 2", ExecutionStatus::Completed, false),
        ("cat raw.json; exit 9", ExecutionStatus::ToolError, false),
        ("cat raw.json", ExecutionStatus::ToolError, true),
        (
            "printf changed > run/context.json; cat raw.json",
            ExecutionStatus::ToolError,
            false,
        ),
        ("sleep 3; cat raw.json", ExecutionStatus::TimedOut, false),
    ] {
        let mut check = producer(true);
        check.argv[2] = script.into();
        check.timeout_seconds = 1;
        check.findings_exit_codes = vec![2];
        let mut value = raw(&snapshot);
        if stale {
            value["snapshot_digest"] = json!(snapshot::digest(b"old snapshot"));
        }
        std::fs::write(
            workspace.path().join("raw.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        let outcome = super::super::commands::execute_outcome(
            &check,
            workspace.path(),
            directory.path(),
            &snapshot,
            &guard,
            None,
            None,
        )
        .await;
        assert_eq!(
            outcome.check_result.execution.status, expected,
            "{script}: {:?}",
            outcome.check_result.execution.reason
        );
        assert_eq!(
            outcome.evidence.tool_facts.is_some(),
            expected == ExecutionStatus::Completed
        );
    }
}

#[tokio::test]
async fn context_overwrite_and_changed_context_are_rejected() {
    let workspace = tempfile::tempdir().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut snapshot = crate::application::test_support::snapshot(workspace.path());
    let check = producer(true);
    let mut result = CheckResult::pending("metrics", true, Severity::Warning);
    snapshot.files.insert(
        "run/context.json".into(),
        File {
            bytes: vec![],
            executable: false,
        },
    );
    assert!(
        prepare_context(
            &check,
            workspace.path(),
            directory.path(),
            &snapshot,
            &mut result
        )
        .await
        .is_err()
    );
    snapshot.files.clear();
    let context = prepare_context(
        &check,
        workspace.path(),
        directory.path(),
        &snapshot,
        &mut result,
    )
    .await
    .unwrap();
    verify_context(context.as_ref()).await.unwrap();
    std::fs::write(workspace.path().join("run/context.json"), "changed").unwrap();
    assert!(verify_context(context.as_ref()).await.is_err());
}
