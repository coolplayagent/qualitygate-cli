mod common;
use common::*;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

const BUG: &str = include_str!("../templates/pilot/rust-v1/bug-fix.yaml");
const REFACTOR: &str = include_str!("../templates/pilot/rust-v1/refactor.yaml");
const BROKEN: &str = "pub fn bounded(value: u32, maximum: u32) -> u32 { let _ = maximum; value }\n";
const FIXED: &str = "pub fn bounded(value: u32, maximum: u32) -> u32 { value.min(maximum) }\n";
const REFACTORED: &str = "pub fn bounded(value: u32, maximum: u32) -> u32 { if value > maximum { maximum } else { value } }\n";
const TESTS: &str = "#[test] fn below_limit() { assert_eq!(pilot_fixture::bounded(4, 5), 4); }\n#[test] fn above_limit() { assert_eq!(pilot_fixture::bounded(8, 5), 5); }\n";

struct Baseline {
    repository: tempfile::TempDir,
    policy: String,
}

impl Baseline {
    fn new() -> Self {
        let repository = fixture();
        let root = repository.path();
        for directory in ["src", "tests", "tasks"] {
            std::fs::create_dir(root.join(directory)).unwrap();
        }
        for (path, text) in [
            (".gitignore", "target/\n"),
            ("qualitygate.yaml", "schema_version: 1\n"),
            (
                "Cargo.toml",
                "[package]\nname='pilot_fixture'\nversion='0.1.0'\nedition='2021'\n",
            ),
            ("src/lib.rs", BROKEN),
            ("tests/regression.rs", TESTS),
            ("tasks/bug-fix.yaml", BUG),
            ("tasks/refactor.yaml", REFACTOR),
        ] {
            std::fs::write(root.join(path), text).unwrap();
        }
        let lock = Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            lock.status.success(),
            "{}",
            String::from_utf8_lossy(&lock.stderr)
        );
        git(root, &["add", "."]);
        git(
            root,
            &["commit", "-qm", "prepare reviewed regression contracts"],
        );
        Self {
            policy: head(root),
            repository,
        }
    }

    fn commit_source(&self, source: &str) -> String {
        std::fs::write(self.repository.path().join("src/lib.rs"), source).unwrap();
        git(self.repository.path(), &["add", "src/lib.rs"]);
        git(
            self.repository.path(),
            &["commit", "-qm", "change implementation"],
        );
        head(self.repository.path())
    }

    fn check(&self, base: &str, head: &str, task: &str, code: i32) -> Value {
        report(
            &cli(
                self.repository.path(),
                &[
                    "check",
                    "--diff",
                    &format!("{base}..{head}"),
                    "--policy-ref",
                    &self.policy,
                    "--task",
                    task,
                    "--profile",
                    "full",
                    "--format",
                    "json",
                ],
            ),
            code,
        )
    }
}

fn head(root: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn execution(report: &Value) -> &Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "rust-regression")
        .unwrap()
}

// Retain test evidence with its original reports and a relocation map. These
// are controlled fixture records, never real-pilot or autonomous-agent claims.
fn retain(reports: &[(&str, &Value)]) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/pilot-phase-a");
    std::fs::create_dir_all(&directory).unwrap();
    let evidence = tempfile::Builder::new()
        .prefix("fixture-")
        .tempdir_in(directory)
        .unwrap();
    let mut records = Vec::new();
    for (name, report) in reports {
        let bytes = serde_json::to_vec_pretty(report).unwrap();
        let filename = format!("{name}.json");
        std::fs::write(evidence.path().join(&filename), &bytes).unwrap();
        let mut artifacts = Vec::new();
        let check = execution(report);
        let probe_artifacts = check["metadata"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|tool| [&tool["stdout"], &tool["stderr"]]);
        for (index, artifact) in check["execution"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .chain(probe_artifacts)
            .enumerate()
        {
            let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
            assert_eq!(bytes.len() as u64, artifact["bytes"].as_u64().unwrap());
            assert_eq!(qualitygate::snapshot::digest(&bytes), artifact["digest"]);
            let stored = format!("{name}-artifact-{index}");
            std::fs::write(evidence.path().join(&stored), &bytes).unwrap();
            artifacts.push(json!({"original":artifact,"stored_path":stored}));
        }
        assert!(!artifacts.is_empty());
        records.push(json!({"case":name,"report_path":filename,
            "report_digest":qualitygate::snapshot::digest(&bytes),"artifacts":artifacts}));
    }
    std::fs::write(
        evidence.path().join("index.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":1,"origin":"controlled_fixture","pilot_acceptance":"pending",
            "known_limits":["No real agent execution, team confirmation or measured pilot benefit"],
            "records":records
        }))
        .unwrap(),
    )
    .unwrap();
    println!(
        "Controlled phase-A baseline evidence: {}",
        evidence.keep().display()
    );
}

