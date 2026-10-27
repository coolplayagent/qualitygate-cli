mod common;
use common::*;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::LazyLock,
};

static AGENT: LazyLock<(tempfile::TempDir, PathBuf)> = LazyLock::new(|| {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("agent.rs");
    std::fs::write(&source, include_str!("fixtures/consuming_agent.rs")).unwrap();
    let executable = root
        .path()
        .join(if cfg!(windows) { "agent.exe" } else { "agent" });
    let result = Command::new("rustc")
        .args(["--edition=2024", "-o"])
        .arg(&executable)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    (root, executable)
});
const SOURCE: &str = "# Host behavior\nHonor the current contract before delivery.\n";

#[cfg(unix)]
#[tokio::test]
async fn fifo_evaluator_is_rejected_before_opening_or_launching_the_agent() {
    let root = setup("immutable_delivery");
    let external = tempfile::tempdir().unwrap();
    let fifo = external.path().join("qualitygate-fifo");
    let created = Command::new("mkfifo").arg(&fifo).output().unwrap();
    assert!(created.status.success(), "{:?}", created.stderr);
    let state = external.path().join("agent-state");
    let command = external.path().join("agent.json");
    std::fs::write(&command, json!([AGENT.1, "forge", state]).to_string()).unwrap();
    let prompt = external.path().join("prompt.txt");
    std::fs::write(&prompt, "Exercise runtime identity validation.").unwrap();
    let args = vec![
        env!("CARGO_BIN_EXE_qualitygate").to_owned(),
        "agent-run".into(),
        "--qualitygate".into(),
        fifo.display().to_string(),
        "--base".into(),
        "HEAD".into(),
        "--policy-ref".into(),
        "HEAD".into(),
        "--task".into(),
        "task.yaml".into(),
        "--prompt-file".into(),
        prompt.display().to_string(),
        "--agent-command".into(),
        command.display().to_string(),
        "--output-dir".into(),
        external.path().display().to_string(),
        "--max-attempts".into(),
        "1".into(),
        "--timeout-seconds".into(),
        "1".into(),
        "--format".into(),
        "json".into(),
    ];
    // Bound the regression itself: old code blocks opening the FIFO before the
    // identity timer can run; cancellation must not leave the child CLI alive.
    let output =
        qualitygate::runner::capture(&args, root.path(), None, std::time::Duration::from_secs(5))
            .await
            .unwrap();
    assert!(
        !output.timed_out,
        "Evaluator FIFO stalled identity preflight"
    );
    assert_eq!(output.exit_code, Some(2));
    assert!(output.capture_error.is_none());
    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["complete"], false);
    assert_eq!(summary["termination"], "execution_incomplete");
    let ledger: Value = serde_json::from_slice(
        &std::fs::read(Path::new(summary["evidence"].as_str().unwrap()).join("index.json"))
            .unwrap(),
    )
    .unwrap();
    assert!(ledger["error"].as_str().unwrap().contains("regular file"));
    assert!(ledger["attempts"].as_array().unwrap().is_empty());
    assert!(ledger["delivery"].is_null());
    assert!(!state.with_extension("prompts").exists());
}

