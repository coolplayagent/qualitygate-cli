//! Real bounded producer execution feeding multiple V4 rules in one prepared run.
#![cfg(unix)]
use crate::{application, config, domain::*, snapshot};
use serde_json::json;
use std::sync::Arc;

async fn run(stale: bool, missing_dependency: bool) -> anyhow::Result<Report> {
    run_rules(
        stale,
        missing_dependency,
        &[("strict".into(), 10), ("relaxed".into(), 20)],
        1,
    )
    .await
}

async fn run_rules(
    stale: bool,
    missing_dependency: bool,
    rules: &[(String, u32)],
    entities: usize,
) -> anyhow::Result<Report> {
    let root = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    let mut snapshot = application::test_support::snapshot(root.path());
    snapshot.scope_evidence.mode = check_scope::CheckScope::Delivery;
    let source = application::test_support::source(&mut snapshot);
    let mut files = std::collections::BTreeMap::new();
    let mut settings = std::collections::BTreeMap::new();
    for (id, limit) in rules {
        settings.insert(id, json!({"depends_on": if id == "strict" && missing_dependency {vec![]} else {vec!["scan"]}}));
        let rule = json!({"schema_version":4,"id":id,"version":1,"source":source,
            "requires_capabilities":["files"],"when":{"entity":"file","change":"all"},"fix":"Reduce complexity",
            "fact_sources":{"metrics":{"producer":"scan","schema":"function_metric","schema_version":1,"project":"app","metric":"cc","unit":"count","measurement":"cc-v1"}},
            "assertion":{"op":"all","input":{"op":"source","name":"metrics"},"predicate":{"op":"compare","left":{"op":"field","name":"value"},"relation":"le","right":{"op":"number","value":limit}}}});
        files.insert(
            format!("qualitygate/rules/{id}.yaml"),
            snapshot::File {
                bytes: serde_json::to_vec(&rule)?,
                executable: false,
            },
        );
    }
    let mut source_text = String::new();
    let mut inventory = Vec::new();
    let mut records = Vec::new();
    for index in 1..=entities {
        let name = format!("sample_{index}");
        source_text.push_str(&format!("fn {name}() {{}}\n"));
        let location = json!({"file":"src/lib.rs", "start_line":index, "end_line":index, "symbol":format!("app::{name}")});
        inventory.push(json!({"namespace":"app","name":name,"location":location}));
        records.push(json!({"kind":"metric","id":format!("{name}-cc"),"location":location,"metric":"cc","value":{"coefficient":"15","scale":0},"unit":"count","measurement":"cc-v1"}));
    }
    let raw = json!({"protocol_version":1,"schema":"function_metric","schema_version":1,"producer":"scan","snapshot_digest":if stale {"sha256:0000000000000000000000000000000000000000000000000000000000000000"}else{"SNAPSHOT"},
        "tool_id":"analyzer","tool_version":"generic-v1","artifact_digest":"untrusted-replaced-by-application","scope":{"project":"app"},"complete":true,"inventory":{"files":["src/lib.rs"],"entities":inventory},
        "metric_definitions":[{"metric":"cc","unit":"count","measurement":"cc-v1"}],
        "records":records});
    files.insert(
        "facts-template.json".into(),
        snapshot::File {
            bytes: serde_json::to_vec(&raw)?,
            executable: false,
        },
    );
    files.insert(
        "src/lib.rs".into(),
        snapshot::File {
            bytes: source_text.into_bytes(),
            executable: false,
        },
    );
    let script = r#"test ! -e scan-once || exit 7
printf x > scan-once
digest=$(sed -n 's/.*"snapshot_digest":"\([^"]*\)".*/\1/p' fact-context.json)
test -n "$digest" || exit 8
sed "s/SNAPSHOT/$digest/" facts-template.json > facts.json
"#;
    let value = json!({"schema_version":1,"custom_rules":"qualitygate/rules",
        "rules":settings,
        "checks":[{"id":"scan","argv":["sh","-c",script],"when_changed":{"paths":["never/**"]},
            "tools":[{"id":"analyzer","argv":["sh","-c","printf generic-v1"]}],
            "tool_facts":{"adapter":"tool_facts","schema":"function_metric","path":"facts.json","context_path":"fact-context.json","tool_id":"analyzer","tool_version":"generic-v1","scope":{"project":"app"},"paths":["src/lib.rs"],"crate_name":"app",
                "metrics":[{"metric":"cc","unit":"count","measurement":"cc-v1"}]}}]});
    let config: config::Config = serde_json::from_value(value)?;
    let catalog = config::catalog::Catalog::load(
        &config,
        files
            .iter()
            .map(|(path, file)| (path.as_str(), file.bytes.as_slice())),
    )?;
    let mut config = catalog.resolve(&config)?;
    for (id, review) in config::source_reviews::evidence(&config, &catalog)? {
        let mut adoption = application::test_support::adoption(&review.expected_binding_digest);
        adoption.id = format!("adopt-{id}");
        config.source_reviews.insert(id, adoption);
    }
    files.insert(
        "qualitygate.yaml".into(),
        snapshot::File {
            bytes: serde_json::to_vec(&config)?,
            executable: false,
        },
    );
    snapshot.files.extend(files);
    snapshot.identity.content_digest = snapshot::content_digest(&snapshot.files);
    snapshot.base_files = snapshot.files.clone();
    let options = application::CheckOptions {
        root: root.path().into(),
        config: "qualitygate.yaml".into(),
        selection: snapshot::Selection::Worktree {
            base: "HEAD".into(),
        },
        snapshot_options: snapshot::CaptureOptions {
            scope: check_scope::CheckScope::Delivery,
            ..Default::default()
        },
        profile: "full".into(),
        task: None,
        policy_ref: None,
        output_dir: Some(output.path().into()),
        trust_store: None,
        evidence_dir: None,
    };
    let snapshot = Arc::new(snapshot);
    let loaded = application::policy::load_plan(&snapshot, &options).await?;
    let report =
        super::check_prepared(options, snapshot, loaded, vec![], false, None, None).await?;
    if !stale {
        let scan = report
            .checks
            .iter()
            .find(|check| check.id == "scan")
            .unwrap();
        let file = scan.metadata["tool_facts"]["artifact"]["path"]
            .as_str()
            .unwrap();
        let envelope = serde_json::from_slice(&tokio::fs::read(file).await?)?;
        assert!(jsonschema::is_valid(
            &crate::tool_evidence::schema::document(),
            &envelope
        ));
    }
    Ok(report)
}

