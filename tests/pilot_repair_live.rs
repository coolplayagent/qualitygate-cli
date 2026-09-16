//! Explicitly invoked, billable Codex repairs of extracted Qualitygate source.
//! Controlled module experiments are not the eight-task business trial.
pub mod common;
#[path = "../examples/agent_loop.rs"]
mod harness;
use clap::Parser;
use common::*;
use qualitygate::snapshot;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

const SOURCE: &str = include_str!("../src/qualitygate/domain/ratchet.rs");
const REGRESSION: &str = r#"
use pilot_fixture::ratchet::{compare, Key};
use std::collections::BTreeMap;
fn key(tool: Option<&str>, rule: &str) -> Key {
    Key { tool: tool.map(str::to_owned), rule: rule.into() }
}
#[test] fn unchanged_counts_do_not_increase() {
    let input = BTreeMap::from([(key(None,"unchanged"), 2), (key(Some("tool"),"zero"), 0)]);
    assert!(compare(&input, &input).iter().all(|row| !row.increased()));
}
#[test] fn independent_buckets_cannot_offset_growth() {
    let before = BTreeMap::from([(key(None,"a"), 10), (key(None,"b"), 2)]);
    let after = BTreeMap::from([(key(None,"a"), 1), (key(Some("other"),"a"), 1)]);
    let result = compare(&before, &after);
    assert_eq!(result.len(), 3);
    assert_eq!(result.iter().filter(|row| row.increased()).map(|row| row.key.clone()).collect::<Vec<_>>(), vec![key(Some("other"),"a")]);
    assert_eq!((result[1].baseline,result[1].current), (2,0));
}
#[test] fn union_order_defaults_and_multiplicity_are_preserved() {
    let before = BTreeMap::from([(key(None,"z"), 3)]);
    let after = BTreeMap::from([(key(None,"a"), 4), (key(None,"z"), 4)]);
    let result = compare(&before,&after);
    assert_eq!(result.iter().map(|r| (&*r.key.rule,r.baseline,r.current,r.increased())).collect::<Vec<_>>(), vec![("a",0,4,true),("z",3,4,true)]);
    assert!(compare(&BTreeMap::new(), &BTreeMap::new()).is_empty());
}
"#;

fn write(root: &Path, name: &str, value: &str) {
    std::fs::write(root.join(name), value).unwrap();
}

fn setup(root: &Path, bug: bool) -> String {
    std::fs::create_dir_all(root).unwrap();
    git(root, &["init", "-q"]);
    git(root, &["config", "user.name", "Controlled repair fixture"]);
    git(root, &["config", "user.email", "fixture@example.invalid"]);
    git(root, &["config", "core.autocrlf", "false"]);
    for directory in ["src", "tests", "tasks"] {
        std::fs::create_dir(root.join(directory)).unwrap();
    }
    write(root, ".gitignore", "target/\n");
    write(
        root,
        "Cargo.toml",
        "[package]\nname='pilot_fixture'\nversion='0.1.0'\nedition='2021'\n[dependencies]\nserde={version='1', features=['derive']}\n",
    );
    write(root, "src/lib.rs", "pub mod ratchet;\n");
    write(
        root,
        "src/ratchet.rs",
        &if bug {
            SOURCE.replace(
                "self.current > self.baseline",
                "self.current >= self.baseline",
            )
        } else {
            SOURCE.into()
        },
    );
    write(root, "tests/regression.rs", REGRESSION);
    write(
        root,
        "qualitygate.yaml",
        "schema_version: 1\nverification_assets: [Cargo.toml, Cargo.lock, tests/**, tasks/**]\n",
    );
    write(
        root,
        "tasks/task.yaml",
        if bug {
            include_str!("../templates/pilot/rust-v1/bug-fix.yaml")
        } else {
            include_str!("../templates/pilot/rust-v1/refactor.yaml")
        },
    );
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
        &[
            "commit",
            "-qm",
            "fixed source, regression inventory and selected contract",
        ],
    );
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    String::from_utf8(head.stdout).unwrap().trim().into()
}

