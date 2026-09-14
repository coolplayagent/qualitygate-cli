mod common;

use common::{cli, fixture, report};
use qualitygate::config::{self, rule_authoring, rule_schema};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn candidate(root: &Path, id: &str, language: &[&str]) -> Value {
    fs::write(
        root.join("AGENTS.md"),
        "# Policy\n\n## Test naming\nTests must describe behavior.\n\n## Elsewhere\nOther rules.\n",
    )
    .unwrap();
    json!({"id":id,"version":1,"source":rule_authoring::source(root,"AGENTS.md","Test naming").unwrap(),
        "language":language,"requires_capabilities":["test_methods"],"when":{"entity":"test_method"},
        "then":{"name_pattern":"^test_"},"fix":"Describe the behavior in the test name"})
}

fn save(root: &Path, path: &str, value: &Value) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_norway::to_string(value).unwrap()).unwrap();
}

fn list(root: &Path, language: &str, source: &str) -> Value {
    report(
        &cli(
            root,
            &[
                "rules",
                "list",
                "--language",
                language,
                "--source",
                source,
                "--format",
                "json",
            ],
        ),
        0,
    )
}

#[test]
fn language_inventory_works_without_policy_and_keeps_origins_distinct() {
    let repo = fixture();
    let root = repo.path();
    let java = list(root, "java", "builtin");
    let rows = java["rules"].as_array().unwrap();
    assert!(rows.iter().any(|r| r["id"] == "junit-naming"));
    assert!(rows.iter().any(|r| r["id"] == "line-ending"));
    assert!(!rows.iter().any(|r| r["id"] == "pytest-naming"));
    assert!(
        rows.iter()
            .all(|r| r["source"] == "builtin" && r["enabled"] == false)
    );
    assert!(!root.join("qualitygate.yaml").exists());
    assert!(
        list(root, "python", "builtin")["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == "pytest-naming")
    );
    assert!(
        list(root, "rust", "project")["rules"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let rule = candidate(root, "junit-naming", &["rust"]);
    save(root, "qualitygate/rules/nested/test.yaml", &rule);
    let project = list(root, "rust", "project");
    assert_eq!(project["project_rules_directory"], "qualitygate/rules");
    assert_eq!(project["rules"].as_array().unwrap().len(), 1);
    assert_eq!(project["rules"][0]["overrides_builtin"], true);
    assert_eq!(project["rules"][0]["enabled"], false);
    assert!(
        list(root, "java", "project")["rules"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let all = report(&cli(root, &["rules", "list", "--format", "json"]), 0);
    assert_eq!(
        all["rules"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["id"] == "junit-naming")
            .count(),
        2
    );
    fs::write(root.join("qualitygate.yaml"),"schema_version: 1\nrulesets: [core, lang-java]\ncustom_rules: qualitygate/rules\nrules:\n  junit-naming: {}\n").unwrap();
    let all = report(&cli(root, &["rules", "list", "--format", "json"]), 0);
    let selected: Vec<_> = all["rules"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"] == "junit-naming")
        .collect();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected.iter().filter(|r| r["enabled"] == true).count(), 1);
    assert!(
        selected
            .iter()
            .any(|r| r["source"] == "project" && r["enabled"] == true)
    );
    for invalid in ["", "Java", "all", "../java", "1java"] {
        report(
            &cli(
                root,
                &["rules", "list", "--language", invalid, "--format", "json"],
            ),
            2,
        );
    }
    report(
        &cli(
            root,
            &[
                "--config",
                "missing.yaml",
                "rules",
                "list",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(config::rule_query::list(root, "qualitygate.yaml", None, "unknown").is_err());
}

#[test]
fn schema_export_and_generation_use_the_shipped_contract_without_enabling() {
    let repo = fixture();
    let root = repo.path();
    let exported = report(&cli(&root.join("does-not-exist"), &["rules", "schema"]), 0);
    assert_eq!(
        exported,
        serde_json::from_str::<Value>(rule_schema::SCHEMA).unwrap()
    );
    let rule = candidate(root, "rust-test-names", &["rust"]);
    save(root, "candidate.yaml", &rule);
    let source = report(
        &cli(
            root,
            &[
                "rules",
                "source",
                "--document",
                "AGENTS.md",
                "--section",
                "Test naming",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(source["source"], rule["source"]);
    let generated = report(
        &cli(
            root,
            &[
                "rules",
                "generate",
                "--input",
                "candidate.yaml",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(generated["file"], "qualitygate/rules/rust-test-names.yaml");
    assert_eq!(generated["schema_digest"], rule_schema::digest());
    assert_eq!(generated["policy_changed"], false);
    assert_eq!(generated["status"], "candidate");
    assert!(!root.join("qualitygate.yaml").exists());
    let before = fs::read(root.join(generated["file"].as_str().unwrap())).unwrap();
    assert!(rule_schema::parse(&before).is_ok());
    let validated = report(&cli(root, &["rules", "validate", "--format", "json"]), 0);
    assert_eq!(validated["complete"], true);
    assert_eq!(validated["files"][0]["valid"], true);
    report(
        &cli(
            root,
            &[
                "rules",
                "generate",
                "--input",
                "candidate.yaml",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(
        fs::read(root.join(generated["file"].as_str().unwrap())).unwrap(),
        before
    );
    fs::write(
        root.join("AGENTS.md"),
        "## Test naming\nA changed requirement.\n",
    )
    .unwrap();
    let invalid = report(&cli(root, &["rules", "validate", "--format", "json"]), 1);
    assert_eq!(invalid["files"][0]["issues"][0]["stage"], "source");
    assert!(rule_authoring::generate(root, "candidate.yaml").is_err());
    fs::remove_file(root.join("AGENTS.md")).unwrap();
    let missing = report(&cli(root, &["rules", "validate", "--format", "json"]), 2);
    assert_eq!(missing["complete"], false);
    assert_eq!(missing["files"][0]["issues"][0]["stage"], "input");
}

#[test]
fn invalid_fields_have_schema_locations_and_cannot_bypass_snapshot_loading() {
    let repo = fixture();
    let root = repo.path();
    let rule = candidate(root, "rule", &["python"]);
    let mutations = [
        ("/schema_version", json!(2)),
        ("/version", json!(0)),
        ("/id", json!("../escape")),
        ("/source/content_hash", json!("sha256:123")),
        ("/language", json!(["all"])),
        ("/language", json!(["rust", "rust"])),
        ("/severity", json!("fatal")),
        ("/requires_capabilities", json!([])),
        ("/requires_capabilities", json!(["files"])),
        ("/when/entity", json!("class")),
        ("/then", json!({})),
        ("/then", json!({"require_marker":false})),
        ("/then", json!({"max_count":-1})),
        ("/then", json!({"require_dependency":{"artifact":"a"}})),
        (
            "/binding",
            json!({"marker":{"type":"annotation","name":"Generated"}}),
        ),
        ("/then", json!({"require_marker":true})),
    ];
    for (pointer, value) in mutations {
        let mut invalid = rule.clone();
        // Optional root fields do not exist in the candidate yet.
        if let Some(slot) = invalid.pointer_mut(pointer) {
            *slot = value;
        } else {
            invalid[pointer.trim_start_matches('/')] = value;
        }
        let bytes = serde_norway::to_string(&invalid).unwrap().into_bytes();
        let (parsed, issues) = rule_schema::inspect(&bytes).unwrap();
        assert!(parsed.is_none(), "accepted {pointer}: {invalid}");
        assert!(
            issues
                .iter()
                .any(|issue| issue.stage == "schema" && issue.schema_path.is_some()),
            "{issues:?}"
        );
        let config = config::Config {
            custom_rules: Some("qualitygate/rules".into()),
            ..Default::default()
        };
        assert!(
            config::catalog::Catalog::load(
                &config,
                [("qualitygate/rules/rule.yaml", bytes.as_slice())]
            )
            .is_err()
        );
    }
    let mut invalid = rule.clone();
    invalid["unknown"] = json!(true);
    save(root, "candidate.yaml", &invalid);
    let result = report(
        &cli(
            root,
            &["rules", "validate", "candidate.yaml", "--format", "json"],
        ),
        1,
    );
    assert_eq!(result["complete"], true);
    assert_eq!(result["files"][0]["issues"][0]["stage"], "schema");
    assert!(rule_authoring::generate(root, "candidate.yaml").is_err());
    assert!(!root.join("qualitygate/rules").exists());
    for pattern in ["(", "(?=unsupported)"] {
        invalid = rule.clone();
        invalid["then"]["name_pattern"] = json!(pattern);
        let (_, issues) = rule_schema::inspect(&serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert_eq!(issues[0].stage, "semantic");
    }
    for bytes in [
        b"id: first\nid: second\n".as_slice(),
        b"[unterminated",
        b"!custom tagged",
        b"value: .nan",
        b"1: value",
    ] {
        let (rule, issues) = rule_schema::inspect(bytes).unwrap();
        assert!(rule.is_none());
        assert_eq!(issues[0].stage, "yaml");
    }
    assert_eq!(
        rule_schema::inspect(&vec![b' '; config::MAX_CONFIG_BYTES + 1])
            .unwrap()
            .1[0]
            .stage,
        "input"
    );
}

#[test]
fn conditional_schema_accepts_every_supported_entity_and_marker_shape() {
    let repo = fixture();
    let root = repo.path();
    let base = candidate(root, "rule", &[]);
    for (entity, capability) in [
        ("test_method", "test_methods"),
        ("comment", "comments"),
        ("import", "imports"),
        ("file", "files"),
        ("commit", "commits"),
    ] {
        let mut rule = base.clone();
        rule["requires_capabilities"] = json!([capability]);
        rule["when"] = json!({"entity":entity});
        rule["then"] = json!({"max_count":0});
        let parsed = rule_schema::parse(&serde_json::to_vec(&rule).unwrap()).unwrap();
        rule_schema::parse(serde_norway::to_string(&parsed).unwrap().as_bytes()).unwrap();
        if ["commit", "comment", "import"].contains(&entity) {
            rule["when"]["change"] = json!("modified");
            assert!(rule_schema::parse(&serde_json::to_vec(&rule).unwrap()).is_err());
        }
        if entity == "commit" {
            rule["when"]["change"] = json!("added");
            rule["language"] = json!(["rust"]);
            assert!(rule_schema::parse(&serde_json::to_vec(&rule).unwrap()).is_err());
        }
    }
    for (kind, capability) in [
        ("annotation", "annotations"),
        ("comment", "comments"),
        ("git_trailer", "commits"),
    ] {
        let mut rule = base.clone();
        rule["requires_capabilities"] =
            json!(["test_methods", capability, "dependency_resolution"]);
        rule["binding"] = json!({"marker":{"type":kind,"name":"Generated","fields":["author"]}});
        rule["applies_to"] = json!({"provenance_scope":"all_added_tests"});
        rule["then"] = json!({"require_marker":true,"require_dependency":{"artifact":"example"}});
        let parsed = rule_schema::parse(&serde_json::to_vec(&rule).unwrap()).unwrap();
        rule_schema::parse(serde_norway::to_string(&parsed).unwrap().as_bytes()).unwrap();
    }
}

#[test]
fn validation_is_bounded_confined_and_rejects_duplicates_or_empty_packages() {
    let repo = fixture();
    let root = repo.path();
    report(&cli(root, &["rules", "validate", "--format", "json"]), 2);
    let rule = candidate(root, "rule", &[]);
    save(root, "qualitygate/rules/a.yaml", &rule);
    save(root, "qualitygate/rules/nested/b.yml", &rule);
    fs::write(root.join("qualitygate/rules/README.md"), "Ignored prose").unwrap();
    let duplicates = report(&cli(root, &["rules", "validate", "--format", "json"]), 1);
    assert_eq!(duplicates["files"].as_array().unwrap().len(), 2);
    assert!(
        duplicates["files"][1]["issues"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Duplicate")
    );
    for path in ["../escape", "missing.yaml", "AGENTS.md/invalid"] {
        report(
            &cli(root, &["rules", "validate", path, "--format", "json"]),
            2,
        );
        assert!(rule_authoring::generate(root, path).is_err());
    }
    for document in ["", "./AGENTS.md", "../AGENTS.md"] {
        assert!(rule_authoring::source(root, document, "Test naming").is_err());
    }
    assert!(rule_authoring::source(root, "AGENTS.md", "Missing").is_err());
    fs::write(
        root.join("oversize.yaml"),
        vec![b' '; config::MAX_CONFIG_BYTES + 1],
    )
    .unwrap();
    assert!(
        !rule_authoring::validate(root, "oversize.yaml")
            .unwrap()
            .complete
    );
    fs::create_dir(root.join("empty")).unwrap();
    fs::write(root.join("empty/README.md"), "No definitions").unwrap();
    assert!(!rule_authoring::validate(root, "empty").unwrap().complete);
    fs::create_dir(root.join("many")).unwrap();
    for index in 0..257 {
        fs::write(root.join(format!("many/{index}.yaml")), "{}").unwrap();
    }
    assert!(!rule_authoring::validate(root, "many").unwrap().complete);
    fs::create_dir(root.join("large")).unwrap();
    for index in 0..2 {
        fs::write(
            root.join(format!("large/{index}.yaml")),
            vec![b' '; 600_000],
        )
        .unwrap();
    }
    assert!(!rule_authoring::validate(root, "large").unwrap().complete);
}

#[cfg(unix)]
#[test]
fn symlink_inputs_and_output_ancestors_are_rejected() {
    use std::os::unix::fs::symlink;
    let repo = fixture();
    let root = repo.path();
    let outside = tempfile::tempdir().unwrap();
    let rule = candidate(root, "rule", &["rust"]);
    save(root, "candidate.yaml", &rule);
    symlink(root.join("candidate.yaml"), root.join("linked.yaml")).unwrap();
    assert!(
        !rule_authoring::validate(root, "linked.yaml")
            .unwrap()
            .complete
    );
    symlink(outside.path(), root.join("qualitygate")).unwrap();
    assert!(rule_authoring::generate(root, "candidate.yaml").is_err());
    assert!(!outside.path().join("rules").exists());
    assert!(config::rule_query::list(root, "qualitygate.yaml", Some("rust"), "project").is_err());
}

#[test]
fn generation_rejects_duplicate_ids_under_other_filenames_and_package_overflow() {
    let repo = fixture();
    let root = repo.path();
    let rule = candidate(root, "rule", &["rust"]);
    save(root, "candidate.yaml", &rule);
    save(root, "qualitygate/rules/nested/existing.yml", &rule);
    assert!(
        rule_authoring::generate(root, "candidate.yaml")
            .unwrap_err()
            .to_string()
            .contains("Duplicate")
    );
    assert!(!root.join("qualitygate/rules/rule.yaml").exists());
    for index in 0..255 {
        fs::write(root.join(format!("qualitygate/rules/{index}.yaml")), "{}").unwrap();
    }
    assert!(
        rule_authoring::generate(root, "candidate.yaml")
            .unwrap_err()
            .to_string()
            .contains("256")
    );
    let other = fixture();
    let root = other.path();
    let rule = candidate(root, "rule", &[]);
    save(root, "candidate.yaml", &rule);
    let mut large = rule.clone();
    large["id"] = json!("other");
    large["fix"] = json!("x".repeat(config::MAX_CONFIG_BYTES - 512));
    save(root, "qualitygate/rules/other.yaml", &large);
    assert!(
        fs::metadata(root.join("qualitygate/rules/other.yaml"))
            .unwrap()
            .len()
            <= config::MAX_CONFIG_BYTES as u64
    );
    assert!(
        rule_authoring::generate(root, "candidate.yaml")
            .unwrap_err()
            .to_string()
            .contains("1 MiB")
    );
    assert!(!root.join("qualitygate/rules/rule.yaml").exists());
}
