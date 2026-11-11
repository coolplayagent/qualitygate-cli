use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};

fn candidate(root: &Path, id: &str) -> serde_json::Value {
    std::fs::write(
        root.join("AGENTS.md"),
        "# Rules\nTests must describe behavior.\n# Other\nUnrelated.\n",
    )
    .unwrap();
    json!({"id":id,"version":1,"source":source(root,"AGENTS.md","Rules").unwrap(),
        "language":["rust"],"requires_capabilities":["test_methods"],"when":{"entity":"test_method"},
        "then":{"name_pattern":"^test_"},"fix":"Describe the behavior"})
}

fn write(root: &Path, path: &str, value: &serde_json::Value) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_norway::to_string(value).unwrap()).unwrap();
}

#[test]
fn generation_creates_source_bound_candidate_without_policy_adoption_or_overwrite() {
    let root = tempfile::tempdir().unwrap();
    let rule = candidate(root.path(), "meaningful-tests");
    write(root.path(), "candidate.yaml", &rule);
    let validation = validate(root.path(), "candidate.yaml").unwrap();
    assert_eq!(validation.decision, Decision::Pass);
    assert!(validation.complete);
    assert!(validation.files[0].valid);
    let generated = generate(root.path(), "candidate.yaml").unwrap();
    assert_eq!(generated["status"], "candidate");
    assert_eq!(generated["policy_changed"], false);
    assert_eq!(generated["review_trust"], "local_candidate");
    assert!(!root.path().join("qualitygate.yaml").exists());
    let path = generated["file"].as_str().unwrap();
    let bytes = std::fs::read(root.path().join(path)).unwrap();
    assert!(
        generate(root.path(), "candidate.yaml")
            .unwrap_err()
            .to_string()
            .contains("Duplicate custom rule id")
    );
    assert_eq!(std::fs::read(root.path().join(path)).unwrap(), bytes);
    assert_eq!(
        validate(root.path(), PROJECT_RULES_DIR).unwrap().decision,
        Decision::Pass
    );
    let rows = crate::rule_catalog::rule_query::list(
        root.path(),
        "qualitygate.yaml",
        Some("rust"),
        "project",
    )
    .unwrap();
    assert_eq!(rows["rules"][0]["enabled"], false);
    assert_eq!(rows["rules"][0]["id"], "meaningful-tests");
    let description = crate::rule_catalog::rule_query::describe(
        root.path(),
        "qualitygate.yaml",
        "meaningful-tests",
    )
    .unwrap();
    assert_eq!(description["implementation"], "project-dsl");
    assert_eq!(description["description"], "Describe the behavior");
    assert_eq!(description["parameters"], json!([]));
}

#[test]
fn invalid_rule_changed_source_and_missing_input_have_distinct_validation_outcomes() {
    let root = tempfile::tempdir().unwrap();
    let rule = candidate(root.path(), "meaningful-tests");
    write(root.path(), "candidate.yaml", &rule);
    std::fs::write(
        root.path().join("AGENTS.md"),
        "# Rules\nChanged obligation.\n",
    )
    .unwrap();
    let stale = validate(root.path(), "candidate.yaml").unwrap();
    assert!(stale.complete);
    assert_eq!(stale.decision, Decision::Fail);
    assert!(
        stale.files[0]
            .issues
            .iter()
            .any(|issue| issue.stage == "source")
    );
    assert!(generate(root.path(), "candidate.yaml").is_err());
    std::fs::remove_file(root.path().join("AGENTS.md")).unwrap();
    let missing = validate(root.path(), "candidate.yaml").unwrap();
    assert!(!missing.complete);
    assert_eq!(missing.decision, Decision::Incomplete);
    assert!(
        missing.files[0]
            .issues
            .iter()
            .any(|issue| issue.stage == "input")
    );
    std::fs::write(root.path().join("invalid.yaml"), "id: incomplete\n").unwrap();
    let invalid = validate(root.path(), "invalid.yaml").unwrap();
    assert!(invalid.complete);
    assert_eq!(invalid.decision, Decision::Fail);
    let absent = validate(root.path(), "absent").unwrap();
    assert!(!absent.complete);
    assert_eq!(absent.decision, Decision::Incomplete);
}