#[tokio::test]
#[ignore = "billable live Codex repairs; run explicitly after reviewing the fixed experiment"]
async fn codex_models_repair_and_refactor_real_module_with_full_external_rechecks() {
    let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/pilot-phase-b");
    std::fs::create_dir_all(&parent).unwrap();
    let evidence = tempfile::Builder::new()
        .prefix("repairs-")
        .tempdir_in(parent)
        .unwrap()
        .keep();
    let evidence = evidence.canonicalize().unwrap();
    let version = Command::new("codex").arg("--version").output().unwrap();
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).unwrap();
    let mut records = Vec::new();
    for model in ["gpt-5.6-terra", "gpt-5.6-luna"] {
        for bug in [true, false] {
            let kind = if bug { "bug-fix" } else { "refactor" };
            let case = format!("{model}-{kind}");
            let root = evidence.join(&case);
            let head = setup(&root, bug);
            let prompt = if bug {
                "Fix src/ratchet.rs: unchanged diagnostic counts currently fail the debt ratchet. Equal or decreased counts must never be growth; independent tool/rule buckets must not offset each other. Preserve public APIs, ordering and multiplicity. Modify production source only."
            } else {
                "Refactor compare in src/ratchet.rs to accumulate the union of measurements in a BTreeMap<Key, Measurement>, replacing the temporary BTreeSet of references. Preserve public APIs, deterministic order, independent tool/rule buckets, missing counts as zero and multiplicity. This is behavior-preserving: baseline tests already pass. Modify production source only."
            };
            let prompt_path = evidence.join(format!("{case}-task.txt"));
            std::fs::write(&prompt_path, prompt).unwrap();
            let schema_path = evidence.join("replacement-schema.json");
            std::fs::write(&schema_path, json!({"type":"object","properties":{"path":{"type":"string","const":"src/ratchet.rs"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false}).to_string()).unwrap();
            let command_path = evidence.join(format!("{case}-agent.json"));
            let argv = json!([
                "codex",
                "exec",
                "--enable",
                "use_legacy_landlock",
                "--ignore-user-config",
                "--ignore-rules",
                "--ephemeral",
                "--sandbox",
                "read-only",
                "-c",
                "approval_policy=\"never\"",
                "-c",
                "model_reasoning_effort=\"medium\"",
                "--model",
                model,
                "--color",
                "never",
                "--json",
                "--output-schema",
                schema_path,
                "-"
            ]);
            std::fs::write(&command_path, argv.to_string()).unwrap();
            let options = harness::Options::try_parse_from([
                "agent_loop",
                "--root",
                root.to_str().unwrap(),
                "--qualitygate",
                env!("CARGO_BIN_EXE_qualitygate"),
                "--replacement-file",
                "src/ratchet.rs",
                "--base",
                &head,
                "--policy-ref",
                &head,
                "--task",
                "tasks/task.yaml",
                "--prompt-file",
                prompt_path.to_str().unwrap(),
                "--agent-command",
                command_path.to_str().unwrap(),
                "--output-dir",
                evidence.to_str().unwrap(),
            ])
            .unwrap();
            let summary = harness::run(options).await.unwrap();
            let index_path = Path::new(summary["evidence"].as_str().unwrap()).join("index.json");
            let bytes = std::fs::read(&index_path).unwrap();
            let index: Value = serde_json::from_slice(&bytes).unwrap();
            records.push(json!({"case":case,"kind":kind,"requested_model":model,"actual_model":null,
                "reasoning_effort":"medium","agent_version":version.trim(),"summary":summary,"index_path":index_path,
                "index_digest":snapshot::digest(&bytes),"attempts":index["attempts"].as_array().unwrap().len(),
                "initial_gate":index["initial"]["gate"],"independent_tests":3}));
            let manifest = json!({"schema_version":1,"origin":"live_codex_on_extracted_module",
                "source_path":"src/qualitygate/domain/ratchet.rs","source_digest":snapshot::digest(SOURCE.as_bytes()),
                "regression_digest":snapshot::digest(REGRESSION.as_bytes()),"feedback_mode":"compact",
                "business_trial":false,"records":records,"known_limits":[
                    "Controlled fault injection and specified refactor; not eight independent production tasks",
                    "No full-vs-compact causal comparison, human review time, monetary cost or seven-day observation",
                    "Requested models recorded; actual service model identity unavailable"]});
            std::fs::write(
                evidence.join("index.json"),
                serde_json::to_vec_pretty(&manifest).unwrap(),
            )
            .unwrap();
            println!("Retained {case}: {}", summary);
        }
    }
    println!("Live module repair evidence: {}", evidence.display());
    assert!(
        records
            .iter()
            .all(|record| record["summary"]["complete"] == true),
        "Retained failed or incomplete attempts at {}",
        evidence.display()
    );
}
