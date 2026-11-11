//! Controller unit scenarios use a fixed executable protocol double, never another CLI build.
use super::*;
use crate::domain::{CheckResult, Diagnostic, Severity, Summary, evaluate};

#[cfg(unix)]
async fn fixture(scenario: &str) -> (tempfile::TempDir, tempfile::TempDir, Options) {
    use std::os::unix::fs::PermissionsExt;
    let root = super::super::test_support::repository("schema_version: 1\n").await;
    let external = super::super::test_support::tempdir();
    let mut options = super::tests::options(root.path(), external.path());
    options.timeout_seconds = 20;
    options.base = resolve(root.path(), "HEAD").await.unwrap();
    options.policy_ref = options.base.clone();
    let runtime = external.path().join("evaluator");
    let script = r#"#!/bin/sh
fixture='DIRECTORY'
for arg in "$@"; do
 if [ "$arg" = capabilities ]; then cat "$fixture/capabilities.json"; exit 0; fi
 if [ "$arg" = agent-contract ]; then
  if [ -f "$fixture/after" ]; then cat "$fixture/after-contract.json"; else cat "$fixture/before-contract.json"; fi
  exit CONTRACT_EXIT
 fi
done
if [ -f "$fixture/after" ]; then cat "$fixture/after-report.json"; exit AFTER_EXIT; fi
cat "$fixture/before-report.json"
exit BEFORE_EXIT
"#.replace("DIRECTORY", external.path().to_str().unwrap())
        .replace("CONTRACT_EXIT", if scenario == "contract_gap" { "2" } else { "0" })
        .replace("AFTER_EXIT", if scenario == "no_progress" || scenario == "attempt_budget" { "1" } else { "0" })
        .replace("BEFORE_EXIT", if scenario == "incomplete" { "2" } else { "1" });
    std::fs::write(&runtime, script).unwrap();
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o755)).unwrap();
    options.qualitygate = runtime.clone();
    let identity = crate::runner::identity::selected_evaluator(&runtime)
        .await
        .unwrap();
    let mut caps = super::super::capabilities::report().await.unwrap();
    caps.executable = identity.clone();
    std::fs::write(
        external.path().join("capabilities.json"),
        serde_json::to_vec(&caps).unwrap(),
    )
    .unwrap();
    let mut policy = super::super::test_support::policy();
    policy.resolved_commit = Some(options.policy_ref.clone());
    policy.task_contract_source = Some(options.task.clone());
    policy.task_contract_digest = Some(snapshot::digest(b"fixed task"));
    let mut current = super::super::test_support::snapshot(root.path());
    current.identity.base = options.base.clone();
    current.identity.verification_digest = Some(snapshot::digest(b"delivery scope"));
    current.files.insert(
        "src/lib.rs".into(),
        snapshot::File {
            bytes: b"fixed code".to_vec(),
            executable: false,
        },
    );
    current.identity.content_digest = snapshot::content_digest(&current.files);
    let delivered = snapshot::delivery::export(&current, external.path()).unwrap();
    let mut before = current.identity.clone();
    before.content_digest = snapshot::digest(b"before repair");
    let mut failing = CheckResult::pending("behavior", true, Severity::Error);
    failing.diagnostics.push(Diagnostic {
        id: "behavior".into(),
        fingerprint: "bug".into(),
        file: None,
        range: None,
        message: "Expected behavior missing".into(),
        evidence: json!({}),
        fix: "Repair the behavior".into(),
        recheck: Default::default(),
    });
    failing.complete();
    if scenario == "incomplete" {
        failing.block(crate::domain::ExecutionStatus::Blocked, "missing input");
    }
    let initial: Report = serde_json::from_value(json!({"schema_version":1,"run_id":"before","scope":"task","profile":"full",
        "evaluator_digest":identity.digest,"snapshot":before,"policy":policy,
        "context":{"report_path":external.path().join("before-report.json"),"recheck":{"argv":["qualitygate","check"]},"delivery_recheck":{"argv":["qualitygate","check","--profile","full"]}},
        "plan":{"task_id":"task","required_checks":["behavior"],"pending_delivery_checks":[],"acceptance":{}},
        "gate":evaluate(&[failing.clone()], &["behavior".into()], &[]),"checks":[failing],"summary":Summary::default()})).unwrap();
    std::fs::write(
        external.path().join("before-report.json"),
        serde_json::to_vec(&initial).unwrap(),
    )
    .unwrap();
    let mut final_report = initial.clone();
    if !["no_progress", "attempt_budget"].contains(&scenario) {
        final_report.snapshot = current.identity.clone();
        final_report.checks[0].diagnostics.clear();
        final_report.checks[0].complete();
        final_report.gate = evaluate(&final_report.checks, &["behavior".into()], &[]);
    }
    std::fs::write(external.path().join("after-report.json"), serde_json::to_vec(&json!({"schema_version":1,"kind":"delivery_check","report":final_report,"delivery":delivered})).unwrap()).unwrap();
    let projection = json!({"schema_version":1,"kind":"agent_contract_projection","repository":options.root,"role":"implementer","task_id":"task",
        "snapshot":initial.snapshot,"policy":policy,"contract_digest":snapshot::digest(b"contract"),"prompt":"Bound instructions",
        "prompt_digest":snapshot::digest(b"Bound instructions"),"ready":scenario != "contract_gap","obligations":[],"gaps":[],"known_limits":[]});
    std::fs::write(
        external.path().join("before-contract.json"),
        serde_json::to_vec(&projection).unwrap(),
    )
    .unwrap();
    let mut after_projection = projection;
    after_projection["snapshot"] = serde_json::to_value(&final_report.snapshot).unwrap();
    std::fs::write(
        external.path().join("after-contract.json"),
        serde_json::to_vec(&after_projection).unwrap(),
    )
    .unwrap();
    let agent = if scenario == "agent_failed" {
        "cat >/dev/null; printf failed >&2; exit 7".into()
    } else {
        format!(
            "cat >/dev/null; touch '{}'; printf '%s\\n' '{{\"type\":\"turn.completed\",\"usage\":{{\"input_tokens\":7}}}}'",
            external.path().join("after").display()
        )
    };
    std::fs::write(
        &options.agent_command,
        serde_json::to_vec(&vec!["sh", "-c", &agent]).unwrap(),
    )
    .unwrap();
    if scenario == "attempt_budget" {
        options.max_attempts = 1;
    }
    (root, external, options)
}