#[test]
fn discovery_is_sorted_bounded_and_duplicate_ids_cannot_be_silently_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let rule = candidate(root.path(), "duplicate");
    write(root.path(), "rules/z.yaml", &rule);
    write(root.path(), "rules/nested/a.yml", &rule);
    std::fs::write(root.path().join("rules/ignored.txt"), b"not a rule").unwrap();
    let report = validate(root.path(), "rules").unwrap();
    assert_eq!(report.decision, Decision::Fail);
    assert!(report.complete);
    assert_eq!(
        report
            .files
            .iter()
            .map(|file| file.file.as_str())
            .collect::<Vec<_>>(),
        ["rules/nested/a.yml", "rules/z.yaml"]
    );
    assert!(
        report.files[1]
            .issues
            .iter()
            .any(|issue| issue.message.contains("Duplicate custom rule id"))
    );
    let empty = root.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    assert!(files(root.path(), "empty", true).unwrap().is_empty());
    assert!(files(root.path(), "empty", false).is_err());
    for index in 0..257 {
        std::fs::write(empty.join(format!("{index}.yaml")), b"{}").unwrap();
    }
    assert!(
        files(root.path(), "empty", false)
            .unwrap_err()
            .to_string()
            .contains("256 files")
    );
    assert!(read_file(root.path(), "empty").is_err());
    std::fs::File::create(root.path().join("large.yaml"))
        .unwrap()
        .set_len(crate::config::MAX_CONFIG_BYTES as u64 + 1)
        .unwrap();
    assert!(
        read_file(root.path(), "large.yaml")
            .unwrap_err()
            .to_string()
            .contains("1 MiB")
    );
}

#[test]
fn normative_binding_ignores_unrelated_sections_and_rejects_ambiguous_or_unconfined_sources() {
    let root = tempfile::tempdir().unwrap();
    candidate(root.path(), "example");
    let expected = source(root.path(), "AGENTS.md", "Rules").unwrap();
    assert_eq!(
        expected.content_hash,
        format!(
            "sha256:{:x}",
            Sha256::digest(b"# Rules\nTests must describe behavior.\n")
        )
    );
    std::fs::write(
        root.path().join("AGENTS.md"),
        "# Rules\nTests must describe behavior.\n# Other\nChanged.\n",
    )
    .unwrap();
    assert_eq!(
        source(root.path(), "AGENTS.md", "Rules")
            .unwrap()
            .content_hash,
        expected.content_hash
    );
    for name in ["", "../AGENTS.md", "./AGENTS.md"] {
        assert!(source(root.path(), name, "Rules").is_err());
    }
    assert!(source(root.path(), "AGENTS.md", "Absent").is_err());
    std::fs::write(root.path().join("AGENTS.md"), "# Rules\nA\n# Rules\nB\n").unwrap();
    assert!(source(root.path(), "AGENTS.md", "Rules").is_err());
}

