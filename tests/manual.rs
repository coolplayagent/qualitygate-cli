mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    root: tempfile::TempDir,
    external: tempfile::TempDir,
    key: SigningKey,
}

impl Fixture {
    fn new() -> Self {
        let root = fixture();
        let external = tempfile::tempdir().unwrap();
        let key = SigningKey::from_bytes(&[19; 32]);
        std::fs::write(
            root.path().join("qualitygate.yaml"),
            "schema_version: 1\nprofiles:\n  quick: {include: []}\n",
        )
        .unwrap();
        std::fs::write(root.path().join("task.yaml"), "schema_version: 1\ntask_id: feature\nacceptance:\n  - id: behavior\n    description: Reviewer observed the expected behavior\n    verification:\n      kind: manual\n      check_id: review\n      evidence_file: review.json\n  - id: subsequent-command\n    description: Execute the command after approval\n    verification:\n      check_id: after-review\n      argv: [git, --version]\n      depends_on: [review]\n").unwrap();
        let value = Self {
            root,
            external,
            key,
        };
        value.trust(json!([]));
        value
    }

    fn trust(&self, revoked: Value) {
        std::fs::write(self.external.path().join("trust.json"), serde_json::to_vec(&json!({
            "schema_version":1,"repository":"fixture/project","max_age_seconds":7200,
            "keys":[{"id":"reviewer","public_key":STANDARD.encode(self.key.verifying_key().as_bytes()),"checks":["review"],"tasks":["feature"]}],
            "revoked_records":revoked
        })).unwrap()).unwrap();
    }

    fn run(&self, code: i32) -> Value {
        report(
            &cli(
                self.root.path(),
                &[
                    "check",
                    "--task",
                    "task.yaml",
                    "--trust-store",
                    self.external.path().join("trust.json").to_str().unwrap(),
                    "--evidence-dir",
                    self.external.path().to_str().unwrap(),
                    "--format",
                    "json",
                ],
            ),
            code,
        )
    }