#[cfg(unix)]
#[tokio::test]
async fn controller_distinguishes_incomplete_contract_failed_agent_and_no_progress() {
    for (scenario, expected, attempts) in [
        ("incomplete", "incomplete_check", 0),
        ("contract_gap", "contract_incomplete", 0),
        ("agent_failed", "agent_failed", 1),
        ("no_progress", "no_progress", 2),
        ("attempt_budget", "attempt_budget", 1),
    ] {
        let (_root, _external, options) = fixture(scenario).await;
        let result = run(options).await.unwrap();
        let ledger: Value = serde_json::from_slice(
            &std::fs::read(Path::new(result["evidence"].as_str().unwrap()).join("index.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["termination"], expected, "{ledger:#}");
        assert_eq!(result["complete"], false);
        assert_eq!(ledger["attempts"].as_array().unwrap().len(), attempts);
        if attempts > 0 {
            assert_eq!(ledger["attempts"][0]["input_channel"], "stdin_provided");
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn accepted_delivery_binds_current_projection_receipt_and_immutable_archive() {
    let (_root, _external, mut options) = fixture("accepted").await;
    options.feedback_mode = FeedbackMode::Compact;
    let result = run(options).await.unwrap();
    let evidence = Path::new(result["evidence"].as_str().unwrap());
    let ledger: Value =
        serde_json::from_slice(&std::fs::read(evidence.join("index.json")).unwrap()).unwrap();
    assert_eq!(result["termination"], "accepted", "{ledger:#}");
    assert_eq!(result["complete"], true);
    assert_eq!(ledger["attempts"][0]["agent"]["usage"]["input_tokens"], 7);
    assert_eq!(ledger["attempts"][0]["injection"]["event"], "start");
    assert_eq!(ledger["attempts"][0]["status"], "checked");
    let prompt = std::fs::read(evidence.join("attempt-1-prompt.txt")).unwrap();
    assert_eq!(
        ledger["attempts"][0]["injection"]["input_digest"],
        snapshot::digest(&prompt)
    );
    let delivery = snapshot::delivery::identify(Path::new(
        result["delivery"]["artifact"]["path"].as_str().unwrap(),
    ))
    .unwrap();
    assert_eq!(result["delivery"]["artifact"]["digest"], delivery.digest);
}

#[cfg(unix)]
#[tokio::test]
async fn controller_rejects_protocol_substitution_before_authorizing_delivery() {
    for (file, pointer, value, reason) in [
        (
            "before-report.json",
            "/evaluator_digest",
            json!("wrong runtime"),
            "identity/exit code",
        ),
        (
            "before-report.json",
            "/profile",
            json!("quick"),
            "fixed task, policy, base or full scope",
        ),
        (
            "before-contract.json",
            "/snapshot/content_digest",
            json!(snapshot::digest(b"other snapshot")),
            "Inputs changed",
        ),
        (
            "after-report.json",
            "/kind",
            json!("unrecognized"),
            "Unsupported delivery report",
        ),
        (
            "after-report.json",
            "/report/policy/task_contract_digest",
            json!(snapshot::digest(b"different task")),
            "Selected task identity changed",
        ),
        (
            "after-report.json",
            "/delivery",
            Value::Null,
            "did not export immutable delivery",
        ),
    ] {
        let (_root, external, options) = fixture("accepted").await;
        let path = external.path().join(file);
        let mut record: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        *record.pointer_mut(pointer).unwrap() = value;
        std::fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
        let result = run(options).await.unwrap();
        let ledger: Value = serde_json::from_slice(
            &std::fs::read(Path::new(result["evidence"].as_str().unwrap()).join("index.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["complete"], false);
        assert_eq!(result["termination"], "execution_incomplete");
        assert!(
            ledger["error"].as_str().unwrap().contains(reason),
            "{ledger}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn replacement_mode_applies_only_controller_validated_file_from_plain_or_event_response() {
    for wrapped in [false, true] {
        let (root, external, mut options) = fixture("accepted").await;
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/source.rs"), "// original").unwrap();
        options.replacement_file = Some("src/source.rs".into());
        let proposal = json!({"path":"src/source.rs","content":"// repaired behavior\n"});
        let response = if wrapped {
            json!({"type":"item.completed","item":{"type":"agent_message","text":proposal.to_string()}})
        } else {
            proposal
        };
        let response_path = external.path().join("proposal.json");
        std::fs::write(&response_path, serde_json::to_vec(&response).unwrap()).unwrap();
        let script = format!(
            "cat >/dev/null; touch '{}'; cat '{}'",
            external.path().join("after").display(),
            response_path.display()
        );
        std::fs::write(
            &options.agent_command,
            serde_json::to_vec(&vec!["sh", "-c", &script]).unwrap(),
        )
        .unwrap();
        let result = run(options).await.unwrap();
        let evidence = Path::new(result["evidence"].as_str().unwrap());
        let ledger: Value =
            serde_json::from_slice(&std::fs::read(evidence.join("index.json")).unwrap()).unwrap();
        assert_eq!(result["termination"], "accepted", "{ledger}");
        assert_eq!(
            std::fs::read_to_string(root.path().join("src/source.rs")).unwrap(),
            "// repaired behavior\n"
        );
        assert!(
            std::fs::read_to_string(evidence.join("attempt-1-prompt.txt"))
                .unwrap()
                .contains("Read-only proposal mode")
        );
        assert!(ledger["attempts"][0]["replacement"].is_object());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn controller_rechecks_contract_checkout_and_timeout_after_agent_execution() {
    for scenario in ["contract-change", "checkout-change", "timeout", "resume"] {
        let (_repository, external, mut options) = fixture(if scenario == "contract-change" {
            "no_progress"
        } else {
            "accepted"
        })
        .await;
        match scenario {
            "contract-change" => {
                let path = external.path().join("after-contract.json");
                let mut record: Value =
                    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                record["contract_digest"] = json!(snapshot::digest(b"changed contract"));
                std::fs::write(path, record.to_string()).unwrap();
            }
            "checkout-change" => {
                let script = format!(
                    "cat >/dev/null; git -c user.name=Unit -c user.email=unit@example.invalid -c commit.gpgsign=false commit --allow-empty -qm changed; touch '{}'",
                    external.path().join("after").display()
                );
                std::fs::write(
                    &options.agent_command,
                    json!(["sh", "-c", script]).to_string(),
                )
                .unwrap();
            }
            "timeout" => {
                options.timeout_seconds = 1;
                std::fs::write(
                    &options.agent_command,
                    json!(["sh", "-c", "cat >/dev/null; sleep 5"]).to_string(),
                )
                .unwrap();
            }
            _ => {
                let previous = external.path().join("previous.json");
                std::fs::write(&previous,json!({"kind":"external_agent_loop","root":options.root,"attempts":[],"agent_argv":["sh","-c","old agent"],"termination":"attempt_budget"}).to_string()).unwrap();
                options.resume = Some(previous);
                options.trust_store = Some(external.path().join("trust.json"));
                options.evidence_dir = Some(external.path().into());
            }
        }
        let result = run(options).await.unwrap();
        let ledger: Value = serde_json::from_slice(
            &std::fs::read(Path::new(result["evidence"].as_str().unwrap()).join("index.json"))
                .unwrap(),
        )
        .unwrap();
        match scenario {
            "resume" => {
                assert_eq!(result["complete"], true, "{ledger:#}");
                assert_eq!(ledger["attempts"][0]["injection"]["event"], "handoff");
            }
            "timeout" => {
                assert_eq!(result["complete"], false);
                assert_eq!(result["termination"], "time_budget", "{ledger:#}");
            }
            _ => {
                assert_eq!(result["complete"], false);
                assert!(
                    ledger["error"]
                        .as_str()
                        .unwrap()
                        .contains(if scenario == "checkout-change" {
                            "checkout commit"
                        } else {
                            "contract changed"
                        }),
                    "{ledger:#}"
                );
            }
        }
    }
}
