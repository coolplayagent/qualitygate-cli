use super::*;
use qualitygate::{
    config,
    snapshot::{self, CaptureOptions, Selection},
};

#[test]
fn command_dependencies_need_retention_and_missing_evidence_stays_incomplete() {
    let temp = fixture();
    let root = temp.path();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("driver.rs");
    let binary = external.path().join(if cfg!(windows) {
        "driver.exe"
    } else {
        "driver"
    });
    std::fs::write(
        &source,
        r#"fn main() {
        if std::env::args().any(|arg| arg == "--version") { println!("resource-test 1"); return; }
        if std::fs::read("asset.pdf").is_err() { std::process::exit(9); }
        println!("{{\"issues\":[]}}");
    }"#,
    )
    .unwrap();
    assert!(
        Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    let mut config = json!({"schema_version":1,"checks":[{"id":"analyzer","argv":[binary],"tools":[{"id":"producer","argv":[binary,"--version"]}],"reports":[{"path":"stdout.json","format":"diagnostics","from_stdout":true}]}]});
    write(root, "asset.pdf", "dependency\n");
    write(root, "qualitygate.yaml", config.to_string());
    commit(root);
    let missing = check(root, &[], 2);
    assert_eq!(missing["gate"]["decision"], "incomplete");
    assert_eq!(missing["checks"][0]["execution"]["exit_code"], 9);
    assert_eq!(
        missing["checks"][0]["execution"]["reason"],
        "Report is empty"
    );
    config["unstructured_documents"] = json!({"retain":["asset.pdf"]});
    write(root, "qualitygate.yaml", config.to_string());
    let retained = check(root, &[], 0);
    assert_eq!(
        retained["selection"]["resource_exclusions"]["default_excluded_count"],
        0
    );
    config
        .as_object_mut()
        .unwrap()
        .remove("unstructured_documents");
    config["checks"][0]["tools"][0]["inputs"] = json!(["asset.pdf"]);
    write(root, "qualitygate.yaml", config.to_string());
    check(root, &[], 0);
    doctor(root, &[], 0);
}

fn check(root: &Path, args: &[&str], code: i32) -> serde_json::Value {
    let mut argv = vec!["check", "--profile", "full", "--format", "json"];
    argv.extend_from_slice(args);
    report(&cli(root, &argv), code)
}

fn doctor(root: &Path, args: &[&str], code: i32) -> serde_json::Value {
    let mut argv = vec!["doctor", "--profile", "full", "--format", "json"];
    argv.extend_from_slice(args);
    let value = report(&cli(root, &argv), code);
    config::preflight_schema::validate_doctor(&value).unwrap();
    value
}

#[test]
fn default_resources_are_excluded_before_budgets_in_every_local_selector() {
    let temp = fixture();
    let root = temp.path();
    policy(root, &[]);
    write(root, "legacy/Slides.PPTX", vec![0; 9 * 1024 * 1024]);
    write(root, "legacy/deleted.pdf", vec![0; 9 * 1024 * 1024]);
    commit(root);
    std::fs::remove_file(root.join("legacy/deleted.pdf")).unwrap();
    std::fs::rename(
        root.join("legacy/Slides.PPTX"),
        root.join("legacy/renamed.pptx"),
    )
    .unwrap();
    write(root, "hello.txt", "changed\n");
    git(root, &["add", "."]);
    for args in [
        vec!["--worktree"],
        vec!["--staged"],
        vec!["--path", "hello.txt"],
    ] {
        let result = check(root, &args, 0);
        assert_eq!(result["selection"]["changed_files"], json!(["hello.txt"]));
        let evidence = &result["selection"]["resource_exclusions"];
        assert_eq!(evidence["default_excluded_count"], 3);
        assert_eq!(evidence["explicit_excluded_count"], 0);
        assert_eq!(evidence["classifier_version"], 1);
        assert_eq!(
            result["selection"]["excluded_paths"],
            evidence["default_excluded_paths"]
        );
        if args[0] != "--path" {
            let preflight = doctor(root, &args, 0);
            assert_eq!(
                preflight["budgets"][0]["resource_exclusions"]["default_excluded_count"],
                2
            );
            assert_eq!(
                preflight["budgets"][1]["resource_exclusions"]["default_excluded_count"],
                1
            );
        }
    }
    commit(root);
    let result = check(root, &["--diff", "HEAD~1..HEAD"], 0);
    assert_eq!(
        result["selection"]["resource_exclusions"]["default_excluded_count"],
        3
    );
    doctor(root, &["--diff", "HEAD~1..HEAD"], 0);
    let initialized = report(&cli(root, &["init", "--format", "json"]), 0);
    assert_eq!(
        initialized["snapshot_preflight"]["resource_exclusions"]["default_excluded_count"],
        1
    );

    write(root, "legacy/renamed.pptx", "changed media\r\n");
    let result = check(root, &[], 0);
    assert_eq!(result["selection"]["empty_delivery"], true);
    assert!(
        result["verification"]["known_limits"]
            .to_string()
            .contains("not verified")
    );
}

