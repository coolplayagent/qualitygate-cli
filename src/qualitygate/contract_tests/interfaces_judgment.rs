use crate::domain::{
    CheckResult, Diagnostic, Report, Severity, Summary, evaluate,
    judgment::{JudgmentMode, JudgmentPolicy, Question},
};
use crate::{application::judgment::Options, snapshot};
use serde_json::{Value, json};
use std::path::Path;
fn fixture(root: &Path, response: &str) -> (Options, Report) {
    let mut check = CheckResult::pending("review", false, Severity::Warning);
    check.diagnostics.push(Diagnostic {
        id: "review-1".into(),
        fingerprint: "fp-1".into(),
        file: Some("src/lib.rs".into()),
        range: None,
        message: "Review this behavior".into(),
        evidence: json!({"language":"rust"}),
        fix: "Inspect the source".into(),
        recheck: Default::default(),
    });
    check.complete();
    let report: Report = serde_json::from_value(json!({"schema_version":1,"run_id":"run-1","scope":"delivery","profile":"full",
        "snapshot":{"mode":"worktree","base":"base","head":"head","content_digest":snapshot::digest(b"snapshot")},
        "policy":json!({"source":"unit","config_digest":snapshot::digest(b"policy"),"rules_digest":snapshot::digest(b"rules"),"trust":"local_candidate","changes":[]}), "plan":{"required_checks":[],"pending_delivery_checks":[],"acceptance":{}},
        "gate":evaluate(&[check.clone()], &[], &[]),"checks":[check],"summary":Summary::default()})).unwrap();
    let bytes = serde_json::to_vec(&report).unwrap();
    std::fs::write(root.join("report.json"), &bytes).unwrap();
    let evidence = snapshot::digest(
        &serde_json::to_vec(&(
            snapshot::digest(&bytes),
            "review",
            "fp-1",
            json!({"language":"rust"}),
        ))
        .unwrap(),
    );
    let response = response.replace("EVIDENCE", &evidence);
    std::fs::write(root.join("response-input.json"), response).unwrap();
    let mut question = Question {
        id: "review".into(),
        version: 1,
        text: "Is the finding valid?".into(),
        criteria: vec!["Independent review".into()],
        choices: vec!["true".into(), "false".into()],
        positive_choice: "true".into(),
        target_event: "finding_valid".into(),
        source_digest: String::new(),
    };
    question.source_digest = question.digest().unwrap();
    let policy = JudgmentPolicy {
        schema_version: 1,
        mode: JudgmentMode::Shadow,
        argv: vec![
            "sh".into(),
            "-c".into(),
            "cat >/dev/null; cat response-input.json".into(),
        ],
        version_argv: vec![
            "sh".into(),
            "-c".into(),
            "printf fixture-provider-v1".into(),
        ],
        provider_inputs: vec!["response-input.json".into()],
        timeout_seconds: 1,
        question,
        calibration: None,
        review_priority_threshold: None,
    };
    std::fs::write(
        root.join("policy.json"),
        serde_json::to_vec(&policy).unwrap(),
    )
    .unwrap();
    (
        Options {
            root: root.into(),
            report: root.join("report.json"),
            policy: root.join("policy.json"),
            output_dir: root.join("decisions"),
            fingerprint: Some("fp-1".into()),
        },
        report,
    )
}

#[cfg(unix)]
#[tokio::test]
async fn judgment_dispatch_resolves_relative_inputs_and_preserves_advisory_authority_boundary() {
    use clap::Parser;
    let root = tempfile::tempdir().unwrap();
    let (_, report) = fixture(
        root.path(),
        r#"{"kind":"deterministic","question_id":"review","value":false,"evidence_refs":["EVIDENCE"]}"#,
    );
    let cli = crate::interfaces::cli::Cli::try_parse_from([
        "qualitygate",
        "--root",
        root.path().to_str().unwrap(),
        "--format",
        "json",
        "judgment",
        "run",
        "--report",
        "report.json",
        "--policy",
        "policy.json",
        "--output-dir",
        "decisions",
        "--fingerprint",
        "fp-1",
    ])
    .unwrap();
    let (text, code) = cli.run().await.unwrap();
    let result: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(code, 0);
    assert_eq!(
        result["original_gate"],
        serde_json::to_value(report.gate).unwrap()
    );
    let policy: JudgmentPolicy =
        serde_json::from_slice(&std::fs::read(root.path().join("policy.json")).unwrap()).unwrap();
    let path = root.path().join("question.yaml");
    std::fs::write(&path, serde_norway::to_string(&policy.question).unwrap()).unwrap();
    let cli = crate::interfaces::cli::Cli::try_parse_from([
        "qualitygate",
        "--root",
        root.path().to_str().unwrap(),
        "--format",
        "json",
        "judgment",
        "question-digest",
        "--input",
        path.to_str().unwrap(),
    ])
    .unwrap();
    let (text, code) = cli.run().await.unwrap();
    assert_eq!(code, 0);
    assert!(text.contains(&policy.question.source_digest));
}