fn setup(control: &str) -> tempfile::TempDir {
    let root = fixture();
    std::fs::write(root.path().join("AGENTS.md"), SOURCE).unwrap();
    std::fs::write(root.path().join("task.yaml"), "schema_version: 1\ntask_id: repair\nacceptance:\n  - id: producer\n    description: Complete configured verification\n    verification: {kind: command, check_id: version, argv: [git, --version]}\n").unwrap();
    let mut policy = json!({"schema_version":1,"rules":{"line-ending":{}},"agent_obligations":[{
        "schema_version":1,"id":"host-contract","version":1,"required":true,
        "source":{"document":"AGENTS.md","section":"Host behavior","content_hash":qualitygate::snapshot::digest(SOURCE.as_bytes())},
        "scope":{"roles":[],"tasks":[]},"triggers":["before_delivery"],"must":["Honor the current contract before delivery"],"must_not":[],
        "enforcement":{"kind":"host","control":control}}]});
    let path = root.path().join("qualitygate.yaml");
    std::fs::write(&path, serde_norway::to_string(&policy).unwrap()).unwrap();
    let config = report(
        &cli(root.path(), &["config", "--show", "--format", "json"]),
        0,
    );
    policy["agent_obligations"][0]["adoption"] = json!({"id":"review-host","reviewer":"fixture reviewer","reference":"fixture independent policy",
        "resolution":"initial_mapping","rationale":"Reviewed the exact host delivery control","binding_digest":config["agent_bindings"]["host-contract"]});
    std::fs::write(path, serde_norway::to_string(&policy).unwrap()).unwrap();
    git(root.path(), &["add", "."]);
    git(
        root.path(),
        &["commit", "-qm", "adopt fixed task and host obligations"],
    );
    std::fs::write(root.path().join("hello.txt"), "bad\r\n").unwrap();
    root
}

fn run(
    root: &Path,
    output: &Path,
    mode: &str,
    attempts: &str,
    resume: Option<&Path>,
    replacement: bool,
    code: i32,
) -> (Value, PathBuf) {
    // Force fixture compilation before the bounded host session begins.
    let command = json!([AGENT.1, mode, output.join("agent-state")]);
    std::fs::write(output.join("agent.json"), command.to_string()).unwrap();
    std::fs::write(
        output.join("prompt.txt"),
        "Repair the selected source. Keep the fixed acceptance contract.",
    )
    .unwrap();
    let mut argv = vec![
        "agent-run".to_owned(),
        "--qualitygate".into(),
        env!("CARGO_BIN_EXE_qualitygate").into(),
        "--base".into(),
        "HEAD".into(),
        "--policy-ref".into(),
        "HEAD".into(),
        "--task".into(),
        "task.yaml".into(),
        "--prompt-file".into(),
        output.join("prompt.txt").display().to_string(),
        "--agent-command".into(),
        output.join("agent.json").display().to_string(),
        "--output-dir".into(),
        output.display().to_string(),
        "--max-attempts".into(),
        attempts.into(),
        "--timeout-seconds".into(),
        "60".into(),
        "--format".into(),
        "json".into(),
    ];
    if let Some(path) = resume {
        argv.extend(["--resume".into(), path.display().to_string()]);
    }
    if replacement {
        argv.extend(["--replacement-file".into(), "src/lib.rs".into()]);
    }
    let refs: Vec<_> = argv.iter().map(String::as_str).collect();
    let summary = report(&cli(root, &refs), code);
    let index = Path::new(summary["evidence"].as_str().unwrap()).join("index.json");
    let ledger = serde_json::from_slice(&std::fs::read(&index).unwrap()).unwrap();
    (ledger, index)
}

#[test]
fn consuming_agent_receives_the_current_prompt_on_start_and_repair_before_immutable_delivery() {
    let root = setup("immutable_delivery");
    let output = tempfile::tempdir().unwrap();
    let (ledger, _) = run(root.path(), output.path(), "two-step", "3", None, false, 0);
    assert_eq!(ledger["complete"], true, "{ledger}");
    assert!(qualitygate::domain::evolution::valid_digest(
        ledger["harness_source_digest"].as_str().unwrap()
    ));
    assert_eq!(ledger["qualitygate_identity"]["retained"], false);
    assert_eq!(ledger["attempts"].as_array().unwrap().len(), 2);
    let first = &ledger["attempts"][0]["injection"];
    let second = &ledger["attempts"][1]["injection"];
    assert_eq!(first["event"], "start");
    assert_eq!(second["event"], "repair");
    assert_eq!(first["contract_digest"], second["contract_digest"]);
    assert_ne!(first["prompt_digest"], second["prompt_digest"]);
    assert_eq!(ledger["attempts"][1]["input_channel"], "stdin_provided");
    assert_eq!(
        ledger["delivery_decision"]["obligations"][0]["status"],
        "host_control_satisfied"
    );
    let received = std::fs::read_to_string(output.path().join("agent-state.prompts")).unwrap();
    assert_eq!(received.matches("=== next launch ===").count(), 2);
    assert!(received.contains(first["contract_digest"].as_str().unwrap()));
    let path = Path::new(ledger["delivery"]["artifact"]["path"].as_str().unwrap());
    assert_eq!(
        qualitygate::snapshot::delivery::identify(path)
            .unwrap()
            .digest,
        ledger["delivery"]["artifact"]["digest"]
    );
}