#[test]
fn aggregate_resources_do_not_consume_content_budget_and_unknown_types_still_fail() {
    let temp = fixture();
    let root = temp.path();
    policy(root, &[]);
    commit(root);
    // Sparse, untracked resources: the check must never read these >1 GiB bytes.
    for name in ["archive.pdf", "recording.mp4"] {
        std::fs::File::create(root.join(name))
            .unwrap()
            .set_len(600 * 1024 * 1024)
            .unwrap();
    }
    let result = check(root, &[], 0);
    assert_eq!(
        result["selection"]["resource_exclusions"]["default_excluded_count"],
        2
    );
    let preflight = doctor(root, &[], 0);
    assert!(preflight["budgets"][1]["total_bytes"].as_u64().unwrap() < 1024);
    for name in ["legacy.bin", "offline.html", "font.woff", "archive.zip"] {
        std::fs::File::create(root.join(name))
            .unwrap()
            .set_len(9 * 1024 * 1024)
            .unwrap();
        let failed = check(root, &["--snapshot-max-file-mib", "8"], 2);
        assert_eq!(failed["gate"]["decision"], "incomplete");
        assert!(failed["gate"]["blockers"].to_string().contains(name));
        std::fs::remove_file(root.join(name)).unwrap();
    }
}

#[test]
fn retain_disable_and_protected_inputs_restore_acquisition_without_bypassing_limits() {
    let temp = fixture();
    let root = temp.path();
    write(root, "asset.pdf", vec![0; 9 * 1024 * 1024]);
    for extra in [
        json!({"unstructured_documents":{"enabled":false}}),
        json!({"unstructured_documents":{"retain":["*.pdf"]}}),
        json!({"verification_assets":["asset.pdf"]}),
    ] {
        let mut policy = json!({"schema_version":1,"rules":{"line-ending":{}}});
        policy
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        write(root, "qualitygate.yaml", policy.to_string());
        check(root, &[], 2);
        let result = doctor(root, &[], 2);
        assert_eq!(result["budgets"][1]["oversized_file_count"], 1);
        assert_eq!(result["budgets"][1]["excluded_file_count"], 0);
        let initialized = report(&cli(root, &["init", "--format", "json"]), 0);
        assert_eq!(initialized["snapshot_preflight"]["oversized_file_count"], 1);
    }
    write(root, "asset.pdf", "small\n");
    write(root, "qualitygate.yaml", json!({"schema_version":1,"rules":{"line-ending":{}},"exclude":["asset.pdf"],"unstructured_documents":{"retain":["asset.pdf"]}}).to_string());
    let result = check(root, &[], 0);
    assert_eq!(
        result["selection"]["resource_exclusions"]["explicit_excluded_paths"],
        json!(["asset.pdf"])
    );
    write(
        root,
        "qualitygate.yaml",
        json!({"schema_version":1,"rules":{"line-ending":{}},"exclude":["asset.pdf"],"verification_assets":["asset.pdf"]})
            .to_string(),
    );
    assert!(
        check(root, &[], 2)
            .to_string()
            .contains("protected verification input")
    );
    doctor(root, &[], 2);
}

#[test]
fn policy_selection_and_strict_configuration_apply_to_default_exclusions() {
    let temp = fixture();
    let root = temp.path();
    policy(root, &[]);
    write(root, "asset.pdf", vec![0; 9 * 1024 * 1024]);
    commit(root);
    write(
        root,
        "qualitygate.yaml",
        "schema_version: 1\nunstructured_documents: {enabled: false}\n",
    );
    check(root, &[], 2);
    check(root, &["--staged"], 0);
    doctor(root, &["--staged"], 0);
    check(root, &["--policy-ref", "HEAD"], 2);
    for invalid in [
        "{enable: false}",
        "{enabled: null}",
        "{retain: ['../asset.pdf']}",
        "{retain: ['!asset.pdf']}",
        "{retain: ['/asset.pdf']}",
        "{retain: false}",
    ] {
        assert!(
            config::parse(
                format!("schema_version: 1\nunstructured_documents: {invalid}\n").as_bytes()
            )
            .is_err(),
            "{invalid}"
        );
    }
    let config = config::parse(b"schema_version: 1\nexclude: []\n").unwrap();
    assert!(config.unstructured_documents.enabled);
}