    fn manual<'a>(&self, report: &'a Value) -> &'a Value {
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["id"] == "review")
            .unwrap_or_else(|| panic!("Missing manual check: {report}"))
    }

    fn record(&self, report: &Value, decision: &str) -> Value {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        json!({"schema_version":1,"record_id":"approval-1",
            "subject":self.manual(report)["metadata"]["manual_acceptance"]["expected_subject"],
            "reviewer":"human@example.test","decision":decision,"reason":"Observed the required behavior",
            "issued_at":now-1,"expires_at":now+600})
    }

    fn publish(&self, record: &Value) {
        let bytes = serde_json::to_vec(record).unwrap();
        let kind = qualitygate::adapters::attestation::PAYLOAD_TYPE;
        let signature = self
            .key
            .sign(&qualitygate::adapters::attestation::pae(kind, &bytes));
        std::fs::write(
            self.external.path().join("review.json"),
            serde_json::to_vec(&json!({
                "payloadType":kind,"payload":STANDARD.encode(bytes),
                "signatures":[{"keyid":"reviewer","sig":STANDARD.encode(signature.to_bytes())}]
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn manual_acceptance_requires_authentication_then_runs_dependent_checks() {
    let fixture = Fixture::new();
    let absent = report(
        &cli(
            fixture.root.path(),
            &["check", "--task", "task.yaml", "--format", "json"],
        ),
        2,
    );
    assert_eq!(fixture.manual(&absent)["verdict"], Value::Null);
    let missing = fixture.run(2);
    let approved = fixture.record(&missing, "approved");
    assert_eq!(approved["subject"]["task"]["acceptance_id"], "behavior");
    fixture.publish(&approved);
    let accepted = fixture.run(0);
    assert_eq!(accepted["scope"], "task");
    assert_eq!(accepted["summary"]["pass"], 2);
    let manual = fixture.manual(&accepted);
    assert_eq!(manual["metadata"]["manual_acceptance"]["verified"], true);
    assert_eq!(manual["execution"]["argv"], json!([]));
    assert_eq!(manual["execution"]["exit_code"], Value::Null);
    assert_eq!(
        manual["execution"]["artifacts"].as_array().unwrap().len(),
        2
    );
    for artifact in manual["execution"]["artifacts"].as_array().unwrap() {
        let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
        assert_eq!(artifact["digest"], qualitygate::snapshot::digest(&bytes));
    }
    fixture.publish(&fixture.record(&accepted, "rejected"));
    let rejected = fixture.run(2); // Rejection blocks the required dependent command.
    assert_eq!(fixture.manual(&rejected)["verdict"], "fail");
    let recheck = fixture.manual(&rejected)["diagnostics"][0]["recheck"]["argv"]
        .as_array()
        .unwrap();
    assert!(recheck.contains(&json!("--trust-store")));
    assert!(recheck.contains(&json!("--evidence-dir")));
    let expected = fixture.record(&rejected, "approved");
    fixture.publish(&expected);
    std::fs::write(fixture.root.path().join("hello.txt"), "new code\n").unwrap();
    let changed = fixture.run(2);
    assert!(
        fixture.manual(&changed)["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("does not match")
    );
    fixture.publish(&fixture.record(&changed, "approved"));
    fixture.run(0);
}

#[test]
fn tampering_expiry_revocation_and_foreign_subjects_remain_incomplete() {
    let fixture = Fixture::new();
    let initial = fixture.run(2);
    let original = fixture.record(&initial, "approved");
    for kind in [
        "signature",
        "unsigned",
        "expired",
        "future",
        "foreign-check",
        "foreign-task",
        "foreign-policy",
        "revoked",
    ] {
        fixture.trust(json!([]));
        let mut value = original.clone();
        match kind {
            "expired" => value["expires_at"] = json!(1),
            "future" => value["issued_at"] = json!(u64::MAX),
            "foreign-check" => value["subject"]["check_id"] = json!("other"),
            "foreign-task" => value["subject"]["task"]["task_id"] = json!("other"),
            "foreign-policy" => value["subject"]["policy"]["config_digest"] = json!("sha256:other"),
            "revoked" => fixture.trust(json!(["approval-1"])),
            _ => {}
        }
        fixture.publish(&value);
        let path = fixture.external.path().join("review.json");
        if kind == "signature" {
            let mut envelope: Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            envelope["payload"] = json!(STANDARD.encode(b"{}"));
            std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
        } else if kind == "unsigned" {
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        }
        let result = fixture.run(2);
        assert_eq!(
            fixture.manual(&result)["execution"]["status"],
            "blocked",
            "{kind}"
        );
        assert_eq!(fixture.manual(&result)["verdict"], Value::Null, "{kind}");
    }
}

#[test]
fn caller_trust_cannot_be_loaded_from_the_checked_repository() {
    let fixture = Fixture::new();
    let store = fixture.external.path().join("trust.json");
    std::fs::copy(&store, fixture.root.path().join("trust.json")).unwrap();
    let invalid = report(
        &cli(
            fixture.root.path(),
            &[
                "check",
                "--task",
                "task.yaml",
                "--trust-store",
                fixture.root.path().join("trust.json").to_str().unwrap(),
                "--evidence-dir",
                fixture.external.path().to_str().unwrap(),
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(invalid["gate"]["decision"], "incomplete");
    let config = qualitygate::config::parse(
        b"schema_version: 1\nchecks: [{id: review, kind: manual, evidence_file: ../escape}]\n",
    );
    assert!(config.is_err());
    for extra in [
        "argv: [git]",
        "cwd: elsewhere",
        "expected_exit_code: 1",
        "required_args: [test]",
        "evidence_file: ./review.json",
    ] {
        assert!(
            qualitygate::config::parse(
                format!("schema_version: 1\nchecks: [{{id: review, kind: manual, {extra}}}]\n")
                    .as_bytes()
            )
            .is_err()
        );
    }
    assert!(qualitygate::config::parse(b"schema_version: 1\nchecks: [{id: command, argv: [git], evidence_file: record.json}]\n").is_err());
}

#[test]
fn repository_review_requires_explicit_authorization_and_rejection_returns_one() {
    let fixture = Fixture::new();
    std::fs::write(fixture.root.path().join("qualitygate.yaml"), "schema_version: 1\nchecks: [{id: review, kind: manual, evidence_file: review.json}]\nprofiles: {quick: {include: []}}\n").unwrap();
    let run = |profile, code| {
        report(
            &cli(
                fixture.root.path(),
                &[
                    "check",
                    "--profile",
                    profile,
                    "--trust-store",
                    fixture.external.path().join("trust.json").to_str().unwrap(),
                    "--evidence-dir",
                    fixture.external.path().to_str().unwrap(),
                    "--format",
                    "json",
                ],
            ),
            code,
        )
    };
    let missing = run("full", 2);
    let rejected = fixture.record(&missing, "rejected");
    assert_eq!(rejected["subject"]["task"], Value::Null);
    fixture.publish(&rejected);
    run("full", 2); // Task authorization does not authorize repository acceptance.
    let path = fixture.external.path().join("trust.json");
    let mut store: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    store["keys"][0]["allow_repository_checks"] = json!(true);
    std::fs::write(path, serde_json::to_vec(&store).unwrap()).unwrap();
    let failed = run("full", 1);
    assert_eq!(fixture.manual(&failed)["execution"]["status"], "completed");
    assert_eq!(fixture.manual(&failed)["verdict"], "fail");
    fixture.publish(&fixture.record(&failed, "approved"));
    run("full", 0);
    let quick = run("quick", 2);
    assert_eq!(quick["plan"]["pending_delivery_checks"], json!(["review"]));
    assert_eq!(quick["summary"]["checks_total"], 0);
}

#[test]
fn external_trust_changes_during_commands_invalidate_previously_approved_evidence() {
    let fixture = Fixture::new();
    let source = fixture.external.path().join("mutator.rs");
    let executable = fixture.external.path().join(if cfg!(windows) {
        "manual-mutator.exe"
    } else {
        "manual-mutator"
    });
    std::fs::write(&source, "fn main() { let path = std::env::args().nth(1).unwrap(); let mut bytes = std::fs::read(&path).unwrap(); bytes.push(b' '); std::fs::write(path, bytes).unwrap(); }\n").unwrap();
    let compiled = std::process::Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let task_path = fixture.root.path().join("task.yaml");
    let mut task: Value = serde_norway::from_slice(&std::fs::read(&task_path).unwrap()).unwrap();
    task["acceptance"][1]["verification"]["argv"] =
        json!([executable, fixture.external.path().join("trust.json")]);
    std::fs::write(task_path, serde_norway::to_string(&task).unwrap()).unwrap();
    let missing = fixture.run(2);
    fixture.publish(&fixture.record(&missing, "approved"));
    let changed = fixture.run(2);
    let review = fixture.manual(&changed);
    assert_eq!(review["metadata"]["manual_acceptance"]["verified"], true);
    assert_eq!(
        review["metadata"]["manual_acceptance"]["valid_at_completion"],
        false
    );
    assert_eq!(review["verdict"], Value::Null);
    assert!(
        review["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("changed")
    );
    assert_eq!(
        review["execution"]["artifacts"].as_array().unwrap().len(),
        2
    );
}

#[test]
fn approval_expiring_during_later_execution_cannot_complete_the_gate() {
    let fixture = Fixture::new();
    let source = fixture.external.path().join("deadline.rs");
    let executable = fixture.external.path().join(if cfg!(windows) {
        "manual-deadline.exe"
    } else {
        "manual-deadline"
    });
    std::fs::write(&source, "fn main() { let deadline: u64 = std::env::args().nth(1).unwrap().parse().unwrap(); while std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() < deadline { std::thread::sleep(std::time::Duration::from_millis(25)); } }\n").unwrap();
    let compiled = std::process::Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let deadline = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 10;
    let task_path = fixture.root.path().join("task.yaml");
    let mut task: Value = serde_norway::from_slice(&std::fs::read(&task_path).unwrap()).unwrap();
    task["acceptance"][1]["verification"]["argv"] =
        json!([executable.to_str().unwrap(), deadline.to_string()]);
    std::fs::write(task_path, serde_norway::to_string(&task).unwrap()).unwrap();
    let missing = fixture.run(2);
    let mut approved = fixture.record(&missing, "approved");
    approved["expires_at"] = json!(deadline);
    fixture.publish(&approved);
    let completed = fixture.run(2);
    let review = fixture.manual(&completed);
    assert_eq!(review["metadata"]["manual_acceptance"]["verified"], true);
    assert_eq!(
        review["metadata"]["manual_acceptance"]["valid_at_completion"],
        false
    );
    assert_eq!(review["verdict"], Value::Null);
    assert!(
        review["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("expired")
    );
}