#[test]
fn forged_completion_and_weakened_policy_never_authorize_delivery() {
    for (mode, termination) in [
        ("forge", "attempt_budget"),
        ("weaken-policy", "incomplete_check"),
    ] {
        let root = setup("final_full_check");
        let output = tempfile::tempdir().unwrap();
        let (ledger, _) = run(root.path(), output.path(), mode, "1", None, false, 2);
        assert_eq!(ledger["termination"], termination, "{ledger}");
        assert_eq!(ledger["complete"], false);
        assert!(ledger["delivery"].is_null());
        assert_ne!(ledger["attempts"][0]["recheck"]["gate"]["decision"], "pass");
    }
}

#[test]
fn unavailable_required_controls_stop_before_launching_the_agent() {
    for control in [
        "tool_authorization",
        "final_response",
        "in_process_context_recovery",
    ] {
        let root = setup(control);
        let output = tempfile::tempdir().unwrap();
        let (ledger, _) = run(root.path(), output.path(), "repair", "1", None, false, 2);
        assert_eq!(ledger["termination"], "contract_incomplete", "{ledger}");
        assert_eq!(ledger["attempts"], json!([]));
        assert!(!output.path().join("agent-state.prompts").exists());
    }
}

#[test]
fn resume_and_agent_handoff_reinject_current_obligations_and_never_reuse_completion() {
    let root = setup("final_full_check");
    let output = tempfile::tempdir().unwrap();
    let (first, index) = run(root.path(), output.path(), "repair", "1", None, false, 0);
    std::fs::write(root.path().join("hello.txt"), "new failure\r\n").unwrap();
    let (resumed, second) = run(
        root.path(),
        output.path(),
        "repair",
        "1",
        Some(&index),
        false,
        0,
    );
    assert_eq!(resumed["initial"]["gate"]["decision"], "fail");
    assert_eq!(resumed["attempts"][0]["injection"]["event"], "resume");
    assert_eq!(resumed["resume"]["evidence_reused"], false);
    assert_ne!(
        resumed["attempts"][0]["injection"]["session"],
        first["attempts"][0]["injection"]["session"]
    );
    std::fs::write(root.path().join("hello.txt"), "another failure\r\n").unwrap();
    let (handoff, _) = run(
        root.path(),
        output.path(),
        "repair2",
        "1",
        Some(&second),
        false,
        0,
    );
    assert_eq!(handoff["attempts"][0]["injection"]["event"], "handoff");
    assert_ne!(
        handoff["delivery"]["artifact"]["digest"],
        resumed["delivery"]["artifact"]["digest"]
    );
}

#[test]
fn generic_json_replacement_uses_the_same_current_contract_and_delivery_checks() {
    let root = setup("policy_integrity");
    let output = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("hello.txt"), "initial\n").unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/lib.rs"), "pub fn bad() {}\r\n").unwrap();
    let (ledger, _) = run(root.path(), output.path(), "replace", "1", None, true, 0);
    assert_eq!(ledger["complete"], true, "{ledger}");
    assert_eq!(ledger["attempts"][0]["replacement"]["path"], "src/lib.rs");
    assert_eq!(
        std::fs::read_to_string(root.path().join("src/lib.rs")).unwrap(),
        "pub fn repaired() {}\n"
    );
}