#[test]
fn bootstrap_protects_configuration_with_a_resource_suffix() {
    let temp = fixture();
    let root = temp.path();
    write(
        root,
        "policy.pdf",
        "schema_version: 1\nrules: {line-ending: {}}\n",
    );
    let result = check(root, &["--config", "policy.pdf"], 0);
    assert_eq!(
        result["selection"]["resource_exclusions"]["default_excluded_count"],
        0
    );
    doctor(root, &["--config", "policy.pdf"], 0);
}

#[cfg(unix)]
#[test]
fn restored_resources_do_not_bypass_confined_path_failures() {
    let temp = fixture();
    let root = temp.path();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("asset.pdf")).unwrap();
    policy(root, &[]);
    check(root, &[], 0);
    write(
        root,
        "qualitygate.yaml",
        "schema_version: 1\nrules: {line-ending: {}}\nunstructured_documents: {retain: ['asset.pdf']}\n",
    );
    assert_eq!(check(root, &[], 2)["gate"]["decision"], "incomplete");
    doctor(root, &[], 2);
}

#[tokio::test]
async fn resource_policy_and_inventory_bind_snapshot_identity_and_history() {
    let temp = fixture();
    let root = temp.path();
    write(root, "asset.pdf", b"resource\n");
    commit(root);
    let selection = Selection::Worktree {
        base: "HEAD".into(),
    };
    let options = CaptureOptions::default();
    let before = snapshot::capture_with_options(root, &selection, &options)
        .await
        .unwrap();
    assert!(!before.files.contains_key("asset.pdf"));
    write(root, "new.png", b"another resource\n");
    let added = snapshot::capture_with_options(root, &selection, &options)
        .await
        .unwrap();
    assert_eq!(
        before.identity.content_digest,
        added.identity.content_digest
    );
    assert_ne!(
        before.identity.verification_digest,
        added.identity.verification_digest
    );
    let mut retained = options.clone();
    retained.unstructured_documents.retain.push("*.pdf".into());
    let restored = snapshot::capture_with_options(root, &selection, &retained)
        .await
        .unwrap();
    assert!(restored.files.contains_key("asset.pdf"));
    assert_ne!(
        before.identity.verification_digest,
        restored.identity.verification_digest
    );
    let history = snapshot::history::capture(&restored).await.unwrap();
    assert!(
        history
            .commits
            .last()
            .unwrap()
            .files
            .contains_key("asset.pdf")
    );
    let history = snapshot::history::capture(&before).await.unwrap();
    assert!(
        !history
            .commits
            .last()
            .unwrap()
            .files
            .contains_key("asset.pdf")
    );
    let mut disabled = options.clone();
    disabled.unstructured_documents.enabled = false;
    let unfiltered = snapshot::capture_with_options(root, &selection, &disabled)
        .await
        .unwrap();
    assert!(unfiltered.files.contains_key("new.png"));
    assert_ne!(
        unfiltered.identity.verification_digest,
        before.identity.verification_digest
    );
    let mut expired = options;
    expired.timeout = std::time::Duration::from_nanos(1);
    assert!(
        snapshot::capture_with_options(root, &selection, &expired)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn resource_inventory_uses_acquired_trees_without_unselected_head_entries() {
    let temp = fixture();
    let root = temp.path();
    write(root, "only-in-intermediate-head.pdf", b"resource\n");
    commit(root);
    git(root, &["rm", "only-in-intermediate-head.pdf"]);
    write(root, "new.png", b"untracked resource\n");
    let captured = snapshot::capture(
        root,
        &Selection::Worktree {
            base: "HEAD~1".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(captured.scope_evidence.excluded_paths, ["new.png"]);
    let staged = snapshot::capture(root, &Selection::Staged).await.unwrap();
    assert_eq!(
        staged.scope_evidence.excluded_paths,
        ["only-in-intermediate-head.pdf"]
    );
}