#[test]
fn task_templates_capture_real_assertions_and_allow_behavior_preserving_refactors() {
    let fixture = Baseline::new();
    let old = fixture.check(&fixture.policy, &fixture.policy, "tasks/bug-fix.yaml", 1);
    assert_eq!(old["gate"]["complete"], true);
    assert_eq!(execution(&old)["execution"]["status"], "completed");
    assert_eq!(execution(&old)["execution"]["exit_code"], 101);
    assert_eq!(execution(&old)["metadata"]["tests"]["executed"], 2);
    let stdout = execution(&old)["execution"]["artifacts"][0]["path"]
        .as_str()
        .unwrap();
    let stdout = std::fs::read_to_string(stdout).unwrap();
    assert!(stdout.contains("assertion `left == right` failed"));
    assert!(stdout.contains("1 passed; 1 failed"));
    let fixed = fixture.commit_source(FIXED);
    let repaired = fixture.check(&fixture.policy, &fixed, "tasks/bug-fix.yaml", 0);
    let before = fixture.check(&fixed, &fixed, "tasks/refactor.yaml", 0);
    let refactored = fixture.commit_source(REFACTORED);
    let after = fixture.check(&fixed, &refactored, "tasks/refactor.yaml", 0);
    for current in [&old, &repaired, &before, &after] {
        assert_eq!(current["policy"]["resolved_commit"], fixture.policy);
        assert_eq!(current["profile"], "full");
        assert_eq!(current["scope"], "task");
        assert!(
            current["plan"]["pending_delivery_checks"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            !execution(current)["metadata"]["tools"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(
        old["policy"]["task_contract_digest"],
        repaired["policy"]["task_contract_digest"]
    );
    assert_eq!(
        before["policy"]["task_contract_digest"],
        after["policy"]["task_contract_digest"]
    );
    assert_eq!(after["snapshot"]["base"], fixed);
    assert_eq!(after["snapshot"]["head"], refactored);
    assert_ne!(
        before["snapshot"]["content_digest"],
        after["snapshot"]["content_digest"]
    );
    assert!(execution(&after)["metadata"]["test_effectiveness"].is_null());
    retain(&[
        ("bug-before", &old),
        ("bug-after", &repaired),
        ("refactor-before", &before),
        ("refactor-after", &after),
    ]);
}

#[test]
fn task_preparation_rejects_zero_tests_compile_errors_and_changed_acceptance() {
    let fixture = Baseline::new();
    let root = fixture.repository.path();
    std::fs::write(root.join("tests/regression.rs"), "").unwrap();
    let check = |code| {
        report(
            &cli(
                root,
                &[
                    "check",
                    "--worktree",
                    "--policy-ref",
                    &fixture.policy,
                    "--task",
                    "tasks/refactor.yaml",
                    "--profile",
                    "full",
                    "--format",
                    "json",
                ],
            ),
            code,
        )
    };
    let zero = check(1);
    assert_eq!(zero["gate"]["decision"], "fail");
    assert_eq!(execution(&zero)["metadata"]["tests"]["executed"], 0);
    std::fs::write(root.join("tests/regression.rs"), TESTS).unwrap();
    std::fs::write(root.join("src/lib.rs"), "this does not compile").unwrap();
    let compile = check(2);
    assert_eq!(compile["gate"]["complete"], false);
    std::fs::write(root.join("src/lib.rs"), FIXED).unwrap();
    std::fs::write(
        root.join("tasks/refactor.yaml"),
        REFACTOR.replace("required: true", "required: false"),
    )
    .unwrap();
    let changed = check(2);
    assert!(!changed["policy"]["changes"].as_array().unwrap().is_empty());
    assert_ne!(execution(&changed)["execution"]["status"], "completed");
}

#[test]
fn separately_passing_branches_require_a_new_full_check_after_merge() {
    let fixture = Baseline::new();
    let root = fixture.repository.path();
    for (path, source) in [
        (
            "src/lib.rs",
            "pub mod left; pub mod right; pub fn total() -> u32 { left::LOAD + right::LOAD }\n",
        ),
        ("src/left.rs", "pub const LOAD: u32 = 3;\n"),
        ("src/right.rs", "pub const LOAD: u32 = 3;\n"),
        (
            "tests/regression.rs",
            "#[test] fn combined_capacity() { assert!(pilot_fixture::total() <= 10); }\n",
        ),
    ] {
        std::fs::write(root.join(path), source).unwrap();
    }
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "fixed combined capacity contract"]);
    let base = head(root);
    git(root, &["checkout", "-qb", "left-change"]);
    std::fs::write(root.join("src/left.rs"), "pub const LOAD: u32 = 7;\n").unwrap();
    git(root, &["commit", "-qam", "left branch"]);
    let check = |code| {
        report(
            &cli(
                root,
                &[
                    "check",
                    "--diff",
                    &format!("{base}..HEAD"),
                    "--policy-ref",
                    &base,
                    "--task",
                    "tasks/refactor.yaml",
                    "--profile",
                    "full",
                    "--feedback",
                ],
            ),
            code,
        )
    };
    let left = check(0);
    git(root, &["checkout", "-qb", "right-change", &base]);
    std::fs::write(root.join("src/right.rs"), "pub const LOAD: u32 = 7;\n").unwrap();
    git(root, &["commit", "-qam", "right branch"]);
    let right = check(0);
    git(root, &["merge", "--no-edit", "left-change"]);
    let merged = check(1);
    for old in [&left, &right] {
        assert_eq!(old["delivery_ready"], true);
        assert_ne!(old["snapshot"]["head"], merged["snapshot"]["head"]);
        assert_ne!(
            old["snapshot"]["content_digest"],
            merged["snapshot"]["content_digest"]
        );
        assert_eq!(old["policy"], merged["policy"]);
    }
    assert_eq!(merged["gate"]["complete"], true);
    assert_eq!(merged["delivery_ready"], false);
    let full: Value = serde_json::from_slice(
        &std::fs::read(merged["full_report"]["path"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(execution(&full)["execution"]["exit_code"], 101);
    assert_eq!(execution(&full)["metadata"]["tests"]["executed"], 1);
    let stdout = std::fs::read_to_string(
        execution(&full)["execution"]["artifacts"][0]["path"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert!(stdout.contains("assertion failed: pilot_fixture::total() <= 10"));
    assert!(stdout.contains("0 passed; 1 failed"));
}