#[tokio::test]
async fn one_completed_producer_feeds_two_rules_with_independent_threshold_verdicts() {
    let report = run(false, false).await.unwrap();
    let result = |id: &str| report.checks.iter().find(|result| result.id == id).unwrap();
    assert_eq!(
        report
            .checks
            .iter()
            .filter(|result| result.id == "scan")
            .count(),
        1
    );
    assert_eq!(
        result("scan").verdict,
        Some(Verdict::Pass),
        "{:?}",
        result("scan")
    );
    assert_eq!(
        result("strict").verdict,
        Some(Verdict::Fail),
        "{:?}",
        result("strict")
    );
    assert_eq!(
        result("relaxed").verdict,
        Some(Verdict::Pass),
        "{:?}",
        result("relaxed")
    );
    assert_eq!(report.gate.decision, Decision::Fail);
    assert!(result("scan").metadata["change_selection"]["required_by"].is_string());
    assert_eq!(
        result("strict").metadata["assertion"]["sources"]["metrics"],
        1
    );
    assert_eq!(
        result("relaxed").metadata["assertion"]["sources"]["metrics"],
        1
    );
}

#[tokio::test]
async fn undeclared_dependency_and_stale_producer_cannot_support_assertions() {
    assert!(
        run(false, true)
            .await
            .unwrap_err()
            .to_string()
            .contains("declared dependency")
    );
    let report = run(true, false).await.unwrap();
    assert_eq!(report.gate.decision, Decision::Incomplete);
    for result in &report.checks {
        assert_ne!(result.verdict, Some(Verdict::Pass));
        assert_eq!(
            result.execution.status,
            if result.id == "scan" {
                ExecutionStatus::ToolError
            } else {
                ExecutionStatus::Blocked
            }
        );
    }
}

async fn verify_fanout(count: usize) {
    let rules: Vec<_> = (0..count)
        .map(|i| (format!("consumer-{i}"), if i % 2 == 0 { 10 } else { 20 }))
        .collect();
    let started = std::time::Instant::now();
    let report = run_rules(false, false, &rules, 128).await.unwrap();
    assert_eq!(report.gate.decision, Decision::Fail);
    assert_eq!(
        report
            .checks
            .iter()
            .filter(|check| check.id == "scan")
            .count(),
        1
    );
    for (id, limit) in rules {
        let check = report.checks.iter().find(|check| check.id == id).unwrap();
        assert_eq!(
            check.verdict,
            Some(if limit < 15 {
                Verdict::Fail
            } else {
                Verdict::Pass
            })
        );
        assert_eq!(check.metadata["assertion"]["sources"]["metrics"], 128);
    }
    println!(
        "128 metric records, {count} V4 consumers, one producer: {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn fanout_1_consumer() {
    verify_fanout(1).await;
}

#[tokio::test]
async fn fanout_10_consumers() {
    verify_fanout(10).await;
}

#[tokio::test]
async fn fanout_100_consumers() {
    verify_fanout(100).await;
}