#[test]
fn package_resource_limits_report_incomplete_and_generation_leaves_existing_rules_intact() {
    let root = tempfile::tempdir().unwrap();
    let rule = candidate(root.path(), "new-rule");
    write(root.path(), "candidate.yaml", &rule);
    let large = format!(
        "{}\n#{}",
        serde_norway::to_string(&rule).unwrap(),
        "x".repeat(600_000)
    );
    std::fs::create_dir_all(root.path().join(PROJECT_RULES_DIR)).unwrap();
    for name in ["one.yaml", "two.yaml"] {
        std::fs::write(root.path().join(PROJECT_RULES_DIR).join(name), &large).unwrap();
    }
    let report = validate(root.path(), PROJECT_RULES_DIR).unwrap();
    assert_eq!(report.decision, Decision::Incomplete);
    assert!(report.issues.iter().any(|i| i.message.contains("1 MiB")));
    // Distinct rule identifiers let the aggregate byte bound, not duplicate detection, decide.
    for (name, id) in [("one.yaml", "old-one"), ("two.yaml", "old-two")] {
        std::fs::write(
            root.path().join(PROJECT_RULES_DIR).join(name),
            large.replace("new-rule", id),
        )
        .unwrap();
    }
    assert!(
        generate(root.path(), "candidate.yaml")
            .unwrap_err()
            .to_string()
            .contains("1 MiB")
    );
    assert!(
        !root
            .path()
            .join(PROJECT_RULES_DIR)
            .join("new-rule.yaml")
            .exists()
    );
    for n in 0..254 {
        std::fs::write(
            root.path()
                .join(PROJECT_RULES_DIR)
                .join(format!("{n}.yaml")),
            b"{}",
        )
        .unwrap();
    }
    assert!(
        generate(root.path(), "candidate.yaml")
            .unwrap_err()
            .to_string()
            .contains("256 files")
    );
    let discovery = root.path().join("discovery");
    std::fs::create_dir(&discovery).unwrap();
    for n in 0..4097 {
        std::fs::write(discovery.join(format!("{n}.txt")), b"").unwrap();
    }
    assert!(
        files(root.path(), "discovery", true)
            .unwrap_err()
            .to_string()
            .contains("discovery budget")
    );
}

#[test]
fn source_sections_removed_after_authoring_and_oversized_files_cannot_validate_as_pass() {
    let root = tempfile::tempdir().unwrap();
    let rule = candidate(root.path(), "unit");
    write(root.path(), "candidate.yaml", &rule);
    std::fs::write(
        root.path().join("AGENTS.md"),
        "# Renamed\nNo original section.\n",
    )
    .unwrap();
    let report = validate(root.path(), "candidate.yaml").unwrap();
    assert_eq!(report.decision, Decision::Fail);
    assert!(report.files[0].issues.iter().any(|i| i.stage == "source"));
    std::fs::File::create(root.path().join("oversize.yaml"))
        .unwrap()
        .set_len(crate::config::MAX_CONFIG_BYTES as u64 + 1)
        .unwrap();
    let report = validate(root.path(), "oversize.yaml").unwrap();
    assert_eq!(report.decision, Decision::Incomplete);
    assert!(!report.files[0].valid);
    assert_eq!(report.files[0].issues[0].stage, "input");
}

#[test]
fn mixed_operator_versions_publish_family_identity_and_semantic_errors_remain_violations() {
    let root = tempfile::tempdir().unwrap();
    for version in [2, 3] {
        let mut rule = candidate(root.path(), &format!("scenario-v{version}"));
        rule["schema_version"] = json!(version);
        rule["then"] = json!({});
        rule["expression"] = json!({"op":"compare","left":{"op":"count","input":{"op":"entities"}},"relation":"ge","right":{"op":"number","value":1}});
        write(root.path(), &format!("rules/v{version}.yaml"), &rule);
    }
    let report = validate(root.path(), "rules").unwrap();
    assert_eq!(report.decision, Decision::Pass);
    assert_eq!(report.schema_id, rule_schema::FAMILY_SCHEMA_ID);
    assert_eq!(report.files.len(), 2);
    for (index, version) in [2, 3].into_iter().enumerate() {
        let (id, digest) = rule_schema::schema_identity(version).unwrap();
        assert_eq!(report.files[index].schema_id.as_deref(), Some(id));
        assert_eq!(
            report.files[index].schema_digest.as_deref(),
            Some(digest.as_str())
        );
    }
    let mut invalid = candidate(root.path(), "contradictory-counts");
    invalid["then"] = json!({"min_count":2,"max_count":1});
    write(root.path(), "invalid.yaml", &invalid);
    let report = validate(root.path(), "invalid.yaml").unwrap();
    assert_eq!(report.decision, Decision::Fail);
    assert!(report.complete);
    assert!(
        report.files[0]
            .issues
            .iter()
            .any(|issue| issue.stage == "semantic" && issue.message.contains("min_count exceeds"))
    );
}
